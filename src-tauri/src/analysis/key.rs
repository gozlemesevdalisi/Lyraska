//! Ton tespiti: şarkının tonu (ör. La minör) ve Camelot kodu (ör. 8A), güveniyle.
//!
//! Yöntem (klasik, öğrenmesiz):
//!
//! 1. **Kroma:** şarkı çözülürken uzun bir pencereyle (~0,37 sn, 44,1 kHz'te 16 384 örnek)
//!    spektrum alınır; 65 Hz – 2,1 kHz arasındaki tepeler bulunur, frekansları parabolle
//!    inceltilir ve yarım tonun onda biri çözünürlükte 120 kutuya toplanır. Saniyede bir
//!    kutular saklanır.
//! 2. **Akort:** bütün tepelerin yarım tondan sapmalarının dairesel ortalaması şarkının akort
//!    kaymasıdır (ör. A = 432 Hz'te −32 sent). Kutular bu kaymaya göre 12 perde sınıfına
//!    katlanır: saniye başına 12 değer (kroma zaman çizgisi).
//! 3. **Ton:** kromanın 24 tonun profiliyle ilintisi (Krumhansl ve Kessler'in 1982 dinleme
//!    deneylerinden çıkan ton profilleri); en yüksek ilinti tondur. **Güven** en iyi ile ikinci
//!    tonun farkından ve ilintinin kendisinden gelir: belirsizse (atonal, yalnızca davul) ton
//!    gösterilmez. Şarkının tonu 15 saniyelik pencerelerin oylamasıyla seçilir: bütün şarkının
//!    kromasını toplamak, ton değiştiren şarkıda iki tonun "ortasındaki" komşu tonu verir.
//! 4. **Bölüm bölüm:** şarkı haritasının bölümleri için aynı hesap o bölümün saniyeleriyle
//!    yapılır; böylece ton değiştiren şarkıda her bölümün tonu ayrı görünür.
//!
//! Kroma, saklanan 32 bant kareden çıkmaz (perde çözünürlüğü yetmez): ayrı bir analiz
//! parçasıdır ve kendi sürümü vardır ([`super::cache::VERSIONS`]).

use std::sync::Arc;

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use serde::{Deserialize, Serialize};

use super::structure::Section;
use crate::audio::Sample;

/// Perde sınıfı sayısı.
pub const PITCH_CLASSES: usize = 12;
/// Yarım ton başına ince kutu ve toplam ince kutu.
const FINE_PER_SEMITONE: usize = 10;
const FINE_BINS: usize = PITCH_CLASSES * FINE_PER_SEMITONE;
/// Kromada kullanılan frekans aralığı (C2 – C7).
const MIN_HZ: f64 = 65.0;
const MAX_HZ: f64 = 2_100.0;
/// İstenen frekans çözünürlüğü (Hz): C2'de yarım ton ~3,9 Hz.
const TARGET_BIN_HZ: f64 = 2.7;
/// Tepe sayılmak için karenin en güçlü tepesine göre en düşük düzey (dB).
const PEAK_FLOOR_DB: f64 = -40.0;
/// Bu seviyenin altındaki pencere sessiz sayılır (dBFS, RMS).
const SILENCE_DB: f64 = -55.0;
/// Akort kaymasının ölçüldüğü en düşük frekans: alçakta tepe frekansları daha belirsiz.
const TUNING_MIN_HZ: f64 = 200.0;

/// Ton gösterilmek için gereken en düşük güven.
pub const KEY_CONFIDENCE_MIN: f32 = 0.35;
/// Güven hesabı: bu ilintinin altı güvensiz, üstü tam güvenli; en iyi ile ikinci ton arasında
/// bu kadar fark tam güven sayılır.
const CORRELATION_MIN: f64 = 0.45;
const CORRELATION_FULL: f64 = 0.8;
const MARGIN_FULL: f64 = 0.08;
/// Bir bölümün tonu için en az bu kadar sesli saniye gerekir.
const SECTION_MIN_SECONDS: usize = 8;
/// Şarkının tonu pencerelerin oylamasıyla seçilir (bkz. [`song_key`]): pencere süresi (saniye)
/// ve kazanan tonun en az payı.
const VOTE_WINDOW_SECONDS: usize = 15;
const VOTE_MAJORITY: f64 = 0.5;

