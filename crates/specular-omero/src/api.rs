//! A connection to an OMERO.web server: API discovery, sign-in, cookies, and requests that
//! sign in again once when the session has ended.
//!
//! - **Discovery.** `/api/` lists the JSON API versions; version 0's root lists the URLs of
//!   the token, server list, login and container endpoints. They are kept as paths on the
//!   server we were given (a proxy's absolute URLs may name another scheme or host).
//! - **Sign-in.** `token/` gives the CSRF token. Django checks it by "double submit" (the
//!   `csrftoken` cookie must equal the `X-CSRFToken` header), and OMERO.web does not always
//!   set that cookie, so we set it ourselves when the server did not. `login/` is a form POST
//!   of the server id, user name and password, with a `Referer` (Django wants one over
//!   HTTPS). The answer's session cookie may have any name (`sessionid`, IDR's
//!   `sessionid_prod133`): every cookie the server sets is kept and sent back.
//! - **Public browsing.** Without credentials requests are made as they are: a server with a
//!   public user signs it in on the first request (or after visiting the web client, which
//!   is tried when a request is refused).
//! - **Expiry.** A request answered with a redirect to the sign-in page, 401, 403 or an HTML
//!   page is retried once after signing in again with the credentials kept in memory (never
//!   on disk). Without them, or when they are refused, the request fails with
//!   [`OmeroError::Expired`] and the application asks for the password.
//!
//! Every request has a timeout (the client's), a size limit, and no redirects followed.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use parking_lot::Mutex;
use serde_json::Value;
use specular_remote::{CancelToken, Client, Error as HttpError, Request, Response, Secret};

/// The largest JSON answer read (a page of a listing, image metadata).
pub const MAX_JSON: u64 = 32 << 20;
/// The largest rendered image read (a tile, a whole small image, a thumbnail).
pub const MAX_IMAGE: u64 = 96 << 20;

/// What went wrong talking to an OMERO server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OmeroError {
    Cancelled,
    /// The server could not be reached (offline, wrong address, timeout, TLS).
    Offline(String),
    /// The server wants a sign-in: it has no public browsing.
    SignInRequired,
    /// The session ended and could not be renewed: the password is needed again.
    Expired,
    /// The user name or password was refused.
    LoginRefused,
    /// 403: the signed-in user may not see this.
    Forbidden,
    /// 404: no such object (or not visible to this user).
    NotFound,
    /// Another HTTP error status.
    Status(u16),
    /// Not an OMERO.web server, or an answer that cannot be read.
    Malformed(String),
    /// The no-network switch is on: nothing was sent.
    NetworkOff,
}

impl OmeroError {
    /// Signing in (again) may help.
    pub fn needs_sign_in(&self) -> bool {
        matches!(
            self,
            OmeroError::SignInRequired | OmeroError::Expired | OmeroError::LoginRefused
        )
    }
}

impl fmt::Display for OmeroError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OmeroError::Cancelled => write!(f, "cancelled"),
            OmeroError::Offline(m) => write!(f, "OMERO server unreachable: {m}"),
            OmeroError::SignInRequired => write!(f, "OMERO server requires signing in"),
            OmeroError::Expired => write!(f, "OMERO session expired: sign in again"),
            OmeroError::LoginRefused => write!(f, "OMERO sign-in refused"),
            OmeroError::Forbidden => write!(f, "OMERO: access denied (403)"),
            OmeroError::NotFound => write!(f, "OMERO: not found (404)"),
            OmeroError::Status(s) => write!(f, "OMERO: HTTP {s}"),
            OmeroError::Malformed(m) => write!(f, "OMERO: {m}"),
            OmeroError::NetworkOff => f.write_str(specular_core::net::off::MESSAGE),
        }
    }
}

impl std::error::Error for OmeroError {}

impl From<HttpError> for OmeroError {
    fn from(e: HttpError) -> Self {
        match e {
            HttpError::Cancelled => OmeroError::Cancelled,
            HttpError::Status(404 | 410) => OmeroError::NotFound,
            HttpError::Status(403) => OmeroError::Forbidden,
            HttpError::Status(401 | 300..=399) => OmeroError::SignInRequired,
            HttpError::Status(s) => OmeroError::Status(s),
            HttpError::Timeout => OmeroError::Offline("timed out".into()),
            HttpError::Connect(m) | HttpError::Tls(m) => OmeroError::Offline(m),
            HttpError::Io(e) => OmeroError::Offline(e.to_string()),
            HttpError::Protocol(m) => OmeroError::Malformed(m),
            HttpError::NetworkOff => OmeroError::NetworkOff,
        }
    }
}

