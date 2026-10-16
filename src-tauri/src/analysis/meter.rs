//! Aksak ölçü: 5/8, 7/8, 9/8 gibi eşit olmayan vuruşlu ölçüler.
//!
//! Vuruş takipçisi ([`super::beats`]) eşit aralıklı vuruş arar; aksak ölçüde vuruşlar uzun ve
//! kısadır (9/8 = 2+2+2+3 sekizlik). Takipçi o zaman iki sekizlikte bir "vuruş" bulur ve üçlü
//! grupları atlar; ölçü başları kayar, görseller yanlış yere vurgu yapar.
//!
//! Çözüm sekizlik düzeyindedir:
//!
//! 1. **Sekizlik ızgarası:** takipçinin vuruş aralığının yarısıyla (ve kendisiyle) başlangıç
//!    gücüne dinamik programlamayla oturan eşit aralıklı bir ızgara.
//! 2. **Vurgu:** her sekizlikte başlangıç gücü ve bas vuruşu (kick).
//! 3. **Kalıplar:** düzenli (2+2+2+2, 2+2+2, 3+3) ve aksak (3+2, 2+3; 2+2+3, 2+3+2, 3+2+2;
//!    2+2+2+3 …) gruplamaların her başlangıç konumu için vurgu dizisiyle ilinti: ölçü başı en
//!    güçlü, grup başları güçlü, diğerleri zayıf. Ölçü süresi farklı olan kalıp şarkı boyunca
//!    kayar, ilintisi düşer.
//! 4. **Karar:** aksak kalıp düzenli kalıplardan açıkça iyiyse (müziğin çoğu düzenli) şarkının
//!    vuruşları grup başları olur (9/8'de ölçüde 4 vuruş) ve ölçü başları ızgaradan gelir.
//!    Değilse takipçinin vuruşları kalır.

use super::beats::{mean_interval, track_period, BeatGrid};

/// Düzenli gruplamalar (sekizlik): 4/4, 3/4, 6/8.
const REGULAR: &[&[u8]] = &[&[2, 2, 2, 2], &[2, 2, 2], &[3, 3]];
/// Aksak gruplamalar (sekizlik): 5/8, 7/8, 9/8 ve dönüşleri.
const AKSAK: &[&[u8]] = &[
    &[2, 3],
    &[3, 2],
    &[2, 2, 3],
    &[2, 3, 2],
    &[3, 2, 2],
    &[2, 2, 2, 3],
    &[2, 2, 3, 2],
    &[2, 3, 2, 2],
    &[3, 2, 2, 2],
];
/// Kalıpta ölçü başının, diğer grup başlarının ve geri kalan sekizliklerin ağırlığı.
const DOWNBEAT_WEIGHT: f64 = 1.0;
const GROUP_WEIGHT: f64 = 0.6;
/// Aksak sayılmak için: kalıbın ilintisi bu kadar olmalı ve en iyi düzenli kalıbı bu kadar
/// geçmeli. Güven bu farka göre (fark `CONFIDENT_MARGIN` olunca tam güven).
const MIN_CORRELATION: f64 = 0.35;
const MIN_MARGIN: f64 = 0.12;
const CONFIDENT_MARGIN: f64 = 0.3;
/// Bas vuruşunun (kick) arandığı süre ve bas bantları (32 bandın ilk 8'i).
const KICK_REACH_SECONDS: f64 = 0.08;
const BASS_BANDS: usize = 8;
/// Bundan az sekizlik varsa karar verilmez.
const MIN_TATUMS: usize = 48;

/// Bulunan aksak ölçü.
#[derive(Debug, Clone, PartialEq)]
pub struct Aksak {
    /// Sekizlik gruplaması (ör. [2, 2, 2, 3] = 9/8).
    pub grouping: Vec<u8>,
    /// Şarkının vuruşları: grup başları (saniye, artan).
    pub beats: Vec<f64>,
    /// İlk ölçü başının `beats` içindeki sırası.
    pub downbeat_phase: usize,
    /// Güven (0..1).
    pub confidence: f32,
}

