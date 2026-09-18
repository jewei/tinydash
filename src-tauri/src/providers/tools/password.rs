use std::sync::OnceLock;

use crate::launcher::result::ToolDetail;

const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const DIGITS: &[u8] = b"0123456789";
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    Symbols,
    Alphanumeric,
    Words,
    Pin,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Spec {
    pub style: Style,
    pub length: usize,
}

pub fn is_candidate(query: &str) -> bool {
    matches!(
        query
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "password" | "passwords" | "passphrase" | "pin"
    )
}

pub fn parse(query: &str) -> Result<Vec<Spec>, String> {
    let lower = query.to_ascii_lowercase();
    let mut parts: Vec<_> = lower.split_whitespace().collect();
    let mut style = match parts.first().copied() {
        Some("passphrase") => {
            parts.remove(0);
            Some(Style::Words)
        }
        Some("pin") => {
            parts.remove(0);
            Some(Style::Pin)
        }
        Some("password" | "passwords") => {
            parts.remove(0);
            None
        }
        _ => None,
    };
    let mut length = None;
    for part in parts {
        let choice = match part {
            "symbols" => Some(Style::Symbols),
            "letters" | "alphanumeric" => Some(Style::Alphanumeric),
            "words" | "passphrase" => Some(Style::Words),
            "pin" => Some(Style::Pin),
            _ => None,
        };
        if let Some(choice) = choice {
            if style.replace(choice).is_some() {
                return Err("Choose one password type: symbols, letters, words, or PIN.".into());
            }
        } else if let Ok(count) = part.parse::<usize>() {
            if length.replace(count).is_some() {
                return Err("Enter one length, such as password 32.".into());
            }
        } else {
            return Err("Try password 32, password letters 20, passphrase 6, or pin 6.".into());
        }
    }
    let specs = match style {
        Some(style) => vec![Spec {
            style,
            length: length.unwrap_or(if matches!(style, Style::Words | Style::Pin) {
                6
            } else {
                20
            }),
        }],
        None => vec![
            Spec {
                style: Style::Symbols,
                length: length.unwrap_or(20),
            },
            Spec {
                style: Style::Alphanumeric,
                length: length.unwrap_or(20),
            },
            Spec {
                style: Style::Words,
                length: 6,
            },
            Spec {
                style: Style::Pin,
                length: 6,
            },
        ],
    };
    for spec in &specs {
        let (range, message) = match spec.style {
            Style::Words => (3..=12, "Use 3 to 12 words for a passphrase."),
            Style::Pin => (4..=12, "Use 4 to 12 digits for a PIN."),
            _ => (6..=64, "Use 6 to 64 characters for a password."),
        };
        if !range.contains(&spec.length) {
            return Err(message.into());
        }
    }
    Ok(specs)
}

fn random_index(bound: usize) -> Result<usize, String> {
    let bound = bound as u32;
    let limit = u32::MAX - u32::MAX % bound;
    loop {
        let mut bytes = [0; 4];
        getrandom::fill(&mut bytes)
            .map_err(|_| "The system random source is unavailable. Try again.".to_string())?;
        let value = u32::from_ne_bytes(bytes);
        // Rejection avoids the bias produced by taking every random value modulo bound.
        if value < limit {
            return Ok((value % bound) as usize);
        }
    }
}

fn words() -> &'static Vec<&'static str> {
    static WORDS: OnceLock<Vec<&str>> = OnceLock::new();
    WORDS.get_or_init(|| {
        include_str!("data/eff_large_wordlist.txt")
            .lines()
            .filter_map(|line| line.split_once('\t').map(|(_, word)| word.trim()))
            .collect()
    })
}

fn character_password(length: usize, groups: &[&[u8]]) -> Result<(String, f64), String> {
    let alphabet: Vec<_> = groups
        .iter()
        .flat_map(|group| group.iter().copied())
        .collect();
    // Draw uniformly from all strings that contain each requested character class.
    // Do not insert predictable characters into fixed positions.
    let value = loop {
        let bytes: Vec<_> = (0..length)
            .map(|_| random_index(alphabet.len()).map(|index| alphabet[index]))
            .collect::<Result<_, _>>()?;
        if groups
            .iter()
            .all(|group| bytes.iter().any(|byte| group.contains(byte)))
        {
            break String::from_utf8(bytes).expect("ASCII alphabet");
        }
    };
    // Inclusion-exclusion counts the accepted strings, including the class requirement.
    let mut probability = 0.0;
    for mask in 0usize..(1 << groups.len()) {
        let excluded: usize = groups
            .iter()
            .enumerate()
            .filter(|(index, _)| mask & (1 << index) != 0)
            .map(|(_, group)| group.len())
            .sum();
        let fraction = (alphabet.len() - excluded) as f64 / alphabet.len() as f64;
        probability += if mask.count_ones() % 2 == 0 {
            1.0
        } else {
            -1.0
        } * fraction.powi(length as i32);
    }
    let bits = length as f64 * (alphabet.len() as f64).log2() + probability.log2();
    Ok((value, bits))
}

