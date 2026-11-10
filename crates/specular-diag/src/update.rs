//! The update check: off unless the user turns it on (Settings › Updates).
//!
//! At most once a day, one plain GET of the release feed: no identifiers, no cookies, no
//! query string, user agent `Specular/<version>`. Nothing is installed; the app shows
//! "Version X is available" with links to the release notes and the download page.
//!
//! The feed is JSON:
//!
//! ```json
//! {"releases": [{"version": "1.2.0", "channel": "stable", "date": "2026-10-01",
//!   "notes_url": "https://…", "download_url": "https://…"}]}
//! ```

use std::cmp::Ordering;
use std::path::Path;
use std::time::Duration;

use specular_core::CancelToken;
use specular_remote::{Client, ClientConfig, Request};

/// Seconds between automatic checks.
pub const INTERVAL: u64 = 86_400;

/// The largest feed read (a list of releases, a few KB).
const MAX_FEED: u64 = 1 << 20;

/// Which releases the user wants to hear about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Channel {
    /// Releases only.
    #[default]
    Stable,
    /// Releases and betas.
    Beta,
}

impl Channel {
    pub fn id(self) -> &'static str {
        match self {
            Channel::Stable => "stable",
            Channel::Beta => "beta",
        }
    }

    pub fn from_id(s: &str) -> Option<Channel> {
        match s {
            "stable" => Some(Channel::Stable),
            "beta" => Some(Channel::Beta),
            _ => None,
        }
    }
}

/// A pre-release identifier: numbers sort below words, numerically.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Ident {
    Num(u64),
    Word(String),
}

/// A semantic version (`1.2.3`, `1.3.0-beta.2`); build metadata (`+…`) is ignored.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pre: Vec<Ident>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Version> {
        let s = s.trim().trim_start_matches('v');
        let s = s.split_once('+').map_or(s, |(v, _)| v);
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, Some(p)),
            None => (s, None),
        };
        let mut n = core.split('.').map(|p| {
            (!p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
                .then(|| p.parse().ok())
                .flatten()
        });
        let (major, minor, patch) = (n.next()??, n.next()??, n.next()??);
        if n.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(p) => p
                .split('.')
                .map(|id| {
                    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
                    {
                        None
                    } else if id.bytes().all(|b| b.is_ascii_digit()) {
                        id.parse().ok().map(Ident::Num)
                    } else {
                        Some(Ident::Word(id.to_owned()))
                    }
                })
                .collect::<Option<Vec<_>>>()?,
        };
        Some(Version {
            major,
            minor,
            patch,
            pre,
        })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Version) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(o.major, o.minor, o.patch))
            .then_with(|| match (self.pre.is_empty(), o.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                // A release is newer than its pre-releases.
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => self.pre.cmp(&o.pre),
            })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, o: &Version) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// One release in the feed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Release {
    pub version: Version,
    /// The version as the feed writes it, for display.
    pub version_text: String,
    pub channel: Channel,
    pub date: String,
    pub notes_url: String,
    pub download_url: String,
}

/// Only web links are opened.
fn web_url(s: &str) -> bool {
    s.starts_with("https://") || s.starts_with("http://")
}

/// The releases in a feed. Entries that are malformed (bad version, unknown channel,
/// non-web links) are skipped; a feed that is not the expected JSON is an error.
pub fn parse(feed: &[u8]) -> Result<Vec<Release>, String> {
    let v: serde_json::Value = serde_json::from_slice(feed).map_err(|e| e.to_string())?;
    let list = v
        .get("releases")
        .and_then(|r| r.as_array())
        .ok_or("no \"releases\" list")?;
    let text =
        |r: &serde_json::Value, k: &str| r.get(k).and_then(|x| x.as_str()).map(str::to_owned);
    Ok(list
        .iter()
        .filter_map(|r| {
            let version_text = text(r, "version")?;
            let version = Version::parse(&version_text)?;
            let channel = Channel::from_id(&text(r, "channel").unwrap_or_else(|| "stable".into()))?;
            let notes_url = text(r, "notes_url")?;
            let download_url = text(r, "download_url")?;
            if !web_url(&notes_url) || !web_url(&download_url) {
                return None;
            }
            Some(Release {
                version,
                version_text,
                channel,
                date: text(r, "date").unwrap_or_default(),
                notes_url,
                download_url,
            })
        })
        .collect())
}