impl Aksak {
    /// Vuruş ızgarası (grup başları); tempo, grup başı başına ortalama.
    pub fn beat_grid(&self) -> BeatGrid {
        let bpm = (600.0 / mean_interval(&self.beats)).round() / 10.0;
        BeatGrid {
            bpm,
            beats: self.beats.clone(),
        }
    }

    /// Ölçüdeki sekizlik sayısı (ör. 9).
    pub fn tatums(&self) -> usize {
        self.grouping.iter().map(|&g| usize::from(g)).sum()
    }
}

/// Bir kalıbın en iyi başlangıç konumu ve ilintisi.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Fit {
    correlation: f64,
    phase: usize,
}

/// Aksak ölçüyü arar. `onset`: kare başına başlangıç gücü; `bands`: kare başına
/// `bands_per_frame` bant seviyesi (0..255); `grid`: takipçinin vuruşları; `latency`:
/// başlangıç gücünün gecikmesi ([`super::beats::onset_latency`]). Aksak değilse `None`.
pub fn detect(
    onset: &[f32],
    bands: &[u8],
    bands_per_frame: usize,
    grid: &BeatGrid,
    fps: f64,
    latency: f64,
) -> Option<Aksak> {
    if grid.bpm <= 0.0 || bands_per_frame < BASS_BANDS {
        return None;
    }
    let beat = 60.0 / grid.bpm;
    let mut best: Option<(f64, Aksak)> = None;
    for tatum in [beat / 2.0, beat] {
        let Some((times, strengths)) = track_period(onset, fps, latency, tatum) else {
            continue;
        };
        if times.len() < MIN_TATUMS {
            continue;
        }
        let accent = accents(&times, &strengths, bands, bands_per_frame, fps, latency);
        let regular = REGULAR
            .iter()
            .map(|g| fit(&accent, g).correlation)
            .fold(f64::MIN, f64::max);
        for &groups in AKSAK {
            let candidate = fit(&accent, groups);
            let margin = candidate.correlation - regular;
            if candidate.correlation < MIN_CORRELATION || margin < MIN_MARGIN {
                continue;
            }
            if best.as_ref().is_some_and(|(m, _)| *m >= margin) {
                continue;
            }
            let aksak = pulses(&times, groups, candidate.phase, margin);
            if aksak.beats.len() >= 8 {
                best = Some((margin, aksak));
            }
        }
    }
    best.map(|(_, aksak)| aksak)
}

/// Her sekizliğin vurgusu: başlangıç gücü + bas vuruşu, ikisi de şarkıya göre ölçeklenmiş.
fn accents(
    times: &[f64],
    strengths: &[f64],
    bands: &[u8],
    bands_per_frame: usize,
    fps: f64,
    latency: f64,
) -> Vec<f64> {
    let frames = bands.len() / bands_per_frame;
    let bass = |f: usize| {
        bands[f * bands_per_frame..f * bands_per_frame + BASS_BANDS]
            .iter()
            .map(|&b| f64::from(b))
            .sum::<f64>()
            / (BASS_BANDS as f64 * 255.0)
    };
    let reach = ((KICK_REACH_SECONDS * fps).round() as usize).max(1);
    let kicks: Vec<f64> = times
        .iter()
        .map(|&t| {
            let f = (((t + latency) * fps).round().max(0.0) as usize).min(frames.saturating_sub(1));
            let after = (f..(f + reach).min(frames)).map(bass).fold(0.0, f64::max);
            let before = (f.saturating_sub(reach)..f).map(bass).fold(1.0, f64::min);
            (after - before.min(after)).max(0.0)
        })
        .collect();
    zscores(strengths)
        .iter()
        .zip(zscores(&kicks))
        .map(|(o, k)| o + k)
        .collect()
}

