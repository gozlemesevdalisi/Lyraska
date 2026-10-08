//! Şarkı haritası önbelleği: analiz sonuçları kütüphane veritabanında (SQLite) saklanır.
//!
//! Bir şarkı bir kez analiz edilir; sonraki açılışlarda spektrum, vuruşlar, ölçü
//! başları, bölümler, droplar ve enerji anında hazırdır. Kayıt anahtarı dosya
//! yolu + boyut + değiştirilme zamanı + [`ANALYSIS_VERSION`]: dosya değişirse ya
//! da analiz algoritması değişirse (sürüm artırılınca) kayıt geçersiz sayılır ve
//! şarkı yeniden analiz edilir.
//!
//! Saklama biçimi: bant ve kanal seviyeleri zamanda fark alınarak (ardışık kareler
//! birbirine çok benzer) sıkıştırılır (deflate); başlangıç gücü ham `f32` olarak
//! sıkıştırılır. Vuruş ızgarası ve şarkı haritası JSON olarak tutulur; böylece
//! kütüphane listesi BPM'i doğrudan okuyabilir.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use super::beats::BeatGrid;
use super::levels::VALUES_PER_FRAME;
use super::spectrogram::{SavedAnalysis, BANDS};
use super::structure::SongMap;

/// Analiz algoritmalarının sürümü. Spektrum, beat, ölçü, bölüm, drop ya da enerji
/// hesabı değiştiğinde **artırılır**: eski kayıtlar kendiliğinden geçersiz olur.
pub const ANALYSIS_VERSION: i64 = 1;

/// Önbellek tablosu. Kütüphane veritabanı da aynı tabloyu kurar (BPM sütunu için).
pub const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS analyses (
    path      TEXT PRIMARY KEY,
    file_size INTEGER NOT NULL,
    modified  INTEGER NOT NULL,
    version   INTEGER NOT NULL,
    frames    INTEGER NOT NULL,
    bpm       REAL,
    levels    BLOB NOT NULL,
    meters    BLOB NOT NULL,
    onset     BLOB NOT NULL,
    beats     TEXT,
    song_map  TEXT
);
";

/// Sıkıştırma düzeyi (0–10): 6 hız ve boyut arasında iyi bir denge.
const COMPRESSION_LEVEL: u8 = 6;

#[derive(Debug, Error)]
pub enum CacheError {
    #[error("Analiz önbelleği hatası: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Analiz önbelleği açılamadı: {0}")]
    Io(#[from] std::io::Error),
    #[error("Analiz önbelleği kilitlenemedi")]
    Lock,
}

/// Bir dosyanın değişip değişmediğini anlamak için boyutu ve değişme zamanı
/// (kütüphane taramasıyla aynı kural: Unix zamanı, saniye).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileStamp {
    pub size: u64,
    pub modified: i64,
}

impl FileStamp {
    pub fn of_metadata(meta: &std::fs::Metadata) -> Self {
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs() as i64);
        Self {
            size: meta.len(),
            modified,
        }
    }

    /// Dosyanın şu anki damgası; dosya okunamıyorsa `None`.
    pub fn of(path: &Path) -> Option<Self> {
        std::fs::metadata(path).ok().map(|m| Self::of_metadata(&m))
    }
}

/// Analiz önbelleği. Kendi veritabanı bağlantısıyla çalışır (kütüphaneyle aynı dosya);
/// oynatıcının ve arka plan analizinin iş parçacıklarından kullanılır.
pub struct AnalysisCache {
    conn: Mutex<Connection>,
}

