//! Kanal seviyeleri: VU ibreleri için her karede sol ve sağ kanalın etkin (RMS)
//! ve tepe seviyesi.
//!
//! Şarkı önceden tarandığı için genel yüksekliği de bilinir: VU ölçeğinin 0
//! noktası ("referans") şarkıya göre ayarlanır. Eski kayıtlar da bugünün çok
//! yüksek seviyede basılmış kayıtları da ibreyi aynı güzellikte oynatır; yüksek
//! bölümler 0 VU civarında, en güçlü anlar kırmızı bölgede gezinir.

use crate::audio::Sample;

/// Bir karede saklanan değer sayısı: sol/sağ RMS, sol/sağ tepe.
pub const VALUES_PER_FRAME: usize = 4;
/// Saklanan en düşük seviye (dBFS); bunun altı sessizlik sayılır.
pub const FLOOR_DB: f32 = -60.0;
/// Saklama çözünürlüğü: desibel başına adım (0,25 dB).
const STEPS_PER_DB: f32 = 4.0;
/// VU ibresinin tümleme süresi (standart: 300 ms).
const VU_WINDOW_SECONDS: f64 = 0.3;
/// Referans: 300 ms'lik seviyelerin %95'i bunun altında kalır.
const REFERENCE_PERCENTILE: f64 = 0.95;
/// Bu seviyenin altındaki anlar (sessiz girişler, aralar) referansa katılmaz.
const REFERENCE_GATE_DB: f64 = -50.0;

/// Bir anın kanal seviyeleri (dBFS). Sessizlik [`FLOOR_DB`] olarak gelir.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelLevels {
    pub rms_db: [f32; 2],
    pub peak_db: [f32; 2],
}

/// Bir analiz karesi boyunca örnekleri toplar.
#[derive(Debug, Default)]
pub struct LevelAccumulator {
    sum_sq: [f64; 2],
    peak: [f64; 2],
    count: usize,
}

impl LevelAccumulator {
    /// Bir kareyi (her kanaldan bir örnek) ekler. Tek kanallı seste iki taraf aynıdır;
    /// ikiden çok kanalda ilk ikisi (sol, sağ) kullanılır.
    pub fn add(&mut self, frame: &[Sample]) {
        let left = frame.first().copied().unwrap_or(0.0);
        let right = frame.get(1).copied().unwrap_or(left);
        for (i, sample) in [left, right].into_iter().enumerate() {
            self.sum_sq[i] += sample * sample;
            self.peak[i] = self.peak[i].max(sample.abs());
        }
        self.count += 1;
    }

    /// Karenin seviyelerini saklama biçiminde döndürür ve sıfırlanır.
    pub fn finish(&mut self) -> [u8; VALUES_PER_FRAME] {
        let n = self.count.max(1) as f64;
        let rms = self.sum_sq.map(|s| amplitude_db(s / n, 10.0));
        let peak = self.peak.map(|p| amplitude_db(p, 20.0));
        *self = Self::default();
        [
            encode_db(rms[0]),
            encode_db(rms[1]),
            encode_db(peak[0]),
            encode_db(peak[1]),
        ]
    }
}

/// Güç (`factor` = 10) ya da genlik (`factor` = 20) değerini desibele çevirir.
fn amplitude_db(value: f64, factor: f64) -> f64 {
    if value <= 0.0 {
        f64::NEG_INFINITY
    } else {
        factor * value.log10()
    }
}

fn encode_db(db: f64) -> u8 {
    let steps = ((db as f32 - FLOOR_DB) * STEPS_PER_DB).round();
    steps.clamp(0.0, f32::from(u8::MAX)) as u8
}

fn decode_db(value: u8) -> f32 {
    FLOOR_DB + f32::from(value) / STEPS_PER_DB
}

/// Saklanan bir kareyi çözer.
pub fn decode_frame(frame: &[u8]) -> Option<ChannelLevels> {
    match *frame {
        [rms_l, rms_r, peak_l, peak_r, ..] => Some(ChannelLevels {
            rms_db: [decode_db(rms_l), decode_db(rms_r)],
            peak_db: [decode_db(peak_l), decode_db(peak_r)],
        }),
        _ => None,
    }
}

