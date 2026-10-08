//! Beat takibi: şarkının vuruşlarını ve temposunu (BPM) bulur.
//!
//! Yöntem (Ellis 2007, "Beat Tracking by Dynamic Programming"):
//!
//! 1. **Başlangıç gücü** ([`OnsetDetector`]): spektrogram geçişinde her karede
//!    logaritmik bantlarda sıkıştırılmış genliğin artışı (spektral akı). Vuruş
//!    anlarında, yani davul ya da nota başlarında yükselir.
//! 2. **Tempo**: başlangıç gücünün öz-ilintisi, 120 BPM çevresini yeğleyen bir
//!    ağırlıkla; iki katı aralıktaki uyum da hesaba katılır (yarım/çift tempo
//!    karışmasın diye).
//! 3. **Vuruşlar**: dinamik programlama, hem güçlü başlangıçlara denk gelen hem
//!    de tempoya uygun aralıklı vuruş dizisini seçer. Kare altı doğruluk için
//!    tepe noktası parabolle inceltilir.

use rustfft::num_complex::Complex;

/// Başlangıç gücü için logaritmik bant sayısı.
const ONSET_BANDS: usize = 48;
const ONSET_MIN_FREQ: f64 = 30.0;
const ONSET_MAX_FREQ: f64 = 11_000.0;
/// Genlik sıkıştırma katsayısı: `ln(1 + γ·genlik)`.
const ONSET_COMPRESSION: f64 = 1000.0;

/// Aranan tempo aralığı ve tercih edilen tempo.
const MIN_BPM: f64 = 60.0;
const MAX_BPM: f64 = 200.0;
const PREFERRED_BPM: f64 = 120.0;
/// Tempo tercihinin genişliği (oktav).
const TEMPO_PRIOR_OCTAVES: f64 = 1.0;
/// Vuruş aralığının tempodan sapmasına verilen ceza (Ellis'teki "tightness").
const TIGHTNESS: f64 = 100.0;
/// Bundan kısa şarkıda (saniye) beat aranmaz.
const MIN_SECONDS: f64 = 4.0;
/// Öz-ilinti tepe değeri bunun altındaysa şarkıda belirgin bir ritim yok sayılır.
const MIN_RHYTHM_STRENGTH: f64 = 0.08;

/// Başlangıç gücünün tepe noktasının vuruşun duyulduğu andan gecikmesi (saniye).
/// Kare `k`'nin zamanı `k / 60` sayıldığında (spektrogramla aynı kural) sentetik
/// davul kayıtlarında ölçülen ortalama gecikme +0,4…+2,4 ms: düzeltme gerekmiyor.
/// Senkron testi bunu doğrular; FFT ya da kare düzeni değişirse burası güncellenir.
const ONSET_LATENCY_SECONDS: f64 = 0.0;

/// Şarkının vuruş ızgarası.
#[derive(Debug, Clone, PartialEq)]
pub struct BeatGrid {
    /// Tempo (vuruş/dakika).
    pub bpm: f64,
    /// Vuruş anları (saniye, artan sırada).
    pub beats: Vec<f64>,
}

/// Bir anın vuruş ızgarasındaki yeri.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BeatPosition {
    /// O ana kadarki son vuruşun sırası (0'dan başlar).
    pub index: usize,
    /// Son vuruştan bu yana geçen süre, vuruş aralığına oranla (0..1).
    pub phase: f64,
}

impl BeatGrid {
    /// Verilen andaki vuruş konumu. İlk vuruştan önce ve son vuruştan bir aralık
    /// sonrasında `None`.
    pub fn position_at(&self, seconds: f64) -> Option<BeatPosition> {
        let first = *self.beats.first()?;
        if !seconds.is_finite() || seconds < first {
            return None;
        }
        let index = self.beats.partition_point(|&b| b <= seconds) - 1;
        let start = self.beats[index];
        let interval = match self.beats.get(index + 1) {
            Some(&next) => next - start,
            None => 60.0 / self.bpm,
        };
        let phase = (seconds - start) / interval;
        (phase < 1.0).then_some(BeatPosition { index, phase })
    }
}

