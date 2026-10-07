//! Clipboard cards: when the clipboard holds a color, a Unix time, or JSON,
//! the widget pane shows it decoded, with ways to copy it in other forms.
//! A color or a Unix time typed in All is an instant answer too.

use chrono::{DateTime, Local, SecondsFormat, TimeZone, Utc};
use serde::Serialize;
use ts_rs::TS;

use crate::search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol};

/// Longer text never gets a card: reading and formatting it would slow the
/// pane down.
pub const MAX_CARD_BYTES: usize = 1_000_000;
/// The full JSON view shows this many lines; Copy Pretty JSON has them all.
const MAX_JSON_LINES: usize = 2_000;
/// A longer string or number is cut in the JSON view.
const MAX_TOKEN_CHARS: usize = 500;
/// The card names this many top-level keys.
const MAX_JSON_KEYS: usize = 12;
const MAX_MINIFIED_PREVIEW_CHARS: usize = 300;
/// Unix times from 2001-09-09 to 2103-02-05: shorter or longer numbers are
/// more likely counts or IDs than times.
const SECONDS: std::ops::Range<i64> = 1_000_000_000..4_200_000_000;

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipCard {
    /// Changes with the text, so a dismissed card returns only for a new copy.
    pub id: String,
    pub content: CardContent,
    /// Copies of the content in other forms; the first is the main one.
    pub actions: Vec<ResultAction>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[ts(export)]
pub enum CardContent {
    Color {
        /// `#2F6F5E`, for the swatch.
        hex: String,
        /// How it was copied when not as hex: `RGB` or `HSL`.
        copied_as: Option<String>,
        /// Values by label (HEX, RGB, HSL), each copied by the action of
        /// the same index.
        rows: Vec<CardRow>,
        on_white: Contrast,
        on_black: Contrast,
    },
    UnixTime {
        raw: String,
        milliseconds: bool,
        /// `3 days ago`, `In 2 hours`.
        relative: String,
        rows: Vec<CardRow>,
    },
    Json {
        array: bool,
        /// Keys of an object, or items of an array.
        count: usize,
        depth: usize,
        bytes: usize,
        /// The first top-level keys, each with `{n}` or `[n]` for an object
        /// or array value.
        keys: Vec<JsonKey>,
        /// The start of the minified text.
        minified: String,
        lines: Vec<JsonLine>,
        /// Lines the view leaves out.
        more_lines: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct CardRow {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct Contrast {
    /// `5.9:1`
    pub ratio: String,
    /// `AAA`, `AA`, `AA large`, or `Fail`, by the WCAG levels.
    pub grade: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct JsonKey {
    pub name: String,
    pub hint: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct JsonLine {
    pub indent: usize,
    pub tokens: Vec<JsonToken>,
}

#[derive(Clone, Debug, PartialEq, Serialize, TS)]
#[ts(export)]
pub struct JsonToken {
    pub kind: TokenKind,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TokenKind {
    Key,
    String,
    Number,
    Literal,
    Punctuation,
}

/// The card for this clipboard text, if it is one of the formats.
pub fn detect(text: &str, now: DateTime<Local>) -> Option<ClipCard> {
    if text.len() > MAX_CARD_BYTES {
        return None;
    }
    let trimmed = text.trim();
    let (content, actions) = color(trimmed)
        .or_else(|| unix_time(trimmed, now))
        .or_else(|| json(trimmed))?;
    Some(ClipCard {
        id: format!("{:016x}", fnv(text)),
        content,
        actions,
    })
}

/// The clipboard JSON, pretty or minified, for Copy; `None` when it is not
/// JSON a card would show.
pub fn format_json(text: &str, pretty: bool) -> Option<String> {
    let text = text.trim();
    if text.len() > MAX_CARD_BYTES || !is_json(text) {
        return None;
    }
    let tokens = tokenize(text);
    if pretty {
        let lines = layout(&tokens, usize::MAX).0;
        let mut out = String::new();
        for line in lines {
            out.push_str(&"  ".repeat(line.indent));
            line.tokens
                .iter()
                .for_each(|token| out.push_str(&token.text));
            out.push('\n');
        }
        out.pop();
        Some(out)
    } else {
        Some(tokens.iter().map(|token| token.text).collect())
    }
}

fn fnv(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

fn copy(label: &str, text: &str) -> ResultAction {
    ResultAction::new(
        label,
        Action::Copy {
            text: text.to_owned(),
        },
    )
}

/// A color or a Unix time typed in All, decoded like its card.
pub fn answer(query: &str, now: DateTime<Local>) -> Option<SearchResult> {
    let text = query.trim();
    let (id, kind, title, subtitle, icon, actions) =
        match color(text).or_else(|| unix_time(text, now))? {
            (
                CardContent::Color {
                    hex,
                    rows,
                    on_white,
                    ..
                },
                actions,
            ) => (
                format!("color:{hex}"),
                ResultKind::Color,
                hex.clone(),
                format!(
                    "{} · {} · {} on white",
                    rows[1].value, rows[2].value, on_white.ratio
                ),
                Icon::Color { hex },
                actions,
            ),
            (CardContent::UnixTime { relative, rows, .. }, actions) => (
                format!("unix:{text}"),
                ResultKind::DateTime,
                rows[0].value.clone(),
                format!("{} · {relative}", rows[1].value),
                Icon::Symbol {
                    name: Symbol::Clock,
                },
                actions,
            ),
            (CardContent::Json { .. }, _) => return None,
        };
    Some(SearchResult {
        id,
        kind,
        title,
        subtitle,
        icon,
        actions,
        pinned: false,
    })
}

type Card = (CardContent, Vec<ResultAction>);

// Colors

fn color(text: &str) -> Option<Card> {
    let lower = text.to_ascii_lowercase();
    let (rgb, copied_as) = if let Some(hex) = lower.strip_prefix('#') {
        (parse_hex(hex)?, None)
    } else if let Some(args) = function_args(&lower, "rgb") {
        let channel = |value: &str| value.parse::<u8>().ok();
        match args.as_slice() {
            [r, g, b] => ([channel(r)?, channel(g)?, channel(b)?], Some("RGB")),
            _ => return None,
        }
    } else {
        match function_args(&lower, "hsl")?.as_slice() {
            [h, s, l] => (
                hsl_to_rgb(
                    h.trim_end_matches("deg").parse().ok()?,
                    percent(s)?,
                    percent(l)?,
                )?,
                Some("HSL"),
            ),
            _ => return None,
        }
    };
    let [r, g, b] = rgb;
    let hex = format!("#{r:02X}{g:02X}{b:02X}");
    let rgb_text = format!("rgb({r}, {g}, {b})");
    let (h, s, l) = rgb_to_hsl(rgb);
    let hsl_text = format!("hsl({h}, {s}%, {l}%)");
    let luminance = relative_luminance(rgb);
    let rows = vec![
        CardRow {
            label: "HEX".into(),
            value: hex.clone(),
        },
        CardRow {
            label: "RGB".into(),
            value: rgb_text.clone(),
        },
        CardRow {
            label: "HSL".into(),
            value: hsl_text.clone(),
        },
    ];
    let actions = vec![
        copy("Copy HEX", &hex),
        copy("Copy RGB", &rgb_text),
        copy("Copy HSL", &hsl_text),
    ];
    Some((
        CardContent::Color {
            hex,
            copied_as: copied_as.map(String::from),
            rows,
            on_white: contrast(1.0, luminance),
            on_black: contrast(luminance, 0.0),
        },
        actions,
    ))
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let digit = |index: usize| u8::from_str_radix(&hex[index..=index], 16).ok();
    match hex.len() {
        3 => Some([digit(0)? * 17, digit(1)? * 17, digit(2)? * 17]),
        6 => Some([
            u8::from_str_radix(&hex[0..2], 16).ok()?,
            u8::from_str_radix(&hex[2..4], 16).ok()?,
            u8::from_str_radix(&hex[4..6], 16).ok()?,
        ]),
        _ => None,
    }
}

/// The arguments of `name(a, b, c)` or `name(a b c)`.
fn function_args<'a>(text: &'a str, name: &str) -> Option<Vec<&'a str>> {
    let inner = text.strip_prefix(name)?.trim_start();
    let inner = inner.strip_prefix('(')?.strip_suffix(')')?;
    Some(
        inner
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|part| !part.is_empty())
            .collect(),
    )
}

fn percent(value: &str) -> Option<f64> {
    let number: f64 = value.strip_suffix('%')?.parse().ok()?;
    (0.0..=100.0).contains(&number).then_some(number / 100.0)
}

fn hsl_to_rgb(hue: f64, saturation: f64, lightness: f64) -> Option<[u8; 3]> {
    if !hue.is_finite() {
        return None;
    }
    let hue = hue.rem_euclid(360.0) / 60.0;
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let x = chroma * (1.0 - (hue % 2.0 - 1.0).abs());
    let (r, g, b) = match hue as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let base = lightness - chroma / 2.0;
    // Each channel is within 0–1, so the cast only drops the fraction.
    let channel = |value: f64| ((value + base) * 255.0).round().clamp(0.0, 255.0) as u8;
    Some([channel(r), channel(g), channel(b)])
}

/// Hue in degrees and saturation and lightness in percent, rounded.
fn rgb_to_hsl([r, g, b]: [u8; 3]) -> (u16, u8, u8) {
    let [r, g, b] = [r, g, b].map(|c| f64::from(c) / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let lightness = (max + min) / 2.0;
    let delta = max - min;
    let (hue, saturation) = if delta == 0.0 {
        (0.0, 0.0)
    } else {
        let saturation = delta / (1.0 - (2.0 * lightness - 1.0).abs());
        let hue = if max == r {
            ((g - b) / delta).rem_euclid(6.0)
        } else if max == g {
            (b - r) / delta + 2.0
        } else {
            (r - g) / delta + 4.0
        };
        (hue * 60.0, saturation)
    };
    // All three are within their ranges, so the casts only drop fractions.
    (
        hue.round() as u16 % 360,
        (saturation * 100.0).round() as u8,
        (lightness * 100.0).round() as u8,
    )
}

fn relative_luminance(rgb: [u8; 3]) -> f64 {
    let [r, g, b] = rgb.map(|c| {
        let c = f64::from(c) / 255.0;
        if c <= 0.039_28 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    });
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn contrast(lighter: f64, darker: f64) -> Contrast {
    let ratio = (lighter + 0.05) / (darker + 0.05);
    let grade = if ratio >= 7.0 {
        "AAA"
    } else if ratio >= 4.5 {
        "AA"
    } else if ratio >= 3.0 {
        "AA large"
    } else {
        "Fail"
    };
    Contrast {
        ratio: format!("{ratio:.1}:1"),
        grade: grade.into(),
    }
}

// Unix times

fn unix_time(text: &str, now: DateTime<Local>) -> Option<Card> {
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let number: i64 = text.parse().ok()?;
    let milliseconds = text.len() == 13;
    let seconds = match text.len() {
        10 => number,
        13 => number / 1000,
        _ => return None,
    };
    if !SECONDS.contains(&seconds) {
        return None;
    }
    let utc = Utc
        .timestamp_millis_opt(if milliseconds { number } else { number * 1000 })
        .single()?;
    let local = utc.with_timezone(&Local);
    let format = if milliseconds {
        "%a %-d %b %Y, %H:%M:%S%.3f"
    } else {
        "%a %-d %b %Y, %H:%M:%S"
    };
    let local_text = local.format(format).to_string();
    let utc_text = format!("{} UTC", utc.format(format));
    let iso = utc.to_rfc3339_opts(
        if milliseconds {
            SecondsFormat::Millis
        } else {
            SecondsFormat::Secs
        },
        true,
    );
    let rows = vec![
        CardRow {
            label: "Local".into(),
            value: local_text.clone(),
        },
        CardRow {
            label: "UTC".into(),
            value: utc_text.clone(),
        },
        CardRow {
            label: "ISO".into(),
            value: iso.clone(),
        },
    ];
    let actions = vec![
        copy("Copy Local Time", &local_text),
        copy("Copy UTC Time", &utc_text),
        copy("Copy ISO 8601", &iso),
    ];
    Some((
        CardContent::UnixTime {
            raw: text.to_owned(),
            milliseconds,
            relative: relative(utc.timestamp() - now.timestamp()),
            rows,
        },
        actions,
    ))
}

/// `3 days ago` or `In 2 hours`, in the largest whole unit.
fn relative(seconds: i64) -> String {
    let distance = seconds.unsigned_abs();
    if distance < 45 {
        return "Just now".into();
    }
    let units = [
        (365 * 86_400, "year"),
        (30 * 86_400, "month"),
        (86_400, "day"),
        (3_600, "hour"),
        (60, "minute"),
    ];
    let (size, name) = units
        .into_iter()
        .find(|(size, _)| distance >= *size)
        .unwrap_or((60, "minute"));
    let count = (distance + size / 2) / size;
    let amount = format!("{count} {name}{}", if count == 1 { "" } else { "s" });
    if seconds < 0 {
        format!("{amount} ago")
    } else {
        format!("In {amount}")
    }
}

// JSON

fn is_json(text: &str) -> bool {
    text.starts_with(['{', '[']) && serde_json::from_str::<serde::de::IgnoredAny>(text).is_ok()
}

fn json(text: &str) -> Option<Card> {
    if !is_json(text) {
        return None;
    }
    let tokens = tokenize(text);
    let (lines, shape) = layout(&tokens, MAX_JSON_LINES);
    let total_lines = shape.lines;
    let minified: String = tokens
        .iter()
        .map(|token| token.text)
        .collect::<String>()
        .chars()
        .take(MAX_MINIFIED_PREVIEW_CHARS)
        .collect();
    let actions = vec![
        ResultAction::new("Copy Pretty JSON", Action::CopyJson { pretty: true }),
        ResultAction::new("Copy Minified JSON", Action::CopyJson { pretty: false }),
    ];
    Some((
        CardContent::Json {
            array: text.starts_with('['),
            count: shape.count,
            depth: shape.depth,
            bytes: text.len(),
            keys: shape.keys,
            minified,
            more_lines: total_lines - lines.len(),
            lines,
        },
        actions,
    ))
}

struct Token<'a> {
    kind: TokenKind,
    text: &'a str,
}

/// Split valid JSON into tokens, skipping whitespace. A string followed by
/// a colon is a key.
fn tokenize(text: &str) -> Vec<Token<'_>> {
    let bytes = text.as_bytes();
    let mut tokens = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        let kind = match bytes[at] {
            b' ' | b'\t' | b'\n' | b'\r' => {
                at += 1;
                continue;
            }
            b'"' => {
                at += 1;
                while at < bytes.len() && bytes[at] != b'"' {
                    at += if bytes[at] == b'\\' { 2 } else { 1 };
                }
                at = (at + 1).min(bytes.len());
                TokenKind::String
            }
            b'{' | b'}' | b'[' | b']' | b':' | b',' => {
                at += 1;
                TokenKind::Punctuation
            }
            b't' | b'f' | b'n' => {
                while at < bytes.len() && bytes[at].is_ascii_alphabetic() {
                    at += 1;
                }
                TokenKind::Literal
            }
            _ => {
                while at < bytes.len()
                    && matches!(bytes[at], b'0'..=b'9' | b'-' | b'+' | b'.' | b'e' | b'E')
                {
                    at += 1;
                }
                TokenKind::Number
            }
        };
        tokens.push(Token {
            kind,
            text: &text[start..at],
        });
    }
    for index in 1..tokens.len() {
        if tokens[index].text == ":" && tokens[index - 1].kind == TokenKind::String {
            tokens[index - 1].kind = TokenKind::Key;
        }
    }
    tokens
}

/// What the card says about the JSON besides its lines.
struct Shape {
    count: usize,
    depth: usize,
    keys: Vec<JsonKey>,
    lines: usize,
}

/// An object or array that is open while laying out.
struct Open {
    array: bool,
    /// Keys of an object, or items of an array, so far.
    children: usize,
}

/// Lay the tokens out in lines, indented two spaces a level, keeping at
/// most `max_lines`. An empty object or array stays on one line.
fn layout(tokens: &[Token<'_>], max_lines: usize) -> (Vec<JsonLine>, Shape) {
    let cut_long = max_lines != usize::MAX;
    let mut lines = Vec::new();
    let mut total = 0;
    let mut line = JsonLine {
        indent: 0,
        tokens: Vec::new(),
    };
    let mut open: Vec<Open> = Vec::new();
    let mut shape = Shape {
        count: 0,
        depth: 0,
        keys: Vec::new(),
        lines: 0,
    };
    // The top-level key whose value is being laid out, when the card names it.
    let mut top_key: Option<usize> = None;

    let mut end_line = |line: &mut JsonLine, indent: usize| {
        let done = std::mem::replace(
            line,
            JsonLine {
                indent,
                tokens: Vec::new(),
            },
        );
        if !done.tokens.is_empty() {
            total += 1;
            if lines.len() < max_lines {
                lines.push(done);
            }
        }
    };
    let push = |line: &mut JsonLine, kind: TokenKind, text: &str| {
        let text = if cut_long && text.chars().count() > MAX_TOKEN_CHARS {
            let cut: String = text.chars().take(MAX_TOKEN_CHARS).collect();
            format!("{cut}…")
        } else {
            text.to_owned()
        };
        line.tokens.push(JsonToken { kind, text });
    };
    // A value starts: an array counts it; an object counted its key.
    let count_value = |open: &mut Vec<Open>| {
        if let Some(parent) = open.last_mut().filter(|parent| parent.array) {
            parent.children += 1;
        }
    };

    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "{" | "[" => {
                count_value(&mut open);
                push(&mut line, TokenKind::Punctuation, token.text);
                open.push(Open {
                    array: token.text == "[",
                    children: 0,
                });
                shape.depth = shape.depth.max(open.len());
                let close = if token.text == "{" { "}" } else { "]" };
                // `{}` and `[]` stay on one line.
                if tokens.get(index + 1).map(|next| next.text) != Some(close) {
                    end_line(&mut line, open.len());
                }
            }
            "}" | "]" => {
                let closed = open.pop().map_or(0, |closed| closed.children);
                if closed > 0 {
                    end_line(&mut line, open.len());
                }
                push(&mut line, TokenKind::Punctuation, token.text);
                match open.len() {
                    0 => shape.count = closed,
                    1 => {
                        if let Some(key) = top_key.take().and_then(|at| shape.keys.get_mut(at)) {
                            key.hint = if token.text == "}" {
                                format!("{{{closed}}}")
                            } else {
                                format!("[{closed}]")
                            };
                        }
                    }
                    _ => {}
                }
            }
            "," => {
                push(&mut line, TokenKind::Punctuation, ",");
                end_line(&mut line, open.len());
            }
            ":" => push(&mut line, TokenKind::Punctuation, ": "),
            text => {
                if token.kind == TokenKind::Key {
                    if let Some(parent) = open.last_mut() {
                        parent.children += 1;
                    }
                    if open.len() == 1 {
                        top_key = (shape.keys.len() < MAX_JSON_KEYS).then(|| {
                            shape.keys.push(JsonKey {
                                // Without its quotes.
                                name: text[1..text.len() - 1].to_owned(),
                                hint: String::new(),
                            });
                            shape.keys.len() - 1
                        });
                    }
                } else {
                    count_value(&mut open);
                }
                push(&mut line, token.kind, text);
            }
        }
    }
    end_line(&mut line, 0);
    shape.lines = total;
    (lines, shape)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Local> {
        Utc.with_ymd_and_hms(2026, 10, 7, 12, 0, 0)
            .unwrap()
            .with_timezone(&Local)
    }

    fn content(text: &str) -> Option<CardContent> {
        detect(text, now()).map(|card| card.content)
    }

    #[test]
    fn reads_colors_in_three_forms() {
        for text in [
            "#2f6f5e",
            " #2F6F5E\n",
            "rgb(47, 111, 94)",
            "rgb(47 111 94)",
            "hsl(164, 41%, 31%)",
        ] {
            let Some(CardContent::Color { hex, rows, .. }) = content(text) else {
                panic!("{text} is a color");
            };
            assert!(
                ["#2F6F5E", "#2F6F5D", "#2F705E"].contains(&hex.as_str()),
                "{text}: {hex}"
            );
            assert_eq!(rows[0].value, hex);
        }
        let Some(CardContent::Color {
            copied_as,
            rows,
            on_white,
            on_black,
            ..
        }) = content("#fff")
        else {
            unreachable!()
        };
        assert_eq!(copied_as, None);
        assert_eq!(rows[1].value, "rgb(255, 255, 255)");
        assert_eq!(rows[2].value, "hsl(0, 0%, 100%)");
        assert_eq!(
            (on_white.ratio.as_str(), on_white.grade.as_str()),
            ("1.0:1", "Fail")
        );
        assert_eq!(
            (on_black.ratio.as_str(), on_black.grade.as_str()),
            ("21.0:1", "AAA")
        );
        let Some(CardContent::Color { copied_as, .. }) = content("hsl(0, 100%, 50%)") else {
            unreachable!()
        };
        assert_eq!(copied_as.as_deref(), Some("HSL"));

        for text in [
            "#12345",
            "#ggg",
            "rgb(300, 0, 0)",
            "rgb(1, 2)",
            "hsl(0, 120%, 50%)",
            "red",
        ] {
            assert_eq!(content(text), None, "{text}");
        }
    }

    #[test]
    fn reads_unix_times_in_seconds_and_milliseconds() {
        let Some(CardContent::UnixTime {
            milliseconds,
            relative: when,
            rows,
            ..
        }) = content("1791354301")
        else {
            panic!("a Unix time");
        };
        assert!(!milliseconds);
        assert_eq!(rows[2].value, "2026-10-07T06:25:01Z");
        assert_eq!(rows[1].value, "Wed 7 Oct 2026, 06:25:01 UTC");
        assert_eq!(when, "6 hours ago");

        let Some(CardContent::UnixTime {
            milliseconds, rows, ..
        }) = content("1791354301250")
        else {
            panic!("a Unix time in milliseconds");
        };
        assert!(milliseconds);
        assert_eq!(rows[2].value, "2026-10-07T06:25:01.250Z");

        for text in [
            "123",
            "999999999",
            "12345678901",
            "4300000000",
            "17913543x1",
        ] {
            assert_eq!(content(text), None, "{text}");
        }
        assert_eq!(relative(30), "Just now");
        assert_eq!(relative(2 * 3600), "In 2 hours");
        assert_eq!(relative(-86_400), "1 day ago");
    }

    #[test]
    fn reads_json_keeping_key_order_and_text() {
        let text = r#"{"id":"evt_8f","tags":["prod","canary"],"metrics":{"pods":6,"ok":true,"rollback":null},"empty":{},"n":1.50}"#;
        let Some(CardContent::Json {
            array,
            count,
            depth,
            keys,
            lines,
            more_lines,
            ..
        }) = content(text)
        else {
            panic!("JSON");
        };
        assert!(!array);
        assert_eq!((count, depth, more_lines), (5, 2, 0));
        let named: Vec<(&str, &str)> = keys
            .iter()
            .map(|k| (k.name.as_str(), k.hint.as_str()))
            .collect();
        assert_eq!(
            named,
            [
                ("id", ""),
                ("tags", "[2]"),
                ("metrics", "{3}"),
                ("empty", "{0}"),
                ("n", "")
            ]
        );
        let shown: Vec<String> = lines
            .iter()
            .map(|line| {
                let text: String = line.tokens.iter().map(|t| t.text.as_str()).collect();
                format!("{}{text}", "  ".repeat(line.indent))
            })
            .collect();
        assert_eq!(shown[0], "{");
        assert_eq!(shown[1], r#"  "id": "evt_8f","#);
        assert_eq!(shown[2], r#"  "tags": ["#);
        assert_eq!(shown[3], r#"    "prod","#);
        assert_eq!(shown[5], "  ],");
        assert_eq!(shown[9], r#"    "rollback": null"#);
        assert_eq!(shown[11], r#"  "empty": {},"#);
        assert_eq!(shown[12], r#"  "n": 1.50"#);
        assert_eq!(shown.last().map(String::as_str), Some("}"));
        assert_eq!(lines[1].tokens[0].kind, TokenKind::Key);
        assert_eq!(lines[1].tokens[2].kind, TokenKind::String);

        assert_eq!(format_json(text, false).as_deref(), Some(text));
        let pretty = format_json(text, true).unwrap();
        assert_eq!(pretty.lines().count(), shown.len());
        assert!(pretty.starts_with("{\n  \"id\": \"evt_8f\","));
        assert_eq!(format_json(&pretty, false).as_deref(), Some(text));

        let Some(CardContent::Json { array, count, .. }) = content("[1, [2, 3], {\"a\": []}]")
        else {
            unreachable!()
        };
        assert!(array);
        assert_eq!(count, 3);
        for text in ["{\"a\":", "42", "\"text\"", "[1,]", "{} trailing"] {
            assert_eq!(content(text), None, "{text}");
        }
    }

    #[test]
    fn long_json_shows_its_first_lines() {
        let items: Vec<String> = (0..3000).map(|n| n.to_string()).collect();
        let text = format!("[{}]", items.join(","));
        let Some(CardContent::Json {
            lines, more_lines, ..
        }) = content(&text)
        else {
            unreachable!()
        };
        assert_eq!(lines.len(), MAX_JSON_LINES);
        assert_eq!(more_lines, 3002 - MAX_JSON_LINES);
        assert!(detect(&" ".repeat(MAX_CARD_BYTES + 1), now()).is_none());
    }

    #[test]
    fn a_typed_color_or_unix_time_is_an_answer() {
        let color = answer(" #2f6f5e ", now()).unwrap();
        assert_eq!(color.kind, ResultKind::Color);
        assert_eq!(color.title, "#2F6F5E");
        assert_eq!(
            color.subtitle,
            "rgb(47, 111, 94) · hsl(164, 41%, 31%) · 5.9:1 on white"
        );
        assert_eq!(
            color.icon,
            Icon::Color {
                hex: "#2F6F5E".into()
            }
        );
        assert_eq!(color.actions[0].label, "Copy HEX");

        let time = answer("1791354301", now()).unwrap();
        assert_eq!(time.kind, ResultKind::DateTime);
        assert!(
            time.subtitle
                .starts_with("Wed 7 Oct 2026, 06:25:01 UTC · 6 hours ago"),
            "{}",
            time.subtitle
        );
        assert_eq!(time.actions[0].label, "Copy Local Time");

        for query in ["{\"a\": 1}", "755", "hello", ""] {
            assert_eq!(answer(query, now()), None, "{query}");
        }
    }

    #[test]
    fn a_new_copy_gets_a_new_card_id() {
        let first = detect("#fff", now()).unwrap();
        assert_eq!(first.id, detect("#fff", now()).unwrap().id);
        assert_ne!(first.id, detect("#ffff00", now()).unwrap().id);
        assert_eq!(first.actions[0].label, "Copy HEX");
    }
}
