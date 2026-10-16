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
//! uygulanır. Filtreler 64-bit tasarlanır ([`super::biquad`]): varsayılan, Equalizer APO ve
//! AutoEq ile bire bir aynı RBJ formülleridir; istenirse analog eğriye eşlenmiş tasarım
//! (yüksek frekanslarda daha doğru) kullanılır. Ayar değişince eski ve yeni filtre takımı
//! ~30 ms boyunca çapraz geçişle karıştırılır (tıkırtı olmaz). Gerçek zamanlı yolda bellek
//! ayırma ve kilit yoktur.
//!
//! Dosya biçimi Equalizer APO'nun anlamlarıyla okunur (kaynak kodundan doğrulandı):
//! "LSC/HSC" orta frekanslı, "LS/HS" köşe frekanslı raftır (orta = köşe × 10^(|kazanç|/(80·S)));
//! raf Q yerine eğimle ("LSC 6 dB": S = 6/12) ya da hiç verilmeden (S = 0,9) yazılabilir;
//! tepe süzgecinde Q yerine bant genişliği ("BW Oct 1") olabilir; LP/HP/BP'de Q yoksa 0,707,
//! çentikte 30'dur.

use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::biquad::{self, Biquad, Kind, Method, Spec};
use super::Sample;

/// Bir profilde en fazla bu kadar filtre (AutoEq genellikle 10 kullanır).
pub const MAX_FILTERS: usize = 20;
/// Kabul edilen sınırlar.
const MIN_FREQ: f64 = 10.0;
const MAX_FREQ: f64 = 24_000.0;
const MAX_GAIN_DB: f64 = 30.0;
const MIN_Q: f64 = 0.05;
const MAX_Q: f64 = 30.0;
/// Equalizer APO'nun varsayılanları: Q'suz rafın eğimi, LP/HP/BP'nin ve çentiğin Q'su.
const DEFAULT_SHELF_SLOPE: f64 = 0.9;
const DEFAULT_Q: f64 = std::f64::consts::FRAC_1_SQRT_2;
const DEFAULT_NOTCH_Q: f64 = 30.0;
/// Ön kazanç sınırı: düzeltme sesi yükseltmez, yalnızca kısabilir.
const MIN_PREAMP_DB: f64 = -40.0;
/// Ayar değişince eski ve yeni filtrelerin karıştırılma süresi.
const CROSSFADE_SECONDS: f64 = 0.03;

/// Filtre türü.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum FilterKind {
    /// Tepe/çukur (PK).
    Peaking,
    /// Alçak raf (LSC/LS): belirtilen frekansın altını yükseltir/kısar.
    LowShelf,
    /// Yüksek raf (HSC/HS): belirtilen frekansın üstünü yükseltir/kısar.
    HighShelf,
    /// Alçak geçiren (LP/LPQ), 12 dB/oktav.
    LowPass,
    /// Yüksek geçiren (HP/HPQ), 12 dB/oktav.
    HighPass,
    /// Bant geçiren (BP), tepe kazancı 0 dB.
    BandPass,
    /// Çentik (NO).
    Notch,
    /// Tüm geçiren (AP): genliğe dokunmaz, fazı döndürür.
    AllPass,
}

impl FilterKind {
    const ALL: [Self; 8] = [
        Self::Peaking,
        Self::LowShelf,
        Self::HighShelf,
        Self::LowPass,
        Self::HighPass,
        Self::BandPass,
        Self::Notch,
        Self::AllPass,
    ];

    /// Kazanç kullanılır mı? (Tepe ve raflar.)
    pub fn has_gain(self) -> bool {
        self.shape().has_gain()
    }

    fn shape(self) -> Kind {
        match self {
            Self::Peaking => Kind::Peaking,
            Self::LowShelf => Kind::LowShelf,
            Self::HighShelf => Kind::HighShelf,
            Self::LowPass => Kind::LowPass,
            Self::HighPass => Kind::HighPass,
            Self::BandPass => Kind::BandPass,
            Self::Notch => Kind::Notch,
            Self::AllPass => Kind::AllPass,
        }
    }

    /// Kilitsiz kanalda taşınan sıra numarası.
    fn code(self) -> usize {
        Self::ALL.iter().position(|&k| k == self).unwrap_or(0)
    }

    fn from_code(code: usize) -> Self {
        Self::ALL.get(code).copied().unwrap_or(Self::Peaking)
    }
}

/// Süzgeçlerin tasarım yöntemi (bkz. [`super::biquad`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub enum DesignMethod {
    /// Equalizer APO ve AutoEq ile bire bir aynı (RBJ).
    #[default]
    Rbj,
    /// Analog eğriye eşlenmiş: yüksek frekanslarda tasarlanan eğriye daha yakın.
    Matched,
}

impl DesignMethod {
    fn method(self) -> Method {
        match self {
            Self::Rbj => Method::Rbj,
            Self::Matched => Method::Matched,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "HeadphoneFilter"))]
pub struct PeqFilter {
    pub kind: FilterKind,
    /// Orta frekans (raflarda orta kazanç noktası), Hz.
    pub freq_hz: f64,
    /// Kazanç (dB); kazanç kullanmayan türlerde 0.
    pub gain_db: f64,
    pub q: f64,
}

impl PeqFilter {
    fn spec(&self) -> Spec {
        Spec {
            kind: self.kind.shape(),
            freq_hz: self.freq_hz,
            gain_db: if self.kind.has_gain() {
                self.gain_db
            } else {
                0.0
            },
            q: self.q,
        }
    }
}

