//! Spektrogram: şarkının her anı için frekans bantlarının seviyesi.
//!
//! Şarkı açılınca ayrı bir iş parçacığında baştan sona taranır (çalmadan çok
//! daha hızlı). Görseller çalma konumuna karşılık gelen kareyi okur; böylece ses
//! ve görüntü tam eş zamanlıdır ve gerçek zamanlı ses yoluna hiç yük binmez.
//! Bu, "görseller şarkıyı önceden bilir" fikrinin ilk adımıdır.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, RwLock};

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};

use super::beats::{self, BeatGrid, OnsetDetector};
use super::levels::{self, ChannelLevels, LevelAccumulator, VALUES_PER_FRAME};
use super::structure::{self, SongMap};
use crate::audio::decode::Decoder;
use crate::audio::Sample;
use crate::director::Choreography;

/// Saniyedeki analiz karesi sayısı (ekran tazeleme hızıyla aynı).
pub const FRAMES_PER_SECOND: f64 = 60.0;
/// Saklanan frekans bandı sayısı. Sahneler kendi bant sayılarına indirger.
pub const BANDS: usize = 32;
/// FFT pencere uzunluğu (44,1 kHz'te ~46 ms; bas frekanslar için yeterli çözünürlük).
const FFT_SIZE: usize = 2048;
/// Bantların kapsadığı aralık.
const MIN_FREQ: f64 = 30.0;
const MAX_FREQ: f64 = 16_000.0;
/// Seviye ölçeği: bu aralıktaki desibel değerleri 0..1'e eşlenir.
const FLOOR_DB: f64 = -66.0;
const CEIL_DB: f64 = -6.0;
/// Müzikte yüksek frekansların enerjisi doğal olarak düşüktür; görsel denge için
/// 1 kHz üstünde oktav başına bu kadar desibel eklenir (altında çıkarılır).
const TILT_DB_PER_OCTAVE: f64 = 3.0;

/// Bir şarkının spektrogramı ve kanal seviyeleri. Analiz sürerken de okunabilir.
pub struct Spectrogram {
    /// Her kare için `BANDS` adet 0..255 seviye (art arda).
    levels: RwLock<Vec<u8>>,
    /// Her kare için kanal seviyeleri ([`levels`] modülünün saklama biçimi).
    meters: RwLock<Vec<u8>>,
    /// Her karenin başlangıç gücü (beat takibi için).
    onset: RwLock<Vec<f32>>,
    /// Analiz bitince hesaplanan VU referansı (dBFS).
    vu_reference: RwLock<Option<f32>>,
    /// Analiz bitince bulunan vuruş ızgarası (ritim yoksa `None`).
    beats: RwLock<Option<Arc<BeatGrid>>>,
    /// Analiz bitince çıkarılan şarkı yapısı (ölçü, bölüm, drop, enerji).
    song_map: RwLock<Option<Arc<SongMap>>>,
    /// Analiz bitince kurulan koreografi (Görsel Yönetmen).
    choreography: RwLock<Option<Arc<Choreography>>>,
    /// Hazır kare sayısı.
    ready: AtomicUsize,
    done: AtomicBool,
    cancelled: AtomicBool,
}

