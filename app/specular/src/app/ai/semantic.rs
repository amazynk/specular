//! Search by description in the Library (the Local AI pack's Search, DESIGN.md §10): a
//! `like:"dog on a beach"` term, or a query of plain words that matched no file name, is
//! encoded by the installed model's text tower (SigLIP 2, PE Core or a CLIP) and compared with every catalogued photo's
//! embedding; the files found narrow the query's other terms and come back best first.
//!
//! The model is loaded by the `ai-search` worker ([`super::index`]) and handed here; the
//! embeddings are read from the catalogue on the Library's search worker, incrementally
//! (only rows added since the last search; a file embedded again replaces its older row), and
//! searched exactly: on the GPU ([`specular_ai::search::GpuIndex`], which appends the rows
//! added and uploads anew only after the removed rows were compacted away) once there are
//! [`GPU_FROM`] of them and the worker has a device, else on the CPU; both give the same
//! hits in the same order. Nothing here runs on the UI thread except [`ready`], an atomic
//! load.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use specular_ai::infer::ClipTextEncoder;
use specular_ai::search::{GpuIndex, Hit, Vectors, search_cpu};
use specular_gfx::wgpu;
use specular_infer::clip::Layout;
use specular_library::{Library, Search};
use specular_query::{AiFeature, ErrorKind, Scope};

/// Hits kept at most (a description narrows a library to its best few hundred).
const CAP: usize = 1000;
/// Hits kept within this much of the best one's cosine (CLIP's text-image similarities sit in
/// a narrow band, roughly 0.15 to 0.35: further below the best is noise).
const MARGIN: f32 = 0.08;
/// The same for SigLIP, whose similarities sit lower: a matching photo scores about 0.1 to
/// 0.2, an unrelated one about 0 (measured on the Kodak check set, the right photo is always
/// within 0.03 of the best).
const MARGIN_SIGLIP: f32 = 0.05;
/// The same for PE Core: a matching photo scores about 0.3 to 0.4; on the Kodak check set
/// the right photo is always the best, the fourth at least 0.05 below it.
const MARGIN_PE: f32 = 0.05;
/// Embeddings from which the search runs on the GPU (fewer are as quick on the CPU).
const GPU_FROM: usize = specular_ai::search::BLOCK;

/// The installed Search model's text half, and the embeddings read so far.
pub(crate) struct Model {
    /// The catalogue's key for the embeddings ([`specular_ai::IndexModel::key`]).
    pub(crate) key: String,
    pub(crate) text: Arc<ClipTextEncoder>,
    /// The search worker's device (None: CPU only).
    gpu: Option<(wgpu::Device, wgpu::Queue)>,
    read: Mutex<Read>,
}

#[derive(Default)]
struct Read {
    vectors: Option<Vectors>,
    /// The row of each file's latest embedding in `vectors`.
    rows: HashMap<i64, usize>,
    /// The last catalogue rowid read.
    after: i64,
    /// The vectors on the GPU, kept in step with `vectors`.
    index: Option<GpuIndex>,
    /// The GPU failed once: the CPU from then on.
    gpu_failed: bool,
}

impl Model {
    pub(crate) fn new(
        key: String,
        text: Arc<ClipTextEncoder>,
        gpu: Option<(wgpu::Device, wgpu::Queue)>,
    ) -> Model {
        Model {
            key,
            text,
            gpu,
            read: Mutex::new(Read::default()),
        }
    }

    fn margin(&self) -> f32 {
        match self.text.layout() {
            Layout::Siglip => MARGIN_SIGLIP,
            Layout::PeCore => MARGIN_PE,
            Layout::OpenClip | Layout::HuggingFace => MARGIN,
        }
    }

