//! Tier 3 hook: image embeddings from the optional AI pack (not shipped yet).
//!
//! An [`Embedder`] turns the same thumbnail the signatures come from into a unit vector; the
//! nearest neighbours by cosine distance become extra similar-shot candidates, passed to
//! [`crate::group::similar_edges`] as `extra` with their distance mapped onto the visual
//! scale. Nothing here runs unless an embedder is installed.
//!
//! A scan is told what tier 3 can do through [`ScanSetup::tier3`](crate::scan::ScanSetup):
//! [`Tier3::NeedsAiPack`] (the default: the pack's Search feature is not installed),
//! [`Tier3::Off`] (installed, switched off) or [`Tier3::Ready`] with the stored embeddings.
//! The scan reports which it was in [`Results::tier3`](crate::scan::Results), so the UI can
//! say "needs the AI pack" rather than nothing.

use std::path::Path;
use std::sync::Arc;

/// Stored embeddings (the AI pack's `embeddings` table), looked up by file.
pub trait Embeddings: Send + Sync {
    /// The unit vector of each file, where one is stored.
    fn vectors(&self, files: &[&Path]) -> Vec<Option<Vec<f32>>>;
}

/// What tier 3 can do in a scan.
#[derive(Clone, Default)]
pub enum Tier3 {
    /// The AI pack (its Search feature) is not installed.
    #[default]
    NeedsAiPack,
    /// Installed but switched off.
    Off,
    /// Embeddings to pair by: candidates within cosine distance `max_cos`.
    Ready {
        embeddings: Arc<dyn Embeddings>,
        max_cos: f32,
    },
}

/// What tier 3 did in a scan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tier3Status {
    /// Not available: the AI pack is not installed.
    #[default]
    NeedsAiPack,
    Off,
    /// Used: `vectors` of the units had embeddings, giving `pairs` extra candidates.
    Used {
        vectors: usize,
        pairs: usize,
    },
}

/// Runs tier 3 for the units whose signature files are `files`: the extra pairs and what
/// happened.
pub fn tier3_pairs(tier3: &Tier3, files: &[&Path]) -> (Vec<(u32, u32, f32)>, Tier3Status) {
    match tier3 {
        Tier3::NeedsAiPack => (Vec::new(), Tier3Status::NeedsAiPack),
        Tier3::Off => (Vec::new(), Tier3Status::Off),
        Tier3::Ready {
            embeddings,
            max_cos,
        } => {
            let v = embeddings.vectors(files);
            let vectors = v.iter().filter(|x| x.is_some()).count();
            let pairs = extra_pairs(&v, *max_cos);
            let status = Tier3Status::Used {
                vectors,
                pairs: pairs.len(),
            };
            (pairs, status)
        }
    }
}

/// A model that embeds an image.
pub trait Embedder: Send + Sync {
    /// Length of the vectors.
    fn dims(&self) -> usize;
    /// The embedding of an RGBA image (any size), L2-normalised; `None` if it cannot.
    fn embed(&self, w: u32, h: u32, rgba: &[u8]) -> Option<Vec<f32>>;
}

/// Extra candidate pairs `(a, b, distance)` from embeddings: every pair within cosine
/// distance `max_cos` (brute force; a real pack would bring an ANN index), with the cosine
/// distance scaled so that `max_cos` maps to the loosest visual threshold.
pub fn extra_pairs(vectors: &[Option<Vec<f32>>], max_cos: f32) -> Vec<(u32, u32, f32)> {
    let loosest = crate::group::threshold(1.0);
    let mut out = Vec::new();
    for (i, a) in vectors.iter().enumerate() {
        let Some(a) = a else { continue };
        for (j, b) in vectors.iter().enumerate().skip(i + 1) {
            let Some(b) = b else { continue };
            if a.len() != b.len() {
                continue;
            }
            let dot: f32 = a.iter().zip(b).map(|(x, y)| x * y).sum();
            let d = 1.0 - dot;
            if d <= max_cos {
                out.push((i as u32, j as u32, d / max_cos.max(1e-6) * loosest));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixed;

    impl Embeddings for Fixed {
        fn vectors(&self, files: &[&Path]) -> Vec<Option<Vec<f32>>> {
            files
                .iter()
                .map(|f| match f.to_str() {
                    Some("a") => Some(vec![1.0, 0.0]),
                    Some("b") => Some(vec![0.999, 0.0447]),
                    _ => None,
                })
                .collect()
        }
    }

    #[test]
    fn tier3_says_when_the_pack_is_missing() {
        let files = [Path::new("a"), Path::new("b"), Path::new("c")];
        assert_eq!(
            tier3_pairs(&Tier3::default(), &files),
            (Vec::new(), Tier3Status::NeedsAiPack)
        );
        assert_eq!(tier3_pairs(&Tier3::Off, &files).1, Tier3Status::Off);
        let ready = Tier3::Ready {
            embeddings: Arc::new(Fixed),
            max_cos: 0.01,
        };
        let (pairs, status) = tier3_pairs(&ready, &files);
        assert_eq!(
            status,
            Tier3Status::Used {
                vectors: 2,
                pairs: 1
            }
        );
        assert_eq!((pairs[0].0, pairs[0].1), (0, 1));
    }

    #[test]
    fn close_vectors_pair() {
        let v = vec![
            Some(vec![1.0, 0.0]),
            Some(vec![0.995, 0.0998]),
            Some(vec![0.0, 1.0]),
            None,
        ];
        let p = super::extra_pairs(&v, 0.05);
        assert_eq!(p.len(), 1);
        assert_eq!((p[0].0, p[0].1), (0, 1));
    }
}