/// Bir kulaklığın düzeltme profili.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct HeadphoneProfile {
    /// Kullanıcıya gösterilen ad (genellikle kulaklık modeli).
    pub name: String,
    /// Bozulmayı önleyen ön kazanç (dB, ≤ 0).
    pub preamp_db: f64,
    pub filters: Vec<PeqFilter>,
    /// Tasarım yöntemi; eski ayar dosyalarında ve içe aktarılan profillerde Equalizer APO ile
    /// aynı (RBJ).
    #[serde(default)]
    pub design: DesignMethod,
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
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
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
            .map(|p| p.response_db(&curve_hz, DISPLAY_RATE))
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
    #[error("{line}. satırdaki \"{kind}\" filtresi desteklenmiyor (desteklenenler: PK, LS, HS, LSC, HSC, LP, HP, BP, NO, AP).")]
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
            filters.push(parse_filter(kind_word, params).map_err(|e| match e {
                FilterError::Unsupported => ProfileError::UnsupportedFilter {
                    line,
                    kind: kind_word.to_owned(),
                },
                FilterError::Bad => bad(line, trimmed),
            })?);
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
            design: DesignMethod::Rbj,
        }
        .sanitized())
    }

    /// Değerleri güvenli aralığa çeker. Ön kazanç, filtrelerin en yüksek
    /// yükseltmesinden az olamaz: düzeltme sesi taşırmasın.
    pub fn sanitized(mut self) -> Self {
        self.filters.truncate(MAX_FILTERS);
        for f in &mut self.filters {
            f.freq_hz = finite_or(f.freq_hz, 1000.0).clamp(MIN_FREQ, MAX_FREQ);
            f.gain_db = if f.kind.has_gain() {
                finite_or(f.gain_db, 0.0).clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
            } else {
                0.0
            };
            f.q = finite_or(f.q, 1.0).clamp(MIN_Q, MAX_Q);
        }
        let peak = response_peak_db(&self.filters, self.design);
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
            .map(|f| design(f, sample_rate, self.design))
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

/// Eğrilerin çizildiği örnekleme hızı (tasarım farkı duyulur bantta en küçük).
const DISPLAY_RATE: f64 = 96_000.0;

/// Filtrelerin toplam yanıtının en yüksek değeri (dB); 20 Hz–20 kHz taranır.
fn response_peak_db(filters: &[PeqFilter], method: DesignMethod) -> f64 {
    let coefs: Vec<Biquad> = filters
        .iter()
        .map(|f| design(f, DISPLAY_RATE, method))
        .collect();
    (0..=400)
        .map(|i| 20.0 * 1000f64.powf(f64::from(i) / 400.0))
        .map(|hz| {
            let w = 2.0 * PI * hz / DISPLAY_RATE;
            coefs.iter().map(|c| c.response_db(w)).sum::<f64>()
        })
        .fold(f64::NEG_INFINITY, f64::max)
}

/// Profildeki bir filtrenin katsayıları.
fn design(filter: &PeqFilter, sample_rate: f64, method: DesignMethod) -> Biquad {
    Biquad::design(&filter.spec(), sample_rate, method.method())
}

/// Bir filtre satırı okunamadığında.
enum FilterError {
    Unsupported,
    Bad,
}

/// Bant genişliğinden (oktav) Q: Q = 1 / (2·sinh(ln2/2 · BW)). (Equalizer APO bant genişliğini
/// sayısal alanda, örnekleme hızına bağlı tanımlar; fark yalnızca çok yüksek frekanslarda.)
fn q_from_bandwidth(octaves: f64) -> f64 {
    1.0 / (2.0 * (std::f64::consts::LN_2 / 2.0 * octaves).sinh())
}

/// Raf eğiminden (S) Q: 1/Q² = (A + 1/A)·(1/S − 1) + 2 (RBJ).
fn shelf_q_from_slope(slope: f64, a: f64) -> Option<f64> {
    let inverse_square = (a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0;
    (inverse_square > 1e-6 && inverse_square.is_finite()).then(|| 1.0 / inverse_square.sqrt())
}

/// Raf Q'sundan eğim (S): Equalizer APO'nun köşe frekansı dönüşümü için.
fn shelf_slope_from_q(q: f64, a: f64) -> f64 {
    1.0 / ((1.0 / (q * q) - 2.0) / (a + 1.0 / a) + 1.0)
}

/// "ON" sözcüğünden sonrası: tür, isteğe bağlı eğim ("6 dB", "6dB") ve parametreler.
fn parse_filter(kind_word: &str, params: &[&str]) -> Result<PeqFilter, FilterError> {
    let (kind, corner) = match kind_word.to_ascii_uppercase().as_str() {
        "PK" | "PEQ" | "MODAL" => (FilterKind::Peaking, false),
        "LS" => (FilterKind::LowShelf, true),
        "LSC" => (FilterKind::LowShelf, false),
        "HS" => (FilterKind::HighShelf, true),
        "HSC" => (FilterKind::HighShelf, false),
        "LP" | "LPQ" => (FilterKind::LowPass, false),
        "HP" | "HPQ" => (FilterKind::HighPass, false),
        "BP" => (FilterKind::BandPass, false),
        "NO" => (FilterKind::Notch, false),
        "AP" => (FilterKind::AllPass, false),
        _ => return Err(FilterError::Unsupported),
    };
    // Raf eğimi türün hemen ardından gelir: "LSC 10.8 dB …", "LS 6dB …".
    let mut params = params;
    let mut slope_db = None;
    if let Some(first) = params.first() {
        let lower = first.to_ascii_lowercase();
        if let Some(number) = lower.strip_suffix("db").and_then(parse_number) {
            slope_db = Some(number);
            params = params.get(1..).unwrap_or_default();
        } else if let (Some(number), Some(unit)) = (parse_number(first), params.get(1)) {
            if unit.eq_ignore_ascii_case("db") {
                slope_db = Some(number);
                params = params.get(2..).unwrap_or_default();
            }
        }
    }
    let after = |key: &str| {
        params
            .iter()
            .position(|w| w.eq_ignore_ascii_case(key))
            .map(|i| i + 1)
    };
    let number_at = |i: usize| params.get(i).and_then(|w| parse_number(w));
    let value = |key: &str| after(key).and_then(number_at);
    // "BW Oct 0.167" (APO) ya da "BW 0.167".
    let bandwidth = after("bw").and_then(|i| match params.get(i) {
        Some(w) if w.eq_ignore_ascii_case("oct") => number_at(i + 1),
        _ => number_at(i),
    });
    let freq_hz = value("fc").ok_or(FilterError::Bad)?;
    let gain_db = if kind.has_gain() {
        value("gain").ok_or(FilterError::Bad)?
    } else {
        0.0
    };
    let given_q = value("q");
    let mut filter = PeqFilter {
        kind,
        freq_hz,
        gain_db,
        q: DEFAULT_Q,
    };
    match kind {
        FilterKind::Peaking | FilterKind::AllPass => {
            filter.q = given_q
                .or(bandwidth.map(q_from_bandwidth))
                .ok_or(FilterError::Bad)?;
        }
        FilterKind::LowPass | FilterKind::HighPass | FilterKind::BandPass => {
            filter.q = given_q
                .or(bandwidth.map(q_from_bandwidth))
                .unwrap_or(DEFAULT_Q);
        }
        FilterKind::Notch => {
            filter.q = given_q
                .or(bandwidth.map(q_from_bandwidth))
                .unwrap_or(DEFAULT_NOTCH_Q);
        }
        FilterKind::LowShelf | FilterKind::HighShelf => {
            let a = 10f64.powf(gain_db / 40.0);
            let slope = match (given_q, slope_db) {
                (Some(q), _) => shelf_slope_from_q(q, a),
                (None, Some(db)) => db / 12.0,
                (None, None) => DEFAULT_SHELF_SLOPE,
            };
            filter.q = match given_q {
                Some(q) => q,
                None => shelf_q_from_slope(slope, a).ok_or(FilterError::Bad)?,
            };
            if corner {
                // Köşe frekansından orta frekansa (Equalizer APO, DCX2496 uyumu).
                let slope = if slope.is_finite() && slope > 0.0 {
                    slope
                } else {
                    DEFAULT_SHELF_SLOPE
                };
                let factor = 10f64.powf(gain_db.abs() / 80.0 / slope);
                filter.freq_hz = if kind == FilterKind::LowShelf {
                    freq_hz * factor
                } else {
                    freq_hz / factor
                };
            }
        }
    }
    Ok(filter)
}

/// Elle parametrik ekolayzerin sınırları.
pub const MAX_BANDS: usize = MAX_FILTERS;
pub const PARAMETRIC_MAX_GAIN_DB: f64 = 24.0;
pub const PARAMETRIC_MIN_Q: f64 = 0.1;
/// Eğrilerin çizildiği nokta sayısı (20 Hz – 20 kHz, logaritmik).
const CURVE_POINTS: usize = 241;

/// Elle parametrik ekolayzerin bir bandı.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ParametricBand {
    pub kind: FilterKind,
    pub freq_hz: f64,
    /// Kazanç (dB); kazanç kullanmayan türlerde yok sayılır.
    pub gain_db: f64,
    pub q: f64,
    /// Kapalı bant sese dokunmaz (ayarı durur).
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
}

fn enabled_by_default() -> bool {
    true
}

impl ParametricBand {
    fn filter(&self) -> PeqFilter {
        PeqFilter {
            kind: self.kind,
            freq_hz: self.freq_hz,
            gain_db: self.gain_db,
            q: self.q,
        }
    }

    /// Sese dokunuyor mu? (Açık ve kazanç kullanan türde kazancı 0 değil.)
    fn is_effective(&self) -> bool {
        self.enabled && (!self.kind.has_gain() || self.gain_db.abs() >= 0.005)
    }
}

/// Kalıcı elle parametrik ekolayzer ayarı.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ParametricSettings {
    pub enabled: bool,
    pub bands: Vec<ParametricBand>,
    /// Varsayılan: analog eğriye eşlenmiş (çizilen eğri duyulan eğridir).
    pub design: DesignMethod,
}

