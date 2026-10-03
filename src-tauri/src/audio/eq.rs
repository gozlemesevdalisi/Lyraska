//! 10 bantlı grafik ekolayzer.
//!
//! Sıradan grafik ekolayzerlerde yan yana bantlar birbirine taşar: bütün
//! sürgüler +6 dB'deyken ses +6 değil +9–10 dB yükselir, zikzak ayarlar ise
//! hiç istenmeyen bir eğri çizer. Lyraska'da sürgülerin konumu gerçekten
//! duyulan eğridir. Bunun için bant filtrelerinin kazançları en küçük kareler
//! yöntemiyle, birbirlerine taşmaları hesaba katılarak bulunur (Välimäki ve
//! Liski, "Accurate Cascade Graphic Equalizer", IEEE SPL 2017).
//!
//! Tasarım iki adımlıdır:
//! 1. Her filtrenin tasarım frekanslarındaki (bant ortaları ve aralarındaki
//!    noktalar) tepkisi bir "etkileşim matrisi"ne yazılır; hedef eğriye en
//!    yakın sonucu veren filtre kazançları çözülür.
//! 2. Filtre biçimi kazançla değiştiği için matris bulunan kazançlarla yeniden
//!    kurulur ve bir kez daha çözülür.
//!
//! Sonuç, ±12 dB içindeki her ayarda ve her örnekleme hızında tasarım
//! noktalarında en fazla ~1 dB, yumuşak eğrilerde çok daha az sapar (testlerde
//! ölçülür). Bütün hesap sabit boyutlu dizilerle yapılır: bellek ayırmaz, bu
//! yüzden gerçek zamanlı ses iş parçacığında da çalışabilir.
//!
//! Ses işleme [`EqProcessor`] ile yapılır. Ayarlar [`EqControl`] üzerinden
//! kilitsiz (atomik) olarak iletilir; değişiklikler ~40 ms içinde yumuşakça
//! uygulanır ("cızırtı" olmasın). Ekolayzer düzken ses hiç işlenmez.
//! Yükseltilen bantlar sesi kırpılmaya (bozulmaya) götürmesin diye en yüksek
//! sürgü kadar ön kazanç düşürülür.

// Matris ve filtre hesaplarında indisli döngüler formüllerle bire bir eşleşir.
#![allow(clippy::needless_range_loop)]

use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use super::Sample;

/// Bant sayısı.
pub const BANDS: usize = 10;
/// Bant orta frekansları (Hz): 1 kHz'ten oktav aralıklı.
pub const CENTERS_HZ: [f64; BANDS] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];
/// Sürgülerin aralığı (±dB).
pub const MAX_GAIN_DB: f64 = 12.0;

/// Tasarım noktası sayısı üst sınırı: bant ortaları + aralarındaki noktalar.
const MAX_POINTS: usize = 2 * BANDS - 1;
/// Orta frekansı örnekleme hızının bu oranından yüksek bantlar kullanılmaz
/// (ör. 22,05 kHz'lik bir dosyada 16 kHz bandı duyulabilir aralığın dışındadır).
const MAX_CENTER_RATIO: f64 = 0.4;
/// İkinci adımda biçim hesaplanırken en küçük kazanç (0'a bölünmesin diye).
const MIN_SHAPE_GAIN_DB: f64 = 0.5;
/// Bu kadar küçük kazançlar "düz" sayılır.
const FLAT_DB: f64 = 1e-3;
/// Ayar değişikliklerinin yumuşatılması: her 2 ms'de bir yeniden tasarlanır,
/// hedefe ~40 ms'lik zaman sabitiyle yaklaşılır.
const UPDATE_SECONDS: f64 = 0.002;
const SMOOTH_SECONDS: f64 = 0.040;
/// Hedefe bu kadar yaklaşınca doğrudan hedefe oturulur (dB).
const SNAP_DB: f64 = 0.01;
/// Arayüzdeki eğri bu örnekleme hızıyla ve bu kadar noktayla çizilir.
const DISPLAY_RATE: f64 = 48_000.0;
const CURVE_POINTS: usize = 96;