/// Spektrumdan her karenin başlangıç gücünü hesaplar.
pub struct OnsetDetector {
    /// Her bandın FFT kutu aralığı [başlangıç, bitiş).
    ranges: Vec<(usize, usize)>,
    previous: Vec<f64>,
    current: Vec<f64>,
    primed: bool,
}

impl OnsetDetector {
    pub fn new(sample_rate: u32, fft_size: usize) -> Self {
        let bin_hz = f64::from(sample_rate) / fft_size as f64;
        let max_freq = ONSET_MAX_FREQ.min(f64::from(sample_rate) * 0.45);
        let ratio = (max_freq / ONSET_MIN_FREQ).powf(1.0 / ONSET_BANDS as f64);
        let max_bin = fft_size / 2;
        let mut ranges = Vec::with_capacity(ONSET_BANDS);
        let mut last_end = 1;
        for band in 0..ONSET_BANDS {
            let lo = ONSET_MIN_FREQ * ratio.powi(band as i32);
            let hi = lo * ratio;
            // Düşük frekanslarda bantlar tek kutudan dardır: her bant en az bir
            // kutu alır ve bir öncekinin bittiği yerden başlar (çakışma olmaz).
            let start = ((lo / bin_hz).round() as usize)
                .max(last_end)
                .min(max_bin - 1);
            let end = ((hi / bin_hz).round() as usize).max(start + 1).min(max_bin);
            ranges.push((start, end));
            last_end = end;
        }
        let bands = ranges.len();
        Self {
            ranges,
            previous: vec![0.0; bands],
            current: vec![0.0; bands],
            primed: false,
        }
    }

    /// FFT sonucundan (`norm` ile tam ölçekli sinüs 1 olur) başlangıç gücü.
    pub fn process(&mut self, spectrum: &[Complex<f64>], norm: f64) -> f32 {
        for (value, &(start, end)) in self.current.iter_mut().zip(&self.ranges) {
            let mean = spectrum[start..end].iter().map(|c| c.norm()).sum::<f64>()
                / (end - start) as f64
                * norm;
            *value = (1.0 + ONSET_COMPRESSION * mean).ln();
        }
        let flux = if self.primed {
            self.current
                .iter()
                .zip(&self.previous)
                .map(|(c, p)| (c - p).max(0.0))
                .sum::<f64>()
                / self.current.len() as f64
        } else {
            0.0
        };
        std::mem::swap(&mut self.current, &mut self.previous);
        self.primed = true;
        flux as f32
    }

    /// Pencere dolmadan üretilen sessiz karelerde çağrılır.
    pub fn silent(&mut self) -> f32 {
        self.previous.iter_mut().for_each(|v| *v = 0.0);
        self.primed = true;
        0.0
    }
}

/// Başlangıç gücü dizisinden (`fps` kare/saniye) vuruş ızgarasını çıkarır.
/// Şarkı çok kısaysa ya da belirgin bir ritim yoksa `None`.
pub fn track(onset: &[f32], fps: f64) -> Option<BeatGrid> {
    if (onset.len() as f64) < MIN_SECONDS * fps {
        return None;
    }
    let strength = normalize(onset, fps)?;
    let period = estimate_period(&strength, fps)?;
    let local = smooth(&strength, period / 32.0);
    let frames = dynamic_beats(&local, period);
    let frames = trim_quiet_ends(&local, frames);
    if frames.len() < 4 {
        return None;
    }

    let beats: Vec<f64> = frames
        .iter()
        .map(|&b| (b as f64 + peak_offset(&local, b)) / fps - ONSET_LATENCY_SECONDS)
        .map(|t| t.max(0.0))
        .collect();
    let bpm = (600.0 / mean_interval(&beats)).round() / 10.0;
    Some(BeatGrid { bpm, beats })
}

