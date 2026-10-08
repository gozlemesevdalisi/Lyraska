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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use rtrb::{Producer, RingBuffer};
use serde::Serialize;

use super::decode::{Decoder, TrackInfo};
use super::eq::{Design, EqControl, EqSettings};
use super::output::{self, DeviceInfo, OutputSpec};
use super::peq::{HeadphoneSettings, PeqControl};
use super::resample::Converter;
use super::{AudioError, Sample};
use crate::analysis::background::ForegroundGuard;
use crate::analysis::beats::BeatPosition;
use crate::analysis::cache::{AnalysisCache, FileStamp};
use crate::analysis::levels::ChannelLevels;
use crate::analysis::spectrogram::{self, Spectrogram, BANDS};
use crate::diagnostics;

/// Halka tamponun süresi. Çözme iş parçacığı bu kadar önden gider.
const RING_SECONDS: usize = 2;
/// Çıkış başlamadan önce tamponda olması istenen ses (takılmasız başlangıç için).
const PREFILL_SECONDS: f64 = 0.25;
/// Ön dolum için en fazla bu kadar beklenir.
const PREFILL_TIMEOUT: Duration = Duration::from_secs(3);
/// Tampon doluyken çözme iş parçacığının bekleme aralığı.
const DECODE_IDLE: Duration = Duration::from_millis(5);
/// Şarkı değiştirirken ya da sararken sesin kısılması için beklenen süre
/// (render geçişi + aygıt periyodu payı).
const CUT_FADE: Duration = Duration::from_millis(25);

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
    /// Oturum başından beri hoparlörden çıkan çıkış karesi sayısı (aygıt hızında).
    /// Çıkış iş parçacığı yazar; şarkı konumu buradan [`Segment`]'lerle hesaplanır.
    pub output_heard: AtomicU64,
    /// Oturumda çalınan şarkılar: boşluksuz geçişte çözme iş parçacığı yenisini ekler.
    /// Gerçek zamanlı yolda kullanılmaz.
    segments: Mutex<Vec<Segment>>,
    /// Boşluksuz geçiş için sıradaki şarkı; çözme iş parçacığı şarkı bitince alır.
    next_path: Mutex<Option<PathBuf>>,
    /// Çalma sırasında veri yetişmediği tur sayısı (hedef: her zaman 0).
    pub underruns: AtomicU64,
    /// Ekolayzer ayarları (oynatıcı boyunca aynı; her oturum paylaşır).
    pub eq: Arc<EqControl>,
    /// Kulaklık düzeltmesi (oynatıcı boyunca aynı; her oturum paylaşır).
    pub headphone: Arc<PeqControl>,
    error: Mutex<Option<String>>,
}

impl SharedState {
    fn new(paused: bool, first: Segment, eq: Arc<EqControl>, headphone: Arc<PeqControl>) -> Self {
        let state = Self {
            eq,
            headphone,
            segments: Mutex::new(vec![first]),
            ..Self::default()
        };
        state.paused.store(paused, Ordering::Release);
        state
    }

    /// Şu an duyulan şarkı ve o şarkıdaki konumu (saniye).
    fn now_playing(&self) -> Option<(TrackInfo, f64)> {
        let heard = self.output_heard.load(Ordering::Acquire);
        let segments = self.segments.lock().ok()?;
        let segment = segments
            .iter()
            .rev()
            .find(|s| s.first_output_frame <= heard)
            .or(segments.first())?;
        let frames = segment.start_frame as f64
            + heard.saturating_sub(segment.first_output_frame) as f64 * segment.rate_ratio;
        let mut seconds = frames.round() / f64::from(segment.info.sample_rate.max(1));
        if let Some(duration) = segment.info.duration_secs {
            seconds = seconds.min(duration);
        }
        Some((segment.info.clone(), seconds))
    }

    fn set_next(&self, path: Option<PathBuf>) {
        if let Ok(mut next) = self.next_path.lock() {
            *next = path;
        }
    }

    fn take_next(&self) -> Option<PathBuf> {
        self.next_path.lock().ok().and_then(|mut n| n.take())
    }

    fn push_segment(&self, segment: Segment) {
        if let Ok(mut segments) = self.segments.lock() {
            segments.push(segment);
        }
    }

    /// İlk hatayı kaydeder (sonrakiler genellikle ilkinin sonucudur).
    pub fn fail(&self, message: String) {
        if let Ok(mut error) = self.error.lock() {
            if error.is_none() {
                crate::diagnostics::error(&format!("Çalma hatası: {message}"));
            }
            error.get_or_insert(message);
        }
    }

    pub fn error(&self) -> Option<String> {
        self.error.lock().ok().and_then(|e| e.clone())
    }
}

/// Bir oturumda çalınan bir şarkı: çıkış akışında nerede başladığı ve konumun
/// nasıl hesaplanacağı.
#[derive(Debug, Clone)]
struct Segment {
    info: TrackInfo,
    /// Şarkının ilk karesinin çıkış akışındaki yeri (aygıt karesi).
    first_output_frame: u64,
    /// O noktadaki şarkı konumu (şarkı karesi; sarmada sıfırdan farklı).
    start_frame: u64,
    /// Bir çıkış karesinin kaç şarkı karesine denk geldiği (şarkı hızı / aygıt hızı).
    rate_ratio: f64,
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
    /// Şarkının temposu (BPM); analiz bitene kadar ya da belirgin ritim yoksa `None`.
    pub bpm: Option<f64>,
    /// Sesin aygıta giden yolu; şarkı açık değilse `None`.
    pub output: Option<SignalPath>,
}

