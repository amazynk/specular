//! Amazon S3 (and S3-compatible) addresses, request signing and credentials.
//!
//! - [`S3Location`]: `s3://bucket/prefix`, virtual-hosted (`https://bucket.s3.region.amazonaws.com/…`)
//!   and path-style (`https://s3.region.amazonaws.com/bucket/…`) URLs, and `s3://` against a
//!   configured endpoint (MinIO, a test server), path-style.
//! - [`sign`]: AWS Signature Version 4 (HMAC-SHA256 from RustCrypto), checked against AWS's
//!   published examples.
//! - [`Credentials::configured`]: keys only from what the user set up: `AWS_ACCESS_KEY_ID` /
//!   `AWS_SECRET_ACCESS_KEY` (with `AWS_SESSION_TOKEN`), or the `AWS_PROFILE` (default
//!   `default`) profile of the shared credentials file. Read once, kept in memory; never
//!   logged, written or shown.
//!
//! Buckets are read anonymously first; a request is signed only after the bucket refused an
//! anonymous one and credentials were configured.

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

use crate::auth::Secret;

/// SHA-256 of an empty body (every GET and HEAD).
pub const EMPTY_SHA256: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

/// Where the S3 settings come from: the environment, or a test's own values.
pub trait Env {
    fn var(&self, name: &str) -> Option<String>;
    /// The home folder (for `~/.aws/credentials`).
    fn home(&self) -> Option<PathBuf>;
}

/// The process environment.
pub struct ProcessEnv;

impl Env for ProcessEnv {
    fn var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok().filter(|v| !v.trim().is_empty())
    }
    fn home(&self) -> Option<PathBuf> {
        specular_platform_home()
    }
}

fn specular_platform_home() -> Option<PathBuf> {
    #[cfg(windows)]
    let h = std::env::var_os("USERPROFILE");
    #[cfg(not(windows))]
    let h = std::env::var_os("HOME");
    h.filter(|h| !h.is_empty()).map(PathBuf::from)
}

/// Fixed values (tests).
#[derive(Clone, Debug, Default)]
pub struct MapEnv {
    pub vars: Vec<(String, String)>,
    pub home: Option<PathBuf>,
}

impl Env for MapEnv {
    fn var(&self, name: &str) -> Option<String> {
        self.vars
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
    }
    fn home(&self) -> Option<PathBuf> {
        self.home.clone()
    }
}

/// An object prefix in a bucket and how to reach it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct S3Location {
    /// `https://host[:port]` the requests go to (for virtual-hosted buckets, the bucket's
    /// own host).
    pub endpoint: String,
    pub bucket: String,
    /// The key prefix (no leading '/'; ends in '/' unless empty).
    pub prefix: String,
    pub region: String,
    /// Path-style: the bucket is the first path segment (else it is in the host).
    pub path_style: bool,
}

/// The region of `AWS_REGION` / `AWS_DEFAULT_REGION`, else us-east-1.
fn env_region(env: &dyn Env) -> String {
    env.var("AWS_REGION")
        .or_else(|| env.var("AWS_DEFAULT_REGION"))
        .unwrap_or_else(|| "us-east-1".to_owned())
}

fn dir_prefix(p: &str) -> String {
    let p = p.trim_matches('/');
    if p.is_empty() {
        String::new()
    } else {
        format!("{p}/")
    }
}

/// The region in an AWS S3 host name (`s3.us-west-2.amazonaws.com`,
/// `s3-us-west-2.amazonaws.com`, `s3.dualstack.eu-west-1.amazonaws.com`), `None` for the
/// global `s3.amazonaws.com`; `Err` when the host is not AWS S3.
fn aws_region(host: &str) -> Result<Option<String>, ()> {
    let h = host.strip_suffix(".amazonaws.com").ok_or(())?;
    if h == "s3" {
        return Ok(None);
    }
    let rest = h
        .strip_prefix("s3.")
        .or_else(|| h.strip_prefix("s3-"))
        .ok_or(())?;
    let rest = rest.strip_prefix("dualstack.").unwrap_or(rest);
    // A region ("us-west-2"); anything with more parts is another service.
    if rest.is_empty() || rest.contains('.') || !rest.contains('-') {
        return Err(());
    }
    Ok(Some(rest.to_owned()))
}

