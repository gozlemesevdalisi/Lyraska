//! Şarkı yapısı: ölçü başları (downbeat), bölümler, droplar ve enerji eğrisi.
//!
//! Beat ızgarası bulunduktan sonra, spektrogram geçişinde biriken bant
//! seviyeleri ve kanal seviyeleriyle hesaplanır (şarkı ikinci kez çözülmez).
//!
//! - **Ölçü başı:** Her vuruş için iki işaret: kick gücü (bas bantlarının vuruş
//!   anındaki yükselişi) ve tını/armoni değişimi (vuruşun ortalama spektrumunun
//!   bir öncekinden farkı; akor ve bas notası genelde ölçü başında değişir).
//!   4/4 ve 3/4 için her olası başlangıç vuruşu puanlanır; belirgin biçimde
//!   üstün değilse 4/4 kabul edilir.
//! - **Bölümler:** Ölçü başına özellik vektörü (bant seviyeleri, ses yüksekliği,
//!   bas). Benzerlik matrisi üzerinde dama tahtası çekirdeğiyle yenilik eğrisi
//!   (Foote 2000); tepeleri bölüm sınırıdır. En kısa bölüm 4 ölçü.
//! - **Drop:** Bassız ya da sakin bir kısımdan sonra bas ve enerjinin birden
//!   güçlü döndüğü ölçü başı (elektronik müzikteki "drop", rock'taki büyük giriş).
//! - **Enerji:** Ses yüksekliğinin 1 sn'lik ortalaması; şarkının en sessiz %5'i
//!   0, en yüksek %5'i 1 olacak biçimde.

use serde::{Deserialize, Serialize};

use super::beats::BeatGrid;

/// Bas sayılan spektrum bantları (32 bandın ilk 8'i: ~30–150 Hz).
const BASS_BANDS: usize = 8;
/// Bölüm sınırı aramada çekirdeğin yarı genişliği (ölçü).
const NOVELTY_HALF_WIDTH: usize = 4;
/// En kısa bölüm (ölçü).
const MIN_SECTION_BARS: usize = 4;
/// Dropun öncesi ve sonrası kaç ölçüye bakılır.
const DROP_BEFORE_BARS: usize = 4;
const DROP_AFTER_BARS: usize = 2;
/// Drop için bas seviyesindeki en az artış (bant ölçeği: 0,1 ≈ 6 dB).
const DROP_MIN_BASS_JUMP: f64 = 0.12;
/// İki drop arası en az (ölçü).
const DROP_MIN_GAP_BARS: usize = 16;

/// Şarkının yapısı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SongMap {
    /// Ölçüdeki vuruş sayısı (3 ya da 4).
    pub meter: usize,
    /// İlk tam ölçünün başladığı vuruşun sırası (0..meter).
    pub downbeat_phase: usize,
    /// Ölçü başlarının zamanları (saniye).
    pub downbeats: Vec<f64>,
    pub sections: Vec<Section>,
    /// Drop anları (saniye).
    pub drops: Vec<f64>,
    /// Saniyede bir enerji değeri (0..1).
    pub energy: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct Section {
    pub start: f64,
    pub end: f64,
    /// Bölümün ortalama enerjisi (0..1).
    pub energy: f32,
    /// Benzer bölümler aynı etiketi alır (ör. her nakarat): 0, 1, 2… ilk görülme sırasıyla.
    pub label: usize,
}

impl SongMap {
    /// Verilen andaki enerji (0..1).
    pub fn energy_at(&self, seconds: f64) -> Option<f32> {
        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }
        self.energy.get(seconds as usize).copied()
    }

    /// Vuruşun ölçüdeki yeri (1 = ölçü başı).
    pub fn bar_beat(&self, beat_index: usize) -> usize {
        let meter = self.meter.max(1) as isize;
        (beat_index as isize - self.downbeat_phase as isize).rem_euclid(meter) as usize + 1
    }

    /// Verilen andaki bölümün sırası.
    pub fn section_at(&self, seconds: f64) -> Option<usize> {
        self.sections
            .iter()
            .position(|s| seconds >= s.start && seconds < s.end)
    }
}

