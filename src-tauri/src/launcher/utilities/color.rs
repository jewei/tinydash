use super::UtilityResult;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct Color {
    pub hex: String,
    pub rgb: String,
    pub hsl: String,
    pub alpha: f64,
}

pub fn parse(input: &str) -> UtilityResult<Color> {
    if input.len() > 128 {
        return Err("Color is too long".into());
    }
    let text = input.trim().to_ascii_lowercase();
    let (r, g, b, a) = if let Some(hex) = text.strip_prefix('#') {
        if !hex.is_ascii() || !hex.bytes().all(|c| c.is_ascii_hexdigit()) {
            return Err("Invalid hex color".into());
        }
        let full = match hex.len() {
            3 | 4 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
            6 | 8 => hex.to_owned(),
            _ => return Err("Use #RGB, #RGBA, #RRGGBB or #RRGGBBAA".into()),
        };
        let byte = |i| u8::from_str_radix(&full[i..i + 2], 16).unwrap_or(0) as f64;
        (
            byte(0),
            byte(2),
            byte(4),
            if full.len() == 8 {
                byte(6) / 255.0
            } else {
                1.0
            },
        )
    } else {
        let (kind, values) = text
            .split_once('(')
            .ok_or("Use hex, rgb()/rgba(), or hsl()/hsla()")?;
        let values = values
            .strip_suffix(')')
            .ok_or("Missing closing parenthesis")?;
        let parts: Vec<_> = values
            .split(|c: char| c == ',' || c == '/' || c.is_ascii_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        if !(3..=4).contains(&parts.len()) {
            return Err("Expected three channels and optional alpha".into());
        }
        let number = |s: &str, max: f64| -> UtilityResult<f64> {
            let (value, percent) = s.strip_suffix('%').map_or((s, false), |v| (v, true));
            let n: f64 = value.parse().map_err(|_| "Invalid numeric channel")?;
            let n = if percent { n * max / 100.0 } else { n };
            if !n.is_finite() || n < 0.0 || n > max {
                return Err("Color channel out of range".into());
            }
            Ok(n)
        };
        let a = if parts.len() == 4 {
            number(parts[3], 1.0)?
        } else {
            1.0
        };
        match kind {
            "rgb" | "rgba" => (
                number(parts[0], 255.0)?,
                number(parts[1], 255.0)?,
                number(parts[2], 255.0)?,
                a,
            ),
            "hsl" | "hsla" => {
                let hue: f64 = parts[0]
                    .trim_end_matches("deg")
                    .parse()
                    .map_err(|_| "Invalid hue")?;
                if !hue.is_finite() || !parts[1].ends_with('%') || !parts[2].ends_with('%') {
                    return Err("HSL needs a finite hue and percentage saturation/lightness".into());
                }
                let h = hue.rem_euclid(360.0) / 60.0;
                let s = number(parts[1], 1.0)?;
                let l = number(parts[2], 1.0)?;
                let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
                let x = c * (1.0 - (h % 2.0 - 1.0).abs());
                let m = l - c / 2.0;
                let (r, g, b) = match h as u8 {
                    0 => (c, x, 0.0),
                    1 => (x, c, 0.0),
                    2 => (0.0, c, x),
                    3 => (0.0, x, c),
                    4 => (x, 0.0, c),
                    _ => (c, 0.0, x),
                };
                ((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0, a)
            }
            _ => return Err("Use hex, rgb()/rgba(), or hsl()/hsla()".into()),
        }
    };
    Ok(convert(
        r.round() as u8,
        g.round() as u8,
        b.round() as u8,
        a,
    ))
}

fn convert(r: u8, g: u8, b: u8, a: f64) -> Color {
    let channels = [r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0];
    let [rf, gf, bf] = channels;
    let max = channels.into_iter().fold(0.0, f64::max);
    let min = channels.into_iter().fold(1.0, f64::min);
    let d = max - min;
    let l = (max + min) / 2.0;
    let s = if d == 0.0 {
        0.0
    } else {
        d / (1.0 - (2.0 * l - 1.0).abs())
    };
    let h = if d == 0.0 {
        0.0
    } else if max == rf {
        60.0 * ((gf - bf) / d).rem_euclid(6.0)
    } else if max == gf {
        60.0 * ((bf - rf) / d + 2.0)
    } else {
        60.0 * ((rf - gf) / d + 4.0)
    };
    let hex = if a >= 1.0 {
        format!("#{r:02X}{g:02X}{b:02X}")
    } else {
        format!("#{r:02X}{g:02X}{b:02X}{:02X}", (a * 255.0).round() as u8)
    };
    Color {
        hex,
        rgb: format!("rgba({r}, {g}, {b}, {a:.3})"),
        hsl: format!("hsla({h:.1}, {:.1}%, {:.1}%, {a:.3})", s * 100.0, l * 100.0),
        alpha: a,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conversions() {
        for input in [
            "#f00",
            "rgb(255,0,0)",
            "hsl(360 100% 50%)",
            "rgb(100% 0% 0%)",
        ] {
            assert_eq!(parse(input).unwrap().hex, "#FF0000");
        }
        assert_eq!(parse("#abcd").unwrap().hex, "#AABBCCDD");
        assert_eq!(parse("rgba(0, 0, 0, .5)").unwrap().hex, "#00000080");
        assert!(parse("#808080").unwrap().hsl.starts_with("hsla(0.0, 0.0%"));
    }
    #[test]
    fn rejects_invalid_channels() {
        for input in [
            "#🤔",
            "rgb(256 0 0)",
            "rgb(NaN 0 0)",
            "hsl(inf 0% 0%)",
            "hsl(0 1 1)",
            "rgba(0 0 0 2)",
            "red",
            "#ggg",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
    }
}
