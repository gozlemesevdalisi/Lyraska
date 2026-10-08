//! Yerel hata ve çökme günlüğü.
//!
//! Program uygulama veri klasöründe `logs/lyraska.log` dosyasına yazar: açılış
//! bilgisi (sürüm, işletim sistemi), kullanıcıya gösterilen hatalar, ses
//! motorunun hataları ve çökmeler (panic: mesaj, yer, yığın izi). Dosya hiçbir
//! yere gönderilmez; kullanıcı "Hata günlüğü" düğmesiyle açıp hata kaydına ekler.
//!
//! Dosya 1 MB'ı geçince `lyraska.1.log` adıyla saklanır ve yenisi başlar
//! (en fazla ~2 MB yer kaplar).
//!
//! Ses çıkış iş parçacığının gerçek zamanlı döngüsünden **çağrılmaz** (dosya
//! erişimi ve kilit içerir).

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Dosya bu boyutu geçince eskisi yedeklenir.
const MAX_LOG_BYTES: u64 = 1024 * 1024;
const LOG_NAME: &str = "lyraska.log";
const OLD_LOG_NAME: &str = "lyraska.1.log";

struct Log {
    path: PathBuf,
    file: File,
}

static LOG: Mutex<Option<Log>> = Mutex::new(None);

/// Günlüğü açar, açılış satırını yazar ve çökme yakalayıcıyı kurar.
pub fn init(dir: &Path) {
    let path = dir.join(LOG_NAME);
    if let Err(error) = open(&path) {
        eprintln!("Hata günlüğü açılamadı ({}): {error}", path.display());
        return;
    }
    info(&format!(
        "Lyraska {} başladı · {} {}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    ));
    install_panic_hook();
}

/// Günlük dosyasının yolu (açılamadıysa `None`).
pub fn log_path() -> Option<PathBuf> {
    LOG.lock().ok()?.as_ref().map(|log| log.path.clone())
}

pub fn info(message: &str) {
    write("BİLGİ", message);
}

pub fn error(message: &str) {
    write("HATA", message);
}

fn open(path: &Path) -> std::io::Result<()> {
    let log = Log::open(path)?;
    if let Ok(mut current) = LOG.lock() {
        *current = Some(log);
    }
    Ok(())
}

impl Log {
    /// Dosyayı açar (yoksa oluşturur); sınırı aşmışsa önce yedekler.
    fn open(path: &Path) -> std::io::Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        rotate_if_large(path);
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    /// Satırı yazar. Uzun süre açık kalan programda da dosya sınırı aşılmasın diye
    /// gerekirse önce yedekler; yedekleme olmazsa (ör. eski yedek kilitli) aynı
    /// dosyaya yazmayı sürdürür: günlük yüzünden program asla çökmez.
    fn write_line(&mut self, line: &str) {
        if self.file.metadata().is_ok_and(|m| m.len() > MAX_LOG_BYTES) {
            if let Ok(fresh) = Self::open(&self.path) {
                *self = fresh;
            }
        }
        let _ = self.file.write_all(line.as_bytes());
        let _ = self.file.flush();
    }
}

fn rotate_if_large(path: &Path) {
    let large = std::fs::metadata(path).is_ok_and(|m| m.len() > MAX_LOG_BYTES);
    if large {
        let _ = std::fs::rename(path, path.with_file_name(OLD_LOG_NAME));
    }
}

fn write(level: &str, message: &str) {
    let Ok(mut guard) = LOG.lock() else {
        return;
    };
    let Some(log) = guard.as_mut() else {
        return;
    };
    log.write_line(&format!(
        "{} [{level}] {}\n",
        timestamp(),
        message.trim_end()
    ));
}

fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "bilinmeyen".to_owned());
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_default();
        let thread = std::thread::current();
        let backtrace = std::backtrace::Backtrace::force_capture();
        write(
            "ÇÖKME",
            &format!(
                "iş parçacığı \"{}\": {message} ({location})\n{backtrace}",
                thread.name().unwrap_or("adsız")
            ),
        );
        previous(info);
    }));
}

/// "2026-10-08 06:45:12 UTC" (saat dilimi kütüphanesi kullanmadan).
pub fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_utc(secs)
}

fn format_utc(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rest = secs % 86_400;
    // Howard Hinnant'ın "civil_from_days" algoritması.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tarih_bicimi() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(format_utc(1_791_441_912), "2026-10-08 06:45:12 UTC");
        assert_eq!(format_utc(951_782_400), "2000-02-29 00:00:00 UTC");
    }

    #[test]
    fn gunluk_yazar_buyuyunce_yedekler_ve_cokmeyi_kaydeder() {
        // Günlük tek bir genel nesne: bu test hepsini sırayla dener.
        let dir = crate::audio::test_util::temp_path("gunluk").with_extension("");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(LOG_NAME);
        // Büyük eski dosya açılışta yedeklenir.
        std::fs::write(&path, vec![b'x'; MAX_LOG_BYTES as usize + 10]).unwrap();
        init(&dir);
        assert!(dir.join(OLD_LOG_NAME).exists());
        assert_eq!(log_path().unwrap(), path);

        error("Şarkı açılamadı: deneme.mp3");
        let result = std::panic::catch_unwind(|| panic!("deneme çökmesi"));
        assert!(result.is_err());
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("[BİLGİ] Lyraska"), "{text}");
        assert!(
            text.contains("[HATA] Şarkı açılamadı: deneme.mp3"),
            "{text}"
        );
        assert!(
            text.contains("[ÇÖKME]") && text.contains("deneme çökmesi"),
            "{text}"
        );
        assert!(text.contains("diagnostics.rs"), "çökmenin yeri yazılır");
        let _ = std::panic::take_hook();

        // Yedekleme olmuyorsa (ör. eski yedek kilitli) program çökmemeli, yazmayı
        // sürdürmeli. Burada yedeğin yerinde boş olmayan bir klasör var.
        std::fs::remove_file(dir.join(OLD_LOG_NAME)).unwrap();
        std::fs::create_dir_all(dir.join(OLD_LOG_NAME).join("kilitli")).unwrap();
        info(&"x".repeat(MAX_LOG_BYTES as usize));
        info("yedeklenemese de yazılır");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.ends_with("yedeklenemese de yazılır\n"));
    }
}
