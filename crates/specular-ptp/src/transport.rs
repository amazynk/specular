//! What the protocol needs from the wire: bulk-OUT containers, bulk-IN transfers that end at
//! a short packet, and the interrupt endpoint's events. [`crate::usb`] is the USB one; the
//! tests use [`crate::mock`].

use std::time::Duration;

/// Why a transfer failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TransportError {
    /// Nothing came in time.
    Timeout,
    /// The camera was unplugged or turned off.
    Disconnected,
    /// The camera stalled the endpoint (it reports an error that way).
    Stall,
    /// Anything else, as the system put it.
    Io(String),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransportError::Timeout => write!(f, "the camera did not answer in time"),
            TransportError::Disconnected => write!(f, "the camera was disconnected"),
            TransportError::Stall => write!(f, "the camera stalled the transfer"),
            TransportError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for TransportError {}

/// A PTP transport.
pub trait Transport: Send {
    /// Sends one container as one bulk-OUT transfer. The transport ends it with a
    /// zero-length packet when its length is a multiple of the packet size.
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), TransportError>;

    /// One bulk-IN transfer of at most `max` bytes: it ends at a short packet (a zero-length
    /// one gives an empty vector) or when `max` bytes came. `max` is a multiple of
    /// [`Transport::packet_size`].
    fn receive(&mut self, max: usize, timeout: Duration) -> Result<Vec<u8>, TransportError>;

    /// The next event container from the interrupt endpoint, or `None` when none came within
    /// `timeout` (and always `None` when the camera has no interrupt endpoint).
    fn event(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError>;

    /// The bulk-IN packet size (512 at high speed).
    fn packet_size(&self) -> usize {
        512
    }

    /// The Still Image class Cancel request for transaction `tid` (§5.2.1); the default does
    /// nothing.
    fn cancel(&mut self, _tid: u32) -> Result<(), TransportError> {
        Ok(())
    }

    /// Clears a stall on both bulk endpoints; the default does nothing.
    fn clear_stall(&mut self) -> Result<(), TransportError> {
        Ok(())
    }
}

impl<T: Transport + ?Sized> Transport for Box<T> {
    fn send(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), TransportError> {
        (**self).send(bytes, timeout)
    }

    fn receive(&mut self, max: usize, timeout: Duration) -> Result<Vec<u8>, TransportError> {
        (**self).receive(max, timeout)
    }

    fn event(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>, TransportError> {
        (**self).event(timeout)
    }

    fn packet_size(&self) -> usize {
        (**self).packet_size()
    }

    fn cancel(&mut self, tid: u32) -> Result<(), TransportError> {
        (**self).cancel(tid)
    }

    fn clear_stall(&mut self) -> Result<(), TransportError> {
        (**self).clear_stall()
    }
}
