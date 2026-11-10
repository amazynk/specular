use crate::Format;

/// Why an archive or entry could not be opened or read. Malformed input always ends here; the
/// crate never panics on it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("not a recognised archive")]
    NotAnArchive,
    #[error("malformed archive: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// The format is recognised but may only be decoded in the sandboxed decode host
    /// (DESIGN.md §13): RAR.
    #[error("{0} archives are decoded only in the sandbox host")]
    NeedsSandbox(Format),
    /// The entry (or a 7z/RAR file list) is encrypted and no password was given.
    #[error("entry is encrypted")]
    Encrypted,
    /// The password given does not decrypt the entry (or the encrypted file list).
    #[error("wrong password")]
    WrongPassword,
    /// A size, ratio, count or memory limit from [`crate::Limits`] was exceeded.
    #[error("limit exceeded: {0}")]
    Limit(String),
    #[error("checksum mismatch")]
    Checksum,
    #[error("no entry {0}")]
    NoEntry(usize),
}
