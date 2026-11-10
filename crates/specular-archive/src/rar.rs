//! RAR / CBR through UnRAR in the sandboxed decode host (DESIGN.md §13).
//!
//! UnRAR's C++ code never parses a file in the viewer process, and the host cannot open files:
//! the viewer opens the archive read-only and passes the open file ([`RarSource`]: a descriptor
//! on Unix, a duplicated handle on Windows). When the bytes did not come from a file that still
//! holds them (an archive inside an archive, the clipboard), a private copy is passed instead.
//! Further volumes of a multi-volume archive are opened on the host's request, next to the
//! first and only under that archive's own name. The host lists the archive when it is opened
//! and extracts one entry per [`Archive::read`](crate::Archive::read) call. Entries are
//! addressed by their position in the host's listing plus their name, which the host checks
//! again.
//!
//! Without a usable host (not shipped, not supported on this platform yet, or built without
//! RAR) opening returns [`Error::NeedsSandbox`]. A host crash or timeout on an archive is
//! reported as a malformed archive; the host restarts for the next request.
//!
//! Encrypted archives (encrypted entries, or encrypted file names with RAR's `-hp`) are read
//! with [`crate::Options::password`], which travels to the host in the request and nowhere
//! else.
//!
//! Solid archives must be decompressed from the start up to the wanted entry, so reading every
//! page of a solid CBR costs O(n²) decompression; non-solid archives seek directly.

use crate::{Entry, Error, Format, Limits, Loc, Method, Password, Result};
use specular_sandbox::{Error as HostError, ErrorKind, Host, ListEntry, RarSource, Secret};
use std::path::Path;

/// RAR major format: 4 (RAR 1.5–4.x) or 5.
pub(crate) fn detect(bytes: &[u8]) -> Option<u8> {
    if bytes.starts_with(b"Rar!\x1a\x07\x01\x00") {
        Some(5)
    } else if bytes.starts_with(b"Rar!\x1a\x07\x00") {
        Some(4)
    } else {
        None
    }
}

/// The RAR format version of `bytes` (4 or 5), for diagnostics.
pub fn rar_version(bytes: &[u8]) -> Option<u8> {
    detect(bytes)
}

/// The archive as the host sees it.
pub(crate) struct Rar {
    input: RarSource,
    /// The host's listing, indexed by [`Loc::Rar`].
    listing: Vec<ListEntry>,
    /// Compressed as one stream: an entry is reached by decompressing all before it.
    pub(crate) solid: bool,
    password: Option<Secret>,
}

/// Most bytes one batch read ([`Rar::read_many`]) brings back from the host at once.
const BATCH_BYTES: u64 = 64 << 20;

fn map_err(e: HostError) -> Error {
    match e {
        HostError::NotInstalled(_) | HostError::Unavailable(_) => Error::NeedsSandbox(Format::Rar),
        HostError::Remote { kind, message } => match kind {
            ErrorKind::NotInstalled => Error::NeedsSandbox(Format::Rar),
            ErrorKind::Encrypted => Error::Encrypted,
            ErrorKind::BadPassword => Error::WrongPassword,
            ErrorKind::TooLarge => Error::Limit(message),
            ErrorKind::Unsupported => Error::Unsupported(message),
            ErrorKind::Io => Error::Io(std::io::Error::other(message)),
            _ => Error::Malformed(message),
        },
        e @ (HostError::Crashed(_) | HostError::Timeout(_) | HostError::Protocol(_)) => {
            Error::Malformed(e.to_string())
        }
        HostError::Io(m) => Error::Io(std::io::Error::other(m)),
    }
}

/// The file at `path` if it still holds `bytes`, else a private copy of `bytes`.
fn source(bytes: &[u8], path: Option<&Path>) -> Result<RarSource> {
    if let Some(p) = path
        && let Ok(s) = RarSource::open(p)
        && s.same_content(bytes)
    {
        return Ok(s);
    }
    RarSource::from_bytes(bytes).map_err(map_err)
}