/// Krumhansl ve Kessler (1982) ton profilleri: tonikten başlayarak 12 perde sınıfının o tonda
/// ne kadar "yerinde" duyulduğu (dinleyici değerlendirmeleri).
const MAJOR_PROFILE: [f64; PITCH_CLASSES] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const MINOR_PROFILE: [f64; PITCH_CLASSES] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

/// Bir ton tahmini.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MusicalKey {
    /// Tonik perde sınıfı: 0 = Do (C), 1 = Do diyez, … 11 = Si (B).
    pub tonic: u8,
    /// Minör mü (değilse majör)?
    pub minor: bool,
    /// Güven (0..1). [`KEY_CONFIDENCE_MIN`]'in altındaki ton hiçbir yerde gösterilmez.
    pub confidence: f32,
    /// Camelot çarkındaki sayı (1..12); harf minörde A, majörde B.
    pub camelot: u8,
}

impl MusicalKey {
    fn new(tonic: usize, minor: bool, confidence: f64) -> Self {
        Self {
            tonic: (tonic % PITCH_CLASSES) as u8,
            minor,
            confidence: confidence.clamp(0.0, 1.0) as f32,
            camelot: camelot_number(tonic, minor),
        }
    }

    /// Camelot kodu (ör. "8A").
    pub fn camelot_code(&self) -> String {
        format!("{}{}", self.camelot, if self.minor { 'A' } else { 'B' })
    }

    /// Ton gösterilecek kadar güvenli mi?
    pub fn is_confident(&self) -> bool {
        self.confidence >= KEY_CONFIDENCE_MIN
    }
}

/// Kullanıcının işaretleme aracında seçtiği ton.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct MarkedKey {
    /// 0 = Do (C) … 11 = Si (B).
    pub tonic: u8,
    pub minor: bool,
}

/// Bulunan ton ile işaretlenen ton arasındaki ilişki (MIREX ton değerlendirmesindeki sınıflar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum KeyRelation {
    /// Aynı ton.
    Same,
    /// Beşli komşusu: Camelot çarkında bir yan (ör. 8A ↔ 9A).
    Fifth,
    /// Paralel majör/minör: aynı Camelot sayısı, öteki harf (ör. 8A ↔ 8B, La minör ↔ Do majör).
    Relative,
    /// Aynı tonik, öteki kip (ör. La minör ↔ La majör).
    Parallel,
    /// Başka bir ton.
    Other,
}

impl KeyRelation {
    /// İsabet sayılır mı? Yol haritasındaki hedef: aynı ton ya da Camelot komşusu (DJ için uyumlu
    /// geçiş): aynı, beşli komşusu ya da paralel majör/minör (aynı Camelot sayısı).
    pub fn is_hit(self) -> bool {
        matches!(self, Self::Same | Self::Fifth | Self::Relative)
    }
}

/// Bulunan tonun işaretlenen tona göre ilişkisi.
pub fn key_relation(detected: &MusicalKey, marked: MarkedKey) -> KeyRelation {
    let marked = MusicalKey::new(usize::from(marked.tonic), marked.minor, 1.0);
    let step = (i16::from(detected.camelot) - i16::from(marked.camelot)).rem_euclid(12);
    if (detected.tonic, detected.minor) == (marked.tonic, marked.minor) {
        KeyRelation::Same
    } else if detected.minor == marked.minor && (step == 1 || step == 11) {
        KeyRelation::Fifth
    } else if detected.camelot == marked.camelot {
        KeyRelation::Relative
    } else if detected.tonic == marked.tonic {
        KeyRelation::Parallel
    } else {
        KeyRelation::Other
    }
}

/// Camelot sayısı: Do majör 8B, La minör 8A; beşli çemberde her adım bir sayı.
fn camelot_number(tonic: usize, minor: bool) -> u8 {
    // Minör tonun Camelot sayısı paralel majörünün (üç yarım ton yukarı) sayısıdır.
    let major_tonic = if minor { tonic + 3 } else { tonic } % PITCH_CLASSES;
    (((7 * major_tonic) % PITCH_CLASSES + 7) % PITCH_CLASSES + 1) as u8
}

/// Bir şarkının ton analizi: saniye başına kroma ve bütün şarkının tonu.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyAnalysis {
    /// Saniye başına 12 perde sınıfının bağıl gücü (0..255; o saniyenin en güçlüsü 255).
    /// Sessiz saniyeler sıfırdır.
    pub chroma: Vec<[u8; PITCH_CLASSES]>,
    /// Şarkının tonu; kroma boşsa (sessiz şarkı) ya da belirsizse yine de en iyi tahmin,
    /// güveni düşük olarak.
    pub key: Option<MusicalKey>,
    /// Akort kayması (yarım ton; −0,5..0,5). Yalnızca bilgi için.
    pub tuning: f32,
}