impl Default for ParametricSettings {
    /// Düz beş bant: alçak raf, üç tepe, yüksek raf (hemen sürüklenebilsin diye).
    fn default() -> Self {
        let band = |kind, freq_hz, q| ParametricBand {
            kind,
            freq_hz,
            gain_db: 0.0,
            q,
            enabled: true,
        };
        let shelf_q = std::f64::consts::FRAC_1_SQRT_2;
        Self {
            enabled: true,
            bands: vec![
                band(FilterKind::LowShelf, 105.0, shelf_q),
                band(FilterKind::Peaking, 250.0, 1.0),
                band(FilterKind::Peaking, 1000.0, 1.0),
                band(FilterKind::Peaking, 4000.0, 1.0),
                band(FilterKind::HighShelf, 10_000.0, shelf_q),
            ],
            design: DesignMethod::Matched,
        }
    }
}

impl ParametricSettings {
    pub fn sanitized(mut self) -> Self {
        self.bands.truncate(MAX_BANDS);
        for b in &mut self.bands {
            b.freq_hz = finite_or(b.freq_hz, 1000.0).clamp(MIN_FREQ, MAX_FREQ);
            b.gain_db = if b.kind.has_gain() {
                finite_or(b.gain_db, 0.0).clamp(-PARAMETRIC_MAX_GAIN_DB, PARAMETRIC_MAX_GAIN_DB)
            } else {
                0.0
            };
            b.q = finite_or(b.q, 1.0).clamp(PARAMETRIC_MIN_Q, MAX_Q);
        }
        self
    }

    /// Sese dokunan bantların filtreleri.
    pub fn filters(&self) -> Vec<PeqFilter> {
        self.bands
            .iter()
            .filter(|b| b.is_effective())
            .map(ParametricBand::filter)
            .collect()
    }

    /// Ses şu an değişiyor mu?
    pub fn is_active(&self) -> bool {
        self.enabled && self.bands.iter().any(ParametricBand::is_effective)
    }

    /// Ses yoluna giden filtreler (kapalıysa hiçbiri).
    fn active_filters(&self) -> Vec<PeqFilter> {
        if self.enabled {
            self.filters()
        } else {
            Vec::new()
        }
    }

    /// En büyük yükseltme (dB, ≥ 0): taşma koruması bunu karşılar.
    pub fn boost_db(&self) -> f64 {
        response_peak_db(&self.active_filters(), self.design).max(0.0)
    }

