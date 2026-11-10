//! HCP-style neuroimaging surfaces and tractography.
//!
//! - [`gifti`]: GIFTI surfaces and the per-vertex data that colours them (`.surf.gii`,
//!   `.func.gii`, `.shape.gii`, `.label.gii`).
//! - [`cifti`]: CIFTI-2 matrices (`.dscalar.nii`, `.dlabel.nii`, `.dtseries.nii`,
//!   `.pscalar.nii`): a NIfTI-2 file whose XML extension says which surface vertices and
//!   subcortical voxels each column is.
//! - [`trk`] and [`tck`]: TrackVis and MRtrix streamlines, in RAS millimetres
//!   ([`Tractogram`]).
//!
//! Every reader is bounded by the declared sizes and the bytes at hand; damaged files are
//! refused with an [`Error`], never a panic.

pub mod cifti;
pub mod gifti;
pub mod tck;
pub mod trk;
pub mod xml;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not a neuroimaging surface or tractogram")]
    NotRecognised,
    #[error("the file ends early ({0})")]
    Truncated(String),
    #[error("malformed {0}")]
    Malformed(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("too large to read")]
    TooLarge,
    #[error("{0}")]
    Io(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Streamlines in RAS millimetres (scanner space), with what the file stores per point and
/// per streamline.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tractogram {
    /// Every point, streamline after streamline.
    pub points: Vec<[f32; 3]>,
    /// Where each streamline starts in `points`, and one past the last point at the end
    /// (`streamlines() + 1` entries).
    pub offsets: Vec<u32>,
    /// Per-point values by name (TrackVis scalars), `points.len()` each.
    pub scalars: Vec<(String, Vec<f32>)>,
    /// Per-streamline values by name (TrackVis properties), `streamlines()` each.
    pub properties: Vec<(String, Vec<f32>)>,
}

impl Tractogram {
    pub fn streamlines(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// The points of streamline `s`.
    pub fn streamline(&self, s: usize) -> &[[f32; 3]] {
        &self.points[self.offsets[s] as usize..self.offsets[s + 1] as usize]
    }

    /// The smallest box holding every point (min, max); `None` without points.
    pub fn bounds(&self) -> Option<([f32; 3], [f32; 3])> {
        let mut it = self
            .points
            .iter()
            .filter(|p| p.iter().all(|v| v.is_finite()));
        let first = *it.next()?;
        Some(it.fold((first, first), |(mut lo, mut hi), p| {
            for a in 0..3 {
                lo[a] = lo[a].min(p[a]);
                hi[a] = hi[a].max(p[a]);
            }
            (lo, hi)
        }))
    }

    /// Every `step`-th streamline (from the first), so that at most `max_points` points
    /// remain; the same tractogram when it is already small enough.
    pub fn subsample(&self, max_points: usize) -> Tractogram {
        if self.points.len() <= max_points || self.streamlines() == 0 {
            return self.clone();
        }
        let step = self.points.len().div_ceil(max_points.max(1));
        let keep: Vec<usize> = (0..self.streamlines()).step_by(step).collect();
        let mut out = Tractogram {
            offsets: vec![0],
            scalars: self
                .scalars
                .iter()
                .map(|(n, _)| (n.clone(), Vec::new()))
                .collect(),
            properties: self
                .properties
                .iter()
                .map(|(n, _)| (n.clone(), Vec::new()))
                .collect(),
            ..Tractogram::default()
        };
        for &s in &keep {
            let (a, b) = (self.offsets[s] as usize, self.offsets[s + 1] as usize);
            out.points.extend_from_slice(&self.points[a..b]);
            for (o, (_, v)) in out.scalars.iter_mut().zip(&self.scalars) {
                o.1.extend_from_slice(&v[a..b]);
            }
            for (o, (_, v)) in out.properties.iter_mut().zip(&self.properties) {
                o.1.push(v[s]);
            }
            out.offsets.push(out.points.len() as u32);
        }
        out
    }
}

/// Decodes base64, skipping whitespace; `None` on any other stray byte.
pub(crate) fn base64(src: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(src.len() / 4 * 3 + 3);
    let mut acc = 0u32;
    let mut n = 0u32;
    let mut pad = 0usize;
    for &c in src {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            b'=' => {
                pad += 1;
                continue;
            }
            c if c.is_ascii_whitespace() => continue,
            _ => return None,
        };
        if pad > 0 {
            return None;
        }
        acc = (acc << 6) | u32::from(v);
        n += 1;
        if n == 4 {
            out.extend_from_slice(&[(acc >> 16) as u8, (acc >> 8) as u8, acc as u8]);
            acc = 0;
            n = 0;
        }
    }
    match n {
        0 => {}
        2 => out.push((acc >> 4) as u8),
        3 => out.extend_from_slice(&[(acc >> 10) as u8, (acc >> 2) as u8]),
        _ => return None,
    }
    Some(out)
}

/// The text of a `key: value` header line, split at the first colon and trimmed.
pub(crate) fn key_value(line: &str) -> Option<(&str, &str)> {
    let (k, v) = line.split_once(':')?;
    Some((k.trim(), v.trim()))
}

/// A NUL-terminated (or full-width) string field.
pub(crate) fn cstr(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        assert_eq!(base64(b"TWFu").unwrap(), b"Man");
        assert_eq!(base64(b"TWE=\n").unwrap(), b"Ma");
        assert_eq!(base64(b"T Q==").unwrap(), b"M");
        assert!(base64(b"T").is_none());
        assert!(base64(b"TQ==TQ").is_none());
        assert!(base64(b"T*==").is_none());
    }

    #[test]
    fn subsampling_keeps_whole_streamlines() {
        let t = Tractogram {
            points: (0..100).map(|i| [i as f32, 0.0, 0.0]).collect(),
            offsets: (0..=10).map(|i| i * 10).collect(),
            scalars: vec![("s".into(), (0..100).map(|i| i as f32).collect())],
            properties: vec![("p".into(), (0..10).map(|i| i as f32).collect())],
        };
        let s = t.subsample(30);
        assert!(s.points.len() <= 30);
        assert_eq!(s.streamlines(), 3);
        assert_eq!(s.streamline(1)[0], [40.0, 0.0, 0.0]);
        assert_eq!(s.properties[0].1, vec![0.0, 4.0, 8.0]);
        assert_eq!(s.scalars[0].1.len(), s.points.len());
    }
}