/// Sesin şarkıdan hoparlöre giden yolu (profesyonel çalarlardaki "sinyal yolu").
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalPath {
    /// Aygıtın adı (ör. "Hoparlörler (Realtek(R) Audio)"); bilinmiyorsa boş.
    pub device_name: String,
    /// Aygıta giden örnekleme hızı ve kanal sayısı.
    pub sample_rate: u32,
    pub channels: usize,
    /// Şarkı aygıtın hızına Lyraska'nın yüksek kaliteli dönüştürücüsüyle çevriliyor mu?
    pub resampled: bool,
}

/// Şu an duyulan anın görsel verisi (önceden yapılmış analizden).
#[derive(Debug, Clone, PartialEq)]
pub struct VisualData {
    /// Verinin ait olduğu çalma konumu (saniye).
    pub seconds: f64,
    /// Frekans bantları (0..1), ekolayzerin etkisi eklenmiş.
    pub bands: [f32; BANDS],
    /// Sol/sağ kanal seviyeleri (ekolayzerden önce).
    pub levels: ChannelLevels,
    /// 0 VU'ya denk gelen seviye (dBFS); analiz bitene kadar `None`.
    pub vu_reference_db: Option<f32>,
    /// Tempo ve o anın vuruş ızgarasındaki yeri; analiz bitene kadar ya da
    /// belirgin ritim yoksa `None`.
    pub beat: Option<(f64, BeatPosition)>,
    /// Şarkı yapısından o an: vuruşun ölçüdeki yeri, enerji, bölüm (analiz bitince).
    pub structure: Option<StructureNow>,
    /// Görsel Yönetmen'in o anki notu (analiz bitince).
    pub director: Option<crate::director::DirectorFrame>,
}

/// Şarkı yapısında o an.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StructureNow {
    /// Vuruşun ölçüdeki yeri (1 = ölçü başı).
    pub bar_beat: Option<usize>,
    pub energy: Option<f32>,
    pub section: Option<usize>,
}

/// Bir şarkının çalınması için kurulan iş parçacıkları ve durumları.
struct Session {
    /// Oturumun açıldığı şarkı (boşluksuz geçişten sonra çalan şarkı farklı olabilir).
    info: TrackInfo,
    path: SignalPath,
    shared: Arc<SharedState>,
    decode: Option<JoinHandle<()>>,
    output: Option<JoinHandle<()>>,
}

