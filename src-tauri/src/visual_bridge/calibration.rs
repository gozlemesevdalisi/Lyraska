//! Senkron kalibrasyonu için tıklama kaydı (metronom).
//!
//! Kullanıcı bu kaydı normal oynatıcıyla dinler ve her tıklamayı duyduğu anda
//! Boşluk tuşuna basar. Basışların, tıklamaların şarkıdaki zamanından ne kadar
//! sonra geldiği sesin aygıtta ne kadar geç duyulduğunu verir (ör. Bluetooth
//! kulaklıkta 150–250 ms). Kayıt programın kendisi tarafından üretilir; dışarıdan
//! ses dosyası gerekmez.

use std::io::Write;
use std::path::Path;

/// Kaydın örnekleme hızı.
pub const RATE: u32 = 48_000;
/// İlk tıklamadan önceki sessizlik (saniye): kullanıcı hazırlansın.
pub const LEAD_IN_SECONDS: f64 = 2.0;
/// Tıklama aralığı (saniye; 100 BPM).
pub const INTERVAL_SECONDS: f64 = 0.6;
/// Tıklama sayısı.
pub const CLICKS: usize = 24;
/// Son tıklamadan sonraki sessizlik (saniye).
const TAIL_SECONDS: f64 = 1.0;
/// Tıklamanın tonu ve uzunluğu.
const CLICK_HZ: f64 = 1_000.0;
const CLICK_SECONDS: f64 = 0.03;
/// Kısık tutulur: kulaklıkta rahatsız etmesin (−12 dBFS civarı).
const CLICK_LEVEL: f64 = 0.25;

/// Tıklamaların kayıttaki zamanları (saniye; tıklamanın başladığı an).
pub fn click_times() -> Vec<f64> {
    (0..CLICKS)
        .map(|i| LEAD_IN_SECONDS + i as f64 * INTERVAL_SECONDS)
        .collect()
}

/// Kaydın örnekleri (mono, −1..1). Her tıklama keskin başlayan, hızla sönen bir ton.
pub fn samples() -> Vec<f64> {
    let total = LEAD_IN_SECONDS + (CLICKS - 1) as f64 * INTERVAL_SECONDS + TAIL_SECONDS;
    let mut out = vec![0.0; (total * f64::from(RATE)).round() as usize];
    let length = (CLICK_SECONDS * f64::from(RATE)) as usize;
    for start in click_times() {
        let first = (start * f64::from(RATE)).round() as usize;
        for n in 0..length {
            let t = n as f64 / f64::from(RATE);
            // Sönüm: 30 ms içinde ~−40 dB; başta 0,3 ms'lik yumuşak giriş (tık sesi olmasın).
            let envelope = (t / 0.0003).min(1.0) * (-t / 0.0065).exp();
            if let Some(slot) = out.get_mut(first + n) {
                *slot = CLICK_LEVEL * envelope * (2.0 * std::f64::consts::PI * CLICK_HZ * t).sin();
            }
        }
    }
    out
}

/// Tıklama kaydını 16 bit PCM WAV olarak yazar.
pub fn write(path: &Path) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let samples = samples();
    let data_len = (samples.len() * 2) as u32;
    let mut bytes = Vec::with_capacity(44 + samples.len() * 2);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_len).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&1u16.to_le_bytes()); // mono
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        let value = (sample.clamp(-1.0, 1.0) * 32767.0).round() as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::File::create(path)?.write_all(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::decode::Decoder;
    use crate::audio::test_util::temp_path;

    #[test]
    fn tiklamalar_belirtilen_anlarda_baslar() {
        let samples = samples();
        let rate = f64::from(RATE);
        // Her tıklamanın ilk duyulur örneği beklenen andan en fazla 1 ms sonra.
        for time in click_times() {
            let first = (time * rate) as usize;
            let onset = (first..first + 480)
                .find(|&i| samples[i].abs() > 0.01)
                .expect("tıklama var");
            assert!(
                onset - first < 48,
                "{time} sn: {} örnek gecikme",
                onset - first
            );
            // Tıklamadan hemen önce sessizlik.
            assert!(samples[first.saturating_sub(100)..first]
                .iter()
                .all(|s| *s == 0.0));
        }
        let peak = samples.iter().fold(0.0f64, |m, s| m.max(s.abs()));
        assert!(peak <= CLICK_LEVEL && peak > CLICK_LEVEL * 0.8);
    }

    #[test]
    fn yazilan_kayit_oynaticida_acilir() {
        let path = temp_path("tiklama.wav");
        write(&path).unwrap();
        let decoder = Decoder::open(&path).unwrap();
        let info = decoder.info();
        assert_eq!(info.sample_rate, RATE);
        let expected = LEAD_IN_SECONDS + (CLICKS - 1) as f64 * INTERVAL_SECONDS + TAIL_SECONDS;
        assert!((info.duration_secs.unwrap() - expected).abs() < 0.01);
        std::fs::remove_file(path).ok();
    }
}