/// Kalıbın (gruplama) en iyi başlangıç konumu: vurgu dizisiyle ilintisi en yüksek olan.
fn fit(accent: &[f64], groups: &[u8]) -> Fit {
    let template = template(groups);
    let n = template.len();
    (0..n)
        .map(|phase| {
            let pattern: Vec<f64> = (0..accent.len())
                .map(|k| template[(k + n - phase % n) % n])
                .collect();
            Fit {
                correlation: correlation(accent, &pattern),
                phase,
            }
        })
        .max_by(|a, b| a.correlation.total_cmp(&b.correlation))
        .unwrap_or(Fit {
            correlation: 0.0,
            phase: 0,
        })
}

/// Bir ölçünün vurgu kalıbı: ölçü başı, grup başları, diğerleri.
fn template(groups: &[u8]) -> Vec<f64> {
    let mut out = Vec::new();
    for (i, &g) in groups.iter().enumerate() {
        out.push(if i == 0 {
            DOWNBEAT_WEIGHT
        } else {
            GROUP_WEIGHT
        });
        out.extend(std::iter::repeat_n(0.0, usize::from(g).saturating_sub(1)));
    }
    out
}

/// Izgaradan grup başları ve ilk ölçü başının sırası.
fn pulses(times: &[f64], groups: &[u8], phase: usize, margin: f64) -> Aksak {
    let n: usize = groups.iter().map(|&g| usize::from(g)).sum();
    let mut starts = vec![false; n];
    let mut position = 0;
    for &g in groups {
        starts[position] = true;
        position += usize::from(g);
    }
    let mut beats = Vec::new();
    let mut downbeat_phase = None;
    for (k, &t) in times.iter().enumerate() {
        let position = (k + n - phase % n) % n;
        if starts[position] {
            if position == 0 && downbeat_phase.is_none() {
                downbeat_phase = Some(beats.len());
            }
            beats.push(t);
        }
    }
    Aksak {
        grouping: groups.to_vec(),
        beats,
        downbeat_phase: downbeat_phase.unwrap_or(0),
        confidence: ((margin - MIN_MARGIN) / (CONFIDENT_MARGIN - MIN_MARGIN)).clamp(0.0, 1.0)
            as f32,
    }
}

fn correlation(a: &[f64], b: &[f64]) -> f64 {
    let n = a.len().min(b.len());
    if n < 2 {
        return 0.0;
    }
    let ma = a[..n].iter().sum::<f64>() / n as f64;
    let mb = b[..n].iter().sum::<f64>() / n as f64;
    let (mut ab, mut aa, mut bb) = (0.0, 0.0, 0.0);
    for (x, y) in a[..n].iter().zip(&b[..n]) {
        ab += (x - ma) * (y - mb);
        aa += (x - ma) * (x - ma);
        bb += (y - mb) * (y - mb);
    }
    if aa <= 1e-12 || bb <= 1e-12 {
        0.0
    } else {
        ab / (aa * bb).sqrt()
    }
}

