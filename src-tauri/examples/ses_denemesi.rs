//! Gerçek ses aygıtıyla uçtan uca deneme (CI'da Windows'ta çalışır).
//!
//! Kısa, kısık bir test sesi üretir; açar, duraklatır, sarar, çalarken ekolayzeri
//! değiştirir ve sonuna kadar çalar. Takılma, konum ve "bitti" durumunu denetler.
//! Makinede ses aygıtı yoksa bunu açıkça yazar ve başarıyla çıkar.
//!
//! Çalıştırma: `cargo run --example ses_denemesi`

// Deneme kodu: beklenmeyen bir hata denemeyi hemen durdurmalı.
#![allow(clippy::expect_used)]

use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use lyraska_lib::audio::eq::{EqSettings, BANDS};
use lyraska_lib::audio::player::{PlaybackState, Player};
use lyraska_lib::audio::AudioError;

const RATE: u32 = 44_100;
const SECONDS: f64 = 1.5;

fn main() -> ExitCode {
    let path = std::env::temp_dir().join("lyraska-ses-denemesi.wav");
    write_test_tone(&path);

    let result = run(&path);
    std::fs::remove_file(&path).ok();
    match result {
        Ok(()) => {
            println!("BAŞARILI: ses aygıtında uçtan uca çalma doğrulandı.");
            ExitCode::SUCCESS
        }
        Err(Outcome::NoDevice) => {
            println!(
                "ATLANDI: bu makinede kullanılabilir ses çıkışı yok; gerçek çalma denenemedi."
            );
            ExitCode::SUCCESS
        }
        Err(Outcome::Failed(message)) => {
            eprintln!("BAŞARISIZ: {message}");
            ExitCode::FAILURE
        }
    }
}

enum Outcome {
    NoDevice,
    Failed(String),
}

fn check(condition: bool, message: impl Into<String>) -> Result<(), Outcome> {
    if condition {
        Ok(())
    } else {
        Err(Outcome::Failed(message.into()))
    }
}

fn run(path: &Path) -> Result<(), Outcome> {
    let mut player = Player::new();
    match player.load(path, true) {
        Ok(info) => println!(
            "Açıldı: {} Hz, {} kanal, {:.2} sn",
            info.sample_rate,
            info.channels,
            info.duration_secs.unwrap_or_default()
        ),
        Err(AudioError::NoOutputDevice | AudioError::OutputUnavailable) => {
            return Err(Outcome::NoDevice)
        }
        Err(error) => return Err(Outcome::Failed(error.to_string())),
    }

    std::thread::sleep(Duration::from_millis(400));
    let status = player.status();
    println!("Çalıyor: konum {:.3} sn", status.position_secs);
    check(
        status.state == PlaybackState::Playing,
        format!("durum {:?}", status.state),
    )?;
    check(status.position_secs > 0.1, "konum ilerlemiyor")?;

    // Duraklat: aygıttaki kısa arabellek bittikten sonra konum donmalı.
    player.pause();
    std::thread::sleep(Duration::from_millis(250));
    let paused_at = player.status().position_secs;
    std::thread::sleep(Duration::from_millis(300));
    let still = player.status().position_secs;
    println!("Duraklatıldı: {paused_at:.3} sn → {still:.3} sn");
    check(
        (still - paused_at).abs() < 0.005,
        "duraklatılmışken konum ilerledi",
    )?;
    check(
        player.state() == PlaybackState::Paused,
        "duraklatma durumu yok",
    )?;

    // Sar: duraklatılmışken 0,9 saniyeye atla; konum orada beklemeli.
    player
        .seek(0.9)
        .map_err(|e| Outcome::Failed(e.to_string()))?;
    std::thread::sleep(Duration::from_millis(150));
    let seeked = player.status().position_secs;
    println!("Sarıldı: {seeked:.3} sn");
    check(
        (seeked - 0.9).abs() < 0.005,
        format!("sarma konumu {seeked:.3}"),
    )?;
    check(player.visual_now().is_some(), "görsel verisi yok")?;

    // Devam et; çalarken ekolayzeri değiştir (gerçek zamanlı yolda yeniden tasarım),
    // sonra kapat. Bunlar takılmaya yol açmamalı.
    player.play().map_err(|e| Outcome::Failed(e.to_string()))?;
    std::thread::sleep(Duration::from_millis(100));
    let mut gains = [0.0; BANDS];
    gains[3] = 9.0;
    gains[7] = -6.0;
    // Ekolayzer, bas düğmesi ve küçük hoparlör bası gerçek aygıtta da çalarken değişir.
    player.set_equalizer(EqSettings {
        enabled: true,
        gains_db: gains,
        bass_db: 9.0,
        small_speaker: true,
    });
    std::thread::sleep(Duration::from_millis(150));
    player.set_equalizer(EqSettings {
        enabled: false,
        gains_db: gains,
        bass_db: 9.0,
        small_speaker: true,
    });
    let deadline = Instant::now() + Duration::from_secs(10);
    while player.state() == PlaybackState::Playing && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let status = player.status();
    println!(
        "Son: durum {:?}, konum {:.3} sn, kesinti {}",
        status.state, status.position_secs, status.underruns
    );
    check(
        status.state == PlaybackState::Ended,
        format!("şarkı bitmedi: {:?}", status.error),
    )?;
    check(
        status.underruns == 0,
        format!("{} kez takıldı", status.underruns),
    )?;
    check(
        (status.position_secs - SECONDS).abs() < 0.01,
        format!(
            "son konum {:.3} sn, beklenen {SECONDS} sn",
            status.position_secs
        ),
    )?;
    Ok(())
}

/// 440 Hz, çok kısık (-26 dB) stereo test sesi; 16 bit WAV.
fn write_test_tone(path: &Path) {
    let frames = (f64::from(RATE) * SECONDS) as usize;
    let data_len = frames * 4;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&RATE.to_le_bytes());
    bytes.extend_from_slice(&(RATE * 4).to_le_bytes());
    bytes.extend_from_slice(&4u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for frame in 0..frames {
        let t = frame as f64 / f64::from(RATE);
        let value = ((2.0 * std::f64::consts::PI * 440.0 * t).sin() * 0.05 * 32767.0) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    std::fs::write(path, bytes).expect("test sesi yazılamadı");
}
