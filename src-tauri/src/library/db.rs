//! Kütüphane veritabanı (SQLite).

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
use serde::Serialize;

use super::{search_key, LibraryError};
use crate::audio::decode::TrackInfo;

/// Şema sürümü; yapı değişince artırılır ve göç adımı eklenir.
const SCHEMA_VERSION: i64 = 1;

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS folders (
    id   INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE
);
CREATE TABLE IF NOT EXISTS tracks (
    id            INTEGER PRIMARY KEY,
    folder_id     INTEGER NOT NULL REFERENCES folders(id) ON DELETE CASCADE,
    path          TEXT NOT NULL UNIQUE,
    file_name     TEXT NOT NULL,
    title         TEXT,
    artist        TEXT,
    album         TEXT,
    album_artist  TEXT,
    track_number  INTEGER,
    disc_number   INTEGER,
    duration_secs REAL,
    codec         TEXT NOT NULL,
    sample_rate   INTEGER NOT NULL,
    channels      INTEGER NOT NULL,
    file_size     INTEGER NOT NULL,
    modified      INTEGER NOT NULL,
    sort_key      TEXT NOT NULL,
    search        TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS tracks_folder ON tracks(folder_id);
CREATE INDEX IF NOT EXISTS tracks_sort ON tracks(sort_key);
";

/// Arama sonucunda dönen en fazla şarkı sayısı.
pub const MAX_RESULTS: usize = 50_000;

/// Kütüphanedeki bir klasör.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderRow {
    pub id: i64,
    pub path: String,
}

/// Listede gösterilen bir şarkı.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackRow {
    pub id: i64,
    pub path: String,
    /// Etiketteki başlık, yoksa dosya adı.
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<u32>,
    pub duration_secs: Option<f64>,
    pub codec: String,
}

/// Bir dosyanın değişip değişmediğini anlamak için boyutu ve değişme zamanı.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub modified: i64,
}

pub struct Library {
    conn: Connection,
}