pub fn generate(spec: Spec) -> Result<(String, String, ToolDetail), String> {
    let (value, bits, variant, unit) = match spec.style {
        Style::Symbols => {
            let (value, bits) = character_password(spec.length, &[LOWER, UPPER, DIGITS, SYMBOLS])?;
            (value, bits, "Letters, digits & symbols", "characters")
        }
        Style::Alphanumeric => {
            let (value, bits) = character_password(spec.length, &[LOWER, UPPER, DIGITS])?;
            (value, bits, "Letters & digits", "characters")
        }
        Style::Words => {
            let words = words();
            let selected: Vec<_> = (0..spec.length)
                .map(|_| random_index(words.len()).map(|index| words[index]))
                .collect::<Result<_, _>>()?;
            (
                selected.join(" "),
                spec.length as f64 * (words.len() as f64).log2(),
                "Word passphrase",
                "words",
            )
        }
        Style::Pin => {
            let value: String = (0..spec.length)
                .map(|_| random_index(10).map(|index| DIGITS[index] as char))
                .collect::<Result<_, _>>()?;
            (value, spec.length as f64 * 10f64.log2(), "PIN", "digits")
        }
    };
    let bits = bits.floor() as u32;
    let strength = match bits {
        0..=39 => "Low",
        40..=59 => "Moderate",
        60..=99 => "Strong",
        _ => "Very strong",
    };
    Ok((
        value,
        format!(
            "{variant} · {} {unit} · {strength} · {bits} bits",
            spec.length
        ),
        ToolDetail::Password {
            variant: variant.into(),
            entropy_bits: bits,
            strength: strength.into(),
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_four_types_and_enforces_lengths() {
        assert_eq!(parse("password").unwrap().len(), 4);
        assert_eq!(parse("password 32").unwrap()[0].length, 32);
        assert_eq!(
            parse("password letters 8").unwrap(),
            vec![Spec {
                style: Style::Alphanumeric,
                length: 8
            }]
        );
        for query in [
            "password 5",
            "password 65",
            "password 999999999999999999999999999999",
            "password words 2",
            "pin 3",
            "password 32 40",
            "password words pin",
        ] {
            assert!(parse(query).is_err(), "{query}");
        }
    }

    #[test]
    fn generated_values_match_their_alphabets_and_strength_estimates() {
        for length in [6, 20, 64] {
            for style in [Style::Symbols, Style::Alphanumeric] {
                for _ in 0..20 {
                    let (value, _, detail) = generate(Spec { style, length }).unwrap();
                    assert_eq!(value.len(), length);
                    for group in [LOWER, UPPER, DIGITS] {
                        assert!(value.bytes().any(|byte| group.contains(&byte)));
                    }
                    if style == Style::Symbols {
                        assert!(value.bytes().any(|byte| SYMBOLS.contains(&byte)));
                    } else {
                        assert!(value.chars().all(|c| c.is_ascii_alphanumeric()));
                    }
                    let ToolDetail::Password { entropy_bits, .. } = detail else {
                        panic!("password detail")
                    };
                    assert!(
                        entropy_bits > 25 && entropy_bits <= (length as f64 * 85f64.log2()) as u32
                    );
                }
            }
        }
        let (pin, _, _) = generate(Spec {
            style: Style::Pin,
            length: 6,
        })
        .unwrap();
        assert_eq!(pin.len(), 6);
        assert!(pin.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(words().len(), 7776);
        assert_eq!(
            words()
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            7776
        );
        let (phrase, _, detail) = generate(Spec {
            style: Style::Words,
            length: 6,
        })
        .unwrap();
        assert!(
            phrase
                .split_whitespace()
                .all(|word| words().contains(&word))
        );
        assert!(matches!(
            detail,
            ToolDetail::Password {
                entropy_bits: 77,
                ..
            }
        ));
    }
}
