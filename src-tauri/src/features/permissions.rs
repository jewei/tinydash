//! Unix file permissions, both ways: `chmod 755` gives `rwxr-xr-x`, and
//! `rwxr-xr-x` or `-rw-r--r--` gives `644`. A bare number such as `755`
//! needs `chmod`, as it may mean anything.

use crate::search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol};

const WHO: [&str; 3] = ["Owner", "Group", "Others"];

pub fn answer(query: &str) -> Option<SearchResult> {
    let query = query.trim();
    let (mode, typed_octal) = match query.strip_prefix("chmod ") {
        Some(rest) => {
            let rest = rest.trim();
            octal(rest)
                .map(|mode| (mode, true))
                .or_else(|| symbolic(rest).map(|mode| (mode, false)))?
        }
        None => (symbolic(query)?, false),
    };
    let (octal_text, symbolic_text) = (octal_text(mode), symbolic_text(mode));
    let (title, other) = if typed_octal {
        (symbolic_text.clone(), octal_text.clone())
    } else {
        (octal_text.clone(), symbolic_text.clone())
    };
    Some(SearchResult {
        id: format!("chmod:{octal_text}"),
        kind: ResultKind::Permissions,
        title: title.clone(),
        subtitle: describe(mode),
        icon: Icon::Symbol { name: Symbol::Lock },
        actions: vec![
            ResultAction::new(format!("Copy {title}"), Action::Copy { text: title }),
            ResultAction::new(format!("Copy {other}"), Action::Copy { text: other }),
        ],
        pinned: false,
    })
}

/// `755` or `4755`: three or four octal digits, the first of four holding
/// setuid (4), setgid (2), and sticky (1).
fn octal(text: &str) -> Option<u16> {
    if !(3..=4).contains(&text.len()) || !text.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
        return None;
    }
    u16::from_str_radix(text, 8).ok()
}

/// `rwxr-xr-x`, or `-rwxr-xr-x` with a file type first. `s` or `S` marks
/// setuid or setgid, and `t` or `T` sticky, with or without the run bit.
fn symbolic(text: &str) -> Option<u16> {
    let text = match text.len() {
        10 if text.starts_with(['-', 'd', 'l']) => &text[1..],
        9 => text,
        _ => return None,
    };
    let bytes = text.as_bytes();
    let mut mode = 0;
    for (index, chunk) in bytes.chunks(3).enumerate() {
        let shift = 6 - 3 * index as u16;
        let special = [0o4000, 0o2000, 0o1000][index];
        let special_letter = if index == 2 { b't' } else { b's' };
        match chunk[0] {
            b'r' => mode |= 0o4 << shift,
            b'-' => {}
            _ => return None,
        }
        match chunk[1] {
            b'w' => mode |= 0o2 << shift,
            b'-' => {}
            _ => return None,
        }
        match chunk[2] {
            b'x' => mode |= 0o1 << shift,
            b'-' => {}
            letter if letter == special_letter => mode |= special | (0o1 << shift),
            letter if letter == special_letter.to_ascii_uppercase() => mode |= special,
            _ => return None,
        }
    }
    Some(mode)
}

fn octal_text(mode: u16) -> String {
    if mode >= 0o1000 {
        format!("{mode:04o}")
    } else {
        format!("{mode:03o}")
    }
}

fn symbolic_text(mode: u16) -> String {
    let mut text = String::with_capacity(9);
    for index in 0..3 {
        let bits = (mode >> (6 - 3 * index)) & 0o7;
        let special = mode & [0o4000, 0o2000, 0o1000][index] != 0;
        text.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        text.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        let run = bits & 0o1 != 0;
        let letter = if index == 2 { 't' } else { 's' };
        text.push(match (special, run) {
            (true, true) => letter,
            (true, false) => letter.to_ascii_uppercase(),
            (false, true) => 'x',
            (false, false) => '-',
        });
    }
    text
}

/// `Owner: read, write, run · Group: read · Others: nothing · setuid`
fn describe(mode: u16) -> String {
    let mut parts: Vec<String> = WHO
        .iter()
        .enumerate()
        .map(|(index, who)| {
            let bits = (mode >> (6 - 3 * index)) & 0o7;
            let can: Vec<&str> = [(0o4, "read"), (0o2, "write"), (0o1, "run")]
                .into_iter()
                .filter(|(bit, _)| bits & bit != 0)
                .map(|(_, word)| word)
                .collect();
            let can = if can.is_empty() {
                "nothing".to_owned()
            } else {
                can.join(", ")
            };
            format!("{who}: {can}")
        })
        .collect();
    for (bit, name) in [(0o4000, "setuid"), (0o2000, "setgid"), (0o1000, "sticky")] {
        if mode & bit != 0 {
            parts.push(name.into());
        }
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn title(query: &str) -> Option<String> {
        answer(query).map(|result| result.title)
    }

    #[test]
    fn converts_both_ways() {
        assert_eq!(title("chmod 755").as_deref(), Some("rwxr-xr-x"));
        assert_eq!(title("chmod 0644").as_deref(), Some("rw-r--r--"));
        assert_eq!(title("chmod 4755").as_deref(), Some("rwsr-xr-x"));
        assert_eq!(title("chmod 1777").as_deref(), Some("rwxrwxrwt"));
        assert_eq!(title("rwxr-xr-x").as_deref(), Some("755"));
        assert_eq!(title("-rw-r--r--").as_deref(), Some("644"));
        assert_eq!(title("drwxrwxrwt").as_deref(), Some("1777"));
        assert_eq!(title("rwSr--r--").as_deref(), Some("4644"));
        assert_eq!(title("chmod rw-------").as_deref(), Some("600"));

        let result = answer("chmod 750").unwrap();
        assert_eq!(
            result.subtitle,
            "Owner: read, write, run · Group: read, run · Others: nothing"
        );
        assert_eq!(result.actions[1].label, "Copy 750");
        assert_eq!(
            answer("chmod 2755").unwrap().subtitle.rsplit(" · ").next(),
            Some("setgid")
        );
    }

    #[test]
    fn ignores_what_is_not_a_mode() {
        for query in [
            "755",
            "chmod 9",
            "chmod 888",
            "chmod 75555",
            "rwxr-xr-y",
            "hello wor",
            "chmod",
        ] {
            assert_eq!(answer(query), None, "{query}");
        }
    }
}
