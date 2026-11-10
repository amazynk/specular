//! Frames over a Unix stream socket, with descriptors passed as `SCM_RIGHTS` (both sides), and
//! shared-memory payloads (Linux `memfd`).
//!
//! Descriptors ride on the 8 header bytes of a frame; the body is read with plain reads of
//! exactly its length, so a read never crosses into the next frame (where it would drop that
//! frame's descriptors).

use crate::protocol::{self, HEADER_LEN, MAX_FDS};
use rustix::fd::{AsFd, BorrowedFd, OwnedFd};
use rustix::net::{
    RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags, SendAncillaryBuffer,
    SendAncillaryMessage, SendFlags,
};
use std::io::{self, IoSlice, IoSliceMut, Read, Write};
use std::mem::MaybeUninit;
use std::os::unix::net::UnixStream;
use std::time::Duration;

/// The descriptor type that travels with frames.
pub type Fd = OwnedFd;

fn proto(e: protocol::ProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

fn send_flags() -> SendFlags {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        SendFlags::NOSIGNAL
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        SendFlags::empty()
    }
}

fn recv_flags() -> RecvFlags {
    #[cfg(any(target_os = "linux", target_os = "android"))]
    {
        RecvFlags::CMSG_CLOEXEC
    }
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    {
        RecvFlags::empty()
    }
}

/// One end of the viewer ↔ host connection.
pub struct Conn {
    sock: UnixStream,
}

impl Conn {
    pub fn new(sock: UnixStream) -> Conn {
        Conn { sock }
    }

    /// Read and write timeouts for every following call (`None`: block).
    pub fn set_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        let t = t.map(|d| d.max(Duration::from_millis(1)));
        self.sock.set_read_timeout(t)?;
        self.sock.set_write_timeout(t)
    }

    /// Sends one frame with `fds` attached.
    pub fn send(&mut self, body: &[u8], fds: &[BorrowedFd<'_>]) -> io::Result<()> {
        let head = protocol::header(body.len(), fds.len()).map_err(proto)?;
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
            return Err(io::Error::other("descriptor buffer too small"));
        }
        let iov = [IoSlice::new(&head), IoSlice::new(body)];
        let sent = loop {
            match rustix::net::sendmsg(&self.sock, &iov, &mut control, send_flags()) {
                Ok(n) => break n,
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            }
        };
        if sent < HEADER_LEN {
            // The descriptors went with the first byte; the rest of the header follows plainly.
            self.sock.write_all(&head[sent..])?;
            self.sock.write_all(body)
        } else {
            self.sock.write_all(&body[sent - HEADER_LEN..])
        }
    }

    /// Receives one frame: its body and the descriptors that came with it. A closed connection
    /// is `UnexpectedEof`; a timeout is `WouldBlock` or `TimedOut`.
    pub fn recv(&mut self) -> io::Result<(Vec<u8>, Vec<OwnedFd>)> {
        let mut head = [0u8; HEADER_LEN];
        let mut got = 0;
        let mut fds = Vec::new();
        while got < HEADER_LEN {
            let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(MAX_FDS))];
            let mut control = RecvAncillaryBuffer::new(&mut space);
            let mut iov = [IoSliceMut::new(&mut head[got..])];
            let msg = match rustix::net::recvmsg(&self.sock, &mut iov, &mut control, recv_flags()) {
                Ok(m) => m,
                Err(rustix::io::Errno::INTR) => continue,
                Err(e) => return Err(e.into()),
            };
            for m in control.drain() {
                if let RecvAncillaryMessage::ScmRights(it) = m {
                    fds.extend(it);
                }
            }
            if msg.flags.contains(ReturnFlags::CTRUNC) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "too many descriptors in one frame",
                ));
            }
            if msg.bytes == 0 {
                return Err(io::ErrorKind::UnexpectedEof.into());
            }
            got += msg.bytes;
        }
        let (len, nfds) = protocol::parse_header(&head).map_err(proto)?;
        if fds.len() != nfds {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("frame announced {nfds} descriptors, {} arrived", fds.len()),
            ));
        }
        let mut body = Vec::new();
        body.try_reserve_exact(len)
            .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "frame body"))?;
        (&self.sock).take(len as u64).read_to_end(&mut body)?;
        if body.len() != len {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok((body, fds))
    }
}

impl AsFd for Conn {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.sock.as_fd()
    }
}

/// Shared-memory buffers: a sealed `memfd` on Linux. Other Unixes send payloads inline.
pub mod shm {
    use super::*;
    use std::fs::File;
    use std::io::{Seek, SeekFrom};

    /// Whether payloads can travel as descriptors on this platform.
    pub const AVAILABLE: bool = cfg!(any(target_os = "linux", target_os = "android"));