pub type Result<T> = std::result::Result<T, OmeroError>;

/// The API's endpoints, as paths on the server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Urls {
    pub token: String,
    pub servers: String,
    pub login: String,
    pub projects: String,
    pub datasets: String,
    pub images: String,
    pub screens: String,
    pub plates: String,
}

/// Who the session belongs to, from the login answer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EventContext {
    pub user_id: i64,
    pub user_name: String,
    pub group_id: i64,
    pub group_name: String,
}

#[derive(Default)]
struct State {
    cookies: Vec<(String, String)>,
    csrf: Option<String>,
    urls: Option<Urls>,
    event: Option<EventContext>,
    /// The user name the connection was made for (kept when credentials are dropped, to ask
    /// for the right password).
    user: Option<String>,
}

/// A connection to one OMERO.web server; share it between threads.
pub struct Omero {
    client: Client,
    /// `https://host[:port][/prefix]`, no trailing `/`.
    base: String,
    state: Mutex<State>,
    /// In memory only; zeroed when dropped.
    credentials: Mutex<Option<(String, Secret)>>,
    /// Serialises signing in again; `generation` counts sessions so that requests that failed
    /// under an old one retry without another login.
    renew: Mutex<()>,
    generation: AtomicU32,
    logins: AtomicU32,
}

impl fmt::Debug for Omero {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Omero")
            .field("base", &self.base)
            .field("user", &self.user())
            .finish_non_exhaustive()
    }
}

/// Checks and normalises a server address: `http(s)://host[:port][/prefix]`, trimmed, no
/// trailing `/`, no credentials, query or fragment.
pub fn check_url(s: &str) -> Option<String> {
    let s = s.trim().trim_end_matches('/');
    let (scheme, rest) = s.split_once("://")?;
    if !scheme.eq_ignore_ascii_case("http") && !scheme.eq_ignore_ascii_case("https") {
        return None;
    }
    let authority = rest.split('/').next().unwrap_or("");
    if authority.is_empty()
        || authority.contains('@')
        || rest.contains(['?', '#'])
        || rest.contains(char::is_whitespace)
    {
        return None;
    }
    Some(format!("{}://{rest}", scheme.to_ascii_lowercase()))
}

/// `scheme://host[:port]` of a URL.
fn origin(url: &str) -> &str {
    let start = url.find("://").map_or(0, |i| i + 3);
    let end = url[start..].find('/').map_or(url.len(), |i| start + i);
    &url[..end]
}

/// The path (and query) of an absolute URL, or the string itself.
fn path_of(url: &str) -> &str {
    if url.contains("://") {
        let o = origin(url).len();
        if o == url.len() { "/" } else { &url[o..] }
    } else {
        url
    }
}

/// Percent-encodes a form or query value.
pub fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~') {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

impl Omero {
    /// A connection to `base` (see [`check_url`]); nothing is sent yet.
    pub fn new(client: Client, base: &str) -> Result<Arc<Omero>> {
        let base = check_url(base).ok_or_else(|| OmeroError::Malformed("bad address".into()))?;
        Ok(Arc::new(Omero {
            client,
            base,
            state: Mutex::new(State::default()),
            credentials: Mutex::new(None),
            renew: Mutex::new(()),
            generation: AtomicU32::new(0),
            logins: AtomicU32::new(0),
        }))
    }

    pub fn base(&self) -> &str {
        &self.base
    }

    /// The user signed in (or to sign in), `None` for public browsing.
    pub fn user(&self) -> Option<String> {
        let st = self.state.lock();
        st.event
            .as_ref()
            .map(|e| e.user_name.clone())
            .filter(|u| !u.is_empty())
            .or_else(|| st.user.clone())
    }

    pub fn event_context(&self) -> Option<EventContext> {
        self.state.lock().event.clone()
    }

    /// Logins made so far (tests).
    pub fn logins(&self) -> u32 {
        self.logins.load(Ordering::Relaxed)
    }

    /// Drops the password kept for signing in again (the session stays until it expires).
    pub fn forget_password(&self) {
        *self.credentials.lock() = None;
    }