impl AnalysisCache {
    /// Veritabanını açar (yoksa oluşturur).
    pub fn open(path: &Path) -> Result<Self, CacheError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// Bellekte geçici önbellek (testler için).
    pub fn open_in_memory() -> Result<Self, CacheError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, CacheError> {
        // Kütüphane taraması aynı dosyaya yazarken kısa süre beklenir.
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CacheError> {
        self.conn.lock().map_err(|_| CacheError::Lock)
    }

    /// Şarkının geçerli (dosya ve analiz sürümü değişmemiş) kaydı; yoksa `None`.
    /// Bozuk kayıt da yok sayılır (şarkı yeniden analiz edilir).
    pub fn load(&self, path: &Path, stamp: FileStamp) -> Result<Option<SavedAnalysis>, CacheError> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT frames, levels, meters, onset, beats, song_map FROM analyses
                 WHERE path = ?1 AND file_size = ?2 AND modified = ?3 AND version = ?4",
                params![
                    path.to_string_lossy(),
                    stamp.size as i64,
                    stamp.modified,
                    ANALYSIS_VERSION
                ],
                |r| {
                    Ok((
                        r.get::<_, i64>(0)?,
                        r.get::<_, Vec<u8>>(1)?,
                        r.get::<_, Vec<u8>>(2)?,
                        r.get::<_, Vec<u8>>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, Option<String>>(5)?,
                    ))
                },
            )
            .optional()?;
        drop(conn);
        Ok(
            row.and_then(|(frames, levels, meters, onset, beats, song_map)| {
                let saved = SavedAnalysis {
                    levels: undelta(&inflate(&levels)?, BANDS),
                    meters: undelta(&inflate(&meters)?, VALUES_PER_FRAME),
                    onset: inflate(&onset)?
                        .chunks_exact(4)
                        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                        .collect(),
                    beats: beats.and_then(|j| serde_json::from_str::<BeatGrid>(&j).ok()),
                    song_map: song_map.and_then(|j| serde_json::from_str::<SongMap>(&j).ok()),
                };
                (saved.is_consistent() && saved.frames() as i64 == frames).then_some(saved)
            }),
        )
    }

    /// Analiz sonucunu saklar (aynı yolun eski kaydının yerini alır).
    pub fn store(
        &self,
        path: &Path,
        stamp: FileStamp,
        saved: &SavedAnalysis,
    ) -> Result<(), CacheError> {
        let onset: Vec<u8> = saved.onset.iter().flat_map(|v| v.to_le_bytes()).collect();
        let beats = saved
            .beats
            .as_ref()
            .and_then(|b| serde_json::to_string(b).ok());
        let song_map = saved
            .song_map
            .as_ref()
            .and_then(|m| serde_json::to_string(m).ok());
        let row = params![
            path.to_string_lossy(),
            stamp.size as i64,
            stamp.modified,
            ANALYSIS_VERSION,
            saved.frames() as i64,
            saved.beats.as_ref().map(|b| b.bpm),
            deflate(&delta(&saved.levels, BANDS)),
            deflate(&delta(&saved.meters, VALUES_PER_FRAME)),
            deflate(&onset),
            beats,
            song_map,
        ];
        self.lock()?.execute(
            "INSERT OR REPLACE INTO analyses
                 (path, file_size, modified, version, frames, bpm, levels, meters, onset, beats, song_map)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            row,
        )?;
        Ok(())
    }

    /// Analizi geçersiz ya da hiç olmayan kütüphane şarkıları (sıralı, en fazla `limit`).
    /// Kütüphane tablosu yoksa boş döner.
    pub fn pending_library_tracks(&self, limit: usize) -> Result<Vec<PathBuf>, CacheError> {
        let conn = self.lock()?;
        let has_tracks: bool = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'tracks'",
                [],
                |r| r.get::<_, i64>(0),
            )
            .map(|n| n > 0)?;
        if !has_tracks {
            return Ok(Vec::new());
        }
        let mut stmt = conn.prepare(
            "SELECT t.path FROM tracks t
             LEFT JOIN analyses a ON a.path = t.path AND a.file_size = t.file_size
                 AND a.modified = t.modified AND a.version = ?1
             WHERE a.path IS NULL
             ORDER BY t.sort_key LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![ANALYSIS_VERSION, limit as i64], |r| {
            r.get::<_, String>(0)
        })?;
        Ok(rows
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .map(PathBuf::from)
            .collect())
    }
}

/// Kareleri bir öncekinden farka çevirir (taşmalı çıkarma): ardışık kareler benzer
/// olduğu için farklar çoğunlukla sıfıra yakındır ve çok daha iyi sıkışır.
fn delta(data: &[u8], stride: usize) -> Vec<u8> {
    let mut out = data.to_vec();
    for i in (stride..out.len()).rev() {
        out[i] = data[i].wrapping_sub(data[i - stride]);
    }
    out
}

fn undelta(data: &[u8], stride: usize) -> Vec<u8> {
    let mut out = data.to_vec();
    for i in stride..out.len() {
        out[i] = out[i].wrapping_add(out[i - stride]);
    }
    out
}

fn deflate(data: &[u8]) -> Vec<u8> {
    miniz_oxide::deflate::compress_to_vec(data, COMPRESSION_LEVEL)
}