/// Analiz verilerinden şarkının yapısını çıkarır.
///
/// `bands`: kare başına `bands_per_frame` seviye (0..255); `loudness_db`: kare başına
/// ses yüksekliği (dBFS); `fps`: saniyedeki kare.
pub fn map_song(
    bands: &[u8],
    bands_per_frame: usize,
    loudness_db: &[f32],
    beats: &BeatGrid,
    fps: f64,
) -> Option<SongMap> {
    let frames = loudness_db.len().min(bands.len() / bands_per_frame.max(1));
    if beats.beats.len() < 8 || frames == 0 || bands_per_frame < BASS_BANDS {
        return None;
    }
    let level =
        |frame: usize, band: usize| f64::from(bands[frame * bands_per_frame + band]) / 255.0;
    let frame_of = |t: f64| ((t * fps).round().max(0.0) as usize).min(frames - 1);

    // Vuruş başına: ortalama spektrum, bas, kick gücü.
    let beat_frames: Vec<(usize, usize)> = beats
        .beats
        .iter()
        .enumerate()
        .map(|(i, &t)| {
            let start = frame_of(t);
            let end = beats
                .beats
                .get(i + 1)
                .map_or(start + (fps * 60.0 / beats.bpm) as usize, |&n| frame_of(n))
                .clamp(start + 1, frames);
            (start, end)
        })
        .collect();
    let spectra: Vec<Vec<f64>> = beat_frames
        .iter()
        .map(|&(s, e)| {
            (0..bands_per_frame)
                .map(|b| (s..e).map(|f| level(f, b)).sum::<f64>() / (e - s) as f64)
                .collect()
        })
        .collect();
    let bass_at = |f: usize| (0..BASS_BANDS).map(|b| level(f, b)).sum::<f64>() / BASS_BANDS as f64;
    let reach = (fps * 0.1) as usize;
    let kicks: Vec<f64> = beat_frames
        .iter()
        .map(|&(s, _)| {
            let after = (s..(s + reach).min(frames))
                .map(bass_at)
                .fold(0.0, f64::max);
            let before = (s.saturating_sub(reach)..s)
                .map(bass_at)
                .fold(1.0, f64::min);
            (after - before.min(after)).max(0.0)
        })
        .collect();
    let changes: Vec<f64> = (0..spectra.len())
        .map(|i| match i.checked_sub(1) {
            Some(p) => distance(&spectra[p], &spectra[i]),
            None => 0.0,
        })
        .collect();

    // Ölçü ve ölçü başı.
    let kick_z = zscores(&kicks);
    let change_z = zscores(&changes);
    let score: Vec<f64> = kick_z.iter().zip(&change_z).map(|(k, c)| k + c).collect();
    let (meter, downbeat_phase) = choose_meter(&score);
    let downbeat_indices: Vec<usize> = (downbeat_phase..beats.beats.len()).step_by(meter).collect();
    let downbeats: Vec<f64> = downbeat_indices.iter().map(|&i| beats.beats[i]).collect();

    // Ses yüksekliğinden enerji (şarkıya göre 0..1).
    let (low, high) = percentile_range(loudness_db, 0.05, 0.95);
    let normalize = |db: f64| ((db - low) / (high - low).max(1.0)).clamp(0.0, 1.0);
    let seconds = (frames as f64 / fps).ceil() as usize;
    let energy: Vec<f32> = (0..seconds)
        .map(|s| {
            let a = (s as f64 * fps) as usize;
            let b = (((s + 1) as f64 * fps) as usize).min(frames);
            if a >= b {
                return 0.0;
            }
            let mean =
                loudness_db[a..b].iter().map(|&d| f64::from(d)).sum::<f64>() / (b - a) as f64;
            normalize(mean) as f32
        })
        .collect();

    // Ölçü başına özellikler.
    let bars: Vec<(usize, usize)> = downbeats
        .iter()
        .enumerate()
        .map(|(k, &t)| {
            let s = frame_of(t);
            let e = downbeats
                .get(k + 1)
                .map_or(frames, |&n| frame_of(n))
                .max(s + 1);
            (s, e.min(frames))
        })
        .filter(|(s, e)| e > s)
        .collect();
    let bar_bass: Vec<f64> = bars
        .iter()
        .map(|&(s, e)| (s..e).map(bass_at).sum::<f64>() / (e - s) as f64)
        .collect();
    let bar_energy: Vec<f64> = bars
        .iter()
        .map(|&(s, e)| {
            normalize(loudness_db[s..e].iter().map(|&d| f64::from(d)).sum::<f64>() / (e - s) as f64)
        })
        .collect();
    let bar_features: Vec<Vec<f64>> = bars
        .iter()
        .enumerate()
        .map(|(k, &(s, e))| {
            let mut v: Vec<f64> = (0..bands_per_frame)
                .map(|b| (s..e).map(|f| level(f, b)).sum::<f64>() / (e - s) as f64)
                .collect();
            // Ses yüksekliği ve bas, tını kadar önemli: ağırlıklı ekle.
            v.extend(std::iter::repeat_n(bar_energy[k], 6));
            v.extend(std::iter::repeat_n(bar_bass[k], 6));
            v
        })
        .collect();

    let boundaries = section_boundaries(&bar_features);
    let bar_time = |k: usize| {
        downbeats
            .get(k)
            .copied()
            .unwrap_or(downbeats[downbeats.len() - 1])
    };
    let song_end = frames as f64 / fps;
    let mut cuts: Vec<usize> = vec![0];
    cuts.extend(boundaries);
    let sections: Vec<Section> = cuts
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let end_bar = cuts.get(i + 1).copied().unwrap_or(bars.len());
            let start = if i == 0 { 0.0 } else { bar_time(k) };
            let end = if end_bar >= bars.len() {
                song_end
            } else {
                bar_time(end_bar)
            };
            let energy = mean(&bar_energy[k..end_bar.max(k + 1).min(bar_energy.len())]) as f32;
            Section {
                start,
                end,
                energy,
                label: 0,
            }
        })
        .collect();
    let mut sections = sections;
    let section_features: Vec<Vec<f64>> = cuts
        .iter()
        .enumerate()
        .map(|(i, &k)| {
            let end_bar = cuts.get(i + 1).copied().unwrap_or(bars.len()).max(k + 1);
            average(&bar_features[k..end_bar.min(bar_features.len())])
        })
        .collect();
    for (section, label) in sections.iter_mut().zip(label_sections(&section_features)) {
        section.label = label;
    }

    let drops = find_drops(&bar_bass, &bar_energy)
        .into_iter()
        .map(bar_time)
        .collect();

    Some(SongMap {
        meter,
        downbeat_phase,
        downbeats,
        sections,
        drops,
        energy,
    })
}

