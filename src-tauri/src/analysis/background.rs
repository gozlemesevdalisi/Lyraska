//! Kütüphanenin arka plan analizi: henüz analiz edilmemiş (ya da dosyası değişmiş)
//! şarkılar tek tek analiz edilip önbelleğe yazılır. Analizin bir parçası eskimişse
//! (sürümü artmış) yalnızca o parça yenilenir ([`spectrogram::refresh_paced`]); yalnızca
//! ritmi eskimiş şarkılar (çözülmeden, milisaniyelerde yenilenir) önce gelir.
//!
//! Öncelik sırası: **çalan şarkı**, **sıradaki şarkı**, **geri kalan kütüphane**.
//! Çalan ve sıradaki şarkıyı oynatıcı kendisi, tam hızla analiz eder (sıradaki,
//! çalanınki bitince başlar). Bu iş parçacığı yalnızca geri kalanlara bakar ve
//! düşük önceliklidir: oynatıcının analizi sürerken hiç çalışmaz, kendi işinde de
//! zamanının yarısında bekler; böylece işlemcinin en fazla bir çekirdeğinin
//! yarısını kullanır ve ses ya da görüntü hiç etkilenmez.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use super::cache::{AnalysisCache, FileStamp};
use super::spectrogram::{self, Spectrogram};
use crate::audio::decode::Decoder;
use crate::diagnostics;

/// Bekleyen şarkı yokken veritabanına bu aralıkla yeniden bakılır (yeni tarama vb.).
const IDLE_POLL: Duration = Duration::from_secs(15);
/// Oynatıcının analizi sürerken bu aralıkla yeniden bakılır.
const FOREGROUND_POLL: Duration = Duration::from_millis(100);
/// Çalışılan her sürenin bu katı kadar beklenir (1,0: zamanın yarısı iş, yarısı bekleme).
const REST_PER_WORK: f64 = 1.0;
/// Beklemeler bu kadar iş biriktikçe yapılır (çok kısa uykular işletim sistemini yorar).
const PACE_SLICE: Duration = Duration::from_millis(20);
/// Veritabanından bir seferde alınan şarkı sayısı.
const BATCH: usize = 16;

/// Oynatıcının o an süren analizlerinin sayacı. Analiz iş parçacığı başlarken
/// artırır, biterken (bu nesne düşünce) azaltır.
pub struct ForegroundGuard(Arc<AtomicUsize>);

impl ForegroundGuard {
    pub fn new(counter: &Arc<AtomicUsize>) -> Self {
        counter.fetch_add(1, Ordering::AcqRel);
        Self(Arc::clone(counter))
    }
}

impl Drop for ForegroundGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

struct Shared {
    stop: AtomicBool,
    /// Uyandırma işareti (yeni şarkılar eklendi).
    wake: Mutex<bool>,
    signal: Condvar,
    /// Bu oturumda arka planda analiz edilen şarkı sayısı.
    analyzed: AtomicUsize,
}

