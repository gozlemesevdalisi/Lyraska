//! Kütüphane servisi: veritabanını ve arka plan taramasını bir arada yönetir.
//! Tauri komutları yalnızca bu servisi çağırır.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::db::MAX_RESULTS;
use super::scan::{self, ScanProgress, ScanState};
use super::{path_covers, FolderRow, Library, LibraryError, TrackRow};

/// Arayüzün gösterdiği kütüphane durumu.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    pub folders: Vec<FolderRow>,
    pub track_count: usize,
    /// Analizi hazır şarkı sayısı (BPM ve şarkı haritası önbellekte).
    pub analyzed: usize,
    pub scan: ScanProgress,
    /// Son taramadaki sorunlar (ör. "Klasör bulunamadı: D:\Müzik").
    pub problems: Vec<String>,
}

/// Pencereye bırakılanların sonucu.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DropOutcome {
    /// Çalınacak şarkılar, bırakılış sırasıyla (zaten kütüphanede olanlar dahil).
    pub tracks: Vec<String>,
    /// Kütüphaneye yeni eklenen şarkılar.
    pub added_tracks: usize,
    /// Kütüphaneye yeni eklenen klasörler (şarkıları arka planda taranır).
    pub added_folders: usize,
    /// Zaten kütüphanede olanlar (şarkı ya da klasör).
    pub already: usize,
    /// Eklenemeyenler (ör. "Bu dosya türü desteklenmiyor: notlar.txt").
    pub problems: Vec<String>,
}

pub struct LibraryService {
    library: Arc<Mutex<Library>>,
    scan: Arc<ScanState>,
    /// Tarama sürerken gelen yeni tarama isteği.
    pending: Arc<AtomicBool>,
    worker_running: Arc<AtomicBool>,
    problems: Arc<Mutex<Vec<String>>>,
}

impl LibraryService {
    pub fn open(path: &Path) -> Result<Self, LibraryError> {
        Ok(Self::new(Library::open(path)?))
    }

