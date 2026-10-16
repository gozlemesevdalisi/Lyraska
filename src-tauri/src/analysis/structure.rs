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
use super::meter::Aksak;

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
/// Drop güveninde bas artışı bu kadar olunca (bant ölçeği: 0,3 ≈ 18 dB) tam puan.
const DROP_SURE_BASS_JUMP: f64 = 0.30;
/// Drop güveninde enerji farkı (önceki 4 ölçüye göre) bu kadar olunca tam puan.
const DROP_SURE_ENERGY_RISE: f64 = 0.40;
/// Drop'tan sonra yüksek enerjinin sürdüğüne bakılan ölçü sayısı.
const DROP_SUSTAIN_BARS: usize = 8;

/// Şarkının yapısı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct SongMap {
    /// Ölçüdeki vuruş sayısı: düzenli ölçüde 3 ya da 4; aksak ölçüde grup sayısı (ör. 9/8 =
    /// 2+2+2+3'te 4: vuruşlar grup başlarıdır).
    pub meter: usize,
    /// İlk tam ölçünün başladığı vuruşun sırası (0..meter).
    pub downbeat_phase: usize,
    /// Aksak ölçünün sekizlik gruplaması (ör. [2, 2, 2, 3] = 9/8); düzenli ölçüde boş.
    #[serde(default)]
    pub grouping: Vec<u8>,
    /// Ölçünün (ölçü başlarının) güveni (0..1). [`METER_CONFIDENCE_MIN`]'in altında görseller
    /// ölçü başını vurgulamaz (lamba, dalga), yalnızca vuruşa tepki verir: yanlış ölçü vurgusu,
    /// hiç vurgu olmamasından kötüdür.
    #[serde(default)]
    pub meter_confidence: f32,
    /// Ölçü başlarının zamanları (saniye).
    pub downbeats: Vec<f64>,
    pub sections: Vec<Section>,
    /// Drop anları (saniye).
    pub drops: Vec<f64>,
    /// Her drop'un güveni (0..1; `drops` ile aynı sırada). Bas ve enerji ne kadar belirgin
    /// döndü, bölüm sınırına denk geliyor mu, yüksek enerji sürüyor mu. Güveni
    /// [`DROP_CONFIDENCE_MIN`]'in altındaki drop hiçbir yerde drop sayılmaz
    /// ([`SongMap::confident_drops`]).
    #[serde(default)]
    pub drop_confidence: Vec<f32>,
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

/// Drop sayılmak için gereken en düşük güven. Altındaki aday hiçbir yerde drop sayılmaz:
/// sayaç, şarkı haritası şeridindeki işaret, Görsel Yönetmen'in beklentisi ve açılımı,
/// doğruluk ölçümü. Arayüzdeki `DROP_CONFIDENCE_MIN` (`src/lib/songMap.ts`) ile aynıdır
/// (testli).
pub const DROP_CONFIDENCE_MIN: f32 = 0.6;

/// Ölçü vurgusu için gereken en düşük ölçü güveni (bkz. [`SongMap::meter_confidence`]).
pub const METER_CONFIDENCE_MIN: f32 = 0.5;
/// Düzenli ölçüde güven, şarkı boyunca tutarlılıktan: 8 ölçülük pencerelerin (yarım adımla)
/// kaçında aynı ölçü başı konumu kazanıyor. Bu payın altı güvensiz, üstü tam güvenli. Ölçü başı
/// diğer vuruşlardan hiç ayrılmıyorsa (z-puanı farkı `METER_CONTRAST_MIN`'in altı) güven yok.
const METER_WINDOW_BARS: usize = 8;
const METER_AGREEMENT_LOW: f64 = 0.5;
const METER_AGREEMENT_FULL: f64 = 0.85;
const METER_CONTRAST_MIN: f64 = 0.2;

impl SongMap {
    /// Ölçü başları vurgulanacak kadar güvenli mi?
    pub fn bars_confident(&self) -> bool {
        self.meter_confidence >= METER_CONFIDENCE_MIN
    }

    /// Güveni yeterli droplar (artan sırada). Güveni kayıtlı olmayan drop sayılmaz.
    pub fn confident_drops(&self) -> impl Iterator<Item = f64> + '_ {
        self.drops
            .iter()
            .zip(&self.drop_confidence)
            .filter(|(_, &confidence)| confidence >= DROP_CONFIDENCE_MIN)
            .map(|(&drop, _)| drop)
    }

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
/// ses yüksekliği (dBFS); `fps`: saniyedeki kare. `aksak`: aksak ölçü bulunduysa
/// ([`super::meter::detect`]); o zaman `beats` grup başlarıdır ve ölçü ondan gelir.
pub fn map_song(
    bands: &[u8],
    bands_per_frame: usize,
    loudness_db: &[f32],
    beats: &BeatGrid,
    fps: f64,
    aksak: Option<&Aksak>,
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
    let (meter, downbeat_phase, meter_confidence, grouping) = match aksak {
        Some(aksak) => (
            aksak.grouping.len(),
            aksak.downbeat_phase % aksak.grouping.len().max(1),
            aksak.confidence,
            aksak.grouping.clone(),
        ),
        None => {
            let (meter, phase, contrast) = choose_meter(&score);
            let confidence = if contrast < METER_CONTRAST_MIN {
                0.0
            } else {
                ((phase_agreement(&score, meter, phase) - METER_AGREEMENT_LOW)
                    / (METER_AGREEMENT_FULL - METER_AGREEMENT_LOW))
                    .clamp(0.0, 1.0)
            };
            (meter, phase, confidence as f32, Vec::new())
        }
    };
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

    let (drops, drop_confidence) = find_drops(&bar_bass, &bar_energy)
        .into_iter()
        .map(|k| {
            (
                bar_time(k),
                drop_confidence(k, &bar_bass, &bar_energy, &cuts),
            )
        })
        .unzip();

    Some(SongMap {
        meter,
        downbeat_phase,
        grouping,
        meter_confidence,
        downbeats,
        sections,
        drops,
        drop_confidence,
        energy,
    })
}

