// Diagnostic overlay. prepare.py copies this into an isolated release build.
// The product build does not compile this module or expose these commands.
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixListener,
    sync::{Mutex, mpsc},
    time::Duration,
};
use serde_json::{Value, json};
use tauri::Manager;

#[derive(Default)]
struct Reply(Mutex<Option<mpsc::Sender<Value>>>);

static CLIPBOARD: std::sync::OnceLock<Mutex<(u64, String)>> = std::sync::OnceLock::new();

pub fn clipboard_fixture(previous: Option<u64>) -> Option<(u64, Option<String>)> {
    let value = CLIPBOARD.get_or_init(||Mutex::new((0, format!("memory fixture 099 {}", "x".repeat(40))))).lock().unwrap();
    (previous != Some(value.0)).then(||(value.0, Some(value.1.clone())))
}

#[tauri::command]
pub fn memory_reply(app: tauri::AppHandle, payload: Value) {
    if let Some(sender) = app.state::<Reply>().0.lock().unwrap().take() {
        let _ = sender.send(payload);
    }
}

fn command(app: &tauri::AppHandle, value: Value) -> Result<Value, String> {
    let label = value["window"].as_str().unwrap_or("main");
    match value["op"].as_str().unwrap_or("") {
        "clipboard" => {
            let text = value["text"].as_str().ok_or("Missing fixture text")?;
            if text.len() > 16384 { return Err("Fixture text too long".into()); }
            clipboard_fixture(None);
            let mut current = CLIPBOARD.get().unwrap().lock().unwrap();
            current.0 += 1;
            current.1 = text.into();
            Ok(Value::Null)
        }
        "status" => Ok(json!({
            "ready": app.state::<crate::launcher::LauncherState>().ready.load(std::sync::atomic::Ordering::Acquire),
            "scanning": app.state::<crate::launcher::LauncherState>().scanning.load(std::sync::atomic::Ordering::Acquire),
            "windows": app.webview_windows().iter().map(|(label, window)| json!({"label": label, "visible": window.is_visible().unwrap_or(false)})).collect::<Vec<_>>()
        })),
        "show" => crate::launcher::window::show(app).map(|_| Value::Null).map_err(|e| e.to_string()),
        "hide" => crate::launcher::window::hide(app).map(|_| Value::Null).map_err(|e| e.to_string()),
        "settings" => tauri::async_runtime::block_on(crate::launcher::preferences::open_settings(app.clone())).map(|_| Value::Null),
        "close" => app.get_webview_window(label).ok_or("Missing window")?.close().map(|_| Value::Null).map_err(|e| e.to_string()),
        "eval" => {
            let window = app.get_webview_window(label).ok_or("Missing window")?;
            let script = value["script"].as_str().ok_or("Missing script")?;
            let (sender, receiver) = mpsc::channel();
            *app.state::<Reply>().0.lock().unwrap() = Some(sender);
            window.eval(format!(r#"(async()=>{{{script}}})().then(value=>window.__TAURI_INTERNALS__.invoke('memory_reply',{{payload:{{value:value??null}}}}),error=>window.__TAURI_INTERNALS__.invoke('memory_reply',{{payload:{{error:String(error)}}}}))"#)).map_err(|e|e.to_string())?;
            receiver.recv_timeout(Duration::from_secs(20)).map_err(|e|e.to_string())
        }
        _ => Err("Unknown diagnostic command".into()),
    }
}

pub fn install(app: tauri::AppHandle) {
    if std::env::var("TINYDASH_MEMORY_DAILY").as_deref() == Ok("1") {
        let path = app.path().app_data_dir().unwrap().join("tinydash.sqlite3");
        let settings = app.path().app_config_dir().unwrap().join("settings.json");
        let mut database = crate::db::Database::open_with_settings(&path, &settings).unwrap();
        for index in 0..100 {
            let text = format!("memory fixture {index:03} {}", "x".repeat(if index % 2 == 0 { 16365 } else { 40 }));
            database.capture_clipboard(&text, index + 1, 100).unwrap();
        }
        database.set_pinned("clipboard:1", crate::launcher::query::SearchMode::Clipboard, true).unwrap();
        database.set_pinned("app:/System/Applications/Utilities/Terminal.app", crate::launcher::query::SearchMode::Apps, true).unwrap();
        database.save_rates(&crate::currency::Rates {
            date: "2026-09-18".into(), fetched_at: 1789776000,
            values: std::collections::BTreeMap::from([("EUR".into(), 1.0), ("USD".into(), 1.17), ("MYR".into(), 4.6)]),
        }).unwrap();
    }
    app.manage(Reply::default());
    let directory = std::path::PathBuf::from(std::env::var("TINYDASH_MEMORY_CONTROL").expect("isolated control directory"));
    let path = directory.join("control.sock");
    let _ = std::fs::remove_file(&path);
    let listener = UnixListener::bind(path).expect("diagnostic socket");
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut line = String::new();
            let result = BufReader::new(&stream).read_line(&mut line)
                .map_err(|e|e.to_string())
                .and_then(|_|serde_json::from_str(&line).map_err(|e|e.to_string()))
                .and_then(|value|command(&app, value));
            let response = match result { Ok(value) => json!({"ok": value}), Err(error) => json!({"error": error}) };
            let _ = writeln!(stream, "{response}");
        }
    });
}
