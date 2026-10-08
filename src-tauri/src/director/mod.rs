//! Görsel Yönetmen: şarkıyı önceden bilen koreografi.
//!
//! Analiz bitince şarkı haritasından (vuruşlar, ölçü başları, bölümler,
//! droplar) ve spektrumdan bir koreografi kurulur. Görseller her karede o anın
//! "yönetmen notunu" ([`DirectorFrame`]) alır; üç katmandır:
//!
//! - **Atmosfer** (yavaş): bölüm, bölümün etiketi (aynı nakarat aynı tema),
//!   ruh hâli (bölüm enerjisi), sıcaklık (sakin kısım soğuk, yüksek kısım sıcak).
//! - **Ritim** (vuruşla): vuruş ve ölçü başı nabzı; **beklenti** (droptan önceki
//!   ölçülerde yavaşça yükselen gerilim) ve **boşalma** (drop anında başlayıp
//!   sönen açılım). Program dropun yerini önceden bildiği için beklentiyi
//!   kurabilir; gerçek zamanlı görselleştiriciler bunu yapamaz.
//! - **Doku** (orta hız): ayrıntı (tiz bantların canlılığı) ve hareket (vuruş
//!   yoğunluğu).
//!
//! **Güvenlik:** Nabız olayları (vuruş, ölçü başı, boşalma) arasında en az
//! [`MIN_PULSE_INTERVAL`] saniye vardır (saniyede en fazla 3); güvenli modda
//! [`SAFE_PULSE_INTERVAL`] (saniyede en fazla 1). Hızlı tempoda nabız iki
//! vuruşta bire iner. Diğer bütün değerler yumuşak değişir. Sahneler parlaklığı
//! yalnızca bu değerlerden türetir; kendi parlama testlerini de taşırlar.

use serde::Serialize;

use crate::analysis::beats::BeatGrid;
use crate::analysis::structure::SongMap;

/// Nabız olayları arası en kısa süre (saniye): saniyede en fazla 3 nabız.
pub const MIN_PULSE_INTERVAL: f64 = 0.34;
/// Epilepsi güvenli modunda: saniyede en fazla 1 nabız.
pub const SAFE_PULSE_INTERVAL: f64 = 1.0;
/// Beklentinin kurulduğu süre (ölçü): droptan önceki 8 ölçü.
const ANTICIPATION_BARS: f64 = 8.0;
/// Boşalmanın söndüğü süre (ölçü).
const RELEASE_BARS: f64 = 4.0;
/// Doku eğrilerinin çözünürlüğü (saniyede değer).
const TEXTURE_RATE: f64 = 10.0;
/// Tiz sayılan bantlar (32 bandın son 10'u: ~3 kHz üstü).
const TREBLE_FROM_BAND: usize = 22;

