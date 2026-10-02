//! Oynatıcı: çözme ve çıkış iş parçacıklarını yönetir, arayüzden gelen
//! komutları (aç, çal, duraklat, durdur) karşılar.
//!
//! Her açılan şarkı için bir "oturum" kurulur:
//!
//! ```text
//! çözme iş parçacığı ──► halka tampon (2 sn, f64) ──► çıkış iş parçacığı (WASAPI)
//!          └──────────── SharedState (atomik işaretler) ───────────┘
//! ```
//!
//! İş parçacıkları birbirleriyle yalnızca kilitsiz halka tampon ve atomik
//! değişkenler üzerinden konuşur; gerçek zamanlı çıkış hiçbir zaman beklemez.

use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Producer, RingBuffer};
use serde::Serialize;

use super::decode::{Decoder, TrackInfo};
use super::output::{self, OutputSpec};
use super::{AudioError, Sample};

/// Halka tamponun süresi. Çözme iş parçacığı bu kadar önden gider.
const RING_SECONDS: usize = 2;
/// Çıkış başlamadan önce tamponda olması istenen ses (takılmasız başlangıç için).
const PREFILL_SECONDS: f64 = 0.25;
/// Ön dolum için en fazla bu kadar beklenir.
const PREFILL_TIMEOUT: Duration = Duration::from_secs(3);
/// Tampon doluyken çözme iş parçacığının bekleme aralığı.
const DECODE_IDLE: Duration = Duration::from_millis(5);

/// İş parçacıkları arasında paylaşılan durum. Yalnızca atomik değişkenler
/// gerçek zamanlı yolda kullanılır; `error` kilidi yalnızca hata anında alınır.
#[derive(Debug, Default)]
pub struct SharedState {
    /// Kullanıcı duraklattı.
    pub paused: AtomicBool,
    /// Oturum kapanıyor; iş parçacıkları çıkmalı.
    pub stop: AtomicBool,
    /// Tamponda çalmaya başlamaya yetecek kadar veri var.
    pub prefilled: AtomicBool,
    /// Dosyanın sonuna gelindi (ya da çözme hatayla bitti).
    pub decode_done: AtomicBool,
    /// Son örnek de hoparlörden çıktı.
    pub ended: AtomicBool,
    /// Duyulan konum (kare cinsinden).
    pub frames_played: AtomicU64,
    /// Çalma sırasında veri yetişmediği tur sayısı (hedef: her zaman 0).
    pub underruns: AtomicU64,
    error: Mutex<Option<String>>,
}

impl SharedState {
    fn new(paused: bool) -> Self {
        let state = Self::default();
        state.paused.store(paused, Ordering::Release);
        state
    }

    /// İlk hatayı kaydeder (sonrakiler genellikle ilkinin sonucudur).
    pub fn fail(&self, message: String) {
        if let Ok(mut error) = self.error.lock() {
            error.get_or_insert(message);
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }
}

/// Oynatıcının durumu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    /// Şarkı açılmadı.
    Idle,
    Playing,
    Paused,
    /// Şarkı sonuna kadar çaldı.
    Ended,
    Error,
}

/// Arayüzün düzenli aralıklarla sorguladığı durum özeti.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackStatus {
    pub state: PlaybackState,
    pub track: Option<TrackInfo>,
    pub position_secs: f64,
    pub underruns: u64,
    pub error: Option<String>,
}

/// Bir şarkının çalınması için kurulan iş parçacıkları ve durumları.
struct Session {
    info: TrackInfo,
    shared: Arc<SharedState>,
    decode: Option<JoinHandle<()>>,
    output: Option<JoinHandle<()>>,
}

impl Session {
    fn start(path: &Path, autoplay: bool) -> Result<Self, AudioError> {
        let decoder = Decoder::open(path)?;
        let info = decoder.info().clone();
        let spec = OutputSpec {
            sample_rate: info.sample_rate,
            channels: info.channels,
        };
        let capacity = info.sample_rate as usize * info.channels * RING_SECONDS;
        let (producer, consumer) = RingBuffer::new(capacity);
        let shared = Arc::new(SharedState::new(!autoplay));

        let decode_shared = Arc::clone(&shared);
        let decode = std::thread::Builder::new()
            .name("lyraska-cozme".to_owned())
            .spawn(move || decode_loop(decoder, producer, &decode_shared))
            .map_err(|e| AudioError::Decode(e.to_string()))?;

        wait_for_prefill(&shared, PREFILL_TIMEOUT);

        let output = match output::spawn(spec, consumer, Arc::clone(&shared)) {
            Ok(handle) => handle,
            Err(error) => {
                shared.stop.store(true, Ordering::Release);
                let _ = decode.join();
                return Err(error);
            }
        };

        Ok(Self {
            info,
            shared,
            decode: Some(decode),
            output: Some(output),
        })
    }

