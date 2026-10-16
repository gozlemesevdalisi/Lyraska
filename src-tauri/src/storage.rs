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
//! - Birinci adım baştan sona `IF NOT EXISTS`'tir ve öyle kalır: 0.0.5–0.0.25 her dosyaya
//!   sürüm 1 yazıyordu ama bu adımın hepsini kurmuyordu (analiz tablosu 0.0.20'de, silme
//!   tetikleyicisi 0.0.22'de geldi). Sürümü 1 olan dosyada adım yeniden uygulanır; eksik olan
//!   kurulur.
//! - Analiz önbelleği yeniden hesaplanabilir, kütüphane verisi değildir: bir adım önbellek
//!   tablosunda takılırsa ya da tablo bu sürümün beklediği sütunlardan birinden yoksunsa önbellek
//!   boşaltılıp yeniden kurulur (şarkılar arka planda yeniden analiz edilir). Klasörlere ve
//!   şarkılara hiçbir koşulda dokunulmaz; göç tutmazsa dosya olduğu gibi kalır.
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

/// 6: analizi kütüphaneden çıkan şarkıyla birlikte silen tetikleyici. 0.0.20–0.0.21'in
/// dosyalarında yoktu ve 0.0.26–0.0.34'ün göçü (sürümü 1 olan dosyada 1. adımı atladığı için)
/// kurmamıştı: çıkarılan şarkıların analizi dosyada kalıyordu. Yalnızca eksikse kurulur.
const V6: &str = "
CREATE TRIGGER IF NOT EXISTS analyses_follow_tracks AFTER DELETE ON tracks
BEGIN DELETE FROM analyses WHERE path = old.path; END;
";

/// 7: ton analizi ([`crate::analysis::key`]): saniye başına kroma (sıkıştırılmış), tahmin
/// (JSON), kütüphane listesi için Camelot kodu (güveni yeterliyse) ve parçanın sürümü. Eski
/// kayıtlarda ton yoktur (sürüm 0): kütüphane arka planda bir kez çözülür; spektrum ve ritim
/// yeniden hesaplanmaz.
const V7: &str = "
ALTER TABLE analyses ADD COLUMN chroma BLOB;
ALTER TABLE analyses ADD COLUMN musical_key TEXT;
ALTER TABLE analyses ADD COLUMN camelot TEXT;
ALTER TABLE analyses ADD COLUMN key_version INTEGER NOT NULL DEFAULT 0;
";

/// Sıralı göç adımları (bkz. modül belgesi). Yalnızca sona eklenir.
const MIGRATIONS: &[&str] = &[V1, V2, V3, V4, V5, V6, V7];

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

/// Göçün sonucu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migrated {
    /// Dosyanın açılıştaki şema sürümü.
    pub from: i64,
    /// Analiz önbelleği şemaya uymadığı için boşaltılıp yeniden kuruldu. Kütüphaneye
    /// dokunulmadı; şarkılar arka planda yeniden analiz edilir.
    pub cache_rebuilt: bool,
}

/// Veritabanını bu sürümün şemasına getirir. Aynı dosyayı aynı anda açan bağlantılar
/// (kütüphane ve önbellek) sırayla bekler; her adım bir kez uygulanır.
pub fn migrate(conn: &mut Connection) -> Result<Migrated, SchemaError> {
    let migrated = apply(conn, MIGRATIONS)?;
    if migrated.from != SCHEMA_VERSION {
        crate::diagnostics::info(&format!(
            "Kütüphane dosyası güncellendi: şema {} → {SCHEMA_VERSION}",
            migrated.from
        ));
    }
    if migrated.cache_rebuilt {
        crate::diagnostics::info(
            "Analiz önbelleği şemaya uymuyordu: boşaltılıp yeniden kuruldu (kütüphaneye \
             dokunulmadı, şarkılar yeniden analiz edilecek)",
        );
    }
    Ok(migrated)
}

