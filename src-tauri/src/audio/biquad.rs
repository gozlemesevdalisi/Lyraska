//! İkinci dereceden süzgeç (biquad): tasarım, frekans yanıtı ve devrik doğrudan biçim II ile
//! işleme. Kulaklık düzeltmesi ve parametrik ekolayzer ([`super::peq`]), bas motoru
//! ([`super::bass`]) kullanır. Hesaplar 64-bittir; tasarım dahil bellek ayırmaz (ses iş
//! parçacığında da çalışır).
//!
//! İki tasarım yöntemi vardır ([`Method`]):
//!
//! - **RBJ** ("Audio EQ Cookbook"): analog prototipin çift doğrusal dönüşümü. Equalizer APO
//!   ve AutoEq bunu kullanır; kulaklık profilleri onlarla bire bir aynı çalsın diye onların
//!   varsayılanıdır. Çift doğrusal dönüşüm frekans eksenini Nyquist'e doğru sıkıştırır
//!   ("cramping"): 44,1 kHz'te 8 kHz'in üstündeki süzgeçler analog tasarımından birkaç dB,
//!   alçak geçirenler Nyquist yakınında onlarca dB sapar.
//! - **Eşlenmiş** ([`Biquad::matched`]): kutuplar analog kutupların tam karşılığıdır
//!   (z = e^{sT}: rezonansın yeri ve sönümü bozulmaz). Pay, genliğin karesinin doğrusal bir
//!   yazılışıyla bulunur (Vicanek, "Matched Second Order Digital Filters", 2016):
//!
//!   ```text
//!   |B(e^{jω})|² = B0·φ0 + B1·φ1 + B2·φ2,   φ0 = cos²(ω/2), φ1 = sin²(ω/2), φ2 = 4·φ0·φ1
//!   B0 = (b0 + b1 + b2)²,  B1 = (b0 − b1 + b2)²,  B2 = −4·b0·b2
//!   ```
//!
//!   Payda için A0, A1, A2 aynı biçimdedir. Analog genliğin karesi T(ω) verilince
//!   B·φ = T·(A·φ) denklemi B'lerde doğrusaldır: B0 DC'den, B1 Nyquist'ten, B2 ya merkez
//!   frekanstan ya da duyulur bant boyunca en küçük kareler yöntemiyle (göreli hata) çıkar.
//!   B'lerden katsayılara dönüş kapalı biçimdedir: W = (√B0 + √B1)/2, b1 = (√B0 − √B1)/2,
//!   b0 = (W + √(W² + B2))/2, b2 = −B2/(4·b0). W² + B2 = (b0 − b2)² olduğundan W² + B2 < 0 olan
//!   bir hedef ikinci dereceden gerçek bir payla yazılamaz; o aday atılır. Pay sıfırları birim
//!   çemberin dışına düşerse içeri yansıtılır (genlik aynı kalır, süzgeç en küçük fazlı olur).
//!
//!   Nyquist'e yakın süzgeçlerde üç nokta birden tutturulamayabilir; bu yüzden birkaç aday
//!   kurulur (Nyquist hedefi 1, ½, ¼ katı; B2 merkezden ya da en küçük karelerle) ve her biri
//!   duyulur bantta (20 Hz – 20 kHz, merkez çevresi sık) analog eğriye göre ölçülür; en yakını
//!   seçilir. Sapma, analog eğrinin −12 dB üstünde tam, −30 dB'ye doğru ¼ ağırlıkla sayılır
//!   ([`error_weight`]). RBJ de adaydır: sonuç hiçbir ayarda RBJ'den uzak olamaz.

use std::f64::consts::PI;

use rustfft::num_complex::Complex;

use super::Sample;

/// Katsayılar (a0 = 1'e bölünmüş).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Biquad {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

/// Bir kanalın süzgeç belleği.
pub type State = [f64; 2];

/// Nyquist'e bu orandan yakın süzgeçler tasarlanmaz (katsayılar bozulur).
const MAX_FREQ_RATIO: f64 = 0.49;

/// Süzgeç türü (analog prototipin biçimi).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Tepe/çukur.
    Peaking,
    /// Alçak raf: `freq_hz` (orta kazanç noktası) altını yükseltir/kısar.
    LowShelf,
    /// Yüksek raf: `freq_hz` (orta kazanç noktası) üstünü yükseltir/kısar.
    HighShelf,
    /// Alçak geçiren (12 dB/oktav; Q = 1/√2 Butterworth).
    LowPass,
    /// Yüksek geçiren (12 dB/oktav).
    HighPass,
    /// Bant geçiren (tepe kazancı 0 dB).
    BandPass,
    /// Çentik.
    Notch,
    /// Tüm geçiren: genliğe dokunmaz, fazı döndürür.
    AllPass,
}

impl Kind {
    /// Kazanç parametresi kullanılır mı?
    pub fn has_gain(self) -> bool {
        matches!(self, Self::Peaking | Self::LowShelf | Self::HighShelf)
    }
}

/// Tasarım yöntemi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Method {
    /// Çift doğrusal dönüşüm (Equalizer APO ve AutoEq ile aynı).
    #[default]
    Rbj,
    /// Analog eğriye eşlenmiş (bkz. modül belgesi).
    Matched,
}

/// Bir süzgecin tanımı: türü, frekansı (Hz), kazancı (dB; yalnızca tepe ve raflarda) ve Q.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spec {
    pub kind: Kind,
    pub freq_hz: f64,
    pub gain_db: f64,
    pub q: f64,
}