impl Library {
    /// Veritabanını açar (yoksa oluşturur).
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Bellekte geçici veritabanı (testler için).
    pub fn open_in_memory() -> Result<Self, LibraryError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, LibraryError> {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;",
        )?;
        conn.execute_batch(SCHEMA)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn })
    }

    pub fn add_folder(&self, path: &str) -> Result<FolderRow, LibraryError> {
        let inserted = self
            .conn
            .execute("INSERT OR IGNORE INTO folders (path) VALUES (?1)", [path])?;
        if inserted == 0 {
            return Err(LibraryError::FolderExists);
        }
        Ok(FolderRow {
            id: self.conn.last_insert_rowid(),
            path: path.to_owned(),
        })
    }

    /// Klasörü ve ona ait bütün şarkıları kütüphaneden çıkarır (dosyalara dokunmaz).
    pub fn remove_folder(&self, id: i64) -> Result<(), LibraryError> {
        self.conn
            .execute("DELETE FROM folders WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn folders(&self) -> Result<Vec<FolderRow>, LibraryError> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, path FROM folders ORDER BY path")?;
        let rows = stmt.query_map([], |r| {
            Ok(FolderRow {
                id: r.get(0)?,
                path: r.get(1)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    pub fn folder(&self, id: i64) -> Result<Option<FolderRow>, LibraryError> {
        Ok(self
            .conn
            .query_row("SELECT id, path FROM folders WHERE id = ?1", [id], |r| {
                Ok(FolderRow {
                    id: r.get(0)?,
                    path: r.get(1)?,
                })
            })
            .optional()?)
    }

    pub fn track_count(&self) -> Result<usize, LibraryError> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM tracks", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Klasördeki bilinen dosyalar ve damgaları (yeniden taramada değişmeyenleri atlamak için).
    pub fn known_files(&self, folder_id: i64) -> Result<HashMap<String, FileStamp>, LibraryError> {
        let mut stmt = self
            .conn
            .prepare("SELECT path, file_size, modified FROM tracks WHERE folder_id = ?1")?;
        let rows = stmt.query_map([folder_id], |r| {
            Ok((
                r.get::<_, String>(0)?,
                FileStamp {
                    size: r.get::<_, i64>(1)? as u64,
                    modified: r.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Şarkıları ekler ya da günceller ve yok olan dosyaları siler; tek işlemde.
    pub fn apply_changes(
        &mut self,
        folder_id: i64,
        upserts: &[(TrackInfo, FileStamp)],
        removed: &[String],
    ) -> Result<(), LibraryError> {
        let tx = self.conn.transaction()?;
        {
            let mut insert = tx.prepare_cached(
                "INSERT INTO tracks (folder_id, path, file_name, title, artist, album, album_artist,
                     track_number, disc_number, duration_secs, codec, sample_rate, channels,
                     file_size, modified, sort_key, search)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                 ON CONFLICT(path) DO UPDATE SET
                     folder_id = excluded.folder_id, file_name = excluded.file_name,
                     title = excluded.title, artist = excluded.artist, album = excluded.album,
                     album_artist = excluded.album_artist, track_number = excluded.track_number,
                     disc_number = excluded.disc_number, duration_secs = excluded.duration_secs,
                     codec = excluded.codec, sample_rate = excluded.sample_rate,
                     channels = excluded.channels, file_size = excluded.file_size,
                     modified = excluded.modified, sort_key = excluded.sort_key,
                     search = excluded.search",
            )?;
            for (info, stamp) in upserts {
                insert.execute(params![
                    folder_id,
                    info.path.to_string_lossy(),
                    info.file_name,
                    info.title,
                    info.artist,
                    info.album,
                    info.album_artist,
                    info.track_number,
                    info.disc_number,
                    info.duration_secs,
                    info.codec,
                    info.sample_rate,
                    info.channels as i64,
                    stamp.size as i64,
                    stamp.modified,
                    sort_key(info),
                    search_text(info),
                ])?;
            }
            let mut delete = tx.prepare_cached("DELETE FROM tracks WHERE path = ?1")?;
            for path in removed {
                delete.execute([path])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// Şarkıları arar. Boş arama bütün kütüphaneyi döndürür. Her kelime başlık,
    /// sanatçı, albüm ya da dosya adında geçmelidir (sıra önemsiz, Türkçe karakter
    /// ve büyük/küçük harf duyarsız). Sıralama: sanatçı, albüm, disk, parça.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<TrackRow>, LibraryError> {
        let words: Vec<String> = search_key(query)
            .split_whitespace()
            .map(|w| format!("%{}%", escape_like(w)))
            .collect();
        let mut sql = String::from(
            "SELECT id, path, COALESCE(title, file_name), artist, album, track_number,
                    duration_secs, codec
             FROM tracks",
        );
        for i in 0..words.len() {
            sql.push_str(if i == 0 { " WHERE " } else { " AND " });
            sql.push_str(&format!("search LIKE ?{} ESCAPE '\\'", i + 1));
        }
        sql.push_str(&format!(
            " ORDER BY sort_key LIMIT {}",
            limit.min(MAX_RESULTS)
        ));

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(words.iter()), |r| {
            Ok(TrackRow {
                id: r.get(0)?,
                path: r.get(1)?,
                title: r.get(2)?,
                artist: r.get(3)?,
                album: r.get(4)?,
                track_number: r.get(5)?,
                duration_secs: r.get(6)?,
                codec: r.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

/// LIKE kalıbındaki özel karakterleri kaçırır.
fn escape_like(word: &str) -> String {
    word.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Sıralama anahtarı: albüm sanatçısı (yoksa sanatçı), albüm, disk, parça, başlık.
fn sort_key(info: &TrackInfo) -> String {
    let artist = info
        .album_artist
        .as_deref()
        .or(info.artist.as_deref())
        .unwrap_or("\u{10FFFF}"); // sanatçısız şarkılar sona
    let title = info.title.as_deref().unwrap_or(&info.file_name);
    format!(
        "{}\u{1}{}\u{1}{:03}\u{1}{:04}\u{1}{}",
        search_key(artist),
        search_key(info.album.as_deref().unwrap_or("")),
        info.disc_number.unwrap_or(0),
        info.track_number.unwrap_or(0),
        search_key(title)
    )
}

/// Aramada taranan metin.
fn search_text(info: &TrackInfo) -> String {
    let parts = [
        info.title.as_deref(),
        info.artist.as_deref(),
        info.album_artist.as_deref(),
        info.album.as_deref(),
        Some(info.file_name.as_str()),
    ];
    search_key(&parts.into_iter().flatten().collect::<Vec<_>>().join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn track(
        path: &str,
        title: Option<&str>,
        artist: Option<&str>,
        album: Option<&str>,
        no: Option<u32>,
    ) -> TrackInfo {
        let path = PathBuf::from(path);
        TrackInfo {
            file_name: path.file_stem().unwrap().to_string_lossy().into_owned(),
            path,
            title: title.map(str::to_owned),
            artist: artist.map(str::to_owned),
            album: album.map(str::to_owned),
            album_artist: None,
            track_number: no,
            disc_number: None,
            codec: "flac".to_owned(),
            sample_rate: 44_100,
            channels: 2,
            duration_secs: Some(200.0),
        }
    }

    const STAMP: FileStamp = FileStamp {
        size: 1000,
        modified: 1,
    };

    fn sample() -> Library {
        let mut lib = Library::open_in_memory().unwrap();
        let folder = lib.add_folder("/muzik").unwrap();
        lib.apply_changes(
            folder.id,
            &[
                (
                    track(
                        "/muzik/b.flac",
                        Some("Bir Kedi Gördüm"),
                        Some("Şebnem Ferah"),
                        Some("Kelimeler"),
                        Some(2),
                    ),
                    STAMP,
                ),
                (
                    track(
                        "/muzik/a.flac",
                        Some("Mayın Tarlası"),
                        Some("Şebnem Ferah"),
                        Some("Kelimeler"),
                        Some(1),
                    ),
                    STAMP,
                ),
                (
                    track(
                        "/muzik/c.mp3",
                        Some("Gülümse"),
                        Some("Sezen Aksu"),
                        Some("Gülümse"),
                        Some(1),
                    ),
                    STAMP,
                ),
                (
                    track("/muzik/etiketsiz_dosya.mp3", None, None, None, None),
                    STAMP,
                ),
            ],
            &[],
        )
        .unwrap();
        lib
    }

    #[test]
    fn bos_arama_hepsini_sanatci_album_parca_sirasiyla_dondurur() {
        let titles: Vec<String> = sample()
            .search("", 100)
            .unwrap()
            .into_iter()
            .map(|t| t.title)
            .collect();
        assert_eq!(
            titles,
            // Şebnem (s-e-b) Sezen'den (s-e-z) önce gelir; etiketsiz şarkılar en sonda.
            [
                "Mayın Tarlası",
                "Bir Kedi Gördüm",
                "Gülümse",
                "etiketsiz_dosya"
            ]
        );
    }

    #[test]
    fn arama_turkce_karakter_ve_buyuk_kucuk_harf_duyarsiz() {
        let lib = sample();
        for query in ["sebnem", "ŞEBNEM", "Şebnem ferah", "ferah kelime"] {
            assert_eq!(lib.search(query, 100).unwrap().len(), 2, "{query}");
        }
        assert_eq!(
            lib.search("gulumse", 100).unwrap()[0].artist.as_deref(),
            Some("Sezen Aksu")
        );
        assert_eq!(
            lib.search("etiketsiz", 100).unwrap().len(),
            1,
            "dosya adında da arar"
        );
        assert!(lib.search("yok böyle", 100).unwrap().is_empty());
        // LIKE özel karakterleri kaçırılır: "_" her karakterle eşleşmemeli.
        assert_eq!(lib.search("z_d", 100).unwrap().len(), 1);
        assert!(lib.search("%", 100).unwrap().is_empty());
    }

    #[test]
    fn guncelleme_ve_silme_tek_islemde() {
        let mut lib = sample();
        let folder = lib.folders().unwrap()[0].id;
        let mut changed = track(
            "/muzik/c.mp3",
            Some("Gülümse (Remaster)"),
            Some("Sezen Aksu"),
            None,
            None,
        );
        changed.duration_secs = Some(10.0);
        lib.apply_changes(
            folder,
            &[(
                changed,
                FileStamp {
                    size: 5,
                    modified: 2,
                },
            )],
            &["/muzik/a.flac".to_owned()],
        )
        .unwrap();
        assert_eq!(lib.track_count().unwrap(), 3);
        assert_eq!(lib.search("remaster", 10).unwrap().len(), 1);
        assert_eq!(
            lib.known_files(folder).unwrap()["/muzik/c.mp3"],
            FileStamp {
                size: 5,
                modified: 2
            }
        );
    }

    #[test]
    fn klasor_tekrar_eklenmez_ve_silinince_sarkilari_da_gider() {
        let lib = sample();
        assert!(matches!(
            lib.add_folder("/muzik"),
            Err(LibraryError::FolderExists)
        ));
        let id = lib.folders().unwrap()[0].id;
        lib.remove_folder(id).unwrap();
        assert_eq!(lib.track_count().unwrap(), 0);
        assert!(lib.folders().unwrap().is_empty());
    }

    #[test]
    fn veritabani_dosyaya_yazilir_ve_yeniden_acilir() {
        let path = crate::audio::test_util::temp_path("kutuphane.sqlite3");
        {
            let lib = Library::open(&path).unwrap();
            lib.add_folder("/muzik").unwrap();
        }
        let lib = Library::open(&path).unwrap();
        assert_eq!(lib.folders().unwrap().len(), 1);
    }
}