fn zscores(values: &[f64]) -> Vec<f64> {
    let n = values.len().max(1) as f64;
    let mean = values.iter().sum::<f64>() / n;
    let deviation = (values.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / n).sqrt();
    values
        .iter()
        .map(|x| {
            if deviation > 1e-12 {
                (x - mean) / deviation
            } else {
                0.0
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::analysis::beats;
    use crate::analysis::spectrogram::{analyze, Spectrogram};
    use crate::audio::decode::Decoder;
    use crate::audio::test_util::{temp_path, write_wav};

    /// Aksak (ya da düzenli) davul kaydı: sekizlik ızgarası, her grup başında bas davul (ölçü
    /// başında daha güçlü), her sekizlikte zil; isteğe bağlı olarak grup başlarında darbuka
    /// gibi "tek" sesi. Grup başlarının zamanları (doğru vuruşlar) ve ölçü başları da döner.
    pub(crate) fn aksak_drums(
        eighth_bpm: f64,
        groups: &[usize],
        seconds: f64,
        rate: u32,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        aksak_drums_with(eighth_bpm, groups, seconds, rate, 0.9)
    }

    /// [`aksak_drums`]; `downbeat_amp`: ölçü başındaki bas davulun gücü (diğer grup başları 0,5).
    fn aksak_drums_with(
        eighth_bpm: f64,
        groups: &[usize],
        seconds: f64,
        rate: u32,
        downbeat_amp: f64,
    ) -> (Vec<f64>, Vec<f64>, Vec<f64>) {
        let n = (seconds * f64::from(rate)) as usize;
        let mut out = vec![0.0f64; n];
        let eighth = 60.0 / eighth_bpm;
        let bar: usize = groups.iter().sum();
        let mut starts = vec![false; bar];
        let mut acc = 0;
        for &g in groups {
            starts[acc] = true;
            acc += g;
        }
        let (mut beats, mut downbeats) = (Vec::new(), Vec::new());
        let mut state = 99u64;
        let mut noise = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0
        };
        for k in 0.. {
            let t = 0.5 + k as f64 * eighth;
            if t > seconds - 0.4 {
                break;
            }
            let position = k % bar;
            let s0 = (t * f64::from(rate)) as usize;
            if starts[position] {
                beats.push(t);
                if position == 0 {
                    downbeats.push(t);
                }
                let amp = if position == 0 { downbeat_amp } else { 0.5 };
                let mut phase = 0.0;
                for i in 0..((0.25 * f64::from(rate)) as usize).min(n - s0) {
                    let x = i as f64 / f64::from(rate);
                    phase += 2.0 * std::f64::consts::PI * (50.0 + 70.0 * (-x * 30.0).exp())
                        / f64::from(rate);
                    out[s0 + i] += amp * phase.sin() * (-x * 14.0).exp();
                }
            }
            for i in 0..((0.05 * f64::from(rate)) as usize).min(n - s0) {
                let x = i as f64 / f64::from(rate);
                out[s0 + i] += 0.15 * noise() * (-x * 60.0).exp();
            }
        }
        (out, beats, downbeats)
    }

    /// Ölçüsü belirsiz davul: her vuruşta bas davul ve her sekizlikte zil; ama her ölçüde vurgu
    /// (güçlü bas davul) rastgele bir vuruşa düşer. Ölçü başını gösteren tutarlı bir şey yok.
    pub(crate) fn shifting_drums(eighth_bpm: f64, seconds: f64, rate: u32) -> Vec<f64> {
        let (mut out, _, _) = aksak_drums_with(eighth_bpm, &[2], seconds, rate, 0.5);
        let eighth = 60.0 / eighth_bpm;
        let n = out.len();
        let mut state = 17u64;
        for bar in 0.. {
            let start = 0.5 + bar as f64 * 8.0 * eighth;
            if start > seconds - 1.0 {
                break;
            }
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let beat = (state >> 33) % 4;
            let s0 = ((start + beat as f64 * 2.0 * eighth) * f64::from(rate)) as usize;
            let mut phase = 0.0;
            for i in 0..((0.25 * f64::from(rate)) as usize).min(n - s0) {
                let x = i as f64 / f64::from(rate);
                phase += 2.0 * std::f64::consts::PI * (50.0 + 70.0 * (-x * 30.0).exp())
                    / f64::from(rate);
                out[s0 + i] += 0.5 * phase.sin() * (-x * 14.0).exp();
            }
        }
        out
    }

    /// Kaydı tam analizden geçirir: başlangıç gücü, bantlar ve takipçinin vuruşları.
    fn analyzed(samples: &[f64], rate: u32) -> (Vec<f32>, Vec<u8>, BeatGrid, f64) {
        let path = temp_path("aksak.wav");
        write_wav(&path, rate, 1, samples.len(), |f, _| {
            samples[f].clamp(-1.0, 1.0)
        });
        let spectrogram = Spectrogram::new();
        analyze(Decoder::open(&path).unwrap(), &spectrogram);
        let saved = spectrogram.saved().unwrap();
        std::fs::remove_file(path).ok();
        let latency = beats::onset_latency(rate, 2048);
        let grid = beats::track(&saved.onset, 60.0, latency).unwrap();
        (saved.onset, saved.levels, grid, latency)
    }

    fn detect_in(samples: &[f64], rate: u32) -> Option<Aksak> {
        let (onset, levels, grid, latency) = analyzed(samples, rate);
        detect(&onset, &levels, 32, &grid, 60.0, latency)
    }

    #[test]
    fn kalip_olcu_basi_ve_grup_baslarini_vurgular() {
        assert_eq!(template(&[2, 2, 2, 3]).len(), 9);
        assert_eq!(
            template(&[2, 3]),
            vec![DOWNBEAT_WEIGHT, 0.0, GROUP_WEIGHT, 0.0, 0.0]
        );
        // Kusursuz 7/8 (2+2+3) vurgusu: kendi kalıbı ve konumu en iyi; 4/4 kalıbı kötü.
        let accent: Vec<f64> = (0..140)
            .map(|k| match (k + 7 - 3) % 7 {
                0 => 2.0,
                2 | 4 => 1.0,
                _ => -0.5,
            })
            .collect();
        let own = fit(&accent, &[2, 2, 3]);
        assert_eq!(own.phase, 3);
        assert!(own.correlation > 0.95, "{own:?}");
        assert!(fit(&accent, &[2, 2, 2, 2]).correlation < 0.3);
    }

    #[test]
    fn dokuz_sekizlik_bulunur() {
        let rate = 22_050;
        for eighth_bpm in [200.0, 270.0, 330.0] {
            let (samples, truth, downbeats) = aksak_drums(eighth_bpm, &[2, 2, 2, 3], 40.0, rate);
            let aksak = detect_in(&samples, rate).unwrap_or_else(|| panic!("{eighth_bpm}"));
            assert_eq!(aksak.grouping, vec![2, 2, 2, 3], "{eighth_bpm}");
            assert!(aksak.confidence > 0.5, "{eighth_bpm}: {aksak:?}");
            // Vuruşlar grup başlarına, ölçü başı gerçek ölçü başına oturur.
            let f = crate::analysis::evaluate::f_measure(&aksak.beats, &truth, 0.05);
            assert!(f > 0.95, "{eighth_bpm}: F {f}");
            let first = aksak.beats[aksak.downbeat_phase];
            assert!(
                downbeats.iter().any(|d| (d - first).abs() < 0.05),
                "{eighth_bpm}: ölçü başı {first}"
            );
        }
    }

    #[test]
    fn yedi_ve_bes_sekizlik_bulunur() {
        let rate = 22_050;
        for (eighth_bpm, groups) in [
            (240.0, vec![2, 2, 3]),
            (300.0, vec![3, 2, 2]),
            (200.0, vec![3, 2]),
        ] {
            let (samples, truth, _) = aksak_drums(eighth_bpm, &groups, 40.0, rate);
            let aksak = detect_in(&samples, rate).unwrap_or_else(|| panic!("{groups:?}"));
            let expected: Vec<u8> = groups.iter().map(|&g| g as u8).collect();
            assert_eq!(aksak.grouping, expected, "{eighth_bpm}");
            let f = crate::analysis::evaluate::f_measure(&aksak.beats, &truth, 0.05);
            assert!(f > 0.95, "{groups:?}: F {f}");
        }
    }

    #[test]
    fn duzenli_olculer_aksak_sayilmaz() {
        let rate = 22_050;
        // 4/4 ve 3/4 sekizlik zilli davul; ve projenin standart 4/4 davul kaydı.
        for (eighth_bpm, groups) in [(240.0, vec![2, 2, 2, 2]), (200.0, vec![2, 2, 2])] {
            let (samples, _, _) = aksak_drums(eighth_bpm, &groups, 40.0, rate);
            assert_eq!(detect_in(&samples, rate), None, "{groups:?}");
        }
        for bpm in [90.0, 124.0, 150.0] {
            let (samples, _) = beats::tests::drum_track(bpm, 0.6, 40.0, rate, 3);
            assert_eq!(detect_in(&samples, rate), None, "{bpm}");
        }
    }
}
