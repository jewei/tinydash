//! Writes the golden corpus that pins the Rust app's answers for the Go port.
//! Run: `TZ=Asia/Kuala_Lumpur GOLDEN_OUT=../testdata/golden cargo test --manifest-path src-tauri/Cargo.toml golden -- --ignored`
//! Each line of `<out>/<feature>.jsonl` is one query and what the Rust code answered.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use serde_json::{Value, json};

use crate::features::{calculator, clip_card, currency::Rates, datetime, emoji, url_cleaner, web};
use crate::search::Context;
use crate::search::matcher::{Matcher, normalize};
use crate::search::usage::{Pins, Usage};

/// Monday 2026-11-09 10:30:00 in the zone that TZ names.
const NOW: (i32, u32, u32, u32, u32) = (2026, 11, 9, 10, 30);
const RATES: &[u8] = br#"{"amount":1.0,"base":"EUR","date":"2026-11-06","rates":{"USD":1.16,"MYR":4.9,"CNY":8.25,"GBP":0.88,"JPY":176.5}}"#;

fn queries(out: &Path, name: &str) -> Vec<String> {
    let text = fs::read_to_string(out.join("queries").join(format!("{name}.txt"))).unwrap();
    text.lines().map(str::to_owned).collect()
}

fn write(out: &Path, name: &str, rows: Vec<Value>) {
    let mut file = fs::File::create(out.join(format!("{name}.jsonl"))).unwrap();
    for row in rows {
        writeln!(file, "{row}").unwrap();
    }
}

#[test]
#[ignore = "writes the golden corpus; run with GOLDEN_OUT set"]
fn golden() {
    let out = PathBuf::from(std::env::var("GOLDEN_OUT").expect("set GOLDEN_OUT"));
    let tz = std::env::var("TZ").expect("set TZ so that dates are repeatable");
    let (y, mo, d, h, mi) = NOW;
    let now = Local.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap();
    let rates = Rates::parse(RATES, now.timestamp()).unwrap();

    write(
        &out,
        "meta",
        vec![json!({
            "tz": tz,
            "now": now.to_rfc3339(),
            "now_unix": now.timestamp(),
            "rates": serde_json::from_slice::<Value>(RATES).unwrap(),
            "commit": option_env!("GOLDEN_COMMIT"),
        })],
    );

    let rows = queries(&out, "calculator")
        .into_iter()
        .map(|q| {
            let expression = calculator::expression(&q).to_owned();
            json!({
                "query": q,
                "expression": expression,
                "answer": calculator::answer(&expression, Some(&rates)),
                "answer_without_rates": calculator::answer(&expression, None),
                "rates_off": calculator::rates_off_answer(&expression),
            })
        })
        .collect();
    write(&out, "calculator", rows);

    let rows = queries(&out, "datetime").into_iter().map(|q| {
        json!({ "query": q, "answers": datetime::answers(&q, now), "is_date_or_clock": datetime::is_date_or_clock(&q) })
    }).collect();
    write(&out, "datetime", rows);

    let rows = queries(&out, "url_cleaner")
        .into_iter()
        .map(|q| json!({ "query": q, "answer": url_cleaner::answer(&q) }))
        .collect();
    write(&out, "url_cleaner", rows);

    let rows = queries(&out, "clip_card")
        .into_iter()
        .map(|q| json!({ "query": q, "answer": clip_card::answer(&q, now) }))
        .collect();
    write(&out, "clip_card", rows);

    let rows = queries(&out, "web")
        .into_iter()
        .map(|q| json!({ "query": q, "answer": web::answer(&q), "encode": web::encode(&q) }))
        .collect();
    write(&out, "web", rows);

    // Every query against every name: the matcher's scores decide ranking.
    let names = queries(&out, "matcher");
    let mut rows = Vec::new();
    for q in &names {
        let mut matcher = Matcher::new(q);
        for name in &names {
            rows.push(json!({
                "query": q,
                "candidate": name,
                "name": matcher.name(name),
                "words": matcher.words(name),
                "alias": matcher.alias(name),
            }));
        }
        rows.push(json!({ "normalize": q, "normalized": normalize(q) }));
    }
    write(&out, "matcher", rows);

    let index = emoji::EmojiIndex::default();
    let (usage, pins) = (Usage::default(), Pins::default());
    let ctx = Context {
        usage: &usage,
        pins: &pins,
        now: 0,
        skin_tone: 0,
    };
    let rows = queries(&out, "emoji")
        .into_iter()
        .map(|q| {
            let hits = index.search(&mut Matcher::new(&q), &ctx, 8);
            let top: Vec<_> = hits
                .iter()
                .map(|hit| json!({ "id": hit.result.id, "score": hit.score }))
                .collect();
            json!({ "query": q, "top": top })
        })
        .collect();
    write(&out, "emoji", rows);
}