/// Ölçü (3 ya da 4) ve ilk ölçü başının sırası.
fn choose_meter(score: &[f64]) -> (usize, usize) {
    let best = |meter: usize| {
        let phases: Vec<f64> = (0..meter)
            .map(|p| {
                mean(
                    &score
                        .iter()
                        .skip(p)
                        .step_by(meter)
                        .copied()
                        .collect::<Vec<_>>(),
                )
            })
            .collect();
        let (phase, top) = phases
            .iter()
            .copied()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap_or((0, 0.0));
        let others = (phases.iter().sum::<f64>() - top) / (meter - 1) as f64;
        (phase, top - others)
    };
    let (phase4, contrast4) = best(4);
    let (phase3, contrast3) = best(3);
    // 3/4 ancak açıkça daha uygunsa: müziğin çoğu 4/4.
    if contrast3 > 1.5 * contrast4.max(0.05) && contrast3 > 0.5 {
        (3, phase3)
    } else {
        (4, phase4)
    }
}

/// Bölüm sınırları (ölçü sırası): Foote yenilik eğrisinin tepeleri.
fn section_boundaries(features: &[Vec<f64>]) -> Vec<usize> {
    let n = features.len();
    let w = NOVELTY_HALF_WIDTH;
    if n < 2 * w + 2 {
        return Vec::new();
    }
    // Her boyutu standartlaştır: büyük değerli boyutlar baskın olmasın.
    let dims = features[0].len();
    let mut normalized = features.to_vec();
    for d in 0..dims {
        let column: Vec<f64> = features.iter().map(|f| f[d]).collect();
        let z = zscores(&column);
        for (row, value) in normalized.iter_mut().zip(z) {
            row[d] = value;
        }
    }
    let similarity = |a: usize, b: usize| -distance(&normalized[a], &normalized[b]);
    let novelty: Vec<f64> = (0..n)
        .map(|k| {
            if k < w || k + w > n {
                return 0.0;
            }
            let mut sum = 0.0;
            for i in k - w..k + w {
                for j in k - w..k + w {
                    let same_side = (i < k) == (j < k);
                    let taper = gauss(i as f64 + 0.5 - k as f64, w as f64)
                        * gauss(j as f64 + 0.5 - k as f64, w as f64);
                    // Sınırın aynı tarafı birbirine benzer, karşı tarafla farklıysa yenilik yüksek.
                    let sign = if same_side { 1.0 } else { -1.0 };
                    sum += sign * taper * similarity(i, j);
                }
            }
            sum
        })
        .collect();
    let threshold = mean(&novelty) + 0.5 * std_dev(&novelty);
    let mut peaks: Vec<usize> = (w..n.saturating_sub(w - 1))
        .filter(|&k| {
            novelty[k] > threshold
                && (k.saturating_sub(MIN_SECTION_BARS)..(k + MIN_SECTION_BARS).min(n))
                    .all(|j| novelty[j] <= novelty[k])
        })
        .collect();
    peaks.dedup();
    peaks
}