/// Ortalama vuruş aralığı: vuruş anlarına doğru uydurma (en küçük kareler).
/// Tek tek aralıklara göre kare ızgarasından etkilenmez.
fn mean_interval(beats: &[f64]) -> f64 {
    let n = beats.len() as f64;
    let mean_index = (n - 1.0) / 2.0;
    let mean_time = beats.iter().sum::<f64>() / n;
    let (num, den) = beats
        .iter()
        .enumerate()
        .fold((0.0, 0.0), |(num, den), (i, &t)| {
            let di = i as f64 - mean_index;
            (num + di * (t - mean_time), den + di * di)
        });
    num / den
}

/// Yavaş değişimi çıkarır (yerel ortalamanın üstü kalır) ve standart sapmaya böler.
fn normalize(onset: &[f32], fps: f64) -> Option<Vec<f64>> {
    let radius = (fps * 0.25).round() as usize;
    let mut prefix = Vec::with_capacity(onset.len() + 1);
    prefix.push(0.0f64);
    for &x in onset {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + f64::from(x));
    }
    let high: Vec<f64> = (0..onset.len())
        .map(|i| {
            let lo = i.saturating_sub(radius);
            let hi = (i + radius + 1).min(onset.len());
            let mean = (prefix[hi] - prefix[lo]) / (hi - lo) as f64;
            (f64::from(onset[i]) - mean).max(0.0)
        })
        .collect();
    let mean = high.iter().sum::<f64>() / high.len() as f64;
    let deviation =
        (high.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / high.len() as f64).sqrt();
    (deviation > 1e-9).then(|| high.iter().map(|x| x / deviation).collect())
}

/// Vuruş aralığı (kare; kesirli). Ritim belirgin değilse `None`.
fn estimate_period(strength: &[f64], fps: f64) -> Option<f64> {
    let min_lag = (fps * 60.0 / MAX_BPM).floor() as usize;
    let max_lag = (fps * 60.0 / MIN_BPM).ceil() as usize;
    let mean = strength.iter().sum::<f64>() / strength.len() as f64;
    let centered: Vec<f64> = strength.iter().map(|x| x - mean).collect();
    let acf: Vec<f64> = (0..=2 * max_lag + 2)
        .map(|lag| {
            if lag >= centered.len() {
                return 0.0;
            }
            let n = centered.len() - lag;
            centered[..n]
                .iter()
                .zip(&centered[lag..])
                .map(|(a, b)| a * b)
                .sum::<f64>()
                / n as f64
        })
        .collect();
    let energy = acf[0];
    if energy <= 0.0 {
        return None;
    }

    let preferred = fps * 60.0 / PREFERRED_BPM;
    let score = |lag: usize| {
        let prior = (-0.5 * ((lag as f64 / preferred).log2() / TEMPO_PRIOR_OCTAVES).powi(2)).exp();
        prior * (acf[lag] + 0.5 * acf[2 * lag] + 0.25 * (acf[2 * lag - 1] + acf[2 * lag + 1]))
    };
    let best = (min_lag.max(2)..=max_lag).max_by(|&a, &b| score(a).total_cmp(&score(b)))?;
    if acf[best] / energy < MIN_RHYTHM_STRENGTH {
        return None;
    }
    // Kare altı tempo: öz-ilinti tepesine parabol.
    let refined = best as f64 + parabola(acf[best - 1], acf[best], acf[best + 1]);
    Some(refined)
}

/// Gauss çekirdeğiyle yumuşatma (σ kare cinsinden).
fn smooth(values: &[f64], sigma: f64) -> Vec<f64> {
    let sigma = sigma.max(0.5);
    let radius = (3.0 * sigma).ceil() as isize;
    let kernel: Vec<f64> = (-radius..=radius)
        .map(|i| (-0.5 * (i as f64 / sigma).powi(2)).exp())
        .collect();
    let len = values.len() as isize;
    (0..len)
        .map(|i| {
            kernel
                .iter()
                .enumerate()
                .map(|(k, w)| {
                    let j = i + k as isize - radius;
                    if (0..len).contains(&j) {
                        w * values[j as usize]
                    } else {
                        0.0
                    }
                })
                .sum()
        })
        .collect()
}