/// Bir anın yönetmen notu.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectorFrame {
    pub atmosphere: Atmosphere,
    pub rhythm: Rhythm,
    pub texture: Texture,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Atmosphere {
    /// Bölümün sırası.
    pub section: usize,
    /// Bölümün etiketi: benzer bölümler (ör. her nakarat) aynı temayı alır.
    pub theme: usize,
    /// Ruh hâli: 0 sakin … 1 yoğun (bölüm enerjisi, bölüm geçişlerinde yumuşak).
    pub mood: f32,
    /// Sıcaklık: 0 soğuk … 1 sıcak.
    pub warmth: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rhythm {
    /// Vuruş nabzı: vuruşta 1'e çıkar, sonra söner (0..1).
    pub pulse: f32,
    /// Ölçü başı vurgusu (0..1).
    pub accent: f32,
    /// Vuruşun içindeki yer (0..1) ve ölçünün içindeki yer (0..1).
    pub beat_phase: f32,
    pub bar_phase: f32,
    /// Droptan önceki gerilim: 0 → 1 (drop anında 1).
    pub anticipation: f32,
    /// Drop anındaki açılım: 1 → 0.
    pub release: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Texture {
    /// Ayrıntı: tizlerin canlılığı (0..1, şarkıya göre).
    pub detail: f32,
    /// Hareket: vuruş yoğunluğu (0..1, şarkıya göre).
    pub motion: f32,
}

/// Bir şarkının koreografisi.
#[derive(Debug, Clone)]
pub struct Choreography {
    beats: Vec<f64>,
    bpm: f64,
    map: SongMap,
    /// Nabız olayları (saniye): normal ve güvenli mod.
    pulses: Vec<f64>,
    safe_pulses: Vec<f64>,
    /// Ölçü başı vurguları (güvenli modda yok).
    accents: Vec<f64>,
    detail: Vec<f32>,
    motion: Vec<f32>,
}

impl Choreography {
    /// Analiz sonuçlarından koreografiyi kurar. `bands`: kare başına
    /// `bands_per_frame` seviye (0..255); `onset`: kare başına başlangıç gücü.
    pub fn build(
        beats: &BeatGrid,
        map: &SongMap,
        bands: &[u8],
        bands_per_frame: usize,
        onset: &[f32],
        fps: f64,
    ) -> Self {
        let pulses = thin(&beats.beats, MIN_PULSE_INTERVAL);
        // Güvenli mod: yalnızca ölçü başları (ve en fazla saniyede bir).
        let safe_pulses = thin(&map.downbeats, SAFE_PULSE_INTERVAL);
        let accents = thin(&map.downbeats, MIN_PULSE_INTERVAL);

        let frames = (bands.len() / bands_per_frame.max(1)).min(onset.len());
        let treble: Vec<f64> = (0..frames)
            .map(|f| {
                let row = &bands[f * bands_per_frame..(f + 1) * bands_per_frame];
                let from = TREBLE_FROM_BAND.min(row.len());
                row[from..].iter().map(|&b| f64::from(b)).sum::<f64>()
                    / (row.len() - from).max(1) as f64
                    / 255.0
            })
            .collect();
        let onsets: Vec<f64> = onset.iter().take(frames).map(|&o| f64::from(o)).collect();
        let detail = normalized(&resample_mean(&treble, fps, TEXTURE_RATE, 0.3));
        let motion = normalized(&resample_mean(&onsets, fps, TEXTURE_RATE, 1.0));

        Self {
            beats: beats.beats.clone(),
            bpm: beats.bpm,
            map: map.clone(),
            pulses,
            safe_pulses,
            accents,
            detail,
            motion,
        }
    }

    /// Verilen anın yönetmen notu. `safe`: epilepsi güvenli modu.
    pub fn frame_at(&self, seconds: f64, safe: bool) -> DirectorFrame {
        let t = if seconds.is_finite() {
            seconds.max(0.0)
        } else {
            0.0
        };
        let beat_len = 60.0 / self.bpm.max(1.0);
        let bar_len = beat_len * self.map.meter.max(1) as f64;

        // Atmosfer: bölüm ve bölüm sınırında yarım ölçülük yumuşak geçiş.
        let index = self.map.section_at(t).unwrap_or(0);
        let section = self.map.sections.get(index);
        let mut mood = f64::from(section.map_or(0.5, |s| s.energy));
        if let (Some(current), Some(next)) = (section, self.map.sections.get(index + 1)) {
            let blend =
                smoothstep(((t - (current.end - bar_len * 0.5)) / (bar_len * 0.5)).clamp(0.0, 1.0));
            mood += (f64::from(next.energy) - mood) * blend;
        }

        // Beklenti ve boşalma: en yakın drop.
        let anticipation = self
            .map
            .drops
            .iter()
            .filter(|&&d| d >= t)
            .map(|&d| {
                let x = 1.0 - (d - t) / (ANTICIPATION_BARS * bar_len);
                smoothstep(x.clamp(0.0, 1.0)).powi(2)
            })
            .fold(0.0, f64::max);
        let release = self
            .map
            .drops
            .iter()
            .filter(|&&d| d <= t)
            .map(|&d| decay(t - d, RELEASE_BARS * bar_len / 3.0))
            .fold(0.0, f64::max);

        let (pulse_events, accent_events) = if safe {
            (&self.safe_pulses, &[][..])
        } else {
            (&self.pulses, &self.accents[..])
        };
        let pulse = last_before(pulse_events, t).map_or(0.0, |p| decay(t - p, beat_len * 0.25));
        let accent = last_before(accent_events, t).map_or(0.0, |p| decay(t - p, beat_len * 0.5));

        let beat_index = self.beats.partition_point(|&b| b <= t);
        let beat_phase = match (
            beat_index.checked_sub(1).map(|i| self.beats[i]),
            self.beats.get(beat_index),
        ) {
            (Some(a), Some(&b)) if b > a => (t - a) / (b - a),
            _ => 0.0,
        };
        let bar_index = self.map.downbeats.partition_point(|&d| d <= t);
        let bar_phase = match (
            bar_index.checked_sub(1).map(|i| self.map.downbeats[i]),
            self.map.downbeats.get(bar_index),
        ) {
            (Some(a), Some(&b)) if b > a => (t - a) / (b - a),
            _ => 0.0,
        };

        let sample = |curve: &[f32]| {
            let i = (t * TEXTURE_RATE) as usize;
            curve.get(i).or(curve.last()).copied().unwrap_or(0.0)
        };

        DirectorFrame {
            atmosphere: Atmosphere {
                section: index,
                theme: section.map_or(0, |s| s.label),
                mood: mood as f32,
                warmth: (0.15 + 0.7 * mood + 0.15 * release) as f32,
            },
            rhythm: Rhythm {
                pulse: (pulse * (0.4 + 0.6 * mood)) as f32,
                accent: (accent * (0.4 + 0.6 * mood)) as f32,
                beat_phase: beat_phase as f32,
                bar_phase: bar_phase as f32,
                anticipation: anticipation as f32,
                release: release as f32,
            },
            texture: Texture {
                detail: sample(&self.detail),
                motion: sample(&self.motion),
            },
        }
    }
}

/// Olayları seyrekleştirir: bir öncekine `min_gap`'ten yakın olan atılır.
fn thin(events: &[f64], min_gap: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::with_capacity(events.len());
    for &e in events {
        if out.last().is_none_or(|&last| e - last >= min_gap) {
            out.push(e);
        }
    }
    out
}

fn last_before(events: &[f64], t: f64) -> Option<f64> {
    let i = events.partition_point(|&e| e <= t);
    i.checked_sub(1).map(|i| events[i])
}

/// Üstel sönme: 0 anında 1.
fn decay(elapsed: f64, time_constant: f64) -> f64 {
    if elapsed < 0.0 {
        0.0
    } else {
        (-elapsed / time_constant.max(1e-3)).exp()
    }
}

fn smoothstep(x: f64) -> f64 {
    x * x * (3.0 - 2.0 * x)
}

/// `values` (`from_rate` hızında) → `to_rate` hızında, `window` saniyelik ortalama.
fn resample_mean(values: &[f64], from_rate: f64, to_rate: f64, window: f64) -> Vec<f64> {
    if values.is_empty() {
        return Vec::new();
    }
    let mut prefix = vec![0.0];
    for v in values {
        prefix.push(prefix.last().copied().unwrap_or(0.0) + v);
    }
    let count = (values.len() as f64 / from_rate * to_rate).ceil() as usize;
    let half = (window * from_rate / 2.0).max(0.5) as usize;
    (0..count)
        .map(|i| {
            let center = (i as f64 / to_rate * from_rate) as usize;
            let lo = center.saturating_sub(half).min(values.len());
            let hi = (center + half + 1)
                .min(values.len())
                .max(lo + 1)
                .min(values.len());
            if hi <= lo {
                return 0.0;
            }
            (prefix[hi] - prefix[lo]) / (hi - lo) as f64
        })
        .collect()
}

/// Şarkının %5–%95 aralığına göre 0..1.
fn normalized(values: &[f64]) -> Vec<f32> {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let Some(&last) = sorted.last() else {
        return Vec::new();
    };
    let at = |p: f64| sorted[((sorted.len() - 1) as f64 * p).round() as usize];
    let (low, high) = (
        at(0.05),
        at(0.95).max(at(0.05) + 1e-9).min(last.max(at(0.05) + 1e-9)),
    );
    values
        .iter()
        .map(|v| ((v - low) / (high - low)).clamp(0.0, 1.0) as f32)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::structure::Section;

    /// `bpm` tempoda, `bars` ölçülük ızgara; 8. ölçüde drop; bölümler 0–8 (sakin), 8– (yoğun).
    fn song(bpm: f64, bars: usize) -> (BeatGrid, SongMap) {
        let beat = 60.0 / bpm;
        let beats: Vec<f64> = (0..bars * 4).map(|i| 1.0 + i as f64 * beat).collect();
        let downbeats: Vec<f64> = beats.iter().step_by(4).copied().collect();
        let drop = downbeats[8];
        let end = 1.0 + (bars * 4) as f64 * beat;
        let map = SongMap {
            meter: 4,
            downbeat_phase: 0,
            downbeats,
            sections: vec![
                Section {
                    start: 0.0,
                    end: drop,
                    energy: 0.2,
                    label: 0,
                },
                Section {
                    start: drop,
                    end,
                    energy: 0.9,
                    label: 1,
                },
            ],
            drops: vec![drop],
            energy: vec![],
        };
        (BeatGrid { bpm, beats }, map)
    }

    fn choreography(bpm: f64, bars: usize) -> (Choreography, SongMap) {
        let (grid, map) = song(bpm, bars);
        let frames = ((bars * 4) as f64 * 60.0 / bpm * 60.0) as usize + 120;
        let bands: Vec<u8> = (0..frames * 32).map(|i| (i % 255) as u8).collect();
        let onset: Vec<f32> = (0..frames).map(|i| (i % 30) as f32 / 30.0).collect();
        (
            Choreography::build(&grid, &map, &bands, 32, &onset, 60.0),
            map,
        )
    }

    /// 60 kare/sn'de nabzın yükseliş sayısı (saniyede).
    fn pulse_rate(c: &Choreography, safe: bool, seconds: f64) -> f64 {
        let mut previous = 0.0;
        let mut rises = 0;
        for i in 0..(seconds * 60.0) as usize {
            let p = c.frame_at(i as f64 / 60.0, safe).rhythm.pulse;
            if p > previous + 0.2 {
                rises += 1;
            }
            previous = p;
        }
        rises as f64 / seconds
    }

    #[test]
    fn nabiz_saniyede_ucten_fazla_olmaz() {
        // 240 BPM: vuruşlar saniyede 4; nabız iki vuruşta bire iner.
        let (fast, _) = choreography(240.0, 32);
        assert!(pulse_rate(&fast, false, 7.0) <= 3.0);
        assert!(pulse_rate(&fast, false, 7.0) >= 1.5, "nabız yine de var");
        assert!(fast
            .pulses
            .windows(2)
            .all(|w| w[1] - w[0] >= MIN_PULSE_INTERVAL - 1e-9));
        // 120 BPM: her vuruşta nabız (saniyede 2).
        let (normal, _) = choreography(120.0, 32);
        assert!((pulse_rate(&normal, false, 8.0) - 2.0).abs() < 0.3);
        // Güvenli mod: saniyede en fazla 1 (yalnızca ölçü başları), ölçü vurgusu yok.
        assert!(pulse_rate(&fast, true, 7.0) <= 1.0);
        assert!(fast
            .safe_pulses
            .windows(2)
            .all(|w| w[1] - w[0] >= SAFE_PULSE_INTERVAL - 1e-9));
        assert_eq!(fast.frame_at(5.0, true).rhythm.accent, 0.0);
    }

    #[test]
    fn drop_once_beklenti_sonra_bosalma() {
        let (c, map) = choreography(128.0, 24);
        let drop = map.drops[0];
        let bar = 4.0 * 60.0 / 128.0;
        let at = |t: f64| c.frame_at(t, false).rhythm;
        // 8 ölçüden önce beklenti yok; drop'a yaklaştıkça artar.
        assert_eq!(at(drop - 8.5 * bar).anticipation, 0.0);
        let rising: Vec<f32> = (0..=16)
            .map(|i| at(drop - 8.0 * bar + i as f64 * 0.5 * bar).anticipation)
            .collect();
        assert!(rising.windows(2).all(|w| w[1] >= w[0]), "{rising:?}");
        assert!(at(drop - 0.01).anticipation > 0.95);
        // Drop anında boşalma başlar, sonra söner; beklenti biter.
        assert!(at(drop).release > 0.99);
        assert!(at(drop + 2.0 * bar).release < 0.25);
        assert_eq!(at(drop + 0.1).anticipation, 0.0);
        assert_eq!(at(drop - 1.0).release, 0.0);
    }

    #[test]
    fn atmosfer_bolume_gore_ve_yumusak() {
        let (c, map) = choreography(120.0, 24);
        let drop = map.drops[0];
        let calm = c.frame_at(drop - 4.0, false).atmosphere;
        let loud = c.frame_at(drop + 4.0, false).atmosphere;
        assert_eq!((calm.section, calm.theme), (0, 0));
        assert_eq!((loud.section, loud.theme), (1, 1));
        assert!(loud.mood > calm.mood + 0.5);
        assert!(loud.warmth > calm.warmth);
        // Bölüm geçişi ani değil: 10 ms adımlarla ruh hâli en fazla küçük adımlarla değişir.
        let moods: Vec<f32> = (0..300)
            .map(|i| {
                c.frame_at(drop - 1.5 + i as f64 * 0.01, false)
                    .atmosphere
                    .mood
            })
            .collect();
        let biggest = moods
            .windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max);
        assert!(biggest < 0.1, "{biggest}");
    }

    #[test]
    fn faz_doku_ve_gecersiz_zaman() {
        let (c, _) = choreography(120.0, 16);
        let f = c.frame_at(1.25, false);
        assert!((f.rhythm.beat_phase - 0.5).abs() < 1e-6);
        assert!((f.rhythm.bar_phase - 0.125).abs() < 1e-6);
        assert!((0.0..=1.0).contains(&f.texture.detail) && (0.0..=1.0).contains(&f.texture.motion));
        let nan = c.frame_at(f64::NAN, false);
        assert_eq!(nan.rhythm.release, 0.0);
        let json = serde_json::to_value(f).unwrap();
        assert!(json["rhythm"]["beatPhase"].is_number() && json["atmosphere"]["theme"].is_number());
    }
}