/// Benzer bölümlere aynı etiket: bölümler sırayla, mevcut etiketlerin ilk
/// örneğine yakınsa o etiketi, değilse yeni etiket alır. "Yakın": bölümler arası
/// uzaklıkların ortalamasının yarısından az.
fn label_sections(features: &[Vec<f64>]) -> Vec<usize> {
    let n = features.len();
    if n == 0 {
        return Vec::new();
    }
    // Boyutları bölümler arasında standartlaştır.
    let dims = features[0].len();
    let mut normalized = features.to_vec();
    for d in 0..dims {
        let column: Vec<f64> = features.iter().map(|f| f[d]).collect();
        for (row, value) in normalized.iter_mut().zip(zscores(&column)) {
            row[d] = value;
        }
    }
    let mut pairs = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            pairs.push(distance(&normalized[i], &normalized[j]));
        }
    }
    let threshold = 0.5 * mean(&pairs);
    let mut prototypes: Vec<usize> = Vec::new();
    (0..n)
        .map(|i| {
            let nearest = prototypes
                .iter()
                .enumerate()
                .map(|(label, &p)| (label, distance(&normalized[i], &normalized[p])))
                .min_by(|a, b| a.1.total_cmp(&b.1));
            match nearest {
                Some((label, d)) if d < threshold => label,
                _ => {
                    prototypes.push(i);
                    prototypes.len() - 1
                }
            }
        })
        .collect()
}

fn average(rows: &[Vec<f64>]) -> Vec<f64> {
    let Some(first) = rows.first() else {
        return Vec::new();
    };
    (0..first.len())
        .map(|d| rows.iter().map(|r| r[d]).sum::<f64>() / rows.len() as f64)
        .collect()
}

