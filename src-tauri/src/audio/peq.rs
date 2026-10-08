//! Kulaklık düzeltmesi: parametrik ekolayzer.
//!
//! Kulaklıklar sesi düz vermez; ölçüm laboratuvarları her modelin frekans
//! yanıtını ölçer, AutoEq gibi araçlar da bunu bir hedef eğriye (Harman) getiren
//! parametrik filtreler hesaplar. Lyraska bu filtreleri, AutoEq'in ve Equalizer
//! APO'nun ortak metin biçiminden (`ParametricEQ.txt`) okur:
//!
//! ```text
//! Preamp: -6.2 dB
//! Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70
//! Filter 2: ON PK Fc 1923 Hz Gain -3.2 dB Q 1.31
//! ```
//!
//! Düzeltme, kullanıcının 10 bantlı ekolayzerinden önce ve ondan bağımsız
//! uygulanır. Filtreler RBJ "Audio EQ Cookbook" formülleriyle, 64-bit tasarlanır.
//! Ayar değişince eski ve yeni filtre takımı ~30 ms boyunca çapraz geçişle
//! karıştırılır (tıkırtı olmaz). Gerçek zamanlı yolda bellek ayırma ve kilit yoktur.

use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Sample;

/// Bir profilde en fazla bu kadar filtre (AutoEq genellikle 10 kullanır).
pub const MAX_FILTERS: usize = 20;
/// Kabul edilen sınırlar.
const MIN_FREQ: f64 = 10.0;
const MAX_FREQ: f64 = 24_000.0;
const MAX_GAIN_DB: f64 = 30.0;
const MIN_Q: f64 = 0.05;
const MAX_Q: f64 = 30.0;
/// Ön kazanç sınırı: düzeltme sesi yükseltmez, yalnızca kısabilir.
const MIN_PREAMP_DB: f64 = -40.0;
/// Ayar değişince eski ve yeni filtrelerin karıştırılma süresi.
const CROSSFADE_SECONDS: f64 = 0.03;

/// Filtre türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterKind {
    /// Tepe/çukur (PK).
    Peaking,
    /// Alçak raf (LSC/LS): belirtilen frekansın altını yükseltir/kısar.
    LowShelf,
    /// Yüksek raf (HSC/HS): belirtilen frekansın üstünü yükseltir/kısar.
    HighShelf,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PeqFilter {
    pub kind: FilterKind,
    pub freq_hz: f64,
    pub gain_db: f64,
    pub q: f64,
}

/// Bir kulaklığın düzeltme profili.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadphoneProfile {
    /// Kullanıcıya gösterilen ad (genellikle kulaklık modeli).
    pub name: String,
    /// Bozulmayı önleyen ön kazanç (dB, ≤ 0).
    pub preamp_db: f64,
    pub filters: Vec<PeqFilter>,
}

/// Kalıcı kulaklık düzeltmesi ayarı.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct HeadphoneSettings {
    pub enabled: bool,
    pub profile: Option<HeadphoneProfile>,
}

impl HeadphoneSettings {
    pub fn sanitized(self) -> Self {
        let profile = self.profile.map(HeadphoneProfile::sanitized);
        Self {
            enabled: self.enabled && profile.is_some(),
            profile,
        }
    }
}

/// Arayüze giden durum: ayar ve çizim için düzeltme eğrisi.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeadphoneState {
    pub enabled: bool,
    pub profile: Option<HeadphoneProfile>,
    /// Eğrinin frekansları (Hz) ve kazançları (dB, ön kazanç dahil); profil yoksa boş.
    pub curve_hz: Vec<f64>,
    pub curve_db: Vec<f64>,
}

impl HeadphoneState {
    pub fn new(settings: HeadphoneSettings) -> Self {
        let curve_hz: Vec<f64> = match settings.profile {
            Some(_) => (0..=120)
                .map(|i| 20.0 * 1000f64.powf(f64::from(i) / 120.0))
                .collect(),
            None => Vec::new(),
        };
        let curve_db = settings
            .profile
            .as_ref()
            .map(|p| p.response_db(&curve_hz, 96_000.0))
            .unwrap_or_default();
        Self {
            enabled: settings.enabled,
            profile: settings.profile,
            curve_hz,
            curve_db,
        }
    }
}

