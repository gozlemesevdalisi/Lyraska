//! Arayüzün (`invoke`) çağırabildiği Tauri komutları.
//!
//! Komutlar ince tutulur: asıl iş ilgili modülde yapılır, burada yalnızca
//! arayüze uygun veri biçimine çevrilir.

use serde::Serialize;

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
}

impl AppInfo {
    /// Derleme anındaki bilgilerden program bilgisini oluşturur.
    pub fn current() -> Self {
        Self {
            name: "Müzik Çalar".to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            phase: "Faz 0".to_owned(),
            audio_engine: audio::status().label().to_owned(),
            analysis: analysis::status().label().to_owned(),
            visual_bridge: visual_bridge::status().label().to_owned(),
        }
    }
}

/// Program adı, sürümü ve modüllerin durumu.
#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo::current()
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
        assert!(json.get("visualBridge").is_some());
        assert!(json.get("audio_engine").is_none());
    }
}