impl Spectrogram {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            levels: RwLock::new(Vec::new()),
            meters: RwLock::new(Vec::new()),
            onset: RwLock::new(Vec::new()),
            vu_reference: RwLock::new(None),
            beats: RwLock::new(None),
            song_map: RwLock::new(None),
            choreography: RwLock::new(None),
            ready: AtomicUsize::new(0),
            done: AtomicBool::new(false),
            cancelled: AtomicBool::new(false),
        })
    }

    /// Analizi durdurur (başka şarkı açıldığında).
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    pub fn is_done(&self) -> bool {
        self.done.load(Ordering::Acquire)
    }

    /// Analiz iptal edildi mi (bitmeden başka şarkı açıldı)?
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Önbellekteki sonuçtan hazır (bitmiş) bir spektrogram kurar: şarkı yeniden
    /// çözülmez. Koreografi ve VU referansı saklanan veriden hesaplanır.
    pub fn from_saved(saved: SavedAnalysis) -> Arc<Self> {
        let spectrogram = Self::new();
        spectrogram.push(&saved.levels, &saved.meters, &saved.onset);
        spectrogram.complete(saved.beats, saved.song_map);
        spectrogram
    }

    /// Bitmiş analizin saklanacak hâli; analiz bitmediyse (ya da yarıda kesildiyse) `None`.
    pub fn saved(&self) -> Option<SavedAnalysis> {
        if !self.is_done() {
            return None;
        }
        Some(SavedAnalysis {
            levels: self.levels.read().ok()?.clone(),
            meters: self.meters.read().ok()?.clone(),
            onset: self.onset.read().ok()?.clone(),
            beats: self.beat_grid().map(|b| (*b).clone()),
            song_map: self.song_map().map(|m| (*m).clone()),
        })
    }

    pub fn ready_frames(&self) -> usize {
        self.ready.load(Ordering::Acquire)
    }

    /// Verilen saniyedeki bant seviyeleri (0..1). O an henüz analiz edilmediyse `None`.
    pub fn frame_at(&self, seconds: f64) -> Option<[f32; BANDS]> {
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        let index = (seconds * FRAMES_PER_SECOND).floor() as usize;
        if index >= self.ready_frames() {
            return None;
        }
        let levels = self.levels.read().ok()?;
        let frame = levels.get(index * BANDS..(index + 1) * BANDS)?;
        let mut out = [0.0f32; BANDS];
        for (dst, &src) in out.iter_mut().zip(frame) {
            *dst = f32::from(src) / 255.0;
        }
        Some(out)
    }

    /// Verilen saniyedeki kanal seviyeleri. O an henüz analiz edilmediyse `None`.
    pub fn meters_at(&self, seconds: f64) -> Option<ChannelLevels> {
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        let index = (seconds * FRAMES_PER_SECOND).floor() as usize;
        if index >= self.ready_frames() {
            return None;
        }
        let meters = self.meters.read().ok()?;
        levels::decode_frame(meters.get(index * VALUES_PER_FRAME..(index + 1) * VALUES_PER_FRAME)?)
    }

    /// 0 VU'ya denk gelen seviye (dBFS); analiz bitene kadar `None`.
    pub fn vu_reference_db(&self) -> Option<f32> {
        self.vu_reference.read().ok().and_then(|r| *r)
    }

    /// Şarkının koreografisi; analiz bitene kadar ya da ritim yoksa `None`.
    pub fn choreography(&self) -> Option<Arc<Choreography>> {
        self.choreography.read().ok().and_then(|c| c.clone())
    }

    /// Şarkının yapısı; analiz bitene kadar ya da ritim yoksa `None`.
    pub fn song_map(&self) -> Option<Arc<SongMap>> {
        self.song_map.read().ok().and_then(|m| m.clone())
    }

    /// Şarkının vuruş ızgarası; analiz bitene kadar ya da ritim yoksa `None`.
    pub fn beat_grid(&self) -> Option<Arc<BeatGrid>> {
        self.beats.read().ok().and_then(|b| b.clone())
    }

    fn push(&self, frames: &[u8], meters: &[u8], onset: &[f32]) {
        // Önce seviyeler: hazır kare sayısı arttığında hepsi okunabilir olsun.
        if let Ok(mut stored) = self.meters.write() {
            stored.extend_from_slice(meters);
        }
        if let Ok(mut stored) = self.onset.write() {
            stored.extend_from_slice(onset);
        }
        if let Ok(mut levels) = self.levels.write() {
            levels.extend_from_slice(frames);
            self.ready.store(levels.len() / BANDS, Ordering::Release);
        }
    }

    /// Analiz bitince: vuruşlar (başlangıç gücünün `onset_latency` saniyelik gecikmesi
    /// düşülerek), şarkı yapısı ve koreografi.
    fn finish(&self, onset_latency: f64) {
        let grid = self
            .onset
            .read()
            .ok()
            .and_then(|o| beats::track(&o, FRAMES_PER_SECOND, onset_latency));
        let map = grid.as_ref().and_then(|grid| {
            let levels = self.levels.read().ok()?;
            let meters = self.meters.read().ok()?;
            // Kare başına ses yüksekliği: iki kanalın etkin seviyesinin ortalaması.
            let loudness: Vec<f32> = meters
                .chunks_exact(VALUES_PER_FRAME)
                .filter_map(levels::decode_frame)
                .map(|m| 0.5 * (m.rms_db[0] + m.rms_db[1]))
                .collect();
            structure::map_song(&levels, BANDS, &loudness, grid, FRAMES_PER_SECOND)
        });
        self.complete(grid, map);
    }

    /// Analizi tamamlar: VU referansı ve koreografi hesaplanır, sonuçlar yayımlanır.
    fn complete(&self, grid: Option<BeatGrid>, map: Option<SongMap>) {
        let reference = self
            .meters
            .read()
            .ok()
            .and_then(|m| levels::vu_reference_db(&m, FRAMES_PER_SECOND));
        if let Ok(mut stored) = self.vu_reference.write() {
            *stored = reference;
        }
        let choreography = match (&grid, &map) {
            (Some(grid), Some(map)) => {
                let levels = self.levels.read().ok();
                let onset = self.onset.read().ok();
                levels.zip(onset).map(|(levels, onset)| {
                    Choreography::build(grid, map, &levels, BANDS, &onset, FRAMES_PER_SECOND)
                })
            }
            _ => None,
        };
        if let Ok(mut stored) = self.beats.write() {
            *stored = grid.map(Arc::new);
        }
        if let Ok(mut stored) = self.choreography.write() {
            *stored = choreography.map(Arc::new);
        }
        if let Ok(mut stored) = self.song_map.write() {
            *stored = map.map(Arc::new);
        }
        self.done.store(true, Ordering::Release);
    }
}

