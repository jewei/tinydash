use std::{env, fs, path::PathBuf};

fn main() {
    println!("cargo:rerun-if-env-changed=TINYDASH_SOURCE_ROOT");
    let root = env::var_os("TINYDASH_SOURCE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("manifest")).join("../../..")
        })
        .canonicalize()
        .expect("source root");
    let files = [
        ("file_provider", "src-tauri/src/providers/files.rs"),
        ("clipboard_provider", "src-tauri/src/providers/clipboard.rs"),
        ("ranking", "src-tauri/src/ranking/mod.rs"),
        ("result", "src-tauri/src/launcher/result.rs"),
        ("query", "src-tauri/src/launcher/query.rs"),
        ("pins", "src-tauri/src/launcher/pins.rs"),
    ];
    let mut modules = String::new();
    for (name, relative) in files {
        let path = root.join(relative);
        println!("cargo:rerun-if-changed={}", path.display());
        modules.push_str(&format!("#[path = {path:?}] pub mod {name};\n"));
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("production.rs"),
        modules,
    )
    .expect("write modules");
}