    /// Müziğe benzer seste ses yüksekliği farkı (dB; bkz. [`loudness_gain_db`]).
    pub fn loudness_db(&self) -> f64 {
        loudness_gain_db(&self.active_filters(), self.design)
    }

    /// Verilen frekanslarda toplam eğri (dB), verilen örnekleme hızında (görseller için).
    pub fn response_db(&self, freqs_hz: &[f64], sample_rate: f64) -> Vec<f64> {
        let coefs: Vec<Biquad> = self
            .active_filters()
            .iter()
            .map(|f| design(f, sample_rate, self.design))
            .collect();
        freqs_hz
            .iter()
            .map(|&hz| {
                let w = 2.0 * PI * hz / sample_rate;
                coefs.iter().map(|c| c.response_db(w)).sum::<f64>()
            })
            .collect()
    }

    /// Bir Equalizer APO dosyasından (AutoEq, REW, Equalizer APO): bütün filtreler bant olur;
    /// tasarım APO ile aynı (RBJ), ön kazanç yok sayılır (taşma koruması kendisi hesaplar).
    pub fn from_profile(profile: &HeadphoneProfile) -> Self {
        Self {
            enabled: true,
            bands: profile
                .filters
                .iter()
                .take(MAX_BANDS)
                .map(|f| ParametricBand {
                    kind: f.kind,
                    freq_hz: f.freq_hz,
                    gain_db: f.gain_db,
                    q: f.q,
                    enabled: true,
                })
                .collect(),
            design: DesignMethod::Rbj,
        }
        .sanitized()
    }

    /// Equalizer APO biçiminde metin (Equalizer APO, Peace, Wavelet ve REW okur). Ön kazanç en
    /// büyük yükseltme kadar düşürülür: başka programda da ses taşmasın.
    pub fn to_apo(&self) -> String {
        let boost = response_peak_db(&self.filters(), self.design).max(0.0);
        let mut text = String::from("# Lyraska parametrik ekolayzer (Equalizer APO biçimi)\n");
        if self.design == DesignMethod::Matched {
            text.push_str(
                "# Lyraska'da analog eğriye eşlenmiş tasarım: Equalizer APO'da 10 kHz üstü biraz farklı duyulabilir\n",
            );
        }
        text.push_str(&format!("Preamp: {:.2} dB\n", -boost));
        for (i, b) in self.bands.iter().enumerate() {
            let state = if self.enabled && b.enabled {
                "ON"
            } else {
                "OFF"
            };
            let (fc, g, q) = (b.freq_hz, b.gain_db, b.q);
            let body = match b.kind {
                FilterKind::Peaking => format!("PK Fc {fc:.2} Hz Gain {g:.2} dB Q {q:.4}"),
                FilterKind::LowShelf => format!("LSC Fc {fc:.2} Hz Gain {g:.2} dB Q {q:.4}"),
                FilterKind::HighShelf => format!("HSC Fc {fc:.2} Hz Gain {g:.2} dB Q {q:.4}"),
                FilterKind::LowPass => format!("LPQ Fc {fc:.2} Hz Q {q:.4}"),
                FilterKind::HighPass => format!("HPQ Fc {fc:.2} Hz Q {q:.4}"),
                FilterKind::BandPass => format!("BP Fc {fc:.2} Hz Q {q:.4}"),
                FilterKind::Notch => format!("NO Fc {fc:.2} Hz Q {q:.4}"),
                FilterKind::AllPass => format!("AP Fc {fc:.2} Hz Q {q:.4}"),
            };
            text.push_str(&format!("Filter {}: {state} {body}\n", i + 1));
        }
        text
    }
}

/// Arayüze giden durum: ayar, eğriler, taşma koruması ve karşılaştırma bilgisi.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct ParametricState {
    pub settings: ParametricSettings,
    /// Karşılaştırma (A/B) açık mı (ses ekolayzersiz ama eşit yükseklikte çalıyor).
    pub comparing: bool,
    /// Eğri frekansları (Hz); toplam eğri ve bant bant eğriler (dB).
    pub curve_hz: Vec<f64>,
    pub curve_db: Vec<f64>,
    pub band_curves_db: Vec<Vec<f64>>,
    /// En büyük yükseltme (dB, ≥ 0): şarkının boşluğu yetmezse ses bu kadar kısılır.
    pub boost_db: f64,
    /// Müziğe benzer seste (pembe gürültü, K ağırlıklı) ses yüksekliği farkı (dB); karşılaştırma
    /// bu kadar telafi edilir.
    pub loudness_db: f64,
    pub max_bands: usize,
    pub max_gain_db: f64,
    pub min_q: f64,
    pub max_q: f64,
}

impl ParametricState {
    pub fn new(settings: ParametricSettings, comparing: bool) -> Self {
        let settings = settings.sanitized();
        let curve_hz: Vec<f64> = (0..CURVE_POINTS)
            .map(|i| 20.0 * 1000f64.powf(i as f64 / (CURVE_POINTS - 1) as f64))
            .collect();
        let band_curves_db = settings
            .bands
            .iter()
            .map(|b| response_curve(&[b.filter()], settings.design, &curve_hz))
            .collect();
        let filters = settings.active_filters();
        let curve_db = response_curve(&filters, settings.design, &curve_hz);
        Self {
            boost_db: response_peak_db(&filters, settings.design).max(0.0),
            loudness_db: loudness_gain_db(&filters, settings.design),
            comparing: comparing && settings.is_active(),
            settings,
            curve_hz,
            curve_db,
            band_curves_db,
            max_bands: MAX_BANDS,
            max_gain_db: PARAMETRIC_MAX_GAIN_DB,
            min_q: PARAMETRIC_MIN_Q,
            max_q: MAX_Q,
        }
    }
}

