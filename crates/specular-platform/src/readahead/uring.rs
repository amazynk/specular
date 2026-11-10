//! The io_uring backend of [`super`]: whole-file reads of several files with a few chunk reads in
//! flight at once, so the device sees a queue instead of one fault at a time.
//!
//! The data is thrown away; what matters is that it lands in the page cache, where the decoder's
//! memory map then finds it. The one `unsafe` call is the submission-queue push (UNSAFE.md):
//! the kernel writes into our buffer and reads our descriptor after `push` returns, so both must
//! outlive the operation. [`Ring`] guarantees that by owning the buffers, borrowing the files for
//! the whole of [`Ring::read`], and waiting for every completion before it returns (or leaking
//! the buffers if it cannot).

use std::fs::File;
use std::io;
use std::os::fd::AsRawFd;

use io_uring::{IoUring, Probe, opcode, types};

/// A chunk read in flight: which file, and how many bytes it asked for.
#[derive(Clone, Copy)]
struct Pending {
    file: usize,
    len: u32,
}

pub(super) struct Ring {
    ring: IoUring,
    /// One buffer per slot, allocated once. Never resized or touched by us while its slot is in
    /// flight (`pending[i].is_some()`).
    bufs: Vec<Box<[u8]>>,
    pending: Vec<Option<Pending>>,
    inflight: usize,
    /// A wait failed with reads still in flight: the buffers can't be freed safely any more.
    poisoned: bool,
}