/// Bir şarkının saklanabilir analiz sonucu (önbellek için): ham kareler ve
/// onlardan çıkarılan vuruş ızgarası ile şarkı haritası.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedAnalysis {
    /// Kare başına `BANDS` bant seviyesi (0..255).
    pub levels: Vec<u8>,
    /// Kare başına kanal seviyeleri ([`levels`] modülünün saklama biçimi).
    pub meters: Vec<u8>,
    /// Kare başına başlangıç gücü.
    pub onset: Vec<f32>,
    pub beats: Option<BeatGrid>,
    pub song_map: Option<SongMap>,
}

impl SavedAnalysis {
    /// Kare sayısı.
    pub fn frames(&self) -> usize {
        self.levels.len() / BANDS
    }

    /// Veri kendi içinde tutarlı mı (bozuk önbellek kaydına karşı)?
    pub fn is_consistent(&self) -> bool {
        let frames = self.frames();
        self.levels.len() == frames * BANDS
            && self.meters.len() == frames * VALUES_PER_FRAME
            && self.onset.len() == frames
    }
}

/// Şarkıyı baştan sona çözerek spektrogramı doldurur. Ayrı iş parçacığında çalışır.
pub fn analyze(decoder: Decoder, target: &Spectrogram) {
    analyze_paced(decoder, target, &mut || {});
}