/// The newest release above `current` that `channel` offers (Stable: releases only).
pub fn newer<'a>(
    releases: &'a [Release],
    current: &Version,
    channel: Channel,
) -> Option<&'a Release> {
    releases
        .iter()
        .filter(|r| r.version > *current)
        .filter(|r| match channel {
            Channel::Stable => r.channel == Channel::Stable && !r.version.is_prerelease(),
            Channel::Beta => true,
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// Whether an automatic check is due: the check is on, and none happened in the last day
/// (or the clock went back by more than a day since).
pub fn due(enabled: bool, last: Option<u64>, now: u64) -> bool {
    enabled
        && match last {
            None => true,
            Some(l) => now >= l.saturating_add(INTERVAL) || now.saturating_add(INTERVAL) < l,
        }
}

/// The time of the last check, stored in `path` (seconds since the epoch).
pub fn load_last(path: &Path) -> Option<u64> {
    std::fs::read_to_string(path).ok()?.trim().parse().ok()
}

pub fn save_last(path: &Path, now: u64) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(path, format!("{now}\n"))
}

/// The outcome of a check.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// A newer release for the channel.
    Available(Release),
    /// Up to date.
    Current,
}

/// A client for the check: the default user agent (`Specular/<version>`), no cookies (the
/// HTTP client keeps none), short timeouts and few retries.
pub fn client() -> Result<Client, String> {
    Client::new(ClientConfig {
        response_timeout: Duration::from_secs(15),
        retries: 1,
        ..ClientConfig::default()
    })
    .map(|c| c.tracked(specular_core::net::What::UpdateCheck))
    .map_err(|e| e.to_string())
}

