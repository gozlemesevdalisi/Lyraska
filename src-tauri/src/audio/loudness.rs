//! Ses yüksekliği ölçümü (ITU-R BS.1770-4 / EBU R128) ve eşitleme kazancı.
//!
//! Her şarkının **bütünleşik ses yüksekliği** (LUFS: kulağın duyduğu ortalama yükseklik)
//! ve **gerçek tepe** seviyesi (dBTP: örnekler arasındaki tepeler dahil) analizde bir kez
//! ölçülür. Çalarken şarkı hedef yüksekliğe getirilir: bütün şarkılar aynı yükseklikte
//! çalar ve çok yüksek kaydedilmiş şarkılar kısıldığı için ekolayzere yer açılır.
//!
//! Ölçüm adımları (BS.1770-4):
//! 1. K-ağırlıklı süzgeç: kulağın tize duyarlılığını taklit eden raf + çok alçağı kesen süzgeç.
//! 2. 400 ms'lik bloklar (100 ms aralıkla, %75 örtüşen) için kanal ağırlıklı ortalama kare.
//! 3. Mutlak kapı (−70 LUFS) ve göreli kapı (kapıdan geçenlerin yüksekliği − 10 LU):
//!    sessiz bölümler ortalamayı düşürmez.
//!
//! Gerçek tepe, sinyal 4 kat (96 kHz üstünde 2 kat) sık örneklenerek bulunur: dijital
//! örnekler 0 dBFS'in altında kalsa da aralarında yeniden oluşan dalga tepesi aşabilir.

use std::f64::consts::PI;

use serde::{Deserialize, Serialize};

use super::Sample;

/// Eşitleme hedefi (LUFS). Yayın ve akış servislerinin ortak düzeyi.
pub const TARGET_LUFS: f64 = -14.0;
/// Eşitleme yükseltirken gerçek tepenin geçmeyeceği sınır (dBTP).
pub const PEAK_CEILING_DBTP: f64 = -1.0;
/// Mutlak kapı: bundan sessiz bloklar ölçüme girmez.
const ABSOLUTE_GATE_LUFS: f64 = -70.0;
/// Göreli kapı: ilk ortalamanın bu kadar altındaki bloklar ölçüme girmez.
const RELATIVE_GATE_LU: f64 = -10.0;
/// BS.1770'te K-ağırlıklı ortalama karenin LUFS'e çevrilmesindeki sabit.
const OFFSET_DB: f64 = -0.691;
/// Gerçek tepe için ara değerleme süzgecinin her fazındaki katsayı sayısı.
const TAPS_PER_PHASE: usize = 12;

/// Bir şarkının ölçülen ses yüksekliği.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Loudness {
    /// Bütünleşik ses yüksekliği (LUFS).
    pub integrated_lufs: f64,
    /// Gerçek tepe (dBTP).
    pub true_peak_dbtp: f64,
}

impl Loudness {
    /// Şarkıyı `target` yüksekliğine getiren kazanç (dB). Kısma her zaman tam yapılır;
    /// yükseltme, gerçek tepe [`PEAK_CEILING_DBTP`]'yi geçmeyecek kadar yapılır (sessiz
    /// kaydedilmiş ama tepeleri yüksek şarkılar bozulmasın).
    pub fn gain_db(&self, target: f64) -> f64 {
        let gain = target - self.integrated_lufs;
        if gain <= 0.0 {
            return gain;
        }
        gain.min((PEAK_CEILING_DBTP - self.true_peak_dbtp).max(0.0))
    }
}

/// İkinci dereceden süzgeç (çift doğrusal dönüşümle tasarlanmış), doğrudan biçim I.
#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    x: [f64; 2],
    y: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self {
            b,
            a,
            x: [0.0; 2],
            y: [0.0; 2],
        }
    }

    fn process(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0]
            - self.a[1] * self.y[1];
        self.x = [x, self.x[0]];
        self.y = [y, self.y[0]];
        y
    }
}