    /// The files `text` describes best, best first, with their scores (searched on the
    /// calling thread, after reading the embeddings stored since the last call).
    pub(crate) fn rank(&self, lib: &Library, text: &str) -> Result<Vec<(i64, f32)>, String> {
        let q = self
            .text
            .encode(text)
            .map_err(|e| crate::error_text::text_reason(&e.to_string()))?;
        let mut read = self.read.lock();
        let dims = q.len();
        if read.vectors.as_ref().is_none_or(|v| v.dims() != dims) {
            *read = Read {
                vectors: Some(Vectors::new(dims)),
                ..Read::default()
            };
        }
        if let Some((rows, last)) = lib
            .embeddings_since(&self.key, dims, read.after)
            .map_err(|e| crate::error_text::reason(&e))?
        {
            let read = &mut *read;
            let v = read.vectors.as_mut().expect("made above");
            for (id, e) in rows {
                // Embedded again (the file changed): the older row goes.
                if let Some(old) = read.rows.insert(id, v.len()) {
                    v.remove(old);
                }
                v.push(id, &e);
            }
            if v.compact_if_sparse() {
                read.rows = v.ids().iter().enumerate().map(|(i, &id)| (id, i)).collect();
            }
            read.after = last;
        }
        let margin = self.margin();
        let read = &mut *read;
        let v = read.vectors.as_ref().expect("made above");
        if let Some((device, queue)) = self.gpu.as_ref()
            && !read.gpu_failed
            && v.len() >= GPU_FROM
        {
            match read.index.as_mut() {
                Some(index) => index.update(device, queue, v),
                None => match GpuIndex::new(device, queue, v) {
                    Ok(i) => read.index = Some(i),
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: search index on the GPU: {e}; searching on the CPU");
                        read.gpu_failed = true;
                    }
                },
            }
            if let Some(index) = read.index.as_ref() {
                match index.search(device, queue, &q, (CAP * 2).min(v.len())) {
                    Ok(hits) => return Ok(select(hits, margin)),
                    Err(e) => {
                        specular_log::warn!(target: specular_log::JOBS, "ai: search on the GPU: {e}; searching on the CPU");
                        read.gpu_failed = true;
                        read.index = None;
                    }
                }
            }
        }
        Ok(rank_vectors(v, &q, threads(), margin))
    }
}

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get().min(8))
}

/// The best of `v` for `query` on the CPU: highest first, each file once (a file embedded
/// again has its later vector further on; the better score counts), within `margin` of the
/// best ([`MARGIN`], [`MARGIN_SIGLIP`]) and at most [`CAP`].
pub(crate) fn rank_vectors(
    v: &Vectors,
    query: &[u16],
    threads: usize,
    margin: f32,
) -> Vec<(i64, f32)> {
    if v.is_empty() {
        return Vec::new();
    }
    select(
        search_cpu(v, query, (CAP * 2).min(v.len()), threads),
        margin,
    )
}

/// The hits kept of a search's best first: each file once, within `margin` of the best,
/// at most [`CAP`].
fn select(hits: Vec<Hit>, margin: f32) -> Vec<(i64, f32)> {
    let Some(best) = hits.first().map(|h| h.score) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    hits.into_iter()
        .take_while(|h| h.score >= best - margin)
        .filter(|h| seen.insert(h.id))
        .take(CAP)
        .map(|h| (h.id, h.score))
        .collect()
}

static MODEL: Mutex<Option<Arc<Model>>> = Mutex::new(None);
static READY: AtomicBool = AtomicBool::new(false);

/// Hands the loaded model over (None: Search is off, not installed or failed to load).
pub(crate) fn set(m: Option<Arc<Model>>) {
    READY.store(m.is_some(), Ordering::Relaxed);
    *MODEL.lock() = m;
}

/// The model searched with now.
pub(crate) fn model() -> Option<Arc<Model>> {
    MODEL.lock().clone()
}

/// Whether a search by description can run (the query bar then hides `like:`'s "needs the
/// Local AI pack").
pub(crate) fn ready() -> bool {
    READY.load(Ordering::Relaxed)
}

/// The `like:` terms of a Library query: their descriptions joined, and the query without
/// them. None without any.
pub(crate) fn split_like(query: &str) -> Option<(String, String)> {
    let q = specular_query::parse_in(query, Scope::Library);
    let mut spans: Vec<(usize, usize)> = q
        .errors
        .iter()
        .filter(|e| e.kind == ErrorKind::NeedsAiPack(AiFeature::Search))
        .map(|e| (e.span.start, e.span.end))
        .filter(|(s, e)| s < e && *e <= query.len())
        .collect();
    if spans.is_empty() {
        return None;
    }
    spans.sort_unstable();
    let mut words = Vec::new();
    let mut rest = String::new();
    let mut at = 0;
    for (s, e) in spans {
        if s < at || !query.is_char_boundary(s) || !query.is_char_boundary(e) {
            continue;
        }
        rest.push_str(&query[at..s]);
        let term = &query[s..e];
        let value = term.split_once(':').map_or("", |(_, v)| v);
        let value = value.trim().trim_matches('"').trim();
        if !value.is_empty() {
            words.push(value.to_owned());
        }
        at = e;
    }
    rest.push_str(&query[at..]);
    let text = words.join(" ");
    (!text.is_empty()).then(|| (text, rest.split_whitespace().collect::<Vec<_>>().join(" ")))
}

