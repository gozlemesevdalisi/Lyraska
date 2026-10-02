//! Gerçek kodeklerle (MP3, AAC/M4A, OGG Vorbis, FLAC) çözme ve sarma testleri.
//!
//! Test dosyaları arka arkaya üç ton içerir: 0–1 sn 440 Hz, 1–2 sn 880 Hz,
//! 2–3 sn 1760 Hz. Sarmadan sonra duyulan tonun frekansı, doğru yere gidilip
//! gidilmediğini gösterir. Ayrıntı: `tests/data/README.md`.

use std::path::PathBuf;

use lyraska_lib::audio::decode::Decoder;

const FORMATS: &[&str] = &["mp3", "m4a", "ogg", "flac"];
const RATE: f64 = 22_050.0;

fn fixture(ext: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(format!("uc-uc-ton.{ext}"))
}

/// Sonraki `seconds` saniyelik sesi (mono) çözer.
fn read_seconds(decoder: &mut Decoder, seconds: f64) -> Vec<f64> {
    let wanted = (seconds * RATE) as usize;
    let mut samples = Vec::with_capacity(wanted);
    while samples.len() < wanted {
        match decoder.next_chunk().unwrap() {
            Some(chunk) => samples.extend_from_slice(chunk),
            None => break,
        }
    }
    samples.truncate(wanted);
    samples
}

/// Sıfırdan geçiş sayısından baskın frekansı kestirir.
fn dominant_frequency(samples: &[f64]) -> f64 {
    let crossings = samples
        .windows(2)
        .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
        .count();
    crossings as f64 / 2.0 / (samples.len() as f64 / RATE)
}

fn assert_near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() / expected < 0.06,
        "{what}: {actual:.1} Hz, beklenen {expected} Hz"
    );
}

#[test]
fn her_bicim_acilir_ve_bilgileri_dogru() {
    for ext in FORMATS {
        let decoder = Decoder::open(&fixture(ext)).unwrap_or_else(|e| panic!("{ext}: {e}"));
        let info = decoder.info();
        assert_eq!(info.sample_rate, 22_050, "{ext}");
        assert_eq!(info.channels, 1, "{ext}");
        let duration = info
            .duration_secs
            .unwrap_or_else(|| panic!("{ext}: süre bilinmiyor"));
        assert!((duration - 3.0).abs() < 0.08, "{ext}: süre {duration}");
        assert_eq!(info.title.as_deref(), Some("Üç Ton"), "{ext}: başlık");
        assert_eq!(
            info.artist.as_deref(),
            Some("Lyraska Test"),
            "{ext}: sanatçı"
        );
    }
}

#[test]
fn her_bicim_bastan_sona_cozulur() {
    for ext in FORMATS {
        let mut decoder = Decoder::open(&fixture(ext)).unwrap();
        let samples = read_seconds(&mut decoder, 10.0);
        let seconds = samples.len() as f64 / RATE;
        assert!((seconds - 3.0).abs() < 0.08, "{ext}: {seconds} sn çözüldü");
        // Ortadaki ton: 1,2–1,8 sn arası 880 Hz.
        let middle = &samples[(1.2 * RATE) as usize..(1.8 * RATE) as usize];
        assert_near(dominant_frequency(middle), 880.0, ext);
    }
}

#[test]
fn her_bicimde_sarma_dogru_yere_gider() {
    for ext in FORMATS {
        let mut decoder = Decoder::open(&fixture(ext)).unwrap();

        // İleri: 2,2 sn → 1760 Hz bölümü
        decoder.seek(2.2).unwrap();
        let after = read_seconds(&mut decoder, 0.5);
        assert_near(dominant_frequency(&after), 1760.0, &format!("{ext} 2,2 sn"));

        // Geri: 0,2 sn → 440 Hz bölümü
        decoder.seek(0.2).unwrap();
        let after = read_seconds(&mut decoder, 0.5);
        assert_near(dominant_frequency(&after), 440.0, &format!("{ext} 0,2 sn"));

        // Bölüm sınırının hemen sonrası: 1,05 sn'den itibaren 880 Hz duyulmalı;
        // kaba bir sarma 440 Hz bölümüne düşerdi.
        decoder.seek(1.05).unwrap();
        let after = read_seconds(&mut decoder, 0.4);
        assert_near(dominant_frequency(&after), 880.0, &format!("{ext} 1,05 sn"));
    }
}
