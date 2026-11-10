//! Raw Deflate (RFC 1951) behind a small trait, so the zlib-rs backend can be replaced by
//! Specular's own inflater (being written for PNG) without touching the ZIP code.

/// Inflates one complete raw Deflate stream whose output size is known in advance.
pub(crate) trait Inflater: Sync {
    /// Inflates `input` into `out`, which is exactly the declared size. Returns the number of
    /// bytes written; an error if the stream is corrupt, truncated, or would produce more than
    /// `out.len()` bytes (so a lying size can never make it allocate or write more).
    fn inflate(&self, input: &[u8], out: &mut [u8]) -> Result<usize, &'static str>;
}

pub(crate) struct ZlibRs;

impl Inflater for ZlibRs {
    fn inflate(&self, input: &[u8], out: &mut [u8]) -> Result<usize, &'static str> {
        // Negative window bits: raw Deflate, no zlib header or Adler-32.
        let config = zlib_rs::InflateConfig { window_bits: -15 };
        let (written, rc) = zlib_rs::decompress_slice(out, input, config);
        match rc {
            zlib_rs::ReturnCode::Ok => Ok(written.len()),
            zlib_rs::ReturnCode::BufError => {
                Err("deflate stream larger than declared or truncated")
            }
            zlib_rs::ReturnCode::DataError => Err("corrupt deflate stream"),
            zlib_rs::ReturnCode::MemError => Err("out of memory inflating"),
            _ => Err("inflate failed"),
        }
    }
}

/// The inflater ZIP entries use.
pub(crate) fn inflater() -> &'static dyn Inflater {
    &ZlibRs
}
