//! Kütüphane veritabanının (`library.sqlite3`) şeması ve göçleri.
//!
//! Kütüphane ([`crate::library`]) ve analiz önbelleği ([`crate::analysis::cache`]) aynı
//! dosyayı kullanır. Şema yalnızca burada, sıralı göç adımlarıyla tanımlanır:
//! `MIGRATIONS[i]` veritabanını `i` sürümünden `i + 1` sürümüne taşır. Sürüm, SQLite'ın
//! `user_version` alanında durur.
//!
//! Kurallar:
//! - Şema değişince listenin **sonuna** yeni adım eklenir (`ALTER TABLE …`, yeni tablo).
//!   Yayımlanmış adımlar asla değiştirilmez: eski kullanıcıların dosyası açılışta
//!   sırayla güncellenir, yeni kurulum bütün adımları baştan uygular.
//! - Analiz sonucunun hesabı değişirse şema değil o parçanın sürümü artırılır
//!   (`analysis::cache::VERSIONS`).
//! - Dosya daha yeni bir Lyraska sürümünce oluşturulmuşsa açılmaz (eski sürüm yeni
//!   şemayı bozabilir); kullanıcıya programı güncellemesi söylenir.

use rusqlite::{Connection, TransactionBehavior};
use thiserror::Error;

/// 1: klasörler, şarkılar, analiz önbelleği; şarkı silinince analizi de silinir.
const V1: &str = "
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
CREATE TRIGGER IF NOT EXISTS analyses_follow_tracks AFTER DELETE ON tracks
BEGIN DELETE FROM analyses WHERE path = old.path; END;
";

/// 2: analize ses yüksekliği (EBU R128, LUFS) ve gerçek tepe (dBTP).
const V2: &str = "
ALTER TABLE analyses ADD COLUMN loudness_lufs REAL;
ALTER TABLE analyses ADD COLUMN true_peak_dbtp REAL;
";

/// 3: analize bas tepeleri (bas düğmesinin şarkının tepesini ne kadar yükselttiği; JSON).
const V3: &str = "
ALTER TABLE analyses ADD COLUMN bass_peaks TEXT;
";

/// 4: şarkıların etiket okuma sürümü ([`crate::library::db::TAGS_VERSION`]). Eski tip (ID3v1)
/// etiketler Türkçe harflerle okunmaya başladı: etiketi eksik ya da harfleri bozuk görünen
/// (Ş, Ğ, İ yerine Þ, Ð, Ý) MP3'ler bir sonraki taramada yeniden okunur; diğerleri ve analiz
/// önbelleği olduğu gibi kalır.
const V4: &str = "
ALTER TABLE tracks ADD COLUMN tags_version INTEGER NOT NULL DEFAULT 1;
UPDATE tracks SET tags_version = 0
 WHERE lower(path) LIKE '%.mp3'
   AND (title IS NULL OR artist IS NULL OR album IS NULL
        OR title GLOB '*[ÐÝÞðýþ]*' OR artist GLOB '*[ÐÝÞðýþ]*'
        OR album GLOB '*[ÐÝÞðýþ]*' OR album_artist GLOB '*[ÐÝÞðýþ]*');
";

/// 5: analiz parçalarının ayrı sürümleri ([`crate::analysis::cache::VERSIONS`]) ve şarkının
/// örnekleme hızı (ritim saklanan karelerden yeniden hesaplanırken vuruş gecikmesi için).
/// Eski tek sürümlü kayıtlar parçalara çevrilir; o sürümde geçerli olan parça geçerli kalır:
/// kareler ve ritim sürüm 2'den, ses yüksekliği 3'ten, bas tepeleri (27 dB'ye kadar) 5'ten
/// beri değişmedi. Böylece bu göç hiçbir şarkıyı yeniden analiz ettirmez.
const V5: &str = "
ALTER TABLE analyses ADD COLUMN spectrum_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analyses ADD COLUMN rhythm_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analyses ADD COLUMN loudness_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analyses ADD COLUMN bass_version INTEGER NOT NULL DEFAULT 0;
ALTER TABLE analyses ADD COLUMN sample_rate INTEGER;
UPDATE analyses SET
    spectrum_version = CASE WHEN version >= 2 THEN 1 ELSE 0 END,
    rhythm_version = CASE WHEN version >= 2 THEN 1 ELSE 0 END,
    loudness_version = CASE WHEN version >= 3 THEN 1 ELSE 0 END,
    bass_version = CASE WHEN version >= 5 THEN 1 ELSE 0 END,
    sample_rate = (SELECT t.sample_rate FROM tracks t WHERE t.path = analyses.path);
