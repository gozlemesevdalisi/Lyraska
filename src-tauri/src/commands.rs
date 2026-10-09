//! Arayüzün (`invoke`) çağırabildiği Tauri komutları.
//!
//! Komutlar ince tutulur: asıl iş ilgili modülde yapılır, burada yalnızca
//! arayüze uygun veri biçimine çevrilir. Hatalar Türkçe metin olarak döner.
//!
//! Oynatıcı komutları `async`tır: Tauri düz komutları ana (pencere) iş parçacığında
//! çalıştırır; şarkı açmak birkaç yüz milisaniye sürebildiği için pencere donmasın diye
//! bu komutlar arka plandaki iş parçacıklarında çalışır.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;
use tauri::State;

use crate::analysis::annotation::{AnnotatedTrack, Annotation, AnnotationStore};
use crate::analysis::background::BackgroundAnalysis;
use crate::analysis::cache::AnalysisCache;
use crate::analysis::evaluate;
use crate::audio::decode::{Decoder, TrackInfo, SUPPORTED_EXTENSIONS};
use crate::audio::eq::{EqSettings, EqState};
use crate::audio::peq::{HeadphoneProfile, HeadphoneSettings, HeadphoneState};
use crate::audio::player::{PlaybackStatus, Player};
use crate::audio::PlaybackOptions;
use crate::library::{DropOutcome, LibraryService, LibraryStatus, TrackRow};
use crate::settings::SettingsStore;
use crate::visual_bridge::VisualFrame;
use crate::{analysis, audio, diagnostics, visual_bridge};

/// Kullanıcıya dönen hatayı günlüğe de yazar.
fn reported(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    diagnostics::error(&message);
    message
}

/// Karşılama ekranında gösterilen program bilgisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub phase: String,
    pub audio_engine: String,
    pub analysis: String,
    pub visual_bridge: String,
    /// "Dosya aç" penceresinde gösterilecek uzantılar.
    pub supported_extensions: Vec<String>,
}

impl AppInfo {
    /// Derleme anındaki bilgilerden program bilgisini oluşturur.
    pub fn current() -> Self {
        Self {
            name: "Lyraska".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            phase: "Faz 1".to_owned(),
            audio_engine: audio::status().label().to_owned(),
            analysis: analysis::status().label().to_owned(),
            visual_bridge: visual_bridge::status().label().to_owned(),
            supported_extensions: SUPPORTED_EXTENSIONS
                .iter()
                .map(|e| (*e).to_owned())
                .collect(),
        }
    }
}

/// Tauri'nin yönettiği oynatıcı.
#[derive(Default)]
pub struct PlayerState(Mutex<Player>);

impl PlayerState {
    fn lock(&self) -> Result<MutexGuard<'_, Player>, String> {
        self.0.lock().map_err(|_| {
            reported("Oynatıcı beklenmedik bir hatayla durdu; programı yeniden başlatın.")
        })
    }
}

/// Kayıtlı ayarları oynatıcıya uygular (program açılırken).
pub fn restore_settings(player: &PlayerState, store: &SettingsStore) {
    if let Ok(mut player) = player.lock() {
        let settings = store.get();
        player.set_equalizer(settings.equalizer);
        player.set_headphone(settings.headphone);
        player.set_visual_safe(settings.visual_safe);
        player.set_audio_delay_ms(settings.audio_delay_ms);
        player.set_playback_options(settings.playback);
    }
}

/// Analiz önbelleğini oynatıcıya bağlar ve kütüphanenin arka plan analizini başlatır
/// (program açılırken). Oynatıcının kendi analizleri sürerken arka plan bekler.
pub fn start_analysis_cache(
    player: &PlayerState,
    cache: Arc<AnalysisCache>,
) -> Option<BackgroundAnalysis> {
    let mut player = player.lock().ok()?;
    player.set_analysis_cache(Arc::clone(&cache));
    Some(BackgroundAnalysis::start(
        cache,
        player.foreground_analyses(),
    ))
}