/// En iyi vuruş dizisi (kare sıraları).
fn dynamic_beats(local: &[f64], period: f64) -> Vec<usize> {
    let n = local.len();
    let near = (period / 2.0).round().max(1.0) as usize;
    let far = (period * 2.0).round() as usize;
    let mut score = vec![0.0; n];
    let mut back: Vec<Option<usize>> = vec![None; n];
    for t in 0..n {
        let mut best: Option<(f64, usize)> = None;
        if t >= near {
            let first = t.saturating_sub(far);
            for (p, &previous) in score.iter().enumerate().take(t - near + 1).skip(first) {
                let gap = (t - p) as f64 / period;
                let value = previous - TIGHTNESS * gap.ln().powi(2);
                if best.is_none_or(|(v, _)| value > v) {
                    best = Some((value, p));
                }
            }
        }
        match best {
            Some((value, p)) if value > 0.0 => {
                score[t] = local[t] + value;
                back[t] = Some(p);
            }
            _ => score[t] = local[t],
        }
    }

    // Son iki aralıktaki en yüksek puanlı vuruştan geriye doğru izle.
    let tail = n.saturating_sub(far.max(1));
    let Some(mut t) = (tail..n).max_by(|&a, &b| score[a].total_cmp(&score[b])) else {
        return Vec::new();
    };
    let mut beats = vec![t];
    while let Some(p) = back[t] {
        beats.push(p);
        t = p;
    }
    beats.reverse();
    beats
}

/// Başta ve sondaki sessiz kısımlara düşen vuruşları atar.
fn trim_quiet_ends(local: &[f64], beats: Vec<usize>) -> Vec<usize> {
    if beats.is_empty() {
        return beats;
    }
    let rms = (beats.iter().map(|&b| local[b].powi(2)).sum::<f64>() / beats.len() as f64).sqrt();
    let threshold = 0.5 * rms;
    let first = beats.iter().position(|&b| local[b] >= threshold);
    let last = beats.iter().rposition(|&b| local[b] >= threshold);
    match (first, last) {
        (Some(f), Some(l)) => beats[f..=l].to_vec(),
        _ => Vec::new(),
    }
}

/// Tepe noktasının kare altı kayması (−0,5..0,5).
fn peak_offset(values: &[f64], index: usize) -> f64 {
    if index == 0 || index + 1 >= values.len() {
        return 0.0;
    }
    parabola(values[index - 1], values[index], values[index + 1])
}