/// Arka plan analizi. Program kapanırken [`BackgroundAnalysis::stop`] çağrılır.
pub struct BackgroundAnalysis {
    shared: Arc<Shared>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl BackgroundAnalysis {
    /// Arka plan analizini başlatır. `foreground`: oynatıcının süren analiz sayısı.
    pub fn start(cache: Arc<AnalysisCache>, foreground: Arc<AtomicUsize>) -> Self {
        let shared = Arc::new(Shared {
            stop: AtomicBool::new(false),
            wake: Mutex::new(false),
            signal: Condvar::new(),
            analyzed: AtomicUsize::new(0),
        });
        let worker = Arc::clone(&shared);
        let thread = std::thread::Builder::new()
            .name("lyraska-arka-plan-analiz".to_owned())
            .spawn(move || run(&cache, &foreground, &worker))
            .map_err(|e| diagnostics::error(&format!("Arka plan analizi başlatılamadı: {e}")))
            .ok();
        Self {
            shared,
            thread: Mutex::new(thread),
        }
    }

    /// Yeni şarkılar eklendiğinde beklemeden bakması için uyandırır.
    pub fn wake(&self) {
        if let Ok(mut wake) = self.shared.wake.lock() {
            *wake = true;
            self.shared.signal.notify_all();
        }
    }

    /// Bu oturumda arka planda analiz edilen şarkı sayısı.
    pub fn analyzed(&self) -> usize {
        self.shared.analyzed.load(Ordering::Acquire)
    }

    /// Durdurur ve iş parçacığının bitmesini bekler (süren analiz yarıda kesilir).
    pub fn stop(&self) {
        self.shared.stop.store(true, Ordering::Release);
        self.wake();
        let thread = self.thread.lock().ok().and_then(|mut t| t.take());
        if let Some(thread) = thread {
            let _ = thread.join();
        }
    }
}

impl Drop for BackgroundAnalysis {
    fn drop(&mut self) {
        self.stop();
    }
}

fn run(cache: &AnalysisCache, foreground: &AtomicUsize, shared: &Shared) {
    // Bu tarama kaydıyla (yol + kütüphanedeki damga) bir daha denenmeyecek şarkılar:
    // açılamayanlar ve taramadan sonra diskte değişenler. Değişen şarkının analizi
    // diskteki damgayla kaydedilir (oynatıcı onu kullanır); ama kütüphane kaydı
    // yeniden taranana kadar "bekliyor" görünür ve yoksa tekrar tekrar analiz edilirdi.
    let mut skip: HashSet<(PathBuf, FileStamp)> = HashSet::new();
    while !shared.stop.load(Ordering::Acquire) {
        let batch: Vec<(PathBuf, FileStamp)> =
            match cache.pending_library_tracks(BATCH + skip.len()) {
                Ok(tracks) => tracks.into_iter().filter(|t| !skip.contains(t)).collect(),
                Err(e) => {
                    diagnostics::error(&e.to_string());
                    Vec::new()
                }
            };
        if batch.is_empty() {
            idle(shared);
            continue;
        }
        for (path, library_stamp) in batch.into_iter().take(BATCH) {
            if shared.stop.load(Ordering::Acquire) {
                return;
            }
            wait_for_foreground(foreground, shared);
            match analyze_one(&path, cache, foreground, shared) {
                Outcome::Stored(stamp) => {
                    shared.analyzed.fetch_add(1, Ordering::AcqRel);
                    if stamp != library_stamp {
                        skip.insert((path, library_stamp));
                    }
                }
                Outcome::Failed => {
                    skip.insert((path, library_stamp));
                }
                Outcome::Interrupted => {}
            }
        }
    }
}

/// Bekleyen iş yokken uyandırılana ya da `IDLE_POLL` dolana kadar bekler.
fn idle(shared: &Shared) {
    let Ok(wake) = shared.wake.lock() else {
        return;
    };
    let wake = shared.signal.wait_timeout_while(wake, IDLE_POLL, |w| {
        !*w && !shared.stop.load(Ordering::Acquire)
    });
    if let Ok((mut wake, _)) = wake {
        *wake = false;
    }
}

fn wait_for_foreground(foreground: &AtomicUsize, shared: &Shared) {
    while foreground.load(Ordering::Acquire) > 0 && !shared.stop.load(Ordering::Acquire) {
        std::thread::sleep(FOREGROUND_POLL);
    }
}

enum Outcome {
    /// Analiz bu damgayla (dosyanın analiz edilen hâli) kaydedildi.
    Stored(FileStamp),
    Failed,
    /// Program kapanıyor; şarkı sonra yeniden denenir.
    Interrupted,
}

fn analyze_one(
    path: &Path,
    cache: &AnalysisCache,
    foreground: &AtomicUsize,
    shared: &Shared,
) -> Outcome {
    let Some(stamp) = FileStamp::of(path) else {
        return Outcome::Failed;
    };
    let target = Spectrogram::new();
    let mut pacer = Pacer::new();
    let mut pace = || {
        if shared.stop.load(Ordering::Acquire) {
            target.cancel();
            return;
        }
        pacer.pace(foreground, shared);
    };
    let cached = match cache.load(path, stamp) {
        Ok(cached) => cached,
        Err(e) => {
            diagnostics::error(&e.to_string());
            None
        }
    };
    match cached {
        // Yalnızca eskimiş parçalar: ritim karelerden (çözmeden), ses yüksekliği ve bas
        // tepeleri FFT'siz tek çözmeyle.
        Some(cached) => {
            let mut open = || Decoder::open(path).ok();
            spectrogram::refresh_paced(cached, &mut open, &target, &mut pace);
        }
        None => {
            let Ok(decoder) = Decoder::open(path) else {
                return Outcome::Failed;
            };
            spectrogram::analyze_paced(decoder, &target, &mut pace);
        }
    }
    let Some(saved) = target.saved() else {
        return if shared.stop.load(Ordering::Acquire) {
            Outcome::Interrupted
        } else {
            Outcome::Failed
        };
    };
    // Analiz sürerken dosya değiştiyse sonuç eskidir; bir sonraki turda yeniden denenir.
    if FileStamp::of(path) != Some(stamp) {
        return Outcome::Interrupted;
    }
    match cache.store(path, stamp, &saved) {
        Ok(()) => Outcome::Stored(stamp),
        Err(e) => {
            diagnostics::error(&e.to_string());
            Outcome::Failed
        }
    }
}

/// İş ve bekleme sürelerini dengeler; oynatıcının analizi başlarsa onu bekler.
struct Pacer {
    since: Instant,
    worked: Duration,
}

impl Pacer {
    fn new() -> Self {
        Self {
            since: Instant::now(),
            worked: Duration::ZERO,
        }
    }