/// Verilen frekanslarda filtrelerin toplam eğrisi (dB).
fn response_curve(filters: &[PeqFilter], method: DesignMethod, freqs_hz: &[f64]) -> Vec<f64> {
    let coefs: Vec<Biquad> = filters
        .iter()
        .map(|f| design(f, DISPLAY_RATE, method))
        .collect();
    freqs_hz
        .iter()
        .map(|&hz| {
            let w = 2.0 * PI * hz / DISPLAY_RATE;
            coefs.iter().map(|c| c.response_db(w)).sum::<f64>()
        })
        .collect()
}

/// Filtrelerin müziğe benzer bir seste (pembe gürültü: oktav başına eşit güç) ses yüksekliğini
/// ne kadar değiştirdiği (dB), ITU-R BS.1770'in K ağırlığıyla (insan kulağının duyarlılığına
/// yakın; EBU R128 ölçümünün kendisi). Karşılaştırma (A/B) bu kadar telafi edilir: ekolayzerli ve
/// ekolayzersiz ses eşit yükseklikte dinlenir, "yüksek olan daha iyi gelir" yanılgısı olmaz.
pub fn loudness_gain_db(filters: &[PeqFilter], method: DesignMethod) -> f64 {
    if filters.is_empty() {
        return 0.0;
    }
    // K ağırlığı: BS.1770'in iki aşaması (raf ve yüksek geçiren), analog karşılıkları.
    let shelf = Spec {
        kind: Kind::HighShelf,
        freq_hz: 1_681.974_450_955_533,
        gain_db: 3.999_843_853_973_347,
        q: 0.707_175_236_955_419_6,
    };
    let highpass = Spec {
        kind: Kind::HighPass,
        freq_hz: 38.135_470_876_024_44,
        gain_db: 0.0,
        q: 0.500_327_037_323_877_3,
    };
    let coefs: Vec<Biquad> = filters
        .iter()
        .map(|f| design(f, DISPLAY_RATE, method))
        .collect();
    let (mut weighted, mut total) = (0.0, 0.0);
    for i in 0..CURVE_POINTS {
        let hz = 20.0 * 1000f64.powf(i as f64 / (CURVE_POINTS - 1) as f64);
        let k = shelf.analog(hz).norm_sqr() * highpass.analog(hz).norm_sqr();
        let w = 2.0 * PI * hz / DISPLAY_RATE;
        let gain = 10f64.powf(coefs.iter().map(|c| c.response_db(w)).sum::<f64>() / 10.0);
        weighted += k * gain;
        total += k;
    }
    10.0 * (weighted / total).log10()
}

/// Arayüzden ses iş parçacığına profil taşıyan kilitsiz kanal (bkz. `EqControl`).
#[derive(Debug)]
pub struct PeqControl {
    version: AtomicU64,
    enabled: AtomicBool,
    count: AtomicUsize,
    preamp_db: AtomicU64,
    /// Eşlenmiş tasarım mı (değilse RBJ)?
    matched: AtomicBool,
    /// Filtrelerin en büyük yükseltmesi (dB, ≥ 0): ön kazançla karşılanmıyorsa taşma koruması
    /// ([`super::normalize::Leveler`]) bunu bilir.
    boost_db: AtomicU64,
    /// Karşılaştırma (A/B): filtreler atlanır, ses `compare_gain_db` kadar değişir (eşit ses
    /// yüksekliğinde dinlensin diye).
    compare: AtomicBool,
    compare_gain_db: AtomicU64,
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
            matched: AtomicBool::new(false),
            boost_db: AtomicU64::new(0f64.to_bits()),
            compare: AtomicBool::new(false),
            compare_gain_db: AtomicU64::new(0f64.to_bits()),
            filters: std::array::from_fn(|_| std::array::from_fn(|_| AtomicU64::new(0))),
        }
    }
}

impl PeqControl {
    /// Profili ve açık/kapalı durumunu uygular. Profil yoksa düzeltme kapalıdır.
    pub fn set(&self, profile: Option<&HeadphoneProfile>, enabled: bool) {
        match profile {
            Some(p) => self.set_filters(&p.filters, p.preamp_db, 0.0, p.design, enabled),
            None => self.set_filters(&[], 0.0, 0.0, DesignMethod::Rbj, false),
        }
    }