    fn state(&self) -> PlaybackState {
        derive_state(
            self.shared.error().is_some(),
            self.shared.ended.load(Ordering::Acquire),
            self.shared.paused.load(Ordering::Acquire),
        )
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Release);
        // Önce çıkış: ses aygıtı hemen bırakılsın.
        if let Some(handle) = self.output.take() {
            let _ = handle.join();
        }
        if let Some(handle) = self.decode.take() {
            let _ = handle.join();
        }
    }
}

/// Oturum işaretlerinden kullanıcıya gösterilecek durumu çıkarır.
fn derive_state(has_error: bool, ended: bool, paused: bool) -> PlaybackState {
    if has_error {
        PlaybackState::Error
    } else if ended {
        PlaybackState::Ended
    } else if paused {
        PlaybackState::Paused
    } else {
        PlaybackState::Playing
    }
}

/// Arayüzün kullandığı oynatıcı. Tauri tarafında bir `Mutex` içinde tutulur.
#[derive(Default)]
pub struct Player {
    session: Option<Session>,
    /// Son açma girişimi başarısız olduysa nedeni.
    last_error: Option<String>,
}

impl Player {
    pub fn new() -> Self {
        Self::default()
    }

    /// Şarkıyı açar. `autoplay` ise hemen çalmaya başlar, değilse başında duraklatılmış bekler.
    pub fn load(&mut self, path: &Path, autoplay: bool) -> Result<TrackInfo, AudioError> {
        // Önceki oturumu kapat: ses aygıtı serbest kalsın, iki şarkı üst üste çalmasın.
        self.session = None;
        self.last_error = None;
        match Session::start(path, autoplay) {
            Ok(session) => {
                let info = session.info.clone();
                self.session = Some(session);
                Ok(info)
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Çalar ya da kaldığı yerden devam eder. Şarkı bittiyse baştan çalar.
    pub fn play(&mut self) -> Result<(), AudioError> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        match session.state() {
            PlaybackState::Ended | PlaybackState::Error => {
                let path = session.info.path.clone();
                self.load(&path, true).map(|_| ())
            }
            _ => {
                session.shared.paused.store(false, Ordering::Release);
                Ok(())
            }
        }
    }

    /// Duraklatır (ses ~10 ms içinde yumuşakça kısılır).
    pub fn pause(&mut self) {
        if let Some(session) = &self.session {
            session.shared.paused.store(true, Ordering::Release);
        }
    }

    /// Çalıyorsa duraklatır, değilse çalar.
    pub fn toggle(&mut self) -> Result<(), AudioError> {
        if self.state() == PlaybackState::Playing {
            self.pause();
            Ok(())
        } else {
            self.play()
        }
    }

    /// Durdurur ve şarkının başına döner (duraklatılmış olarak).
    pub fn stop(&mut self) -> Result<(), AudioError> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        let path = session.info.path.clone();
        self.load(&path, false).map(|_| ())
    }

    pub fn state(&self) -> PlaybackState {
        match &self.session {
            Some(session) => session.state(),
            None if self.last_error.is_some() => PlaybackState::Error,
            None => PlaybackState::Idle,
        }
    }

    pub fn status(&self) -> PlaybackStatus {
        match &self.session {
            Some(session) => {
                let frames = session.shared.frames_played.load(Ordering::Acquire);
                PlaybackStatus {
                    state: session.state(),
                    track: Some(session.info.clone()),
                    position_secs: frames as f64 / f64::from(session.info.sample_rate),
                    underruns: session.shared.underruns.load(Ordering::Relaxed),
                    error: session.shared.error(),
                }
            }
            None => PlaybackStatus {
                state: self.state(),
                track: None,
                position_secs: 0.0,
                underruns: 0,
                error: self.last_error.clone(),
            },
        }
    }
}

/// Çözme iş parçacığı: dosyayı çözer ve halka tamponu dolu tutar.
///
/// Tampon doluysa kısa aralıklarla bekler. Halka tampona her zaman tam kareler
/// yazılır; böylece çıkış tarafı kanalları hiçbir zaman karıştırmaz.
fn decode_loop(mut decoder: Decoder, mut producer: Producer<Sample>, shared: &SharedState) {
    let channels = decoder.info().channels;
    let prefill_target = ((f64::from(decoder.info().sample_rate) * PREFILL_SECONDS) as usize
        * channels)
        .min(producer.buffer().capacity() / 2);
    let mut pending: Vec<Sample> = Vec::new();
    let mut pos = 0;
    let mut pushed = 0usize;

    loop {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        if pos == pending.len() {
            match decoder.next_chunk() {
                Ok(Some(chunk)) => {
                    pending.clear();
                    pending.extend_from_slice(chunk);
                    pos = 0;
                    continue;
                }
                Ok(None) => break,
                Err(error) => {
                    shared.fail(error.to_string());
                    break;
                }
            }
        }

        let free = producer.slots() / channels * channels;
        let n = free.min(pending.len() - pos);
        if n == 0 {
            // Tampon dolu: çalmaya başlamak için kesinlikle yeterli.
            shared.prefilled.store(true, Ordering::Release);
            std::thread::sleep(DECODE_IDLE);
            continue;
        }
        if let Ok(chunk) = producer.write_chunk_uninit(n) {
            chunk.fill_from_iter(pending[pos..pos + n].iter().copied());
        }
        pos += n;
        pushed += n;
        if pushed >= prefill_target {
            shared.prefilled.store(true, Ordering::Release);
        }
    }
    shared.prefilled.store(true, Ordering::Release);
    shared.decode_done.store(true, Ordering::Release);
}

fn wait_for_prefill(shared: &SharedState, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while !shared.prefilled.load(Ordering::Acquire) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(2));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::{temp_path, write_wav};