impl Ring {
    /// A ring with `depth` reads of `chunk` bytes in flight. Fails when the kernel has no
    /// io_uring (ENOSYS: before 5.1, or `CONFIG_IO_URING=n`), forbids it (EPERM:
    /// `kernel.io_uring_disabled`, a seccomp filter, a container runtime), can't lock the ring's
    /// memory (ENOMEM before 5.12), or lacks `IORING_OP_READ` (before 5.6).
    pub(super) fn new(depth: usize, chunk: usize) -> io::Result<Ring> {
        let depth = depth.clamp(1, 64);
        let chunk = chunk.clamp(4096, 16 << 20);
        let ring = IoUring::new(depth.next_power_of_two() as u32)?;
        let mut probe = Probe::new();
        ring.submitter().register_probe(&mut probe)?;
        if !probe.is_supported(opcode::Read::CODE) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "io_uring without IORING_OP_READ",
            ));
        }
        Ok(Ring {
            ring,
            bufs: (0..depth)
                .map(|_| vec![0u8; chunk].into_boxed_slice())
                .collect(),
            pending: vec![None; depth],
            inflight: 0,
            poisoned: false,
        })
    }

    /// Reads each of `files` from start to end, in order, into the page cache. Stops issuing
    /// reads when `stop()` says so (a newer request arrived). Returns the bytes read.
    ///
    /// A file that fails to read is skipped. The error is returned only if nothing at all could
    /// be read and the kernel rejected the operation itself (EINVAL/EOPNOTSUPP), so the caller
    /// can fall back to another backend.
    pub(super) fn read(&mut self, files: &[File], stop: &dyn Fn() -> bool) -> io::Result<u64> {
        if self.poisoned {
            return Err(io::Error::other("io_uring ring poisoned"));
        }
        let lens: Vec<u64> = files
            .iter()
            .map(|f| f.metadata().map_or(0, |m| m.len()))
            .collect();
        let mut state = Pump {
            lens,
            failed: vec![false; files.len()],
            file: 0,
            offset: 0,
            bytes: 0,
            rejected: None,
        };
        let result = self.pump(files, &mut state, stop);
        // Every read must be finished before `files` (their descriptors) are given back.
        self.drain(&mut state);
        result?;
        match state.rejected {
            Some(e) if state.bytes == 0 => Err(e),
            _ => Ok(state.bytes),
        }
    }

    fn pump(&mut self, files: &[File], st: &mut Pump, stop: &dyn Fn() -> bool) -> io::Result<()> {
        loop {
            while !stop()
                && let Some(slot) = self.pending.iter().position(Option::is_none)
            {
                let Some((file, offset, len)) = st.next_chunk(self.bufs[slot].len()) else {
                    break;
                };
                self.push(slot, &files[file], file, offset, len)?;
            }
            if self.inflight == 0 {
                return Ok(());
            }
            // Submit what was queued and wait for at least one completion.
            match self.ring.submit_and_wait(1) {
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
            self.reap(st);
        }
    }

    fn push(
        &mut self,
        slot: usize,
        file: &File,
        index: usize,
        offset: u64,
        len: u32,
    ) -> io::Result<()> {
        let buf = &mut self.bufs[slot];
        let entry = opcode::Read::new(types::Fd(file.as_raw_fd()), buf.as_mut_ptr(), len)
            .offset(offset)
            .build()
            .user_data(slot as u64);
        // SAFETY: the read writes at most `len` (≤ `buf.len()`) bytes into `self.bufs[slot]`, a
        // heap buffer owned by `self` that is never resized, moved out or accessed while this
        // slot is pending, and is only freed after the completion was reaped (`read` drains
        // every slot before returning; `Drop` leaks the buffers if it cannot). The descriptor
        // belongs to `file`, borrowed from `read`'s `files` for longer than the operation,
        // since `read` drains before returning.
        #[allow(unsafe_code)]
        let pushed = unsafe { self.ring.submission().push(&entry) };
        // The queue holds at least one entry per slot and is submitted before slots are reused,
        // so it is never full.
        pushed.map_err(|_| io::Error::other("io_uring submission queue full"))?;
        self.pending[slot] = Some(Pending { file: index, len });
        self.inflight += 1;
        Ok(())
    }

    fn reap(&mut self, st: &mut Pump) {
        let done: Vec<(u64, i32)> = self
            .ring
            .completion()
            .map(|c| (c.user_data(), c.result()))
            .collect();
        for (slot, res) in done {
            let Some(p) = self.pending.get_mut(slot as usize).and_then(Option::take) else {
                continue;
            };
            self.inflight -= 1;
            if res < 0 {
                let e = io::Error::from_raw_os_error(-res);
                if matches!(e.raw_os_error(), Some(22 | 95)) {
                    // EINVAL / EOPNOTSUPP: the kernel can't do this read at all.
                    st.rejected.get_or_insert(e);
                }
                st.failed[p.file] = true;
            } else {
                st.bytes += res as u64;
                if (res as u32) < p.len {
                    // Short read: the file shrank. Its tail is not worth a second request.
                    st.failed[p.file] = true;
                }
            }
        }
    }

    /// Waits for every read in flight.
    fn drain(&mut self, st: &mut Pump) {
        while self.inflight > 0 {
            match self.ring.submit_and_wait(1) {
                Ok(_) => self.reap(st),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => {
                    specular_log::warn!(target: specular_log::IO,
                        "io_uring: waiting for {} reads failed: {e}; leaking their buffers",
                        self.inflight);
                    self.poisoned = true;
                    return;
                }
            }
        }
    }
}

impl Drop for Ring {
    fn drop(&mut self) {
        if self.inflight > 0 || self.poisoned {
            // The kernel may still write into these buffers; never free them.
            std::mem::forget(std::mem::take(&mut self.bufs));
        }
    }
}

/// Where the reads have got to.
struct Pump {
    lens: Vec<u64>,
    failed: Vec<bool>,
    file: usize,
    offset: u64,
    bytes: u64,
    rejected: Option<io::Error>,
}

impl Pump {
    /// The next chunk to read: file index, offset and length.
    fn next_chunk(&mut self, chunk: usize) -> Option<(usize, u64, u32)> {
        while self.file < self.lens.len() {
            let len = self.lens[self.file];
            if self.failed[self.file] || self.offset >= len {
                self.file += 1;
                self.offset = 0;
                continue;
            }
            let n = (len - self.offset).min(chunk as u64) as u32;
            let at = (self.file, self.offset, n);
            self.offset += u64::from(n);
            return Some(at);
        }
        None
    }
}
