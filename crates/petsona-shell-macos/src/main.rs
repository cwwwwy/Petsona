#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() -> petsona_app::RunResult {
    petsona_shell_macos::run()
}