impl S3Location {
    /// Parses an S3 address. `s3://bucket/prefix` goes to `AWS_ENDPOINT_URL_S3` /
    /// `AWS_ENDPOINT_URL` (path-style) when set, else to the bucket's AWS host; an HTTPS URL
    /// is S3 only when its host is AWS S3's. `None` for anything else.
    pub fn parse(url: &str, env: &dyn Env) -> Option<S3Location> {
        let url = url.trim();
        let url = url.split(['?', '#']).next()?;
        if let Some(rest) = url
            .strip_prefix("s3://")
            .or_else(|| url.strip_prefix("S3://"))
        {
            let (bucket, prefix) = rest.split_once('/').unwrap_or((rest, ""));
            if !valid_bucket(bucket) {
                return None;
            }
            let region = env_region(env);
            if let Some(ep) = env
                .var("AWS_ENDPOINT_URL_S3")
                .or_else(|| env.var("AWS_ENDPOINT_URL"))
            {
                let ep = ep.trim_end_matches('/').to_owned();
                if !(ep.starts_with("http://") || ep.starts_with("https://")) {
                    return None;
                }
                return Some(S3Location {
                    endpoint: ep,
                    bucket: bucket.to_owned(),
                    prefix: dir_prefix(prefix),
                    region,
                    path_style: true,
                });
            }
            // Buckets with dots break the wildcard certificate of virtual hosts.
            let path_style = bucket.contains('.');
            let host = if region == "us-east-1" {
                "s3.amazonaws.com".to_owned()
            } else {
                format!("s3.{region}.amazonaws.com")
            };
            return Some(S3Location {
                endpoint: if path_style {
                    format!("https://{host}")
                } else {
                    format!("https://{bucket}.{host}")
                },
                bucket: bucket.to_owned(),
                prefix: dir_prefix(prefix),
                region,
                path_style,
            });
        }
        let (scheme, rest) = url.split_once("://")?;
        if !matches!(scheme.to_ascii_lowercase().as_str(), "https" | "http") {
            return None;
        }
        let (authority, path) = rest.split_once('/').unwrap_or((rest, ""));
        let host_port = authority.rsplit('@').next()?;
        let host = host_port.split(':').next()?.to_ascii_lowercase();
        let base = format!("{}://{}", scheme.to_ascii_lowercase(), host_port);
        // Path-style: the host is S3's own.
        if let Ok(region) = aws_region(&host) {
            let (bucket, prefix) = path.split_once('/').unwrap_or((path, ""));
            if !valid_bucket(bucket) {
                return None;
            }
            return Some(S3Location {
                endpoint: base,
                bucket: bucket.to_owned(),
                prefix: dir_prefix(prefix),
                region: region.unwrap_or_else(|| env_region(env)),
                path_style: true,
            });
        }
        // Virtual-hosted: `bucket.` before S3's host.
        let mut at = 0;
        while let Some(i) = host[at..].find('.') {
            let (bucket, s3host) = (&host[..at + i], &host[at + i + 1..]);
            if let Ok(region) = aws_region(s3host) {
                if !valid_bucket(bucket) {
                    return None;
                }
                return Some(S3Location {
                    endpoint: base,
                    bucket: bucket.to_owned(),
                    prefix: dir_prefix(path),
                    region: region.unwrap_or_else(|| env_region(env)),
                    path_style: false,
                });
            }
            at += i + 1;
        }
        None
    }

    /// The URL of the object `key` (relative to the prefix).
    pub fn object_url(&self, key: &str) -> String {
        format!("{}{}", self.endpoint, self.object_path(key))
    }

    /// The request path of the object `key`, URI-encoded as S3 signs it.
    pub fn object_path(&self, key: &str) -> String {
        let full = format!("{}{}", self.prefix, key);
        let enc = uri_encode(&full, false);
        if self.path_style {
            format!("/{}/{}", uri_encode(&self.bucket, true), enc)
        } else {
            format!("/{enc}")
        }
    }

    /// The bucket's own path ("/" or "/bucket"), where ListObjectsV2 goes.
    pub fn bucket_path(&self) -> String {
        if self.path_style {
            format!("/{}", uri_encode(&self.bucket, true))
        } else {
            "/".to_owned()
        }
    }

    /// The address for the info panel: `s3://bucket/prefix`.
    pub fn display(&self) -> String {
        format!("s3://{}/{}", self.bucket, self.prefix)
    }
}

/// S3 bucket naming rules, loosely (3–63 of lower-case letters, digits, '.', '-'; legacy
/// buckets with upper case and '_' are let through).
fn valid_bucket(b: &str) -> bool {
    (3..=255).contains(&b.len())
        && b.bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-' | b'_'))
}

