//! Klasör taraması: ses dosyalarını bulur, etiketlerini okur ve kütüphaneyi günceller.
//!
//! - Değişmeyen dosyalar (aynı boyut ve değişme zamanı) yeniden okunmaz.
//! - Etiketler birkaç iş parçacığında paralel okunur.
//! - Diskte artık olmayan dosyalar kütüphaneden çıkarılır.
//! - Okunamayan (bozuk ya da desteklenmeyen) dosyalar atlanır, tarama sürer.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::sync::Mutex;

use serde::Serialize;
use walkdir::WalkDir;

use super::db::FileStamp;
use super::{FolderRow, Library, LibraryError};
use crate::audio::decode::{Decoder, TrackInfo, SUPPORTED_EXTENSIONS};

/// Veritabanına tek işlemde yazılan şarkı sayısı.
const BATCH: usize = 200;
/// Etiket okuyan en fazla iş parçacığı.
const MAX_WORKERS: usize = 4;

/// Arayüze gösterilen tarama durumu (iş parçacıkları arasında paylaşılır).
#[derive(Debug, Default)]
pub struct ScanState {
    scanning: AtomicBool,
    found: AtomicUsize,
    processed: AtomicUsize,
    cancel: AtomicBool,
    current: Mutex<Option<String>>,
}

/// Tarama durumunun anlık görüntüsü.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub scanning: bool,
    /// Taranan klasördeki ses dosyası sayısı.
    pub found: usize,
    /// İşlenen dosya sayısı (değişmeyenler dahil).
    pub processed: usize,
    /// Şu an taranan klasör.
    pub current: Option<String>,
}

/// Bir klasör taramasının özeti.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScanSummary {
    pub found: usize,
    /// Etiketi okunup eklenen ya da güncellenen dosya sayısı.
    pub updated: usize,
    /// Değişmediği için atlanan dosya sayısı.
    pub unchanged: usize,
    /// Okunamadığı için atlanan dosya sayısı.
    pub failed: usize,
    /// Diskten silindiği için kütüphaneden çıkarılan dosya sayısı.
    pub removed: usize,
}

impl ScanState {
    pub fn progress(&self) -> ScanProgress {
        ScanProgress {
            scanning: self.scanning.load(Ordering::Acquire),
            found: self.found.load(Ordering::Relaxed),
            processed: self.processed.load(Ordering::Relaxed),
            current: self.current.lock().ok().and_then(|c| c.clone()),
        }
    }

    pub fn is_scanning(&self) -> bool {
        self.scanning.load(Ordering::Acquire)
    }

    /// Süren taramayı durdurur (program kapanırken).
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }

    fn begin(&self, folder: &str) {
        self.found.store(0, Ordering::Relaxed);
        self.processed.store(0, Ordering::Relaxed);
        if let Ok(mut current) = self.current.lock() {
            *current = Some(folder.to_owned());
        }
    }

    fn set_scanning(&self, scanning: bool) {
        self.scanning.store(scanning, Ordering::Release);
        if !scanning {
            if let Ok(mut current) = self.current.lock() {
                *current = None;
            }
        }
    }
}

/// Verilen klasörleri sırayla tarar. Tarama süresince `state.scanning` açıktır.
pub fn scan_folders(
    library: &Mutex<Library>,
    folders: &[FolderRow],
    state: &ScanState,
) -> Result<ScanSummary, LibraryError> {
    state.set_scanning(true);
    let mut total = ScanSummary::default();
    let result = folders.iter().try_for_each(|folder| {
        let summary = scan_folder(library, folder, state)?;
        total.found += summary.found;
        total.updated += summary.updated;
        total.unchanged += summary.unchanged;
        total.failed += summary.failed;
        total.removed += summary.removed;
        Ok(())
    });
    state.set_scanning(false);
    result.map(|()| total)
}