impl KeyAnalysis {
    /// Verilen saniye aralığının (bölüm) tonu. Yeterince sesli saniye yoksa `None`.
    pub fn key_between(&self, start: f64, end: f64) -> Option<MusicalKey> {
        let from = start.max(0.0).floor() as usize;
        let to = (end.max(0.0).ceil() as usize).min(self.chroma.len());
        let seconds = self.chroma.get(from..to)?;
        let sounding = seconds.iter().filter(|c| c.iter().any(|&v| v > 0)).count();
        if sounding < SECTION_MIN_SECONDS {
            return None;
        }
        estimate(&sum_chroma(seconds))
    }
}

/// Çalan şarkının arayüzde gösterilen tonu: şarkının tonu ve tonu farklı bölümler.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct KeyMap {
    /// Şarkının tonu; güveni yetmiyorsa `None` (ton gösterilmez).
    pub key: Option<MusicalKey>,
    /// Şarkı haritasının bölümleriyle aynı sırada: bölümün tonu şarkınınkinden belirgin biçimde
    /// farklıysa (ton değişimi) o ton, değilse `None` (bölüm şarkının tonunda). Paralel majör ve
    /// minör (aynı Camelot sayısı) aynı notaları kullandığı için değişim sayılmaz.
    pub sections: Vec<Option<MusicalKey>>,
}

impl KeyAnalysis {
    /// Şarkının ve bölümlerinin tonu.
    pub fn key_map(&self, sections: &[Section]) -> KeyMap {
        let key = self.key.filter(MusicalKey::is_confident);
        let sections = sections
            .iter()
            .map(|s| {
                self.key_between(s.start, s.end)
                    .filter(MusicalKey::is_confident)
                    .filter(|k| key.is_none_or(|g| g.camelot != k.camelot))
            })
            .collect();
        KeyMap { key, sections }
    }
}

/// Kromaların toplamı.
fn sum_chroma(seconds: &[[u8; PITCH_CLASSES]]) -> [f64; PITCH_CLASSES] {
    let mut total = [0.0; PITCH_CLASSES];
    for second in seconds {
        for (t, &v) in total.iter_mut().zip(second) {
            *t += f64::from(v);
        }
    }
    total
}

/// Şarkının tonu: 15 saniyelik pencerelerin (en az iki) güvenli tonları sesli süreleriyle oy
/// verir; bir ton oyların en az yarısını alırsa şarkının tonu odur (güveni pencerelerinin
/// ortalama güveni × oy payı). Çoğunluk yoksa ya da şarkı kısaysa bütün şarkının kroması.
fn song_key(chroma: &[[u8; PITCH_CLASSES]]) -> Option<MusicalKey> {
    let whole = estimate(&sum_chroma(chroma));
    let mut votes: Vec<(MusicalKey, f64, f64)> = Vec::new();
    let mut total = 0.0;
    let mut windows = 0;
    for window in chroma.chunks(VOTE_WINDOW_SECONDS) {
        let sounding = window.iter().filter(|c| c.iter().any(|&v| v > 0)).count();
        if sounding < SECTION_MIN_SECONDS {
            continue;
        }
        let Some(key) = estimate(&sum_chroma(window)).filter(MusicalKey::is_confident) else {
            continue;
        };
        windows += 1;
        let weight = sounding as f64;
        total += weight;
        match votes
            .iter_mut()
            .find(|(k, _, _)| (k.tonic, k.minor) == (key.tonic, key.minor))
        {
            Some((_, w, confidence)) => {
                *w += weight;
                *confidence += weight * f64::from(key.confidence);
            }
            None => votes.push((key, weight, weight * f64::from(key.confidence))),
        }
    }
    let winner = votes.iter().max_by(|a, b| a.1.total_cmp(&b.1));
    match winner {
        Some(&(key, weight, confidence)) if windows >= 2 && weight >= VOTE_MAJORITY * total => {
            Some(MusicalKey::new(
                usize::from(key.tonic),
                key.minor,
                (confidence / weight) * (weight / total),
            ))
        }
        _ => whole,
    }
}