    pub fn new(library: Library) -> Self {
        Self {
            library: Arc::new(Mutex::new(library)),
            scan: Arc::new(ScanState::default()),
            pending: Arc::new(AtomicBool::new(false)),
            worker_running: Arc::new(AtomicBool::new(false)),
            problems: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Library>, LibraryError> {
        self.library
            .lock()
            .map_err(|_| LibraryError::Database(rusqlite::Error::InvalidQuery))
    }

    pub fn status(&self) -> Result<LibraryStatus, LibraryError> {
        let lib = self.lock()?;
        Ok(LibraryStatus {
            folders: lib.folders()?,
            track_count: lib.track_count()?,
            analyzed: lib.analyzed_count()?,
            scan: self.scan.progress(),
            problems: self.problems.lock().map(|p| p.clone()).unwrap_or_default(),
        })
    }

    pub fn search(&self, query: &str, limit: Option<usize>) -> Result<Vec<TrackRow>, LibraryError> {
        self.lock()?.search(query, limit.unwrap_or(MAX_RESULTS))
    }

    /// Kaynak ekler — bir klasör ya da tek bir şarkı — ve taramayı başlatır.
    ///
    /// Zaten kütüphanedeki bir klasörün içindeyse eklenmez (şarkılar iki kez
    /// görünmesin). Yeni klasör daha önce eklenmiş kaynakları kapsıyorsa onların
    /// yerini alır; şarkıları bu klasörün taramasıyla geri gelir.
    pub fn add_folder(&self, path: &Path) -> Result<FolderRow, LibraryError> {
        let text = path.to_string_lossy().into_owned();
        if path.is_file() && !scan::is_audio(path) {
            return Err(LibraryError::UnsupportedFile(text));
        }
        if !path.exists() {
            return Err(LibraryError::FolderMissing(text));
        }
        let folder = {
            let lib = self.lock()?;
            let existing = lib.folders()?;
            if let Some(parent) = existing
                .iter()
                .find(|f| path_covers(Path::new(&f.path), path))
            {
                return Err(if path_covers(path, Path::new(&parent.path)) {
                    LibraryError::FolderExists
                } else {
                    LibraryError::AlreadyCovered(parent.path.clone())
                });
            }
            for covered in existing
                .iter()
                .filter(|f| path_covers(path, Path::new(&f.path)))
            {
                lib.remove_folder(covered.id)?;
            }
            lib.add_folder(&text)?
        };
        self.request_scan();
        Ok(folder)
    }

    /// Pencereye bırakılanları kütüphaneye ekler: klasörler ve şarkılar kaynak olur,
    /// şarkılar ayrıca çalınmak üzere bırakılış sırasıyla döndürülür.
    ///
    /// Dosya mı klasör mü olduğu diskte denetlenir (uzantıya bakılmaz). Zaten
    /// kütüphanede olan şarkı yine çalınır. Biri eklenemese de diğerleri eklenir;
    /// her sorun kullanıcıya gösterilecek bir cümle olarak döner.
    pub fn add_dropped(&self, paths: &[PathBuf]) -> DropOutcome {
        let mut outcome = DropOutcome::default();
        for path in paths {
            let name = path
                .file_name()
                .map_or_else(|| path.to_string_lossy(), |n| n.to_string_lossy())
                .into_owned();
            let is_track = path.is_file();
            if is_track && !scan::is_audio(path) {
                outcome
                    .problems
                    .push(LibraryError::UnsupportedFile(name).to_string());
                continue;
            }
            if !is_track && !path.is_dir() {
                // Ör. zip içinden ya da taşınmış dosya: diskte yok.
                outcome
                    .problems
                    .push(LibraryError::FolderMissing(name).to_string());
                continue;
            }
            if is_track {
                outcome.tracks.push(path.to_string_lossy().into_owned());
            }
            match self.add_folder(path) {
                Ok(_) if is_track => outcome.added_tracks += 1,
                Ok(_) => outcome.added_folders += 1,
                Err(LibraryError::FolderExists | LibraryError::AlreadyCovered(_)) => {
                    outcome.already += 1;
                }
                Err(error) => outcome.problems.push(error.to_string()),
            }
        }
        outcome
    }

    pub fn remove_folder(&self, id: i64) -> Result<(), LibraryError> {
        self.lock()?.remove_folder(id)
    }

    /// Bütün klasörleri arka planda yeniden tarar. Tarama sürüyorsa bittikten
    /// sonra bir kez daha tarar (aradaki değişiklikler kaçmasın).
    pub fn request_scan(&self) {
        self.pending.store(true, Ordering::Release);
        if self.worker_running.swap(true, Ordering::AcqRel) {
            return; // çalışan tarayıcı isteği görecek
        }
        let (library, scan, pending, running, problems) = (
            Arc::clone(&self.library),
            Arc::clone(&self.scan),
            Arc::clone(&self.pending),
            Arc::clone(&self.worker_running),
            Arc::clone(&self.problems),
        );
        let spawned = std::thread::Builder::new()
            .name("lyraska-kutuphane-tarama".to_owned())
            .spawn(move || {
                loop {
                    while pending.swap(false, Ordering::AcqRel) {
                        let found = run_scan(&library, &scan);
                        if let Ok(mut p) = problems.lock() {
                            *p = found;
                        }
                    }
                    running.store(false, Ordering::Release);
                    // Bayrağı bıraktıktan hemen sonra gelen isteği kaçırma.
                    if !pending.load(Ordering::Acquire) || running.swap(true, Ordering::AcqRel) {
                        break;
                    }
                }
            });
        if spawned.is_err() {
            self.worker_running.store(false, Ordering::Release);
        }
    }

    /// Tarama sürüyor mu (testler ve durum için).
    pub fn is_scanning(&self) -> bool {
        self.worker_running.load(Ordering::Acquire) || self.scan.is_scanning()
    }

    /// Program kapanırken süren taramayı durdurur.
    pub fn shutdown(&self) {
        self.scan.cancel();
    }
}

/// Bütün klasörleri tarar; bir klasördeki sorun diğerlerini durdurmaz.
fn run_scan(library: &Mutex<Library>, scan: &ScanState) -> Vec<String> {
    let folders = match library.lock() {
        Ok(lib) => match lib.folders() {
            Ok(folders) => folders,
            Err(e) => return vec![e.to_string()],
        },
        Err(_) => return vec!["Kütüphane kilitlenemedi".to_owned()],
    };
    folders
        .iter()
        .filter_map(|folder| scan::scan_folders(library, std::slice::from_ref(folder), scan).err())
        .map(|e| e.to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::temp_path;
    use std::time::{Duration, Instant};

    fn wait_idle(service: &LibraryService) {
        let deadline = Instant::now() + Duration::from_secs(10);
        while service.is_scanning() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!service.is_scanning(), "tarama bitmedi");
    }

    fn folder_with_songs() -> std::path::PathBuf {
        let dir = temp_path("servis-muzik");
        std::fs::create_dir_all(&dir).unwrap();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        for ext in ["mp3", "flac"] {
            let name = format!("uc-uc-ton.{ext}");
            std::fs::copy(data.join(&name), dir.join(name)).unwrap();
        }
        dir
    }

    #[test]
    fn klasor_eklenince_arka_planda_taranir() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        service.add_folder(&folder_with_songs()).unwrap();
        wait_idle(&service);
        let status = service.status().unwrap();
        assert_eq!(status.track_count, 2);
        assert_eq!(status.folders.len(), 1);
        assert!(status.problems.is_empty());
        assert_eq!(service.search("üç ton", None).unwrap().len(), 2);
    }

    #[test]
    fn eksik_klasor_digerlerini_durdurmaz_ve_sorun_olarak_bildirilir() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        service
            .lock()
            .unwrap()
            .add_folder("/olmayan/klasor")
            .unwrap();
        service.add_folder(&folder_with_songs()).unwrap();
        wait_idle(&service);
        let status = service.status().unwrap();
        assert_eq!(status.track_count, 2);
        assert_eq!(status.problems.len(), 1);
        assert!(status.problems[0].contains("/olmayan/klasor"));
    }