fn inflate(data: &[u8]) -> Option<Vec<u8>> {
    miniz_oxide::inflate::decompress_to_vec(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::structure::Section;

    fn sample(frames: usize) -> SavedAnalysis {
        SavedAnalysis {
            levels: (0..frames * BANDS)
                .map(|i| ((i / BANDS) % 200 + i % BANDS) as u8)
                .collect(),
            meters: (0..frames * VALUES_PER_FRAME)
                .map(|i| (i * 7 % 251) as u8)
                .collect(),
            onset: (0..frames).map(|i| (i as f32 * 0.37).sin().abs()).collect(),
            beats: Some(BeatGrid {
                bpm: 128.0,
                beats: vec![0.25, 0.72, 1.19],
            }),
            song_map: Some(SongMap {
                meter: 4,
                downbeat_phase: 1,
                downbeats: vec![0.72],
                sections: vec![Section {
                    start: 0.0,
                    end: 3.0,
                    energy: 0.5,
                    label: 0,
                }],
                drops: vec![],
                energy: vec![0.1, 0.9, 0.4],
            }),
        }
    }

    const STAMP: FileStamp = FileStamp {
        size: 1234,
        modified: 1_700_000_000,
    };

    #[test]
    fn saklanan_analiz_aynen_geri_okunur() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        let path = Path::new("C:/Müzik/şarkı.flac");
        let saved = sample(600);
        cache.store(path, STAMP, &saved).unwrap();
        assert_eq!(cache.load(path, STAMP).unwrap(), Some(saved));
    }

    #[test]
    fn dosya_ya_da_surum_degisince_kayit_gecersiz() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        let path = Path::new("/muzik/a.mp3");
        cache.store(path, STAMP, &sample(10)).unwrap();
        let bigger = FileStamp {
            size: 1235,
            ..STAMP
        };
        let newer = FileStamp {
            modified: STAMP.modified + 1,
            ..STAMP
        };
        assert_eq!(cache.load(path, bigger).unwrap(), None);
        assert_eq!(cache.load(path, newer).unwrap(), None);
        assert_eq!(cache.load(Path::new("/muzik/b.mp3"), STAMP).unwrap(), None);
        // Eski analiz sürümüyle yazılmış kayıt da geçersiz.
        cache
            .lock()
            .unwrap()
            .execute("UPDATE analyses SET version = version - 1", [])
            .unwrap();
        assert_eq!(cache.load(path, STAMP).unwrap(), None);
    }

    #[test]
    fn bozuk_kayit_yok_sayilir() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        let path = Path::new("/muzik/a.mp3");
        cache.store(path, STAMP, &sample(10)).unwrap();
        cache
            .lock()
            .unwrap()
            .execute("UPDATE analyses SET levels = x'00ff'", [])
            .unwrap();
        assert_eq!(cache.load(path, STAMP).unwrap(), None);
    }

    #[test]
    fn ritimsiz_sarki_da_saklanir() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        let path = Path::new("/muzik/yagmur.ogg");
        let saved = SavedAnalysis {
            beats: None,
            song_map: None,
            ..sample(30)
        };
        cache.store(path, STAMP, &saved).unwrap();
        assert_eq!(cache.load(path, STAMP).unwrap(), Some(saved));
    }

    #[test]
    fn fark_kodlamasi_geri_alinir_ve_sikistirir() {
        let saved = sample(3600); // 1 dakika
        assert_eq!(undelta(&delta(&saved.levels, BANDS), BANDS), saved.levels);
        let plain = deflate(&saved.levels).len();
        let coded = deflate(&delta(&saved.levels, BANDS)).len();
        assert!(coded < plain, "fark kodlaması {coded} ≥ düz {plain}");
    }

    #[test]
    fn onbellek_yazarken_kutuphane_taramasi_bekler_hata_vermez() {
        use crate::library::Library;
        let db = crate::audio::test_util::temp_path("kilit.sqlite3");
        let mut library = Library::open(&db).unwrap();
        let folder = library.add_folder("/muzik").unwrap();
        let cache = AnalysisCache::open(&db).unwrap();
        // Önbellek bir analiz sonucunu yazarken (yazma kilidi elinde) kütüphane taraması gelir.
        cache
            .lock()
            .unwrap()
            .execute_batch("BEGIN IMMEDIATE")
            .unwrap();
        let writer = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(150));
            cache.lock().unwrap().execute_batch("COMMIT").unwrap();
        });
        let result = library.apply_changes(folder.id, &[], &["/muzik/yok.mp3".to_owned()]);
        writer.join().unwrap();
        assert!(result.is_ok(), "kütüphane beklemeli: {result:?}");
    }

    #[test]
    fn kutuphane_tablosu_yoksa_bekleyen_sarki_yok() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        assert!(cache.pending_library_tracks(10).unwrap().is_empty());
    }
}