impl Spec {
    /// Analog prototipin `hz`'teki karmaşık yanıtı (RBJ'nin analog prototipleri: Equalizer APO
    /// ve AutoEq'in parametreleri bu eğrileri tarif eder).
    pub fn analog(&self, hz: f64) -> Complex<f64> {
        let s = Complex::new(0.0, hz / self.freq_hz);
        let a = 10f64.powf(self.gain_db / 40.0);
        let q = self.q;
        let one = Complex::new(1.0, 0.0);
        let (num, den) = match self.kind {
            Kind::Peaking => (s * s + s * (a / q) + 1.0, s * s + s / (a * q) + 1.0),
            Kind::LowShelf => {
                let k = a.sqrt() / q;
                ((s * s + s * k + a) * a, s * s * a + s * k + 1.0)
            }
            Kind::HighShelf => {
                let k = a.sqrt() / q;
                ((s * s * a + s * k + 1.0) * a, s * s + s * k + a)
            }
            Kind::LowPass => (one, s * s + s / q + 1.0),
            Kind::HighPass => (s * s, s * s + s / q + 1.0),
            Kind::BandPass => (s / q, s * s + s / q + 1.0),
            Kind::Notch => (s * s + 1.0, s * s + s / q + 1.0),
            Kind::AllPass => (s * s - s / q + 1.0, s * s + s / q + 1.0),
        };
        num / den
    }

    /// Analog prototipin `hz`'teki kazancı (dB).
    pub fn analog_db(&self, hz: f64) -> f64 {
        10.0 * self.analog(hz).norm_sqr().max(1e-300).log10()
    }

    /// Sese dokunmuyor mu? (Kazançlı türlerde kazanç ~0.)
    fn is_flat(&self) -> bool {
        self.kind.has_gain() && self.gain_db.abs() < 1e-6
    }

    /// Analog paydanın doğal frekansı (`freq_hz` katı) ve sönüm oranı ζ.
    fn pole(&self) -> (f64, f64) {
        let a = 10f64.powf(self.gain_db / 40.0);
        let zeta = 1.0 / (2.0 * self.q);
        match self.kind {
            // s² + s/(A·Q) + 1
            Kind::Peaking => (1.0, 1.0 / (2.0 * a * self.q)),
            // A·s² + (√A/Q)·s + 1  →  doğal frekans 1/√A
            Kind::LowShelf => (1.0 / a.sqrt(), zeta),
            // s² + (√A/Q)·s + A  →  doğal frekans √A
            Kind::HighShelf => (a.sqrt(), zeta),
            _ => (1.0, zeta),
        }
    }
}

impl Biquad {
    /// Sese dokunmayan süzgeç.
    pub const IDENTITY: Self = Self {
        b0: 1.0,
        b1: 0.0,
        b2: 0.0,
        a1: 0.0,
        a2: 0.0,
    };

    fn normalized(b: [f64; 3], a: [f64; 3]) -> Self {
        Self {
            b0: b[0] / a[0],
            b1: b[1] / a[0],
            b2: b[2] / a[0],
            a1: a[1] / a[0],
            a2: a[2] / a[0],
        }
    }

    /// Kazançlı süzgeçler için ortak hazırlık; etkisizse `None`.
    fn shape(freq_hz: f64, gain_db: f64, q: f64, sample_rate: f64) -> Option<(f64, f64, f64, f64)> {
        if freq_hz >= sample_rate * MAX_FREQ_RATIO || gain_db.abs() < 1e-6 {
            return None;
        }
        let a = 10f64.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        Some((a, cos, sin / (2.0 * q), sin))
    }

    /// Tepe/çukur.
    pub fn peaking(freq_hz: f64, gain_db: f64, q: f64, sample_rate: f64) -> Self {
        let Some((a, cos, alpha, _)) = Self::shape(freq_hz, gain_db, q, sample_rate) else {
            return Self::IDENTITY;
        };
        Self::normalized(
            [1.0 + alpha * a, -2.0 * cos, 1.0 - alpha * a],
            [1.0 + alpha / a, -2.0 * cos, 1.0 - alpha / a],
        )
    }