/// Tek bir kaynağı (klasör ya da tek şarkı) tarar ve kütüphaneyi günceller.
pub fn scan_folder(
    library: &Mutex<Library>,
    folder: &FolderRow,
    state: &ScanState,
) -> Result<ScanSummary, LibraryError> {
    state.begin(&folder.path);
    let root = Path::new(&folder.path);
    if !(root.is_dir() || root.is_file() && is_audio(root)) {
        return Err(LibraryError::FolderMissing(folder.path.clone()));
    }

    let files = find_audio_files(root);
    state.found.store(files.len(), Ordering::Relaxed);
    let known = lock(library)?.known_files(folder.id)?;

    let on_disk: HashSet<String> = files
        .iter()
        .map(|(p, _)| p.to_string_lossy().into_owned())
        .collect();
    let removed: Vec<String> = known
        .keys()
        .filter(|p| !on_disk.contains(*p))
        .cloned()
        .collect();
    let to_read: Vec<(PathBuf, FileStamp)> = files
        .into_iter()
        .filter(|(path, stamp)| known.get(path.to_string_lossy().as_ref()) != Some(stamp))
        .collect();

    let mut summary = ScanSummary {
        found: on_disk.len(),
        unchanged: on_disk.len() - to_read.len(),
        removed: removed.len(),
        ..ScanSummary::default()
    };
    state.processed.store(summary.unchanged, Ordering::Relaxed);

    // Etiketleri paralel oku; sonuçları tek bir yazıcı toplu halde veritabanına yazar.
    let workers = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, MAX_WORKERS);
    let next = AtomicUsize::new(0);
    let (tx, rx) = mpsc::channel::<Option<(TrackInfo, FileStamp)>>();
    let write_result = std::thread::scope(|scope| {
        for _ in 0..workers {
            let tx = tx.clone();
            let (next, to_read) = (&next, &to_read);
            scope.spawn(move || loop {
                if state.cancel.load(Ordering::Acquire) {
                    break;
                }
                let i = next.fetch_add(1, Ordering::Relaxed);
                let Some((path, stamp)) = to_read.get(i) else {
                    break;
                };
                let info = Decoder::open(path).ok().map(|d| (d.info().clone(), *stamp));
                if tx.send(info).is_err() {
                    break;
                }
            });
        }
        drop(tx);

        let mut batch = Vec::with_capacity(BATCH);
        for result in rx {
            state.processed.fetch_add(1, Ordering::Relaxed);
            match result {
                Some(item) => batch.push(item),
                None => summary.failed += 1,
            }
            if batch.len() >= BATCH {
                summary.updated += batch.len();
                lock(library)?.apply_changes(folder.id, &batch, &[])?;
                batch.clear();
            }
        }
        summary.updated += batch.len();
        lock(library)?.apply_changes(folder.id, &batch, &removed)
    });
    write_result?;
    Ok(summary)
}

/// Klasördeki (alt klasörler dahil) desteklenen ses dosyaları ve damgaları.
fn find_audio_files(root: &Path) -> Vec<(PathBuf, FileStamp)> {
    WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file() && is_audio(entry.path()))
        .filter_map(|entry| {
            let meta = entry.metadata().ok()?;
            Some((entry.into_path(), FileStamp::of_metadata(&meta)))
        })
        .collect()
}

/// Uzantısı desteklenen bir ses dosyası mı? (Büyük/küçük harf fark etmez.)
pub fn is_audio(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        SUPPORTED_EXTENSIONS
            .iter()
            .any(|s| s.eq_ignore_ascii_case(e))
    })
}

