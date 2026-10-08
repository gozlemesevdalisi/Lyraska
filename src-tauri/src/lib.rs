//! Lyraska çekirdeği.
//!
//! Modül yapısı:
//! - [`audio`]: ses motoru (çözme, 64-bit iç işlem, WASAPI çıkışı)
//! - [`analysis`]: şarkı haritası analizi (beat, ölçü, bölümler, drop, enerji)
//! - [`director`]: Görsel Yönetmen — şarkıyı önceden bilen koreografi (atmosfer, ritim, doku)
//! - [`visual_bridge`]: analiz ve çalma zamanını arayüzdeki görsellere taşıyan köprü
//! - [`library`]: müzik kütüphanesi (SQLite, klasör tarama, arama)
//! - [`settings`]: kalıcı kullanıcı ayarları (ekolayzer vb.)
//! - [`commands`]: arayüzün çağırabildiği Tauri komutları
//! - [`diagnostics`]: yerel hata ve çökme günlüğü

pub mod analysis;
pub mod audio;
pub mod commands;
pub mod diagnostics;
pub mod director;
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
            // Hata ve çökme günlüğü en önce açılır: kurulumdaki hatalar da yazılsın.
            if let Ok(dir) = app.path().app_data_dir() {
                diagnostics::init(&dir.join("logs"));
            }
            // Kütüphane uygulama veri klasöründe durur; açılır açılmaz arka planda güncellenir.
            let service = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    library::LibraryService::open(&dir.join("library.sqlite3"))
                        .map_err(|e| e.to_string())
                });
            match &service {
                Ok(service) => service.request_scan(),
                Err(error) => diagnostics::error(&format!("Kütüphane açılamadı: {error}")),
            }
            app.manage(commands::LibraryState::new(service));

            // Şarkı haritası önbelleği kütüphaneyle aynı veritabanında; kütüphane arka
            // planda, düşük öncelikle analiz edilir (önce çalan, sonra sıradaki şarkı).
            let background = app
                .path()
                .app_data_dir()
                .map_err(|e| e.to_string())
                .and_then(|dir| {
                    analysis::cache::AnalysisCache::open(&dir.join("library.sqlite3"))
                        .map_err(|e| e.to_string())
                })
                .map_err(|e| diagnostics::error(&format!("Analiz önbelleği açılamadı: {e}")))
                .ok()
                .and_then(|cache| {
                    commands::start_analysis_cache(
                        &app.state::<commands::PlayerState>(),
                        std::sync::Arc::new(cache),
                    )
                });
            app.manage(commands::BackgroundState(background));

            // Ayarlar (ekolayzer) kaldığı gibi geri yüklenir.
            let store = match app.path().app_data_dir() {
                Ok(dir) => settings::SettingsStore::open(&dir.join("settings.json")),
                Err(_) => settings::SettingsStore::in_memory(),
            };
            commands::restore_settings(&app.state::<commands::PlayerState>(), &store);
            app.manage(store);

            // İnsan işaretleri (beat/drop): uygulama veri klasöründe, ses içermez.
            app.manage(match app.path().app_data_dir() {
                Ok(dir) => analysis::annotation::AnnotationStore::new(dir.join("isaretler")),
                Err(_) => analysis::annotation::AnnotationStore::unavailable(),
            });
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
            commands::log_frontend_error,
            commands::open_log,
            commands::annotation_get,
            commands::annotation_save,
            commands::annotation_evaluate,
            commands::annotation_open_folder,
            commands::song_map,
            commands::headphone_get,
            commands::headphone_import,
            commands::headphone_set_enabled,
            commands::headphone_clear,
            commands::visual_safe_get,
            commands::visual_safe_set,
        ])
        .build(tauri::generate_context!())
        .expect("Lyraska başlatılamadı")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                app.state::<commands::LibraryState>().shutdown();
                app.state::<commands::BackgroundState>().shutdown();
            }
        });
}