    /// Filtreleri, ön kazancı, taşma korumasına bildirilen yükseltmeyi ve yöntemi uygular.
    pub fn set_filters(
        &self,
        filters: &[PeqFilter],
        preamp_db: f64,
        boost_db: f64,
        design: DesignMethod,
        enabled: bool,
    ) {
        let filters = &filters[..filters.len().min(MAX_FILTERS)];
        for (slot, f) in self.filters.iter().zip(filters) {
            let kind = f.kind.code() as f64;
            for (cell, value) in slot.iter().zip([kind, f.freq_hz, f.gain_db, f.q]) {
                cell.store(value.to_bits(), Ordering::Relaxed);
            }
        }
        self.count.store(filters.len(), Ordering::Relaxed);
        self.matched
            .store(design == DesignMethod::Matched, Ordering::Relaxed);
        self.preamp_db.store(preamp_db.to_bits(), Ordering::Relaxed);
        let enabled = enabled && !filters.is_empty();
        self.boost_db.store(
            if enabled { boost_db.max(0.0) } else { 0.0 }.to_bits(),
            Ordering::Relaxed,
        );
        self.enabled.store(enabled, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Karşılaştırma (A/B) açık mı; açıksa ses `gain_db` kadar değişir.
    pub fn set_compare(&self, compare: bool, gain_db: f64) {
        self.compare_gain_db
            .store(finite_or(gain_db, 0.0).to_bits(), Ordering::Relaxed);
        self.compare.store(compare, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
    }

    /// Taşma korumasına bildirilen en büyük yükseltme (dB, ≥ 0).
    pub fn boost_db(&self) -> f64 {
        f64::from_bits(self.boost_db.load(Ordering::Relaxed))
    }

    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }

    /// Ses iş parçacığı için katsayılar (ayırma yok).
    fn design(&self, sample_rate: f64, bank: &mut Bank) {
        let enabled = self.enabled.load(Ordering::Relaxed);
        if enabled && self.compare.load(Ordering::Relaxed) {
            // Karşılaştırma: filtresiz, eşit ses yüksekliğinde.
            bank.coefs = [Biquad::IDENTITY; MAX_FILTERS];
            bank.count = 0;
            bank.gain =
                10f64.powf(f64::from_bits(self.compare_gain_db.load(Ordering::Relaxed)) / 20.0);
            return;
        }
        let count = if enabled {
            self.count.load(Ordering::Relaxed).min(MAX_FILTERS)
        } else {
            0
        };
        let method = if self.matched.load(Ordering::Relaxed) {
            DesignMethod::Matched
        } else {
            DesignMethod::Rbj
        };
        for (i, coef) in bank.coefs.iter_mut().enumerate() {
            *coef = if i < count {
                let v = |k: usize| f64::from_bits(self.filters[i][k].load(Ordering::Relaxed));
                design(
                    &PeqFilter {
                        kind: FilterKind::from_code(v(0) as usize),
                        freq_hz: v(1),
                        gain_db: v(2),
                        q: v(3),
                    },
                    sample_rate,
                    method,
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
            y = f.process(s, y);
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
        // Sürüm tasarımdan önce okunur: profil tam arada değişirse ilk turda yeniden tasarlanır
        // (sonra okunsaydı değişiklik bu oturum boyunca uygulanmazdı; `EqProcessor` da böyle).
        let seen_version = control.version();
        let mut current = Bank::new(channels);
        control.design(rate, &mut current);
        Self {
            seen_version,
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

    /// Taşma korumasının hesaba katacağı yükseltme (dB, ≥ 0). Değişince koruma 10 ms'de
    /// iner, filtreler 30 ms'de geçer: yükseltme korumadan önce gelmez.
    pub fn boost_db(&self) -> f64 {
        self.control.boost_db()
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
            bank.state.iter_mut().flatten().for_each(biquad::flush);
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
            HeadphoneProfile::parse("Filter 1: ON IIR Order 2 Coefficients 1 0 0 1 0 0\n", "x"),
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
    fn virgullu_sayi_kabul_edilir() {
        let profile =
            HeadphoneProfile::parse("\u{feff}Filter: ON LSC Fc 100 Hz Gain 3,5 dB Q 0,7\n", "x")
                .unwrap();
        assert!((profile.filters[0].gain_db - 3.5).abs() < 1e-9);
        assert!((profile.filters[0].q - 0.7).abs() < 1e-9);
    }

    /// Equalizer APO'da Q'su yazılmamış raf, eğimi S = 0,9 olan raftır (Q = 0,707 değil):
    /// 1/Q² = (A + 1/A)·(1/S − 1) + 2, A = 10^(kazanç/40).
    fn q_from_slope(gain_db: f64, slope: f64) -> f64 {
        let a = 10f64.powf(gain_db / 40.0);
        1.0 / ((a + 1.0 / a) * (1.0 / slope - 1.0) + 2.0).sqrt()
    }

    #[test]
    fn apo_q_suz_raf_egimi_0_9_sayilir() {
        let profile = HeadphoneProfile::parse("Filter: ON LSC Fc 100 Hz Gain 6 dB\n", "x").unwrap();
        let f = profile.filters[0];
        assert!((f.freq_hz - 100.0).abs() < 1e-9);
        assert!((f.q - q_from_slope(6.0, 0.9)).abs() < 1e-9, "{}", f.q);
    }

    /// "LS" ve "HS" (sonunda C yok) Equalizer APO'da köşe frekanslıdır: orta frekans
    /// köşe × 10^(|kazanç|/(80·S)) (alçak rafta; yüksek rafta bölü).
    #[test]
    fn apo_ls_hs_kose_frekansini_ortaya_cevirir() {
        let profile = HeadphoneProfile::parse(
            "Filter 1: ON LS Fc 100 Hz Gain 6 dB\nFilter 2: ON HS Fc 8000 Hz Gain -6 dB Q 0.7\n",
            "x",
        )
        .unwrap();
        let low = profile.filters[0];
        assert!(
            (low.freq_hz - 100.0 * 10f64.powf(6.0 / 72.0)).abs() < 1e-6,
            "{}",
            low.freq_hz
        );
        assert!((low.q - q_from_slope(6.0, 0.9)).abs() < 1e-9);
        let high = profile.filters[1];
        let a = 10f64.powf(-6.0 / 40.0);
        let slope = 1.0 / ((1.0 / (0.7 * 0.7) - 2.0) / (a + 1.0 / a) + 1.0);
        let expected = 8000.0 / 10f64.powf(6.0 / 80.0 / slope);
        assert!(
            (high.freq_hz - expected).abs() < 1e-6,
            "{} != {expected}",
            high.freq_hz
        );
        assert!((high.q - 0.7).abs() < 1e-9);
    }

    #[test]
    fn apo_egim_ve_bant_genisligi_okunur() {
        let profile = HeadphoneProfile::parse(
            "Filter: ON LSC 6 dB Fc 300 Hz Gain 5 dB\n\
             Filter: ON LS 12dB Fc 2000 Hz Gain -5.0 dB\n\
             Filter: ON PK Fc 1000 Hz Gain 3 dB BW Oct 1\n\
             Filter: ON PEQ Fc 100 Hz Gain 1.0 dB BW Oct 0.5\n",
            "x",
        )
        .unwrap();
        let [six, twelve, bw, peq] = [0, 1, 2, 3].map(|i| profile.filters[i]);
        assert!((six.q - q_from_slope(5.0, 0.5)).abs() < 1e-9, "{}", six.q);
        assert!((six.freq_hz - 300.0).abs() < 1e-9);
        assert!((twelve.q - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-9);
        assert!((twelve.freq_hz - 2000.0 * 10f64.powf(5.0 / 80.0)).abs() < 1e-6);
        // Q = 1 / (2·sinh(ln2/2 · BW)): 1 oktavda √2.
        assert!((bw.q - std::f64::consts::SQRT_2).abs() < 1e-9, "{}", bw.q);
        let expected = 1.0 / (2.0 * (std::f64::consts::LN_2 / 2.0 * 0.5).sinh());
        assert!((peq.q - expected).abs() < 1e-9);
    }

    #[test]
    fn apo_diger_suzgec_turleri_okunur() {
        let profile = HeadphoneProfile::parse(
            "Filter: ON LP Fc 8000 Hz\n\
             Filter: ON LPQ Fc 10000 Hz Q 0.400\n\
             Filter: ON HP Fc 30.0 Hz\n\
             Filter: ON HPQ Fc 20.0 Hz Q 0.500\n\
             Filter: ON BP Fc 1000 Hz Q 0.100\n\
             Filter: ON NO Fc 800 Hz\n\
             Filter: ON AP Fc 900 Hz Q 0.707\n\
             Filter: ON Modal Fc 100 Hz Gain 3.0 dB Q 5.41 T60 target 100 ms\n",
            "x",
        )
        .unwrap();
        let kinds: Vec<FilterKind> = profile.filters.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            [
                FilterKind::LowPass,
                FilterKind::LowPass,
                FilterKind::HighPass,
                FilterKind::HighPass,
                FilterKind::BandPass,
                FilterKind::Notch,
                FilterKind::AllPass,
                FilterKind::Peaking,
            ]
        );
        let q: Vec<f64> = profile.filters.iter().map(|f| f.q).collect();
        let r = std::f64::consts::FRAC_1_SQRT_2;
        for (got, want) in q.iter().zip([r, 0.4, r, 0.5, 0.1, 30.0, 0.707, 5.41]) {
            assert!((got - want).abs() < 1e-9, "{q:?}");
        }
        // Kazanç kullanmayan türlerde kazanç 0; tepe süzgecinde Q zorunlu.
        assert!(profile.filters[..7].iter().all(|f| f.gain_db == 0.0));
        assert!(matches!(
            HeadphoneProfile::parse("Filter: ON PK Fc 100 Hz Gain 3 dB\n", "x"),
            Err(ProfileError::BadLine { .. })
        ));
        assert!(matches!(
            HeadphoneProfile::parse("Filter: ON AP Fc 100 Hz\n", "x"),
            Err(ProfileError::BadLine { .. })
        ));
    }

    #[test]
    fn filtreler_istenen_egriyi_verir() {
        let pk = HeadphoneProfile {
            name: "pk".into(),
            design: Default::default(),
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
            design: Default::default(),
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
            design: Default::default(),
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

    fn band(kind: FilterKind, freq_hz: f64, gain_db: f64, q: f64) -> ParametricBand {
        ParametricBand {
            kind,
            freq_hz,
            gain_db,
            q,
            enabled: true,
        }
    }

    #[test]
    fn parametrik_varsayilan_duz_ve_sese_dokunmaz() {
        let settings = ParametricSettings::default();
        assert_eq!(settings.bands.len(), 5);
        assert!(settings.filters().is_empty(), "düz bantlar filtre değil");
        assert!(!settings.is_active());
        let state = ParametricState::new(settings, true);
        assert!(state.curve_db.iter().all(|&db| db.abs() < 1e-9));
        assert_eq!(state.boost_db, 0.0);
        assert_eq!(state.loudness_db, 0.0);
        assert!(
            !state.comparing,
            "düz ekolayzerde karşılaştırılacak bir şey yok"
        );
        assert_eq!(state.band_curves_db.len(), 5);
        let json = serde_json::to_value(&state).unwrap();
        assert_eq!(json["settings"]["design"], "matched");
        assert_eq!(json["settings"]["bands"][0]["kind"], "lowShelf");
        assert!(json["bandCurvesDb"].is_array());
    }

    #[test]
    fn parametrik_ayar_sinirlara_cekilir_ve_eski_dosya_okunur() {
        let wild = ParametricSettings {
            enabled: true,
            bands: (0..30)
                .map(|_| band(FilterKind::Peaking, f64::NAN, 99.0, 0.0))
                .collect(),
            design: DesignMethod::Matched,
        }
        .sanitized();
        assert_eq!(wild.bands.len(), MAX_BANDS);
        assert_eq!(wild.bands[0].freq_hz, 1000.0);
        assert_eq!(wild.bands[0].gain_db, PARAMETRIC_MAX_GAIN_DB);
        assert_eq!(wild.bands[0].q, PARAMETRIC_MIN_Q);
        // Kazanç kullanmayan türde kazanç sıfırlanır.
        let lp = ParametricSettings {
            bands: vec![band(FilterKind::LowPass, 8000.0, 5.0, 0.7)],
            ..ParametricSettings::default()
        }
        .sanitized();
        assert_eq!(lp.bands[0].gain_db, 0.0);
        // "enabled" yazılmamış bant açık sayılır; eksik alanlar varsayılanla dolar.
        let old: ParametricSettings =
            serde_json::from_str(r#"{"bands":[{"kind":"peaking","freqHz":500,"gainDb":2,"q":1}]}"#)
                .unwrap();
        assert!(old.enabled && old.bands[0].enabled);
        assert_eq!(old.design, DesignMethod::Matched);
    }

    /// Equalizer APO'ya yazılan dosya aynı bantları geri verir (kapalı bantlar OFF).
    #[test]
    fn parametrik_apo_ile_gidip_gelir() {
        let settings = ParametricSettings {
            enabled: true,
            bands: vec![
                band(FilterKind::LowShelf, 105.5, 4.5, 0.71),
                band(FilterKind::Peaking, 1923.0, -3.25, 1.31),
                band(FilterKind::HighShelf, 9000.0, 2.0, 0.7),
                band(FilterKind::HighPass, 25.0, 0.0, 0.54),
                band(FilterKind::LowPass, 18_000.0, 0.0, 0.707),
                band(FilterKind::BandPass, 3000.0, 0.0, 2.0),
                band(FilterKind::Notch, 60.0, 0.0, 30.0),
                band(FilterKind::AllPass, 900.0, 0.0, 0.707),
            ],
            design: DesignMethod::Rbj,
        };
        let text = settings.to_apo();
        assert!(text.contains("Preamp: -"), "{text}");
        let back = ParametricSettings::from_profile(&HeadphoneProfile::parse(&text, "").unwrap());
        assert_eq!(back.bands.len(), settings.bands.len());
        for (a, b) in settings.bands.iter().zip(&back.bands) {
            assert_eq!(a.kind, b.kind);
            assert!((a.freq_hz - b.freq_hz).abs() < 0.006, "{a:?} {b:?}");
            assert!((a.gain_db - b.gain_db).abs() < 0.006);
            assert!((a.q - b.q).abs() < 1e-4);
        }
        // Kapalı bant APO'da da kapalı (okuyucu atlar).
        let mut off = settings.clone();
        off.bands[1].enabled = false;
        let text = off.to_apo();
        assert!(text.contains("Filter 2: OFF PK"));
        let back = ParametricSettings::from_profile(&HeadphoneProfile::parse(&text, "").unwrap());
        assert_eq!(back.bands.len(), settings.bands.len() - 1);
    }

    /// Karşılaştırma telafisi, müziğe benzer seste (pembe gürültü, K ağırlıklı) ses yüksekliği
    /// farkıdır: bütün duyulur bandı 6 dB yükselten ayar ~6 dB, dar bir tepe çok daha az.
    #[test]
    fn ses_yuksekligi_telafisi_algiya_gore() {
        let all = [PeqFilter {
            kind: FilterKind::LowShelf,
            freq_hz: 24_000.0,
            gain_db: 6.0,
            q: 0.707,
        }];
        let broad = loudness_gain_db(&all, DesignMethod::Matched);
        assert!(broad > 5.0 && broad < 6.01, "{broad}");
        let narrow = [PeqFilter {
            kind: FilterKind::Peaking,
            freq_hz: 3000.0,
            gain_db: 6.0,
            q: 4.0,
        }];
        let small = loudness_gain_db(&narrow, DesignMethod::Matched);
        assert!(small > 0.1 && small < 1.5, "{small}");
        // Derin bas K ağırlığında az sayılır (kulak alçak frekansta daha az duyarlı).
        let sub = [PeqFilter {
            kind: FilterKind::LowShelf,
            freq_hz: 40.0,
            gain_db: 6.0,
            q: 0.707,
        }];
        let bass = loudness_gain_db(&sub, DesignMethod::Matched);
        assert!(bass < small, "{bass} {small}");
        assert_eq!(loudness_gain_db(&[], DesignMethod::Matched), 0.0);
    }

    /// Karşılaştırmada filtreler atlanır, ses yalnızca telafi kazancıyla çarpılır; taşma
    /// korumasına bildirilen yükseltme değişmez (iki taraf aynı korumayla dinlenir).
    #[test]
    fn karsilastirma_filtresiz_ve_esit_yukseklikte() {
        let rate = 48_000;
        let control = Arc::new(PeqControl::default());
        let filters = [PeqFilter {
            kind: FilterKind::Peaking,
            freq_hz: 1000.0,
            gain_db: 6.0,
            q: 1.0,
        }];
        control.set_filters(&filters, 0.0, 6.0, DesignMethod::Matched, true);
        let mut processor = PeqProcessor::new(Arc::clone(&control), 2, rate);
        assert!(!processor.is_bypass());
        assert_eq!(processor.boost_db(), 6.0);
        control.set_compare(true, 2.0);
        measure(&mut processor, 1000.0, rate); // geçiş biter
        let gain = 10f64.powf(2.0 / 20.0);
        let mut frame = [0.25, -0.5];
        processor.process_frame(&mut frame);
        assert_eq!(frame, [0.25 * gain, -0.5 * gain]);
        assert_eq!(processor.boost_db(), 6.0);
        // Kapatınca filtreler döner.
        control.set_compare(false, 2.0);
        let measured = measure(&mut processor, 1000.0, rate);
        assert!((measured - 6.0).abs() < 0.05, "{measured}");
        // Ekolayzer kapalıysa bildirilen yükseltme yok.
        control.set_filters(&filters, 0.0, 6.0, DesignMethod::Matched, false);
        assert_eq!(processor.boost_db(), 0.0);
    }

    #[test]
    fn parametrik_egri_tasarlanan_sesle_ayni() {
        let settings = ParametricSettings {
            enabled: true,
            bands: vec![
                band(FilterKind::Peaking, 120.0, 5.0, 0.9),
                band(FilterKind::HighShelf, 12_000.0, -4.0, 0.7),
            ],
            design: DesignMethod::Matched,
        };
        let state = ParametricState::new(settings.clone(), false);
        // Toplam eğri bantların toplamı; en büyük yükseltme ~5 dB.
        for (i, total) in state.curve_db.iter().enumerate() {
            let sum: f64 = state.band_curves_db.iter().map(|c| c[i]).sum();
            assert!((total - sum).abs() < 1e-9);
        }
        assert!((state.boost_db - 5.0).abs() < 0.1, "{}", state.boost_db);
        // İşlemci aynı eğriyi çalar (48 kHz).
        let control = Arc::new(PeqControl::default());
        control.set_filters(
            &settings.filters(),
            0.0,
            state.boost_db,
            settings.design,
            true,
        );
        let mut processor = PeqProcessor::new(Arc::clone(&control), 2, 48_000);
        for hz in [120.0, 1000.0, 12_000.0] {
            let measured = measure(&mut processor, hz, 48_000);
            let expected = settings.response_db(&[hz], 48_000.0)[0];
            assert!(
                (measured - expected).abs() < 0.05,
                "{hz}: {measured} != {expected}"
            );
        }
        // Kapalıyken eğri düz, yükseltme yok.
        let off = ParametricState::new(
            ParametricSettings {
                enabled: false,
                ..settings
            },
            false,
        );
        assert!(off.curve_db.iter().all(|&db| db.abs() < 1e-9));
        assert_eq!(off.boost_db, 0.0);
    }
}