/// Tauri'nin yönettiği arka plan analizi (önbellek açılamadıysa yok).
pub struct BackgroundState(pub Option<BackgroundAnalysis>);

impl BackgroundState {
    /// Program kapanırken süren arka plan analizini durdurur.
    pub fn shutdown(&self) {
        if let Some(background) = &self.0 {
            background.stop();
        }
    }
}

/// Tauri'nin yönettiği kütüphane. Veritabanı açılamadıysa nedenini taşır.
pub struct LibraryState {
    service: Result<LibraryService, String>,
}

impl LibraryState {
    pub fn new(service: Result<LibraryService, String>) -> Self {
        Self { service }
    }

    fn get(&self) -> Result<&LibraryService, String> {
        self.service.as_ref().map_err(Clone::clone)
    }

    /// Program kapanırken süren taramayı durdurur.
    pub fn shutdown(&self) {
        if let Ok(service) = &self.service {
            service.shutdown();
        }
    }
}

/// Program adı, sürümü ve modüllerin durumu.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::current()
}

/// Şarkıyı açar ve çalmaya başlar.
#[tauri::command]
pub async fn open_track(
    path: PathBuf,
    player: State<'_, PlayerState>,
) -> Result<TrackInfo, String> {
    player.lock()?.load(&path, true).map_err(reported)
}

/// Boşluksuz geçiş için sıradaki şarkıyı bildirir (`null`: sıra yok).
#[tauri::command]
pub async fn set_next_track(
    path: Option<PathBuf>,
    player: State<'_, PlayerState>,
) -> Result<(), String> {
    player.lock()?.set_next(path);
    Ok(())
}

/// Çalıyorsa duraklatır, değilse çalar.
#[tauri::command]
pub async fn toggle_playback(player: State<'_, PlayerState>) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.toggle().map_err(reported)?;
    Ok(player.status())
}

/// Durdurur ve şarkının başına döner.
#[tauri::command]
pub async fn stop_playback(player: State<'_, PlayerState>) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.stop().map_err(reported)?;
    Ok(player.status())
}

/// Şarkıda verilen saniyeye atlar.
#[tauri::command]
pub async fn seek_playback(
    seconds: f64,
    player: State<'_, PlayerState>,
) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.seek(seconds).map_err(reported)?;
    Ok(player.status())
}

/// Şu an duyulan anın görsel verisi. Arayüz her ekran karesinde sorar;
/// analiz o ana yetişmediyse `null` döner.
#[tauri::command]
pub async fn visual_frame(player: State<'_, PlayerState>) -> Result<Option<VisualFrame>, String> {
    Ok(player.lock()?.visual_now().map(VisualFrame::from))
}

/// Kütüphane durumu: klasörler, şarkı sayısı, tarama ilerlemesi.
#[tauri::command]
pub async fn library_status(library: State<'_, LibraryState>) -> Result<LibraryStatus, String> {
    library.get()?.status().map_err(reported)
}

/// Klasörü kütüphaneye ekler ve arka planda taramaya başlar.
#[tauri::command]
pub async fn library_add_folder(
    path: PathBuf,
    library: State<'_, LibraryState>,
) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.add_folder(&path).map_err(reported)?;
    service.status().map_err(reported)
}

/// Pencereye bırakılan şarkıları ve klasörleri kütüphaneye ekler; çalınacak şarkıları döndürür.
#[tauri::command]
pub async fn library_add_dropped(
    paths: Vec<PathBuf>,
    library: State<'_, LibraryState>,
) -> Result<DropOutcome, String> {
    let outcome = library.get()?.add_dropped(&paths);
    diagnostics::info(&format!(
        "Bırakılanlar işlendi: {} öğe; {} şarkı çalınacak, {} şarkı ve {} klasör eklendi, \
         {} zaten kütüphanede, {} sorun",
        paths.len(),
        outcome.tracks.len(),
        outcome.added_tracks,
        outcome.added_folders,
        outcome.already,
        outcome.problems.len(),
    ));
    for problem in &outcome.problems {
        diagnostics::info(&format!("Bırakılan eklenemedi: {problem}"));
    }
    Ok(outcome)
}