fn lock(library: &Mutex<Library>) -> Result<std::sync::MutexGuard<'_, Library>, LibraryError> {
    library
        .lock()
        .map_err(|_| LibraryError::Database(rusqlite::Error::InvalidQuery))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::temp_path;

    /// Test verisinden geçici bir müzik klasörü kurar:
    /// 4 şarkı (biri alt klasörde), bir metin dosyası ve bozuk bir ".mp3".
    fn music_folder() -> PathBuf {
        let dir = temp_path("muzik");
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        std::fs::create_dir_all(dir.join("Alt Klasör")).unwrap();
        for ext in ["mp3", "m4a", "ogg"] {
            let name = format!("uc-uc-ton.{ext}");
            std::fs::copy(data.join(&name), dir.join(name)).unwrap();
        }
        std::fs::copy(
            data.join("uc-uc-ton.flac"),
            dir.join("Alt Klasör/şarkı.FLAC"),
        )
        .unwrap();
        std::fs::write(dir.join("notlar.txt"), "ses değil").unwrap();
        std::fs::write(dir.join("bozuk.mp3"), b"bozuk".repeat(50)).unwrap();
        dir
    }

    fn setup() -> (Mutex<Library>, FolderRow, PathBuf) {
        let dir = music_folder();
        let library = Library::open_in_memory().unwrap();
        let folder = library.add_folder(&dir.to_string_lossy()).unwrap();
        (Mutex::new(library), folder, dir)
    }

    #[test]
    fn klasoru_alt_klasorleriyle_tarar_bozuk_dosyayi_atlar() {
        let (library, folder, _dir) = setup();
        let state = ScanState::default();
        let summary = scan_folders(&library, &[folder], &state).unwrap();
        assert_eq!(
            summary.found, 5,
            "4 şarkı + bozuk mp3; metin dosyası sayılmaz"
        );
        assert_eq!(summary.updated, 4);
        assert_eq!(summary.failed, 1);
        assert!(!state.is_scanning());
        assert_eq!(state.progress().processed, 5);

        let lib = library.lock().unwrap();
        assert_eq!(lib.track_count().unwrap(), 4);
        let found = lib.search("deneme albümü", 10).unwrap();
        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|t| t.title == "Üç Ton"));
    }

    #[test]
    fn yeniden_taramada_degismeyenleri_atlar_silinenleri_cikarir() {
        let (library, folder, dir) = setup();
        let state = ScanState::default();
        scan_folder(&library, &folder, &state).unwrap();

        let again = scan_folder(&library, &folder, &state).unwrap();
        assert_eq!(
            again.unchanged, 4,
            "bozuk dosya kayıtlı olmadığı için yeniden denenir"
        );
        assert_eq!(again.updated, 0);

        std::fs::remove_file(dir.join("uc-uc-ton.ogg")).unwrap();
        let after = scan_folder(&library, &folder, &state).unwrap();
        assert_eq!(after.removed, 1);
        assert_eq!(library.lock().unwrap().track_count().unwrap(), 3);
    }

    #[test]
    fn degisen_dosya_yeniden_okunur() {
        let (library, folder, dir) = setup();
        let state = ScanState::default();
        scan_folder(&library, &folder, &state).unwrap();

        // Dosyayı başka bir şarkıyla değiştir (boyut değişir).
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        std::fs::copy(data.join("uc-uc-ton.flac"), dir.join("uc-uc-ton.mp3")).unwrap();
        let after = scan_folder(&library, &folder, &state).unwrap();
        assert_eq!(after.updated, 1);
        let lib = library.lock().unwrap();
        let mp3 = lib.search("uc-uc-ton", 10).unwrap();
        assert!(mp3
            .iter()
            .any(|t| t.path.ends_with("uc-uc-ton.mp3") && t.codec == "flac"));
    }

    #[test]
    fn olmayan_klasor_hata_verir_ve_tarama_durumu_kapanir() {
        let library = Mutex::new(Library::open_in_memory().unwrap());
        let folder = library
            .lock()
            .unwrap()
            .add_folder("/olmayan/klasor")
            .unwrap();
        let state = ScanState::default();
        let error = scan_folders(&library, &[folder], &state).unwrap_err();
        assert!(matches!(error, LibraryError::FolderMissing(_)));
        assert!(!state.is_scanning());
    }
}