/// K-ağırlıklı süzgecin iki aşaması, her örnekleme hızı için (48 kHz'te BS.1770'teki
/// katsayıların aynısını verir; libebur128 ile aynı analog örnekten türetilir).
fn k_weighting(sample_rate: f64) -> [Biquad; 2] {
    // 1. aşama: ~1,7 kHz üstünü +4 dB yükselten raf (başın akustik etkisi).
    let (f0, gain_db, q) = (
        1_681.974_450_955_533,
        3.999_843_853_973_347,
        0.707_175_236_955_419_6,
    );
    let k = (PI * f0 / sample_rate).tan();
    let vh = 10f64.powf(gain_db / 20.0);
    let vb = vh.powf(0.499_666_774_154_541_6);
    let a0 = 1.0 + k / q + k * k;
    let shelf = Biquad::new(
        [
            (vh + vb * k / q + k * k) / a0,
            2.0 * (k * k - vh) / a0,
            (vh - vb * k / q + k * k) / a0,
        ],
        [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    // 2. aşama: ~38 Hz altını kesen süzgeç (RLB).
    let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
    let k = (PI * f0 / sample_rate).tan();
    let a0 = 1.0 + k / q + k * k;
    let highpass = Biquad::new(
        [1.0, -2.0, 1.0],
        [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
    );
    [shelf, highpass]
}

/// Bir kanalın gerçek tepesini bulan sık örnekleyici (çok fazlı, pencereli sinc).
#[derive(Debug, Clone)]
struct TruePeak {
    /// `phases[p][t]`: p. ara noktanın t. katsayısı.
    phases: Vec<[f64; TAPS_PER_PHASE]>,
    /// Bir fazın mutlak katsayı toplamı (en büyüğü): ara değer bunu aşamaz.
    max_gain: f64,
    history: [f64; TAPS_PER_PHASE],
    head: usize,
    peak: f64,
}

impl TruePeak {
    fn new(sample_rate: f64) -> Self {
        let factor = if sample_rate < 96_000.0 {
            4
        } else if sample_rate < 192_000.0 {
            2
        } else {
            1
        };
        let len = factor * TAPS_PER_PHASE;
        let center = (len as f64 - 1.0) / 2.0;
        let mut phases = vec![[0.0; TAPS_PER_PHASE]; factor];
        for n in 0..len {
            let x = (n as f64 - center) / factor as f64;
            let sinc = if x.abs() < 1e-12 {
                1.0
            } else {
                (PI * x).sin() / (PI * x)
            };
            // Blackman penceresi: kısa süzgeçte geçirme bandı düz kalır.
            let w = 0.42 - 0.5 * (2.0 * PI * n as f64 / (len as f64 - 1.0)).cos()
                + 0.08 * (4.0 * PI * n as f64 / (len as f64 - 1.0)).cos();
            phases[n % factor][n / factor] = sinc * w;
        }
        // Her faz tek başına birim kazançlı olsun (düz sinyal aynen kalsın).
        for phase in &mut phases {
            let sum: f64 = phase.iter().sum();
            if sum.abs() > 1e-12 {
                for c in phase.iter_mut() {
                    *c /= sum;
                }
            }
        }
        let max_gain = phases
            .iter()
            .map(|p| p.iter().map(|c| c.abs()).sum::<f64>())
            .fold(0.0, f64::max);
        Self {
            phases,
            max_gain,
            history: [0.0; TAPS_PER_PHASE],
            head: 0,
            peak: 0.0,
        }
    }

    fn add(&mut self, x: f64) {
        self.history[self.head] = x;
        self.head = (self.head + 1) % TAPS_PER_PHASE;
        self.peak = self.peak.max(x.abs());
        // Ara değerler pencerenin en büyük örneğinin `max_gain` katını aşamaz: tepeye
        // yaklaşamayacak pencereler hesaplanmaz (ölçümü çok hızlandırır).
        let window_max = self.history.iter().fold(0.0f64, |m, v| m.max(v.abs()));
        if window_max * self.max_gain <= self.peak {
            return;
        }
        for phase in &self.phases {
            let mut sum = 0.0;
            for (t, &c) in phase.iter().enumerate() {
                // t = 0 en yeni örnek.
                let index = (self.head + TAPS_PER_PHASE - 1 - t) % TAPS_PER_PHASE;
                sum += c * self.history[index];
            }
            self.peak = self.peak.max(sum.abs());
        }
    }
}

/// Şarkı boyunca ses yüksekliğini ve gerçek tepeyi ölçer (analiz iş parçacığında).
#[derive(Debug, Clone)]
pub struct LoudnessMeter {
    /// Kanal başına K-ağırlıklı süzgeç ve ağırlık.
    filters: Vec<[Biquad; 2]>,
    weights: Vec<f64>,
    peaks: Vec<TruePeak>,
    /// 100 ms'lik alt blok uzunluğu (kare).
    hop: usize,
    /// Süren alt bloğun ağırlıklı kare toplamı ve kare sayısı.
    sub_sum: f64,
    sub_frames: usize,
    /// Son dört alt bloğun toplamları (400 ms'lik blok bunlardan oluşur).
    recent: [f64; 4],
    recent_count: usize,
    /// 400 ms'lik blokların ortalama kareleri.
    blocks: Vec<f64>,
    /// Blok oluşmayacak kadar kısa sesler için bütün sesin toplamı.
    total_sum: f64,
    total_frames: usize,
}

impl LoudnessMeter {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let rate = f64::from(sample_rate.max(1));
        let channels = channels.max(1);
        // BS.1770 kanal ağırlıkları: ön kanallar 1, arka (çevre) kanallar 1,41; LFE (4.) yok.
        // Tek kanallı şarkı iki hoparlörden aynı çalınır: iki kanal sayılır.
        let weights = (0..channels)
            .map(|c| match (channels, c) {
                (1, _) => 2.0,
                (_, 3) if channels >= 6 => 0.0,
                (_, 4 | 5) if channels >= 6 => 1.41,
                _ => 1.0,
            })
            .collect();
        Self {
            filters: vec![k_weighting(rate); channels],
            weights,
            peaks: vec![TruePeak::new(rate); channels],
            hop: ((rate * 0.1).round() as usize).max(1),
            sub_sum: 0.0,
            sub_frames: 0,
            recent: [0.0; 4],
            recent_count: 0,
            blocks: Vec::new(),
            total_sum: 0.0,
            total_frames: 0,
        }
    }

    /// Bir kare (her kanaldan bir örnek) ekler.
    pub fn add(&mut self, frame: &[Sample]) {
        let mut energy = 0.0;
        for (channel, &x) in frame.iter().enumerate().take(self.filters.len()) {
            let x = if x.is_finite() { x } else { 0.0 };
            self.peaks[channel].add(x);
            let [shelf, highpass] = &mut self.filters[channel];
            let y = highpass.process(shelf.process(x));
            energy += self.weights[channel] * y * y;
        }
        self.sub_sum += energy;
        self.sub_frames += 1;
        self.total_sum += energy;
        self.total_frames += 1;
        if self.sub_frames == self.hop {
            self.recent.rotate_left(1);
            self.recent[3] = self.sub_sum;
            self.recent_count += 1;
            self.sub_sum = 0.0;
            self.sub_frames = 0;
            if self.recent_count >= 4 {
                self.blocks
                    .push(self.recent.iter().sum::<f64>() / (4 * self.hop) as f64);
            }
        }
    }

    /// Ölçümü bitirir. Sessiz (ya da boş) seste `None`.
    pub fn finish(&self) -> Option<Loudness> {
        let to_lufs = |z: f64| OFFSET_DB + 10.0 * z.log10();
        let integrated = if self.blocks.is_empty() {
            // 400 ms'den kısa ses: bütünü tek blok sayılır.
            if self.total_frames == 0 {
                return None;
            }
            let z = self.total_sum / self.total_frames as f64;
            (to_lufs(z) > ABSOLUTE_GATE_LUFS).then(|| to_lufs(z))?
        } else {
            let gated: Vec<f64> = self
                .blocks
                .iter()
                .copied()
                .filter(|&z| z > 0.0 && to_lufs(z) > ABSOLUTE_GATE_LUFS)
                .collect();
            if gated.is_empty() {
                return None;
            }
            let relative =
                to_lufs(gated.iter().sum::<f64>() / gated.len() as f64) + RELATIVE_GATE_LU;
            let kept: Vec<f64> = gated
                .into_iter()
                .filter(|&z| to_lufs(z) > relative)
                .collect();
            if kept.is_empty() {
                return None;
            }
            to_lufs(kept.iter().sum::<f64>() / kept.len() as f64)
        };
        let peak = self.peaks.iter().map(|p| p.peak).fold(0.0, f64::max);
        Some(Loudness {
            integrated_lufs: integrated,
            true_peak_dbtp: 20.0 * peak.max(1e-12).log10(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// `parts`: (saniye, dBFS) bölümleri; her bölüm 1 kHz sinüs, `channels` kanalda aynı.
    fn measure(rate: u32, channels: usize, freq: f64, parts: &[(f64, f64)]) -> Loudness {
        let mut meter = LoudnessMeter::new(rate, channels);
        let mut n = 0u64;
        for &(seconds, db) in parts {
            let amplitude = 10f64.powf(db / 20.0);
            for _ in 0..(seconds * f64::from(rate)) as usize {
                let x = amplitude * (2.0 * PI * freq * n as f64 / f64::from(rate)).sin();
                n += 1;
                meter.add(&vec![x; channels]);
            }
        }
        meter.finish().expect("ses var")
    }

    #[test]
    fn k_agirlikli_suzgec_48_khzde_standart_katsayilari_verir() {
        // BS.1770-4, Tablo 1 ve 2.
        let [shelf, highpass] = k_weighting(48_000.0);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-8;
        assert!(close(shelf.b[0], 1.535_124_859_586_97));
        assert!(close(shelf.b[1], -2.691_696_189_406_38));
        assert!(close(shelf.b[2], 1.198_392_810_852_85));
        assert!(close(shelf.a[0], -1.690_659_293_182_41));
        assert!(close(shelf.a[1], 0.732_480_774_215_85));
        assert!(close(highpass.a[0], -1.990_047_454_833_98));
        assert!(close(highpass.a[1], 0.990_072_250_366_21));
    }

    #[test]
    fn ebu_3341_durum_1_ve_2_sabit_sinus() {
        // Stereo 1 kHz sinüs −23 dBFS → −23,0 LUFS; −33 dBFS → −33,0 LUFS (±0,1).
        let a = measure(RATE, 2, 1000.0, &[(20.0, -23.0)]);
        assert!((a.integrated_lufs + 23.0).abs() < 0.1, "{a:?}");
        let b = measure(RATE, 2, 1000.0, &[(20.0, -33.0)]);
        assert!((b.integrated_lufs + 33.0).abs() < 0.1, "{b:?}");
    }

    #[test]
    fn ebu_3341_durum_3_4_5_kapilar() {
        // Durum 3: −36 / −23 / −36 dBFS (10 / 60 / 10 sn) → −23,0 LUFS (göreli kapı).
        let c3 = measure(
            RATE,
            2,
            1000.0,
            &[(10.0, -36.0), (60.0, -23.0), (10.0, -36.0)],
        );
        assert!((c3.integrated_lufs + 23.0).abs() < 0.1, "{c3:?}");
        // Durum 4: −72 / −36 / −23 / −36 / −72 dBFS → −23,0 LUFS (mutlak kapı da).
        let c4 = measure(
            RATE,
            2,
            1000.0,
            &[
                (10.0, -72.0),
                (10.0, -36.0),
                (60.0, -23.0),
                (10.0, -36.0),
                (10.0, -72.0),
            ],
        );
        assert!((c4.integrated_lufs + 23.0).abs() < 0.1, "{c4:?}");
        // Durum 5: −26 / −20 / −26 dBFS (20 / 20,1 / 20 sn) → −23,0 LUFS.
        let c5 = measure(
            RATE,
            2,
            1000.0,
            &[(20.0, -26.0), (20.1, -20.0), (20.0, -26.0)],
        );
        assert!((c5.integrated_lufs + 23.0).abs() < 0.1, "{c5:?}");
    }

    #[test]
    fn her_ornekleme_hizinda_ayni_sonuc() {
        for rate in [22_050, 44_100, 48_000, 96_000, 192_000] {
            let l = measure(rate, 2, 1000.0, &[(10.0, -23.0)]);
            assert!((l.integrated_lufs + 23.0).abs() < 0.1, "{rate} Hz: {l:?}");
        }
    }

    #[test]
    fn tek_kanalli_sarki_iki_hoparlorde_calindigi_gibi_olculur() {
        let mono = measure(RATE, 1, 1000.0, &[(10.0, -23.0)]);
        let stereo = measure(RATE, 2, 1000.0, &[(10.0, -23.0)]);
        assert!((mono.integrated_lufs - stereo.integrated_lufs).abs() < 0.01);
    }

    #[test]
    fn sessizlik_olculmez() {
        let mut meter = LoudnessMeter::new(RATE, 2);
        for _ in 0..RATE {
            meter.add(&[0.0, 0.0]);
        }
        assert_eq!(meter.finish(), None);
        assert_eq!(LoudnessMeter::new(RATE, 2).finish(), None);
    }

    #[test]
    fn gercek_tepe_ornekler_arasindaki_tepeyi_bulur() {
        // fs/4'te 45° kaydırılmış tam ölçekli sinüs: örnekler ±0,707'de (−3,01 dBFS),
        // dalganın tepesi 0 dBTP'dir.
        let mut meter = LoudnessMeter::new(RATE, 1);
        for n in 0..RATE {
            let x = (PI / 2.0 * f64::from(n) + PI / 4.0).sin();
            meter.add(&[x]);
        }
        let l = meter.finish().unwrap();
        assert!(l.true_peak_dbtp > -0.3 && l.true_peak_dbtp < 0.2, "{l:?}");
        // Alçak frekansta gerçek tepe örnek tepesiyle aynıdır.
        let low = measure(RATE, 2, 997.0, &[(2.0, -6.0)]);
        assert!((low.true_peak_dbtp + 6.0).abs() < 0.05, "{low:?}");
    }

    #[test]
    fn esitleme_kazanci() {
        let loud = Loudness {
            integrated_lufs: -8.0,
            true_peak_dbtp: 0.0,
        };
        assert_eq!(loud.gain_db(TARGET_LUFS), -6.0, "yüksek şarkı kısılır");
        let quiet = Loudness {
            integrated_lufs: -20.0,
            true_peak_dbtp: -10.0,
        };
        assert_eq!(
            quiet.gain_db(TARGET_LUFS),
            6.0,
            "boşluğu olan sessiz şarkı yükseltilir"
        );
        let dynamic = Loudness {
            integrated_lufs: -20.0,
            true_peak_dbtp: -3.0,
        };
        assert_eq!(dynamic.gain_db(TARGET_LUFS), 2.0, "tepe −1 dBTP'yi geçmez");
        let clipped = Loudness {
            integrated_lufs: -16.0,
            true_peak_dbtp: 0.5,
        };
        assert_eq!(
            clipped.gain_db(TARGET_LUFS),
            0.0,
            "tepesi zaten yüksekse yükseltilmez"
        );
    }
}