/// Profil dosyası okunamadığında.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ProfileError {
    #[error("Dosyada kulaklık düzeltmesi bulunamadı. AutoEq'ten \"ParametricEQ.txt\" (Equalizer APO biçimi) dosyasını seçin.")]
    Empty,
    #[error("{line}. satır okunamadı: \"{text}\"")]
    BadLine { line: usize, text: String },
    #[error("{line}. satırdaki \"{kind}\" filtresi desteklenmiyor (desteklenenler: PK, LSC, HSC, LS, HS).")]
    UnsupportedFilter { line: usize, kind: String },
    #[error("Profilde en fazla {MAX_FILTERS} filtre olabilir.")]
    TooManyFilters,
}

impl HeadphoneProfile {
    /// Dosya adından kulaklık adı: AutoEq dosyaları "Model ParametricEQ.txt" biçimindedir.
    pub fn name_from_file(stem: &str) -> String {
        let trimmed = stem.trim();
        for suffix in [
            " ParametricEQ",
            " ParametricEq",
            "_ParametricEQ",
            " parametric eq",
        ] {
            if let Some(name) = trimmed.strip_suffix(suffix) {
                return name.trim().to_owned();
            }
        }
        trimmed.to_owned()
    }

    /// AutoEq / Equalizer APO parametrik biçimini okur. Kapalı (OFF) filtreler atlanır.
    pub fn parse(text: &str, name: &str) -> Result<Self, ProfileError> {
        let mut preamp_db = 0.0;
        let mut filters = Vec::new();
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let trimmed = raw.trim().trim_start_matches('\u{feff}');
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            let lower = trimmed.to_ascii_lowercase();
            if let Some(rest) = lower.strip_prefix("preamp:") {
                preamp_db = parse_number(rest.trim().trim_end_matches("db").trim())
                    .ok_or_else(|| bad(line, trimmed))?;
                continue;
            }
            if !lower.starts_with("filter") {
                // Equalizer APO'nun başka komutları (Device:, Include: vb.) yok sayılır.
                continue;
            }
            let Some((_, body)) = trimmed.split_once(':') else {
                return Err(bad(line, trimmed));
            };
            let words: Vec<&str> = body.split_whitespace().collect();
            let Some((&state, rest)) = words.split_first() else {
                return Err(bad(line, trimmed));
            };
            if state.eq_ignore_ascii_case("off") {
                continue;
            }
            if !state.eq_ignore_ascii_case("on") {
                return Err(bad(line, trimmed));
            }
            let Some((&kind_word, params)) = rest.split_first() else {
                return Err(bad(line, trimmed));
            };
            let kind = match kind_word.to_ascii_uppercase().as_str() {
                "PK" | "PEQ" => FilterKind::Peaking,
                "LSC" | "LS" => FilterKind::LowShelf,
                "HSC" | "HS" => FilterKind::HighShelf,
                other => {
                    return Err(ProfileError::UnsupportedFilter {
                        line,
                        kind: other.to_owned(),
                    })
                }
            };
            let value = |key: &str| {
                params
                    .iter()
                    .position(|w| w.eq_ignore_ascii_case(key))
                    .and_then(|i| params.get(i + 1))
                    .and_then(|w| parse_number(w))
            };
            let freq_hz = value("fc").ok_or_else(|| bad(line, trimmed))?;
            let gain_db = value("gain").ok_or_else(|| bad(line, trimmed))?;
            // Raf filtrelerinde Q yazılmamışsa Equalizer APO'nun varsayılanı (0,707).
            let q = match value("q") {
                Some(q) => q,
                None if kind != FilterKind::Peaking => std::f64::consts::FRAC_1_SQRT_2,
                None => return Err(bad(line, trimmed)),
            };
            filters.push(PeqFilter {
                kind,
                freq_hz,
                gain_db,
                q,
            });
            if filters.len() > MAX_FILTERS {
                return Err(ProfileError::TooManyFilters);
            }
        }
        if filters.is_empty() {
            return Err(ProfileError::Empty);
        }
        Ok(Self {
            name: name.trim().to_owned(),
            preamp_db,
            filters,
        }
        .sanitized())
    }

    /// Değerleri güvenli aralığa çeker. Ön kazanç, filtrelerin en yüksek
    /// yükseltmesinden az olamaz: düzeltme sesi taşırmasın.
    pub fn sanitized(mut self) -> Self {
        self.filters.truncate(MAX_FILTERS);
        for f in &mut self.filters {
            f.freq_hz = finite_or(f.freq_hz, 1000.0).clamp(MIN_FREQ, MAX_FREQ);
            f.gain_db = finite_or(f.gain_db, 0.0).clamp(-MAX_GAIN_DB, MAX_GAIN_DB);
            f.q = finite_or(f.q, 1.0).clamp(MIN_Q, MAX_Q);
        }
        let peak = response_peak_db(&self.filters);
        self.preamp_db = finite_or(self.preamp_db, 0.0)
            .min(-peak.max(0.0))
            .clamp(MIN_PREAMP_DB, 0.0);
        self
    }

    /// Verilen frekanslarda toplam düzeltme eğrisi (dB, ön kazanç dahil).
    pub fn response_db(&self, freqs_hz: &[f64], sample_rate: f64) -> Vec<f64> {
        let coefs: Vec<Biquad> = self
            .filters
            .iter()
            .map(|f| Biquad::design(f, sample_rate))
            .collect();
        freqs_hz
            .iter()
            .map(|&hz| {
                let w = 2.0 * PI * hz / sample_rate;
                self.preamp_db + coefs.iter().map(|c| c.response_db(w)).sum::<f64>()
            })
            .collect()
    }
}