/// Kromadan ton: 24 tonun profiliyle ilinti. Kroma düzse (her perde eşit: davul, sessizlik)
/// `None`.
pub fn estimate(chroma: &[f64; PITCH_CLASSES]) -> Option<MusicalKey> {
    let mut scores: Vec<(f64, usize, bool)> = Vec::with_capacity(24);
    for tonic in 0..PITCH_CLASSES {
        for (minor, profile) in [(false, &MAJOR_PROFILE), (true, &MINOR_PROFILE)] {
            let rotated: [f64; PITCH_CLASSES] =
                std::array::from_fn(|i| profile[(i + PITCH_CLASSES - tonic) % PITCH_CLASSES]);
            scores.push((correlation(chroma, &rotated)?, tonic, minor));
        }
    }
    scores.sort_by(|a, b| b.0.total_cmp(&a.0));
    let (best, tonic, minor) = scores[0];
    let second = scores[1].0;
    let strength =
        ((best - CORRELATION_MIN) / (CORRELATION_FULL - CORRELATION_MIN)).clamp(0.0, 1.0);
    let margin = ((best - second) / MARGIN_FULL).clamp(0.0, 1.0);
    Some(MusicalKey::new(tonic, minor, strength * margin.sqrt()))
}

/// Pearson ilinti katsayısı; değerlerden biri sabitse `None`.
fn correlation(a: &[f64; PITCH_CLASSES], b: &[f64; PITCH_CLASSES]) -> Option<f64> {
    let n = PITCH_CLASSES as f64;
    let (ma, mb) = (a.iter().sum::<f64>() / n, b.iter().sum::<f64>() / n);
    let (mut sab, mut saa, mut sbb) = (0.0, 0.0, 0.0);
    for (x, y) in a.iter().zip(b) {
        sab += (x - ma) * (y - mb);
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
    }
    // Düz kroma: perdeler birbirinden ayrılmıyor (sessizlik ya da yalnızca vurmalı).
    if saa <= 1e-9 * (ma * ma * n).max(1e-12) || sbb <= 0.0 {
        return None;
    }
    Some(sab / (saa * sbb).sqrt())
}

/// Şarkı çözülürken kromayı toplayan çözümleyici. Örnekler tek kanala indirilmiş olarak
/// [`ChromaAnalyzer::add`] ile verilir; şarkı bitince [`ChromaAnalyzer::finish`].
pub struct ChromaAnalyzer {
    fft: Arc<dyn Fft<f64>>,
    size: usize,
    hop: usize,
    hann: Vec<f64>,
    norm: f64,
    window: Vec<f64>,
    head: usize,
    filled: usize,
    since_hop: usize,
    buffer: Vec<Complex<f64>>,
    magnitudes: Vec<f64>,
    bin_hz: f64,
    /// Kromada kullanılan FFT kutuları [başlangıç, bitiş).
    bins: (usize, usize),
    samples_per_second: u64,
    position: u64,
    /// Saniye başına ince kutular ve şu anki saniye.
    seconds: Vec<[f32; FINE_BINS]>,
    current: [f64; FINE_BINS],
    /// Akort kaymasının dairesel ortalaması için (ağırlıklı sinüs ve kosinüs toplamı).
    tuning_sin: f64,
    tuning_cos: f64,
}

impl ChromaAnalyzer {
    pub fn new(sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let size = (2f64.powf((rate / TARGET_BIN_HZ).log2().round()) as usize).clamp(4096, 32768);
        let fft = FftPlanner::new().plan_fft_forward(size);
        let hann: Vec<f64> = (0..size)
            .map(|i| 0.5 - 0.5 * (2.0 * std::f64::consts::PI * i as f64 / size as f64).cos())
            .collect();
        let norm = 2.0 / hann.iter().sum::<f64>();
        let bin_hz = rate / size as f64;
        let bins = (
            ((MIN_HZ / bin_hz).floor() as usize).max(2),
            ((MAX_HZ / bin_hz).ceil() as usize).min(size / 2 - 2),
        );
        Self {
            fft,
            size,
            // Örtüşmesiz pencere: ton uzun süreli bir ortalamadır; analiz süresi az uzar.
            hop: size,
            hann,
            norm,
            window: vec![0.0; size],
            head: 0,
            filled: 0,
            since_hop: 0,
            buffer: vec![Complex::default(); size],
            magnitudes: vec![0.0; size / 2 + 1],
            bin_hz,
            bins,
            samples_per_second: u64::from(sample_rate.max(1)),
            position: 0,
            seconds: Vec::new(),
            current: [0.0; FINE_BINS],
            tuning_sin: 0.0,
            tuning_cos: 0.0,
        }
    }