/// Lists the archive in the host. Folders are dropped; the remaining entries keep their index
/// into the host's listing. `path` is the file `bytes` came from, if any.
pub(crate) fn open(
    bytes: &[u8],
    path: Option<&Path>,
    limits: &Limits,
    password: Option<&Password>,
) -> Result<(Vec<Entry>, Rar)> {
    let max = u32::try_from(limits.max_entries).unwrap_or(u32::MAX);
    let input = source(bytes, path)?;
    let password = password.map(|p| Secret::new(p.as_str().to_owned()));
    let listing = Host::global()
        .rar_list(&input, max, password.as_ref())
        .map_err(map_err)?;
    let entries = listing
        .entries
        .iter()
        .enumerate()
        .filter(|(_, e)| !e.directory)
        .map(|(i, e)| Entry {
            name: e.name.trim_start_matches(['/', '\\']).replace('\\', "/"),
            size: e.size,
            compressed_size: 0,
            method: Method::Other(if listing.solid {
                "RAR (solid)".into()
            } else {
                "RAR".into()
            }),
            encrypted: e.encrypted,
            loc: Loc::Rar { index: i as u32 },
        })
        .collect();
    Ok((
        entries,
        Rar {
            input,
            listing: listing.entries,
            solid: listing.solid,
            password,
        },
    ))
}

impl Rar {
    pub(crate) fn read(
        &self,
        index: u32,
        e: &Entry,
        limits: &Limits,
        out: &mut Vec<u8>,
    ) -> Result<()> {
        let le = self
            .listing
            .get(index as usize)
            .ok_or_else(|| Error::Malformed("RAR entry index out of range".into()))?;
        // The compressed size is not listed, so only the size limit applies.
        limits.check_entry(e.size, e.size)?;
        let data = Host::global()
            .rar_read(
                &self.input,
                index,
                le,
                limits.max_entry_size,
                self.password.as_ref(),
            )
            .map_err(map_err)?;
        if data.len() as u64 != e.size {
            return Err(Error::Malformed(format!(
                "RAR entry is {} bytes, {} declared",
                data.len(),
                e.size
            )));
        }
        *out = data;
        Ok(())
    }

    /// Reads `items` (listing index and entry, ascending) in as few passes over the archive as
    /// [`BATCH_BYTES`] allows, handing each to `each` with its position in `items`; stops when
    /// `each` returns false. When a batch fails, its entries are read one by one, so each gets
    /// its own result.
    pub(crate) fn read_many(
        &self,
        items: &[(u32, &Entry)],
        limits: &Limits,
        each: &mut dyn FnMut(usize, Result<Vec<u8>>) -> bool,
    ) {
        let mut start = 0;
        while start < items.len() {
            // As many as fit the byte budget (at least one), each within the limits.
            let mut end = start;
            let mut bytes = 0u64;
            while end < items.len()
                && (end == start || bytes + items[end].1.size <= BATCH_BYTES)
                && limits
                    .check_entry(items[end].1.size, items[end].1.size)
                    .is_ok()
                && self.listing.get(items[end].0 as usize).is_some()
            {
                bytes += items[end].1.size;
                end += 1;
            }
            if end == start {
                // Refused or unknown: its own read says why.
                let (index, e) = items[start];
                let mut out = Vec::new();
                let r = self.read(index, e, limits, &mut out).map(|()| out);
                if !each(start, r) {
                    return;
                }
                start += 1;
                continue;
            }
            let batch: Vec<(u32, &ListEntry)> = items[start..end]
                .iter()
                .map(|&(i, _)| (i, &self.listing[i as usize]))
                .collect();
            let read = Host::global().rar_read_many(
                &self.input,
                &batch,
                limits.max_entry_size,
                bytes.max(1),
                self.password.as_ref(),
            );
            match read {
                Ok(all) if all.len() == end - start => {
                    for (k, data) in all.into_iter().enumerate() {
                        let e = items[start + k].1;
                        let r = if data.len() as u64 == e.size {
                            Ok(data)
                        } else {
                            Err(Error::Malformed(format!(
                                "RAR entry is {} bytes, {} declared",
                                data.len(),
                                e.size
                            )))
                        };
                        if !each(start + k, r) {
                            return;
                        }
                    }
                }
                _ => {
                    for (k, &(index, e)) in items[start..end].iter().enumerate() {
                        let mut out = Vec::new();
                        let r = self.read(index, e, limits, &mut out).map(|()| out);
                        if !each(start + k, r) {
                            return;
                        }
                    }
                }
            }
            start = end;
        }
    }
}
