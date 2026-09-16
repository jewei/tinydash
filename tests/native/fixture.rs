#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::{env, fs, io, path::PathBuf};

fn main() -> io::Result<()> {
    let mut args = env::args_os().skip(1);
    let marker = args
        .next()
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing marker path"))?;
    let label = args
        .next()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "Missing app label"))?;
    fs::write(marker, label.as_encoded_bytes())
}
