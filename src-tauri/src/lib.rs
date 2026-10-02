//! Lyraska çekirdeği.
//!
//! Modül yapısı:
//! - [`audio`]: ses motoru (çözme, 64-bit iç işlem, WASAPI çıkışı)
//! - [`analysis`]: şarkı haritası analizi (beat, ölçü, bölümler, drop, enerji)
//! - [`visual_bridge`]: analiz ve çalma zamanını arayüzdeki görsellere taşıyan köprü
//! - [`commands`]: arayüzün çağırabildiği Tauri komutları

pub mod analysis;
pub mod audio;
pub mod commands;
pub mod visual_bridge;

/// Programı başlatır.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::PlayerState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::open_track,
            commands::toggle_playback,
            commands::stop_playback,
            commands::seek_playback,
            commands::playback_status,
            commands::visual_frame,
        ])
        .run(tauri::generate_context!())
        .expect("Lyraska başlatılamadı");
}
