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
use crate::audio::player::{PlaybackStatus, Player};
use crate::library::{LibraryService, LibraryStatus, TrackRow};
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
    Ok(player
        .lock()?
        .spectrum_now()
        .map(|(position_secs, bands)| VisualFrame {
            position_secs,
            bands: bands.to_vec(),
        }))
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
    fn oynatici_durumu_arayuz_bicimine_uyar() {
        let json = serde_json::to_value(Player::new().status()).unwrap();
        assert_eq!(json["state"], "idle");
        assert_eq!(json["positionSecs"], 0.0);
        assert!(json.get("underruns").is_some());
    }
}
