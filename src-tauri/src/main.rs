// Windows'ta sürüm (release) derlemesinde arka planda konsol penceresi açılmasın.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    muzik_calar_lib::run()
}