/// [`analyze`] gibi; ama her çözülen parçadan sonra `pace` çağrılır (arka plan
/// analizi burada bekleyerek işlemciyi çalan şarkıya bırakır).
pub fn analyze_paced(mut decoder: Decoder, target: &Spectrogram, pace: &mut dyn FnMut()) {
    let info = decoder.info().clone();
    let channels = info.channels.max(1);
    let mut analyzer = BandAnalyzer::new(info.sample_rate);

    // Dairesel pencere: son FFT_SIZE mono örnek (`head` en eski örneğin yeri).
    // Her 1/60 saniyede bir kare üretilir ([`frame_end`]).
    let mut window = vec![0.0; FFT_SIZE];
    let mut head = 0usize;
    let mut filled = 0usize;
    // İşlenen örnek sayısı ve sıradaki karenin bittiği örnek.
    let mut position = 0u64;
    let mut frame_index = 0u64;
    let mut next_end = frame_end(0, info.sample_rate);
    let mut batch: Vec<u8> = Vec::with_capacity(BANDS * 64);
    let mut meters = LevelAccumulator::default();
    let mut meter_batch: Vec<u8> = Vec::with_capacity(VALUES_PER_FRAME * 64);
    let mut onset_batch: Vec<f32> = Vec::with_capacity(64);

    loop {
        if target.cancelled.load(Ordering::Acquire) {
            return;
        }
        pace();
        let chunk = match decoder.next_chunk() {
            Ok(Some(chunk)) => chunk,
            Ok(None) | Err(_) => break,
        };
        for frame in chunk.chunks_exact(channels) {
            meters.add(frame);
            let mono = frame.iter().sum::<Sample>() / channels as Sample;
            window[head] = mono;
            head = (head + 1) % FFT_SIZE;
            filled = (filled + 1).min(FFT_SIZE);
            position += 1;
            if position == next_end {
                frame_index += 1;
                next_end = frame_end(frame_index, info.sample_rate);
                // k. karenin penceresi (k+1)/60 saniyede biter; 44,1 kHz'te ortası k/60
                // saniyenin yaklaşık 6 ms gerisindedir: görüntü sesle hizalı kalır.
                // Pencerenin yarısı dolana kadar kare sessiz sayılır.
                let (levels, onset) = if filled < FFT_SIZE / 2 {
                    ([0u8; BANDS], analyzer.onset.silent())
                } else {
                    analyzer.process(&window, head)
                };
                batch.extend_from_slice(&levels);
                meter_batch.extend_from_slice(&meters.finish());
                onset_batch.push(onset);
                if batch.len() >= BANDS * 64 {
                    target.push(&batch, &meter_batch, &onset_batch);
                    batch.clear();
                    meter_batch.clear();
                    onset_batch.clear();
                }
            }
        }
    }
    target.push(&batch, &meter_batch, &onset_batch);
    target.finish(beats::onset_latency(info.sample_rate, FFT_SIZE));
}

/// `k`. karenin bittiği örnek (hariç): (k + 1) / 60 saniyeye en yakın örnek.
/// Kare aralığı tam sayı olmayan hızlarda da (ör. 22 050 / 60 = 367,5 örnek) kareler
/// zamanda kaymaz; aralıklar 367 ile 368 arasında değişir.
fn frame_end(k: u64, sample_rate: u32) -> u64 {
    let fps = FRAMES_PER_SECOND as u64;
    ((k + 1) * u64::from(sample_rate) * 2 + fps) / (2 * fps)
}

/// Bir pencereyi frekans bantlarına ayıran FFT çözümleyici.
struct BandAnalyzer {
    fft: Arc<dyn Fft<f64>>,
    hann: Vec<f64>,
    buffer: Vec<Complex<f64>>,
    /// Her bandın FFT kutu aralığı [başlangıç, bitiş).
    ranges: Vec<(usize, usize)>,
    /// Her bandın orta frekansına göre eğim düzeltmesi (dB).
    tilt: Vec<f64>,
    /// Tam ölçekli bir sinüsün 0 dB çıkması için ölçek.
    norm: f64,
    /// Aynı FFT'den beat takibi için başlangıç gücü.
    onset: OnsetDetector,
}