    #[test]
    fn durum_onceligi_dogru() {
        assert_eq!(derive_state(true, true, true), PlaybackState::Error);
        assert_eq!(derive_state(false, true, true), PlaybackState::Ended);
        assert_eq!(derive_state(false, false, true), PlaybackState::Paused);
        assert_eq!(derive_state(false, false, false), PlaybackState::Playing);
    }

    #[test]
    fn yeni_oynatici_bos_durumda() {
        let player = Player::new();
        let status = player.status();
        assert_eq!(status.state, PlaybackState::Idle);
        assert!(status.track.is_none());
        assert_eq!(status.position_secs, 0.0);
    }

    #[test]
    fn cozme_dongusu_butun_ornekleri_sirasiyla_aktarir() {
        let path = temp_path("akis.wav");
        let frames = 30_000;
        // Her örnek kendi sırasını taşır; sıra bozulursa test yakalar.
        write_wav(&path, 8_000, 2, frames, |frame, channel| {
            ((frame * 2 + channel) % 2000) as f64 / 2000.0 - 0.5
        });
        let decoder = Decoder::open(&path).unwrap();

        // Küçük tampon: geri basınç (tampon dolu → bekle) yolunu da sınar.
        let (producer, mut consumer) = RingBuffer::<Sample>::new(1_001);
        let shared = Arc::new(SharedState::new(false));
        let thread_shared = Arc::clone(&shared);
        let handle = std::thread::spawn(move || decode_loop(decoder, producer, &thread_shared));

        let mut received = Vec::with_capacity(frames * 2);
        while received.len() < frames * 2 {
            match consumer.pop() {
                Ok(sample) => received.push(sample),
                Err(_) => std::thread::yield_now(),
            }
        }
        handle.join().unwrap();

        assert!(shared.decode_done.load(Ordering::Acquire));
        assert!(shared.prefilled.load(Ordering::Acquire));
        assert!(consumer.is_empty());
        for (i, sample) in received.iter().enumerate() {
            let expected = (i % 2000) as f64 / 2000.0 - 0.5;
            assert!(
                (sample - expected).abs() < 1e-4,
                "örnek {i}: {sample} != {expected}"
            );
        }
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn durdurma_isareti_cozme_dongusunu_bitirir() {
        let path = temp_path("durdur.wav");
        write_wav(&path, 8_000, 1, 80_000, |_, _| 0.1);
        let decoder = Decoder::open(&path).unwrap();
        let (producer, _consumer) = RingBuffer::<Sample>::new(100); // hiç boşalmayacak
        let shared = Arc::new(SharedState::new(false));
        let thread_shared = Arc::clone(&shared);
        let handle = std::thread::spawn(move || decode_loop(decoder, producer, &thread_shared));

        wait_for_prefill(&shared, Duration::from_secs(5));
        shared.stop.store(true, Ordering::Release);
        handle.join().unwrap();
        assert!(
            !shared.decode_done.load(Ordering::Acquire),
            "dosya sonuna gelinmemeli"
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn acilamayan_dosya_hata_durumuna_gecirir() {
        let mut player = Player::new();
        let error = player
            .load(Path::new("/olmayan/sarki.flac"), true)
            .err()
            .unwrap();
        assert!(matches!(error, AudioError::Open(_)));
        let status = player.status();
        assert_eq!(status.state, PlaybackState::Error);
        assert!(status.error.unwrap().contains("Dosya açılamadı"));

        // Oynatıcı bozulmaz: yeni komutlar sorunsuz çalışır.
        player.play().unwrap();
        player.stop().unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn windows_disinda_cikis_yok_hatasi_verir() {
        let path = temp_path("cikis.wav");
        write_wav(&path, 44_100, 2, 44_100, |_, _| 0.0);
        let mut player = Player::new();
        let error = player.load(&path, true).err().unwrap();
        assert!(matches!(error, AudioError::OutputUnavailable));
        assert_eq!(player.state(), PlaybackState::Error);
        std::fs::remove_file(path).ok();
    }
}