/// Drop olan ölçüler (sırası).
fn find_drops(bar_bass: &[f64], bar_energy: &[f64]) -> Vec<usize> {
    let n = bar_bass.len();
    let mut candidates: Vec<(usize, f64)> = (DROP_BEFORE_BARS
        ..n.saturating_sub(DROP_AFTER_BARS - 1))
        .filter_map(|k| {
            let before_bass = mean(&bar_bass[k - DROP_BEFORE_BARS..k]);
            let after_bass = mean(&bar_bass[k..(k + DROP_AFTER_BARS).min(n)]);
            let before_energy = mean(&bar_energy[k - DROP_BEFORE_BARS..k]);
            let after_energy = mean(&bar_energy[k..(k + DROP_AFTER_BARS).min(n)]);
            let jump = after_bass - before_bass;
            // Bas birden güçlü dönmeli, sonrası şarkının yüksek kısmı olmalı, öncesi sessiz olmamalı.
            (jump >= DROP_MIN_BASS_JUMP && after_energy >= 0.65 && before_energy > 0.05)
                .then_some((k, jump + (after_energy - before_energy)))
        })
        .collect();
    // En güçlüsünden başlayarak, birbirine yakın adayları ele.
    candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
    let mut chosen: Vec<usize> = Vec::new();
    for (k, _) in candidates {
        if chosen.iter().all(|&c| c.abs_diff(k) >= DROP_MIN_GAP_BARS) {
            chosen.push(k);
        }
    }
    chosen.sort_unstable();
    chosen
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y).powi(2))
        .sum::<f64>()
        .sqrt()
}

fn gauss(x: f64, width: f64) -> f64 {
    (-0.5 * (x / (0.5 * width)).powi(2)).exp()
}

fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}

fn std_dev(values: &[f64]) -> f64 {
    let m = mean(values);
    mean(&values.iter().map(|v| (v - m).powi(2)).collect::<Vec<_>>()).sqrt()
}

fn zscores(values: &[f64]) -> Vec<f64> {
    let m = mean(values);
    let s = std_dev(values).max(1e-9);
    values.iter().map(|v| (v - m) / s).collect()
}

