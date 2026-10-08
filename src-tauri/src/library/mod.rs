//! Müzik kütüphanesi.
//!
//! - [`db`]: SQLite veritabanı (kaynaklar, şarkılar, arama).
//! - [`scan`]: kaynakları tarayıp veritabanını güncel tutan tarama.
//!
//! Kütüphaneye "kaynak" eklenir: bir klasör (alt klasörleriyle birlikte) ya da
//! tek bir şarkı dosyası. İkisi de aynı şekilde taranır, güncellenir ve çıkarılır.
//! - [`service`]: veritabanı + arka plan taraması; Tauri komutlarının kullandığı katman.
//!
//! Veritabanı kullanıcının bilgisayarında, uygulama veri klasöründe durur
//! (`%APPDATA%\io.github.gozlemesevdalisi.lyraska\library.sqlite3`). İnternet gerekmez.

pub mod db;
pub mod scan;
pub mod service;

use std::path::{Component, Path};

use thiserror::Error;

pub use db::{FolderRow, Library, TrackRow};
pub use service::{DropOutcome, LibraryService, LibraryStatus};

/// Kütüphane hataları. Mesajlar doğrudan kullanıcıya gösterilir.
#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Kütüphane veritabanı hatası: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    Schema(#[from] crate::storage::SchemaError),
    #[error("Klasör okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bu zaten kütüphanede.")]
    FolderExists,
    #[error("Bu zaten kütüphanede: \"{0}\" klasörünün içinde.")]
    AlreadyCovered(String),
    #[error("Bulunamadı (taşınmış ya da silinmiş olabilir): {0}")]
    FolderMissing(String),
    #[error("Bu dosya türü desteklenmiyor: {0}")]
    UnsupportedFile(String),
}

/// `parent` yolu `child` yolunu kapsıyor mu (aynı yol ya da onun içinde mi)?
/// Bileşen bileşen karşılaştırılır: "D:/Müzik", "D:/Müzik 2" klasörünü kapsamaz.
/// Windows'ta büyük/küçük harf farkı yok sayılır.
pub fn path_covers(parent: &Path, child: &Path) -> bool {
    let mut child_parts = child.components().filter(|c| *c != Component::CurDir);
    parent
        .components()
        .filter(|c| *c != Component::CurDir)
        .all(|p| child_parts.next().is_some_and(|c| same_component(p, c)))
}

fn same_component(a: Component<'_>, b: Component<'_>) -> bool {
    let (a, b) = (
        a.as_os_str().to_string_lossy(),
        b.as_os_str().to_string_lossy(),
    );
    if cfg!(windows) {
        a.to_lowercase() == b.to_lowercase()
    } else {
        a == b
    }
}

/// Aramada kullanılan sadeleştirilmiş metin: küçük harf, Türkçe harfler ve
/// aksanlar sade karşılıklarına çevrilir ("Şebnem Ferah" → "sebnem ferah").
/// Böylece Türkçe klavyesi olmayan biri de "sebnem" yazarak bulabilir.
pub fn search_key(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let folded = match ch {
            'I' | 'ı' | 'İ' | 'i' | 'Í' | 'í' | 'Ì' | 'ì' | 'Î' | 'î' | 'Ï' | 'ï' => 'i',
            'Ş' | 'ş' | 'Ś' | 'ś' | 'Š' | 'š' => 's',
            'Ç' | 'ç' | 'Ć' | 'ć' | 'Č' | 'č' => 'c',
            'Ğ' | 'ğ' => 'g',
            'Ö' | 'ö' | 'Ó' | 'ó' | 'Ò' | 'ò' | 'Ô' | 'ô' | 'Õ' | 'õ' | 'Ø' | 'ø' => {
                'o'
            }
            'Ü' | 'ü' | 'Ú' | 'ú' | 'Ù' | 'ù' | 'Û' | 'û' => 'u',
            'Á' | 'á' | 'À' | 'à' | 'Â' | 'â' | 'Ä' | 'ä' | 'Ã' | 'ã' | 'Å' | 'å' => {
                'a'
            }
            'É' | 'é' | 'È' | 'è' | 'Ê' | 'ê' | 'Ë' | 'ë' => 'e',
            'Ñ' | 'ñ' => 'n',
            'Ý' | 'ý' | 'Ÿ' | 'ÿ' => 'y',
            'Ž' | 'ž' => 'z',
            other => {
                out.extend(other.to_lowercase());
                continue;
            }
        };
        out.push(folded);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yol_kapsama_bilesen_bilesen_karsilastirilir() {
        let covers = |a: &str, b: &str| path_covers(Path::new(a), Path::new(b));
        assert!(covers("/muzik", "/muzik"));
        assert!(covers("/muzik", "/muzik/rock/a.mp3"));
        assert!(covers("/muzik/", "/muzik/a.mp3"));
        assert!(
            !covers("/muzik", "/muzik 2/a.mp3"),
            "benzer adlı kardeş klasör"
        );
        assert!(!covers("/muzik/rock", "/muzik"));
        assert!(!covers("/muzik/a.mp3", "/muzik/a.mp3.bak"));
    }

    #[test]
    fn turkce_harfler_ve_aksanlar_sadelesir() {
        assert_eq!(search_key("Şebnem Ferah"), "sebnem ferah");
        assert_eq!(search_key("IŞIK İÇİN ĞÖÜ"), "isik icin gou");
        assert_eq!(search_key("Beyoncé – Déjà Vu"), "beyonce – deja vu");
        assert_eq!(search_key("Sigur Rós"), "sigur ros");
    }
}
