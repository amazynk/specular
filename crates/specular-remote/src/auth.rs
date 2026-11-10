//! Credentials for remote servers.
//!
//! Passwords and tokens live in [`Secret`]s: never printed by `Debug`, wiped on drop, and never
//! written to disk by this crate. The app has no platform secret store yet, so
//! [`SessionCredentials`] keeps them in memory until the app quits; [`CredentialStore`] is the
//! seam for a keychain (Secret Service, macOS Keychain, Windows Credential Manager) later.

use std::collections::HashMap;
use std::fmt;

use parking_lot::Mutex;

/// A password or token. `Debug` prints `<redacted>`; the bytes are zeroed when dropped.
#[derive(Clone, Default, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(s: impl Into<String>) -> Self {
        Secret(s.into())
    }
    /// The secret itself; only for building a request header.
    pub fn expose(&self) -> &str {
        &self.0
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.0).into_bytes();
        bytes.iter_mut().for_each(|b| *b = 0);
        std::hint::black_box(&bytes);
    }
}

/// How to authenticate to a server.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Auth {
    #[default]
    None,
    /// HTTP Basic (RFC 7617).
    Basic { user: String, password: Secret },
    /// `Authorization: Bearer <token>` (OAuth access tokens, API keys).
    Bearer(Secret),
}

impl Auth {
    /// The `Authorization` header value, if any.
    pub fn header_value(&self) -> Option<Secret> {
        match self {
            Auth::None => None,
            Auth::Basic { user, password } => {
                let mut raw = Vec::with_capacity(user.len() + 1 + password.0.len());
                raw.extend_from_slice(user.as_bytes());
                raw.push(b':');
                raw.extend_from_slice(password.0.as_bytes());
                let v = Secret(format!("Basic {}", base64(&raw)));
                raw.iter_mut().for_each(|b| *b = 0);
                std::hint::black_box(&raw);
                Some(v)
            }
            Auth::Bearer(t) => Some(Secret(format!("Bearer {}", t.0))),
        }
    }
    pub fn kind(&self) -> AuthKind {
        match self {
            Auth::None => AuthKind::None,
            Auth::Basic { .. } => AuthKind::Basic,
            Auth::Bearer(_) => AuthKind::Bearer,
        }
    }
}

/// The kind of credentials a server wants, without the secret (safe to save).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum AuthKind {
    #[default]
    None,
    Basic,
    Bearer,
}

impl AuthKind {
    pub fn id(self) -> &'static str {
        match self {
            AuthKind::None => "none",
            AuthKind::Basic => "basic",
            AuthKind::Bearer => "bearer",
        }
    }
    pub fn from_id(s: &str) -> Option<Self> {
        Some(match s {
            "none" => AuthKind::None,
            "basic" => AuthKind::Basic,
            "bearer" => AuthKind::Bearer,
            _ => return None,
        })
    }
}

/// Where credentials are kept, keyed by a server id.
pub trait CredentialStore: Send + Sync {
    fn load(&self, key: &str) -> Option<Auth>;
    fn store(&self, key: &str, auth: Auth);
    fn forget(&self, key: &str);
    /// Whether credentials survive a restart (a keychain); the UI says so.
    fn persistent(&self) -> bool;
}

/// Credentials kept in memory for this session only.
#[derive(Default)]
pub struct SessionCredentials(Mutex<HashMap<String, Auth>>);

impl fmt::Debug for SessionCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionCredentials")
            .field("servers", &self.0.lock().len())
            .finish()
    }
}

impl CredentialStore for SessionCredentials {
    fn load(&self, key: &str) -> Option<Auth> {
        self.0.lock().get(key).cloned()
    }
    fn store(&self, key: &str, auth: Auth) {
        self.0.lock().insert(key.to_string(), auth);
    }
    fn forget(&self, key: &str) {
        self.0.lock().remove(key);
    }
    fn persistent(&self) -> bool {
        false
    }
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (u32::from(c[0]) << 16)
            | (u32::from(*c.get(1).unwrap_or(&0)) << 8)
            | u32::from(*c.get(2).unwrap_or(&0));
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_header_matches_rfc7617() {
        let a = Auth::Basic {
            user: "Aladdin".into(),
            password: Secret::new("open sesame"),
        };
        assert_eq!(
            a.header_value().unwrap().expose(),
            "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ=="
        );
        assert_eq!(base64(b"a"), "YQ==");
        assert_eq!(base64(b"ab"), "YWI=");
        assert_eq!(base64(b""), "");
    }

    #[test]
    fn secrets_never_print() {
        let a = Auth::Bearer(Secret::new("tok-123"));
        assert!(!format!("{a:?}").contains("tok-123"));
        let b = Auth::Basic {
            user: "u".into(),
            password: Secret::new("hunter2"),
        };
        assert!(!format!("{b:?}").contains("hunter2"));
        let s = SessionCredentials::default();
        s.store("srv", b.clone());
        assert!(!format!("{s:?}").contains("hunter2"));
        assert_eq!(s.load("srv"), Some(b));
        s.forget("srv");
        assert_eq!(s.load("srv"), None);
        assert!(!s.persistent());
    }
}