fn percentile_range(values: &[f32], low: f64, high: f64) -> (f64, f64) {
    let mut sorted: Vec<f64> = values.iter().map(|&v| f64::from(v)).collect();
    sorted.sort_by(f64::total_cmp);
    if sorted.is_empty() {
        return (0.0, 1.0);
    }
    let at = |p: f64| sorted[((sorted.len() - 1) as f64 * p).round() as usize];
    (at(low), at(high))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::beats::tests::{drum_track, Lcg};
    use crate::analysis::spectrogram::{analyze, Spectrogram};
    use crate::audio::decode::Decoder;
    use crate::audio::test_util::{temp_path, write_wav};
    use std::f64::consts::PI;
    use std::sync::Arc;

    fn analyze_samples(samples: &[f64], rate: u32) -> Arc<Spectrogram> {
        let path = temp_path("yapi.wav");
        write_wav(&path, rate, 1, samples.len(), |f, _| {
            samples[f].clamp(-1.0, 1.0)
        });
        let spectrogram = Spectrogram::new();
        analyze(Decoder::open(&path).unwrap(), &spectrogram);
        std::fs::remove_file(&path).ok();
        spectrogram
    }

    #[test]
    fn olcu_basi_bas_notasinin_degistigi_vurus() {
        // Davul kaydında bas notası her ölçünün ilk vuruşunda değişir (kick 1 ve 3'te).
        let rate = 22_050;
        let (samples, truth) = drum_track(120.0, 0.8, 32.0, rate, 4);
        let spectrogram = analyze_samples(&samples, rate);
        let map = spectrogram.song_map().expect("yapı bulunmalı");
        assert_eq!(map.meter, 4);
        let on_bar_start = map
            .downbeats
            .iter()
            .filter(|&&d| {
                let i = truth
                    .iter()
                    .enumerate()
                    .min_by(|a, b| (a.1 - d).abs().total_cmp(&(b.1 - d).abs()))
                    .unwrap()
                    .0;
                (truth[i] - d).abs() < 0.07 && i % 4 == 0
            })
            .count();
        assert!(
            on_bar_start as f64 >= 0.9 * map.downbeats.len() as f64,
            "{on_bar_start}/{} ölçü başı doğru",
            map.downbeats.len()
        );
    }

    /// Elektronik müzik kalıbı: ana kısım (8 ölçü) → sakin kısım (8) → yükseliş (4)
    /// → drop (8) → kapanış (4). 128 BPM, 0,5 sn sessiz başlangıç.
    fn edm_track(rate: u32) -> (Vec<f64>, f64, f64) {
        let bpm = 128.0;
        let beat = 60.0 / bpm;
        let start = 0.5;
        let bars = 32;
        let n = ((start + bars as f64 * 4.0 * beat + 1.0) * f64::from(rate)) as usize;
        let mut out = vec![0.0f64; n];
        let mut rng = Lcg(11);
        let r = f64::from(rate);
        let mut add = |at: f64, len: f64, f: &mut dyn FnMut(f64) -> f64| {
            let s0 = (at * r) as usize;
            for i in 0..((len * r) as usize).min(n.saturating_sub(s0)) {
                out[s0 + i] += f(i as f64 / r);
            }
        };
        for bar in 0..bars {
            let section = match bar {
                0..=7 => "ana",
                8..=15 => "sakin",
                16..=19 => "yukselis",
                20..=27 => "drop",
                _ => "kapanis",
            };
            for b in 0..4 {
                let t = start + (bar * 4 + b) as f64 * beat;
                let full = section == "ana" || section == "drop";
                let gain = if section == "drop" { 1.0 } else { 0.8 };
                if full {
                    let mut phase = 0.0;
                    add(t, 0.25, &mut |x| {
                        phase += 2.0 * PI * (45.0 + 70.0 * (-x * 30.0).exp()) / r;
                        0.6 * gain * phase.sin() * (-x * 12.0).exp()
                    });
                    let note = [55.0, 55.0, 49.0, 65.4][bar % 4];
                    add(t, beat * 0.9, &mut |x| {
                        0.3 * gain * (2.0 * PI * note * x).sin()
                    });
                }
                if section != "kapanis" {
                    // Hi-hat: sekizlikler.
                    for half in [0.0, 0.5] {
                        let mut previous = 0.0;
                        let mut noise = Lcg(rng.0.wrapping_add((bar * 8 + b * 2) as u64));
                        add(t + half * beat, 0.04, &mut |x| {
                            let v = noise.next();
                            let hp = v - previous;
                            previous = v;
                            0.08 * hp * (-x * 90.0).exp()
                        });
                    }
                }
                if section == "yukselis" {
                    // Yükselen gürültü ve sıklaşan trampet.
                    let progress = ((bar - 16) * 4 + b) as f64 / 16.0;
                    let mut noise = Lcg(rng.0 ^ (bar * 4 + b) as u64);
                    add(t, beat, &mut |_| 0.05 * (0.3 + progress) * noise.next());
                }
            }
            // Akor (her yerde; sakin kısımda daha belirgin).
            let pad = if section == "sakin" || section == "kapanis" {
                0.12
            } else {
                0.05
            };
            let t = start + (bar * 4) as f64 * beat;
            add(t, 4.0 * beat, &mut |x| {
                pad * [220.0, 261.6, 329.6]
                    .iter()
                    .map(|f| (2.0 * PI * f * x).sin())
                    .sum::<f64>()
            });
        }
        let _ = rng.next();
        let drop_time = start + 20.0 * 4.0 * beat;
        (out, drop_time, 4.0 * beat)
    }

    #[test]
    fn drop_ve_bolumler_dogru_yerde() {
        let rate = 22_050;
        let (samples, drop_time, bar) = edm_track(rate);
        let spectrogram = analyze_samples(&samples, rate);
        let map = spectrogram.song_map().expect("yapı bulunmalı");
        eprintln!(
            "ölçü {} · droplar {:?} · bölümler {:?}",
            map.meter,
            map.drops,
            map.sections
                .iter()
                .map(|s| (s.start, s.energy, s.label))
                .collect::<Vec<_>>()
        );
        assert_eq!(map.drops.len(), 1, "tek drop: {:?}", map.drops);
        // Benzer bölümler aynı etiketi alır: ana kısım ve drop aynı, sakin kısım farklı.
        let label_at = |t: f64| map.sections[map.section_at(t).unwrap()].label;
        assert_eq!(label_at(5.0), label_at(drop_time + 5.0));
        assert_ne!(label_at(20.0), label_at(drop_time + 5.0));
        assert!(
            (map.drops[0] - drop_time).abs() < 0.6 * bar / 4.0 * 4.0,
            "drop {:?}",
            map.drops
        );
        // Bölüm sınırları: 8, 16, 20, 28. ölçüler (±1 ölçü). En az üçü bulunmalı, fazlası olmamalı.
        let expected = [8.0, 16.0, 20.0, 28.0].map(|b| 0.5 + b * bar);
        let found = expected
            .iter()
            .filter(|&&e| {
                map.sections
                    .iter()
                    .any(|s| (s.start - e).abs() <= bar * 1.01)
            })
            .count();
        assert!(found >= 3, "bulunan sınır {found}/4: {:?}", map.sections);
        assert!(map.sections.len() <= 7, "{:?}", map.sections);
        // Drop bölümü sakin bölümden daha enerjik.
        let energy_at = |t: f64| map.sections[map.section_at(t).unwrap()].energy;
        assert!(energy_at(drop_time + 2.0) > energy_at(drop_time - 10.0) + 0.2);
        assert!(map.energy_at(drop_time + 2.0).unwrap() > 0.6);

        // Görsel Yönetmen aynı analizden kurulur: droptan önce gerilim, drop'ta açılım.
        let director = spectrogram.choreography().expect("koreografi kurulmalı");
        let before = director.frame_at(drop_time - 0.5, false).rhythm;
        let after = director.frame_at(drop_time + 0.1, false).rhythm;
        assert!(
            before.anticipation > 0.8 && before.release == 0.0,
            "{before:?}"
        );
        assert!(after.release > 0.9, "{after:?}");
        assert_eq!(
            director
                .frame_at(drop_time - 20.0, false)
                .rhythm
                .anticipation,
            0.0
        );
    }

    #[test]
    fn olcudeki_vurus_sirasi() {
        let map = SongMap {
            meter: 4,
            downbeat_phase: 2,
            downbeats: vec![],
            sections: vec![],
            drops: vec![],
            energy: vec![0.1, 0.9],
        };
        assert_eq!(
            (0..7).map(|i| map.bar_beat(i)).collect::<Vec<_>>(),
            [3, 4, 1, 2, 3, 4, 1]
        );
        assert_eq!(map.energy_at(1.5), Some(0.9));
        assert_eq!(map.energy_at(9.0), None);
    }

    #[test]
    fn duz_sarkida_drop_yok() {
        let rate = 22_050;
        let (samples, _) = drum_track(120.0, 0.5, 30.0, rate, 8);
        let map = analyze_samples(&samples, rate).song_map().unwrap();
        assert!(map.drops.is_empty(), "{:?}", map.drops);
        assert_eq!(map.section_at(-1.0), None);
    }
}