fn bad(line: usize, text: &str) -> ProfileError {
    ProfileError::BadLine {
        line,
        text: text.chars().take(80).collect(),
    }
}

/// Sayı okur; Türkçe ondalık virgülünü de kabul eder.
fn parse_number(word: &str) -> Option<f64> {
    word.replace(',', ".")
        .parse::<f64>()
        .ok()
        .filter(|x| x.is_finite())
}

fn finite_or(x: f64, fallback: f64) -> f64 {
    if x.is_finite() {
        x
    } else {
        fallback
    }
}

/// Filtrelerin toplam yanıtının en yüksek değeri (dB); 20 Hz–20 kHz taranır.
fn response_peak_db(filters: &[PeqFilter]) -> f64 {
    let rate = 96_000.0;
    let coefs: Vec<Biquad> = filters.iter().map(|f| Biquad::design(f, rate)).collect();
    (0..=400)
        .map(|i| 20.0 * 1000f64.powf(f64::from(i) / 400.0))
        .map(|hz| {
            let w = 2.0 * PI * hz / rate;
            coefs.iter().map(|c| c.response_db(w)).sum::<f64>()
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// İkinci dereceden filtre katsayıları (a0 = 1).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

impl Biquad {
    const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    /// RBJ Audio EQ Cookbook. Nyquist'e çok yakın filtreler etkisiz bırakılır.
    fn design(filter: &PeqFilter, sample_rate: f64) -> Self {
        if filter.freq_hz >= sample_rate * 0.49 || filter.gain_db.abs() < 1e-6 {
            return Self::IDENTITY;
        }
        let a = 10f64.powf(filter.gain_db / 40.0);
        let w0 = 2.0 * PI * filter.freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * filter.q);
        let (b0, b1, b2, a0, a1, a2) = match filter.kind {
            FilterKind::Peaking => (
                1.0 + alpha * a,
                -2.0 * cos,
                1.0 - alpha * a,
                1.0 + alpha / a,
                -2.0 * cos,
                1.0 - alpha / a,
            ),
            FilterKind::LowShelf => {
                let k = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) - (a - 1.0) * cos + k),
                    2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                    a * ((a + 1.0) - (a - 1.0) * cos - k),
                    (a + 1.0) + (a - 1.0) * cos + k,
                    -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                    (a + 1.0) + (a - 1.0) * cos - k,
                )
            }
            FilterKind::HighShelf => {
                let k = 2.0 * a.sqrt() * alpha;
                (
                    a * ((a + 1.0) + (a - 1.0) * cos + k),
                    -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                    a * ((a + 1.0) + (a - 1.0) * cos - k),
                    (a + 1.0) - (a - 1.0) * cos + k,
                    2.0 * ((a - 1.0) - (a + 1.0) * cos),
                    (a + 1.0) - (a - 1.0) * cos - k,
                )
            }
        };
        Self {
            b0: b0 / a0,
            b1: b1 / a0,
            b2: b2 / a0,
            a1: a1 / a0,
            a2: a2 / a0,
        }
    }

    fn response_db(&self, w: f64) -> f64 {
        let (cos_w, cos_2w) = (w.cos(), (2.0 * w).cos());
        let Self { b0, b1, b2, a1, a2 } = *self;
        let num = b0 * b0
            + b1 * b1
            + b2 * b2
            + 2.0 * (b0 * b1 + b1 * b2) * cos_w
            + 2.0 * b0 * b2 * cos_2w;
        let den = 1.0 + a1 * a1 + a2 * a2 + 2.0 * (a1 + a1 * a2) * cos_w + 2.0 * a2 * cos_2w;
        10.0 * (num.max(1e-300) / den.max(1e-300)).log10()
    }
}

