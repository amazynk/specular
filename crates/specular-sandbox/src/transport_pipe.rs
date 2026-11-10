//! The transport where no descriptors travel (Windows): frames go over the pipes of
//! [`crate::pipe`] and every payload is inline. This module mirrors the Unix one's names so the
//! code above it is shared.

/// No descriptors travel: the descriptor type has no values.
pub use crate::pipe::NoFd as Fd;

/// Shared memory: none, so payloads go inline.
pub mod shm {
    use super::Fd;
    use std::fs::File;
    use std::io;

    /// Whether payloads can travel as descriptors on this platform.
    pub const AVAILABLE: bool = false;

    pub fn create(_name: &str) -> io::Result<File> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no descriptor passing on this platform",
        ))
    }

    pub fn seal(_f: &File) -> io::Result<()> {
        Ok(())
    }

    pub fn from_bytes(name: &str, _bytes: &[u8]) -> io::Result<File> {
        create(name)
    }

    pub fn read(fd: Fd, _len: u64, _max: u64) -> io::Result<Vec<u8>> {
        match fd {}
    }

    pub fn read_all(fd: Fd, _max: u64) -> io::Result<Vec<u8>> {
        match fd {}
    }
}
