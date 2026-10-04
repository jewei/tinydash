//! Remove tracking parameters from a pasted HTTP(S) URL. Works offline and
//! never follows the link.

use url::Url;

use crate::search::result::{Action, Icon, ResultAction, ResultKind, SearchResult, Symbol};

const MAX_LENGTH: usize = 8192;

/// Parameters that only identify a campaign or click, on any site.
const TRACKERS: &[&str] = &[
    "fbclid",
    "gclid",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "igshid",
    "mc_cid",
    "mc_eid",
    "_hsenc",
    "_hsmi",
    "yclid",
    "twclid",
    "ttclid",
    "vero_id",
    "oly_anon_id",
    "oly_enc_id",
];

/// Site-specific parameters, matched by host and its subdomains.
const SITE_TRACKERS: &[(&[&str], &[&str])] = &[
    (
        &[
            "amazon.com",
            "amazon.co.uk",
            "amazon.de",
            "amazon.fr",
            "amazon.it",
            "amazon.es",
            "amazon.ca",
            "amazon.co.jp",
            "amazon.in",
            "amazon.com.au",
            "amazon.sg",
            "amazon.nl",
        ],
        &[
            "tag",
            "linkcode",
            "linkid",
            "ref",
            "ref_",
            "ascsubtag",
            "creative",
            "creativeasin",
            "camp",
            "adid",
            "sr",
            "qid",
            "sprefix",
            "crid",
            "dib",
            "dib_tag",
            "pd_rd_w",
            "pd_rd_r",
            "pd_rd_wg",
            "pf_rd_p",
            "pf_rd_r",
        ],
    ),
    (
        &["youtube.com", "youtu.be"],
        &["si", "feature", "pp", "ab_channel"],
    ),
    (&["spotify.com"], &["si", "nd", "dlsi"]),
    (&["twitter.com", "x.com"], &["s", "t"]),
    (&["instagram.com"], &["igsh"]),
];

pub fn answer(query: &str) -> Option<SearchResult> {
    let cleaned = clean(query)?;
    Some(SearchResult {
        id: format!("url:{cleaned}"),
        kind: ResultKind::Url,
        title: cleaned.clone(),
        subtitle: "Removed tracking parameters".into(),
        icon: Icon::Symbol { name: Symbol::Link },
        actions: vec![
            ResultAction::new(
                "Copy Clean URL",
                Action::Copy {
                    text: cleaned.clone(),
                },
            ),
            ResultAction::new("Open Clean URL", Action::OpenUrl { url: cleaned }),
        ],
        pinned: false,
    })
}

/// The cleaned URL, or `None` when the input is not a URL or has nothing to remove.
fn clean(input: &str) -> Option<String> {
    let input = input.trim();
    if input.len() > MAX_LENGTH || input.contains(char::is_whitespace) {
        return None;
    }
    let lower = input.to_ascii_lowercase();
    if !lower.starts_with("http://") && !lower.starts_with("https://") {
        return None;
    }
    let mut url = Url::parse(input).ok()?;
    let host = url.host_str()?.to_ascii_lowercase();
    let site: Vec<&str> = SITE_TRACKERS
        .iter()
        .filter(|(domains, _)| domains.iter().any(|domain| on_domain(&host, domain)))
        .flat_map(|(_, keys)| keys.iter().copied())
        .collect();
    let tracking = |key: &str| {
        let key = key.to_ascii_lowercase();
        key.starts_with("utm_") || TRACKERS.contains(&key.as_str()) || site.contains(&key.as_str())
    };
    let query = url.query()?.to_owned();
    // Keep untouched pairs byte for byte, so encoded values survive.
    let kept: Vec<&str> = query
        .split('&')
        .filter(|pair| {
            let key = pair.split('=').next().unwrap_or_default();
            !pair.is_empty() && !tracking(&percent_decode(key))
        })
        .collect();
    if kept.len() == query.split('&').filter(|pair| !pair.is_empty()).count() {
        return None;
    }
    url.set_query((!kept.is_empty()).then(|| kept.join("&")).as_deref());
    Some(url.into())
}

fn on_domain(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn percent_decode(text: &str) -> String {
    url::form_urlencoded::parse(format!("{text}=").as_bytes())
        .next()
        .map(|(key, _)| key.into_owned())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_trackers_and_keeps_everything_else() {
        assert_eq!(
            clean("https://example.com/a?utm_source=x&id=1&q=a%26b&fbclid=y#top").as_deref(),
            Some("https://example.com/a?id=1&q=a%26b#top")
        );
        assert_eq!(
            clean("https://www.youtube.com/watch?v=abc&t=42&si=share").as_deref(),
            Some("https://www.youtube.com/watch?v=abc&t=42")
        );
        assert_eq!(
            clean("https://shop.example?utm_campaign=x").as_deref(),
            Some("https://shop.example/")
        );
    }

    #[test]
    fn ignores_clean_urls_and_other_text() {
        assert_eq!(clean("https://example.com/?id=1"), None);
        assert_eq!(clean("https://example.com/"), None);
        assert_eq!(clean("example.com/?utm_source=x"), None);
        assert_eq!(clean("see https://example.com/?utm_source=x"), None);
    }

    #[test]
    fn site_rules_apply_only_on_their_site() {
        assert_eq!(
            clean("https://www.amazon.de/dp/B0?tag=aff&th=1").as_deref(),
            Some("https://www.amazon.de/dp/B0?th=1")
        );
        assert_eq!(clean("https://example.com/?tag=keep"), None);
    }
}