    /// Ends the session here: cookies and password dropped.
    pub fn sign_out(&self) {
        self.forget_password();
        let mut st = self.state.lock();
        st.cookies.clear();
        st.event = None;
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Discovers the API and, with `login`, signs in (keeping the password in memory to sign
    /// in again when the session expires). Without, browses as the public user.
    pub fn connect(&self, login: Option<(&str, Secret)>, cancel: &CancelToken) -> Result<()> {
        self.discover(cancel)?;
        match login {
            Some((user, password)) => {
                let user = user.trim().to_string();
                self.state.lock().user = Some(user.clone());
                self.login(&user, &password, cancel)?;
                *self.credentials.lock() = Some((user, password));
            }
            None => {
                self.state.lock().user = None;
            }
        }
        Ok(())
    }

    /// The API's endpoints, discovered on first use.
    pub fn urls(&self, cancel: &CancelToken) -> Result<Urls> {
        if let Some(u) = self.state.lock().urls.clone() {
            return Ok(u);
        }
        self.discover(cancel)
    }

    fn discover(&self, cancel: &CancelToken) -> Result<Urls> {
        let api = self.plain_json(&format!("{}/api/", self.base), cancel)?;
        let versions = api
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| OmeroError::Malformed("not an OMERO.web server (no /api/)".into()))?;
        let v0 = versions
            .iter()
            .find(|v| v.get("version").and_then(Value::as_str) == Some("0"))
            .or_else(|| versions.last())
            .and_then(|v| v.get("url:base").and_then(Value::as_str))
            .ok_or_else(|| OmeroError::Malformed("no API version 0".into()))?;
        let root = self.plain_json(&self.resolve(v0), cancel)?;
        let get = |k: &str, default: &str| -> String {
            root.get(k).and_then(Value::as_str).map_or_else(
                || format!("{}{default}", path_of(v0)),
                |u| path_of(u).into(),
            )
        };
        let urls = Urls {
            token: get("url:token", "token/"),
            servers: get("url:servers", "servers/"),
            login: get("url:login", "login/"),
            projects: get("url:projects", "m/projects/"),
            datasets: get("url:datasets", "m/datasets/"),
            images: get("url:images", "m/images/"),
            screens: get("url:screens", "m/screens/"),
            plates: get("url:plates", "m/plates/"),
        };
        self.state.lock().urls = Some(urls.clone());
        Ok(urls)
    }

    /// An absolute URL for a path the API handed out (absolute or not), or one of ours
    /// (`/webgateway/…`, resolved under the base and its prefix).
    pub fn resolve(&self, url: &str) -> String {
        if url.contains("://") {
            format!("{}{}", origin(&self.base), path_of(url))
        } else if url.starts_with('/') && self.has_prefix(url) {
            format!("{}{url}", origin(&self.base))
        } else {
            format!("{}{url}", self.base)
        }
    }

    /// Whether `path` already starts with the base's path prefix (an API path).
    fn has_prefix(&self, path: &str) -> bool {
        let prefix = &self.base[origin(&self.base).len()..];
        !prefix.is_empty() && path.starts_with(&format!("{prefix}/"))
    }

    fn login(&self, user: &str, password: &Secret, cancel: &CancelToken) -> Result<()> {
        let urls = self.urls(cancel)?;
        // The server to sign in to: the first one listed (OMERO.web usually knows one).
        let server = self
            .plain_json(&self.resolve(&urls.servers), cancel)
            .ok()
            .and_then(|v| v.get("data")?.as_array()?.first()?.get("id")?.as_i64())
            .unwrap_or(1);
        let token = self.plain_json(&self.resolve(&urls.token), cancel)?;
        let token = token
            .get("data")
            .and_then(Value::as_str)
            .filter(|t| !t.is_empty() && t.len() <= 256)
            .ok_or_else(|| OmeroError::Malformed("no CSRF token".into()))?
            .to_string();
        {
            let mut st = self.state.lock();
            if !st.cookies.iter().any(|(k, _)| k == "csrftoken") {
                st.cookies.push(("csrftoken".into(), token.clone()));
            }
            st.csrf = Some(token);
        }
        let body = format!(
            "server={server}&username={}&password={}",
            encode_component(user),
            encode_component(password.expose())
        );
        let req = self.decorate(
            Request::post(
                self.resolve(&urls.login),
                "application/x-www-form-urlencoded",
                body.into_bytes(),
            )
            .accept("application/json"),
        );
        self.logins.fetch_add(1, Ordering::Relaxed);
        let resp = match self.client.post(&req, cancel) {
            Ok(r) => r,
            Err(HttpError::Status(400 | 401 | 403)) => return Err(OmeroError::LoginRefused),
            Err(e) => return Err(e.into()),
        };
        self.take_cookies(&resp);
        let v: Value = serde_json::from_slice(&resp.body)
            .map_err(|_| OmeroError::Malformed("login answer is not JSON".into()))?;
        if v.get("success").and_then(Value::as_bool) != Some(true) {
            return Err(OmeroError::LoginRefused);
        }
        let ec = v.get("eventContext");
        let s = |k: &str| {
            ec.and_then(|e| e.get(k))
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };
        let i = |k: &str| {
            ec.and_then(|e| e.get(k))
                .and_then(Value::as_i64)
                .unwrap_or(-1)
        };
        let event = EventContext {
            user_id: i("userId"),
            user_name: if s("userName").is_empty() {
                user.to_string()
            } else {
                s("userName")
            },
            group_id: i("groupId"),
            group_name: s("groupName"),
        };
        self.state.lock().event = Some(event);
        self.generation.fetch_add(1, Ordering::AcqRel);
        Ok(())
    }

    /// Adds the session's cookies, CSRF token and a `Referer`; never follows redirects.
    fn decorate(&self, mut req: Request) -> Request {
        let st = self.state.lock();
        if !st.cookies.is_empty() {
            let c: Vec<String> = st.cookies.iter().map(|(k, v)| format!("{k}={v}")).collect();
            req = req.header("Cookie", c.join("; "));
        }
        if let Some(t) = &st.csrf {
            req = req.header("X-CSRFToken", t.clone());
        }
        req.header("Referer", format!("{}/", self.base))
            .no_redirects()
    }

    /// Keeps every cookie the server sets; an empty value (or `Max-Age=0`) removes one.
    fn take_cookies(&self, resp: &Response) {
        let mut st = self.state.lock();
        for (k, v) in &resp.headers.0 {
            if k != "set-cookie" {
                continue;
            }
            let mut parts = v.split(';');
            let Some((name, value)) = parts.next().and_then(|p| p.split_once('=')) else {
                continue;
            };
            let (name, value) = (name.trim(), value.trim().trim_matches('"'));
            if name.is_empty() || name.len() > 128 || value.len() > 4096 {
                continue;
            }
            let gone =
                value.is_empty() || parts.any(|p| p.trim().eq_ignore_ascii_case("max-age=0"));
            st.cookies.retain(|(k, _)| k != name);
            if !gone {
                st.cookies.push((name.to_string(), value.to_string()));
            }
            if name == "csrftoken" && !gone {
                st.csrf = Some(value.to_string());
            }
        }
    }

    /// A GET without sign-in handling (discovery, token): JSON or an error.
    fn plain_json(&self, url: &str, cancel: &CancelToken) -> Result<Value> {
        let req = self.decorate(Request::get(url).accept("application/json").limit(MAX_JSON));
        let resp = self.client.get(&req, cancel)?;
        self.take_cookies(&resp);
        parse_json(&resp)
    }

    /// GET `url` (absolute or a path, see [`Omero::resolve`]) as JSON, signing in again once
    /// if the session has ended.
    pub fn get_json(&self, url: &str, cancel: &CancelToken) -> Result<Value> {
        let url = self.resolve(url);
        let resp = self.call(cancel, true, || {
            Request::get(url.as_str())
                .accept("application/json")
                .limit(MAX_JSON)
        })?;
        parse_json(&resp)
    }

    /// GET `url` as bytes (an image), signing in again once if the session has ended.
    pub fn get_bytes(&self, url: &str, cancel: &CancelToken) -> Result<Vec<u8>> {
        let url = self.resolve(url);
        let resp = self.call(cancel, false, || {
            Request::get(url.as_str())
                .accept("image/jpeg, image/png, image/*")
                .limit(MAX_IMAGE)
                .activity(specular_core::net::What::ImageTiles)
        })?;
        Ok(resp.body)
    }

    fn call(
        &self,
        cancel: &CancelToken,
        json: bool,
        make: impl Fn() -> Request,
    ) -> Result<Response> {
        let generation = self.generation.load(Ordering::Acquire);
        match self.send(cancel, json, &make) {
            Err(Fail::Session) => {}
            Err(Fail::Other(e)) => return Err(e),
            Ok(r) => return Ok(r),
        }
        self.renew_session(generation, cancel)?;
        match self.send(cancel, json, &make) {
            Ok(r) => Ok(r),
            Err(Fail::Other(e)) => Err(e),
            // Signed in afresh and still refused: not a session problem.
            Err(Fail::Session) => Err(if self.credentials.lock().is_some() {
                OmeroError::Forbidden
            } else {
                OmeroError::SignInRequired
            }),
        }
    }

    fn send(
        &self,
        cancel: &CancelToken,
        json: bool,
        make: &impl Fn() -> Request,
    ) -> std::result::Result<Response, Fail> {
        let req = self.decorate(make());
        match self.client.get(&req, cancel) {
            Ok(resp) => {
                self.take_cookies(&resp);
                // A sign-in page served in place of the answer.
                let html = resp
                    .headers
                    .content_type()
                    .is_some_and(|c| c.to_ascii_lowercase().starts_with("text/html"));
                if html && (json || resp.body.starts_with(b"<")) {
                    return Err(Fail::Session);
                }
                Ok(resp)
            }
            Err(HttpError::Status(401 | 403 | 300..=399)) => Err(Fail::Session),
            Err(e) => Err(Fail::Other(e.into())),
        }
    }

    /// Signs in again (or starts a public session) unless another request did since
    /// `generation`.
    fn renew_session(&self, generation: u32, cancel: &CancelToken) -> Result<()> {
        let _one = self.renew.lock();
        if self.generation.load(Ordering::Acquire) != generation {
            return Ok(());
        }
        let creds = self
            .credentials
            .lock()
            .as_ref()
            .map(|(u, p)| (u.clone(), Secret::new(p.expose())));
        let wanted_user = self.state.lock().user.clone();
        match (creds, wanted_user) {
            (Some((user, password)), _) => {
                specular_log::info!(target: specular_log::IO, "omero: session ended, signing in again to {}", self.base);
                self.state.lock().cookies.retain(|(k, _)| k == "csrftoken");
                match self.login(&user, &password, cancel) {
                    Ok(()) => Ok(()),
                    Err(OmeroError::LoginRefused) => {
                        self.forget_password();
                        Err(OmeroError::Expired)
                    }
                    Err(e) => Err(e),
                }
            }
            // Signed in before, password no longer at hand.
            (None, Some(_)) => Err(OmeroError::Expired),
            // A public session: the web client signs the public user in (a server without
            // one redirects to its sign-in page). Each request retries once, so this cannot
            // loop.
            (None, None) => {
                let req = self.decorate(
                    Request::get(format!("{}/webclient/?experimenter=-1", self.base))
                        .limit(MAX_JSON),
                );
                match self.client.get(&req, cancel) {
                    Ok(resp) => {
                        self.take_cookies(&resp);
                        self.generation.fetch_add(1, Ordering::AcqRel);
                        Ok(())
                    }
                    Err(HttpError::Status(401 | 403 | 300..=399)) => {
                        Err(OmeroError::SignInRequired)
                    }
                    Err(e) => Err(e.into()),
                }
            }
        }
    }
}

enum Fail {
    /// Refused for want of a session.
    Session,
    Other(OmeroError),
}

fn parse_json(resp: &Response) -> Result<Value> {
    serde_json::from_slice(&resp.body)
        .map_err(|e| OmeroError::Malformed(format!("answer is not JSON: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_checked() {
        assert_eq!(
            check_url(" HTTPS://idr.openmicroscopy.org/ ").as_deref(),
            Some("https://idr.openmicroscopy.org")
        );
        assert_eq!(
            check_url("http://h:4080/omero").as_deref(),
            Some("http://h:4080/omero")
        );
        assert_eq!(check_url("ftp://h"), None);
        assert_eq!(check_url("https://u:p@h"), None);
        assert_eq!(check_url("https://h/?x"), None);
        assert_eq!(check_url("h"), None);
    }

    #[test]
    fn resolves_api_and_own_paths() {
        let c = Client::new(specular_remote::ClientConfig::for_tests()).unwrap();
        let o = Omero::new(c.clone(), "https://h.example/omero").unwrap();
        assert_eq!(
            o.resolve("http://internal:8080/omero/api/v0/m/projects/"),
            "https://h.example/omero/api/v0/m/projects/"
        );
        assert_eq!(
            o.resolve("/omero/api/v0/token/"),
            "https://h.example/omero/api/v0/token/"
        );
        assert_eq!(
            o.resolve("/webgateway/imgData/1/"),
            "https://h.example/omero/webgateway/imgData/1/"
        );
        let o = Omero::new(c, "https://h.example").unwrap();
        assert_eq!(o.resolve("/api/"), "https://h.example/api/");
    }

    #[test]
    fn form_values_are_encoded() {
        assert_eq!(encode_component("a b&c=d/é"), "a%20b%26c%3Dd%2F%C3%A9");
    }
}