impl BandAnalyzer {
    fn new(sample_rate: u32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(FFT_SIZE);
        let hann: Vec<f64> = (0..FFT_SIZE)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / FFT_SIZE as f64).cos())
            .collect();
        let norm = 2.0 / hann.iter().sum::<f64>();
        let edges = band_edges(BANDS, f64::from(sample_rate));
        let bin_hz = f64::from(sample_rate) / FFT_SIZE as f64;
        let max_bin = FFT_SIZE / 2;
        let ranges = edges
            .windows(2)
            .map(|e| {
                let start = ((e[0] / bin_hz).round() as usize).clamp(1, max_bin - 1);
                let end = ((e[1] / bin_hz).round() as usize).clamp(start + 1, max_bin);
                (start, end)
            })
            .collect();
        let tilt = edges
            .windows(2)
            .map(|e| TILT_DB_PER_OCTAVE * ((e[0] * e[1]).sqrt() / 1000.0).log2())
            .collect();
        Self {
            fft,
            hann,
            buffer: vec![Complex::default(); FFT_SIZE],
            ranges,
            tilt,
            norm,
            onset: OnsetDetector::new(sample_rate, FFT_SIZE),
        }
    }

    /// `window` dairesel tampondur; en eski örnek `head` konumundadır.
    /// Bant seviyelerini ve başlangıç gücünü döndürür.
    fn process(&mut self, window: &[Sample], head: usize) -> ([u8; BANDS], f32) {
        let ordered = window[head..].iter().chain(&window[..head]);
        for ((dst, &x), &w) in self.buffer.iter_mut().zip(ordered).zip(&self.hann) {
            *dst = Complex::new(x * w, 0.0);
        }
        self.fft.process(&mut self.buffer);

        let mut out = [0u8; BANDS];
        for (band, &(start, end)) in self.ranges.iter().enumerate() {
            // Banttaki en güçlü bileşen: dar bantlarda tek bir notayı da yakalar.
            let peak = self.buffer[start..end]
                .iter()
                .map(|c| c.norm())
                .fold(0.0, f64::max)
                * self.norm;
            let db = 20.0 * peak.max(1e-12).log10() + self.tilt[band];
            out[band] = to_level(db);
        }
        (out, self.onset.process(&self.buffer, self.norm))
    }
}

/// Desibeli 0..255 seviyeye çevirir.
fn to_level(db: f64) -> u8 {
    let x = ((db - FLOOR_DB) / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0);
    (x * 255.0).round() as u8
}

/// Her bandın orta frekansı (Hz, geometrik orta).
pub fn band_centers(sample_rate: f64) -> [f64; BANDS] {
    let edges = band_edges(BANDS, sample_rate);
    std::array::from_fn(|i| (edges[i] * edges[i + 1]).sqrt())
}

/// Seviyeyi (0..1) verilen desibel kadar kaydırır: ekolayzer gibi sonradan
/// uygulanan kazançlar görsellerde de görünsün. Sessiz (0) bantlar sessiz kalır.
pub fn shift_level(level: f32, db: f64) -> f32 {
    if level <= 0.0 {
        return 0.0;
    }
    (f64::from(level) + db / (CEIL_DB - FLOOR_DB)).clamp(0.0, 1.0) as f32
}