fn parabola(left: f64, center: f64, right: f64) -> f64 {
    let denominator = left - 2.0 * center + right;
    if denominator.abs() < 1e-12 {
        return 0.0;
    }
    (0.5 * (left - right) / denominator).clamp(-0.5, 0.5)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::analysis::evaluate::f_measure;
    use crate::analysis::spectrogram::{analyze, Spectrogram};
    use crate::audio::decode::Decoder;
    use crate::audio::test_util::{temp_path, write_wav};
    use std::sync::Arc;

    const FPS: f64 = 60.0;

    /// Verilen tempoda dürtü dizisi (başlangıç gücü olarak).
    fn impulses(bpm: f64, offset: f64, seconds: f64) -> (Vec<f32>, Vec<f64>) {
        let frames = (seconds * FPS) as usize;
        let mut onset = vec![0.05f32; frames];
        let mut truth = Vec::new();
        let mut t = offset;
        while t < seconds - 0.05 {
            let k = (t * FPS).round() as usize;
            if k < frames {
                onset[k] = 1.0;
            }
            truth.push(t);
            t += 60.0 / bpm;
        }
        (onset, truth)
    }

    #[test]
    fn duzgun_durtulerde_tempo_ve_vuruslar() {
        for bpm in [72.0, 96.0, 120.0, 128.0, 140.0, 174.0] {
            let (onset, truth) = impulses(bpm, 0.5, 30.0);
            let grid = track(&onset, FPS).unwrap();
            assert!((grid.bpm - bpm).abs() < bpm * 0.02, "{bpm}: {}", grid.bpm);
            let f = f_measure(&grid.beats, &truth, 0.07);
            assert!(f > 0.95, "{bpm} BPM: F = {f}");
        }
    }

    #[test]
    fn baslangic_bantlari_her_orneklemede_gecerli() {
        for rate in [8_000, 22_050, 44_100, 48_000, 96_000, 192_000] {
            let detector = OnsetDetector::new(rate, 2048);
            assert!(
                detector
                    .ranges
                    .iter()
                    .all(|&(s, e)| s >= 1 && s < e && e <= 1024),
                "{rate}"
            );
            assert!(
                detector.ranges.windows(2).all(|w| w[0].1 <= w[1].0),
                "{rate}: çakışma"
            );
        }
    }

    #[test]
    fn sessizlik_ve_kisa_parca_ritimsiz() {
        assert_eq!(track(&vec![0.0; 600], FPS), None);
        assert_eq!(track(&[1.0; 60], FPS), None);
    }

    #[test]
    fn konum_ve_faz() {
        let grid = BeatGrid {
            bpm: 120.0,
            beats: vec![1.0, 1.5, 2.0],
        };
        assert_eq!(grid.position_at(0.5), None);
        let p = grid.position_at(1.25).unwrap();
        assert_eq!(p.index, 0);
        assert!((p.phase - 0.5).abs() < 1e-9);
        assert_eq!(grid.position_at(2.0).unwrap().index, 2);
        assert!((grid.position_at(2.25).unwrap().phase - 0.5).abs() < 1e-9);
        assert_eq!(grid.position_at(2.6), None, "son vuruştan bir aralık sonra");
        assert_eq!(grid.position_at(f64::NAN), None);
    }

    /// Basit sözde rastgele sayı üreteci (testler tekrarlanabilir olsun).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64 / (1u64 << 53) as f64) * 2.0 - 1.0
        }
    }

    /// Gerçekçi bir davul + bas + akor kaydı üretir; gerçek vuruş anlarını döndürür.
    /// Kick 1 ve 3'te (bazen 3'ün arkasında da), snare 2 ve 4'te, hi-hat sekizliklerde.
    pub(crate) fn drum_track(
        bpm: f64,
        start: f64,
        seconds: f64,
        rate: u32,
        seed: u64,
    ) -> (Vec<f64>, Vec<f64>) {
        let n = (seconds * f64::from(rate)) as usize;
        let mut out = vec![0.0f64; n];
        let mut rng = Lcg(seed);
        let beat = 60.0 / bpm;
        let mut truth = Vec::new();
        let mut add = |at: f64, len: f64, f: &mut dyn FnMut(f64) -> f64| {
            let s0 = (at * f64::from(rate)) as usize;
            let len = (len * f64::from(rate)) as usize;
            for i in 0..len.min(n.saturating_sub(s0)) {
                out[s0 + i] += f(i as f64 / f64::from(rate));
            }
        };
        let mut index = 0usize;
        let mut t = start;
        while t < seconds - 0.3 {
            truth.push(t);
            let velocity = 0.8 + 0.2 * rng.next().abs();
            let in_bar = index % 4;
            if in_bar == 0 || in_bar == 2 {
                // Kick: 110 Hz'den 45 Hz'e inen sinüs.
                let mut phase = 0.0;
                add(t, 0.25, &mut |x| {
                    phase += 2.0 * std::f64::consts::PI * (45.0 + 65.0 * (-x * 30.0).exp())
                        / f64::from(rate);
                    0.7 * velocity * phase.sin() * (-x * 14.0).exp()
                });
            } else {
                // Snare: gürültü + 190 Hz gövde.
                let mut noise = Lcg(seed ^ index as u64);
                add(t, 0.2, &mut |x| {
                    velocity
                        * (0.35 * noise.next() * (-x * 22.0).exp()
                            + 0.25
                                * (2.0 * std::f64::consts::PI * 190.0 * x).sin()
                                * (-x * 30.0).exp())
                });
            }
            if in_bar == 2 && index % 8 == 2 {
                let mut phase = 0.0;
                add(t + beat * 0.5, 0.2, &mut |x| {
                    phase += 2.0 * std::f64::consts::PI * (45.0 + 65.0 * (-x * 30.0).exp())
                        / f64::from(rate);
                    0.45 * phase.sin() * (-x * 14.0).exp()
                });
            }
            // Hi-hat: türevi alınmış gürültü (tiz), sekizliklerde.
            for half in [0.0, 0.5] {
                let mut noise = Lcg(seed.wrapping_add(index as u64 * 2 + (half * 2.0) as u64));
                let mut previous = 0.0;
                let accent = if half == 0.0 { 0.12 } else { 0.08 };
                add(t + beat * half, 0.05, &mut |x| {
                    let v = noise.next();
                    let hp = v - previous;
                    previous = v;
                    accent * hp * (-x * 90.0).exp()
                });
            }
            // Bas: her vuruşta değişen nota, yumuşak başlangıçlı.
            let note = [55.0, 55.0, 65.4, 49.0][(index / 4) % 4];
            add(t, beat * 0.95, &mut |x| {
                0.18 * (2.0 * std::f64::consts::PI * note * x).sin()
                    * (1.0 - (-x * 60.0).exp())
                    * (-x * 2.0).exp()
            });
            index += 1;
            t = start + index as f64 * beat;
        }
        // Sürekli akor (ritimsiz zemin).
        for (i, sample) in out.iter_mut().enumerate() {
            let x = i as f64 / f64::from(rate);
            *sample += 0.05
                * [220.0, 277.2, 329.6]
                    .iter()
                    .map(|f| (2.0 * std::f64::consts::PI * f * x).sin())
                    .sum::<f64>();
        }
        (out, truth)
    }

    fn analyze_samples(samples: &[f64], rate: u32) -> Option<Arc<BeatGrid>> {
        let path = temp_path("beat.wav");
        write_wav(&path, rate, 2, samples.len(), |f, _| 0.8 * samples[f]);
        let spectrogram = Spectrogram::new();
        analyze(Decoder::open(&path).unwrap(), &spectrogram);
        std::fs::remove_file(&path).ok();
        spectrogram.beat_grid()
    }

    #[test]
    fn davul_kaydinda_tempo_vuruslar_ve_senkron() {
        let rate = 44_100;
        for (bpm, start, seed) in [
            (90.0, 1.37, 1),
            (120.0, 0.52, 2),
            (128.0, 2.0, 3),
            (140.0, 0.9, 4),
            (105.5, 1.1, 5),
        ] {
            let (samples, truth) = drum_track(bpm, start, 32.0, rate, seed);
            let grid = analyze_samples(&samples, rate).expect("ritim bulunmalı");
            assert!((grid.bpm - bpm).abs() <= 0.3, "{bpm}: {}", grid.bpm);
            let f = f_measure(&grid.beats, &truth, 0.07);
            assert!(f > 0.95, "{bpm} BPM: F = {f}");
            // Senkron: eşleşen vuruşların ortalama ve en büyük hatası.
            let errors: Vec<f64> = grid
                .beats
                .iter()
                .filter_map(|&b| {
                    truth
                        .iter()
                        .map(|&t| b - t)
                        .min_by(|a, c| a.abs().total_cmp(&c.abs()))
                })
                .filter(|e| e.abs() < 0.07)
                .collect();
            let mean = errors.iter().sum::<f64>() / errors.len() as f64;
            let worst = errors.iter().fold(0.0f64, |m, e| m.max(e.abs()));
            eprintln!(
                "{bpm} BPM: bulunan {} F={f:.3} ortalama hata {:.1} ms, en büyük {:.1} ms",
                grid.bpm,
                mean * 1000.0,
                worst * 1000.0
            );
            assert!(
                mean.abs() < 0.005,
                "{bpm} BPM ortalama hata {:.1} ms",
                mean * 1000.0
            );
            assert!(
                worst < 0.02,
                "{bpm} BPM en büyük hata {:.1} ms",
                worst * 1000.0
            );
        }
    }

    #[test]
    fn ritimsiz_gurultude_vurus_uydurmaz() {
        let rate = 22_050;
        let mut rng = Lcg(7);
        let samples: Vec<f64> = (0..rate as usize * 20).map(|_| 0.3 * rng.next()).collect();
        assert_eq!(analyze_samples(&samples, rate), None);
    }
}
