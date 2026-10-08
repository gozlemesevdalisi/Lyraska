//! Lyraska çekirdeği.
//!
//! Modül yapısı:
//! - [`audio`]: ses motoru (çözme, 64-bit iç işlem, WASAPI çıkışı)
//! - [`analysis`]: şarkı haritası analizi (beat, ölçü, bölümler, drop, enerji)
//! - [`visual_bridge`]: analiz ve çalma zamanını arayüzdeki görsellere taşıyan köprü
//! - [`library`]: müzik kütüphanesi (SQLite, klasör tarama, arama)
//! - [`settings`]: kalıcı kullanıcı ayarları (ekolayzer vb.)
//! - [`commands`]: arayüzün çağırabildiği Tauri komutları

pub mod analysis;
pub mod audio;
pub mod commands;
pub mod library;
pub mod settings;
pub mod visual_bridge;

use tauri::Manager;

/// Programı başlatır.
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(commands::PlayerState::default())
        .setup(|app| {
            // Kütüphane uygulama veri klasöründe durur; açılır açılmaz arka planda güncellenir.
            let service = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    library::LibraryService::open(&dir.join("library.sqlite3"))
                        .map_err(|e| e.to_string())
                });
            if let Ok(service) = &service {
                service.request_scan();
            }
            app.manage(commands::LibraryState::new(service));

            // Ayarlar (ekolayzer) kaldığı gibi geri yüklenir.
            let store = match app.path().app_data_dir() {
                Ok(dir) => settings::SettingsStore::open(&dir.join("settings.json")),
                Err(_) => settings::SettingsStore::in_memory(),
            };
            commands::restore_settings(&app.state::<commands::PlayerState>(), &store);
            app.manage(store);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::open_track,
            commands::toggle_playback,
            commands::stop_playback,
            commands::seek_playback,
            commands::playback_status,
            commands::visual_frame,
            commands::library_status,
            commands::library_add_folder,
            commands::library_remove_folder,
            commands::library_rescan,
            commands::library_search,
            commands::equalizer_get,
            commands::equalizer_set,
            commands::set_next_track,
            commands::headphone_get,
            commands::headphone_import,
            commands::headphone_set_enabled,
            commands::headphone_clear,
        ])
        .build(tauri::generate_context!())
        .expect("Lyraska başlatılamadı")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<commands::LibraryState>().shutdown();
            }
        });
}