/// Filtre biçimini belirleyen değerler. Bütün örnekleme hızlarında ve çok sayıda
/// ayarda en büyük sapmayı en aza indirecek şekilde ölçülerek seçildi
/// (`ayar_taramasi` testi).
#[derive(Debug, Clone, Copy)]
struct Tuning {
    /// Bant genişliği / orta frekans.
    bandwidth_ratio: f64,
    /// Bant kenarındaki kazancın tepe kazancına oranı (dB cinsinden).
    edge_ratio: f64,
    /// Nyquist'e yakın bantlarda bant genişliği sınırı: `limit · (π − w0)`.
    limit: f64,
    /// İlk adımda filtre biçimini belirlemek için kullanılan örnek kazanç (dB).
    prototype_db: f64,
    /// Tasarım adımı sayısı.
    passes: usize,
}

const TUNING: Tuning = Tuning {
    bandwidth_ratio: 1.0,
    edge_ratio: 0.4,
    limit: 1.8,
    prototype_db: 12.0,
    passes: 2,
};

/// Kullanıcının ekolayzer ayarları. Ayarlar dosyasında da bu biçimde saklanır.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct EqSettings {
    pub enabled: bool,
    pub gains_db: [f64; BANDS],
}

impl Default for EqSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            gains_db: [0.0; BANDS],
        }
    }
}

impl EqSettings {
    /// Geçersiz değerleri düzeltir: kazançlar ±12 dB'ye kırpılır, sayı olmayanlar 0 olur.
    pub fn sanitized(mut self) -> Self {
        for gain in &mut self.gains_db {
            *gain = if gain.is_finite() {
                gain.clamp(-MAX_GAIN_DB, MAX_GAIN_DB)
            } else {
                0.0
            };
        }
        self
    }

    /// Sesi gerçekten değiştiriyor mu? (Kapalı ya da bütün sürgüler 0'da ise hayır.)
    pub fn is_active(&self) -> bool {
        self.enabled && self.gains_db.iter().any(|g| g.abs() > FLAT_DB)
    }

    /// Ses işlemede hedeflenen kazançlar: kapalıyken düz.
    fn effective_gains(&self) -> [f64; BANDS] {
        if self.enabled {
            self.gains_db
        } else {
            [0.0; BANDS]
        }
    }
}

/// Arayüzün gösterdiği ekolayzer durumu: ayarlar ve gerçekten uygulanan eğri.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EqState {
    pub enabled: bool,
    pub gains_db: [f64; BANDS],
    pub bands_hz: [f64; BANDS],
    pub max_gain_db: f64,
    /// Kırpılmayı önlemek için düşürülen kazanç (dB, ≤ 0).
    pub preamp_db: f64,
    /// Filtrelerin toplam tepkisi (ön kazanç hariç): frekanslar (Hz) ve kazançlar (dB).
    pub curve_hz: Vec<f64>,
    pub curve_db: Vec<f64>,
}

impl EqState {
    pub fn new(settings: EqSettings) -> Self {
        let design = Design::new(&settings.effective_gains(), DISPLAY_RATE);
        let curve_hz: Vec<f64> = (0..CURVE_POINTS)
            .map(|i| 20.0 * 1000f64.powf(i as f64 / (CURVE_POINTS - 1) as f64))
            .collect();
        let curve_db = curve_hz
            .iter()
            .map(|&hz| design.response_db(hz, DISPLAY_RATE))
            .collect();
        Self {
            enabled: settings.enabled,
            gains_db: settings.gains_db,
            bands_hz: CENTERS_HZ,
            max_gain_db: MAX_GAIN_DB,
            preamp_db: design.preamp_db,
            curve_hz,
            curve_db,
        }
    }
}

/// Arayüzden ses iş parçacığına ayar taşıyan kilitsiz kanal.
///
/// Yazan önce değerleri, sonra sürüm sayacını günceller; okuyan sürüm
/// değiştiyse değerleri yeniden okur. Arada bir yarım okuma olsa bile sürüm
/// bir kez daha değiştiği için bir sonraki turda düzelir.
#[derive(Debug)]
pub struct EqControl {
    version: AtomicU64,
    enabled: AtomicBool,
    gains: [AtomicU64; BANDS],
}

impl Default for EqControl {
    fn default() -> Self {
        Self::new(EqSettings::default())
    }
}

impl EqControl {
    pub fn new(settings: EqSettings) -> Self {
        let control = Self {
            version: AtomicU64::new(0),
            enabled: AtomicBool::new(false),
            gains: std::array::from_fn(|_| AtomicU64::new(0f64.to_bits())),
        };
        control.set(settings);
        control
    }

