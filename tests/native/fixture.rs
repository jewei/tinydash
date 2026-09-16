#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let marker = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing marker path"))?;
    if let Some(label) = args.next() {
        fs::write(marker, label.as_encoded_bytes())
    } else {
        // The OS document association supplies the opened file as the only arg.
        // The launcher never reads the document contents to find this marker.
        let opened = marker.with_extension("opened");
        fs::write(opened, marker.to_string_lossy().as_bytes())
    }
}
