//! Müzik kütüphanesi.
//!
//! - [`db`]: SQLite veritabanı (klasörler, şarkılar, arama).
//! - [`scan`]: klasörleri tarayıp veritabanını güncel tutan tarama.
//! - [`service`]: veritabanı + arka plan taraması; Tauri komutlarının kullandığı katman.
//!
//! Veritabanı kullanıcının bilgisayarında, uygulama veri klasöründe durur
//! (`%APPDATA%\io.github.gozlemesevdalisi.lyraska\library.sqlite3`). İnternet gerekmez.

pub mod db;
pub mod scan;
pub mod service;

use thiserror::Error;

pub use db::{FolderRow, Library, TrackRow};
pub use service::{LibraryService, LibraryStatus};

/// Kütüphane hataları. Mesajlar doğrudan kullanıcıya gösterilir.
#[derive(Debug, Error)]
pub enum LibraryError {
    #[error("Kütüphane veritabanı hatası: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Klasör okunamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("Bu klasör zaten kütüphanede.")]
    FolderExists,
    #[error("Klasör bulunamadı: {0}")]
    FolderMissing(String),
    #[error("Bu bir klasör ya da desteklenen bir ses dosyası değil: {0}")]
    NotAFolder(String),
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
    fn turkce_harfler_ve_aksanlar_sadelesir() {
        assert_eq!(search_key("Şebnem Ferah"), "sebnem ferah");
        assert_eq!(search_key("IŞIK İÇİN ĞÖÜ"), "isik icin gou");
        assert_eq!(search_key("Beyoncé – Déjà Vu"), "beyonce – deja vu");
        assert_eq!(search_key("Sigur Rós"), "sigur ros");
    }
}