";

/// Sıralı göç adımları (bkz. modül belgesi). Yalnızca sona eklenir.
const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5];

/// Bu sürümün bildiği en yeni şema.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Kütüphane veritabanı hatası: {0}")]
    Database(#[from] rusqlite::Error),
    #[error(
        "Kütüphane dosyası Lyraska'nın daha yeni bir sürümüyle oluşturulmuş (şema {found}, \
         bu sürüm en fazla {supported}). Lütfen programı güncelleyin."
    )]
    Newer { found: i64, supported: i64 },
}

/// Veritabanını bu sürümün şemasına getirir. Aynı dosyayı aynı anda açan bağlantılar
/// (kütüphane ve önbellek) sırayla bekler; her adım bir kez uygulanır.
pub fn migrate(conn: &mut Connection) -> Result<(), SchemaError> {
    apply(conn, MIGRATIONS)
}

fn apply(conn: &mut Connection, steps: &[&str]) -> Result<(), SchemaError> {
    // Yazma kilidi baştan alınır: iki bağlantı aynı anda sürümü okuyup aynı adımı
    // iki kez uygulamaya çalışmaz.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = steps.len() as i64;
    if found > supported {
        return Err(SchemaError::Newer { found, supported });
    }
    let applied = usize::try_from(found).unwrap_or(0);
    for step in steps.iter().skip(applied) {
        tx.execute_batch(step)?;
    }
    if found != supported {
        tx.pragma_update(None, "user_version", supported)?;
    }
    tx.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> i64 {
        conn.pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap()
    }

    fn tables(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect()
    }

    #[test]
    fn yeni_dosya_en_son_semaya_kurulur() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(version(&conn), SCHEMA_VERSION);
        assert_eq!(tables(&conn), ["analyses", "folders", "tracks"]);
    }

    #[test]
    fn surum_yazilmamis_eski_dosya_verisini_korur() {
        // İlk sürümler şemayı (1. adım) kurup `user_version`'ı hep 0 bırakıyordu.
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V1).unwrap();
        conn.execute("INSERT INTO folders (path) VALUES ('C:/Müzik')", [])
            .unwrap();
        migrate(&mut conn).unwrap();
        assert_eq!(version(&conn), SCHEMA_VERSION);
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM folders", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn adimlar_sirayla_ve_bir_kez_uygulanir() {
        let steps = [
            V1,
            "ALTER TABLE analyses ADD COLUMN loudness REAL;",
            "CREATE TABLE notes (id INTEGER PRIMARY KEY);",
        ];
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &steps[..1]).unwrap();
        conn.execute("INSERT INTO folders (path) VALUES ('C:/Müzik')", [])
            .unwrap();
        // Program güncellenir: yalnızca yeni adımlar uygulanır, veri yerinde kalır.
        apply(&mut conn, &steps).unwrap();
        assert_eq!(version(&conn), 3);
        conn.execute(
            "INSERT INTO analyses (path, file_size, modified, version, frames, levels, meters, \
             onset, loudness) VALUES ('a', 1, 1, 1, 1, x'', x'', x'', -14.0)",
            [],
        )
        .unwrap();
        // Yeniden açmak hiçbir şeyi ikinci kez uygulamaz (ALTER ikinci kez hata verirdi).
        apply(&mut conn, &steps).unwrap();
        assert_eq!(tables(&conn), ["analyses", "folders", "notes", "tracks"]);
    }

    #[test]
    fn etiketi_bozuk_gorunen_mp3ler_yeniden_okunmak_uzere_isaretlenir() {
        // 0.0.32'nin veritabanı (3. adım): ID3v1 etiketleri Latin-1 okunmuştu.
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &MIGRATIONS[..3]).unwrap();
        conn.execute("INSERT INTO folders (id, path) VALUES (1, 'C:/Müzik')", [])
            .unwrap();
        let rows = [
            (
                "C:/Müzik/bozuk.mp3",
                Some("Þarký"),
                Some("Sanatçı"),
                Some("Albüm"),
            ),
            ("C:/Müzik/etiketsiz.MP3", None, None, None),
            (
                "C:/Müzik/saglam.mp3",
                Some("Şarkı"),
                Some("Sanatçı"),
                Some("Albüm"),
            ),
            ("C:/Müzik/kayipsiz.flac", None, None, None),
        ];
        for (path, title, artist, album) in rows {
            conn.execute(
                "INSERT INTO tracks (folder_id, path, file_name, title, artist, album, codec,
                     sample_rate, channels, file_size, modified, sort_key, search)
                 VALUES (1, ?1, 'ad', ?2, ?3, ?4, 'mp3', 44100, 2, 1, 1, '', '')",
                rusqlite::params![path, title, artist, album],
            )
            .unwrap();
        }
        migrate(&mut conn).unwrap();
        let mut stmt = conn
            .prepare("SELECT path FROM tracks WHERE tags_version = 0 ORDER BY path")
            .unwrap();
        let stale: Vec<String> = stmt
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(stale, ["C:/Müzik/bozuk.mp3", "C:/Müzik/etiketsiz.MP3"]);
    }

    #[test]
    fn eski_analiz_kayitlari_parca_surumlerine_cevrilir_yeniden_analiz_gerekmez() {
        // 0.0.33'ün veritabanı (4. adım): tek sürümlü analiz kayıtları.
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &MIGRATIONS[..4]).unwrap();
        conn.execute("INSERT INTO folders (id, path) VALUES (1, 'C:/Müzik')", [])
            .unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, file_name, codec, sample_rate, channels,
                 file_size, modified, sort_key, search)
             VALUES (1, 'C:/Müzik/v5.flac', 'v5', 'flac', 48000, 2, 1, 1, '', '')",
            [],
        )
        .unwrap();
        for (path, version) in [
            ("C:/Müzik/v1.flac", 1),
            ("C:/Müzik/v3.flac", 3),
            ("C:/Müzik/v4.flac", 4),
            ("C:/Müzik/v5.flac", 5),
        ] {
            conn.execute(
                "INSERT INTO analyses (path, file_size, modified, version, frames, levels, meters,
                     onset) VALUES (?1, 1, 1, ?2, 0, x'', x'', x'')",
                rusqlite::params![path, version],
            )
            .unwrap();
        }
        migrate(&mut conn).unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT path, spectrum_version, rhythm_version, loudness_version, bass_version,
                        sample_rate
                 FROM analyses ORDER BY path",
            )
            .unwrap();
        let rows: Vec<(String, i64, i64, i64, i64, Option<i64>)> = stmt
            .query_map([], |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        // Göç, eski kayıtları parçaların ilk sürümüne (1) çevirir; 0: yeniden hesaplanır.
        assert_eq!(
            rows,
            [
                ("C:/Müzik/v1.flac".to_owned(), 0, 0, 0, 0, None),
                ("C:/Müzik/v3.flac".to_owned(), 1, 1, 1, 0, None),
                ("C:/Müzik/v4.flac".to_owned(), 1, 1, 1, 0, None),
                ("C:/Müzik/v5.flac".to_owned(), 1, 1, 1, 1, Some(48_000)),
            ]
        );
    }

    #[test]
    fn daha_yeni_surumun_dosyasi_acilmaz() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        let error = migrate(&mut conn).unwrap_err();
        assert!(matches!(error, SchemaError::Newer { .. }), "{error}");
        assert!(error.to_string().contains("güncelleyin"));
        // Dosyaya dokunulmaz.
        assert_eq!(version(&conn), SCHEMA_VERSION + 1);
    }

    #[test]
    fn ayni_dosyayi_iki_baglanti_birlikte_acabilir() {
        let dir = std::env::temp_dir().join(format!("lyraska-sema-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("library.sqlite3");
        let _ = std::fs::remove_file(&path);
        let threads: Vec<_> = (0..4)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || {
                    let mut conn = Connection::open(&path).unwrap();
                    conn.busy_timeout(std::time::Duration::from_secs(5))
                        .unwrap();
                    migrate(&mut conn).unwrap();
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        let conn = Connection::open(&path).unwrap();
        assert_eq!(version(&conn), SCHEMA_VERSION);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