fn apply(conn: &mut Connection, steps: &[&str]) -> Result<Migrated, SchemaError> {
    // Yazma kilidi baştan alınır: iki bağlantı aynı anda sürümü okuyup aynı adımı
    // iki kez uygulamaya çalışmaz. Bir hata olursa işlem bütünüyle geri alınır.
    let mut tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let found: i64 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;
    let supported = steps.len() as i64;
    if found > supported {
        return Err(SchemaError::Newer { found, supported });
    }
    // Sürüm 1, 1. adımın tamamlandığını garanti etmez (bkz. modül belgesi).
    let start = if found <= 1 {
        0
    } else {
        usize::try_from(found).unwrap_or(0)
    };
    let mut cache_rebuilt = false;
    let attempt = {
        let savepoint = tx.savepoint()?;
        let result = run(&savepoint, &steps[start..]);
        if result.is_ok() {
            savepoint.commit()?;
        }
        // Tutmadıysa `savepoint` bırakılırken adımların yaptığı her şey geri alınır.
        result
    };
    if let Err(error) = attempt {
        if !is_schema_mismatch(&error) {
            return Err(error.into());
        }
        // Bir adım, dosyanın sürümünün söylediği biçimde olmayan bir tabloda takıldı. Takılan
        // önbellekse, önbellek o sürümdeki biçimiyle boş kurulup adımlar yeniden denenir.
        // Kütüphane tablosundaysa yine tutmaz ve işlem geri alınır: dosyaya dokunulmaz.
        rebuild_cache(&tx, &steps[..start])?;
        run(&tx, &steps[start..])?;
        cache_rebuilt = true;
    }
    // Sürüm güncel görünse de önbellek tablosu eksik ya da bir sütunu yoksa yeniden kurulur.
    if !cache_complete(&tx, steps)? {
        rebuild_cache(&tx, steps)?;
        cache_rebuilt = true;
    }
    if found != supported {
        tx.pragma_update(None, "user_version", supported)?;
    }
    tx.commit()?;
    Ok(Migrated {
        from: found,
        cache_rebuilt,
    })
}

fn run(conn: &Connection, steps: &[&str]) -> rusqlite::Result<()> {
    steps.iter().try_for_each(|step| conn.execute_batch(step))
}

/// Hata, şemanın beklenenden farklı olmasından mı ("no such table", "duplicate column name")?
/// Kilit, disk ve bozulma hataları değildir: onlarda önbelleğe dokunulmaz.
fn is_schema_mismatch(error: &rusqlite::Error) -> bool {
    match error {
        rusqlite::Error::SqliteFailure(e, _) | rusqlite::Error::SqlInputError { error: e, .. } => {
            e.extended_code == rusqlite::ffi::SQLITE_ERROR
        }
        _ => false,
    }
}

/// `steps` adımları boş bir bellek veritabanında uygulanmış hali: bir sürümün beklediği biçim.
fn shape_after(steps: &[&str]) -> rusqlite::Result<Connection> {
    let conn = Connection::open_in_memory()?;
    run(&conn, steps)?;
    Ok(conn)
}

/// Tablonun sütunları (ad, tür); tablo yoksa boş.
fn columns(conn: &Connection, table: &str) -> rusqlite::Result<Vec<(String, String)>> {
    let mut stmt = conn.prepare("SELECT name, type FROM pragma_table_info(?1)")?;
    let rows = stmt.query_map([table], |r| Ok((r.get(0)?, r.get(1)?)))?;
    rows.collect()
}

/// Önbellek tablosunda `steps`'in kurduğu bütün sütunlar var mı?
fn cache_complete(conn: &Connection, steps: &[&str]) -> rusqlite::Result<bool> {
    let expected = columns(&shape_after(steps)?, "analyses")?;
    let found = columns(conn, "analyses")?;
    Ok(expected.iter().all(|column| found.contains(column)))
}