    /// Bir tek kanallı örnek ekler.
    pub fn add(&mut self, mono: Sample) {
        self.window[self.head] = mono;
        self.head = (self.head + 1) % self.size;
        self.filled = (self.filled + 1).min(self.size);
        self.since_hop += 1;
        if self.filled == self.size && self.since_hop >= self.hop {
            self.since_hop = 0;
            self.process();
        }
        self.position += 1;
        if self.position % self.samples_per_second == 0 {
            self.close_second();
        }
    }

    fn close_second(&mut self) {
        self.seconds.push(self.current.map(|v| v as f32));
        self.current = [0.0; FINE_BINS];
    }

    fn process(&mut self) {
        let ordered = self.window[self.head..]
            .iter()
            .chain(&self.window[..self.head]);
        let mut energy = 0.0;
        for ((dst, &x), &w) in self.buffer.iter_mut().zip(ordered).zip(&self.hann) {
            energy += x * x;
            *dst = Complex::new(x * w, 0.0);
        }
        let rms_db = 10.0 * (energy / self.size as f64).max(1e-20).log10();
        if rms_db < SILENCE_DB {
            return;
        }
        self.fft.process(&mut self.buffer);
        let (lo, hi) = self.bins;
        let mut loudest = 0.0f64;
        for k in lo - 1..=hi + 1 {
            let m = self.buffer[k].norm() * self.norm;
            self.magnitudes[k] = m;
            loudest = loudest.max(m);
        }
        let floor = loudest * 10f64.powf(PEAK_FLOOR_DB / 20.0);
        for k in lo..hi {
            let (a, b, c) = (
                self.magnitudes[k - 1],
                self.magnitudes[k],
                self.magnitudes[k + 1],
            );
            if b <= floor || b <= a || b < c {
                continue;
            }
            // Parabolle tepenin gerçek yeri ve yüksekliği (logaritmik genlikte).
            let (la, lb, lc) = (a.max(1e-12).ln(), b.ln(), c.max(1e-12).ln());
            let denominator = la - 2.0 * lb + lc;
            let p = if denominator.abs() > 1e-12 {
                (0.5 * (la - lc) / denominator).clamp(-0.5, 0.5)
            } else {
                0.0
            };
            let hz = (k as f64 + p) * self.bin_hz;
            let magnitude = (lb - 0.25 * (la - lc) * p).exp();
            let midi = 69.0 + 12.0 * (hz / 440.0).log2();
            // Güçlü tepeler ağır basmasın: genliğin karekökü.
            let weight = magnitude.sqrt();
            let fine = ((midi * FINE_PER_SEMITONE as f64).round() as i64)
                .rem_euclid(FINE_BINS as i64) as usize;
            self.current[fine] += weight;
            if hz >= TUNING_MIN_HZ {
                let angle = 2.0 * std::f64::consts::PI * (midi - midi.round());
                self.tuning_sin += weight * angle.sin();
                self.tuning_cos += weight * angle.cos();
            }
        }
    }

    /// Şarkı bitti: akort kayması bulunur, kroma 12 perdeye katlanır ve ton tahmin edilir.
    pub fn finish(mut self) -> KeyAnalysis {
        if self.position % self.samples_per_second != 0 {
            self.close_second();
        }
        let tuning = if self.tuning_sin == 0.0 && self.tuning_cos == 0.0 {
            0.0
        } else {
            self.tuning_sin.atan2(self.tuning_cos) / (2.0 * std::f64::consts::PI)
        };
        let chroma: Vec<[u8; PITCH_CLASSES]> = self
            .seconds
            .iter()
            .map(|fine| quantize(&fold(fine, tuning)))
            .collect();
        let key = song_key(&chroma);
        KeyAnalysis {
            chroma,
            key,
            tuning: tuning as f32,
        }
    }
}

/// İnce kutuları akort kaymasına göre 12 perde sınıfına katlar: her kutu en yakın perdeye,
/// uzaklığına göre azalan ağırlıkla (cos²) eklenir.
fn fold(fine: &[f32; FINE_BINS], tuning: f64) -> [f64; PITCH_CLASSES] {
    let mut out = [0.0; PITCH_CLASSES];
    for (bin, &value) in fine.iter().enumerate() {
        if value <= 0.0 {
            continue;
        }
        let semitone = bin as f64 / FINE_PER_SEMITONE as f64 - tuning;
        let nearest = semitone.round();
        let distance = semitone - nearest;
        let class = (nearest as i64).rem_euclid(PITCH_CLASSES as i64) as usize;
        let weight = (std::f64::consts::PI * distance).cos().powi(2);
        out[class] += f64::from(value) * weight;
    }
    out
}

