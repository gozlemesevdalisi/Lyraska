//! Şarkı haritası önbelleği: analiz sonuçları kütüphane veritabanında (SQLite) saklanır.
//!
//! Bir şarkı bir kez analiz edilir; sonraki açılışlarda spektrum, vuruşlar, ölçü
//! başları, bölümler, droplar ve enerji anında hazırdır. Kayıt anahtarı dosya
//! yolu + boyut + değiştirilme zamanı: dosya değişirse kayıt geçersiz sayılır ve şarkı
//! yeniden analiz edilir.
//!
//! Analiz bağımsız **parçalardan** oluşur ve her parçanın kendi sürümü vardır
//! ([`VERSIONS`]). Bir parçanın hesabı değişince yalnızca o parçanın sürümü artırılır;
//! kütüphanede yalnızca o parça yeniden hesaplanır (bkz. [`PartVersions`]).
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
use crate::audio::bass::BassPeaks;
use crate::audio::loudness::Loudness;

/// Analizin bağımsız parçalarının hesap sürümleri.
///
/// Bir parçanın hesabı değişince **yalnızca o parçanın** sürümü artırılır; kütüphanede
/// yalnızca o parça yeniden hesaplanır, geri kalanı önbellekten okunur:
///
/// - `spectrum`: kareler (bant ve kanal seviyeleri, başlangıç gücü). Artarsa şarkı baştan
///   çözülür ve her şey yeniden hesaplanır (ritim de bu karelerden çıkar).
/// - `rhythm`: vuruşlar, ölçü, bölümler, droplar, enerji. Saklanan karelerden hesaplanır:
///   şarkı **yeniden çözülmez**, şarkı başına milisaniyeler sürer.
/// - `loudness`: ses yüksekliği (EBU R128) ve gerçek tepe. Şarkı yeniden çözülür (FFT'siz).
/// - `bass`: bas tepeleri (bas düğmesinin akıllı koruması). Şarkı yeniden çözülür (FFT'siz).
///
/// Tek sürüm döneminin geçmişi (`ANALYSIS_VERSION`): 2 kareler her örnekleme hızında tam
/// 1/60 sn, vuruş gecikmesi hıza göre düşülüyor; 3 ses yüksekliği; 4 bas tepeleri; 5 bas
/// tepeleri 27 dB'ye kadar. Göç 5 o kayıtları parça sürümlerine çevirdi (`storage.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartVersions {
    pub spectrum: i64,
    pub rhythm: i64,
    pub loudness: i64,
    pub bass: i64,
}

/// Parçaların şu anki sürümleri.
pub const VERSIONS: PartVersions = PartVersions {
    spectrum: 1,
    rhythm: 1,
    loudness: 1,
    bass: 1,
};

/// Eski tek sürüm sütununa (`version`) yazılan değer. Parça sürümlerinden önceki program
/// sürümleri bu kaydı geçersiz sayar (5 bekler) ve kendi kaydını yazar; onların yazdığı
/// kayıtta parça sürümleri 0 kalır, bu sürüm de onları yeniden hesaplar.
const LEGACY_VERSION: i64 = 6;

/// Önbellekteki kaydın yeniden hesaplanması gereken (sürümü eskimiş) parçaları.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StaleParts {
    pub rhythm: bool,
    pub loudness: bool,
    pub bass: bool,
}

impl StaleParts {
    pub fn any(self) -> bool {
        self.rhythm || self.loudness || self.bass
    }

    /// Şarkının yeniden çözülmesi gerekir mi (ses yüksekliği ya da bas tepeleri eski)?
    pub fn needs_decoding(self) -> bool {
        self.loudness || self.bass
    }
}

/// Önbellekten okunan kayıt. Eskimiş parçalar `saved` içinde boştur (`None`) ve `stale`
/// içinde işaretlidir; kareler (spektrum) her zaman günceldir.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedAnalysis {
    pub saved: SavedAnalysis,
    pub stale: StaleParts,
}

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
    #[error("{0}")]
    Schema(#[from] crate::storage::SchemaError),
}

