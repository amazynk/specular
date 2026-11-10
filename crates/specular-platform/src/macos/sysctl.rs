//! `sysctlbyname` reads: CPU brand, core counts and Apple Silicon performance levels.

use std::ffi::CString;

use crate::CoreCluster;

/// Reads a sysctl into `buf`; the number of bytes written, or `None` if the name is unknown or
/// the value does not fit.
#[allow(unsafe_code)]
fn read(name: &str, buf: &mut [u8]) -> Option<usize> {
    let name = CString::new(name).ok()?;
    let mut len: libc::size_t = buf.len();
    // SAFETY: `name` is a NUL-terminated string that outlives the call; `buf` is valid for
    // writes of `len` bytes and `len` is a valid in/out pointer. sysctlbyname writes at most
    // `len` bytes (failing with ENOMEM otherwise) and stores the written size in `len`. No
    // new value is passed (null, 0).
    let r = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    (r == 0 && len <= buf.len()).then_some(len)
}

/// An integer sysctl (`int` or `int64`).
pub(crate) fn int(name: &str) -> Option<i64> {
    let mut b = [0u8; 8];
    match read(name, &mut b)? {
        4 => Some(i32::from_ne_bytes([b[0], b[1], b[2], b[3]]) as i64),
        8 => Some(i64::from_ne_bytes(b)),
        _ => None,
    }
}

/// A string sysctl, without the trailing NUL.
pub(crate) fn string(name: &str) -> Option<String> {
    let mut b = [0u8; 256];
    let n = read(name, &mut b)?;
    let s = &b[..n];
    let s = s.split(|&c| c == 0).next().unwrap_or(s);
    let s = String::from_utf8_lossy(s).trim().to_owned();
    (!s.is_empty()).then_some(s)
}

/// `hw.perflevelN.*` (macOS 12+ on Apple Silicon): level 0 is the fastest. Intel Macs have no
/// `hw.nperflevels` (or report 1): empty.
pub(crate) fn core_clusters() -> Vec<CoreCluster> {
    let levels = int("hw.nperflevels").unwrap_or(0);
    if levels < 2 {
        return Vec::new();
    }
    (0..levels)
        .filter_map(|i| {
            let n = |k: &str| {
                int(&format!("hw.perflevel{i}.{k}")).and_then(|v| usize::try_from(v).ok())
            };
            Some(CoreCluster {
                name: string(&format!("hw.perflevel{i}.name"))
                    .unwrap_or_else(|| format!("level {i}")),
                physical: n("physicalcpu")?,
                logical: n("logicalcpu")?,
            })
        })
        .collect()
}