/// Settings files and what `settings::load` made of them, and a database
/// that `Store` filled by a fixed sequence of calls, with every table dumped.
#[test]
#[ignore = "writes golden fixtures; run with GOLDEN_OUT set"]
fn golden_fixtures() {
    use crate::features::clipboard::Content;
    use crate::features::library::{LibraryItem, LibraryKind};
    use crate::search::usage::Use;
    use crate::store::Store;

    let out = PathBuf::from(std::env::var("GOLDEN_OUT").expect("set GOLDEN_OUT")).join("fixtures");
    fs::create_dir_all(&out).unwrap();
    let scratch = std::env::temp_dir().join(format!("tinydash-golden-{}", std::process::id()));

    let cases: &[(&str, Option<&str>)] = &[
        ("absent", None),
        ("empty-object", Some("{}")),
        ("empty-array", Some("[]")),
        ("not-an-object", Some("[1, 2]")),
        ("damaged", Some("{\"theme\": ")),
        (
            "custom",
            Some(
                r#"{"shortcut":"Alt+Space","theme":"dark","hideOnBlur":false,"launchAtLogin":true,"showTrayIcon":true,"clipboardHistoryEnabled":true,"clipboardHistoryLimit":250,"clipboardCaptureImages":false,"clipboardCaptureFiles":true,"fileSearchFolders":["~/Documents","~/Projects"],"fileSearchExcludedDirs":["node_modules","target"],"spotlightFiles":false,"emojiSkinTone":3,"emojiLanguages":["zh","es"],"currencyRatesEnabled":true,"checkForUpdates":false,"searchEngine":"duckDuckGo","launcherPosition":{"x":120.5,"y":80.0},"tabs":[{"category":"all","shown":true},{"category":"apps","shown":true,"inAll":true},{"category":"files","shown":false,"inAll":false},{"category":"clipboard","shown":true,"inAll":false}],"showClocks":true,"clockCities":["Tokyo","Europe/London"],"showDiskSpace":false}"#,
            ),
        ),
        (
            "dropped-features",
            Some(
                r#"{"expandSnippets":true,"shellCommands":true,"showUpNext":true,"showNotepad":true,"showFocusTimer":true,"focusMinutes":50,"showWeather":true,"weatherCity":"Ipoh","temperatureUnit":"fahrenheit","showClipboardCards":true,"theme":"light"}"#,
            ),
        ),
        (
            "unreadable-values",
            Some(
                r#"{"theme":"sage","clipboardHistoryLimit":"many","tabs":[{"category":"apps","shown":true},{"category":"weather","shown":true},{"category":"emoji","shown":"yes"}],"hideOnBlur":false}"#,
            ),
        ),
        (
            "clamped",
            Some(
                r#"{"shortcut":"A","clipboardHistoryLimit":50000,"emojiSkinTone":9,"fileSearchFolders":["~/Docs","~/Docs","relative/path",""],"clockCities":["Tokyo","Tokyo","Atlantis","kl","pst"],"emojiLanguages":["zh","zh"]}"#,
            ),
        ),
        (
            "unknown-keys",
            Some(r#"{"appearance":"sage","futureFlag":1,"theme":"dark"}"#),
        ),
    ];
    let mut rows = Vec::new();
    for (name, input) in cases {
        let dir = scratch.join(name);
        fs::create_dir_all(&dir).unwrap();
        if let Some(text) = input {
            fs::write(dir.join("settings.json"), text).unwrap();
        }
        let loaded = crate::settings::load(&dir);
        let shown = dir.display().to_string();
        rows.push(json!({
            "name": name,
            "input": input,
            "settings": loaded.settings,
            "warning": loaded.warning.map(|w| w.replace(&shown, "<dir>")),
            "backup": dir.join("settings.invalid.json").exists(),
        }));
    }
    write(&out, "settings", rows);

    let dir = scratch.join("store");
    let (store, _) = Store::open(&dir).unwrap();
    let now = 1_794_191_400;
    store
        .record_use(
            "app:/Applications/Safari.app",
            Use {
                count: 3,
                last_used: now - 86_400,
            },
        )
        .unwrap();
    store
        .record_use(
            "emoji:🚀",
            Use {
                count: 1,
                last_used: now,
            },
        )
        .unwrap();
    store
        .set_pinned("app:/Applications/Safari.app", true)
        .unwrap();
    store.set_pinned("system:lock", true).unwrap();
    store
        .save_clip(&Content::Text("hello world".into()), now - 60, 100)
        .unwrap();
    store
        .save_clip(&Content::Text("second\nline".into()), now - 30, 100)
        .unwrap();
    store
        .save_clip(
            &Content::Files(vec!["/tmp/a.txt".into(), "/tmp/b b.png".into()]),
            now - 20,
            100,
        )
        .unwrap();
    store
        .save_clip(
            &Content::Image {
                png: vec![137, 80, 78, 71, 1, 2, 3],
                width: 2,
                height: 1,
            },
            now - 10,
            100,
        )
        .unwrap();
    store
        .save_clip(&Content::Text("hello world".into()), now - 10, 100)
        .unwrap();
    store.set_pinned("clip:1", true).unwrap();
    let snippet = store
        .save_library_item(&LibraryItem {
            id: None,
            kind: LibraryKind::Snippet,
            name: "Signature".into(),
            keyword: "sig".into(),
            text: "Best,\n{clipboard}".into(),
        })
        .unwrap();
    store
        .save_library_item(&LibraryItem {
            id: None,
            kind: LibraryKind::Quicklink,
            name: "Search MDN".into(),
            keyword: "mdn".into(),
            text: "https://developer.mozilla.org/search?q={query}".into(),
        })
        .unwrap();
    store
        .set_hidden("app:/Applications/Chess.app", true, now)
        .unwrap();
    store
        .set_alias("app:/Applications/Safari.app", Some("saf"))
        .unwrap();
    store
        .set_hotkey(
            &format!("snippet:{}", snippet.id.unwrap()),
            Some("Control+Alt+S"),
        )
        .unwrap();
    store.save_calculation("12 * 8", "96", now - 5).unwrap();
    store
        .save_calculation("5 ft to cm", "152.4 cm", now)
        .unwrap();
    store.save_note("scratch note").unwrap();
    store
        .save_rates(&Rates::parse(RATES, now).unwrap())
        .unwrap();
    drop(store);
    fs::copy(dir.join("tinydash.db"), out.join("tinydash.db")).unwrap();

    let db = rusqlite::Connection::open(dir.join("tinydash.db")).unwrap();
    let version: i64 = db
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap();
    let mut tables = serde_json::Map::new();
    let names: Vec<String> = db
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap()
        .query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
    for name in names {
        let mut statement = db
            .prepare(&format!("SELECT * FROM {name} ORDER BY 1"))
            .unwrap();
        let columns: Vec<String> = statement.column_names().map(str::to_owned).collect();
        let table_rows: Vec<Value> = statement
            .query_map([], |row| {
                let mut object = serde_json::Map::new();
                for (i, column) in columns.iter().enumerate() {
                    let value = match row.get_ref(i)? {
                        rusqlite::types::ValueRef::Null => Value::Null,
                        rusqlite::types::ValueRef::Integer(n) => json!(n),
                        rusqlite::types::ValueRef::Real(f) => json!(f),
                        rusqlite::types::ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
                        rusqlite::types::ValueRef::Blob(b) => json!({ "hex": b.iter().map(|x| format!("{x:02x}")).collect::<String>() }),
                    };
                    object.insert(column.clone(), value);
                }
                Ok(Value::Object(object))
            }).unwrap()
            .map(Result::unwrap)
            .collect();
        tables.insert(name, Value::Array(table_rows));
    }
    let hashes: Vec<Value> = [
        Content::Text("hello world".into()),
        Content::Text("東京 🚀".into()),
        Content::Files(vec!["/tmp/a.txt".into(), "/tmp/b b.png".into()]),
        Content::Image {
            png: vec![137, 80, 78, 71, 1, 2, 3],
            width: 2,
            height: 1,
        },
    ]
    .iter()
    .map(|c| json!({ "kind": format!("{:?}", c.kind()), "hash": c.hash(), "title": c.title() }))
    .collect();
    write(
        &out,
        "store",
        vec![json!({ "user_version": version, "tables": tables, "hashes": hashes })],
    );
    let _ = fs::remove_dir_all(&scratch);
}