    /// Yeni ayarları uygular (düzeltilmiş halini döndürür).
    pub fn set(&self, settings: EqSettings) -> EqSettings {
        let settings = settings.sanitized();
        for (slot, gain) in self.gains.iter().zip(settings.gains_db) {
            slot.store(gain.to_bits(), Ordering::Relaxed);
        }
        self.enabled.store(settings.enabled, Ordering::Relaxed);
        self.version.fetch_add(1, Ordering::Release);
        settings
    }

    pub fn settings(&self) -> EqSettings {
        EqSettings {
            enabled: self.enabled.load(Ordering::Relaxed),
            gains_db: std::array::from_fn(|i| {
                f64::from_bits(self.gains[i].load(Ordering::Relaxed))
            }),
        }
    }

    /// Ayarlar her değiştiğinde artan sayaç.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Acquire)
    }
}

/// İkinci dereceden IIR filtre ("biquad") katsayıları; a0 = 1'e normalize.
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

    /// Tepe (peaking) filtresi. `w0` orta frekans, `bandwidth` bant genişliği
    /// (ikisi de radyan/örnek). Bant kenarlarında kazanç, tepe kazancının
    /// (dB cinsinden) `edge_ratio` katıdır.
    fn peaking(w0: f64, bandwidth: f64, gain_db: f64, edge_ratio: f64) -> Self {
        if gain_db.abs() < FLAT_DB {
            return Self::IDENTITY;
        }
        let g = db_to_linear(gain_db);
        let gb = db_to_linear(edge_ratio * gain_db);
        let beta =
            (bandwidth / 2.0).tan() * ((gb * gb - 1.0).abs() / (g * g - gb * gb).abs()).sqrt();
        let norm = 1.0 / (1.0 + beta);
        let c = -2.0 * w0.cos() * norm;
        Self {
            b0: (1.0 + g * beta) * norm,
            b1: c,
            b2: (1.0 - g * beta) * norm,
            a1: c,
            a2: (1.0 - beta) * norm,
        }
    }

    /// `w` frekansındaki kazanç (dB). `cos_w` = cos(w), `cos_2w` = cos(2w).
    fn response_db(&self, cos_w: f64, cos_2w: f64) -> f64 {
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

fn db_to_linear(db: f64) -> f64 {
    10f64.powf(db / 20.0)
}

/// Bir örnekleme hızı için sabit olan tasarım bilgileri.
#[derive(Debug, Clone, Copy)]
struct Layout {
    /// Kullanılan bant sayısı (yüksek bantlar Nyquist'e yakınsa düşer).
    bands: usize,
    /// Bantların orta frekansı ve bant genişliği (radyan/örnek).
    w0: [f64; BANDS],
    bandwidth: [f64; BANDS],
    /// Tasarım noktaları: cos(w) ve cos(2w).
    points: usize,
    cos_w: [f64; MAX_POINTS],
    cos_2w: [f64; MAX_POINTS],
    tuning: Tuning,
}

impl Layout {
    fn new(sample_rate: f64) -> Self {
        Self::tuned(sample_rate, TUNING)
    }

    fn tuned(sample_rate: f64, tuning: Tuning) -> Self {
        let bands = CENTERS_HZ
            .iter()
            .take_while(|&&f| f < MAX_CENTER_RATIO * sample_rate)
            .count();
        let mut layout = Self {
            bands,
            w0: [0.0; BANDS],
            bandwidth: [0.0; BANDS],
            points: (2 * bands).saturating_sub(1),
            cos_w: [0.0; MAX_POINTS],
            cos_2w: [0.0; MAX_POINTS],
            tuning,
        };
        for m in 0..bands {
            let w0 = 2.0 * PI * CENTERS_HZ[m] / sample_rate;
            layout.w0[m] = w0;
            // Nyquist'e yakın bantlar daraltılır: filtre π'yi aşamaz.
            layout.bandwidth[m] = (tuning.bandwidth_ratio * w0).min(tuning.limit * (PI - w0));
        }
        for k in 0..layout.points {
            let w = 2.0 * PI * design_point_hz(k) / sample_rate;
            layout.cos_w[k] = w.cos();
            layout.cos_2w[k] = (2.0 * w).cos();
        }
        layout
    }
}

/// k. tasarım noktasının frekansı: çift sıradakiler bant ortaları, tek
/// sıradakiler iki komşu bandın geometrik ortası.
fn design_point_hz(k: usize) -> f64 {
    if k % 2 == 0 {
        CENTERS_HZ[k / 2]
    } else {
        (CENTERS_HZ[k / 2] * CENTERS_HZ[k / 2 + 1]).sqrt()
    }
}

/// k. tasarım noktasındaki hedef: bant ortasında sürgü, arada komşuların ortalaması.
fn design_target(gains_db: &[f64; BANDS], k: usize) -> f64 {
    if k % 2 == 0 {
        gains_db[k / 2]
    } else {
        0.5 * (gains_db[k / 2] + gains_db[k / 2 + 1])
    }
}

/// Tasarlanmış ekolayzer: bant filtreleri ve kırpılmayı önleyen ön kazanç.
#[derive(Debug, Clone, Copy)]
pub struct Design {
    filters: [Biquad; BANDS],
    bands: usize,
    /// Kırpılmayı önlemek için uygulanan kazanç (dB, ≤ 0): en yüksek sürgü kadar.
    pub preamp_db: f64,
}

impl Design {
    /// Sürgü konumlarından filtreleri tasarlar.
    pub fn new(gains_db: &[f64; BANDS], sample_rate: f64) -> Self {
        Self::with_layout(gains_db, &Layout::new(sample_rate))
    }

    fn with_layout(gains_db: &[f64; BANDS], layout: &Layout) -> Self {
        let n = layout.bands;
        let mut design = Self {
            filters: [Biquad::IDENTITY; BANDS],
            bands: n,
            preamp_db: -gains_db.iter().take(n).fold(0.0f64, |max, &g| max.max(g)),
        };
        if gains_db.iter().take(n).all(|g| g.abs() < FLAT_DB) {
            return design;
        }

        let mut target = [0.0; MAX_POINTS];
        for (k, t) in target.iter_mut().enumerate().take(layout.points) {
            *t = design_target(gains_db, k);
        }

        let mut gains = solve_gains(layout, &[layout.tuning.prototype_db; BANDS], &target);
        for _ in 1..layout.tuning.passes {
            let mut shape = [0.0; BANDS];
            for (s, &g) in shape.iter_mut().zip(&gains).take(n) {
                *s = if g.abs() < MIN_SHAPE_GAIN_DB {
                    MIN_SHAPE_GAIN_DB.copysign(g)
                } else {
                    g
                };
            }
            gains = solve_gains(layout, &shape, &target);
        }
        for m in 0..n {
            design.filters[m] = Biquad::peaking(
                layout.w0[m],
                layout.bandwidth[m],
                gains[m],
                layout.tuning.edge_ratio,
            );
        }
        design
    }

    /// Düz mü (ses işlenmeden geçirilebilir mi)?
    pub fn is_flat(&self) -> bool {
        self.filters.iter().all(|f| *f == Biquad::IDENTITY) && self.preamp_db.abs() < FLAT_DB
    }

    /// Verilen frekanstaki toplam kazanç (dB), ön kazanç hariç.
    pub fn response_db(&self, hz: f64, sample_rate: f64) -> f64 {
        let w = 2.0 * PI * hz / sample_rate;
        let (cos_w, cos_2w) = (w.cos(), (2.0 * w).cos());
        self.filters
            .iter()
            .take(self.bands)
            .map(|f| f.response_db(cos_w, cos_2w))
            .sum()
    }
}

/// Filtre biçimleri `shape` kazançlarıyla hesaplanmış etkileşim matrisini kurar
/// ve hedef eğriye en yakın filtre kazançlarını en küçük kareler ile çözer.
fn solve_gains(layout: &Layout, shape: &[f64; BANDS], target: &[f64; MAX_POINTS]) -> [f64; BANDS] {
    let (n, points) = (layout.bands, layout.points);
    // Etkileşim matrisi: m. filtrenin k. noktadaki tepkisi, kendi kazancına bölünmüş.
    let mut matrix = [[0.0; BANDS]; MAX_POINTS];
    for m in 0..n {
        let filter = Biquad::peaking(
            layout.w0[m],
            layout.bandwidth[m],
            shape[m],
            layout.tuning.edge_ratio,
        );
        for k in 0..points {
            matrix[k][m] = filter.response_db(layout.cos_w[k], layout.cos_2w[k]) / shape[m];
        }
    }
    // Normal denklemler: (AᵀA) g = Aᵀt.
    let mut normal = [[0.0; BANDS]; BANDS];
    let mut rhs = [0.0; BANDS];
    for i in 0..n {
        for j in 0..n {
            normal[i][j] = (0..points).map(|k| matrix[k][i] * matrix[k][j]).sum();
        }
        rhs[i] = (0..points).map(|k| matrix[k][i] * target[k]).sum();
    }
    solve_linear(&mut normal, &mut rhs, n);
    rhs
}

/// `a·x = b` sistemini kısmi pivotlu Gauss eleme ile çözer; sonuç `b`'ye yazılır.
/// Tekil bir sistemde (pratikte oluşmaz) ilgili bilinmeyen 0 kalır.
fn solve_linear(a: &mut [[f64; BANDS]; BANDS], b: &mut [f64; BANDS], n: usize) {
    for col in 0..n {
        let pivot = (col..n)
            .max_by(|&x, &y| a[x][col].abs().total_cmp(&a[y][col].abs()))
            .unwrap_or(col);
        a.swap(col, pivot);
        b.swap(col, pivot);
        let p = a[col][col];
        if p.abs() < 1e-12 {
            continue;
        }
        for row in col + 1..n {
            let factor = a[row][col] / p;
            for k in col..n {
                a[row][k] -= factor * a[col][k];
            }
            b[row] -= factor * b[col];
        }
    }
    for col in (0..n).rev() {
        let p = a[col][col];
        let tail: f64 = (col + 1..n).map(|k| a[col][k] * b[k]).sum();
        b[col] = if p.abs() < 1e-12 {
            0.0
        } else {
            (b[col] - tail) / p
        };
    }
}

/// Bir kanalın filtre durumları (transpoze direkt form II).
type ChannelState = [[f64; 2]; BANDS];

/// Sesi ekolayzerden geçiren gerçek zamanlı işlemci.
///
/// Bellek yalnızca [`EqProcessor::new`] içinde ayrılır; diğer yöntemler ses
/// çıkış iş parçacığında güvenle çağrılabilir (ayırma, kilit, panic yok).
pub struct EqProcessor {
    control: Arc<EqControl>,
    layout: Layout,
    seen_version: u64,
    target: [f64; BANDS],
    current: [f64; BANDS],
    design: Design,
    preamp: f64,
    flat: bool,
    state: Vec<ChannelState>,
    update_frames: usize,
    countdown: usize,
    smoothing: f64,
}

impl EqProcessor {
    /// İşlemciyi kurar. Başlangıçta geçiş yapılmaz: ayarlar hemen uygulanır.
    pub fn new(control: Arc<EqControl>, channels: usize, sample_rate: u32) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let update_frames = ((UPDATE_SECONDS * rate).round() as usize).max(1);
        let seen_version = control.version();
        let target = control.settings().effective_gains();
        let mut processor = Self {
            control,
            layout: Layout::new(rate),
            seen_version,
            target,
            current: target,
            design: Design::new(&[0.0; BANDS], rate),
            preamp: 1.0,
            flat: true,
            state: vec![[[0.0; 2]; BANDS]; channels.max(1)],
            update_frames,
            countdown: 0,
            smoothing: 1.0 - (-(update_frames as f64) / (SMOOTH_SECONDS * rate)).exp(),
        };
        processor.redesign();
        processor
    }

    /// Ses şu an değişmeden mi geçiyor?
    pub fn is_flat(&self) -> bool {
        self.flat
    }

    /// Her doldurma turunun başında çağrılır: yeni ayar var mı bakar.
    pub fn begin_block(&mut self) {
        let version = self.control.version();
        if version != self.seen_version {
            self.seen_version = version;
            self.target = self.control.settings().effective_gains();
        }
        // Sessizlikte sönümlenen filtre durumları "denormal" sayılara inip
        // işlemciyi yavaşlatmasın.
        for value in self.state.iter_mut().flatten().flatten() {
            if value.abs() < 1e-200 {
                *value = 0.0;
            }
        }
    }

    /// Her kareden önce çağrılır; ayar değişiyorsa kısa aralıklarla yumuşakça ilerletir.
    #[inline]
    pub fn advance_frame(&mut self) {
        if self.countdown == 0 {
            self.countdown = self.update_frames;
            if self.current != self.target {
                self.step();
            }
        }
        self.countdown -= 1;
    }

    /// Bir kanalın bir örneğini işler.
    #[inline]
    pub fn process(&mut self, channel: usize, x: Sample) -> Sample {
        if self.flat {
            return x;
        }
        let Some(state) = self.state.get_mut(channel) else {
            return x;
        };
        let mut y = x;
        for (f, s) in self
            .design
            .filters
            .iter()
            .zip(state.iter_mut())
            .take(self.layout.bands)
        {
            let input = y;
            y = f.b0 * input + s[0];
            s[0] = f.b1 * input - f.a1 * y + s[1];
            s[1] = f.b2 * input - f.a2 * y;
        }
        y * self.preamp
    }

    /// Mevcut kazançları hedefe bir adım yaklaştırır ve yeniden tasarlar.
    fn step(&mut self) {
        for (current, &target) in self.current.iter_mut().zip(&self.target) {
            let delta = target - *current;
            *current = if delta.abs() <= SNAP_DB {
                target
            } else {
                *current + delta * self.smoothing
            };
        }
        self.redesign();
    }

    fn redesign(&mut self) {
        self.design = Design::with_layout(&self.current, &self.layout);
        self.preamp = db_to_linear(self.design.preamp_db);
        let flat = self.design.is_flat();
        if flat && !self.flat {
            // Düz hale gelince işlem durur; sonraki açılışta eski durum kalmasın.
            for value in self.state.iter_mut().flatten().flatten() {
                *value = 0.0;
            }
        }
        self.flat = flat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Deterministik sözde rastgele sayılar (testler her seferinde aynı sonucu versin).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 11) as f64 / (1u64 << 53) as f64
        }
    }

    const RATES: [f64; 7] = [
        22050.0, 32000.0, 44100.0, 48000.0, 88200.0, 96000.0, 192000.0,
    ];

    fn max_error_tuned(gains: &[f64; BANDS], rate: f64, tuning: Tuning) -> f64 {
        let layout = Layout::tuned(rate, tuning);
        let design = Design::with_layout(gains, &layout);
        (0..layout.points)
            .map(|k| (design.response_db(design_point_hz(k), rate) - design_target(gains, k)).abs())
            .fold(0.0, f64::max)
    }

    /// Tasarım noktalarındaki en büyük sapma (dB).
    fn max_error(gains: &[f64; BANDS], rate: f64) -> f64 {
        max_error_tuned(gains, rate, TUNING)
    }

    /// Zor uç durumlar ve rastgele ayarlar.
    fn test_settings() -> Vec<[f64; BANDS]> {
        let mut settings = vec![
            [12.0; BANDS],
            [-12.0; BANDS],
            [
                12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0,
            ],
            [
                -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0, -12.0, 12.0,
            ],
            [
                12.0, 12.0, 12.0, 12.0, 12.0, -12.0, -12.0, -12.0, -12.0, -12.0,
            ],
            [0.0, 0.0, 0.0, 0.0, 12.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [12.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 12.0],
        ];
        let mut rng = Lcg(42);
        for _ in 0..300 {
            settings.push(std::array::from_fn(|_| {
                (rng.next() * 2.0 - 1.0) * MAX_GAIN_DB
            }));
        }
        settings
    }

    /// Bir sinüsü işlemciden geçirip kararlı durumdaki kazancını ölçer (dB).
    fn measured_gain_db(processor: &mut EqProcessor, hz: f64, rate: u32) -> f64 {
        // Etkin değer (RMS) kullanılır: tize yakın sinüslerde örnekler tepeye denk gelmez.
        let w = 2.0 * PI * hz / f64::from(rate);
        let total = rate as usize; // 1 sn: en düşük bandın geçişi de söner
        let (mut energy, mut count) = (0.0, 0.0);
        for i in 0..total {
            processor.advance_frame();
            let y = processor.process(0, (w * i as f64).sin());
            if i >= total / 2 {
                energy += y * y;
                count += 1.0;
            }
        }
        10.0 * (2.0 * energy / count).log10()
    }

    #[test]
    fn her_ayarda_ve_her_hizda_egri_surgulere_uyar() {
        for rate in RATES {
            for gains in test_settings() {
                let error = max_error(&gains, rate);
                assert!(error < 1.2, "{rate} Hz, {gains:?}: sapma {error:.2} dB");
            }
        }
    }

    #[test]
    fn yumusak_egrilerde_sapma_cok_kucuk() {
        let presets = [
            [6.0, 5.0, 4.0, 2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 4.0, 5.0, 6.0],
            [-2.0, -2.0, -1.0, 0.0, 2.0, 4.0, 4.0, 2.0, 0.0, -1.0],
            [6.0; BANDS],
        ];
        for gains in presets {
            for rate in [44100.0, 48000.0] {
                let error = max_error(&gains, rate);
                assert!(error < 0.5, "{gains:?}: sapma {error:.2} dB");
            }
        }
    }

    #[test]
    fn butun_surguler_ayni_ise_ses_o_kadar_degisir() {
        // Sıradan ekolayzerlerde bantlar üst üste binip +6 dB'yi +9–10 dB'ye çıkarır.
        let design = Design::new(&[6.0; BANDS], 44100.0);
        for hz in [40.0, 100.0, 440.0, 1000.0, 3000.0, 10000.0] {
            let db = design.response_db(hz, 44100.0);
            assert!((db - 6.0).abs() < 0.5, "{hz} Hz: {db:.2} dB");
        }
        assert_eq!(design.preamp_db, -6.0);
    }

    #[test]
    fn nyquist_ustundeki_bantlar_kullanilmaz() {
        // 22,05 kHz'lik dosyada 16 kHz bandı anlamsız: yok sayılır, ön kazancı da etkilemez.
        let mut gains = [0.0; BANDS];
        gains[9] = 12.0;
        let design = Design::new(&gains, 22050.0);
        assert!(design.is_flat());
        assert_eq!(Layout::new(22050.0).bands, 9);
        assert_eq!(Layout::new(44100.0).bands, BANDS);
    }

    #[test]
    fn ayarlar_duzeltilir() {
        let mut gains = [0.0; BANDS];
        gains[0] = 40.0;
        gains[1] = f64::NAN;
        gains[2] = -15.0;
        let fixed = EqSettings {
            enabled: true,
            gains_db: gains,
        }
        .sanitized();
        assert_eq!(fixed.gains_db[0], 12.0);
        assert_eq!(fixed.gains_db[1], 0.0);
        assert_eq!(fixed.gains_db[2], -12.0);
        assert!(fixed.is_active());
        assert!(!EqSettings::default().is_active());
        assert!(!EqSettings {
            enabled: false,
            ..fixed
        }
        .is_active());
    }

    #[test]
    fn duzken_ses_hic_degismez() {
        let control = Arc::new(EqControl::default());
        let mut processor = EqProcessor::new(control, 2, 44100);
        assert!(processor.is_flat());
        processor.begin_block();
        for x in [0.0, 0.5, -1.0, 0.123_456_789] {
            processor.advance_frame();
            assert_eq!(processor.process(0, x), x);
            assert_eq!(processor.process(1, x), x);
        }
    }

    #[test]
    fn islemci_tasarlanan_kazanci_uygular() {
        let rate = 48000;
        let mut gains = [0.0; BANDS];
        gains[5] = 9.0; // 1 kHz
        gains[1] = -6.0; // 62,5 Hz
        let control = Arc::new(EqControl::new(EqSettings {
            enabled: true,
            gains_db: gains,
        }));
        let mut processor = EqProcessor::new(control, 1, rate);
        processor.begin_block();
        // Ön kazanç -9 dB: 1 kHz'te toplam ~0 dB, 62,5 Hz'te ~-15 dB, 8 kHz'te ~-9 dB.
        let at_1k = measured_gain_db(&mut processor, 1000.0, rate);
        let at_62 = measured_gain_db(&mut processor, 62.5, rate);
        let at_8k = measured_gain_db(&mut processor, 8000.0, rate);
        assert!(at_1k.abs() < 0.6, "1 kHz: {at_1k:.2}");
        assert!((at_62 + 15.0).abs() < 0.8, "62,5 Hz: {at_62:.2}");
        assert!((at_8k + 9.0).abs() < 0.6, "8 kHz: {at_8k:.2}");
    }

    #[test]
    fn ayar_degisikligi_yumusakca_uygulanir() {
        let rate = 48000;
        let control = Arc::new(EqControl::default());
        let mut processor = EqProcessor::new(Arc::clone(&control), 1, rate);
        let mut gains = [0.0; BANDS];
        gains[5] = 12.0;
        control.set(EqSettings {
            enabled: true,
            gains_db: gains,
        });
        processor.begin_block();
        // 1 kHz bandı yükselir, ön kazanç -12 dB'ye iner: 4 kHz'teki ses -12 dB'ye
        // düşer. Bu iniş ani olmamalı.
        let w = 2.0 * PI * 4000.0 / f64::from(rate);
        let mut levels = Vec::new();
        let mut peak = 0.0f64;
        for i in 0..(rate as usize / 2) {
            processor.advance_frame();
            peak = peak.max(processor.process(0, (w * i as f64).sin()).abs());
            if i % 480 == 479 {
                levels.push(peak);
                peak = 0.0;
            }
        }
        // İlk 10 ms'de hâlâ yüksek, 0,5 sn sonra hedefte.
        assert!(levels[0] > 0.7, "ilk 10 ms: {}", levels[0]);
        let last = *levels.last().unwrap();
        assert!((20.0 * last.log10() + 12.0).abs() < 0.7, "son: {last}");
        // Ani sıçrama yok: 12 dB'lik değişim 10 ms'lik pencerelerde en fazla ~3 dB'lik
        // adımlarla (her 2 ms'de küçük adımlarla) iner.
        for pair in levels.windows(2) {
            let jump = 20.0 * (pair[1] / pair[0]).log10();
            assert!(jump.abs() < 3.5, "sıçrama {jump:.2} dB");
        }
        assert!(!processor.is_flat());

        // Kapatınca yine yumuşakça düzleşir ve işlem tamamen durur.
        control.set(EqSettings {
            enabled: false,
            gains_db: gains,
        });
        processor.begin_block();
        for _ in 0..rate {
            processor.advance_frame();
            processor.process(0, 0.0);
        }
        assert!(processor.is_flat());
    }

    #[test]
    fn denetim_kanali_ayarlari_tasir() {
        let control = EqControl::default();
        let before = control.version();
        let mut gains = [0.0; BANDS];
        gains[3] = 99.0;
        let applied = control.set(EqSettings {
            enabled: false,
            gains_db: gains,
        });
        assert_eq!(applied.gains_db[3], 12.0);
        assert_eq!(control.settings(), applied);
        assert!(control.version() > before);
    }

    #[test]
    fn arayuz_durumu_egriyi_ve_on_kazanci_icerir() {
        let mut gains = [0.0; BANDS];
        gains[0] = 6.0;
        let state = EqState::new(EqSettings {
            enabled: true,
            gains_db: gains,
        });
        assert_eq!(state.preamp_db, -6.0);
        assert_eq!(state.curve_hz.len(), state.curve_db.len());
        assert!((state.curve_hz[0] - 20.0).abs() < 1e-9);
        assert!((state.curve_hz[CURVE_POINTS - 1] - 20000.0).abs() < 1e-6);
        // Bas yükseltilmiş: düşük frekanslarda eğri yüksek, tizde düz.
        assert!(state.curve_db[5] > 4.0);
        assert!(state.curve_db[CURVE_POINTS - 1].abs() < 0.2);

        let off = EqState::new(EqSettings {
            enabled: false,
            gains_db: gains,
        });
        assert_eq!(off.preamp_db, 0.0);
        assert!(off.curve_db.iter().all(|d| d.abs() < 1e-9));
        assert_eq!(off.gains_db[0], 6.0, "kapalıyken de sürgüler hatırlanır");
    }

    #[test]
    #[ignore = "ayar keşfi için: cargo test --release -- --ignored --nocapture ayar_taramasi"]
    fn ayar_taramasi() {
        let settings = test_settings();
        let mut results = Vec::new();
        for bandwidth_ratio in [0.9, 0.95, 1.0, 1.05, 1.1] {
            for edge_ratio in [0.38, 0.4, 0.42] {
                for limit in [0.8, 1.0, 1.2, 1.4, 1.6, 1.8, 2.2] {
                    let tuning = Tuning {
                        bandwidth_ratio,
                        edge_ratio,
                        limit,
                        ..TUNING
                    };
                    let worst = RATES
                        .iter()
                        .flat_map(|&rate| {
                            settings
                                .iter()
                                .map(move |g| max_error_tuned(g, rate, tuning))
                        })
                        .fold(0.0, f64::max);
                    results.push((worst, tuning));
                }
            }
        }
        results.sort_by(|a, b| a.0.total_cmp(&b.0));
        for (worst, tuning) in results.iter().take(10) {
            println!("{worst:.3} dB  {tuning:?}");
        }
    }
}