/// Logaritmik aralıklı bant sınırları (Hz). `bands + 1` değer döner.
/// Üst sınır Nyquist frekansını aşmaz.
pub fn band_edges(bands: usize, sample_rate: f64) -> Vec<f64> {
    let max = MAX_FREQ.min(sample_rate / 2.0 * 0.95);
    let ratio = (max / MIN_FREQ).ln();
    (0..=bands)
        .map(|i| MIN_FREQ * (ratio * i as f64 / bands as f64).exp())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::{sine, temp_path, write_wav};
    use std::path::Path;

    #[test]
    fn seviye_kaydirma_desibel_olcegine_uyar() {
        // 60 dB'lik ölçekte +6 dB = 0,1 seviye.
        assert!((shift_level(0.5, 6.0) - 0.6).abs() < 1e-6);
        assert!((shift_level(0.5, -12.0) - 0.3).abs() < 1e-6);
        assert_eq!(shift_level(0.95, 12.0), 1.0);
        assert_eq!(shift_level(0.0, 12.0), 0.0, "sessizlik yükseltilmez");
        let centers = band_centers(44100.0);
        assert!(centers[0] > 30.0 && centers[BANDS - 1] < 16_000.0);
        assert!(centers.windows(2).all(|w| w[0] < w[1]));
    }

    fn analyze_file(path: &Path) -> Arc<Spectrogram> {
        let spectrogram = Spectrogram::new();
        analyze(Decoder::open(path).unwrap(), &spectrogram);
        spectrogram
    }

    fn band_of(freq: f64, rate: f64) -> usize {
        let edges = band_edges(BANDS, rate);
        edges
            .windows(2)
            .position(|e| freq >= e[0] && freq < e[1])
            .unwrap()
    }

    #[test]
    fn her_ornekleme_hizinda_saniyede_tam_60_kare() {
        // 22 050 / 60 = 367,5 gibi tam bölünmeyen hızlarda kare aralığı yuvarlanınca
        // kareler zamanda kayıyordu (4. dakikada ~0,3 sn): görüntü ve vuruşlar sesten
        // önce ya da sonra gelirdi.
        for rate in [8_000, 22_050, 32_000, 44_100] {
            let path = temp_path(&format!("kare-{rate}.wav"));
            let frames = rate as usize * 60;
            write_wav(&path, rate, 1, frames, |i, _| 0.1 * sine(440.0, rate, i));
            let spectrogram = analyze_file(&path);
            assert_eq!(spectrogram.ready_frames(), 3_600, "{rate} Hz");
            std::fs::remove_file(path).ok();
        }
    }

    #[test]
    fn bant_sinirlari_logaritmik_ve_artan() {
        let edges = band_edges(BANDS, 44_100.0);
        assert_eq!(edges.len(), BANDS + 1);
        assert!((edges[0] - MIN_FREQ).abs() < 1e-9);
        assert!((edges[BANDS] - MAX_FREQ).abs() < 1e-6);
        let ratios: Vec<f64> = edges.windows(2).map(|e| e[1] / e[0]).collect();
        assert!(
            ratios.iter().all(|r| (r - ratios[0]).abs() < 1e-9),
            "eşit oranlı olmalı"
        );
        // Düşük örnekleme hızında Nyquist aşılmaz.
        assert!(band_edges(BANDS, 16_000.0)[BANDS] < 8_000.0);
    }

    #[test]
    fn sinus_dogru_bantta_en_yuksek_cikar() {
        let path = temp_path("spektrum-1k.wav");
        let rate = 44_100;
        write_wav(&path, rate, 2, rate as usize, |f, _| {
            0.5 * sine(1000.0, rate, f)
        });
        let spectrogram = analyze_file(&path);
        assert!(spectrogram.is_done());

        let frame = spectrogram.frame_at(0.5).unwrap();
        let loudest = frame
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap()
            .0;
        assert_eq!(loudest, band_of(1000.0, f64::from(rate)));
        assert!(
            frame[loudest] > 0.8,
            "1 kHz bandı güçlü olmalı: {}",
            frame[loudest]
        );
        // Uzak bantlar sessiz kalır.
        assert!(frame[0] < 0.2 && frame[BANDS - 1] < 0.2, "{frame:?}");
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn kanal_seviyeleri_ve_vu_referansi_hesaplanir() {
        let path = temp_path("seviye-stereo.wav");
        let rate = 44_100;
        // Sol kanal -6 dB tepeli sinüs, sağ kanal sessiz; 2 saniye.
        write_wav(&path, rate, 2, 2 * rate as usize, |f, ch| {
            if ch == 0 {
                0.5 * sine(440.0, rate, f)
            } else {
                0.0
            }
        });
        let spectrogram = analyze_file(&path);
        let levels = spectrogram.meters_at(1.0).unwrap();
        assert!((levels.peak_db[0] + 6.0).abs() < 0.3, "{levels:?}");
        assert!((levels.rms_db[0] + 9.0).abs() < 0.3, "{levels:?}");
        assert_eq!(levels.rms_db[1], levels::FLOOR_DB);
        // Sabit seviyeli şarkıda 0 VU, o seviyedir.
        let reference = spectrogram.vu_reference_db().unwrap();
        assert!((reference + 9.0).abs() < 0.3, "{reference}");
        assert!(spectrogram.meters_at(2.5).is_none());
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn saklanan_analizden_ayni_spektrogram_kurulur() {
        // 120 BPM tıklamalar üstünde 200 Hz ton: vuruşlar ve seviyeler olsun.
        let path = temp_path("spektrum-sakla.wav");
        let rate = 22_050;
        write_wav(&path, rate, 2, rate as usize * 8, |frame, _| {
            let t = frame as f64 / f64::from(rate);
            let click = if (t * 2.0).fract() < 0.01 { 0.8 } else { 0.0 };
            click + 0.2 * (2.0 * std::f64::consts::PI * 200.0 * t).sin()
        });
        let original = Spectrogram::new();
        analyze(Decoder::open(&path).unwrap(), &original);
        let saved = original.saved().expect("analiz bitti");
        assert!(saved.is_consistent());
        assert!(saved.beats.is_some(), "tıklamalarda tempo bulunur");

        let restored = Spectrogram::from_saved(saved.clone());
        assert!(restored.is_done());
        assert_eq!(restored.ready_frames(), original.ready_frames());
        assert_eq!(restored.saved(), Some(saved));
        for seconds in [0.5, 3.0, 7.5] {
            assert_eq!(restored.frame_at(seconds), original.frame_at(seconds));
            assert_eq!(restored.meters_at(seconds), original.meters_at(seconds));
        }
        assert_eq!(restored.vu_reference_db(), original.vu_reference_db());
        assert_eq!(restored.beat_grid(), original.beat_grid());
        assert_eq!(restored.song_map(), original.song_map());
        assert_eq!(
            restored.choreography().is_some(),
            original.choreography().is_some()
        );
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn bitmeyen_analiz_saklanmaz() {
        assert_eq!(Spectrogram::new().saved(), None);
    }

    #[test]
    fn sessizlik_sifir_seviye_verir_ve_kare_sayisi_dogru() {
        let path = temp_path("spektrum-sessiz.wav");
        write_wav(&path, 48_000, 1, 96_000, |_, _| 0.0); // 2 saniye
        let spectrogram = analyze_file(&path);
        assert_eq!(spectrogram.ready_frames(), 120, "saniyede 60 kare");
        assert!(spectrogram.frame_at(1.0).unwrap().iter().all(|&l| l == 0.0));
        assert!(
            spectrogram.frame_at(2.5).is_none(),
            "şarkı sonrası kare yok"
        );
        assert!(spectrogram.frame_at(-1.0).is_none());
        assert_eq!(
            spectrogram.meters_at(1.0).unwrap().peak_db,
            [levels::FLOOR_DB; 2]
        );
        assert_eq!(spectrogram.vu_reference_db(), None);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn kare_zamani_sesle_hizali() {
        // İlk yarı sessiz, ikinci yarı 200 Hz: geçiş 1,0 saniyede görünmeli.
        let path = temp_path("spektrum-gecis.wav");
        let rate = 44_100;
        write_wav(&path, rate, 1, rate as usize * 2, |f, _| {
            if f < rate as usize {
                0.0
            } else {
                0.5 * sine(200.0, rate, f)
            }
        });
        let spectrogram = analyze_file(&path);
        let band = band_of(200.0, f64::from(rate));
        assert!(spectrogram.frame_at(0.9).unwrap()[band] < 0.1);
        assert!(spectrogram.frame_at(1.1).unwrap()[band] > 0.7);
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn iptal_edilen_analiz_durur() {
        let path = temp_path("spektrum-iptal.wav");
        write_wav(&path, 8_000, 1, 80_000, |f, _| sine(440.0, 8_000, f));
        let spectrogram = Spectrogram::new();
        spectrogram.cancel();
        analyze(Decoder::open(&path).unwrap(), &spectrogram);
        assert!(!spectrogram.is_done());
        assert_eq!(spectrogram.ready_frames(), 0);
        std::fs::remove_file(path).ok();
    }
}