/// Klasörü (ve şarkılarını) kütüphaneden çıkarır; diskteki dosyalara dokunmaz.
#[tauri::command]
pub async fn library_remove_folder(
    id: i64,
    library: State<'_, LibraryState>,
) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.remove_folder(id).map_err(reported)?;
    service.status().map_err(reported)
}

/// Bütün klasörleri yeniden tarar (değişmeyen dosyalar atlanır).
#[tauri::command]
pub async fn library_rescan(library: State<'_, LibraryState>) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.request_scan();
    service.status().map_err(reported)
}

/// Kütüphanede arar; boş arama bütün şarkıları döndürür.
#[tauri::command]
pub async fn library_search(
    query: String,
    limit: Option<usize>,
    library: State<'_, LibraryState>,
) -> Result<Vec<TrackRow>, String> {
    library.get()?.search(&query, limit).map_err(reported)
}

/// Ekolayzer ayarları ve uygulanan eğri.
#[tauri::command]
pub async fn equalizer_get(player: State<'_, PlayerState>) -> Result<EqState, String> {
    let player = player.lock()?;
    Ok(EqState::new(player.equalizer(), player.headroom_db()))
}

/// Ekolayzer ayarlarını değiştirir (çalan sese hemen yansır) ve kaydeder.
#[tauri::command]
pub async fn equalizer_set(
    settings: EqSettings,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<EqState, String> {
    let applied = player.lock()?.set_equalizer(settings);
    store
        .update(|s| s.equalizer = applied)
        .map_err(|e| reported(format!("Ekolayzer ayarı kaydedilemedi: {e}")))?;
    Ok(EqState::new(applied, player.lock()?.headroom_db()))
}

/// Ses aygıtının ek gecikmesi (ms; görseller bu kadar geriden gösterilir).
#[tauri::command]
pub async fn audio_delay_get(player: State<'_, PlayerState>) -> Result<i32, String> {
    Ok(player.lock()?.audio_delay_ms())
}

/// Ses gecikmesini ayarlar (görsellere hemen yansır), kaydeder ve uygulanan değeri döndürür.
#[tauri::command]
pub async fn audio_delay_set(
    ms: i32,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<i32, String> {
    let applied = player.lock()?.set_audio_delay_ms(ms);
    store
        .update(|s| s.audio_delay_ms = applied)
        .map_err(|e| reported(format!("Senkron ayarı kaydedilemedi: {e}")))?;
    Ok(applied)
}

/// Senkron ölçümü için tıklama kaydı: dosya yolu ve tıklamaların zamanları.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct CalibrationTrack {
    pub path: PathBuf,
    pub clicks: Vec<f64>,
}

/// Tıklama kaydını uygulama veri klasörüne yazar (her seferinde yeniden; küçük bir dosya).
#[tauri::command]
pub async fn calibration_track(app: tauri::AppHandle) -> Result<CalibrationTrack, String> {
    use tauri::Manager;
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| reported(format!("Uygulama klasörü bulunamadı: {e}")))?;
    let path = dir.join("kalibrasyon").join("tiklama.wav");
    visual_bridge::calibration::write(&path)
        .map_err(|e| reported(format!("Tıklama kaydı yazılamadı: {e}")))?;
    Ok(CalibrationTrack {
        path,
        clicks: visual_bridge::calibration::click_times(),
    })
}

/// Epilepsi güvenli modu açık mı.
#[tauri::command]
pub async fn visual_safe_get(player: State<'_, PlayerState>) -> Result<bool, String> {
    Ok(player.lock()?.visual_safe())
}

