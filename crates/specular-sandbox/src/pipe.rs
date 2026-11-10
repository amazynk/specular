//! Frames over a pair of byte pipes: the transport on Windows, where the host's stdin and stdout
//! are anonymous pipes to the viewer.
//!
//! The framing is the same as on the Unix socket ([`crate::protocol::header`]), but no handles
//! travel: every payload is inline (frames go up to [`crate::protocol::MAX_FRAME`]).
//!
//! Anonymous pipes have no read or write timeout. A deadline is therefore enforced by a
//! [`Watchdog`] that *kills the peer* when it expires: the host's end of the pipes closes, the
//! blocked call returns, and the call reports [`io::ErrorKind::TimedOut`]. The viewer discards a
//! host that timed out anyway, so killing it early costs nothing.
//!
//! The module is platform-neutral (it only needs `Read` + `Write`), so its tests run on Linux
//! over `std::io::pipe`.

use crate::protocol::{self, HEADER_LEN};
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::time::{Duration, Instant};

/// No descriptors travel over pipes: the descriptor type of this transport has no values.
#[derive(Debug)]
pub enum NoFd {}

fn proto(e: protocol::ProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, e)
}

/// One end of a pipe connection: frames are read from `r` and written to `w`.
pub struct Conn<R, W> {
    r: R,
    w: W,
    timeout: Cell<Option<Duration>>,
    dog: Option<Watchdog>,
}

impl<R: Read, W: Write> Conn<R, W> {
    /// A connection without deadlines (the host's side: it blocks until the viewer writes).
    pub fn new(r: R, w: W) -> Self {
        Conn {
            r,
            w,
            timeout: Cell::new(None),
            dog: None,
        }
    }

    /// A connection whose calls are bounded by [`Conn::set_timeout`]: when one takes longer,
    /// `kill` runs (it must make the blocked read or write return, e.g. by terminating the
    /// peer) and the call fails with `TimedOut`.
    pub fn with_watchdog(r: R, w: W, kill: impl FnMut() + Send + 'static) -> io::Result<Self> {
        Ok(Conn {
            r,
            w,
            timeout: Cell::new(None),
            dog: Some(Watchdog::new(kill)?),
        })
    }

    /// Deadline for each following call (`None`: block). Only enforced with a watchdog.
    pub fn set_timeout(&self, t: Option<Duration>) -> io::Result<()> {
        self.timeout.set(t.map(|d| d.max(Duration::from_millis(1))));
        Ok(())
    }

    /// Sends one frame. Pipes carry no descriptors, so `fds` is always empty.
    pub fn send(&mut self, body: &[u8], fds: &[NoFd]) -> io::Result<()> {
        let head = protocol::header(body.len(), fds.len()).map_err(proto)?;
        let armed = Armed::new(self.dog.as_ref(), self.timeout.get());
        let r = self
            .w
            .write_all(&head)
            .and_then(|()| self.w.write_all(body))
            .and_then(|()| self.w.flush());
        armed.finish(r)
    }

    /// Receives one frame. A closed pipe is `UnexpectedEof`, an expired deadline `TimedOut`.
    pub fn recv(&mut self) -> io::Result<(Vec<u8>, Vec<NoFd>)> {
        let armed = Armed::new(self.dog.as_ref(), self.timeout.get());
        let r = recv_frame(&mut self.r);
        armed.finish(r)
    }
}

/// Reads one frame (without a deadline).
fn recv_frame(r: &mut impl Read) -> io::Result<(Vec<u8>, Vec<NoFd>)> {
    let mut head = [0u8; HEADER_LEN];
    r.read_exact(&mut head)?;
    let (len, nfds) = protocol::parse_header(&head).map_err(proto)?;
    if nfds != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("frame announced {nfds} descriptors over a pipe"),
        ));
    }
    let mut body = Vec::new();
    body.try_reserve_exact(len)
        .map_err(|_| io::Error::new(io::ErrorKind::OutOfMemory, "frame body"))?;
    r.take(len as u64).read_to_end(&mut body)?;
    if body.len() != len {
        return Err(io::ErrorKind::UnexpectedEof.into());
    }
    Ok((body, Vec::new()))
}

/// A deadline armed for one call; disarmed when the call returns.
struct Armed<'a> {
    dog: Option<&'a Watchdog>,
}

impl<'a> Armed<'a> {
    fn new(dog: Option<&'a Watchdog>, timeout: Option<Duration>) -> Self {
        let dog = dog.filter(|_| timeout.is_some());
        if let (Some(d), Some(t)) = (dog, timeout) {
            d.arm(Some(Instant::now() + t));
        }
        Armed { dog }
    }

    /// The call's result, or `TimedOut` if the watchdog fired during it.
    fn finish<T>(self, r: io::Result<T>) -> io::Result<T> {
        match self.dog {
            Some(d) if d.fired() => Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "deadline expired; the peer was killed",
            )),
            _ => r,
        }
    }
}

impl Drop for Armed<'_> {
    fn drop(&mut self) {
        if let Some(d) = self.dog {
            d.arm(None);
        }
    }
}

/// A thread that runs `kill` once a deadline passes without being cleared. One per connection;
/// it ends when the connection is dropped.
pub struct Watchdog {
    tx: mpsc::Sender<Option<Instant>>,
    fired: Arc<AtomicBool>,
}