/// Fetches `url` and compares with `current`.
pub fn check(
    client: &Client,
    url: &str,
    current: &Version,
    channel: Channel,
    cancel: &CancelToken,
) -> Result<Outcome, String> {
    let req = Request::get(url).accept("application/json").limit(MAX_FEED);
    let resp = client.get(&req, cancel).map_err(|e| e.to_string())?;
    let releases = parse(&resp.body)?;
    Ok(match newer(&releases, current, channel) {
        Some(r) => Outcome::Available(r.clone()),
        None => Outcome::Current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    const FEED: &str = r#"{"releases": [
        {"version": "1.2.0", "channel": "stable", "date": "2026-09-01",
         "notes_url": "https://example.com/n/1.2.0", "download_url": "https://example.com/d"},
        {"version": "1.3.0-beta.2", "channel": "beta", "date": "2026-10-01",
         "notes_url": "https://example.com/n/1.3.0-beta.2", "download_url": "https://example.com/d"},
        {"version": "1.3.0-beta.10", "channel": "beta",
         "notes_url": "https://example.com/n/b10", "download_url": "https://example.com/d"},
        {"version": "1.1.0", "notes_url": "https://example.com/n/1.1.0", "download_url": "https://example.com/d"},
        {"version": "not a version", "notes_url": "https://x", "download_url": "https://x"},
        {"version": "9.0.0", "channel": "nightly", "notes_url": "https://x", "download_url": "https://x"},
        {"version": "9.0.0", "notes_url": "file:///etc/passwd", "download_url": "https://x"},
        {"version": "9.0.0"}
    ]}"#;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn versions_order_like_semver() {
        let order = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
            "1.0.1",
            "1.10.0",
            "2.0.0",
        ];
        for w in order.windows(2) {
            assert!(v(w[0]) < v(w[1]), "{} < {}", w[0], w[1]);
        }
        assert_eq!(v("v1.2.3+build.5"), v("1.2.3"));
        for bad in [
            "1.2",
            "1.2.3.4",
            "1..3",
            "a.b.c",
            "1.2.3-",
            "1.2.3-a..b",
            "",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn the_feed_parses_and_bad_entries_are_skipped() {
        let r = parse(FEED.as_bytes()).unwrap();
        let names: Vec<&str> = r.iter().map(|r| r.version_text.as_str()).collect();
        assert_eq!(names, ["1.2.0", "1.3.0-beta.2", "1.3.0-beta.10", "1.1.0"]);
        assert_eq!(r[3].channel, Channel::Stable);
        assert_eq!(r[0].date, "2026-09-01");
        assert!(parse(b"[]").is_err());
        assert!(parse(b"<html>").is_err());
        assert!(parse(br#"{"releases": []}"#).unwrap().is_empty());
    }

    #[test]
    fn the_channel_picks_the_release() {
        let r = parse(FEED.as_bytes()).unwrap();
        let pick = |cur: &str, ch| newer(&r, &v(cur), ch).map(|r| r.version_text.as_str());
        assert_eq!(pick("1.0.0", Channel::Stable), Some("1.2.0"));
        assert_eq!(pick("1.0.0", Channel::Beta), Some("1.3.0-beta.10"));
        assert_eq!(pick("1.2.0", Channel::Stable), None);
        assert_eq!(pick("1.3.0-beta.2", Channel::Beta), Some("1.3.0-beta.10"));
        assert_eq!(pick("1.3.0", Channel::Beta), None);
    }

    #[test]
    fn the_scheduler_checks_at_most_daily() {
        let day = INTERVAL;
        let t = 1_791_800_430;
        assert!(!due(false, None, t), "off by default means never");
        assert!(due(true, None, t));
        assert!(!due(true, Some(t), t));
        assert!(!due(true, Some(t), t + day - 1));
        assert!(due(true, Some(t), t + day));
        // The clock went back a little: wait; a lot (a bad clock was fixed): check.
        assert!(!due(true, Some(t), t - 60));
        assert!(due(true, Some(t), t - day - 1));

        let dir = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/diag"
        ))
        .join(format!("sched-{}", std::process::id()));
        let file = dir.join("last-update-check");
        assert_eq!(load_last(&file), None);
        save_last(&file, t).unwrap();
        assert_eq!(load_last(&file), Some(t));
        std::fs::write(&file, "garbage").unwrap();
        assert_eq!(load_last(&file), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Against a local server only: one plain GET, no cookies, no query, our user agent.
    #[test]
    fn the_check_is_a_plain_get() {
        let srv = TestServer::start(ServerOptions::default(), |req| {
            if req.path() == "/releases.json" {
                TestResponse::ok(FEED).header("Set-Cookie", "id=1")
            } else {
                TestResponse::status(404)
            }
        })
        .unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let url = srv.url("/releases.json");
        let got = check(&client, &url, &v("1.0.0"), Channel::Stable, &cancel).unwrap();
        assert!(matches!(got, Outcome::Available(ref r) if r.version_text == "1.2.0"));
        // A second request: the cookie is not sent back.
        let got = check(&client, &url, &v("1.2.0"), Channel::Stable, &cancel).unwrap();
        assert_eq!(got, Outcome::Current);
        let log = srv.log();
        assert_eq!(log.len(), 2);
        for r in &log {
            assert!(r.header("cookie").is_none());
            assert!(r.query().is_empty());
            let ua = r.header("user-agent").unwrap_or_default();
            assert_eq!(ua, concat!("Specular/", env!("CARGO_PKG_VERSION")));
        }
        assert!(
            check(
                &client,
                &srv.url("/missing"),
                &v("1.0.0"),
                Channel::Stable,
                &cancel
            )
            .is_err()
        );
    }
}