/// Epilepsi güvenli modunu açar ya da kapatır (görsellere hemen yansır) ve kaydeder.
#[tauri::command]
pub async fn visual_safe_set(
    enabled: bool,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<bool, String> {
    player.lock()?.set_visual_safe(enabled);
    store
        .update(|s| s.visual_safe = enabled)
        .map_err(|e| reported(format!("Güvenli mod ayarı kaydedilemedi: {e}")))?;
    Ok(enabled)
}

/// Çalma seçenekleri (ses yüksekliği eşitlemesi vb.).
#[tauri::command]
pub async fn playback_options_get(
    player: State<'_, PlayerState>,
) -> Result<PlaybackOptions, String> {
    Ok(player.lock()?.playback_options())
}

/// Çalma seçeneklerini uygular (çalan sese hemen yansır) ve kaydeder.
#[tauri::command]
pub async fn playback_options_set(
    options: PlaybackOptions,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<PlaybackOptions, String> {
    player.lock()?.set_playback_options(options);
    store
        .update(|s| s.playback = options)
        .map_err(|e| reported(format!("Çalma seçenekleri kaydedilemedi: {e}")))?;
    Ok(options)
}

/// Kulaklık düzeltmesi ve eğrisi.
#[tauri::command]
pub async fn headphone_get(player: State<'_, PlayerState>) -> Result<HeadphoneState, String> {
    Ok(HeadphoneState::new(player.lock()?.headphone()))
}

/// AutoEq / Equalizer APO parametrik profil dosyasını okur, açar ve kaydeder.
#[tauri::command]
pub async fn headphone_import(
    path: String,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<HeadphoneState, String> {
    let path = std::path::Path::new(&path);
    let text = std::fs::read_to_string(path)
        .map_err(|e| reported(format!("Profil dosyası okunamadı: {e}")))?;
    let name = path
        .file_stem()
        .map(|s| HeadphoneProfile::name_from_file(&s.to_string_lossy()))
        .unwrap_or_default();
    let profile = HeadphoneProfile::parse(&text, &name).map_err(reported)?;
    apply_headphone(
        HeadphoneSettings {
            enabled: true,
            profile: Some(profile),
        },
        &player,
        &store,
    )
}

/// Kulaklık düzeltmesini açar ya da kapatır (profil korunur).
#[tauri::command]
pub async fn headphone_set_enabled(
    enabled: bool,
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<HeadphoneState, String> {
    let current = player.lock()?.headphone();
    apply_headphone(HeadphoneSettings { enabled, ..current }, &player, &store)
}

/// Kulaklık profilini kaldırır.
#[tauri::command]
pub async fn headphone_clear(
    player: State<'_, PlayerState>,
    store: State<'_, SettingsStore>,
) -> Result<HeadphoneState, String> {
    apply_headphone(HeadphoneSettings::default(), &player, &store)
}

fn apply_headphone(
    settings: HeadphoneSettings,
    player: &PlayerState,
    store: &SettingsStore,
) -> Result<HeadphoneState, String> {
    let applied = player.lock()?.set_headphone(settings);
    store
        .update(|s| s.headphone = applied.clone())
        .map_err(|e| reported(format!("Kulaklık ayarı kaydedilemedi: {e}")))?;
    Ok(HeadphoneState::new(applied))
}

/// İşaretleme: şarkının kayıtlı işaretleri (yoksa `null`).
#[tauri::command]
pub async fn annotation_get(
    path: PathBuf,
    store: State<'_, AnnotationStore>,
) -> Result<Option<Annotation>, String> {
    Ok(store.load(&annotated_track(&path)?))
}

/// İşaretleri şarkının işaret dosyasına yazar; dosyanın yolunu döndürür.
/// Önceki "doğruluk" sonucu, işaretler değiştiği için silinir.
#[tauri::command]
pub async fn annotation_save(
    path: PathBuf,
    beats: Vec<f64>,
    drops: Vec<f64>,
    store: State<'_, AnnotationStore>,
) -> Result<String, String> {
    let annotation = Annotation::new(annotated_track(&path)?, beats, drops);
    store
        .save(&annotation)
        .map(|p| p.display().to_string())
        .map_err(|e| reported(format!("İşaretler kaydedilemedi: {e}")))
}

/// Kayıtlı işaretleri programın beat analiziyle karşılaştırır (birkaç saniye
/// sürer); sonucu işaret dosyasına ekler ve döndürür.
#[tauri::command]
pub async fn annotation_evaluate(
    path: PathBuf,
    store: State<'_, AnnotationStore>,
) -> Result<Annotation, String> {
    let track = annotated_track(&path)?;
    let mut annotation = store
        .load(&track)
        .ok_or_else(|| "Önce işaretleri kaydedin.".to_owned())?;
    if annotation.beats.len() < 8 {
        return Err("Doğruluğu ölçmek için en az 8 beat işaretleyin.".to_owned());
    }
    let marked = annotation.beats.clone();
    let marked_drops = annotation.drops.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        evaluate::evaluate_file(&path, &marked, &marked_drops)
    })
    .await
    .map_err(reported)?
    .map_err(reported)?;
    annotation.evaluation = Some(result);
    store
        .save(&annotation)
        .map_err(|e| reported(format!("Sonuç kaydedilemedi: {e}")))?;
    Ok(annotation)
}

/// Çalan şarkının yapısı (ölçü, bölümler, droplar, enerji); analiz bitmediyse `null`.
#[tauri::command]
pub async fn song_map(
    player: State<'_, PlayerState>,
) -> Result<Option<crate::analysis::structure::SongMap>, String> {
    Ok(player.lock()?.song_map().map(|m| (*m).clone()))
}

/// İşaret dosyalarının klasörünü dosya gezgininde açar.
#[tauri::command]
pub fn annotation_open_folder(store: State<'_, AnnotationStore>) -> Result<(), String> {
    let dir = store
        .dir()
        .ok_or_else(|| "İşaret klasörü bulunamadı.".to_owned())?;
    std::fs::create_dir_all(dir).map_err(|e| reported(format!("Klasör açılamadı: {e}")))?;
    open_folder(dir).map_err(|e| reported(format!("Klasör açılamadı: {e}")))
}

fn annotated_track(path: &std::path::Path) -> Result<AnnotatedTrack, String> {
    let decoder = Decoder::open(path).map_err(reported)?;
    Ok(AnnotatedTrack::from_info(decoder.info()))
}

/// Arayüzde oluşan hatayı (yakalanmamış istisna vb.) günlüğe yazar.
#[tauri::command]
pub fn log_frontend_error(message: String) {
    diagnostics::error(&format!(
        "arayüz: {}",
        message.chars().take(4000).collect::<String>()
    ));
}

/// Hata günlüğünü dosya gezgininde seçili olarak gösterir (hata kaydına eklemek için).
#[tauri::command]
pub fn open_log() -> Result<(), String> {
    let path = diagnostics::log_path()
        .ok_or_else(|| "Hata günlüğü açılamadı: uygulama veri klasörü bulunamadı.".to_owned())?;
    reveal(&path).map_err(|e| format!("Hata günlüğü açılamadı: {e}"))
}

#[cfg(windows)]
fn open_folder(dir: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("explorer")
        .arg(dir)
        .spawn()
        .map(|_| ())
}

#[cfg(not(windows))]
fn open_folder(dir: &std::path::Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map(|_| ())
}

#[cfg(windows)]
fn reveal(path: &std::path::Path) -> std::io::Result<()> {
    use std::os::windows::process::CommandExt;
    // Olduğu gibi verilir: `arg` boşluklu yolu bütünüyle tırnaklardı.
    std::process::Command::new("explorer")
        .raw_arg(select_argument(path))
        .spawn()
        .map(|_| ())
}

/// `explorer /select,"<dosya>"`: klasörü açar ve dosyayı seçili gösterir. Tırnak
/// yalnızca yolun çevresinde olmalı. Explorer bütünüyle tırnaklı `"/select,C:\…"`
/// biçimini anlamaz ve dosyayı seçmek yerine Belgeler'i açar. Kullanıcı adında
/// boşluk varsa ("C:\Users\Ali Veli\…") olan buydu. Windows yollarında `"` olmaz.
#[cfg(any(windows, test))]
fn select_argument(path: &std::path::Path) -> std::ffi::OsString {
    let mut argument = std::ffi::OsString::from("/select,\"");
    argument.push(path);
    argument.push("\"");
    argument
}

#[cfg(not(windows))]
fn reveal(path: &std::path::Path) -> std::io::Result<()> {
    let dir = path.parent().unwrap_or(path);
    std::process::Command::new("xdg-open")
        .arg(dir)
        .spawn()
        .map(|_| ())
}

/// Konum, durum ve şarkı bilgisi. Arayüz bunu düzenli aralıklarla sorar.
#[tauri::command]
pub async fn playback_status(player: State<'_, PlayerState>) -> Result<PlaybackStatus, String> {
    Ok(player.lock()?.status())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gezginde_secme_yolu_bosluklu_kullanici_adinda_da_dogru() {
        let path = std::path::Path::new(r"C:\Users\Ali Veli\AppData\logs\lyraska.log");
        assert_eq!(
            select_argument(path),
            r#"/select,"C:\Users\Ali Veli\AppData\logs\lyraska.log""#
        );
    }

    #[test]
    fn surum_cargo_toml_ile_ayni() {
        assert_eq!(AppInfo::current().version, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn arayuze_camel_case_gonderilir() {
        let json = serde_json::to_value(AppInfo::current()).unwrap();
        assert!(json.get("audioEngine").is_some());
        assert!(json.get("supportedExtensions").is_some());
        assert!(json.get("audio_engine").is_none());
    }

    #[test]
    fn ekolayzer_arayuz_bicimine_uyar() {
        let json = serde_json::to_value(EqState::new(EqSettings::default(), 0.0)).unwrap();
        for key in [
            "enabled",
            "gainsDb",
            "bandsHz",
            "maxGainDb",
            "preampDb",
            "curveHz",
            "curveDb",
        ] {
            assert!(json.get(key).is_some(), "{key} eksik");
        }
        // Arayüzden gelen ayar biçimi.
        let settings: EqSettings = serde_json::from_value(
            serde_json::json!({"enabled": false, "gainsDb": [1,0,0,0,0,0,0,0,0,0]}),
        )
        .unwrap();
        assert!(!settings.enabled);
        assert_eq!(settings.gains_db[0], 1.0);
    }

    #[test]
    fn kayitli_ayarlar_oynaticiya_uygulanir() {
        let store = SettingsStore::in_memory();
        let profile = HeadphoneProfile::parse(
            "Preamp: -3 dB\nFilter 1: ON PK Fc 1000 Hz Gain 3 dB Q 1\n",
            "Deneme",
        )
        .unwrap();
        store
            .update(|s| {
                s.equalizer.gains_db[4] = 7.0;
                s.visual_safe = true;
                s.audio_delay_ms = 180;
                s.headphone = HeadphoneSettings {
                    enabled: true,
                    profile: Some(profile.clone()),
                };
            })
            .unwrap();
        let player = PlayerState::default();
        restore_settings(&player, &store);
        let restored = player.lock().unwrap();
        assert_eq!(restored.equalizer().gains_db[4], 7.0);
        assert!(restored.headphone().enabled);
        assert!(restored.visual_safe());
        assert_eq!(restored.audio_delay_ms(), 180);
        assert_eq!(restored.headphone().profile.unwrap().name, "Deneme");
    }

    #[test]
    fn eski_ayar_dosyasi_kulaklik_ayari_olmadan_okunur() {
        let settings: crate::settings::Settings = serde_json::from_str(
            r#"{"equalizer":{"enabled":true,"gainsDb":[0,0,0,0,0,0,0,0,0,0]}}"#,
        )
        .unwrap();
        assert_eq!(settings.headphone, HeadphoneSettings::default());
        assert!(!settings.visual_safe);
        assert_eq!(settings.audio_delay_ms, 0);
    }

    #[test]
    fn oynatici_durumu_arayuz_bicimine_uyar() {
        let json = serde_json::to_value(Player::new().status()).unwrap();
        assert_eq!(json["state"], "idle");
        assert_eq!(json["positionSecs"], 0.0);
        assert!(json.get("underruns").is_some());
    }
}
