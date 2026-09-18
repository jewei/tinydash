use url::Url;

pub fn is_candidate(query: &str) -> bool {
    let query = query.to_ascii_lowercase();
    query.starts_with("https://")
        || query.starts_with("http://")
        || query.starts_with("clean ")
        || query.starts_with("url ")
        || query == "clean"
        || query == "url"
}

pub struct Cleaned {
    pub original: String,
    pub value: String,
    pub removed: usize,
}

fn host_is(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

fn tracking(key: &str, host: &str) -> bool {
    let key = key.to_ascii_lowercase();
    if key.starts_with("utm_")
        || matches!(
            key.as_str(),
            "fbclid"
                | "gclid"
                | "dclid"
                | "msclkid"
                | "gbraid"
                | "wbraid"
                | "igshid"
                | "mc_cid"
                | "mc_eid"
                | "_hsenc"
                | "_hsmi"
                | "vero_id"
                | "oly_anon_id"
                | "oly_enc_id"
        )
    {
        return true;
    }
    if amazon(host)
        && matches!(
            key.as_str(),
            "tag"
                | "linkcode"
                | "linkid"
                | "ref"
                | "ref_"
                | "ascsubtag"
                | "creative"
                | "creativeasin"
                | "camp"
                | "adid"
                | "sr"
                | "qid"
                | "sprefix"
                | "crid"
                | "dib"
                | "dib_tag"
                | "pd_rd_w"
                | "pd_rd_r"
                | "pd_rd_wg"
                | "pf_rd_p"
                | "pf_rd_r"
        )
    {
        return true;
    }
    if ["youtube.com", "youtu.be", "youtube-nocookie.com"]
        .iter()
        .any(|domain| host_is(host, domain))
        && matches!(
            key.as_str(),
            "si" | "feature" | "pp" | "ab_channel" | "embeds_referring_euri" | "source_ve_path"
        )
    {
        return true;
    }
    host_is(host, "spotify.com") && matches!(key.as_str(), "si" | "nd" | "dlsi")
}

fn amazon(host: &str) -> bool {
    [
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
        "amazon.com.br",
        "amazon.com.mx",
        "amazon.nl",
        "amazon.se",
        "amazon.pl",
        "amazon.sg",
        "amazon.ae",
        "amazon.sa",
        "amazon.com.tr",
        "amazon.eg",
        "amazon.com.be",
        "amazon.co.za",
        "amazon.ie",
    ]
    .iter()
    .any(|domain| host_is(host, domain))
}

pub fn clean(query: &str) -> Result<Cleaned, String> {
    let original = query.trim();
    let original = original
        .split_once(' ')
        .filter(|(command, _)| {
            command.eq_ignore_ascii_case("clean") || command.eq_ignore_ascii_case("url")
        })
        .map_or(original, |(_, value)| value.trim());
    if original.chars().any(char::is_control) {
        return Err("Use a URL without line breaks or control characters.".into());
    }
    let mut url =
        Url::parse(original).map_err(|_| "Enter a full http:// or https:// URL.".to_string())?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Use an http:// or https:// URL without a username or password.".into());
    }
    let host = url.host_str().unwrap().to_owned();
    let mut removed = 0;
    if let Some(query) = url.query() {
        let kept: Vec<_> = query
            .split('&')
            .filter(|part| {
                let key = url::form_urlencoded::parse(part.as_bytes())
                    .next()
                    .map(|(key, _)| key.into_owned())
                    .unwrap_or_default();
                if tracking(&key, &host) {
                    removed += 1;
                    false
                } else {
                    true
                }
            })
            .collect();
        if removed > 0 {
            let query = kept.join("&");
            url.set_query((!query.is_empty()).then_some(&query));
        }
    }
    if amazon(&host)
        && let Some(index) = url.path().find("/ref=")
    {
        let path = url.path()[..index].to_owned();
        url.set_path(&path);
        removed += 1;
    }
    // Keep the original bytes when there is nothing to clean. Keep unrelated
    // query segments byte-for-byte, including duplicate keys, +, and percent escapes.
    let value = if removed == 0 {
        original.to_owned()
    } else {
        url.into()
    };
    Ok(Cleaned {
        original: original.into(),
        value,
        removed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removes_generic_trackers_without_reencoding_meaningful_parameters() {
        let result = clean(
            "https://example.com/p?q=a%20b&q=c+d&utm_source=x&%66bclid=y&token=a%2Fb%3D#section",
        )
        .unwrap();
        assert_eq!(result.removed, 2);
        assert_eq!(
            result.value,
            "https://example.com/p?q=a%20b&q=c+d&token=a%2Fb%3D#section"
        );
        assert_eq!(
            clean("https://example.com?q=%2f").unwrap().value,
            "https://example.com?q=%2f"
        );
    }
    #[test]
    fn domain_rules_keep_products_playlists_timestamps_and_other_sites_intact() {
        let result = clean("https://www.amazon.co.uk/Title/dp/B012345678/ref=sr_1_1?tag=affiliate&th=1&psc=1&qid=123").unwrap();
        assert_eq!(result.removed, 3);
        assert_eq!(
            result.value,
            "https://www.amazon.co.uk/Title/dp/B012345678?th=1&psc=1"
        );
        assert_eq!(
            clean("https://youtu.be/abc?si=tracking&t=90&list=xyz")
                .unwrap()
                .value,
            "https://youtu.be/abc?t=90&list=xyz"
        );
        assert_eq!(
            clean("https://www.youtube.com/watch?v=abc&feature=share&si=1&index=2&t=30")
                .unwrap()
                .value,
            "https://www.youtube.com/watch?v=abc&index=2&t=30"
        );
        assert_eq!(
            clean("https://open.spotify.com/track/abc?si=1&nd=1&utm_source=copy&context=album")
                .unwrap()
                .value,
            "https://open.spotify.com/track/abc?context=album"
        );
        assert_eq!(
            clean("https://notamazon.com/?tag=keep&si=keep&ref=keep")
                .unwrap()
                .removed,
            0
        );
        assert_eq!(
            clean("https://amazon.com.evil.example/?tag=keep")
                .unwrap()
                .removed,
            0
        );
    }
    #[test]
    fn rejects_unsafe_or_malformed_urls() {
        for input in [
            "javascript:alert(1)",
            "file:///tmp/a",
            "https://user:pass@example.com",
            "not a url",
            "https://example.com/\npath",
        ] {
            assert!(clean(input).is_err(), "{input}");
        }
    }
}
