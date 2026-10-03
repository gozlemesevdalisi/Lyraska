//! Arayüzün (`invoke`) çağırabildiği Tauri komutları.
//!
//! Komutlar ince tutulur: asıl iş ilgili modülde yapılır, burada yalnızca
//! arayüze uygun veri biçimine çevrilir. Hatalar Türkçe metin olarak döner.
//!
//! Oynatıcı komutları `async`tır: Tauri düz komutları ana (pencere) iş parçacığında
//! çalıştırır; şarkı açmak birkaç yüz milisaniye sürebildiği için pencere donmasın diye
//! bu komutlar arka plandaki iş parçacıklarında çalışır.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

use serde::Serialize;
use tauri::State;

use crate::audio::decode::{TrackInfo, SUPPORTED_EXTENSIONS};
use crate::audio::eq::{EqSettings, EqState};
use crate::audio::peq::{HeadphoneProfile, HeadphoneSettings, HeadphoneState};
use crate::audio::player::{PlaybackStatus, Player};
use crate::library::{LibraryService, LibraryStatus, TrackRow};
use crate::settings::SettingsStore;
use crate::visual_bridge::VisualFrame;
use crate::{analysis, audio, visual_bridge};

/// Karşılama ekranında gösterilen program bilgisi.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
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
            "Oynatıcı beklenmedik bir hatayla durdu; programı yeniden başlatın.".to_owned()
        })
    }
}

/// Kayıtlı ayarları oynatıcıya uygular (program açılırken).
pub fn restore_settings(player: &PlayerState, store: &SettingsStore) {
    if let Ok(mut player) = player.lock() {
        let settings = store.get();
        player.set_equalizer(settings.equalizer);
        player.set_headphone(settings.headphone);
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
    player.lock()?.load(&path, true).map_err(|e| e.to_string())
}

/// Çalıyorsa duraklatır, değilse çalar.
#[tauri::command]
pub async fn toggle_playback(player: State<'_, PlayerState>) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.toggle().map_err(|e| e.to_string())?;
    Ok(player.status())
}

/// Durdurur ve şarkının başına döner.
#[tauri::command]
pub async fn stop_playback(player: State<'_, PlayerState>) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.stop().map_err(|e| e.to_string())?;
    Ok(player.status())
}

/// Şarkıda verilen saniyeye atlar.
#[tauri::command]
pub async fn seek_playback(
    seconds: f64,
    player: State<'_, PlayerState>,
) -> Result<PlaybackStatus, String> {
    let mut player = player.lock()?;
    player.seek(seconds).map_err(|e| e.to_string())?;
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
    library.get()?.status().map_err(|e| e.to_string())
}

/// Klasörü kütüphaneye ekler ve arka planda taramaya başlar.
#[tauri::command]
pub async fn library_add_folder(
    path: PathBuf,
    library: State<'_, LibraryState>,
) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.add_folder(&path).map_err(|e| e.to_string())?;
    service.status().map_err(|e| e.to_string())
}

/// Klasörü (ve şarkılarını) kütüphaneden çıkarır; diskteki dosyalara dokunmaz.
#[tauri::command]
pub async fn library_remove_folder(
    id: i64,
    library: State<'_, LibraryState>,
) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.remove_folder(id).map_err(|e| e.to_string())?;
    service.status().map_err(|e| e.to_string())
}

/// Bütün klasörleri yeniden tarar (değişmeyen dosyalar atlanır).
#[tauri::command]
pub async fn library_rescan(library: State<'_, LibraryState>) -> Result<LibraryStatus, String> {
    let service = library.get()?;
    service.request_scan();
    service.status().map_err(|e| e.to_string())
}

/// Kütüphanede arar; boş arama bütün şarkıları döndürür.
#[tauri::command]
pub async fn library_search(
    query: String,
    limit: Option<usize>,
    library: State<'_, LibraryState>,
) -> Result<Vec<TrackRow>, String> {
    library
        .get()?
        .search(&query, limit)
        .map_err(|e| e.to_string())
}

/// Ekolayzer ayarları ve uygulanan eğri.
#[tauri::command]
pub async fn equalizer_get(player: State<'_, PlayerState>) -> Result<EqState, String> {
    Ok(EqState::new(player.lock()?.equalizer()))
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
        .map_err(|e| format!("Ekolayzer ayarı kaydedilemedi: {e}"))?;
    Ok(EqState::new(applied))
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
    let text =
        std::fs::read_to_string(path).map_err(|e| format!("Profil dosyası okunamadı: {e}"))?;
    let name = path
        .file_stem()
        .map(|s| HeadphoneProfile::name_from_file(&s.to_string_lossy()))
        .unwrap_or_default();
    let profile = HeadphoneProfile::parse(&text, &name).map_err(|e| e.to_string())?;
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
        .map_err(|e| format!("Kulaklık ayarı kaydedilemedi: {e}"))?;
    Ok(HeadphoneState::new(applied))
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
        let json = serde_json::to_value(EqState::new(EqSettings::default())).unwrap();
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
        assert_eq!(restored.headphone().profile.unwrap().name, "Deneme");
    }

    #[test]
    fn eski_ayar_dosyasi_kulaklik_ayari_olmadan_okunur() {
        let settings: crate::settings::Settings = serde_json::from_str(
            r#"{"equalizer":{"enabled":true,"gainsDb":[0,0,0,0,0,0,0,0,0,0]}}"#,
        )
        .unwrap();
        assert_eq!(settings.headphone, HeadphoneSettings::default());
    }

    #[test]
    fn oynatici_durumu_arayuz_bicimine_uyar() {
        let json = serde_json::to_value(Player::new().status()).unwrap();
        assert_eq!(json["state"], "idle");
        assert_eq!(json["positionSecs"], 0.0);
        assert!(json.get("underruns").is_some());
    }
}
