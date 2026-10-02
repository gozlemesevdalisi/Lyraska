//! Testlerde kullanılan yardımcılar: geçici dosya yolu ve WAV üretici.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

/// Testler paralel çalıştığı için her çağrıda benzersiz bir geçici dosya yolu üretir.
pub fn temp_path(name: &str) -> PathBuf {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir()
        .join(format!("lyraska-test-{}", std::process::id()))
        .join(n.to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Belirli bir frekansta sinüs örneği (-1..1).
pub fn sine(freq: f64, rate: u32, frame: usize) -> f64 {
    (2.0 * std::f64::consts::PI * freq * frame as f64 / f64::from(rate)).sin()
}

/// 16 bit PCM WAV dosyası yazar. `sample(kare, kanal)` -1..1 aralığında değer döndürür.
pub fn write_wav(
    path: &Path,
    rate: u32,
    channels: u16,
    frames: usize,
    sample: impl Fn(usize, usize) -> f64,
) {
    let data_len = frames * usize::from(channels) * 2;
    let mut bytes = Vec::with_capacity(44 + data_len);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&((36 + data_len) as u32).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes()); // PCM
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
    bytes.extend_from_slice(&(channels * 2).to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&(data_len as u32).to_le_bytes());
    for frame in 0..frames {
        for channel in 0..usize::from(channels) {
            let value = (sample(frame, channel).clamp(-1.0, 1.0) * 32767.0).round() as i16;
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    std::fs::File::create(path)
        .unwrap()
        .write_all(&bytes)
        .unwrap();
}