/// URI-encodes as SigV4 wants: unreserved characters stay, '/' stays unless `slash`.
pub fn uri_encode(s: &str, slash: bool) -> String {
    let mut o = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                o.push(b as char)
            }
            b'/' if !slash => o.push('/'),
            _ => o.push_str(&format!("%{b:02X}")),
        }
    }
    o
}

/// An access key and its secret (and a session token for temporary credentials).
#[derive(Clone, PartialEq, Eq)]
pub struct Credentials {
    pub access_key: String,
    pub secret: Secret,
    pub session_token: Option<Secret>,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Credentials(<redacted>)")
    }
}

impl Credentials {
    /// The credentials the user configured, if any: the environment's keys, else the
    /// profile (`AWS_PROFILE`, else `default`) of `AWS_SHARED_CREDENTIALS_FILE` or
    /// `~/.aws/credentials`. Nothing is looked up when `AWS_NO_SIGN_REQUEST` is set.
    pub fn configured(env: &dyn Env) -> Option<Credentials> {
        if env
            .var("AWS_NO_SIGN_REQUEST")
            .is_some_and(|v| !v.eq_ignore_ascii_case("false") && v != "0")
        {
            return None;
        }
        if let (Some(k), Some(s)) = (
            env.var("AWS_ACCESS_KEY_ID"),
            env.var("AWS_SECRET_ACCESS_KEY"),
        ) {
            return Some(Credentials {
                access_key: k,
                secret: Secret::new(s),
                session_token: env.var("AWS_SESSION_TOKEN").map(Secret::new),
            });
        }
        let file = env
            .var("AWS_SHARED_CREDENTIALS_FILE")
            .map(PathBuf::from)
            .or_else(|| env.home().map(|h| h.join(".aws").join("credentials")))?;
        let profile = env.var("AWS_PROFILE").unwrap_or_else(|| "default".into());
        Self::from_file(&file, &profile)
    }

    /// The profile `profile` of a shared credentials file.
    pub fn from_file(path: &Path, profile: &str) -> Option<Credentials> {
        let meta = std::fs::metadata(path).ok()?;
        if !meta.is_file() || meta.len() > 1 << 20 {
            return None;
        }
        let text = std::fs::read_to_string(path).ok()?;
        Self::from_ini(&text, profile)
    }

    fn from_ini(text: &str, profile: &str) -> Option<Credentials> {
        let mut inside = false;
        let (mut key, mut secret, mut token) = (None, None, None);
        for line in text.lines() {
            let l = line.trim();
            if l.is_empty() || l.starts_with('#') || l.starts_with(';') {
                continue;
            }
            if let Some(name) = l.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
                let name = name.trim();
                let name = name.strip_prefix("profile ").unwrap_or(name).trim();
                inside = name == profile;
                continue;
            }
            if !inside {
                continue;
            }
            let Some((k, v)) = l.split_once('=') else {
                continue;
            };
            let v = v.trim().to_owned();
            match k.trim().to_ascii_lowercase().as_str() {
                "aws_access_key_id" => key = Some(v),
                "aws_secret_access_key" => secret = Some(Secret::new(v)),
                "aws_session_token" => token = Some(Secret::new(v)),
                _ => {}
            }
        }
        Some(Credentials {
            access_key: key.filter(|k| !k.is_empty())?,
            secret: secret.filter(|s| !s.is_empty())?,
            session_token: token,
        })
    }
}

/// `YYYYMMDDTHHMMSSZ` for a time.
pub fn amz_date(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn hmac(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut m = <Hmac<Sha256> as KeyInit>::new_from_slice(key).expect("HMAC takes any key");
    m.update(data);
    m.finalize().into_bytes().to_vec()
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}

/// Hex SHA-256.
pub fn sha256_hex(data: &[u8]) -> String {
    hex(&Sha256::digest(data))
}

/// The SigV4 signing key for a day (`YYYYMMDD`), region and service.
pub fn signing_key(secret: &str, date: &str, region: &str, service: &str) -> Vec<u8> {
    let k = hmac(format!("AWS4{secret}").as_bytes(), date.as_bytes());
    let k = hmac(&k, region.as_bytes());
    let k = hmac(&k, service.as_bytes());
    hmac(&k, b"aws4_request")
}

/// A request to sign.
pub struct ToSign<'a> {
    pub method: &'a str,
    /// The path, already URI-encoded ("/bucket/key").
    pub path: &'a str,
    /// Query parameters, not encoded.
    pub query: &'a [(String, String)],
    /// Headers to sign besides `x-amz-date` (`host` must be among them).
    pub headers: &'a [(String, String)],
    /// Hex SHA-256 of the body, or `UNSIGNED-PAYLOAD`.
    pub payload_hash: &'a str,
    pub region: &'a str,
    pub service: &'a str,
    /// `YYYYMMDDTHHMMSSZ`.
    pub amz_date: &'a str,
}

