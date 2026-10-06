// Geen extra consolevenster op Windows in de release-versie.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    ft991a_app_lib::run()
}
