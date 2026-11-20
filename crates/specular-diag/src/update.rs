//! Manual and opt-in automatic update checks.
//!
//! Each check makes one GET with the user agent `Specular/<version>`. Automatic checks
//! run at most once a day. The release page provides the notes and downloads.
//!
//! Stable checks GitHub's latest published release. Beta also considers the recent
//! prereleases in the repository's public releases list.

use std::cmp::Ordering;
use std::path::Path;
use std::time::Duration;

use specular_core::CancelToken;
use specular_remote::{Client, ClientConfig, Request};

/// Seconds between automatic checks.
pub const INTERVAL: u64 = 86_400;

/// GitHub includes metadata for every installer with each release.
const MAX_FEED: u64 = 8 << 20;

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
    pub fn feed_url(self) -> &'static str {
        match self {
            Channel::Stable => "https://api.github.com/repos/amazynk/specular/releases/latest",
            Channel::Beta => "https://api.github.com/repos/amazynk/specular/releases",
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
    /// The published release page contains both the notes and the installers.
    pub url: String,
}

/// Read the latest-release object or the releases list from GitHub. A malformed response
/// must not make a failed check look like a confirmation that the app is up to date.
fn parse(feed: &[u8], channel: Channel) -> Result<Vec<Release>, String> {
    let json: serde_json::Value = serde_json::from_slice(feed).map_err(|e| e.to_string())?;
    let entries = match channel {
        Channel::Stable => std::slice::from_ref(&json),
        Channel::Beta => json
            .as_array()
            .ok_or("expected a GitHub releases list")?
            .as_slice(),
    };
    let mut releases = Vec::new();
    for entry in entries {
        if entry["draft"]
            .as_bool()
            .ok_or("missing release draft flag")?
        {
            continue;
        }
        let tag = entry["tag_name"].as_str().ok_or("missing release tag")?;
        let Some(version) = Version::parse(tag) else {
            continue;
        };
        let prerelease = entry["prerelease"]
            .as_bool()
            .ok_or("missing prerelease flag")?;
        let url = entry["html_url"]
            .as_str()
            .filter(|u| u.starts_with("https://github.com/amazynk/specular/releases/tag/"))
            .ok_or("invalid Specular release page")?;
        let published = entry["published_at"]
            .as_str()
            .ok_or("missing release date")?;
        releases.push(Release {
            version,
            version_text: tag.strip_prefix('v').unwrap_or(tag).to_owned(),
            channel: if prerelease {
                Channel::Beta
            } else {
                Channel::Stable
            },
            date: published.split('T').next().unwrap_or(published).to_owned(),
            url: url.to_owned(),
        });
    }
    if releases.is_empty() && !entries.is_empty() {
        return Err("no published Specular versions in the release feed".into());
    }
    Ok(releases)
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
    let req = Request::get(url)
        .accept("application/vnd.github+json")
        .limit(MAX_FEED);
    let resp = client.get(&req, cancel).map_err(|e| e.to_string())?;
    let releases = parse(&resp.body, channel)?;
    Ok(match newer(&releases, current, channel) {
        Some(r) => Outcome::Available(r.clone()),
        None => Outcome::Current,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_remote::testing::{ServerOptions, TestResponse, TestServer};

    fn entry(tag: &str, beta: bool) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag, "draft": false, "prerelease": beta,
            "published_at": "2026-09-01T12:00:00Z",
            "html_url": format!("https://github.com/amazynk/specular/releases/tag/{tag}")
        })
    }

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
    fn invalid_responses_do_not_report_up_to_date() {
        for bad in [
            b"[]".as_slice(),
            b"<html>",
            br#"{"message":"rate limited"}"#,
        ] {
            assert!(parse(bad, Channel::Stable).is_err());
        }
        let mut release = entry("v1.2.0", false);
        release["html_url"] = "file:///tmp/not-a-release".into();
        assert!(parse(release.to_string().as_bytes(), Channel::Stable).is_err());
    }

    #[test]
    fn the_channel_picks_the_release() {
        let mut draft = entry("v9.0.0", false);
        draft["draft"] = true.into();
        let feed = serde_json::json!([
            entry("v1.2.0", false),
            entry("v1.3.0-beta.2", true),
            entry("v1.3.0-beta.10", true),
            entry("v1.1.0", false),
            draft,
            entry("v1.2.1", true)
        ]);
        let r = parse(feed.to_string().as_bytes(), Channel::Beta).unwrap();
        assert_eq!(r.len(), 5);
        assert_eq!(r[0].date, "2026-09-01");
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

    /// Exercise the production HTTP client, TLS, both endpoints and the response parser.
    #[test]
    #[ignore = "contacts the public Specular releases on GitHub"]
    fn published_releases_are_reachable() {
        let client = client().unwrap();
        let cancel = CancelToken::new();
        for channel in [Channel::Stable, Channel::Beta] {
            let url = channel.feed_url();
            let Outcome::Available(release) =
                check(&client, url, &v("0.0.0"), channel, &cancel).unwrap()
            else {
                panic!("expected a published release on {channel:?}");
            };
            eprintln!("{channel:?}: {} ({})", release.version_text, release.url);
            assert_eq!(
                check(&client, url, &release.version, channel, &cancel).unwrap(),
                Outcome::Current
            );
        }
    }

    /// Against a local server only: one plain GET, no cookies, no query, our user agent.
    #[test]
    fn the_check_is_a_plain_get() {
        let srv = TestServer::start(ServerOptions::default(), |req| {
            if req.path() == "/releases/latest" {
                TestResponse::ok(entry("v1.2.0", false).to_string()).header("Set-Cookie", "id=1")
            } else {
                TestResponse::status(404)
            }
        })
        .unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let url = srv.url("/releases/latest");
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