/// Bir dosyanın değişip değişmediğini anlamak için boyutu ve değişme zamanı
/// (kütüphane taramasıyla aynı kural: Unix zamanı, saniye).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
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

    fn init(mut conn: Connection) -> Result<Self, CacheError> {
        // Kütüphane taraması aynı dosyaya yazarken kısa süre beklenir.
        conn.busy_timeout(Duration::from_secs(5))?;
        conn.execute_batch("PRAGMA journal_mode = WAL; PRAGMA synchronous = NORMAL;")?;
        // Tablo kütüphaneyle ortak şemada (`crate::storage`).
        crate::storage::migrate(&mut conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, CacheError> {
        self.conn.lock().map_err(|_| CacheError::Lock)
    }

    /// Testlerde veritabanına doğrudan erişim (ör. bir parçayı eskitmek).
    #[cfg(test)]
    pub fn lock_for_test(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap()
    }

    /// Şarkının kaydı (dosya değişmemişse). Kareleri eski bir sürümle hesaplanmışsa, kayıt
    /// yoksa ya da bozuksa `None` (şarkı baştan analiz edilir). Diğer parçalardan eskimiş
    /// olanlar boş döner ve [`CachedAnalysis::stale`] içinde işaretlidir.
    pub fn load(
        &self,
        path: &Path,
        stamp: FileStamp,
    ) -> Result<Option<CachedAnalysis>, CacheError> {
        let conn = self.lock()?;
        let row = conn
            .query_row(
                "SELECT frames, levels, meters, onset, beats, song_map, loudness_lufs, true_peak_dbtp,
                        bass_peaks, sample_rate, rhythm_version, loudness_version, bass_version
                 FROM analyses
                 WHERE path = ?1 AND file_size = ?2 AND modified = ?3 AND spectrum_version = ?4",
                params![
                    path.to_string_lossy(),
                    stamp.size as i64,
                    stamp.modified,
                    VERSIONS.spectrum
                ],
                |r| {
                    Ok(Row {
                        frames: r.get(0)?,
                        levels: r.get(1)?,
                        meters: r.get(2)?,
                        onset: r.get(3)?,
                        beats: r.get(4)?,
                        song_map: r.get(5)?,
                        lufs: r.get(6)?,
                        peak: r.get(7)?,
                        bass_peaks: r.get(8)?,
                        sample_rate: r.get(9)?,
                        stale: StaleParts {
                            rhythm: r.get::<_, i64>(10)? != VERSIONS.rhythm,
                            loudness: r.get::<_, i64>(11)? != VERSIONS.loudness,
                            bass: r.get::<_, i64>(12)? != VERSIONS.bass,
                        },
                    })
                },
            )
            .optional()?;
        drop(conn);
        Ok(row.and_then(Row::into_cached))
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
        let bass_peaks = saved
            .bass_peaks
            .as_ref()
            .and_then(|b| serde_json::to_string(b).ok());
        let row = params![
            path.to_string_lossy(),
            stamp.size as i64,
            stamp.modified,
            LEGACY_VERSION,
            saved.frames() as i64,
            saved.beats.as_ref().map(|b| b.bpm),
            deflate(&delta(&saved.levels, BANDS)),
            deflate(&delta(&saved.meters, VALUES_PER_FRAME)),
            deflate(&onset),
            beats,
            song_map,
            saved.loudness.map(|l| l.integrated_lufs),
            saved.loudness.map(|l| l.true_peak_dbtp),
            bass_peaks,
            saved.sample_rate,
            VERSIONS.spectrum,
            VERSIONS.rhythm,
            VERSIONS.loudness,
            VERSIONS.bass,
        ];
        self.lock()?.execute(
            "INSERT OR REPLACE INTO analyses
                 (path, file_size, modified, version, frames, bpm, levels, meters, onset, beats,
                  song_map, loudness_lufs, true_peak_dbtp, bass_peaks, sample_rate,
                  spectrum_version, rhythm_version, loudness_version, bass_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
                     ?18, ?19)",
            row,
        )?;
        Ok(())
    }

    /// Analizi olmayan ya da bir parçası eskimiş kütüphane şarkıları ve kütüphanedeki (son
    /// taramadaki) damgaları (en fazla `limit`). Kütüphane tablosu yoksa boş döner.
    ///
    /// Yalnızca ritmi eskimiş olanlar (şarkı çözülmeden, milisaniyelerde yenilenir) önce
    /// gelir: ritim hesabı değişince bütün kütüphanenin haritası çabucak güncellenir.
    pub fn pending_library_tracks(
        &self,
        limit: usize,
    ) -> Result<Vec<(PathBuf, FileStamp)>, CacheError> {
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
            "SELECT t.path, t.file_size, t.modified FROM tracks t
             LEFT JOIN analyses a ON a.path = t.path AND a.file_size = t.file_size
                 AND a.modified = t.modified
             WHERE a.path IS NULL OR a.spectrum_version != ?1 OR a.rhythm_version != ?2
                 OR a.loudness_version != ?3 OR a.bass_version != ?4
             ORDER BY (a.path IS NULL OR a.spectrum_version != ?1 OR a.loudness_version != ?3
                       OR a.bass_version != ?4),
                      t.sort_key
             LIMIT ?5",
        )?;
        let v = VERSIONS;
        let values = params![v.spectrum, v.rhythm, v.loudness, v.bass, limit as i64];
        let rows = stmt.query_map(values, |r| {
            Ok((
                PathBuf::from(r.get::<_, String>(0)?),
                FileStamp {
                    size: r.get::<_, i64>(1)? as u64,
                    modified: r.get(2)?,
                },
            ))
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }
}

/// Veritabanındaki bir analiz kaydı (çözülmemiş hâli).
struct Row {
    frames: i64,
    levels: Vec<u8>,
    meters: Vec<u8>,
    onset: Vec<u8>,
    beats: Option<String>,
    song_map: Option<String>,
    lufs: Option<f64>,
    peak: Option<f64>,
    bass_peaks: Option<String>,
    sample_rate: Option<u32>,
    stale: StaleParts,
}

impl Row {
    /// Kaydı çözer; bozuksa `None`. Eskimiş parçalar boş bırakılır.
    fn into_cached(self) -> Option<CachedAnalysis> {
        let stale = self.stale;
        let saved = SavedAnalysis {
            levels: undelta(&inflate(&self.levels)?, BANDS),
            meters: undelta(&inflate(&self.meters)?, VALUES_PER_FRAME),
            onset: inflate(&self.onset)?
                .chunks_exact(4)
                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .collect(),
            beats: self
                .beats
                .filter(|_| !stale.rhythm)
                .and_then(|j| serde_json::from_str::<BeatGrid>(&j).ok()),
            song_map: self
                .song_map
                .filter(|_| !stale.rhythm)
                .and_then(|j| serde_json::from_str::<SongMap>(&j).ok()),
            loudness: self.lufs.zip(self.peak).filter(|_| !stale.loudness).map(
                |(integrated_lufs, true_peak_dbtp)| Loudness {
                    integrated_lufs,
                    true_peak_dbtp,
                },
            ),
            bass_peaks: self
                .bass_peaks
                .filter(|_| !stale.bass)
                .and_then(|j| serde_json::from_str::<BassPeaks>(&j).ok()),
            sample_rate: self.sample_rate,
        };
        (saved.is_consistent() && saved.frames() as i64 == self.frames)
            .then_some(CachedAnalysis { saved, stale })
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
            loudness: Some(Loudness {
                integrated_lufs: -9.25,
                true_peak_dbtp: 0.4,
            }),
            bass_peaks: Some(BassPeaks {
                shelf_rise_db: [0.2, 0.5, 1.25, 2.5, 4.0, 6.5, 8.75, 11.0, 13.5],
                octave_band_db: -7.5,
            }),
            sample_rate: Some(44_100),
        }
    }

    /// Kaydın güncel hâli (bütün parçaları geçerliyse).
    fn fresh(cache: &AnalysisCache, path: &Path, stamp: FileStamp) -> Option<SavedAnalysis> {
        cache
            .load(path, stamp)
            .unwrap()
            .filter(|c| !c.stale.any())
            .map(|c| c.saved)
    }

    /// Bir parçanın sürümünü elle eskitir (o parçanın hesabı değişmiş gibi).
    fn age(cache: &AnalysisCache, column: &str) {
        cache
            .lock()
            .unwrap()
            .execute(&format!("UPDATE analyses SET {column} = {column} - 1"), [])
            .unwrap();
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
        assert_eq!(fresh(&cache, path, STAMP), Some(saved));
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
        // Kareleri eski sürümle hesaplanmış kayıt da geçersiz: şarkı baştan analiz edilir.
        age(&cache, "spectrum_version");
        assert_eq!(cache.load(path, STAMP).unwrap(), None);
    }

    #[test]
    fn yalnizca_eskiyen_parca_bos_doner_digerleri_kullanilir() {
        let cache = AnalysisCache::open_in_memory().unwrap();
        let path = Path::new("/muzik/a.mp3");
        let saved = sample(60);
        cache.store(path, STAMP, &saved).unwrap();

        // Ritim hesabı değişti: kareler, ses yüksekliği ve bas tepeleri kullanılır.
        age(&cache, "rhythm_version");
        let cached = cache.load(path, STAMP).unwrap().unwrap();
        assert_eq!(
            cached.stale,
            StaleParts {
                rhythm: true,
                ..StaleParts::default()
            }
        );
        assert!(!cached.stale.needs_decoding(), "ritim için şarkı çözülmez");
        assert_eq!(cached.saved.levels, saved.levels);
        assert_eq!(cached.saved.onset, saved.onset);
        assert_eq!((cached.saved.beats, cached.saved.song_map), (None, None));
        assert_eq!(cached.saved.loudness, saved.loudness);
        assert_eq!(cached.saved.bass_peaks, saved.bass_peaks);
        assert_eq!(cached.saved.sample_rate, Some(44_100));

        // Yenilenip saklanınca yeniden tamamen güncel.
        cache.store(path, STAMP, &saved).unwrap();
        assert_eq!(fresh(&cache, path, STAMP), Some(saved.clone()));

        // Ses yüksekliği ya da bas tepeleri eskiyince şarkı çözülmeli; ritim kullanılır.
        age(&cache, "loudness_version");
        age(&cache, "bass_version");
        let cached = cache.load(path, STAMP).unwrap().unwrap();
        assert!(cached.stale.loudness && cached.stale.bass && !cached.stale.rhythm);
        assert!(cached.stale.needs_decoding());
        assert_eq!(cached.saved.beats, saved.beats);
        assert_eq!(
            (cached.saved.loudness, cached.saved.bass_peaks),
            (None, None)
        );
    }

    #[test]
    fn yalnizca_ritmi_eskiyenler_once_yenilenir() {
        use crate::library::Library;
        let db = crate::audio::test_util::temp_path("oncelik.sqlite3");
        let mut library = Library::open(&db).unwrap();
        let folder = library.add_folder("/muzik").unwrap();
        let cache = AnalysisCache::open(&db).unwrap();
        // Sırayla: analizi olmayan, sesi eskimiş, ritmi eskimiş, güncel.
        let names = ["a-yok", "b-ses", "c-ritim", "d-guncel"];
        let tracks: Vec<_> = names
            .iter()
            .map(|name| {
                let mut info = crate::audio::test_util::track_info(&format!("/muzik/{name}.flac"));
                info.title = Some((*name).to_owned());
                (info, STAMP)
            })
            .collect();
        library.apply_changes(folder.id, &tracks, &[]).unwrap();
        for name in &names[1..] {
            cache
                .store(
                    Path::new(&format!("/muzik/{name}.flac")),
                    STAMP,
                    &sample(10),
                )
                .unwrap();
        }
        let set = |name: &str, column: &str| {
            cache
                .lock()
                .unwrap()
                .execute(
                    &format!("UPDATE analyses SET {column} = 0 WHERE path = ?1"),
                    [format!("/muzik/{name}.flac")],
                )
                .unwrap();
        };
        set("b-ses", "loudness_version");
        set("c-ritim", "rhythm_version");
        let pending: Vec<String> = cache
            .pending_library_tracks(10)
            .unwrap()
            .into_iter()
            .map(|(p, _)| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(
            pending,
            [
                "/muzik/c-ritim.flac",
                "/muzik/a-yok.flac",
                "/muzik/b-ses.flac"
            ]
        );
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
            loudness: None,
            bass_peaks: None,
            ..sample(30)
        };
        cache.store(path, STAMP, &saved).unwrap();
        assert_eq!(fresh(&cache, path, STAMP), Some(saved));
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