/// Ölçü (3 ya da 4), ilk ölçü başının sırası ve ölçü başlarının diğer vuruşlardan ayrılışı
/// (ortalama z-puanı farkı).
fn choose_meter(score: &[f64]) -> (usize, usize, f64) {
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
        (3, phase3, contrast3)
    } else {
        (4, phase4, contrast4)
    }
}

/// Ölçü başı konumunun şarkı boyunca tutarlılığı: 8 ölçülük pencerelerin (yarım pencere adımla)
/// kaçında en yüksek puanlı konum `phase`. Pencere sayısı ikiden azsa bütün şarkı tek pencere.
fn phase_agreement(score: &[f64], meter: usize, phase: usize) -> f64 {
    let meter = meter.max(1);
    let window = METER_WINDOW_BARS * meter;
    let step = (window / 2).max(1);
    let mut windows = 0usize;
    let mut agree = 0usize;
    let mut start = 0;
    while start + window <= score.len() || (windows == 0 && start == 0) {
        let end = (start + window).min(score.len());
        let best = (0..meter)
            .map(|p| {
                let values: Vec<f64> = (start..end)
                    .filter(|&i| i % meter == p)
                    .map(|i| score[i])
                    .collect();
                (p, mean(&values))
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .map_or(0, |(p, _)| p);
        windows += 1;
        if best == phase % meter {
            agree += 1;
        }
        start += step;
    }
    agree as f64 / windows.max(1) as f64
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

/// `k`. ölçüdeki drop'un güveni (0..1). Dört işaret, ağırlıklarıyla:
/// - bas ne kadar güçlü döndü (%35; eşikte 0, 18 dB'de tam),
/// - enerji önceki dört ölçüye göre ne kadar arttı (%30),
/// - bölüm sınırına (±1 ölçü) denk geliyor mu (%20): gerçek droplar yeni bölüm başlatır,
/// - yüksek enerji sonraki sekiz ölçü boyunca sürüyor mu (%15): tek bir vurgu drop değildir.
fn drop_confidence(k: usize, bar_bass: &[f64], bar_energy: &[f64], cuts: &[usize]) -> f32 {
    let n = bar_bass.len();
    let before = k.saturating_sub(DROP_BEFORE_BARS)..k;
    let after = k..(k + DROP_AFTER_BARS).min(n);
    let jump = mean(&bar_bass[after.clone()]) - mean(&bar_bass[before.clone()]);
    let rise = mean(&bar_energy[after]) - mean(&bar_energy[before]);
    let unit = |x: f64| x.clamp(0.0, 1.0);
    let bass = unit((jump - DROP_MIN_BASS_JUMP) / (DROP_SURE_BASS_JUMP - DROP_MIN_BASS_JUMP));
    let energy = unit(rise / DROP_SURE_ENERGY_RISE);
    let boundary = if cuts.iter().any(|&c| c > 0 && c.abs_diff(k) <= 1) {
        1.0
    } else {
        0.0
    };
    let sustained = mean(&bar_energy[k..(k + DROP_SUSTAIN_BARS).min(n)]);
    let sustain = unit((sustained - 0.45) / 0.3);
    (0.35 * bass + 0.30 * energy + 0.20 * boundary + 0.15 * sustain) as f32
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

    #[test]
    fn aksak_dokuz_sekizlikte_vurus_ve_olcu_baslari_dogru() {
        let rate = 22_050;
        let (samples, truth, downbeats) =
            crate::analysis::meter::tests::aksak_drums(270.0, &[2, 2, 2, 3], 40.0, rate);
        let spectrogram = analyze_samples(&samples, rate);
        let map = spectrogram.song_map().expect("yapı bulunmalı");
        assert_eq!(map.grouping, vec![2, 2, 2, 3]);
        assert_eq!(map.meter, 4, "ölçüde 4 (uzun ve kısa) vuruş");
        assert!(map.bars_confident(), "{}", map.meter_confidence);
        // Vuruşlar grup başları (takipçinin eşit aralıklı vuruşları değil).
        let grid = spectrogram.beat_grid().unwrap();
        let f = crate::analysis::evaluate::f_measure(&grid.beats, &truth, 0.05);
        assert!(f > 0.95, "F {f}");
        // Ölçü başları gerçek ölçü başlarına oturur.
        let hits = map
            .downbeats
            .iter()
            .filter(|&&d| downbeats.iter().any(|t| (t - d).abs() < 0.05))
            .count();
        assert!(
            hits as f64 >= 0.9 * map.downbeats.len() as f64,
            "{hits}/{}",
            map.downbeats.len()
        );
        // Her ölçü başı, vuruşların `meter` aralıklı dizisinde (ölçüdeki yer = 1).
        let first = grid
            .beats
            .iter()
            .position(|&b| (b - map.downbeats[1]).abs() < 1e-9)
            .unwrap();
        assert_eq!(map.bar_beat(first), 1);
        assert_eq!(map.bar_beat(first + 3), 4);
        assert_eq!(map.bar_beat(first + 4), 1);
    }

    #[test]
    fn olcu_basi_tutarsiz_kayitta_olcu_vurgusu_kapanir() {
        // Aynı davul; birinde ölçü başı her ölçüde aynı yerde, ötekinde her ölçüde başka vuruşta.
        let rate = 22_050;
        let (samples, _, _) =
            crate::analysis::meter::tests::aksak_drums(240.0, &[2, 2, 2, 2], 60.0, rate);
        let accented = analyze_samples(&samples, rate).song_map().unwrap();
        assert!(accented.bars_confident(), "{}", accented.meter_confidence);
        let shifting = crate::analysis::meter::tests::shifting_drums(240.0, 60.0, rate);
        let unsure = analyze_samples(&shifting, rate).song_map().unwrap();
        assert!(!unsure.bars_confident(), "{}", unsure.meter_confidence);
        assert!(unsure.grouping.is_empty());
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
        // Sakin kısımdan sonra bas ve enerji birden dönüyor, yeni bölüm başlıyor, enerji
        // sürüyor: drop güveni yüksek (drop sayacı gösterilir).
        assert_eq!(map.drop_confidence.len(), map.drops.len());
        assert!(
            map.drop_confidence[0] >= 0.8,
            "güven {:?}",
            map.drop_confidence
        );
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
            grouping: Vec::new(),
            meter_confidence: 1.0,
            downbeats: vec![],
            sections: vec![],
            drops: vec![],
            drop_confidence: vec![],
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
    fn belirgin_drop_yuksek_sinirda_aday_dusuk_guven_alir() {
        // Ölçü başına bas ve enerji: 8 ölçü sakin (bas yok), sonra 12 ölçü tam drop.
        let mut bass = vec![0.15; 8];
        bass.extend([0.55; 12]);
        let mut energy = vec![0.35; 8];
        energy.extend([0.95; 12]);
        let drops = find_drops(&bass, &energy);
        assert_eq!(drops, [8]);
        let strong = drop_confidence(8, &bass, &energy, &[0, 8]);
        assert!(strong > 0.95, "belirgin drop: {strong}");

        // Zaten yüksek bir kısımda bas biraz artıyor (eşiğin hemen üstünde), bölüm sınırı yok,
        // iki ölçü sonra enerji düşüyor: drop sayılır ama güveni düşük (sayaç gösterilmez).
        let mut bass = vec![0.40; 8];
        bass.extend([0.53, 0.53, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40]);
        let mut energy = vec![0.60; 8];
        energy.extend([0.72, 0.72, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40, 0.40]);
        assert_eq!(find_drops(&bass, &energy), [8]);
        let weak = drop_confidence(8, &bass, &energy, &[0]);
        assert!(weak < 0.4, "sınırdaki aday: {weak}");
        // Güven her zaman 0..1.
        assert!((0.0..=1.0).contains(&strong) && (0.0..=1.0).contains(&weak));
    }

    #[test]
    fn yalnizca_guveni_yeterli_droplar_drop_sayilir() {
        let map = SongMap {
            meter: 4,
            downbeat_phase: 0,
            grouping: Vec::new(),
            meter_confidence: 1.0,
            downbeats: vec![],
            sections: vec![],
            drops: vec![30.0, 60.0, 90.0, 120.0],
            // Güveni eksik olan son drop da sayılmaz (eski kayıt).
            drop_confidence: vec![0.2, 0.6, 0.95],
            energy: vec![],
        };
        assert_eq!(map.confident_drops().collect::<Vec<_>>(), [60.0, 90.0]);
    }

    #[test]
    fn drop_guven_esigi_arayuzdekiyle_ayni() {
        // Arayüz (sayaç, şerit) ile Görsel Yönetmen aynı droplara bakmalı.
        let source = include_str!("../../../src/lib/songMap.ts");
        let line = source
            .lines()
            .find(|l| l.starts_with("export const DROP_CONFIDENCE_MIN ="))
            .expect("songMap.ts'te DROP_CONFIDENCE_MIN yok");
        let value: f32 = line
            .split('=')
            .nth(1)
            .unwrap()
            .trim()
            .trim_end_matches(';')
            .parse()
            .unwrap();
        assert_eq!(value, DROP_CONFIDENCE_MIN);
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
