//! An archive password, held in memory only.
//!
//! [`Password`] is cheap to clone (one shared, reference-counted copy) and wipes its bytes when
//! the last clone is dropped. Its `Debug` never shows them. Nothing in this crate writes a
//! password anywhere: it reaches the ZIP decryptor, `sevenz-rust2` and, for RAR, the decode
//! host's pipe, and nothing else.

use std::sync::Arc;
use zeroize::Zeroizing;

/// A password for encrypted entries (or an encrypted 7z/RAR header), as the user typed it.
#[derive(Clone)]
pub struct Password(Arc<Zeroizing<String>>);

impl Password {
    /// Takes `s` over; the caller's `String` is moved, not copied, so no stray copy is left.
    pub fn new(s: String) -> Password {
        Password(Arc::new(Zeroizing::new(s)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The UTF-8 bytes. ZIP passwords are hashed as bytes; tools on current systems use UTF-8
    /// (older Windows tools the OEM code page, which agrees for ASCII passwords).
    pub(crate) fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

impl std::fmt::Debug for Password {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Password(..)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_password() {
        let p = Password::new("hunter2".into());
        assert_eq!(format!("{p:?}"), "Password(..)");
        assert_eq!(p.clone().as_str(), "hunter2");
    }
}