    fn pace(&mut self, foreground: &AtomicUsize, shared: &Shared) {
        self.worked += self.since.elapsed();
        if foreground.load(Ordering::Acquire) > 0 {
            wait_for_foreground(foreground, shared);
            self.worked = Duration::ZERO;
        } else if self.worked >= PACE_SLICE {
            std::thread::sleep(self.worked.mul_f64(REST_PER_WORK));
            self.worked = Duration::ZERO;
        }
        self.since = Instant::now();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::test_util::temp_path;
    use crate::library::LibraryService;

    fn wait_until(mut condition: impl FnMut() -> bool, seconds: u64) -> bool {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        condition()
    }

    /// Test verisindeki şarkıları içeren, taranmış bir kütüphane ve aynı dosyada önbellek.
    fn library_with_songs() -> (LibraryService, Arc<AnalysisCache>, Vec<PathBuf>) {
        let dir = temp_path("arka-plan-muzik");
        std::fs::create_dir_all(&dir).unwrap();
        let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
        let mut songs = Vec::new();
        for ext in ["mp3", "flac", "ogg"] {
            let name = format!("uc-uc-ton.{ext}");
            std::fs::copy(data.join(&name), dir.join(&name)).unwrap();
            songs.push(dir.join(name));
        }
        // Çözülemeyen dosya: sonsuza dek yeniden denenmemeli.
        std::fs::write(dir.join("bozuk.mp3"), b"ses degil").unwrap();
        let db = dir.join("library.sqlite3");
        let service = LibraryService::open(&db).unwrap();
        service.add_folder(&dir).unwrap();
        assert!(wait_until(|| !service.is_scanning(), 10));
        let cache = Arc::new(AnalysisCache::open(&db).unwrap());
        (service, cache, songs)
    }

    #[test]
    fn kutuphane_arka_planda_analiz_edilip_onbellege_yazilir() {
        let (service, cache, songs) = library_with_songs();
        assert_eq!(service.status().unwrap().analyzed, 0);
        let foreground = Arc::new(AtomicUsize::new(0));
        let background = BackgroundAnalysis::start(Arc::clone(&cache), foreground);
        assert!(wait_until(|| background.analyzed() == songs.len(), 30));
        for song in &songs {
            let saved = cache.load(song, FileStamp::of(song).unwrap()).unwrap();
            assert!(
                saved.is_some_and(|s| s.saved.frames() > 100 && !s.stale.any()),
                "{song:?} önbellekte yok"
            );
        }
        // Bozuk dosya bekleyenlerde kalır ama yeniden denenmez; analiz sayısı artmaz.
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(background.analyzed(), songs.len());
        assert_eq!(service.status().unwrap().analyzed, songs.len());
        background.stop();
    }

    #[test]
    fn ritim_hesabi_degisince_yalnizca_ritim_yenilenir() {
        let (service, cache, songs) = library_with_songs();
        let foreground = Arc::new(AtomicUsize::new(0));
        let background = BackgroundAnalysis::start(Arc::clone(&cache), Arc::clone(&foreground));
        assert!(wait_until(|| background.analyzed() == songs.len(), 30));
        background.stop();
        let before: Vec<_> = songs
            .iter()
            .map(|s| cache.load(s, FileStamp::of(s).unwrap()).unwrap().unwrap())
            .collect();

        // Yeni sürümde ritim hesabı değişti (ör. drop güveni eklendi): kayıtlar "ritmi eski".
        cache
            .lock_for_test()
            .execute(
                "UPDATE analyses SET rhythm_version = rhythm_version - 1",
                [],
            )
            .unwrap();
        assert_eq!(service.status().unwrap().analyzed, 0);
        let background = BackgroundAnalysis::start(Arc::clone(&cache), foreground);
        assert!(wait_until(|| background.analyzed() == songs.len(), 30));
        for (song, old) in songs.iter().zip(&before) {
            let now = cache
                .load(song, FileStamp::of(song).unwrap())
                .unwrap()
                .unwrap();
            assert!(!now.stale.any(), "{song:?} yenilendi");
            // Kareler, ses yüksekliği ve bas tepeleri dokunulmadan kaldı; ritim aynı hesaplandı.
            assert_eq!(now.saved, old.saved, "{song:?}");
        }
        assert_eq!(service.status().unwrap().analyzed, songs.len());
        background.stop();
    }

    #[test]
    fn taramadan_sonra_degisen_dosya_dongude_yeniden_analiz_edilmez() {
        let (_service, cache, songs) = library_with_songs();
        // Program açıkken şarkının etiketi başka bir programla düzenlendi: diskteki
        // damga kütüphanenin (son taramanın) damgasından farklı.
        let file = std::fs::File::options()
            .write(true)
            .open(&songs[0])
            .unwrap();
        file.set_modified(std::time::UNIX_EPOCH + Duration::from_secs(1_000_000))
            .unwrap();
        drop(file);

        let foreground = Arc::new(AtomicUsize::new(0));
        let background = BackgroundAnalysis::start(Arc::clone(&cache), foreground);
        assert!(wait_until(|| background.analyzed() >= songs.len(), 30));
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(
            background.analyzed(),
            songs.len(),
            "aynı şarkı döngüde yeniden analiz edildi"
        );
        // Oynatıcı açınca diskteki damgayla önbellekten bulunur (iş boşa gitmez).
        let stamp = FileStamp::of(&songs[0]).unwrap();
        assert!(cache.load(&songs[0], stamp).unwrap().is_some());
        background.stop();
    }

    #[test]
    fn calan_sarkinin_analizi_surerken_beklenir() {
        let (_service, cache, songs) = library_with_songs();
        let foreground = Arc::new(AtomicUsize::new(0));
        let busy = ForegroundGuard::new(&foreground);
        let background = BackgroundAnalysis::start(Arc::clone(&cache), Arc::clone(&foreground));
        std::thread::sleep(Duration::from_millis(400));
        assert_eq!(
            background.analyzed(),
            0,
            "oynatıcı analiz ederken arka plan bekler"
        );
        drop(busy);
        assert!(wait_until(|| background.analyzed() == songs.len(), 30));
        assert_eq!(foreground.load(Ordering::Acquire), 0);
    }

    #[test]
    fn durdurunca_hemen_biter() {
        let (_service, cache, _songs) = library_with_songs();
        let foreground = Arc::new(AtomicUsize::new(1)); // hep meşgul: iş parçacığı bekliyor
        let background = BackgroundAnalysis::start(cache, foreground);
        std::thread::sleep(Duration::from_millis(50));
        let started = Instant::now();
        background.stop();
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
