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

use crate::audio::decode::Decoder;
use crate::audio::Sample;

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

/// Bir şarkının spektrogramı. Analiz sürerken de okunabilir.
pub struct Spectrogram {
    /// Her kare için `BANDS` adet 0..255 seviye (art arda).
    levels: RwLock<Vec<u8>>,
    /// Hazır kare sayısı.
    ready: AtomicUsize,
    done: AtomicBool,
    cancelled: AtomicBool,
}

impl Spectrogram {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            levels: RwLock::new(Vec::new()),
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

    fn push(&self, frames: &[u8]) {
        if let Ok(mut levels) = self.levels.write() {
            levels.extend_from_slice(frames);
            self.ready.store(levels.len() / BANDS, Ordering::Release);
        }
    }
}

/// Şarkıyı baştan sona çözerek spektrogramı doldurur. Ayrı iş parçacığında çalışır.
pub fn analyze(mut decoder: Decoder, target: &Spectrogram) {
    let info = decoder.info().clone();
    let channels = info.channels.max(1);
    let hop = (f64::from(info.sample_rate) / FRAMES_PER_SECOND).round() as usize;
    let mut analyzer = BandAnalyzer::new(info.sample_rate);

    // Dairesel pencere: son FFT_SIZE mono örnek (`head` en eski örneğin yeri).
    // Her `hop` örnekte bir kare üretilir.
    let mut window = vec![0.0; FFT_SIZE];
    let mut head = 0usize;
    let mut filled = 0usize;
    let mut since_last = 0usize;
    let mut batch: Vec<u8> = Vec::with_capacity(BANDS * 64);

    loop {
        if target.cancelled.load(Ordering::Acquire) {
            return;
        }
        let chunk = match decoder.next_chunk() {
            Ok(Some(chunk)) => chunk,
            Ok(None) | Err(_) => break,
        };
        for frame in chunk.chunks_exact(channels) {
            let mono = frame.iter().sum::<Sample>() / channels as Sample;
            window[head] = mono;
            head = (head + 1) % FFT_SIZE;
            filled = (filled + 1).min(FFT_SIZE);
            since_last += 1;
            if since_last == hop {
                since_last = 0;
                // k. karenin penceresi (k+1)·hop örnekte biter, ortası k/60 saniyenin
                // yaklaşık 6 ms gerisindedir: görüntü sesle hizalı kalır. Pencerenin
                // yarısı dolana kadar kare sessiz sayılır.
                let levels = if filled < FFT_SIZE / 2 {
                    [0u8; BANDS]
                } else {
                    analyzer.process(&window, head)
                };
                batch.extend_from_slice(&levels);
                if batch.len() >= BANDS * 64 {
                    target.push(&batch);
                    batch.clear();
                }
            }
        }
    }
    target.push(&batch);
    target.done.store(true, Ordering::Release);
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
        }
    }

    /// `window` dairesel tampondur; en eski örnek `head` konumundadır.
    fn process(&mut self, window: &[Sample], head: usize) -> [u8; BANDS] {
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
        out
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
    use crate::audio::test_util::{sine, temp_path, write_wav};
    use std::path::Path;

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