impl Session {
    /// Oturumu başlatır. `start_secs` verilirse şarkının o noktasından başlar.
    fn start(
        path: &Path,
        autoplay: bool,
        start_secs: Option<f64>,
        next: Option<PathBuf>,
        eq: &Arc<EqControl>,
        headphone: &Arc<PeqControl>,
    ) -> Result<Self, AudioError> {
        let mut decoder = Decoder::open(path)?;
        let start_frame = match start_secs {
            Some(secs) if secs > 0.0 => decoder.seek(secs)?,
            _ => 0,
        };
        let info = decoder.info().clone();
        // Aygıtın kendi hızında çal: dönüştürmeyi Windows değil, biz yaparız.
        let device = output::device_info();
        let spec = OutputSpec {
            sample_rate: device.as_ref().map_or(info.sample_rate, |d| d.sample_rate),
            channels: Converter::output_channels(info.channels),
        };
        let converter = Converter::new(
            info.sample_rate,
            info.channels,
            spec.sample_rate,
            spec.channels,
        )?;
        let path = signal_path(device, &info, spec);
        let capacity = spec.sample_rate as usize * spec.channels * RING_SECONDS;
        let (producer, consumer) = RingBuffer::new(capacity);
        let first = Segment {
            info: info.clone(),
            first_output_frame: 0,
            start_frame,
            rate_ratio: f64::from(info.sample_rate) / f64::from(spec.sample_rate),
        };
        let shared = Arc::new(SharedState::new(
            !autoplay,
            first,
            Arc::clone(eq),
            Arc::clone(headphone),
        ));
        shared.set_next(next);

        let decode_shared = Arc::clone(&shared);
        let decode = std::thread::Builder::new()
            .name("lyraska-cozme".to_owned())
            .spawn(move || decode_loop(decoder, converter, spec, producer, &decode_shared))
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
            path,
            shared,
            decode: Some(decode),
            output: Some(output),
        })
    }

    /// Şu an duyulan şarkı ve konumu.
    fn now_playing(&self) -> (TrackInfo, f64) {
        self.shared
            .now_playing()
            .unwrap_or_else(|| (self.info.clone(), 0.0))
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

/// Ekranda gösterilecek sinyal yolu.
fn signal_path(device: Option<DeviceInfo>, info: &TrackInfo, spec: OutputSpec) -> SignalPath {
    SignalPath {
        device_name: device.map(|d| d.name).unwrap_or_default(),
        sample_rate: spec.sample_rate,
        channels: spec.channels,
        resampled: spec.sample_rate != info.sample_rate,
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

/// Bir şarkının arka planda süren (ya da biten) spektrum analizi.
struct Analysis {
    path: PathBuf,
    spectrogram: Arc<Spectrogram>,
    thread: Option<JoinHandle<()>>,
}

/// Analizlerin ortak bağlamı: önbellek ve oynatıcının süren analiz sayacı
/// (arka plan analizi bu sayaç sıfırdan büyükken bekler).
#[derive(Clone, Default)]
struct AnalysisContext {
    cache: Option<Arc<AnalysisCache>>,
    foreground: Arc<AtomicUsize>,
}

impl Analysis {
    /// Şarkının analizini başlatır. Önbellekte geçerli kaydı varsa anında hazırdır.
    /// `after` verilirse (sıradaki şarkı) o analiz bitene kadar bekler: önce çalan şarkı.
    fn start(
        path: &Path,
        context: &AnalysisContext,
        after: Option<Arc<Spectrogram>>,
    ) -> Option<Self> {
        let stamp = FileStamp::of(path);
        if let (Some(cache), Some(stamp)) = (&context.cache, stamp) {
            match cache.load(path, stamp) {
                Ok(Some(saved)) => {
                    return Some(Self {
                        path: path.to_path_buf(),
                        spectrogram: Spectrogram::from_saved(saved),
                        thread: None,
                    })
                }
                Ok(None) => {}
                Err(e) => diagnostics::error(&e.to_string()),
            }
        }
        let decoder = Decoder::open(path).ok()?;
        let spectrogram = Spectrogram::new();
        let target = Arc::clone(&spectrogram);
        let busy = ForegroundGuard::new(&context.foreground);
        let cache = context.cache.clone();
        let file = path.to_path_buf();
        let thread = std::thread::Builder::new()
            .name("lyraska-analiz".to_owned())
            .spawn(move || {
                let _busy = busy;
                if let Some(first) = after {
                    while !first.is_done() && !first.is_cancelled() && !target.is_cancelled() {
                        std::thread::sleep(Duration::from_millis(20));
                    }
                }
                spectrogram::analyze(decoder, &target);
                if let (Some(cache), Some(stamp), Some(saved)) = (cache, stamp, target.saved()) {
                    if let Err(e) = cache.store(&file, stamp, &saved) {
                        diagnostics::error(&e.to_string());
                    }
                }
            })
            .ok()?;
        Some(Self {
            path: path.to_path_buf(),
            spectrogram,
            thread: Some(thread),
        })
    }
}

impl Drop for Analysis {
    fn drop(&mut self) {
        self.spectrogram.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Arayüzün kullandığı oynatıcı. Tauri tarafında bir `Mutex` içinde tutulur.
#[derive(Default)]
pub struct Player {
    session: Option<Session>,
    /// Çalan şarkının spektrumu. Sarma ve durdurmada (aynı şarkı) korunur.
    analysis: Option<Analysis>,
    /// Boşluksuz geçiş için sıradaki şarkı ve önceden başlatılan analizi.
    next_path: Option<PathBuf>,
    next_analysis: Option<Analysis>,
    /// Son açma girişimi başarısız olduysa nedeni.
    last_error: Option<String>,
    /// Ekolayzer ayarları; bütün oturumların ses çıkışı buradan okur.
    eq: Arc<EqControl>,
    /// Kulaklık düzeltmesi: ses çıkışının okuduğu kanal ve son ayar.
    headphone_control: Arc<PeqControl>,
    headphone: HeadphoneSettings,
    /// Epilepsi güvenli modu: Görsel Yönetmen nabızları saniyede en fazla bire indirir.
    visual_safe: bool,
    /// Analiz önbelleği ve süren analiz sayacı.
    analysis_context: AnalysisContext,
    /// Ses aygıtının ek gecikmesi (ms): görseller bu kadar geriden okunur.
    audio_delay_ms: i32,
    /// Görsellere uygulanan ekolayzer kazançları (dB, spektrum bantları için),
    /// hangi ayar sürümü ve örnekleme hızı için hesaplandığıyla birlikte.
    visual_eq: Option<(u64, u32, [f64; BANDS])>,
}

impl Player {
    pub fn new() -> Self {
        Self::default()
    }

    /// Şarkıyı açar. `autoplay` ise hemen çalmaya başlar, değilse başında duraklatılmış bekler.
    pub fn load(&mut self, path: &Path, autoplay: bool) -> Result<TrackInfo, AudioError> {
        self.restart(path, autoplay, None)
    }

    /// Oturumu yeniden kurar (yeni şarkı, sarma, durdurma için ortak yol).
    fn restart(
        &mut self,
        path: &Path,
        autoplay: bool,
        start_secs: Option<f64>,
    ) -> Result<TrackInfo, AudioError> {
        // Çalan sesi önce yumuşakça kıs, sonra oturumu kapat: "tık" sesi olmasın.
        if let Some(session) = &self.session {
            if session.state() == PlaybackState::Playing {
                session.shared.paused.store(true, Ordering::Release);
                std::thread::sleep(CUT_FADE);
            }
        }
        // Önceki oturumu kapat: ses aygıtı serbest kalsın, iki şarkı üst üste çalmasın.
        self.session = None;
        self.last_error = None;
        // Sıradaki şarkı açılan şarkının kendisiyse (boşluksuz geçiş sonrası) artık sırada değil.
        if self.next_path.as_deref() == Some(path) {
            self.next_path = None;
        }
        match Session::start(
            path,
            autoplay,
            start_secs,
            self.next_path.clone(),
            &self.eq,
            &self.headphone_control,
        ) {
            Ok(session) => {
                let info = session.info.clone();
                self.session = Some(session);
                self.adopt_analysis(path);
                Ok(info)
            }
            Err(error) => {
                self.last_error = Some(error.to_string());
                Err(error)
            }
        }
    }

    /// Boşluksuz geçiş için sıradaki şarkıyı bildirir (`None`: sıra yok).
    /// Çalan şarkı bitince ses akışı kesilmeden bu şarkıyla devam eder; analizi de
    /// şimdiden başlar.
    pub fn set_next(&mut self, path: Option<PathBuf>) {
        if self.next_path == path {
            return;
        }
        self.next_path = path.clone();
        if let Some(session) = &self.session {
            session.shared.set_next(path.clone());
        }
        self.next_analysis = match &path {
            Some(p) if self.next_analysis.as_ref().is_some_and(|a| &a.path == p) => {
                self.next_analysis.take()
            }
            Some(p) if self.analysis.as_ref().is_none_or(|a| &a.path != p) => {
                // Sıradaki şarkı, çalanın analizi bitince analiz edilir.
                let current = self.analysis.as_ref().map(|a| Arc::clone(&a.spectrogram));
                Analysis::start(p, &self.analysis_context, current)
            }
            _ => None,
        };
    }

    /// Analiz önbelleğini bağlar: analizler önce önbellekten okunur, bitenler yazılır.
    pub fn set_analysis_cache(&mut self, cache: Arc<AnalysisCache>) {
        self.analysis_context.cache = Some(cache);
    }

    /// Oynatıcının süren analiz sayacı (arka plan analizi bununla bekler).
    pub fn foreground_analyses(&self) -> Arc<AtomicUsize> {
        Arc::clone(&self.analysis_context.foreground)
    }

    /// Analizi verilen şarkınınkine geçirir: önceden başlatılmışsa onu kullanır.
    fn adopt_analysis(&mut self, path: &Path) {
        if self.analysis.as_ref().is_some_and(|a| a.path == path) {
            return;
        }
        if self.next_analysis.as_ref().is_some_and(|a| a.path == path) {
            self.analysis = self.next_analysis.take();
        } else {
            self.analysis = None;
            self.analysis = Analysis::start(path, &self.analysis_context, None);
        }
    }

    /// Çalan şarkının analizi (boşluksuz geçişten hemen sonra önceden başlatılan da olabilir).
    fn analysis_for(&self, path: &Path) -> Option<&Analysis> {
        [&self.analysis, &self.next_analysis]
            .into_iter()
            .flatten()
            .find(|a| a.path == path)
    }

    /// Çalar ya da kaldığı yerden devam eder. Şarkı bittiyse baştan çalar.
    pub fn play(&mut self) -> Result<(), AudioError> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        match session.state() {
            PlaybackState::Ended | PlaybackState::Error => {
                let path = session.now_playing().0.path;
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

    /// Şarkıda verilen saniyeye atlar. Çalıyorsa çalmaya devam eder, değilse
    /// yeni noktada duraklatılmış bekler.
    pub fn seek(&mut self, seconds: f64) -> Result<(), AudioError> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        let path = session.now_playing().0.path;
        let resume = session.state() == PlaybackState::Playing;
        self.restart(&path, resume, Some(seconds)).map(|_| ())
    }

    /// Durdurur ve şarkının başına döner (duraklatılmış olarak).
    pub fn stop(&mut self) -> Result<(), AudioError> {
        let Some(session) = &self.session else {
            return Ok(());
        };
        let path = session.now_playing().0.path;
        self.load(&path, false).map(|_| ())
    }

    pub fn state(&self) -> PlaybackState {
        match &self.session {
            Some(session) => session.state(),
            None if self.last_error.is_some() => PlaybackState::Error,
            None => PlaybackState::Idle,
        }
    }

    /// Kulaklık düzeltmesi ayarı.
    pub fn headphone(&self) -> HeadphoneSettings {
        self.headphone.clone()
    }

    /// Kulaklık düzeltmesini değiştirir; çalan ses ~30 ms içinde yumuşakça geçer.
    pub fn set_headphone(&mut self, settings: HeadphoneSettings) -> HeadphoneSettings {
        let settings = settings.sanitized();
        self.headphone_control
            .set(settings.profile.as_ref(), settings.enabled);
        self.headphone = settings.clone();
        settings
    }

    /// Ekolayzer ayarları.
    pub fn equalizer(&self) -> EqSettings {
        self.eq.settings()
    }

    /// Ekolayzer ayarlarını değiştirir; çalan ses ~40 ms içinde yumuşakça uyar.
    /// Düzeltilmiş (±12 dB'ye kırpılmış) ayarları döndürür.
    pub fn set_equalizer(&mut self, settings: EqSettings) -> EqSettings {
        self.eq.set(settings)
    }

    /// Şu an duyulan anın görsel verisi. Analiz o ana yetişmediyse `None`.
    /// Spektrum ekolayzerden önce çıkarıldığı için ekolayzerin etkisi burada eklenir:
    /// görseller duyulanı gösterir.
    pub fn visual_now(&mut self) -> Option<VisualData> {
        let (track, heard) = self.session.as_ref()?.now_playing();
        // Gecikme telafisi: ekranda görüneceği an ve ses aygıtının ek gecikmesi.
        let seconds = crate::visual_bridge::visual_time(heard, self.audio_delay_ms);
        // Boşluksuz geçişle şarkı değiştiyse analizi de değiştir.
        self.adopt_analysis(&track.path);
        let rate = track.sample_rate;
        let spectrogram = &self.analysis.as_ref()?.spectrogram;
        let mut bands = spectrogram.frame_at(seconds)?;
        let levels = spectrogram.meters_at(seconds)?;
        let vu_reference_db = spectrogram.vu_reference_db();
        let beat = spectrogram
            .beat_grid()
            .and_then(|grid| Some((grid.bpm, grid.position_at(seconds)?)));
        let director = spectrogram
            .choreography()
            .map(|c| c.frame_at(seconds, self.visual_safe));
        let structure = spectrogram.song_map().map(|map| StructureNow {
            bar_beat: beat.map(|(_, position)| map.bar_beat(position.index)),
            energy: map.energy_at(seconds),
            section: map.section_at(seconds),
        });
        let offsets = self.visual_eq_offsets(rate);
        for (level, &db) in bands.iter_mut().zip(&offsets) {
            *level = spectrogram::shift_level(*level, db);
        }
        Some(VisualData {
            seconds,
            bands,
            levels,
            vu_reference_db,
            beat,
            structure,
            director,
        })
    }

    /// Ses aygıtının ek gecikmesi (ms).
    pub fn audio_delay_ms(&self) -> i32 {
        self.audio_delay_ms
    }

    /// Ses gecikmesini ayarlar (sınırlar içine alınır) ve uygulanan değeri döndürür.
    pub fn set_audio_delay_ms(&mut self, ms: i32) -> i32 {
        self.audio_delay_ms = crate::visual_bridge::clamp_audio_delay_ms(ms);
        self.audio_delay_ms
    }

    /// Epilepsi güvenli modu açık mı?
    pub fn visual_safe(&self) -> bool {
        self.visual_safe
    }

    pub fn set_visual_safe(&mut self, safe: bool) {
        self.visual_safe = safe;
    }

    /// Çalan şarkının yapısı (analiz bittiyse).
    pub fn song_map(&self) -> Option<Arc<crate::analysis::structure::SongMap>> {
        let (track, _) = self.session.as_ref()?.now_playing();
        self.analysis_for(&track.path)?.spectrogram.song_map()
    }

    /// Ekolayzerin spektrum bantlarındaki toplam kazancı (ön kazanç dahil, dB).
    /// Ayar ya da örnekleme hızı değişmedikçe yeniden hesaplanmaz.
    fn visual_eq_offsets(&mut self, rate: u32) -> [f64; BANDS] {
        let version = self.eq.version();
        if let Some((v, r, offsets)) = self.visual_eq {
            if v == version && r == rate {
                return offsets;
            }
        }
        let settings = self.eq.settings();
        let offsets = if settings.is_active() {
            let rate_hz = f64::from(rate);
            let design = Design::new(&settings.gains_db, rate_hz);
            spectrogram::band_centers(rate_hz)
                .map(|hz| design.response_db(hz, rate_hz) + design.preamp_db)
        } else {
            [0.0; BANDS]
        };
        self.visual_eq = Some((version, rate, offsets));
        offsets
    }

    pub fn status(&self) -> PlaybackStatus {
        match &self.session {
            Some(session) => {
                let (track, position_secs) = session.now_playing();
                let bpm = self
                    .analysis_for(&track.path)
                    .and_then(|a| a.spectrogram.beat_grid())
                    .map(|grid| grid.bpm);
                let mut output = session.path.clone();
                output.resampled = output.sample_rate != track.sample_rate;
                PlaybackStatus {
                    state: session.state(),
                    track: Some(track),
                    position_secs,
                    underruns: session.shared.underruns.load(Ordering::Relaxed),
                    error: session.shared.error(),
                    bpm,
                    output: Some(output),
                }
            }
            None => PlaybackStatus {
                state: self.state(),
                track: None,
                position_secs: 0.0,
                underruns: 0,
                error: self.last_error.clone(),
                bpm: None,
                output: None,
            },
        }
    }
}

/// Çözme iş parçacığı: dosyayı çözer ve halka tamponu dolu tutar.
///
/// Tampon doluysa kısa aralıklarla bekler. Halka tampona her zaman tam kareler
/// yazılır; böylece çıkış tarafı kanalları hiçbir zaman karıştırmaz.
fn decode_loop(
    decoder: Decoder,
    converter: Converter,
    spec: OutputSpec,
    mut producer: Producer<Sample>,
    shared: &SharedState,
) {
    let channels = spec.channels;
    let prefill_target = ((f64::from(spec.sample_rate) * PREFILL_SECONDS) as usize * channels)
        .min(producer.buffer().capacity() / 2);
    let mut pending: Vec<Sample> = Vec::new();
    let mut pos = 0;
    let mut pushed = 0usize;
    let mut finished = false;
    let mut decoder = decoder;
    let mut converter = converter;

    loop {
        if shared.stop.load(Ordering::Acquire) {
            return;
        }
        if pos == pending.len() {
            if finished {
                // Boşluksuz geçiş: sıradaki şarkı aynı akışa eklenir. Önceki şarkının
                // son örneği tampona yazıldı; yenisinin ilki hemen ardından gelir.
                match open_next(shared, spec, (pushed / channels) as u64) {
                    Some((next_decoder, next_converter)) => {
                        decoder = next_decoder;
                        converter = next_converter;
                        finished = false;
                        continue;
                    }
                    None => break,
                }
            }
            // Çözülen dilim aygıt biçimine çevrilir; şarkı bitince dönüştürücünün
            // içinde kalanlar da alınır.
            let converted = match decoder.next_chunk() {
                Ok(Some(chunk)) => converter.process(chunk),
                Ok(None) => {
                    finished = true;
                    converter.finish()
                }
                Err(error) => {
                    shared.fail(error.to_string());
                    break;
                }
            };
            match converted {
                Ok(samples) => {
                    pending.clear();
                    pending.extend_from_slice(samples);
                    pos = 0;
                }
                Err(error) => {
                    shared.fail(error.to_string());
                    break;
                }
            }
            continue;
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

/// Sıradaki şarkıyı açar ve oturuma ekler. Sıra yoksa, açılamazsa ya da kanal
/// sayısı akışa uymuyorsa `None` (şarkı normal biçimde biter).
fn open_next(
    shared: &SharedState,
    spec: OutputSpec,
    first_output_frame: u64,
) -> Option<(Decoder, Converter)> {
    let path = shared.take_next()?;
    let decoder = Decoder::open(&path).ok()?;
    let info = decoder.info().clone();
    if Converter::output_channels(info.channels) != spec.channels {
        return None;
    }
    let converter = Converter::new(
        info.sample_rate,
        info.channels,
        spec.sample_rate,
        spec.channels,
    )
    .ok()?;
    shared.push_segment(Segment {
        rate_ratio: f64::from(info.sample_rate) / f64::from(spec.sample_rate),
        info,
        first_output_frame,
        start_frame: 0,
    });
    Some((decoder, converter))
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

    fn test_segment() -> Segment {
        Segment {
            info: TrackInfo {
                path: PathBuf::from("deneme.wav"),
                file_name: "deneme".into(),
                title: None,
                artist: None,
                album: None,
                album_artist: None,
                track_number: None,
                disc_number: None,
                codec: "pcm".into(),
                sample_rate: 8_000,
                channels: 2,
                duration_secs: None,
            },
            first_output_frame: 0,
            start_frame: 0,
            rate_ratio: 1.0,
        }
    }

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
        let shared = Arc::new(SharedState::new(
            false,
            test_segment(),
            Arc::default(),
            Arc::default(),
        ));
        let thread_shared = Arc::clone(&shared);
        let converter = Converter::new(8_000, 2, 8_000, 2).unwrap();
        let spec = OutputSpec {
            sample_rate: 8_000,
            channels: 2,
        };
        let handle = std::thread::spawn(move || {
            decode_loop(decoder, converter, spec, producer, &thread_shared)
        });

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
        let shared = Arc::new(SharedState::new(
            false,
            test_segment(),
            Arc::default(),
            Arc::default(),
        ));
        let thread_shared = Arc::clone(&shared);
        let converter = Converter::new(8_000, 1, 8_000, 2).unwrap();
        let spec = OutputSpec {
            sample_rate: 8_000,
            channels: 2,
        };
        let handle = std::thread::spawn(move || {
            decode_loop(decoder, converter, spec, producer, &thread_shared)
        });

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

    /// Koşul sağlanana kadar (en fazla 5 sn) bekler.
    fn wait_until(mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(2));
        }
        false
    }

    /// Her karesi kendi zamanını taşıyan 2 saniyelik mono test şarkısı.
    fn ramp_song(name: &str) -> PathBuf {
        let path = temp_path(name);
        write_wav(&path, 8_000, 1, 16_000, |frame, _| frame as f64 / 16_000.0);
        path
    }

    #[test]
    fn bosluksuz_geciste_ornekler_ara_vermeden_surer() {
        // A: 0..3000, B: 3000..5000 değerlerini taşıyan iki şarkı. Halka tampona
        // aralarında tek örnek bile eksik ya da fazla olmadan art arda gelmeli.
        let a = temp_path("bosluksuz-a.wav");
        let b = temp_path("bosluksuz-b.wav");
        write_wav(&a, 8_000, 2, 3_000, |f, _| f as f64 / 10_000.0);
        write_wav(&b, 8_000, 2, 2_000, |f, _| (3_000 + f) as f64 / 10_000.0);
        let decoder = Decoder::open(&a).unwrap();
        let (producer, mut consumer) = RingBuffer::<Sample>::new(777);
        let shared = Arc::new(SharedState::new(
            false,
            test_segment(),
            Arc::default(),
            Arc::default(),
        ));
        shared.set_next(Some(b.clone()));
        let thread_shared = Arc::clone(&shared);
        let converter = Converter::new(8_000, 2, 8_000, 2).unwrap();
        let spec = OutputSpec {
            sample_rate: 8_000,
            channels: 2,
        };
        let handle = std::thread::spawn(move || {
            decode_loop(decoder, converter, spec, producer, &thread_shared)
        });
        let mut received = Vec::new();
        while !(shared.decode_done.load(Ordering::Acquire) && consumer.is_empty()) {
            match consumer.pop() {
                Ok(sample) => received.push(sample),
                Err(_) => std::thread::yield_now(),
            }
        }
        handle.join().unwrap();
        assert_eq!(received.len(), 5_000 * 2);
        for (i, pair) in received.chunks(2).enumerate() {
            assert!(
                (pair[0] - i as f64 / 10_000.0).abs() < 1e-4,
                "kare {i}: {}",
                pair[0]
            );
        }
        // İkinci şarkı, çıkış akışının 3000. karesinde başlar.
        let segments = shared.segments.lock().unwrap().clone();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[1].first_output_frame, 3_000);
        assert_eq!(segments[1].info.path, b);
        // Konum hesabı: sınırdan önce A'da, sonra B'nin başında.
        shared.output_heard.store(2_999, Ordering::Release);
        assert_eq!(
            shared.now_playing().unwrap().0.path,
            PathBuf::from("deneme.wav")
        );
        shared.output_heard.store(3_400, Ordering::Release);
        let (track, seconds) = shared.now_playing().unwrap();
        assert_eq!(track.path, b);
        assert!((seconds - 0.05).abs() < 1e-9, "{seconds}");
        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
    }

    #[test]
    fn oynatici_siradakine_kesintisiz_gecer() {
        // 8 kHz şarkılar, 48 kHz sanal aygıt: geçiş, aygıt hızına çevrilen akışta olur.
        output::simulated::set_device_rate(Some(48_000));
        let a = ramp_song("gecis-a.wav");
        let b = ramp_song("gecis-b.wav");
        let mut player = Player::new();
        player.load(&a, true).unwrap();
        player.set_next(Some(b.clone()));
        assert!(wait_until(|| player
            .status()
            .track
            .is_some_and(|t| t.path == b)));
        assert_ne!(
            player.state(),
            PlaybackState::Ended,
            "ilk şarkı bitince durmaz"
        );
        assert!(wait_until(|| player.state() == PlaybackState::Ended));
        let status = player.status();
        assert_eq!(status.track.unwrap().path, b);
        assert!(
            (status.position_secs - 2.0).abs() < 1e-9,
            "{}",
            status.position_secs
        );
        assert_eq!(status.underruns, 0);
        // Görseller de yeni şarkının analizine geçer.
        // (Şarkı sonunda görsel kare yok; çağrı yine de analizi yeni şarkıya geçirir.)
        player.visual_now();
        assert_eq!(player.analysis.as_ref().unwrap().path, b);
        output::simulated::set_device_rate(None);
        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
    }

    #[test]
    fn kanal_sayisi_uymayan_siradaki_sarkiya_gecilmez() {
        let a = ramp_song("kanal-a.wav");
        let b = temp_path("kanal-b.wav");
        write_wav(&b, 8_000, 3, 8_000, |_, _| 0.1); // 3 kanal: stereo akışa eklenemez
        let mut player = Player::new();
        player.load(&a, true).unwrap();
        player.set_next(Some(b.clone()));
        assert!(wait_until(|| player.state() == PlaybackState::Ended));
        assert_eq!(
            player.status().track.unwrap().path,
            a,
            "ilk şarkı normal biter"
        );
        std::fs::remove_file(a).ok();
        std::fs::remove_file(b).ok();
    }

    #[test]
    fn sarki_sonuna_kadar_calar_ve_bitti_durumuna_gecer() {
        let path = ramp_song("sona-kadar.wav");
        let mut player = Player::new();
        let info = player.load(&path, true).unwrap();
        assert_eq!(info.sample_rate, 8_000);
        assert!(wait_until(|| player.state() == PlaybackState::Ended));
        let status = player.status();
        assert!(
            (status.position_secs - 2.0).abs() < 1e-9,
            "{}",
            status.position_secs
        );

        // Bitince "çal" baştan başlatır.
        player.play().unwrap();
        assert!(matches!(
            player.state(),
            PlaybackState::Playing | PlaybackState::Ended
        ));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn aygitin_hizina_cevirip_calar_ve_konumu_dogru_sayar() {
        // Şarkı 8 kHz, sanal aygıt 48 kHz: ses aygıtın hızına çevrilir; konum
        // yine şarkı zamanıyla ilerler ve tam şarkı süresinde biter.
        output::simulated::set_device_rate(Some(48_000));
        let path = ramp_song("aygit-hizi.wav");
        let mut player = Player::new();
        player.load(&path, true).unwrap();
        let path_info = player.status().output.unwrap();
        assert_eq!(path_info.sample_rate, 48_000);
        assert_eq!(path_info.channels, 2);
        assert!(path_info.resampled);
        assert_eq!(path_info.device_name, "Sanal aygıt");
        assert!(wait_until(|| player.state() == PlaybackState::Ended));
        let status = player.status();
        assert!(
            (status.position_secs - 2.0).abs() < 1e-9,
            "{}",
            status.position_secs
        );
        assert_eq!(status.underruns, 0);
        output::simulated::set_device_rate(None);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn duraklatilmisken_sarinca_yeni_noktada_bekler() {
        let path = ramp_song("sar-duraklat.wav");
        let mut player = Player::new();
        player.load(&path, false).unwrap();
        assert_eq!(player.state(), PlaybackState::Paused);

        player.seek(1.5).unwrap();
        assert_eq!(player.state(), PlaybackState::Paused);
        std::thread::sleep(Duration::from_millis(30));
        assert!((player.status().position_secs - 1.5).abs() < 1e-9);

        // Devam edince 1,5 saniyeden sona kadar çalar.
        player.play().unwrap();
        assert!(wait_until(|| player.state() == PlaybackState::Ended));
        assert!((player.status().position_secs - 2.0).abs() < 1e-9);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn calarken_sarinca_calmaya_devam_eder() {
        let path = ramp_song("sar-cal.wav");
        let mut player = Player::new();
        player.load(&path, true).unwrap();
        player.seek(0.5).unwrap();
        assert!(player.status().position_secs >= 0.5);
        assert!(matches!(
            player.state(),
            PlaybackState::Playing | PlaybackState::Ended
        ));
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn durdur_basa_sarar_ve_duraklatir() {
        let path = ramp_song("durdur.wav");
        let mut player = Player::new();
        player.load(&path, true).unwrap();
        assert!(wait_until(|| player.status().position_secs > 0.5));
        player.stop().unwrap();
        assert_eq!(player.state(), PlaybackState::Paused);
        assert_eq!(player.status().position_secs, 0.0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn spektrum_calan_ana_karsilik_gelir_ve_sarmada_korunur() {
        let path = ramp_song("spektrum.wav");
        let mut player = Player::new();
        player.load(&path, false).unwrap();
        let first = Arc::clone(&player.analysis.as_ref().unwrap().spectrogram);
        assert!(wait_until(|| first.is_done()));

        player.seek(1.0).unwrap();
        let visual = player.visual_now().unwrap();
        let (seconds, bands) = (visual.seconds, visual.bands);
        assert!(
            visual.vu_reference_db.is_some(),
            "analiz bitti: referans hazır"
        );
        // Görseller ekranda görünecekleri an için okunur (ekran gecikmesi kadar ileri).
        let lead = crate::visual_bridge::DISPLAY_LEAD_SECONDS;
        assert!((seconds - (1.0 + lead)).abs() < 1e-9, "{seconds}");
        assert_eq!(bands.len(), BANDS);
        // Ses aygıtı 200 ms geç çalıyorsa görseller o kadar geriden okunur.
        assert_eq!(player.set_audio_delay_ms(200), 200);
        let delayed = player.visual_now().unwrap().seconds;
        assert!((delayed - (1.0 + lead - 0.2)).abs() < 1e-9, "{delayed}");
        assert_eq!(player.set_audio_delay_ms(10_000), 400, "sınır");
        player.set_audio_delay_ms(0);
        // Aynı şarkıda sarma analizi yeniden başlatmaz.
        assert!(Arc::ptr_eq(
            &first,
            &player.analysis.as_ref().unwrap().spectrogram
        ));

        // Başka şarkı açılınca analiz yenilenir.
        let other = ramp_song("spektrum-2.wav");
        player.load(&other, false).unwrap();
        assert!(!Arc::ptr_eq(
            &first,
            &player.analysis.as_ref().unwrap().spectrogram
        ));
        std::fs::remove_file(path).ok();
        std::fs::remove_file(other).ok();
    }
    #[test]
    fn analiz_onbellege_yazilir_ve_sonraki_acilista_aninda_hazir() {
        let path = ramp_song("onbellek.wav");
        let cache = Arc::new(AnalysisCache::open_in_memory().unwrap());
        let stamp = FileStamp::of(&path).unwrap();

        let mut first = Player::new();
        first.set_analysis_cache(Arc::clone(&cache));
        first.load(&path, false).unwrap();
        let analyzed = Arc::clone(&first.analysis.as_ref().unwrap().spectrogram);
        assert!(wait_until(|| cache.load(&path, stamp).unwrap().is_some()));
        assert!(analyzed.is_done());

        // Yeni açılış (ör. program yeniden başladı): şarkı çözülmeden, anında hazır.
        let mut second = Player::new();
        second.set_analysis_cache(Arc::clone(&cache));
        second.load(&path, false).unwrap();
        let analysis = second.analysis.as_ref().unwrap();
        assert!(
            analysis.thread.is_none(),
            "önbellekten okundu, analiz başlamadı"
        );
        assert!(analysis.spectrogram.is_done());
        assert_eq!(analysis.spectrogram.frame_at(1.0), analyzed.frame_at(1.0));
        assert_eq!(second.foreground_analyses().load(Ordering::Acquire), 0);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn siradaki_sarki_calanin_analizi_bitince_analiz_edilir() {
        let current = temp_path("once-calan.wav");
        write_wav(&current, 44_100, 2, 44_100 * 6, |frame, _| {
            0.3 * crate::audio::test_util::sine(440.0, 44_100, frame)
        });
        let next = ramp_song("sonra-siradaki.wav");
        let mut player = Player::new();
        let foreground = player.foreground_analyses();
        player.load(&current, false).unwrap();
        player.set_next(Some(next.clone()));
        let first = Arc::clone(&player.analysis.as_ref().unwrap().spectrogram);
        let second = Arc::clone(&player.next_analysis.as_ref().unwrap().spectrogram);
        assert!(wait_until(|| {
            // Sıradakinin ilk karesi ancak çalanın analizi bittikten sonra gelir.
            assert!(second.ready_frames() == 0 || first.is_done());
            second.is_done()
        }));
        assert!(wait_until(|| foreground.load(Ordering::Acquire) == 0));
        std::fs::remove_file(current).ok();
        std::fs::remove_file(next).ok();
    }

    #[test]
    fn ekolayzer_her_oturuma_ulasir_ve_gorsellere_yansir() {
        use crate::audio::eq::BANDS as EQ_BANDS;
        let path = temp_path("eq-sinus.wav");
        let w = 2.0 * std::f64::consts::PI * 1000.0 / 44_100.0;
        write_wav(&path, 44_100, 1, 88_200, |frame, _| {
            0.5 * (w * frame as f64).sin()
        });
        let mut player = Player::new();
        player.load(&path, false).unwrap();
        let analysis = Arc::clone(&player.analysis.as_ref().unwrap().spectrogram);
        assert!(wait_until(|| analysis.is_done()));
        player.seek(1.0).unwrap();
        let before = player.visual_now().unwrap().bands;
        let loudest = (0..BANDS)
            .max_by(|&a, &b| before[a].total_cmp(&before[b]))
            .unwrap();

        // 1 kHz bandını 12 dB kıs: ses yolu ve görseller aynı ayarı kullanır.
        let mut gains = [0.0; EQ_BANDS];
        gains[5] = -12.0;
        let applied = player.set_equalizer(EqSettings {
            enabled: true,
            gains_db: gains,
        });
        assert_eq!(player.equalizer(), applied);
        let after = player.visual_now().unwrap().bands;
        let drop_db = f64::from(before[loudest] - after[loudest]) * 60.0;
        assert!((drop_db - 12.0).abs() < 1.5, "görsel düşüş {drop_db:.1} dB");

        // Sarma yeni bir oturum açar; ekolayzer ayarı yine aynıdır.
        player.seek(0.5).unwrap();
        let session = player.session.as_ref().unwrap();
        assert!(Arc::ptr_eq(&session.shared.eq, &player.eq));
        assert_eq!(session.shared.eq.settings().gains_db[5], -12.0);
        std::fs::remove_file(path).ok();
    }
}