/// Bütün karelerden VU referansını (0 VU'ya denk gelen dBFS) hesaplar.
/// Şarkı baştan sona sessizse `None`.
pub fn vu_reference_db(frames: &[u8], frames_per_second: f64) -> Option<f32> {
    // Her karenin gücü (yüksek olan kanal).
    let power: Vec<f64> = frames
        .chunks_exact(VALUES_PER_FRAME)
        .map(|f| {
            let loudest = decode_db(f[0].max(f[1]));
            if f[0].max(f[1]) == 0 {
                0.0
            } else {
                10f64.powf(f64::from(loudest) / 10.0)
            }
        })
        .collect();
    let window = ((VU_WINDOW_SECONDS * frames_per_second).round() as usize).max(1);
    if power.len() < window {
        return None;
    }
    // 300 ms'lik kayan ortalama: VU ibresinin gördüğü seviye.
    let mut sum: f64 = power[..window].iter().sum();
    let mut levels = Vec::with_capacity(power.len());
    for i in window..=power.len() {
        let db = 10.0 * (sum / window as f64).max(1e-30).log10();
        if db > REFERENCE_GATE_DB {
            levels.push(db);
        }
        if i < power.len() {
            sum += power[i] - power[i - window];
        }
    }
    if levels.is_empty() {
        return None;
    }
    levels.sort_by(f64::total_cmp);
    let index = ((levels.len() - 1) as f64 * REFERENCE_PERCENTILE).round() as usize;
    Some(levels[index] as f32)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames_of(levels: &[[f64; 2]]) -> Vec<u8> {
        levels
            .iter()
            .flat_map(|&[l, r]| [encode_db(l), encode_db(r), encode_db(l), encode_db(r)])
            .collect()
    }

    #[test]
    fn tam_olcekli_sinusun_rms_ve_tepesi() {
        let mut acc = LevelAccumulator::default();
        let n = 4800;
        for i in 0..n {
            let s = (2.0 * std::f64::consts::PI * 1000.0 * i as f64 / 48_000.0).sin();
            acc.add(&[s, 0.5 * s]);
        }
        let levels = decode_frame(&acc.finish()).unwrap();
        // Sinüsün RMS'i tepesinin 3 dB altındadır.
        assert!((levels.rms_db[0] + 3.0).abs() < 0.2, "{levels:?}");
        assert!((levels.rms_db[1] + 9.0).abs() < 0.2, "{levels:?}");
        assert!(levels.peak_db[0].abs() < 0.2);
        assert!((levels.peak_db[1] + 6.0).abs() < 0.2);
        // Toplayıcı sıfırlandı: boş kare sessizdir.
        assert_eq!(decode_frame(&acc.finish()).unwrap().rms_db, [FLOOR_DB; 2]);
    }

    #[test]
    fn tek_kanalda_iki_taraf_ayni() {
        let mut acc = LevelAccumulator::default();
        acc.add(&[0.5]);
        acc.add(&[-0.5]);
        let levels = decode_frame(&acc.finish()).unwrap();
        assert_eq!(levels.rms_db[0], levels.rms_db[1]);
        assert!((levels.peak_db[0] + 6.02).abs() < 0.2);
    }

    #[test]
    fn seviyeler_sinirlarda_kirpilir() {
        assert_eq!(decode_db(encode_db(f64::NEG_INFINITY)), FLOOR_DB);
        assert_eq!(decode_db(encode_db(-100.0)), FLOOR_DB);
        assert_eq!(decode_db(encode_db(6.0)), FLOOR_DB + 255.0 / STEPS_PER_DB);
        assert!((decode_db(encode_db(-12.3)) + 12.25).abs() < 0.13);
        assert!(decode_frame(&[1, 2, 3]).is_none());
    }

    #[test]
    fn referans_sarkinin_yuksek_bolumlerine_gore() {
        // 10 sn: 2 sn sessiz giriş, 6 sn -20 dB, 2 sn -8 dB (nakarat).
        let mut levels = vec![[f64::NEG_INFINITY; 2]; 120];
        levels.extend(vec![[-20.0, -21.0]; 360]);
        levels.extend(vec![[-9.0, -8.0]; 120]);
        let reference = vu_reference_db(&frames_of(&levels), 60.0).unwrap();
        // Sessiz giriş katılmaz; %95'lik dilim nakarata düşer.
        assert!((reference + 8.0).abs() < 0.3, "{reference}");

        // Aynı şarkı 10 dB daha kısık basılsa referans da 10 dB iner: ibre aynı oynar.
        let quiet: Vec<[f64; 2]> = levels.iter().map(|&[l, r]| [l - 10.0, r - 10.0]).collect();
        let quiet_ref = vu_reference_db(&frames_of(&quiet), 60.0).unwrap();
        assert!((reference - quiet_ref - 10.0).abs() < 0.3);
    }

    #[test]
    fn sessiz_ya_da_cok_kisa_sarkida_referans_yok() {
        let silent = frames_of(&vec![[f64::NEG_INFINITY; 2]; 600]);
        assert_eq!(vu_reference_db(&silent, 60.0), None);
        let short = frames_of(&[[-10.0, -10.0]; 5]);
        assert_eq!(vu_reference_db(&short, 60.0), None);
    }
}