/// The canonical query string: parameters encoded, sorted by name then value.
pub fn canonical_query(query: &[(String, String)]) -> String {
    let mut q: Vec<(String, String)> = query
        .iter()
        .map(|(k, v)| (uri_encode(k, true), uri_encode(v, true)))
        .collect();
    q.sort();
    q.iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join("&")
}

/// The canonical request, the string to sign and the `Authorization` value, in that order.
pub fn sign_parts(r: &ToSign<'_>, creds: &Credentials) -> (String, String, String) {
    let mut h: Vec<(String, String)> = r
        .headers
        .iter()
        .map(|(k, v)| {
            (
                k.trim().to_ascii_lowercase(),
                v.split_whitespace().collect::<Vec<_>>().join(" "),
            )
        })
        .collect();
    if !h.iter().any(|(k, _)| k == "x-amz-date") {
        h.push(("x-amz-date".into(), r.amz_date.to_owned()));
    }
    if let Some(t) = &creds.session_token
        && !h.iter().any(|(k, _)| k == "x-amz-security-token")
    {
        h.push(("x-amz-security-token".into(), t.expose().to_owned()));
    }
    h.sort();
    let canonical_headers: String = h.iter().map(|(k, v)| format!("{k}:{v}\n")).collect();
    let signed: String = h
        .iter()
        .map(|(k, _)| k.as_str())
        .collect::<Vec<_>>()
        .join(";");
    let creq = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        r.method,
        r.path,
        canonical_query(r.query),
        canonical_headers,
        signed,
        r.payload_hash
    );
    let date = &r.amz_date[..8.min(r.amz_date.len())];
    let scope = format!("{date}/{}/{}/aws4_request", r.region, r.service);
    let sts = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        r.amz_date,
        sha256_hex(creq.as_bytes())
    );
    let key = signing_key(creds.secret.expose(), date, r.region, r.service);
    let sig = hex(&hmac(&key, sts.as_bytes()));
    let auth = format!(
        "AWS4-HMAC-SHA256 Credential={}/{scope}, SignedHeaders={signed}, Signature={sig}",
        creds.access_key
    );
    (creq, sts, auth)
}

/// The headers that sign `r`: `Authorization`, `x-amz-date` and, with a session token,
/// `x-amz-security-token` (send them with the request, plus the signed ones).
pub fn sign(r: &ToSign<'_>, creds: &Credentials) -> Vec<(String, String)> {
    let (_, _, auth) = sign_parts(r, creds);
    let mut out = vec![
        ("authorization".to_owned(), auth),
        ("x-amz-date".to_owned(), r.amz_date.to_owned()),
    ];
    if let Some(t) = &creds.session_token {
        out.push(("x-amz-security-token".into(), t.expose().to_owned()));
    }
    out
}

