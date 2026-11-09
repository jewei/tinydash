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