/// Arayüzden ses iş parçacığına profil taşıyan kilitsiz kanal (bkz. `EqControl`).
#[derive(Debug)]
pub struct PeqControl {
    version: AtomicU64,
    enabled: AtomicBool,
    count: AtomicUsize,
    preamp_db: AtomicU64,
    /// Her filtre için tür, frekans, kazanç, Q.
    filters: [[AtomicU64; 4]; MAX_FILTERS],
}

impl Default for PeqControl {
    fn default() -> Self {
        Self {
            version: AtomicU64::new(0),
            enabled: AtomicBool::new(false),
            count: AtomicUsize::new(0),
            preamp_db: AtomicU64::new(0f64.to_bits()),
            filters: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
        }
    }
}

impl PeqControl {
    /// Profili ve açık/kapalı durumunu uygular. Profil yoksa düzeltme kapalıdır.
    pub fn set(&self, profile: Option<&HeadphoneProfile>, enabled: bool) {
        let filters = profile.map_or(&[][..], |p| &p.filters[..p.filters.len().min(MAX_FILTERS)]);
        for (slot, f) in self.filters.iter().zip(filters) {
            let kind = match f.kind {
                FilterKind::Peaking => 0.0,
                FilterKind::LowShelf => 1.0,
                FilterKind::HighShelf => 2.0,
            };
            for (cell, value) in slot.iter().zip([kind, f.freq_hz, f.gain_db, f.q]) {
                cell.store(value.to_bits(), Ordering::Relaxed);
            }
        }
        self.count.store(filters.len(), Ordering::Relaxed);
        self.preamp_db.store(
            profile.map_or(0.0, |p| p.preamp_db).to_bits(),
            Ordering::Relaxed,
        );
        self.enabled
            .store(enabled && !filters.is_empty(), Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Ses iş parçacığı için katsayılar (ayırma yok).
    fn design(&self, sample_rate: f64, bank: &mut Bank) {
        let enabled = self.enabled.load(Ordering::Relaxed);
        let count = if enabled {
            self.count.load(Ordering::Relaxed).min(MAX_FILTERS)
        } else {
            0
        };
        for (i, coef) in bank.coefs.iter_mut().enumerate() {
            *coef = if i < count {
                let v = |k: usize| f64::from_bits(self.filters[i][k].load(Ordering::Relaxed));
                let kind = match v(0) as u8 {
                    1 => FilterKind::LowShelf,
                    2 => FilterKind::HighShelf,
                    _ => FilterKind::Peaking,
                };
                Biquad::design(
                    &PeqFilter {
                        kind,
                        freq_hz: v(1),
                        gain_db: v(2),
                        q: v(3),
                    },
                    sample_rate,
                )
            } else {
                Biquad::IDENTITY
            };
        }
        bank.count = count;
        bank.gain = if enabled {
            10f64.powf(f64::from_bits(self.preamp_db.load(Ordering::Relaxed)) / 20.0)
        } else {
            1.0
        };
    }
}

/// Bir filtre takımı ve her kanal için durumları.
struct Bank {
    coefs: [Biquad; MAX_FILTERS],
    count: usize,
    gain: f64,
    state: Vec<[[f64; 2]; MAX_FILTERS]>,
}

impl Bank {
    fn new(channels: usize) -> Self {
        Self {
            coefs: [Biquad::IDENTITY; MAX_FILTERS],
            count: 0,
            gain: 1.0,
            state: vec![[[0.0; 2]; MAX_FILTERS]; channels],
        }
    }

    fn is_bypass(&self) -> bool {
        self.count == 0 && self.gain == 1.0
    }

    fn clear(&mut self) {
        for value in self.state.iter_mut().flatten().flatten() {
            *value = 0.0;
        }
    }

    #[inline]
    fn process(&mut self, channel: usize, x: Sample) -> Sample {
        if self.is_bypass() {
            return x;
        }
        let Some(state) = self.state.get_mut(channel) else {
            return x;
        };
        let mut y = x;
        for (f, s) in self.coefs.iter().zip(state.iter_mut()).take(self.count) {
            let input = y;
            y = f.b0 * input + s[0];
            s[0] = f.b1 * input - f.a1 * y + s[1];
            s[1] = f.b2 * input - f.a2 * y;
        }
        y * self.gain
    }
}

/// Kulaklık düzeltmesini uygulayan gerçek zamanlı işlemci.
pub struct PeqProcessor {
    control: Arc<PeqControl>,
    sample_rate: f64,
    seen_version: u64,
    current: Bank,
    next: Bank,
    /// Çapraz geçişte kalan kare sayısı (0 = geçiş yok).
    fade_left: usize,
    fade_frames: usize,
}

impl PeqProcessor {
    pub fn new(control: Arc<PeqControl>, channels: usize, sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let channels = channels.max(1);
        let mut current = Bank::new(channels);
        control.design(rate, &mut current);
        Self {
            seen_version: control.version(),
            control,
            sample_rate: rate,
            current,
            next: Bank::new(channels),
            fade_left: 0,
            fade_frames: ((CROSSFADE_SECONDS * rate) as usize).max(1),
        }
    }

    /// Düzeltme şu an sese dokunmuyor mu?
    pub fn is_bypass(&self) -> bool {
        self.fade_left == 0 && self.current.is_bypass()
    }

    /// Her doldurma turunun başında: yeni profil varsa çapraz geçişi başlatır.
    pub fn begin_block(&mut self) {
        let version = self.control.version();
        if version != self.seen_version && self.fade_left == 0 {
            self.seen_version = version;
            self.control.design(self.sample_rate, &mut self.next);
            self.next.clear();
            self.fade_left = self.fade_frames;
        }
        for bank in [&mut self.current, &mut self.next] {
            for value in bank.state.iter_mut().flatten().flatten() {
                if value.abs() < 1e-200 {
                    *value = 0.0;
                }
            }
        }
    }

    /// Bir karenin bütün kanallarını yerinde işler.
    #[inline]
    pub fn process_frame(&mut self, frame: &mut [Sample]) {
        if self.fade_left == 0 {
            if !self.current.is_bypass() {
                for (channel, x) in frame.iter_mut().enumerate() {
                    *x = self.current.process(channel, *x);
                }
            }
            return;
        }
        // Eşit güçlü değil, doğrusal çapraz geçiş: iki takım aynı sesi işlediği için
        // (ilintili) doğrusal geçiş seviyeyi korur.
        let t = 1.0 - self.fade_left as f64 / self.fade_frames as f64;
        for (channel, x) in frame.iter_mut().enumerate() {
            let old = self.current.process(channel, *x);
            let new = self.next.process(channel, *x);
            *x = old + (new - old) * t;
        }
        self.fade_left -= 1;
        if self.fade_left == 0 {
            std::mem::swap(&mut self.current, &mut self.next);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AUTOEQ_SAMPLE: &str = "Preamp: -6.4 dB
Filter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70
Filter 2: ON PK Fc 1923 Hz Gain -3.2 dB Q 1.31
Filter 3: ON PK Fc 3500 Hz Gain 4.0 dB Q 2.00
Filter 4: OFF PK Fc 5000 Hz Gain 9.0 dB Q 1.00
Filter 5: ON HSC Fc 10000 Hz Gain 2.0 dB Q 0.70
";

    fn gain_at(profile: &HeadphoneProfile, hz: f64) -> f64 {
        profile.response_db(&[hz], 48_000.0)[0]
    }

    #[test]
    fn dosya_adindan_kulaklik_adi() {
        assert_eq!(
            HeadphoneProfile::name_from_file("Sennheiser HD 600 ParametricEQ"),
            "Sennheiser HD 600"
        );
        assert_eq!(HeadphoneProfile::name_from_file("kulakligim"), "kulakligim");
    }

    #[test]
    fn ayar_ve_durum() {
        let profile = HeadphoneProfile::parse(AUTOEQ_SAMPLE, "x").unwrap();
        let settings = HeadphoneSettings {
            enabled: true,
            profile: None,
        }
        .sanitized();
        assert!(!settings.enabled, "profil yoksa açık olamaz");
        let state = HeadphoneState::new(HeadphoneSettings {
            enabled: true,
            profile: Some(profile),
        });
        assert_eq!(state.curve_hz.len(), state.curve_db.len());
        assert!(state.curve_hz.len() > 100);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["profile"]["filters"][0]["kind"], "lowShelf");
        assert!(json["profile"]["preampDb"].is_number());
        assert!(json["curveDb"].is_array());
    }

    #[test]
    fn autoeq_dosyasini_okur() {
        let profile = HeadphoneProfile::parse(AUTOEQ_SAMPLE, " Sennheiser HD 600 ").unwrap();
        assert_eq!(profile.name, "Sennheiser HD 600");
        assert_eq!(profile.filters.len(), 4, "kapalı filtre atlanır");
        assert_eq!(profile.filters[0].kind, FilterKind::LowShelf);
        assert_eq!(profile.filters[3].kind, FilterKind::HighShelf);
        assert!((profile.filters[1].freq_hz - 1923.0).abs() < 1e-9);
        assert!((profile.filters[1].gain_db + 3.2).abs() < 1e-9);
        assert!((profile.filters[1].q - 1.31).abs() < 1e-9);
        assert!((profile.preamp_db + 6.4).abs() < 1e-9);
    }

    #[test]
    fn bozuk_ve_desteklenmeyen_dosyalar_anlasilir_hata_verir() {
        assert_eq!(HeadphoneProfile::parse("", "x"), Err(ProfileError::Empty));
        assert_eq!(
            HeadphoneProfile::parse("merhaba dünya\n", "x"),
            Err(ProfileError::Empty)
        );
        assert!(matches!(
            HeadphoneProfile::parse("Filter 1: ON LP Fc 100 Hz\n", "x"),
            Err(ProfileError::UnsupportedFilter { line: 1, .. })
        ));
        assert!(matches!(
            HeadphoneProfile::parse(
                "Preamp: -3 dB\nFilter 1: ON PK Fc abc Hz Gain 1 dB Q 1\n",
                "x"
            ),
            Err(ProfileError::BadLine { line: 2, .. })
        ));
        let many: String = (1..=25)
            .map(|i| format!("Filter {i}: ON PK Fc 1000 Hz Gain 1 dB Q 1\n"))
            .collect();
        assert_eq!(
            HeadphoneProfile::parse(&many, "x"),
            Err(ProfileError::TooManyFilters)
        );
    }

    #[test]
    fn virgullu_sayi_ve_q_suz_raf_kabul_edilir() {
        let profile =
            HeadphoneProfile::parse("\u{feff}Filter: ON LS Fc 100 Hz Gain 3,5 dB\n", "x").unwrap();
        assert!((profile.filters[0].gain_db - 3.5).abs() < 1e-9);
        assert!((profile.filters[0].q - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
    }

    #[test]
    fn filtreler_istenen_egriyi_verir() {
        let pk = HeadphoneProfile {
            name: "pk".into(),
            preamp_db: -6.0,
            filters: vec![PeqFilter {
                kind: FilterKind::Peaking,
                freq_hz: 1000.0,
                gain_db: 6.0,
                q: 1.0,
            }],
        };
        assert!(
            (gain_at(&pk, 1000.0) - 0.0).abs() < 0.01,
            "tepe + ön kazanç"
        );
        assert!((gain_at(&pk, 30.0) + 6.0).abs() < 0.05);

        let low = HeadphoneProfile {
            name: "ls".into(),
            preamp_db: -8.0,
            filters: vec![PeqFilter {
                kind: FilterKind::LowShelf,
                freq_hz: 100.0,
                gain_db: 8.0,
                q: std::f64::consts::FRAC_1_SQRT_2,
            }],
        };
        assert!((gain_at(&low, 20.0) - 0.0).abs() < 0.3, "raf altı yükselir");
        assert!(
            (gain_at(&low, 100.0) + 4.0).abs() < 0.1,
            "köşede yarı kazanç"
        );
        assert!((gain_at(&low, 5000.0) + 8.0).abs() < 0.05);
    }

    #[test]
    fn on_kazanc_tasmayi_onler() {
        // Dosyada ön kazanç yok (0 dB) ama 6 dB yükseltme var: ön kazanç düşürülür.
        let profile =
            HeadphoneProfile::parse("Filter 1: ON PK Fc 1000 Hz Gain 6 dB Q 1\n", "x").unwrap();
        assert!(profile.preamp_db <= -5.99, "{}", profile.preamp_db);
        // Yükseltmeyen profilde ön kazanç 0'ı geçmez.
        let cut = HeadphoneProfile::parse(
            "Preamp: 4 dB\nFilter 1: ON PK Fc 1000 Hz Gain -6 dB Q 1\n",
            "x",
        )
        .unwrap();
        assert_eq!(cut.preamp_db, 0.0);
    }

    fn measure(processor: &mut PeqProcessor, hz: f64, rate: u32) -> f64 {
        let n = rate as usize;
        let w = 2.0 * PI * hz / f64::from(rate);
        let mut sum = 0.0;
        for i in 0..n {
            if i % 512 == 0 {
                processor.begin_block();
            }
            let mut frame = [(w * i as f64).sin(), 0.0];
            processor.process_frame(&mut frame);
            if i >= n / 2 {
                sum += frame[0] * frame[0];
            }
        }
        10.0 * (sum / (n / 2) as f64 * 2.0).log10()
    }

    #[test]
    fn islemci_profili_uygular_ve_kapatinca_dokunmaz() {
        let rate = 48_000;
        let control = Arc::new(PeqControl::default());
        let profile = HeadphoneProfile::parse(AUTOEQ_SAMPLE, "x").unwrap();
        let mut processor = PeqProcessor::new(Arc::clone(&control), 2, rate);
        assert!(processor.is_bypass());

        control.set(Some(&profile), true);
        for hz in [50.0, 1923.0, 3500.0, 12_000.0] {
            let measured = measure(&mut processor, hz, rate);
            let expected = gain_at(&profile, hz);
            assert!(
                (measured - expected).abs() < 0.05,
                "{hz} Hz: {measured} != {expected}"
            );
        }

        // Kapatınca ses bit bit aynı geçer (geçiş bittikten sonra).
        control.set(Some(&profile), false);
        measure(&mut processor, 1000.0, rate);
        assert!(processor.is_bypass());
        let mut frame = [0.123, -0.456];
        processor.process_frame(&mut frame);
        assert_eq!(frame, [0.123, -0.456]);
    }

    #[test]
    fn profil_degisirken_ani_sicrama_olmaz() {
        let rate = 48_000;
        let control = Arc::new(PeqControl::default());
        let mut processor = PeqProcessor::new(Arc::clone(&control), 1, rate);
        let loud = HeadphoneProfile {
            name: "x".into(),
            preamp_db: -12.0,
            filters: vec![PeqFilter {
                kind: FilterKind::Peaking,
                freq_hz: 200.0,
                gain_db: 12.0,
                q: 0.7,
            }],
        };
        let w = 2.0 * PI * 200.0 / f64::from(rate);
        let mut previous = 0.0;
        let mut biggest = 0.0f64;
        for i in 0..rate as usize {
            if i == rate as usize / 3 {
                control.set(Some(&loud), true);
            }
            if i % 256 == 0 {
                processor.begin_block();
            }
            let mut frame = [0.5 * (w * i as f64).sin()];
            processor.process_frame(&mut frame);
            biggest = biggest.max((frame[0] - previous).abs());
            previous = frame[0];
        }
        // 200 Hz, 0,5 genlikli sinüsün örnekler arası en büyük adımı ~0,013.
        assert!(biggest < 0.02, "{biggest}");
    }
}