/// Saniyenin kromasını en güçlü perdeye göre 0..255'e çevirir.
fn quantize(chroma: &[f64; PITCH_CLASSES]) -> [u8; PITCH_CLASSES] {
    let top = chroma.iter().copied().fold(0.0, f64::max);
    if top <= 0.0 {
        return [0; PITCH_CLASSES];
    }
    chroma.map(|v| ((v / top) * 255.0).round().clamp(0.0, 255.0) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Testler hızlı olsun diye düşük hız; çözümleyici pencereyi hıza göre seçer.
    const RATE: u32 = 22_050;

    /// Notanın frekansı (MIDI numarası; 69 = La 440 Hz), akort kaymasıyla (sent).
    fn hz(midi: f64, cents: f64) -> f64 {
        440.0 * 2f64.powf((midi - 69.0 + cents / 100.0) / 12.0)
    }

    /// Akorların ardı ardına çalındığı sentetik müzik: her nota 6 harmonikli (genlik 1/h,
    /// tel ya da piyano gibi), her akor `chord_secs` sürer; altında akorun kök notası basta.
    /// `noise`: vurmalı gibi geniş bantlı gürültü (0..1).
    fn music(
        chords: &[&[f64]],
        chord_secs: f64,
        repeats: usize,
        cents: f64,
        noise: f64,
    ) -> Vec<f64> {
        let mut out = Vec::new();
        let mut state = 7u64;
        let per_chord = (chord_secs * f64::from(RATE)) as usize;
        for _ in 0..repeats {
            for chord in chords {
                let start = out.len();
                for i in 0..per_chord {
                    let t = (start + i) as f64 / f64::from(RATE);
                    let local = i as f64 / f64::from(RATE);
                    let envelope = (local / 0.02).min(1.0) * (-local * 0.6).exp();
                    let mut x = 0.0;
                    for (n, &note) in chord.iter().enumerate() {
                        // Kök nota bir oktav aşağıda da (bas).
                        let notes: &[f64] = if n == 0 {
                            &[note - 12.0, note]
                        } else {
                            &[note]
                        };
                        for &m in notes {
                            let f = hz(m, cents);
                            for h in 1..=6 {
                                let fh = f * h as f64;
                                if fh < 9_000.0 {
                                    x += (2.0 * std::f64::consts::PI * fh * t).sin() / h as f64;
                                }
                            }
                        }
                    }
                    state = state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1_442_695_040_888_963_407);
                    let white = (state >> 11) as f64 / (1u64 << 53) as f64 * 2.0 - 1.0;
                    out.push(0.05 * envelope * x + noise * 0.1 * white * (-local * 8.0).exp());
                }
            }
        }
        out
    }

    fn analyze(samples: &[f64]) -> KeyAnalysis {
        let mut analyzer = ChromaAnalyzer::new(RATE);
        for &x in samples {
            analyzer.add(x);
        }
        analyzer.finish()
    }

    /// MIDI numaralarıyla akorlar (60 = Do4).
    const C: f64 = 60.0;
    fn triad(root: f64, minor: bool) -> [f64; 3] {
        [root, root + if minor { 3.0 } else { 4.0 }, root + 7.0]
    }

    fn progression(tonic: f64, minor: bool) -> Vec<Vec<f64>> {
        if minor {
            // i – iv – V – i (armonik minör: V majör, yeden ses)
            vec![
                triad(tonic, true).to_vec(),
                triad(tonic + 5.0, true).to_vec(),
                triad(tonic + 7.0, false).to_vec(),
                triad(tonic, true).to_vec(),
            ]
        } else {
            // I – IV – V – I
            vec![
                triad(tonic, false).to_vec(),
                triad(tonic + 5.0, false).to_vec(),
                triad(tonic + 7.0, false).to_vec(),
                triad(tonic, false).to_vec(),
            ]
        }
    }

    fn detect(chords: &[Vec<f64>], cents: f64, noise: f64) -> MusicalKey {
        let refs: Vec<&[f64]> = chords.iter().map(Vec::as_slice).collect();
        analyze(&music(&refs, 1.0, 2, cents, noise)).key.unwrap()
    }

    #[test]
    fn camelot_carki_dogru() {
        // Majör: Do 8B, Sol 9B, Fa 7B, Si 1B; minör: La 8A, Mi 9A, Re 7A, Do 5A.
        let code = |tonic: usize, minor: bool| MusicalKey::new(tonic, minor, 1.0).camelot_code();
        assert_eq!(code(0, false), "8B");
        assert_eq!(code(7, false), "9B");
        assert_eq!(code(5, false), "7B");
        assert_eq!(code(11, false), "1B");
        assert_eq!(code(9, true), "8A");
        assert_eq!(code(4, true), "9A");
        assert_eq!(code(2, true), "7A");
        assert_eq!(code(0, true), "5A");
        // Her sayı bir majör ve bir minör tona düşer.
        let mut seen = std::collections::HashSet::new();
        for tonic in 0..12 {
            for minor in [false, true] {
                assert!(seen.insert(code(tonic, minor)));
            }
        }
        assert_eq!(seen.len(), 24);
    }

    #[test]
    fn isaretle_karsilastirma_mirex_siniflari() {
        let detected = |tonic: usize, minor: bool| MusicalKey::new(tonic, minor, 0.9);
        let am = MarkedKey {
            tonic: 9,
            minor: true,
        };
        assert_eq!(key_relation(&detected(9, true), am), KeyRelation::Same);
        // Mi minör (9A) ve Re minör (7A): beşli komşuları.
        assert_eq!(key_relation(&detected(4, true), am), KeyRelation::Fifth);
        assert_eq!(key_relation(&detected(2, true), am), KeyRelation::Fifth);
        // Do majör (8B): paralel majör.
        assert_eq!(key_relation(&detected(0, false), am), KeyRelation::Relative);
        // La majör: aynı tonik.
        assert_eq!(key_relation(&detected(9, false), am), KeyRelation::Parallel);
        assert_eq!(key_relation(&detected(1, true), am), KeyRelation::Other);
        // Çarkın ucu: 12A ile 1A komşudur.
        let csm = MarkedKey {
            tonic: 1,
            minor: true,
        };
        assert_eq!(key_relation(&detected(8, true), csm), KeyRelation::Fifth);
        assert!(KeyRelation::Relative.is_hit() && !KeyRelation::Parallel.is_hit());
    }

    #[test]
    fn profil_kromasi_kendi_tonunu_verir() {
        for tonic in 0..PITCH_CLASSES {
            for (minor, profile) in [(false, MAJOR_PROFILE), (true, MINOR_PROFILE)] {
                let chroma: [f64; 12] = std::array::from_fn(|i| profile[(i + 12 - tonic) % 12]);
                let key = estimate(&chroma).unwrap();
                assert_eq!((key.tonic as usize, key.minor), (tonic, minor));
                assert!(key.confidence > 0.9, "{key:?}");
            }
        }
        // Düz kroma: ton yok.
        assert_eq!(estimate(&[1.0; 12]), None);
        assert_eq!(estimate(&[0.0; 12]), None);
    }

    #[test]
    fn do_majorde_akor_dizisi_do_major_bulunur() {
        let key = detect(&progression(C, false), 0.0, 0.0);
        assert_eq!((key.tonic, key.minor), (0, false), "{key:?}");
        assert_eq!(key.camelot_code(), "8B");
        assert!(key.is_confident(), "{key:?}");
    }

    #[test]
    fn la_minorde_akor_dizisi_la_minor_bulunur() {
        let key = detect(&progression(C - 3.0, true), 0.0, 0.0);
        assert_eq!((key.tonic, key.minor), (9, true), "{key:?}");
        assert_eq!(key.camelot_code(), "8A");
        assert!(key.is_confident(), "{key:?}");
    }

    #[test]
    fn yirmi_dort_tonun_hepsi_bulunur() {
        let mut right = 0;
        let mut misses = Vec::new();
        for tonic in 0..12 {
            for minor in [false, true] {
                // Notalar 55–75 aralığında kalsın.
                let root = 55.0 + tonic as f64;
                let key = detect(&progression(root, minor), 0.0, 0.0);
                let expected = ((root as usize) % 12, minor);
                if (key.tonic as usize, key.minor) == expected {
                    right += 1;
                } else {
                    misses.push((expected, (key.tonic, key.minor)));
                }
            }
        }
        assert_eq!(right, 24, "{misses:?}");
    }

    #[test]
    fn pop_dizisi_ve_davul_gurultusuyle_de_bulunur() {
        // I – V – vi – IV (Do – Sol – Lam – Fa): en yaygın pop dizisi; La minör güçlü bir rakip.
        let chords = vec![
            triad(C, false).to_vec(),
            triad(C + 7.0 - 12.0, false).to_vec(),
            triad(C - 3.0, true).to_vec(),
            triad(C + 5.0 - 12.0, false).to_vec(),
        ];
        let key = detect(&chords, 0.0, 0.6);
        assert_eq!((key.tonic, key.minor), (0, false), "{key:?}");
    }

    #[test]
    fn akort_kaymasi_bulunur_ve_ton_bozulmaz() {
        // A = 432 Hz: −31,8 sent. Kayma bulunmazsa notalar iki perdeye bölünürdü.
        let refs: Vec<Vec<f64>> = progression(C - 3.0, true);
        let refs: Vec<&[f64]> = refs.iter().map(Vec::as_slice).collect();
        let cents = 1200.0 * (432.0f64 / 440.0).log2();
        let analysis = analyze(&music(&refs, 1.5, 3, cents, 0.0));
        assert!(
            (f64::from(analysis.tuning) - cents / 100.0).abs() < 0.05,
            "{}",
            analysis.tuning
        );
        let key = analysis.key.unwrap();
        assert_eq!((key.tonic, key.minor), (9, true), "{key:?}");
        assert!(key.is_confident(), "{key:?}");
    }

    #[test]
    fn yalnizca_davul_ve_sessizlik_tonsuz_ya_da_guvensiz() {
        // Yalnızca gürültü vuruşları.
        let drums = music(&[&[]], 0.5, 40, 0.0, 1.0);
        let key = analyze(&drums).key;
        assert!(key.is_none_or(|k| !k.is_confident()), "{key:?}");
        // Sessizlik: kroma sıfır, ton yok.
        let silent = analyze(&vec![0.0; RATE as usize * 5]);
        assert_eq!(silent.key, None);
        assert_eq!(silent.chroma.len(), 5);
        assert!(silent.chroma.iter().all(|c| c.iter().all(|&v| v == 0)));
    }

    #[test]
    fn bolum_bolum_ton_degisimi_gorunur() {
        // Pop şarkılarındaki gibi: 30 sn Do majör, son nakarat bir ton yukarıda (12 sn Re majör).
        let mut samples = Vec::new();
        for (tonic, repeats) in [(C, 5), (C + 2.0, 2)] {
            let chords = progression(tonic, false);
            let refs: Vec<&[f64]> = chords.iter().map(Vec::as_slice).collect();
            samples.extend(music(&refs, 1.5, repeats, 0.0, 0.0));
        }
        let analysis = analyze(&samples);
        assert_eq!(analysis.chroma.len(), 42);
        let first = analysis.key_between(0.0, 30.0).unwrap();
        let last = analysis.key_between(30.0, 42.0).unwrap();
        assert_eq!((first.tonic, first.minor), (0, false), "{first:?}");
        assert_eq!((last.tonic, last.minor), (2, false), "{last:?}");
        // Çok kısa bölümde ton söylenmez.
        assert_eq!(analysis.key_between(0.0, 4.0), None);

        // Arayüze: şarkının tonu Do majör; yalnızca son bölüm ton değişimi olarak görünür.
        let section = |start: f64, end: f64| Section {
            start,
            end,
            energy: 0.5,
            label: 0,
        };
        let map = analysis.key_map(&[section(0.0, 30.0), section(30.0, 42.0)]);
        assert_eq!(
            map.key.map(|k| k.camelot_code()),
            Some("8B".to_owned()),
            "{map:?}"
        );
        assert_eq!(map.sections[0], None);
        assert_eq!(
            map.sections[1].map(|k| k.camelot_code()),
            Some("10B".to_owned())
        );
    }

    #[test]
    fn tek_tonlu_sarkida_bolumler_ton_degisimi_gostermez() {
        let chords = progression(C - 3.0, true);
        let refs: Vec<&[f64]> = chords.iter().map(Vec::as_slice).collect();
        let analysis = analyze(&music(&refs, 1.5, 6, 0.0, 0.0));
        let section = |start: f64, end: f64| Section {
            start,
            end,
            energy: 0.5,
            label: 0,
        };
        let map = analysis.key_map(&[section(0.0, 18.0), section(18.0, 36.0), section(36.0, 38.0)]);
        assert_eq!(map.key.map(|k| k.camelot_code()), Some("8A".to_owned()));
        assert_eq!(map.sections, vec![None, None, None]);
    }
}