/// The common prefixes (sub-folders) and whether more follow, with the token for the next
/// page, from a ListObjectsV2 answer.
pub fn parse_list(xml: &str) -> (Vec<String>, Option<String>) {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<CommonPrefixes>") {
        rest = &rest[i + 16..];
        let end = rest.find("</CommonPrefixes>").unwrap_or(rest.len());
        if let Some(p) = tag(&rest[..end], "Prefix") {
            out.push(xml_unescape(p));
        }
        rest = &rest[end..];
    }
    let more = tag(xml, "IsTruncated").is_some_and(|t| t.trim() == "true");
    let token = more
        .then(|| tag(xml, "NextContinuationToken").map(xml_unescape))
        .flatten();
    (out, token)
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&close)? + a;
    Some(&s[a..b])
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(vars: &[(&str, &str)]) -> MapEnv {
        MapEnv {
            vars: vars
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
            home: None,
        }
    }

    #[test]
    fn parses_s3_addresses() {
        let e = env(&[]);
        let l = S3Location::parse("s3://idr-data/zarr/v0.4/6001240.zarr", &e).unwrap();
        assert_eq!(l.endpoint, "https://idr-data.s3.amazonaws.com");
        assert_eq!(l.prefix, "zarr/v0.4/6001240.zarr/");
        assert!(!l.path_style);
        assert_eq!(
            l.object_url(".zattrs"),
            "https://idr-data.s3.amazonaws.com/zarr/v0.4/6001240.zarr/.zattrs"
        );
        let l = S3Location::parse("https://bk1.s3.us-west-2.amazonaws.com/a/x.zarr/", &e).unwrap();
        assert_eq!(
            (l.bucket.as_str(), l.region.as_str(), l.path_style),
            ("bk1", "us-west-2", false)
        );
        assert_eq!(l.prefix, "a/x.zarr/");
        let l = S3Location::parse(
            "https://s3.eu-central-1.amazonaws.com/my.bucket/x.zarr?x=1",
            &e,
        )
        .unwrap();
        assert_eq!(
            (l.bucket.as_str(), l.region.as_str(), l.path_style),
            ("my.bucket", "eu-central-1", true)
        );
        assert_eq!(
            l.object_url("0/.zarray"),
            "https://s3.eu-central-1.amazonaws.com/my.bucket/x.zarr/0/.zarray"
        );
        let l = S3Location::parse("https://bkt.s3-eu-west-1.amazonaws.com/k", &e).unwrap();
        assert_eq!(l.region, "eu-west-1");
        assert!(S3Location::parse("https://example.org/x.zarr", &e).is_none());
        assert!(S3Location::parse("https://ec2.us-east-1.amazonaws.com/x", &e).is_none());
        // A configured endpoint takes s3:// path-style.
        let e = env(&[
            ("AWS_ENDPOINT_URL", "http://127.0.0.1:9000/"),
            ("AWS_REGION", "eu-west-2"),
        ]);
        let l = S3Location::parse("s3://bucket/data/img.zarr", &e).unwrap();
        assert_eq!(
            l.object_url("zarr.json"),
            "http://127.0.0.1:9000/bucket/data/img.zarr/zarr.json"
        );
        assert_eq!(l.region, "eu-west-2");
        assert_eq!(l.display(), "s3://bucket/data/img.zarr/");
    }

    #[test]
    fn dates_are_utc() {
        let t = UNIX_EPOCH + std::time::Duration::from_secs(1_440_938_160);
        assert_eq!(amz_date(t), "20150830T123600Z");
        assert_eq!(amz_date(UNIX_EPOCH), "19700101T000000Z");
        let t = UNIX_EPOCH + std::time::Duration::from_secs(951_782_400); // 2000-02-29
        assert_eq!(&amz_date(t)[..8], "20000229");
    }

    #[test]
    fn credentials_come_only_from_configuration() {
        let dir = std::env::temp_dir().join(format!("specular-s3-cred-{}", std::process::id()));
        let aws = dir.join(".aws");
        std::fs::create_dir_all(&aws).unwrap();
        std::fs::write(
            aws.join("credentials"),
            "[default]\naws_access_key_id = AKIDDEFAULT\naws_secret_access_key = s1\n\n[work]\naws_access_key_id=AKIDWORK\naws_secret_access_key=s2\naws_session_token = tok\n",
        )
        .unwrap();
        let mut e = env(&[]);
        assert!(Credentials::configured(&e).is_none(), "no home, no keys");
        e.home = Some(dir.clone());
        assert_eq!(
            Credentials::configured(&e).unwrap().access_key,
            "AKIDDEFAULT"
        );
        e.vars.push(("AWS_PROFILE".into(), "work".into()));
        let c = Credentials::configured(&e).unwrap();
        assert_eq!(c.access_key, "AKIDWORK");
        assert_eq!(c.session_token.as_ref().map(Secret::expose), Some("tok"));
        assert!(!format!("{c:?}").contains("s2"), "Debug redacts");
        e.vars.push(("AWS_ACCESS_KEY_ID".into(), "AKIDENV".into()));
        e.vars.push(("AWS_SECRET_ACCESS_KEY".into(), "s3".into()));
        assert_eq!(Credentials::configured(&e).unwrap().access_key, "AKIDENV");
        e.vars.push(("AWS_NO_SIGN_REQUEST".into(), "true".into()));
        assert!(Credentials::configured(&e).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn lists_common_prefixes() {
        let xml = "<ListBucketResult><IsTruncated>true</IsTruncated><CommonPrefixes><Prefix>a/b&amp;c/</Prefix></CommonPrefixes><CommonPrefixes><Prefix>a/d/</Prefix></CommonPrefixes><NextContinuationToken>t1</NextContinuationToken></ListBucketResult>";
        let (p, t) = parse_list(xml);
        assert_eq!(p, ["a/b&c/", "a/d/"]);
        assert_eq!(t.as_deref(), Some("t1"));
    }

    fn creds(key: &str, secret: &str) -> Credentials {
        Credentials {
            access_key: key.into(),
            secret: Secret::new(secret),
            session_token: None,
        }
    }

    fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect()
    }

    /// AWS's published signing-key example (IAM, 2012-02-15).
    #[test]
    fn signing_key_matches_the_aws_example() {
        let k = signing_key(
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "20120215",
            "us-east-1",
            "iam",
        );
        assert_eq!(
            hex(&k),
            "f4780e2d9f65fa895f9c67b32ce1baf0b0d8a43505a000a1a9e090d414db404d"
        );
    }

    /// The SigV4 test suite's `get-vanilla` and `get-vanilla-query-order-key-case`.
    #[test]
    fn signs_the_test_suite_requests() {
        let c = creds("AKIDEXAMPLE", "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY");
        let headers = h(&[
            ("Host", "example.amazonaws.com"),
            ("X-Amz-Date", "20150830T123600Z"),
        ]);
        let r = ToSign {
            method: "GET",
            path: "/",
            query: &[],
            headers: &headers,
            payload_hash: EMPTY_SHA256,
            region: "us-east-1",
            service: "service",
            amz_date: "20150830T123600Z",
        };
        let (creq, sts, auth) = sign_parts(&r, &c);
        assert_eq!(
            creq,
            "GET\n/\n\nhost:example.amazonaws.com\nx-amz-date:20150830T123600Z\n\nhost;x-amz-date\ne3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sts,
            "AWS4-HMAC-SHA256\n20150830T123600Z\n20150830/us-east-1/service/aws4_request\nbb579772317eb040ac9ed261061d46c1f17a8133879d6129b6e1c25292927e63"
        );
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/20150830/us-east-1/service/aws4_request, SignedHeaders=host;x-amz-date, Signature=5fa00fa31553b73ebf1942676e86291e8372ff2a2260956d9b8aae1d763fbf31"
        );
        let q = h(&[("Param2", "value2"), ("Param1", "value1")]);
        let r = ToSign { query: &q, ..r };
        let (_, _, auth) = sign_parts(&r, &c);
        assert!(
            auth.ends_with(
                "Signature=b97d918cfa904a5beff61c982a1b6f458b799221646efd99d3219ec94cdf2500"
            ),
            "{auth}"
        );
    }

    /// The S3 documentation's examples: GET Object with a Range, and List Objects.
    #[test]
    fn signs_the_s3_examples() {
        let c = creds(
            "AKIAIOSFODNN7EXAMPLE",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
        );
        let headers = h(&[
            ("Host", "examplebucket.s3.amazonaws.com"),
            ("Range", "bytes=0-9"),
            ("x-amz-content-sha256", EMPTY_SHA256),
            ("x-amz-date", "20130524T000000Z"),
        ]);
        let r = ToSign {
            method: "GET",
            path: "/test.txt",
            query: &[],
            headers: &headers,
            payload_hash: EMPTY_SHA256,
            region: "us-east-1",
            service: "s3",
            amz_date: "20130524T000000Z",
        };
        let (creq, _, auth) = sign_parts(&r, &c);
        assert_eq!(
            sha256_hex(creq.as_bytes()),
            "7344ae5b7ee6c3e7e6b0fe0640412a37625d1fbfff95c48bbb2dc43964946972"
        );
        assert_eq!(
            auth,
            "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
        );
        let headers = h(&[
            ("Host", "examplebucket.s3.amazonaws.com"),
            ("x-amz-content-sha256", EMPTY_SHA256),
            ("x-amz-date", "20130524T000000Z"),
        ]);
        let q = h(&[("max-keys", "2"), ("prefix", "J")]);
        let r = ToSign {
            path: "/",
            query: &q,
            headers: &headers,
            ..r
        };
        let (_, _, auth) = sign_parts(&r, &c);
        assert!(
            auth.ends_with(
                "Signature=34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"
            ),
            "{auth}"
        );
    }
}
