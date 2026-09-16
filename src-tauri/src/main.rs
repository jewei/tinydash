#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = tinydash_lib::run() {
        eprintln!("TinyDash could not start: {error:#}");
        std::process::exit(1);
    }
}