    /// Alçak raf: `freq_hz`'in altını `gain_db` kadar yükseltir/kısar.
    pub fn low_shelf(freq_hz: f64, gain_db: f64, q: f64, sample_rate: f64) -> Self {
        let Some((a, cos, alpha, _)) = Self::shape(freq_hz, gain_db, q, sample_rate) else {
            return Self::IDENTITY;
        };
        let k = 2.0 * a.sqrt() * alpha;
        Self::normalized(
            [
                a * ((a + 1.0) - (a - 1.0) * cos + k),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cos),
                a * ((a + 1.0) - (a - 1.0) * cos - k),
            ],
            [
                (a + 1.0) + (a - 1.0) * cos + k,
                -2.0 * ((a - 1.0) + (a + 1.0) * cos),
                (a + 1.0) + (a - 1.0) * cos - k,
            ],
        )
    }

    /// Yüksek raf: `freq_hz`'in üstünü `gain_db` kadar yükseltir/kısar.
    pub fn high_shelf(freq_hz: f64, gain_db: f64, q: f64, sample_rate: f64) -> Self {
        let Some((a, cos, alpha, _)) = Self::shape(freq_hz, gain_db, q, sample_rate) else {
            return Self::IDENTITY;
        };
        let k = 2.0 * a.sqrt() * alpha;
        Self::normalized(
            [
                a * ((a + 1.0) + (a - 1.0) * cos + k),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cos),
                a * ((a + 1.0) + (a - 1.0) * cos - k),
            ],
            [
                (a + 1.0) - (a - 1.0) * cos + k,
                2.0 * ((a - 1.0) - (a + 1.0) * cos),
                (a + 1.0) - (a - 1.0) * cos - k,
            ],
        )
    }

    /// Alçak geçiren (Q = 1/√2: Butterworth). Nyquist'e çok yakınsa etkisiz.
    pub fn lowpass(freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        if freq_hz >= sample_rate * MAX_FREQ_RATIO {
            return Self::IDENTITY;
        }
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized(
            [(1.0 - cos) / 2.0, 1.0 - cos, (1.0 - cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// Yüksek geçiren (Q = 1/√2: Butterworth). Frekans Nyquist'e yakınsa oraya çekilir.
    pub fn highpass(freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        let freq_hz = freq_hz.min(sample_rate * MAX_FREQ_RATIO);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized(
            [(1.0 + cos) / 2.0, -(1.0 + cos), (1.0 + cos) / 2.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// Bant geçiren (tepe kazancı 0 dB). Frekans Nyquist'e yakınsa oraya çekilir.
    pub fn bandpass(freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        let freq_hz = freq_hz.min(sample_rate * MAX_FREQ_RATIO);
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * cos, 1.0 - alpha])
    }

    /// Çentik. Nyquist'e çok yakınsa etkisiz.
    pub fn notch(freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        if freq_hz >= sample_rate * MAX_FREQ_RATIO {
            return Self::IDENTITY;
        }
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized(
            [1.0, -2.0 * cos, 1.0],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// Tüm geçiren. Nyquist'e çok yakınsa etkisiz.
    pub fn allpass(freq_hz: f64, q: f64, sample_rate: f64) -> Self {
        if freq_hz >= sample_rate * MAX_FREQ_RATIO {
            return Self::IDENTITY;
        }
        let w0 = 2.0 * PI * freq_hz / sample_rate;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * q);
        Self::normalized(
            [1.0 - alpha, -2.0 * cos, 1.0 + alpha],
            [1.0 + alpha, -2.0 * cos, 1.0 - alpha],
        )
    }

    /// Tanımlanan süzgeci verilen yöntemle tasarlar.
    pub fn design(spec: &Spec, sample_rate: f64, method: Method) -> Self {
        match method {
            Method::Rbj => Self::rbj(spec, sample_rate),
            Method::Matched => Self::matched(spec, sample_rate),
        }
    }

    /// RBJ tasarımı (Equalizer APO'nun formülleri).
    pub fn rbj(spec: &Spec, sample_rate: f64) -> Self {
        let Spec {
            kind,
            freq_hz,
            gain_db,
            q,
        } = *spec;
        match kind {
            Kind::Peaking => Self::peaking(freq_hz, gain_db, q, sample_rate),
            Kind::LowShelf => Self::low_shelf(freq_hz, gain_db, q, sample_rate),
            Kind::HighShelf => Self::high_shelf(freq_hz, gain_db, q, sample_rate),
            Kind::LowPass => Self::lowpass(freq_hz, q, sample_rate),
            Kind::HighPass => Self::highpass(freq_hz, q, sample_rate),
            Kind::BandPass => Self::bandpass(freq_hz, q, sample_rate),
            Kind::Notch => Self::notch(freq_hz, q, sample_rate),
            Kind::AllPass => Self::allpass(freq_hz, q, sample_rate),
        }
    }

    /// Analog eğriye eşlenmiş tasarım (modül belgesi). Nyquist'e çok yakın ya da etkisiz
    /// süzgeçlerde RBJ ile aynıdır.
    pub fn matched(spec: &Spec, sample_rate: f64) -> Self {
        let rbj = Self::rbj(spec, sample_rate);
        if spec.is_flat() || spec.freq_hz >= sample_rate * MAX_FREQ_RATIO {
            return rbj;
        }
        let Some(grid) = Grid::new(spec, sample_rate) else {
            return rbj;
        };
        let (w_n, zeta) = spec.pole();
        let (a1, a2) = matched_poles(2.0 * PI * spec.freq_hz * w_n / sample_rate, zeta);
        let den = Power::of(1.0, a1, a2);
        let target = |w: f64| spec.analog(w * sample_rate / (2.0 * PI)).norm_sqr();
        let w0 = 2.0 * PI * spec.freq_hz / sample_rate;

        let mut best: Option<(f64, Self)> = None;
        let mut consider = |candidate: Self| {
            let error = grid.error(&candidate);
            if error.is_finite() && best.is_none_or(|(e, _)| error < e) {
                best = Some((error, candidate));
            }
        };
        let with_poles = |b: [f64; 3]| Self {
            b0: b[0],
            b1: b[1],
            b2: b[2],
            a1,
            a2,
        };
        match spec.kind {
            Kind::AllPass => {
                // Kutupların tersleri sıfır: genlik her yerde tam 1.
                consider(with_poles([a2, a1, 1.0]));
            }
            Kind::Notch => {
                // Sıfırlar tam çentik frekansında (birim çember üzerinde); ölçek DC'ye ya da
                // Nyquist'e göre.
                let cos = w0.cos();
                let dc = (1.0 + a1 + a2) / (2.0 - 2.0 * cos);
                let nyquist = (1.0 - a1 + a2) / (2.0 + 2.0 * cos);
                for k in [dc, nyquist] {
                    consider(with_poles([k, -2.0 * cos * k, k]));
                }
            }
            Kind::HighPass => {
                // Pay k·(1 − z⁻¹)²: |B|² = 16·k²·φ1². k merkezde ya da en küçük karelerle.
                let (_, p1, _) = phi(w0);
                if p1 > 0.0 {
                    let k = (target(w0) * den.at(w0) / (16.0 * p1 * p1)).sqrt();
                    consider(with_poles([k, -2.0 * k, k]));
                }
                let (mut su, mut suu) = (0.0, 0.0);
                for i in grid.used() {
                    let u =
                        16.0 * grid.p1[i] * grid.p1[i] / (grid.target[i] * den.at_phi(&grid, i));
                    su += u;
                    suu += u * u;
                }
                if suu > 0.0 {
                    let k = (su / suu).sqrt();
                    consider(with_poles([k, -2.0 * k, k]));
                }
            }
            _ => {
                let b0_power = if spec.kind == Kind::BandPass {
                    0.0
                } else {
                    target(0.0) * den.b0
                };
                let nyquist_power = target(PI) * den.b1;
                let (p0, p1, p2) = phi(w0);
                // Bant geçirende Nyquist'te sıfır da aday (κ = 0): alçak frekanslı bant
                // geçirenin eteği Nyquist'e kadar düşmeyi sürdürür.
                const KAPPAS: [f64; 4] = [1.0, 0.5, 0.25, 0.0];
                let kappas = if spec.kind == Kind::BandPass {
                    &KAPPAS[..]
                } else {
                    &KAPPAS[..KAPPAS.len() - 1]
                };
                for &kappa in kappas {
                    let b1_power = nyquist_power * kappa;
                    // B2 en küçük karelerle (göreli hata): Σ w·(B·φ − T·D)², w = 1/(T·D)².
                    let (mut num, mut den2) = (0.0, 0.0);
                    for i in grid.used() {
                        let td = grid.target[i] * den.at_phi(&grid, i);
                        let r = td - b0_power * grid.p0[i] - b1_power * grid.p1[i];
                        let w = 1.0 / (td * td);
                        num += w * grid.p2[i] * r;
                        den2 += w * grid.p2[i] * grid.p2[i];
                    }
                    let mut options = [None, None];
                    if den2 > 0.0 {
                        options[0] = Some(num / den2);
                    }
                    if p2 > 0.0 {
                        options[1] =
                            Some((target(w0) * den.at(w0) - b0_power * p0 - b1_power * p1) / p2);
                    }
                    for b2_power in options.into_iter().flatten() {
                        if let Some(b) = factor(b0_power, b1_power, b2_power) {
                            consider(with_poles(b));
                        }
                    }
                }
            }
        }
        // RBJ de aday: yalnızca açıkça daha yakınsa seçilir.
        let rbj_error = grid.error(&rbj);
        match best {
            Some((error, candidate)) if error <= rbj_error + 1e-9 => candidate,
            _ => rbj,
        }
    }

    /// `w` açısal frekansındaki (radyan/örnek) karmaşık yanıt.
    pub fn response(&self, w: f64) -> Complex<f64> {
        let z1 = Complex::from_polar(1.0, -w);
        let z2 = z1 * z1;
        let num = z1 * self.b1 + z2 * self.b2 + self.b0;
        let den = z1 * self.a1 + z2 * self.a2 + 1.0;
        num / den
    }

    /// `w` açısal frekansındaki (radyan/örnek) kazanç (dB).
    ///
    /// φ-tabanında hesaplanır: |B|² = B0·φ0 + B1·φ1 + B2·φ2. Kosinüslü yazılış
    /// (b0² + b1² + b2² + 2(b0b1 + b1b2)cos ω + 2b0b2 cos 2ω) alçak frekanstaki sıfırlarda
    /// (çentik, yüksek geçiren) büyük sayıların farkını alır ve birkaç dB yanılır; bu yazılışta
    /// terimler küçük frekansta da doğru kalır.
    pub fn response_db(&self, w: f64) -> f64 {
        let (p0, p1, p2) = phi(w);
        self.response_db_phi(p0, p1, p2)
    }

    fn response_db_phi(&self, p0: f64, p1: f64, p2: f64) -> f64 {
        let num = Power::of(self.b0, self.b1, self.b2);
        let den = Power::of(1.0, self.a1, self.a2);
        let n = num.b0 * p0 + num.b1 * p1 + num.b2 * p2;
        let d = den.b0 * p0 + den.b1 * p1 + den.b2 * p2;
        10.0 * (n.max(1e-300) / d.max(1e-300)).log10()
    }

    /// Kararlı mı? (Kutuplar birim çemberin içinde: |a2| < 1 ve |a1| < 1 + a2.)
    pub fn is_stable(&self) -> bool {
        self.a2.abs() < 1.0 && self.a1.abs() < 1.0 + self.a2
    }

    /// Pay sıfırlarının en büyük yarıçapı (en küçük fazlıda ≤ 1).
    pub fn max_zero_radius(&self) -> f64 {
        let Self { b0, b1, b2, .. } = *self;
        if b0 == 0.0 {
            return f64::INFINITY;
        }
        let disc = b1 * b1 - 4.0 * b0 * b2;
        if disc < 0.0 {
            (b2 / b0).abs().sqrt()
        } else {
            let root = disc.sqrt();
            ((-b1 + root) / (2.0 * b0))
                .abs()
                .max(((-b1 - root) / (2.0 * b0)).abs())
        }
    }

    /// Bir örneği işler (devrik doğrudan biçim II).
    #[inline]
    pub fn process(&self, state: &mut State, x: Sample) -> Sample {
        let y = self.b0 * x + state[0];
        state[0] = self.b1 * x - self.a1 * y + state[1];
        state[1] = self.b2 * x - self.a2 * y;
        y
    }
}

/// φ0 = cos²(ω/2), φ1 = sin²(ω/2), φ2 = 4·φ0·φ1.
fn phi(w: f64) -> (f64, f64, f64) {
    let (sin, cos) = (w / 2.0).sin_cos();
    let (p0, p1) = (cos * cos, sin * sin);
    (p0, p1, 4.0 * p0 * p1)
}

/// Analog kutupların z = e^{sT} karşılığı: `w_n` doğal frekans (radyan/örnek), `zeta` sönüm.
fn matched_poles(w_n: f64, zeta: f64) -> (f64, f64) {
    if zeta < 1.0 {
        let r = (-zeta * w_n).exp();
        (-2.0 * r * (w_n * (1.0 - zeta * zeta).sqrt()).cos(), r * r)
    } else {
        // İki gerçek kutup; üsler negatif (taşma yok).
        let d = (zeta * zeta - 1.0).sqrt();
        let z1 = (w_n * (-zeta + d)).exp();
        let z2 = (w_n * (-zeta - d)).exp();
        (-(z1 + z2), z1 * z2)
    }
}

/// Bir ikinci dereceden polinomun genliğinin karesi φ-tabanında: B0, B1, B2.
#[derive(Debug, Clone, Copy)]
struct Power {
    b0: f64,
    b1: f64,
    b2: f64,
}

impl Power {
    fn of(c0: f64, c1: f64, c2: f64) -> Self {
        Self {
            b0: (c0 + c1 + c2).powi(2),
            b1: (c0 - c1 + c2).powi(2),
            b2: -4.0 * c0 * c2,
        }
    }

    fn at(&self, w: f64) -> f64 {
        let (p0, p1, p2) = phi(w);
        self.b0 * p0 + self.b1 * p1 + self.b2 * p2
    }

    fn at_phi(&self, grid: &Grid, i: usize) -> f64 {
        self.b0 * grid.p0[i] + self.b1 * grid.p1[i] + self.b2 * grid.p2[i]
    }
}

/// B0, B1, B2'den en küçük fazlı pay katsayıları; ikinci dereceden gerçek bir payla
/// yazılamıyorsa `None`.
fn factor(b0_power: f64, b1_power: f64, b2_power: f64) -> Option<[f64; 3]> {
    if b0_power < 0.0 || b1_power < 0.0 || !b2_power.is_finite() {
        return None;
    }
    let (r0, r1) = (b0_power.sqrt(), b1_power.sqrt());
    let w = 0.5 * (r0 + r1);
    let mut disc = w * w + b2_power;
    if disc < -1e-12 * (w * w).max(1e-300) {
        return None;
    }
    disc = disc.max(0.0);
    let b0 = 0.5 * (w + disc.sqrt());
    if b0 <= 0.0 || !b0.is_finite() {
        return None;
    }
    Some(min_phase([b0, 0.5 * (r0 - r1), -b2_power / (4.0 * b0)]))
}

/// Birim çemberin dışındaki gerçek sıfırları içeri yansıtır (|1 − z·e^{−jω}| = |z|·|1 − e^{−jω}/z|:
/// genlik değişmez). Karmaşık eşlenik sıfırlar [`factor`]'de zaten içeridedir.
fn min_phase(b: [f64; 3]) -> [f64; 3] {
    let [b0, b1, b2] = b;
    let disc = b1 * b1 - 4.0 * b0 * b2;
    if disc < 0.0 || b0 == 0.0 || b2 == 0.0 {
        return b;
    }
    let root = disc.sqrt();
    let mut zeros = [(-b1 + root) / (2.0 * b0), (-b1 - root) / (2.0 * b0)];
    let mut gain = b0;
    for z in &mut zeros {
        if z.abs() > 1.0 {
            gain *= z.abs();
            *z = 1.0 / *z;
        }
    }
    [
        gain,
        -gain * (zeros[0] + zeros[1]),
        gain * zeros[0] * zeros[1],
    ]
}

/// Adayların ölçüldüğü frekans ızgarası: 20 Hz – 20 kHz (ya da 0,49·fs) arası 48 logaritmik
/// nokta, merkezin çevresinde bant genişliğine göre sık 17 nokta (dar tepeler kaçmasın) ve
/// raflarda merkezin iki oktav çevresinde 9 nokta (raf tümsekleri kaçmasın).
struct Grid {
    len: usize,
    reference_db: [f64; GRID_MAX],
    weight: [f64; GRID_MAX],
    target: [f64; GRID_MAX],
    p0: [f64; GRID_MAX],
    p1: [f64; GRID_MAX],
    p2: [f64; GRID_MAX],
}

const GRID_LOG: usize = 48;
const GRID_NEAR: usize = 17;
const GRID_SHELF: usize = 9;
const GRID_MAX: usize = GRID_LOG + GRID_NEAR + GRID_SHELF;
/// Analog eğrinin bu düzeyin altındaki kısmı ölçülmez (söndürme bölgesinde dB farkı anlamsız).
const FLOOR_DB: f64 = -30.0;

/// Sapmanın ağırlığı: analog eğri −12 dB'nin üstündeyse 1, −30 dB'ye doğru ¼'e iner (derin
/// eteklerdeki sapma geçiş bandındakinden çok daha az duyulur).
pub fn error_weight(reference_db: f64) -> f64 {
    ((reference_db - FLOOR_DB) / 18.0).clamp(0.25, 1.0)
}

impl Grid {
    fn new(spec: &Spec, sample_rate: f64) -> Option<Self> {
        let low = 20.0;
        let high = (sample_rate * MAX_FREQ_RATIO).min(20_000.0);
        if high <= low * 1.01 {
            return None;
        }
        let mut grid = Self {
            len: 0,
            reference_db: [0.0; GRID_MAX],
            weight: [0.0; GRID_MAX],
            target: [0.0; GRID_MAX],
            p0: [0.0; GRID_MAX],
            p1: [0.0; GRID_MAX],
            p2: [0.0; GRID_MAX],
        };
        let ratio = high / low;
        for i in 0..GRID_LOG {
            grid.push(
                spec,
                sample_rate,
                low * ratio.powf(i as f64 / (GRID_LOG - 1) as f64),
            );
        }
        let mut near = |count: usize, step: f64| {
            for j in 0..count {
                let octaves = (j as f64 - (count / 2) as f64) * step;
                let hz = spec.freq_hz * 2f64.powf(octaves);
                if (low..=high).contains(&hz) {
                    grid.push(spec, sample_rate, hz);
                }
            }
        };
        near(GRID_NEAR, 0.25 / spec.q.max(1e-3));
        if matches!(spec.kind, Kind::LowShelf | Kind::HighShelf) {
            near(GRID_SHELF, 0.5);
        }
        Some(grid)
    }

    fn push(&mut self, spec: &Spec, sample_rate: f64, hz: f64) {
        let i = self.len;
        if i >= GRID_MAX {
            return;
        }
        let w = 2.0 * PI * hz / sample_rate;
        let t = spec.analog(hz).norm_sqr();
        let (p0, p1, p2) = phi(w);
        self.reference_db[i] = 10.0 * t.max(1e-300).log10();
        self.weight[i] = error_weight(self.reference_db[i]);
        self.target[i] = t;
        self.p0[i] = p0;
        self.p1[i] = p1;
        self.p2[i] = p2;
        self.len += 1;
    }

    /// Ölçüme giren noktalar (analog eğri söndürme bölgesinde değil).
    fn used(&self) -> impl Iterator<Item = usize> + '_ {
        (0..self.len).filter(|&i| self.reference_db[i] > FLOOR_DB)
    }

    /// Adayın analog eğriden en büyük (ağırlıklı) sapması (dB).
    fn error(&self, candidate: &Biquad) -> f64 {
        if !candidate.is_stable() {
            return f64::INFINITY;
        }
        self.used()
            .map(|i| {
                let error = candidate.response_db_phi(self.p0[i], self.p1[i], self.p2[i])
                    - self.reference_db[i];
                error.abs() * self.weight[i]
            })
            .fold(0.0, f64::max)
    }
}

/// Çok küçük (denormal) değerleri sıfırlar: işlemci denormal sayılarda çok yavaşlar.
#[inline]
pub fn flush(state: &mut State) {
    for value in state.iter_mut() {
        if value.abs() < 1e-200 {
            *value = 0.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f64 = 48_000.0;

    fn db_at(filter: &Biquad, hz: f64) -> f64 {
        filter.response_db(2.0 * PI * hz / RATE)
    }

    #[test]
    fn tasarimlar_beklenen_egriyi_verir() {
        let shelf = Biquad::low_shelf(100.0, 9.0, 0.707, RATE);
        assert!((db_at(&shelf, 20.0) - 9.0).abs() < 0.3);
        assert!((db_at(&shelf, 100.0) - 4.5).abs() < 0.2, "köşede yarısı");
        assert!(db_at(&shelf, 2000.0).abs() < 0.05);
        let high = Biquad::high_shelf(8000.0, -6.0, 0.707, RATE);
        assert!((db_at(&high, 20_000.0) + 6.0).abs() < 0.5);
        assert!(db_at(&high, 200.0).abs() < 0.05);
        let peak = Biquad::peaking(1000.0, 6.0, 1.0, RATE);
        assert!((db_at(&peak, 1000.0) - 6.0).abs() < 1e-9);
        let low = Biquad::lowpass(120.0, std::f64::consts::FRAC_1_SQRT_2, RATE);
        assert!(db_at(&low, 20.0).abs() < 0.05);
        assert!(
            (db_at(&low, 120.0) + 3.01).abs() < 0.05,
            "Butterworth köşesi −3 dB"
        );
        assert!(db_at(&low, 1200.0) < -39.0, "on katında −40 dB");
        let high = Biquad::highpass(120.0, std::f64::consts::FRAC_1_SQRT_2, RATE);
        assert!(db_at(&high, 2000.0).abs() < 0.05);
        assert!(db_at(&high, 12.0) < -39.0);
    }

    #[test]
    fn etkisiz_tasarimlar_sese_dokunmaz() {
        assert_eq!(Biquad::low_shelf(100.0, 0.0, 0.7, RATE), Biquad::IDENTITY);
        assert_eq!(Biquad::peaking(30_000.0, 6.0, 1.0, RATE), Biquad::IDENTITY);
        assert_eq!(Biquad::lowpass(24_000.0, 0.7, RATE), Biquad::IDENTITY);
        let mut state = [0.0; 2];
        assert_eq!(Biquad::IDENTITY.process(&mut state, 0.123), 0.123);
        let flat = Spec {
            kind: Kind::Peaking,
            freq_hz: 1000.0,
            gain_db: 0.0,
            q: 1.0,
        };
        assert_eq!(Biquad::matched(&flat, RATE), Biquad::IDENTITY);
    }

    #[test]
    fn isleme_tasarlanan_kazanci_verir() {
        let filter = Biquad::peaking(1000.0, 6.0, 1.0, RATE);
        let mut state = [0.0; 2];
        let w = 2.0 * PI * 1000.0 / RATE;
        let mut peak: f64 = 0.0;
        for i in 0..48_000 {
            let y = filter.process(&mut state, (w * f64::from(i)).sin());
            if i > 24_000 {
                peak = peak.max(y.abs());
            }
        }
        assert!((20.0 * peak.log10() - 6.0).abs() < 0.01, "{peak}");
    }

    /// Equalizer APO'nun formülleri (BP, NO, AP): elle hesaplanmış katsayılar.
    #[test]
    fn rbj_equalizer_apo_ile_ayni() {
        let w0 = 2.0 * PI * 1000.0 / RATE;
        let (sin, cos) = w0.sin_cos();
        let alpha = sin / (2.0 * 2.0);
        let a0 = 1.0 + alpha;
        let expect = |f: Biquad, b: [f64; 3]| {
            let got = [f.b0, f.b1, f.b2, f.a1, f.a2];
            let want = [
                b[0] / a0,
                b[1] / a0,
                b[2] / a0,
                -2.0 * cos / a0,
                (1.0 - alpha) / a0,
            ];
            for (g, w) in got.iter().zip(want) {
                assert!((g - w).abs() < 1e-15, "{got:?} != {want:?}");
            }
        };
        expect(Biquad::bandpass(1000.0, 2.0, RATE), [alpha, 0.0, -alpha]);
        expect(Biquad::notch(1000.0, 2.0, RATE), [1.0, -2.0 * cos, 1.0]);
        expect(
            Biquad::allpass(1000.0, 2.0, RATE),
            [1.0 - alpha, -2.0 * cos, 1.0 + alpha],
        );
    }

    const KINDS: [Kind; 8] = [
        Kind::Peaking,
        Kind::LowShelf,
        Kind::HighShelf,
        Kind::LowPass,
        Kind::HighPass,
        Kind::BandPass,
        Kind::Notch,
        Kind::AllPass,
    ];

    /// Geniş bir ayar ızgarası: her tür, 20 Hz – 0,45·fs, Q 0,3–8, kazanç ±18 dB.
    fn cases(rate: f64) -> Vec<Spec> {
        let mut out = Vec::new();
        for kind in KINDS {
            for i in 0..31 {
                let freq_hz = 20.0 * 1000f64.powf(f64::from(i) / 30.0);
                if freq_hz > 0.45 * rate {
                    continue;
                }
                for q in [0.3, 0.5, 0.707, 1.0, 2.0, 4.0, 8.0] {
                    let gains: &[f64] = if kind.has_gain() {
                        &[-18.0, -12.0, -6.0, -1.0, 1.0, 6.0, 12.0, 18.0]
                    } else {
                        &[0.0]
                    };
                    for &gain_db in gains {
                        out.push(Spec {
                            kind,
                            freq_hz,
                            gain_db,
                            q,
                        });
                    }
                }
            }
        }
        out
    }

    /// Analog eğriden en büyük sapma (dB, ağırlıklı), sık ızgarada (1000 nokta, 20 Hz – 20 kHz).
    fn dense_error(spec: &Spec, filter: &Biquad, rate: f64) -> f64 {
        dense_error_to(spec, filter, rate, 20_000.0)
    }

    /// Tasarım ölçütüyle aynı ağırlıkla ([`error_weight`]).
    fn dense_error_to(spec: &Spec, filter: &Biquad, rate: f64, top_hz: f64) -> f64 {
        let high = (rate / 2.0 * 0.999).min(top_hz);
        (0..1000)
            .map(|i| 20.0 * (high / 20.0).powf(f64::from(i) / 999.0))
            .filter(|&hz| spec.analog_db(hz) > FLOOR_DB)
            .map(|hz| {
                let reference = spec.analog_db(hz);
                (filter.response_db(2.0 * PI * hz / rate) - reference).abs()
                    * error_weight(reference)
            })
            .fold(0.0, f64::max)
    }

    /// Eşlenmiş tasarım her ayarda kararlı, en küçük fazlı ve RBJ'den uzak değil; ortalamada
    /// analog eğriye RBJ'den çok daha yakın.
    #[test]
    fn eslenmis_tasarim_analog_egriye_rbj_den_yakin() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            for kind in KINDS {
                let (mut sum_rbj, mut sum_matched, mut n) = (0.0, 0.0, 0.0);
                for spec in cases(rate).into_iter().filter(|s| s.kind == kind) {
                    let matched = Biquad::matched(&spec, rate);
                    let rbj = Biquad::rbj(&spec, rate);
                    assert!(matched.is_stable(), "{spec:?} @ {rate}");
                    // Tüm geçirenin sıfırları tanım gereği kutupların tersidir (çemberin dışında).
                    assert!(
                        kind == Kind::AllPass || matched.max_zero_radius() <= 1.0 + 1e-9,
                        "{spec:?} @ {rate}: en küçük fazlı değil"
                    );
                    let (e_m, e_r) = (
                        dense_error(&spec, &matched, rate),
                        dense_error(&spec, &rbj, rate),
                    );
                    assert!(
                        e_m <= e_r + 1.0,
                        "{spec:?} @ {rate}: eşlenmiş {e_m:.2} dB, RBJ {e_r:.2} dB"
                    );
                    sum_rbj += e_r;
                    sum_matched += e_m;
                    n += 1.0;
                }
                let (avg_r, avg_m) = (sum_rbj / n, sum_matched / n);
                assert!(
                    avg_m < avg_r * 0.7 || (avg_m < 0.05 && kind == Kind::AllPass),
                    "{kind:?} @ {rate}: ortalama eşlenmiş {avg_m:.3} dB, RBJ {avg_r:.3} dB"
                );
            }
        }
    }

    /// Duyulur bantta sık kullanılan ayarlarda (f ≤ 10 kHz, Q ≤ 4, ±12 dB) eşlenmiş tasarım
    /// 44,1 kHz'te bile analog eğriden en fazla ~2,5 dB, ortalamada 0,3 dB'den az sapar; RBJ
    /// aynı ayarlarda 27 dB'ye kadar sapar (alçak geçiren, Nyquist yakını).
    #[test]
    fn tipik_ayarlarda_eslenmis_tasarimin_sapmasi_kucuk() {
        let rate = 44_100.0;
        let typical: Vec<Spec> = cases(rate)
            .into_iter()
            .filter(|s| s.freq_hz <= 10_000.0 && s.q <= 4.0 && s.gain_db.abs() <= 12.0)
            .collect();
        let errors: Vec<(f64, Spec)> = typical
            .iter()
            .map(|s| (dense_error(s, &Biquad::matched(s, rate), rate), *s))
            .collect();
        let (worst, worst_spec) =
            errors
                .iter()
                .copied()
                .fold((0.0, typical[0]), |a, b| if b.0 > a.0 { b } else { a });
        let mean = errors.iter().map(|e| e.0).sum::<f64>() / errors.len() as f64;
        assert!(worst < 2.5, "en kötü {worst:.2} dB: {worst_spec:?}");
        assert!(mean < 0.3, "ortalama {mean:.3} dB");
        // Orta frekanslı (≤ 2 kHz, Q ≥ 0,7) süzgeçler 10 kHz'e kadar analogla aynı. (Daha geniş
        // süzgeçlerin ve bant geçirenin eteği Nyquist yakınında biraz sapar: analog eğri
        // Nyquist'ten sonra da sürer, ikinci dereceden sayısal süzgeç orada durur.)
        let mid: Vec<(f64, Spec)> = typical
            .iter()
            .filter(|s| s.freq_hz <= 2000.0 && s.q >= 0.7 && s.kind != Kind::BandPass)
            .map(|s| {
                (
                    dense_error_to(s, &Biquad::matched(s, rate), rate, 10_000.0),
                    *s,
                )
            })
            .collect();
        let mid_worst = mid
            .iter()
            .copied()
            .fold((0.0, typical[0]), |a, b| if b.0 > a.0 { b } else { a });
        let mid_mean = mid.iter().map(|e| e.0).sum::<f64>() / mid.len() as f64;
        assert!(
            mid_worst.0 < 0.2 && mid_mean < 0.02,
            "orta frekanslarda en kötü {:.3} dB ({:?}), ortalama {mid_mean:.4} dB",
            mid_worst.0,
            mid_worst.1
        );
    }

    /// Belgedeki tablo: `cargo test --lib biquad::tests::dogruluk_tablosu -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn dogruluk_tablosu() {
        for rate in [44_100.0, 48_000.0, 96_000.0] {
            println!("{rate} Hz: tür, RBJ ortalama / en kötü, eşlenmiş ortalama / en kötü (dB)");
            for kind in KINDS {
                let errors: Vec<(f64, f64)> = cases(rate)
                    .into_iter()
                    .filter(|s| s.kind == kind)
                    .map(|s| {
                        (
                            dense_error(&s, &Biquad::rbj(&s, rate), rate),
                            dense_error(&s, &Biquad::matched(&s, rate), rate),
                        )
                    })
                    .collect();
                let n = errors.len() as f64;
                let mean = |f: fn(&(f64, f64)) -> f64| errors.iter().map(f).sum::<f64>() / n;
                let worst = |f: fn(&(f64, f64)) -> f64| errors.iter().map(f).fold(0.0, f64::max);
                println!(
                    "  {kind:?}: {:.3} / {:.2}   {:.3} / {:.2}",
                    mean(|e| e.0),
                    worst(|e| e.0),
                    mean(|e| e.1),
                    worst(|e| e.1)
                );
            }
        }
    }

    #[test]
    fn eslenmis_tum_geciren_genlige_dokunmaz() {
        let spec = Spec {
            kind: Kind::AllPass,
            freq_hz: 3000.0,
            gain_db: 0.0,
            q: 0.9,
        };
        let filter = Biquad::matched(&spec, RATE);
        for hz in [20.0, 300.0, 3000.0, 15_000.0, 23_000.0] {
            assert!(db_at(&filter, hz).abs() < 1e-9);
        }
    }

    #[test]
    fn eslenmis_sesi_hesaplanan_egriyle_isler() {
        let spec = Spec {
            kind: Kind::HighShelf,
            freq_hz: 9000.0,
            gain_db: 6.0,
            q: 0.8,
        };
        let filter = Biquad::matched(&spec, 44_100.0);
        for hz in [1000.0, 9000.0, 16_000.0] {
            let w = 2.0 * PI * hz / 44_100.0;
            let mut state = [0.0; 2];
            let mut peak: f64 = 0.0;
            for i in 0..44_100 {
                let y = filter.process(&mut state, (w * f64::from(i)).sin());
                if i > 22_050 {
                    peak = peak.max(y.abs());
                }
            }
            let measured = 20.0 * peak.log10();
            assert!(
                (measured - filter.response_db(w)).abs() < 0.02,
                "{hz} Hz: {measured} != {}",
                filter.response_db(w)
            );
        }
        // Ve analog eğriye yakın: 16 kHz'te RBJ'nin sapması eşlenmişinkinden büyük.
        let rbj = Biquad::rbj(&spec, 44_100.0);
        let at = |f: &Biquad| {
            (f.response_db(2.0 * PI * 16_000.0 / 44_100.0) - spec.analog_db(16_000.0)).abs()
        };
        assert!(at(&filter) < at(&rbj), "{} !< {}", at(&filter), at(&rbj));
    }

    #[test]
    fn karmasik_yanit_genlikle_tutarli() {
        let filter = Biquad::peaking(2000.0, -5.0, 1.4, RATE);
        for hz in [50.0, 2000.0, 9000.0] {
            let w = 2.0 * PI * hz / RATE;
            let from_complex = 10.0 * filter.response(w).norm_sqr().log10();
            assert!((from_complex - filter.response_db(w)).abs() < 1e-9);
        }
    }
}
