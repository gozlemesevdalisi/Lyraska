//! İkinci dereceden süzgeç (biquad): RBJ "Audio EQ Cookbook" tasarımları, frekans
//! yanıtı ve devrik doğrudan biçim II ile işleme. Kulaklık düzeltmesi ([`super::peq`])
//! ve bas motoru ([`super::bass`]) kullanır. Hesaplar 64-bittir; bellek ayırmaz.

use std::f64::consts::PI;

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

    /// `w` açısal frekansındaki (radyan/örnek) kazanç (dB).
    pub fn response_db(&self, w: f64) -> f64 {
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

    /// Bir örneği işler (devrik doğrudan biçim II).
    #[inline]
    pub fn process(&self, state: &mut State, x: Sample) -> Sample {
        let y = self.b0 * x + state[0];
        state[0] = self.b1 * x - self.a1 * y + state[1];
        state[1] = self.b2 * x - self.a2 * y;
        y
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
}