/// Analiz önbelleğini (tablo ve silme tetikleyicisi) boşaltıp `steps` adımlarının kurduğu
/// biçimde yeniden kurar. Kütüphane tablolarına dokunmaz.
fn rebuild_cache(conn: &Connection, steps: &[&str]) -> rusqlite::Result<()> {
    let shape = shape_after(steps)?;
    let mut stmt = shape.prepare(
        "SELECT sql FROM sqlite_master
         WHERE (tbl_name = 'analyses' OR name = 'analyses_follow_tracks') AND sql IS NOT NULL
         ORDER BY type != 'table'",
    )?;
    let objects: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    conn.execute_batch(
        "DROP TRIGGER IF EXISTS analyses_follow_tracks; DROP TABLE IF EXISTS analyses;",
    )?;
    objects.iter().try_for_each(|sql| conn.execute_batch(sql))
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

    /// 0.0.5–0.0.25'in kütüphane şeması (`library/db.rs`, `SCHEMA`), değiştirilmeden alındı.
    /// Bu sürümler veritabanını her açılışta bunu çalıştırıp `user_version`'a 1 yazardı.
    const LEGACY_LIBRARY: &str = "
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

    /// 0.0.20–0.0.25'in analiz önbelleği şeması (`analysis/cache.rs`, `SCHEMA`), değiştirilmeden.
    const LEGACY_CACHE: &str = "
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

    /// 0.0.22–0.0.25'in eklediği silme tetikleyicisi (`library/db.rs`), değiştirilmeden.
    const LEGACY_TRIGGER: &str =
        "CREATE TRIGGER IF NOT EXISTS analyses_follow_tracks AFTER DELETE ON tracks
             BEGIN DELETE FROM analyses WHERE path = old.path; END;";

    /// 0.0.5–0.0.25'in açılışı: `Library::init` (0.0.20'den sonra önbellek tablosu, 0.0.22'den
    /// sonra tetikleyici) ve sabit `user_version = 1`.
    fn open_like_legacy(conn: &Connection, cache: bool, trigger: bool) {
        conn.execute_batch(
            "PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;",
        )
        .unwrap();
        conn.execute_batch(LEGACY_LIBRARY).unwrap();
        if cache {
            conn.execute_batch(LEGACY_CACHE).unwrap();
        }
        if trigger {
            conn.execute_batch(LEGACY_TRIGGER).unwrap();
        }
        conn.pragma_update(None, "user_version", 1).unwrap();
    }

    /// Şemanın özeti: nesneler (tür, ad, tablo) ve tabloların sütunları (ad, tür, NOT NULL,
    /// varsayılan, birincil anahtar). İki dosya aynı özeti veriyorsa aynı sorguları kaldırır.
    fn shape(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare(
                "SELECT type, name, tbl_name FROM sqlite_master
                 WHERE name NOT LIKE 'sqlite%' ORDER BY type, name",
            )
            .unwrap();
        let objects: Vec<(String, String, String)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        let mut out = Vec::new();
        for (kind, name, table) in objects {
            out.push(format!("{kind} {name} ({table})"));
            if kind == "table" {
                let mut columns = conn
                    .prepare(
                        "SELECT name, type, \"notnull\", dflt_value, pk
                         FROM pragma_table_info(?1) ORDER BY name",
                    )
                    .unwrap();
                let rows = columns
                    .query_map([&name], |r| {
                        Ok(format!(
                            "  {} {} notnull={} default={:?} pk={}",
                            r.get::<_, String>(0)?,
                            r.get::<_, String>(1)?,
                            r.get::<_, i64>(2)?,
                            r.get::<_, Option<String>>(3)?,
                            r.get::<_, i64>(4)?,
                        ))
                    })
                    .unwrap();
                out.extend(rows.map(Result::unwrap));
            }
        }
        out
    }

    /// Lyraska'nın bugüne kadar bıraktığı her tür dosya (hangi sürümlerin bıraktığıyla).
    fn every_past_file() -> Vec<(String, Connection)> {
        let mut files = Vec::new();
        let legacy = |cache, trigger| {
            let conn = Connection::open_in_memory().unwrap();
            open_like_legacy(&conn, cache, trigger);
            conn
        };
        files.push(("0.0.5–0.0.19".to_owned(), legacy(false, false)));
        files.push(("0.0.20–0.0.21".to_owned(), legacy(true, false)));
        files.push(("0.0.22–0.0.25".to_owned(), legacy(true, true)));
        // 0.0.26'dan beri sıralı göçler: n adımlık şema.
        for n in 1..=MIGRATIONS.len() {
            let mut conn = Connection::open_in_memory().unwrap();
            apply(&mut conn, &MIGRATIONS[..n]).unwrap();
            files.push((format!("şema {n}"), conn));
        }
        // 0.0.20–0.0.21'in dosyası, 1. adımı atlayan eski göçle (0.0.28–0.0.34) güncellenmiş:
        // analizler yerinde ama silme tetikleyicisi hiç kurulmamış.
        for n in 2..=5 {
            let conn = legacy(true, false);
            for step in &MIGRATIONS[1..n] {
                conn.execute_batch(step).unwrap();
            }
            conn.pragma_update(None, "user_version", n as i64).unwrap();
            files.push((format!("0.0.20–0.0.21 → şema {n}"), conn));
        }
        files
    }

    #[test]
    fn her_eski_dosya_yeni_kurulumla_ayni_semaya_ulasir() {
        let mut fresh = Connection::open_in_memory().unwrap();
        migrate(&mut fresh).unwrap();
        let expected = shape(&fresh);
        for (name, mut conn) in every_past_file() {
            let migrated = match migrate(&mut conn) {
                Ok(migrated) => migrated,
                Err(error) => panic!("{name}: {error}"),
            };
            assert_eq!(version(&conn), SCHEMA_VERSION, "{name}");
            assert_eq!(shape(&conn), expected, "{name}");
            // Olağan yükseltme analizleri asla silmez (büyük kütüphanede saatlerce sürerdi).
            assert!(!migrated.cache_rebuilt, "{name}");
        }
    }

    fn add_track(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO folders (id, path) VALUES (1, 'C:/Müzik')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tracks (folder_id, path, file_name, codec, sample_rate, channels,
                 file_size, modified, sort_key, search)
             VALUES (1, ?1, 'ad', 'flac', 44100, 2, 1, 1, '', '')",
            [path],
        )
        .unwrap();
    }

    fn count(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn analiz_tablosu_kayipsa_kutuphane_acilir_onbellek_yeniden_kurulur() {
        let mut conn = Connection::open_in_memory().unwrap();
        migrate(&mut conn).unwrap();
        let expected = shape(&conn);
        add_track(&conn, "C:/Müzik/a.flac");
        conn.execute_batch("DROP TRIGGER analyses_follow_tracks; DROP TABLE analyses;")
            .unwrap();
        let migrated = migrate(&mut conn).unwrap();
        assert!(migrated.cache_rebuilt);
        assert_eq!(shape(&conn), expected);
        assert_eq!(count(&conn, "tracks"), 1);
    }

    #[test]
    fn adim_onbellekte_takilirsa_onbellek_yeniden_kurulur_sarkilar_kalir() {
        // Şema 2'de olduğu söylenen dosyanın önbelleğinde 3. adımın sütunu zaten var:
        // 3. adım "duplicate column name" ile takılır.
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &MIGRATIONS[..2]).unwrap();
        add_track(&conn, "C:/Müzik/a.flac");
        conn.execute_batch(
            "ALTER TABLE analyses ADD COLUMN bass_peaks TEXT;
             INSERT INTO analyses (path, file_size, modified, version, frames, levels, meters,
                 onset) VALUES ('C:/Müzik/a.flac', 1, 1, 2, 0, x'', x'', x'');",
        )
        .unwrap();
        let migrated = migrate(&mut conn).unwrap();
        assert!(migrated.cache_rebuilt);
        assert_eq!(version(&conn), SCHEMA_VERSION);
        assert_eq!(count(&conn, "tracks"), 1);
        assert_eq!(count(&conn, "analyses"), 0);
        let mut fresh = Connection::open_in_memory().unwrap();
        migrate(&mut fresh).unwrap();
        assert_eq!(shape(&conn), shape(&fresh));
    }

    #[test]
    fn kutuphane_tablosunda_takilirsa_dosyaya_dokunulmaz() {
        // Şarkı tablosunda 4. adımın sütunu zaten var: önbelleği yeniden kurmak da çözmez.
        let mut conn = Connection::open_in_memory().unwrap();
        apply(&mut conn, &MIGRATIONS[..3]).unwrap();
        add_track(&conn, "C:/Müzik/a.flac");
        conn.execute_batch(
            "ALTER TABLE tracks ADD COLUMN tags_version INTEGER NOT NULL DEFAULT 1;
             INSERT INTO analyses (path, file_size, modified, version, frames, levels, meters,
                 onset) VALUES ('C:/Müzik/a.flac', 1, 1, 3, 0, x'', x'', x'');",
        )
        .unwrap();
        let before = shape(&conn);
        let error = migrate(&mut conn).unwrap_err();
        assert!(error.to_string().contains("duplicate column"), "{error}");
        // Hiçbir şey değişmedi: sürüm, şema, şarkı ve analiz yerinde.
        assert_eq!(version(&conn), 3);
        assert_eq!(shape(&conn), before);
        assert_eq!(count(&conn, "tracks"), 1);
        assert_eq!(count(&conn, "analyses"), 1);
    }

    #[test]
    fn yalnizca_sema_hatasi_onbellegi_yeniden_kurdurur() {
        let failure = |code| rusqlite::Error::SqliteFailure(rusqlite::ffi::Error::new(code), None);
        assert!(is_schema_mismatch(&failure(rusqlite::ffi::SQLITE_ERROR)));
        // Hatanın yeri bilinen SQL hatası da (rusqlite bunu ayrı türde döndürebilir).
        assert!(is_schema_mismatch(&rusqlite::Error::SqlInputError {
            error: rusqlite::ffi::Error::new(rusqlite::ffi::SQLITE_ERROR),
            msg: "no such table: analyses".to_owned(),
            sql: "ALTER TABLE analyses ADD COLUMN x".to_owned(),
            offset: 12,
        }));
        // Kilit, disk ve bozulma: analizler silinmez, hata kullanıcıya gösterilir.
        for code in [
            rusqlite::ffi::SQLITE_BUSY,
            rusqlite::ffi::SQLITE_LOCKED,
            rusqlite::ffi::SQLITE_FULL,
            rusqlite::ffi::SQLITE_IOERR,
            rusqlite::ffi::SQLITE_CORRUPT,
            rusqlite::ffi::SQLITE_READONLY,
        ] {
            assert!(!is_schema_mismatch(&failure(code)), "{code}");
        }
    }

    /// 0.0.19'un yazdığı gibi bir kütüphane dosyası: bir klasör, iki şarkı, analiz tablosu yok.
    fn write_legacy_library(path: &std::path::Path, cache: bool, trigger: bool) {
        let conn = Connection::open(path).unwrap();
        open_like_legacy(&conn, cache, trigger);
        conn.execute("INSERT INTO folders (path) VALUES ('C:/Müzik')", [])
            .unwrap();
        // 0.0.19'un `apply_changes`'indeki sütunlar.
        for (path, title, artist, size) in [
            (
                "C:/Müzik/gece.flac",
                "Gece Yarısı",
                "Işıl Ağaoğlu",
                41_234_567_i64,
            ),
            ("C:/Müzik/şafak.mp3", "Şafak", "Çağrı Öztürk", 9_876_543),
        ] {
            conn.execute(
                "INSERT INTO tracks (folder_id, path, file_name, title, artist, album, album_artist,
                     track_number, disc_number, duration_secs, codec, sample_rate, channels,
                     file_size, modified, sort_key, search)
                 VALUES (1, ?1, ?2, ?2, ?3, 'Lyra', NULL, 1, NULL, 245.5, 'FLAC', 44100, 2,
                     ?4, 1700000000, ?2, lower(?2 || ' ' || ?3))",
                rusqlite::params![path, title, artist, size],
            )
            .unwrap();
        }
    }

    #[test]
    fn surum_0_0_19_kutuphanesi_veri_kaybetmeden_acilir() {
        // Proje sahibinin 0.0.33'te gördüğü hata: "no such table: analyses".
        let path = crate::audio::test_util::temp_path("library.sqlite3");
        write_legacy_library(&path, false, false);

        // Programın açılışı (lib.rs): önce kütüphane, sonra analiz önbelleği.
        let library = crate::library::db::Library::open(&path).unwrap();
        let cache = crate::analysis::cache::AnalysisCache::open(&path).unwrap();

        let folders: Vec<String> = library
            .folders()
            .unwrap()
            .into_iter()
            .map(|f| f.path)
            .collect();
        assert_eq!(folders, ["C:/Müzik"]);
        let tracks = library.search("", 100).unwrap();
        let summary: Vec<_> = tracks
            .iter()
            .map(|t| {
                (
                    t.path.as_str(),
                    t.title.as_str(),
                    t.artist.as_deref(),
                    t.duration_secs,
                )
            })
            .collect();
        assert_eq!(
            summary,
            [
                (
                    "C:/Müzik/gece.flac",
                    "Gece Yarısı",
                    Some("Işıl Ağaoğlu"),
                    Some(245.5)
                ),
                (
                    "C:/Müzik/şafak.mp3",
                    "Şafak",
                    Some("Çağrı Öztürk"),
                    Some(245.5)
                ),
            ]
        );
        // Analiz önbelleği çalışır: iki şarkı da analiz sırasına girer.
        assert_eq!(cache.pending_library_tracks(10).unwrap().len(), 2);
        // Kütüphaneden çıkan şarkının analizi de silinir (tetikleyici kuruldu).
        cache
            .lock_for_test()
            .execute(
                "INSERT INTO analyses (path, file_size, modified, version, frames, levels, meters,
                     onset) VALUES ('C:/Müzik/gece.flac', 1, 1, 0, 0, x'', x'', x'')",
                [],
            )
            .unwrap();
        library.remove_folder(1).unwrap();
        let left: i64 = cache
            .lock_for_test()
            .query_row("SELECT COUNT(*) FROM analyses", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 0);
    }

    #[test]
    fn surum_0_0_25_kutuphanesinin_analizleri_korunur() {
        // 0.0.23–0.0.25: analizler (sürüm 2) kütüphaneyle aynı dosyada.
        let path = crate::audio::test_util::temp_path("library.sqlite3");
        write_legacy_library(&path, true, true);
        {
            let conn = Connection::open(&path).unwrap();
            conn.execute(
                "INSERT INTO analyses (path, file_size, modified, version, frames, bpm, levels,
                     meters, onset, beats, song_map)
                 VALUES ('C:/Müzik/gece.flac', 41234567, 1700000000, 2, 3, 128.0, x'0102',
                     x'0304', x'0506', '{}', '{}')",
                [],
            )
            .unwrap();
        }
        let library = crate::library::db::Library::open(&path).unwrap();
        assert_eq!(library.search("", 100).unwrap().len(), 2);
        drop(library);
        let conn = Connection::open(&path).unwrap();
        let row: (i64, Option<f64>, Vec<u8>, i64) = conn
            .query_row(
                "SELECT frames, bpm, levels, spectrum_version FROM analyses
                 WHERE path = 'C:/Müzik/gece.flac'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .unwrap();
        // Kareler geçerli sayılır: şarkı baştan analiz edilmez.
        assert_eq!(row, (3, Some(128.0), vec![1, 2], 1));
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