    #[test]
    fn olmayan_klasor_eklenemez() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let error = service.add_folder(Path::new("/olmayan/yer")).unwrap_err();
        assert!(matches!(error, LibraryError::FolderMissing(_)));
        assert!(service.status().unwrap().folders.is_empty());
    }

    #[test]
    fn desteklenmeyen_dosya_eklenemez() {
        // Pencereye ses dosyası olmayan bir dosya bırakılınca anlaşılır bir hata verilir.
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let file = folder_with_songs().join("notlar.txt");
        std::fs::write(&file, "merhaba").unwrap();
        let error = service.add_folder(&file).unwrap_err();
        assert!(matches!(error, LibraryError::UnsupportedFile(_)));
        assert!(error.to_string().contains("notlar.txt"));
        assert!(service.status().unwrap().folders.is_empty());
    }

    #[test]
    fn birakilanlar_eklenir_sarkilar_calinmak_uzere_doner() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let dir = folder_with_songs();
        let elsewhere = temp_path("servis-birakilan");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let song = elsewhere.join("Gece Otoyolu.FLAC");
        std::fs::copy(dir.join("uc-uc-ton.flac"), &song).unwrap();
        // Desteklenmeyen tür: eskiden uzantı listede yok diye klasör sanılıyordu.
        let wma = elsewhere.join("Eski Kayıt.wma");
        std::fs::write(&wma, "değil").unwrap();
        // Zip içinden sürüklenen dosya gibi diskte olmayan yol.
        let missing = elsewhere.join("yok.mp3");

        let outcome = service.add_dropped(&[dir.clone(), wma, song.clone(), missing]);
        assert_eq!(outcome.tracks, vec![song.to_string_lossy().into_owned()]);
        assert_eq!((outcome.added_folders, outcome.added_tracks), (1, 1));
        assert_eq!(outcome.already, 0);
        assert_eq!(outcome.problems.len(), 2, "{:?}", outcome.problems);
        assert_eq!(
            outcome.problems[0],
            "Bu dosya türü desteklenmiyor: Eski Kayıt.wma"
        );
        assert!(outcome.problems[1].contains("yok.mp3"));
        wait_idle(&service);
        let status = service.status().unwrap();
        assert_eq!(status.folders.len(), 2);
        assert_eq!(status.track_count, 3);

        // Zaten kütüphanede olan şarkılar yine çalınır, iki kez eklenmez.
        let inside = dir.join("uc-uc-ton.mp3");
        let again = service.add_dropped(&[song.clone(), inside.clone(), dir]);
        assert_eq!(
            again.tracks,
            vec![
                song.to_string_lossy().into_owned(),
                inside.to_string_lossy().into_owned()
            ]
        );
        assert_eq!((again.added_folders, again.added_tracks), (0, 0));
        assert_eq!(again.already, 3);
        assert!(again.problems.is_empty(), "{:?}", again.problems);
        assert_eq!(service.status().unwrap().folders.len(), 2);
    }

    #[test]
    fn tek_sarki_eklenir_taranir_ve_cikarilir() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let dir = folder_with_songs();
        // Windows'ta sık görülen büyük harfli uzantı.
        let song = dir.join("Gülümse.MP3");
        std::fs::rename(dir.join("uc-uc-ton.mp3"), &song).unwrap();
        let source = service.add_folder(&song).unwrap();
        wait_idle(&service);
        let status = service.status().unwrap();
        assert_eq!(status.track_count, 1, "yalnızca seçilen şarkı");
        assert!(status.problems.is_empty(), "{:?}", status.problems);
        assert_eq!(
            service.search("", None).unwrap()[0].path,
            song.to_string_lossy()
        );

        // Yeniden tarama şarkıyı korur; kaynak çıkarılınca şarkı da çıkar.
        service.request_scan();
        wait_idle(&service);
        assert_eq!(service.status().unwrap().track_count, 1);
        service.remove_folder(source.id).unwrap();
        assert_eq!(service.status().unwrap().track_count, 0);
    }

    #[test]
    fn ayni_sarki_iki_kez_eklenmez() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let dir = folder_with_songs();
        service.add_folder(&dir).unwrap();
        wait_idle(&service);

        // Klasörü zaten kütüphanede olan şarkı ayrıca eklenmez.
        let error = service.add_folder(&dir.join("uc-uc-ton.flac")).unwrap_err();
        assert!(matches!(error, LibraryError::AlreadyCovered(_)), "{error}");
        assert!(error.to_string().contains(&*dir.to_string_lossy()));
        // Aynı klasör de iki kez eklenmez.
        assert!(matches!(
            service.add_folder(&dir).unwrap_err(),
            LibraryError::FolderExists
        ));
        assert_eq!(service.status().unwrap().folders.len(), 1);
    }

    #[test]
    fn klasor_icindeki_tek_sarkilarin_yerini_alir() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        let dir = folder_with_songs();
        service.add_folder(&dir.join("uc-uc-ton.mp3")).unwrap();
        wait_idle(&service);
        assert_eq!(service.status().unwrap().track_count, 1);

        // Sonra bütün klasör eklenince tek şarkı kaynağı kalkar, şarkılar klasörden gelir.
        service.add_folder(&dir).unwrap();
        wait_idle(&service);
        let status = service.status().unwrap();
        assert_eq!(status.folders.len(), 1);
        assert_eq!(status.folders[0].path, dir.to_string_lossy());
        assert_eq!(status.track_count, 2);
    }

    #[test]
    fn ust_uste_tarama_istekleri_guvenle_birlesir() {
        let service = LibraryService::new(Library::open_in_memory().unwrap());
        service.add_folder(&folder_with_songs()).unwrap();
        for _ in 0..20 {
            service.request_scan();
        }
        wait_idle(&service);
        assert_eq!(service.status().unwrap().track_count, 2);
    }
}