    /// A writable anonymous shared-memory file.
    #[cfg(any(target_os = "linux", target_os = "android"))]
    pub fn create(name: &str) -> io::Result<File> {
        use rustix::fs::{MemfdFlags, memfd_create};
        let fd = memfd_create(name, MemfdFlags::CLOEXEC | MemfdFlags::ALLOW_SEALING)?;
        Ok(File::from(fd))
    }

    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    pub fn create(_name: &str) -> io::Result<File> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "no anonymous shared memory on this platform",
        ))
    }

    /// Makes the buffer immutable (and its size fixed) before it is handed over, so the
    /// receiver can trust its length while reading.
    pub fn seal(f: &File) -> io::Result<()> {
        #[cfg(any(target_os = "linux", target_os = "android"))]
        {
            use rustix::fs::{SealFlags, fcntl_add_seals};
            fcntl_add_seals(
                f,
                SealFlags::SHRINK | SealFlags::GROW | SealFlags::WRITE | SealFlags::SEAL,
            )?;
        }
        let _ = f;
        Ok(())
    }

    /// A sealed shared-memory copy of `bytes`.
    pub fn from_bytes(name: &str, bytes: &[u8]) -> io::Result<File> {
        let mut f = create(name)?;
        f.write_all(bytes)?;
        seal(&f)?;
        Ok(f)
    }

    /// Reads exactly `len` bytes from the start of `fd`, refusing more than `max`.
    pub fn read(fd: OwnedFd, len: u64, max: u64) -> io::Result<Vec<u8>> {
        if len > max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("payload of {len} bytes exceeds {max}"),
            ));
        }
        let mut f = File::from(fd);
        if f.metadata()?.len() < len {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "payload shorter than announced",
            ));
        }
        let len = usize::try_from(len).map_err(|_| io::Error::from(io::ErrorKind::OutOfMemory))?;
        let mut out = Vec::new();
        out.try_reserve_exact(len)
            .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "payload"))?;
        f.seek(SeekFrom::Start(0))?;
        (&mut f).take(len as u64).read_to_end(&mut out)?;
        if out.len() != len {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        Ok(out)
    }

    /// Reads the whole of `fd` (an input file), refusing more than `max` bytes.
    pub fn read_all(fd: OwnedFd, max: u64) -> io::Result<Vec<u8>> {
        let len = File::from(fd.try_clone()?).metadata()?.len();
        read(fd, len, max)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;

    #[test]
    fn frames_and_descriptors() {
        let (a, b) = UnixStream::pair().unwrap();
        let (mut a, mut b) = (Conn::new(a), Conn::new(b));
        let t = std::thread::spawn(move || {
            let mut out = Vec::new();
            for _ in 0..3 {
                out.push(b.recv().unwrap());
            }
            out
        });
        a.send(b"hello", &[]).unwrap();
        let big = vec![0xA5u8; 3 << 20];
        a.send(&big, &[]).unwrap();
        if shm::AVAILABLE {
            let f = shm::from_bytes("test", b"shared bytes").unwrap();
            a.send(b"x", &[f.as_fd()]).unwrap();
        } else {
            a.send(b"x", &[]).unwrap();
        }
        let got = t.join().unwrap();
        assert_eq!(got[0].0, b"hello");
        assert!(got[0].1.is_empty());
        assert_eq!(got[1].0.len(), big.len());
        assert!(got[1].0.iter().all(|&v| v == 0xA5));
        if shm::AVAILABLE {
            let mut fds = got.into_iter().nth(2).unwrap().1;
            assert_eq!(fds.len(), 1);
            let data = shm::read_all(fds.pop().unwrap(), 1 << 20).unwrap();
            assert_eq!(data, b"shared bytes");
        }
    }

    #[test]
    fn eof_and_timeout() {
        let (a, b) = UnixStream::pair().unwrap();
        let mut b = Conn::new(b);
        b.set_timeout(Some(Duration::from_millis(20))).unwrap();
        let e = b.recv().unwrap_err();
        assert!(
            matches!(
                e.kind(),
                io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
            ),
            "{e:?}"
        );
        drop(a);
        assert_eq!(b.recv().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn rejects_bad_header() {
        let (mut a, b) = UnixStream::pair().unwrap();
        let mut b = Conn::new(b);
        a.write_all(&[1, 0, 0, 0, 0, 9, 0, 0, 0]).unwrap();
        assert_eq!(b.recv().unwrap_err().kind(), io::ErrorKind::InvalidData);
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    #[test]
    fn sealed_buffers_are_read_only() {
        let mut f = shm::from_bytes("ro", b"abc").unwrap();
        assert!(f.write_all(b"d").is_err());
        assert!(f.set_len(1).is_err());
        assert!(shm::read_all(OwnedFd::from(f), 2).is_err());
    }
}