impl Watchdog {
    pub fn new(mut kill: impl FnMut() + Send + 'static) -> io::Result<Watchdog> {
        let (tx, rx) = mpsc::channel::<Option<Instant>>();
        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        specular_thread::builder("wd-host").spawn(move || {
            let mut deadline: Option<Instant> = None;
            loop {
                let msg = match deadline {
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    Some(d) => rx.recv_timeout(d.saturating_duration_since(Instant::now())),
                };
                match msg {
                    Ok(d) => deadline = d,
                    Err(RecvTimeoutError::Timeout) => {
                        f.store(true, Ordering::SeqCst);
                        kill();
                        deadline = None;
                    }
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })?;
        Ok(Watchdog { tx, fired })
    }

    /// Sets (`Some`) or clears (`None`) the deadline.
    pub fn arm(&self, deadline: Option<Instant>) {
        let _ = self.tx.send(deadline);
    }

    /// Whether the deadline ever passed (the peer was killed).
    pub fn fired(&self) -> bool {
        self.fired.load(Ordering::SeqCst)
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    use super::*;
    use std::io::{PipeReader, PipeWriter};
    use std::sync::Mutex;

    type PipeConn = Conn<PipeReader, PipeWriter>;

    /// Two connected ends, like the viewer and the host.
    fn pair() -> (PipeConn, PipeConn) {
        let (ar, bw) = std::io::pipe().unwrap();
        let (br, aw) = std::io::pipe().unwrap();
        (Conn::new(ar, aw), Conn::new(br, bw))
    }

    #[test]
    fn frames_round_trip() {
        let (mut a, mut b) = pair();
        let t = std::thread::spawn(move || {
            let mut out = Vec::new();
            for _ in 0..3 {
                let (body, fds) = b.recv().unwrap();
                assert!(fds.is_empty());
                b.send(&body, &[]).unwrap();
                out.push(body);
            }
            out
        });
        let big: Vec<u8> = (0..3 << 20).map(|i| (i * 13) as u8).collect();
        for body in [&b"hello"[..], &[], &big] {
            a.send(body, &[]).unwrap();
            assert_eq!(a.recv().unwrap().0, body);
        }
        assert_eq!(t.join().unwrap()[2], big);
    }

    #[test]
    fn eof_and_bad_frames() {
        let (ar, mut bw) = std::io::pipe().unwrap();
        let (_br, aw) = std::io::pipe().unwrap();
        let mut a = Conn::new(ar, aw);
        // Descriptors announced over a pipe, then a bad header, then end of file.
        bw.write_all(&[0, 0, 0, 0, 1, 0, 0, 0]).unwrap();
        assert_eq!(a.recv().unwrap_err().kind(), io::ErrorKind::InvalidData);
        bw.write_all(&[1, 0, 0, 0, 0, 9, 0, 0]).unwrap();
        assert_eq!(a.recv().unwrap_err().kind(), io::ErrorKind::InvalidData);
        bw.write_all(&[5, 0, 0, 0, 0, 0, 0, 0, b'a']).unwrap();
        drop(bw);
        assert_eq!(a.recv().unwrap_err().kind(), io::ErrorKind::UnexpectedEof);
    }

    /// The watchdog's kill closes the peer's write end, which is what terminating the host
    /// does on Windows.
    #[test]
    fn deadline_kills_the_peer() {
        let (ar, bw) = std::io::pipe().unwrap();
        let (_br, aw) = std::io::pipe().unwrap();
        let peer = Arc::new(Mutex::new(Some(bw)));
        let p = peer.clone();
        let kills = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let k = kills.clone();
        let mut a = Conn::with_watchdog(ar, aw, move || {
            k.fetch_add(1, Ordering::SeqCst);
            p.lock().unwrap().take();
        })
        .unwrap();
        // Without a timeout nothing is armed: a frame that arrives in time is received.
        peer.lock()
            .unwrap()
            .as_mut()
            .unwrap()
            .write_all(&[2, 0, 0, 0, 0, 0, 0, 0, 7, 8])
            .unwrap();
        a.set_timeout(Some(Duration::from_secs(5))).unwrap();
        assert_eq!(a.recv().unwrap().0, [7, 8]);
        // A silent peer is killed once the deadline passes.
        a.set_timeout(Some(Duration::from_millis(100))).unwrap();
        let t0 = Instant::now();
        let e = a.recv().unwrap_err();
        assert_eq!(e.kind(), io::ErrorKind::TimedOut, "{e:?}");
        assert!(t0.elapsed() >= Duration::from_millis(100));
        assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
        assert_eq!(kills.load(Ordering::SeqCst), 1);
        assert!(peer.lock().unwrap().is_none());
    }

    #[test]
    fn disarmed_deadline_never_fires() {
        let kills = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let k = kills.clone();
        let dog = Watchdog::new(move || {
            k.fetch_add(1, Ordering::SeqCst);
        })
        .unwrap();
        dog.arm(Some(Instant::now() + Duration::from_millis(30)));
        dog.arm(None);
        std::thread::sleep(Duration::from_millis(80));
        assert_eq!(kills.load(Ordering::SeqCst), 0);
        assert!(!dog.fired());
        dog.arm(Some(Instant::now() + Duration::from_millis(10)));
        let t0 = Instant::now();
        while !dog.fired() && t0.elapsed() < Duration::from_secs(5) {
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(kills.load(Ordering::SeqCst), 1);
    }
}