/// Whether `query` is plain words only (no fields, no negations, no errors): a description
/// when no file name matched it.
pub(crate) fn plain_words(query: &str) -> bool {
    let q = specular_query::parse_in(query, Scope::Library);
    !q.terms.is_empty()
        && q.errors.is_empty()
        && q.terms.iter().all(|t| !t.negated && t.field().is_none())
}

/// A Library search with its `like:` terms resolved (ranked, and removed from the query).
/// The request as it was, and false, without any or without a model.
pub(crate) fn with_like(lib: &Library, req: &Search) -> (Search, bool) {
    let Some(m) = model() else {
        return (req.clone(), false);
    };
    let Some((text, rest)) = split_like(&req.query) else {
        return (req.clone(), false);
    };
    match m.rank(lib, &text) {
        Ok(r) => {
            let mut req = req.clone();
            req.query = rest;
            req.ranked = Some(Arc::new(r));
            (req, true)
        }
        Err(e) => {
            specular_log::warn!(target: specular_log::JOBS, "ai: search by description: {e}");
            (req.clone(), false)
        }
    }
}

/// The search again as a description, when it was plain words that found nothing.
pub(crate) fn as_description(lib: &Library, req: &Search) -> Option<Search> {
    if !plain_words(&req.query) {
        return None;
    }
    let m = model()?;
    let r = m.rank(lib, &req.query).ok()?;
    let mut req = req.clone();
    req.query = String::new();
    req.ranked = Some(Arc::new(r));
    Some(req)
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_ai::testing::random_vectors;

    #[test]
    fn like_terms_are_lifted_out_of_the_query() {
        assert_eq!(
            split_like(r#"like:"dog on a beach" iso:100"#),
            Some(("dog on a beach".into(), "iso:100".into()))
        );
        assert_eq!(
            split_like("rating:3 like:sunset like:\"red kite\""),
            Some(("sunset red kite".into(), "rating:3".into()))
        );
        assert_eq!(split_like("iso:100 beach"), None);
        assert_eq!(split_like("like:"), None);
        assert!(plain_words("dog on a beach"));
        assert!(!plain_words("dog iso:100"));
        assert!(!plain_words("-dog"));
        assert!(!plain_words(""));
    }

    /// Search's whole path on a tiny random CLIP (written by `testing::tiny_clip`, loaded as
    /// an installed size is): synthetic photos embedded by the image tower are found again
    /// from a slightly changed copy, and a description ranks by the text tower's cosine.
    #[test]
    fn synthetic_photos_rank_by_the_clip_towers() {
        use specular_ai::infer::InferRuntime;
        use specular_ai::runtime::{Outputs, Runtime, Thumb};
        use specular_core::paths::{Engine, selector};

        let dir = std::env::temp_dir().join(format!("specular-semantic-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        specular_ai::testing::tiny_clip(&dir, "tiny", 3);
        let (rt, text) = InferRuntime::load_clip(
            &dir.join("tiny.safetensors"),
            &dir.join("merges.txt"),
            "tiny-1",
            specular_ai::manifest::Norm::Openai,
            None,
            selector(),
            2,
        )
        .unwrap();
        let side = rt.input_side();
        let photo = |k: u32, noise: u32| -> Vec<u8> {
            let mut px = Vec::with_capacity((side * side * 4) as usize);
            for y in 0..side {
                for x in 0..side {
                    let n = (x * 31 + y * 17 + noise) % 5 * (noise > 0) as u32;
                    let stripe = ((x * (k % 7 + 1) / side.max(1)) % 2) * 120;
                    px.extend([
                        ((k * 37 + x * 255 / side) % 256 + n).min(255) as u8,
                        ((k * 91 + stripe) % 256) as u8,
                        ((k * 53 + y * 255 / side) % 256) as u8,
                        255,
                    ]);
                }
            }
            px
        };
        let embed = |rgba: &[u8]| -> Vec<u16> {
            match rt
                .run(
                    Engine::Cpu,
                    &Thumb {
                        width: side,
                        height: side,
                        rgba,
                    },
                )
                .unwrap()
            {
                Outputs::Embedding(e) => e,
                o => panic!("{o:?}"),
            }
        };
        const N: u32 = 24;
        let t = std::time::Instant::now();
        let embs: Vec<Vec<u16>> = (0..N).map(|k| embed(&photo(k, 0))).collect();
        eprintln!(
            "tiny CLIP: {:.2} ms per photo ({side}² input)",
            t.elapsed().as_secs_f64() * 1e3 / f64::from(N)
        );
        let mut v = Vectors::new(embs[0].len());
        for (k, e) in embs.iter().enumerate() {
            v.push(100 + k as i64, e);
        }
        for k in 0..N {
            let q = embed(&photo(k, 1));
            let r = rank_vectors(&v, &q, 2, MARGIN);
            assert_eq!(r[0].0, 100 + i64::from(k), "photo {k} found first");
        }
        let q = text.encode("a red square on a beach").unwrap();
        assert_eq!(q.len(), v.dims());
        let r = rank_vectors(&v, &q, 2, MARGIN);
        let cos = |e: &[u16]| -> f32 {
            e.iter()
                .zip(&q)
                .map(|(a, b)| {
                    specular_ai::search::half_to_f32(u32::from(*a))
                        * specular_ai::search::half_to_f32(u32::from(*b))
                })
                .sum()
        };
        let best = (0..embs.len())
            .max_by(|a, b| cos(&embs[*a]).total_cmp(&cos(&embs[*b])))
            .unwrap();
        assert_eq!(r[0].0, 100 + best as i64);
        assert!(r.windows(2).all(|p| p[0].1 >= p[1].1));
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Search latency over 10k and 100k embeddings of 512 dimensions (CLIP B/32's), the
    /// query's ranking on the CPU: `cargo test --release -- --ignored search_latency`.
    #[test]
    #[ignore = "measurement"]
    fn search_latency() {
        for n in [10_000, 100_000] {
            let v = random_vectors(n, 512, 5);
            let q = random_vectors(1, 512, 6).get(0);
            let _ = rank_vectors(&v, &q, threads(), MARGIN);
            let t = std::time::Instant::now();
            const RUNS: u32 = 20;
            for _ in 0..RUNS {
                std::hint::black_box(rank_vectors(&v, &q, threads(), MARGIN));
            }
            eprintln!(
                "{n} embeddings: {:.2} ms per search ({} threads)",
                t.elapsed().as_secs_f64() * 1e3 / f64::from(RUNS),
                threads()
            );
        }
    }

    /// The ranking is the brute-force order (best first), each file once, cut at the margin.
    #[test]
    fn ranking_matches_brute_force() {
        let dims = 64;
        let v = random_vectors(5_000, dims, 7);
        let own = rank_vectors(&v, &v.get(1234), 4, MARGIN);
        assert_eq!(own[0].0, 1234, "a vector finds itself first");
        let q = random_vectors(1, dims, 99).get(0);
        let r = rank_vectors(&v, &q, 4, MARGIN);
        assert!(r.len() > 3, "{}", r.len());
        let dot = |i: usize| -> f32 {
            let a = v.get(i);
            a.iter()
                .zip(&q)
                .map(|(x, y)| {
                    specular_ai::search::half_to_f32(u32::from(*x))
                        * specular_ai::search::half_to_f32(u32::from(*y))
                })
                .sum()
        };
        let mut all: Vec<(i64, f32)> = (0..v.len()).map(|i| (i as i64, dot(i))).collect();
        all.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        let best = all[0].1;
        let want: Vec<i64> = all
            .iter()
            .take_while(|x| x.1 >= best - MARGIN)
            .take(CAP)
            .map(|x| x.0)
            .collect();
        let got: Vec<i64> = r.iter().map(|x| x.0).collect();
        assert_eq!(got.len(), want.len());
        // Equal up to f32 summation order: compare as sets of the top ones and the first.
        assert_eq!(got[0], want[0]);
        let (g, w): (HashSet<_>, HashSet<_>) = (got.iter().collect(), want.iter().collect());
        assert!(g.intersection(&w).count() + 2 >= w.len());
        assert!(r.windows(2).all(|p| p[0].1 >= p[1].1));
    }
}
