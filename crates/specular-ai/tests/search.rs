//! Exact search over 100 000 synthetic 512-d f16 embeddings: the GPU's scores and ranking
//! are the CPU's, bit for bit. Timings are printed (`--nocapture`).
//!
//! Skips the GPU half without an adapter unless SPECULAR_REQUIRE_GPU is set.

use std::time::Instant;

use specular_ai::search::{GpuIndex, scores_cpu, search_cpu, top_k};
use specular_ai::testing::random_vectors;

const N: usize = 100_000;
const DIMS: usize = 512;
const K: usize = 50;

fn threads() -> usize {
    std::thread::available_parallelism().map_or(1, |n| n.get())
}

#[test]
fn gpu_and_cpu_agree_bit_for_bit_at_100k() {
    let t = Instant::now();
    let v = random_vectors(N, DIMS, 1);
    eprintln!(
        "build {N} x {DIMS} f16 vectors: {:.0} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    let queries: Vec<Vec<u16>> = (0..4)
        .map(|i| random_vectors(1, DIMS, 1000 + i).get(0))
        .collect();
    // A query equal to a stored vector finds it first.
    let mut queries = queries;
    queries.push(v.get(77_777));

    let mut cpu = Vec::new();
    for (i, q) in queries.iter().enumerate() {
        let t = Instant::now();
        let s = scores_cpu(&v, q, threads());
        let hits = top_k(&v, &s, K);
        eprintln!(
            "cpu query {i}: {:.1} ms ({} threads)",
            t.elapsed().as_secs_f64() * 1e3,
            threads()
        );
        cpu.push((s, hits));
    }
    let t = Instant::now();
    let one = search_cpu(&v, &queries[0], K, 1);
    eprintln!(
        "cpu query 0, one thread: {:.1} ms",
        t.elapsed().as_secs_f64() * 1e3
    );
    assert_eq!(one, cpu[0].1);
    assert_eq!(cpu[4].1[0].id, 77_777);

    let gpu = match specular_gfx::Gpu::new() {
        Ok(g) => g,
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): GPU search skipped");
            return;
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    eprintln!("gpu: {} ({:?})", gpu.info.name, gpu.info.backend);
    let t = Instant::now();
    let index = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
    eprintln!("gpu upload: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
    for (i, q) in queries.iter().enumerate() {
        let t = Instant::now();
        let s = index.scores(&gpu.device, &gpu.queue, q).unwrap();
        let hits = top_k(&v, &s, K);
        eprintln!("gpu query {i}: {:.1} ms", t.elapsed().as_secs_f64() * 1e3);
        let (cs, ch) = &cpu[i];
        let differ = cs
            .iter()
            .zip(&s)
            .filter(|(a, b)| a.to_bits() != b.to_bits())
            .count();
        assert_eq!(differ, 0, "query {i}: {differ} scores differ");
        assert_eq!(&hits, ch, "query {i}");
        assert_eq!(index.search(&gpu.device, &gpu.queue, q, K).unwrap(), *ch);
    }
}

#[test]
fn gpu_handles_partial_blocks_and_tiny_sets() {
    let gpu = match specular_gfx::Gpu::new() {
        Ok(g) => g,
        Err(_) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => return,
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    for n in [0, 1, 3, 1023, 1025, 4097] {
        let v = random_vectors(n, 64, n as u64);
        let q = random_vectors(1, 64, 5).get(0);
        let index = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
        let g = index.scores(&gpu.device, &gpu.queue, &q).unwrap();
        let c = scores_cpu(&v, &q, 3);
        assert_eq!(g.len(), n);
        assert!(
            g.iter().zip(&c).all(|(a, b)| a.to_bits() == b.to_bits()),
            "n = {n}"
        );
    }
}

/// Adds and removes in random batches (crossing block and chunk boundaries, with lazy
/// compaction), updating one GPU index all along: after every step its hits are the CPU's on
/// the same vectors, and both are a fresh build's over the rows left (same ids, same scores,
/// same order).
#[test]
fn incremental_index_matches_a_fresh_build() {
    use specular_ai::search::{Hit, Vectors};
    let gpu = match specular_gfx::Gpu::new() {
        Ok(g) => Some(g),
        Err(e) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => {
            eprintln!("no GPU ({e}): CPU against a fresh build only");
            None
        }
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    const D: usize = 32;
    let pool = random_vectors(12_000, D, 3);
    let queries: Vec<Vec<u16>> = (0..3)
        .map(|i| random_vectors(1, D, 500 + i).get(0))
        .collect();
    let mut rng = 0x2545_f491_4f6c_dd1du64;
    let mut next = |n: usize| {
        rng ^= rng << 13;
        rng ^= rng >> 7;
        rng ^= rng << 17;
        (rng % n.max(1) as u64) as usize
    };
    let key =
        |h: &[Hit]| -> Vec<(i64, u32)> { h.iter().map(|h| (h.id, h.score.to_bits())).collect() };
    let mut v = Vectors::new(D);
    let mut index = None::<GpuIndex>;
    let mut taken = 0usize;
    let mut compactions = 0;
    let t0 = Instant::now();
    for step in 0..40 {
        // Add up to 1,500 rows (ids repeat: an id is a file, embedded again).
        for _ in 0..next(1500) {
            let i = taken % pool.len();
            v.push((i % 9000) as i64, &pool.get(i));
            taken += 1;
        }
        // Remove some rows (now and then many).
        for _ in 0..next(if step % 5 == 4 { 3000 } else { 300 }) {
            v.remove(next(v.len()));
        }
        if v.compact_if_sparse() {
            compactions += 1;
        }
        let mut fresh = Vectors::new(D);
        for i in (0..v.len()).filter(|&i| !v.is_removed(i)) {
            fresh.push(v.ids()[i], &v.get(i));
        }
        assert_eq!(fresh.len(), v.live());
        let ks: Vec<usize> = queries.iter().map(|_| 1 + next(60)).collect();
        for (q, &k) in queries.iter().zip(&ks) {
            let c = search_cpu(&v, q, k, 3);
            assert!(c.iter().all(|h| !v.is_removed(h.index as usize)));
            let f = search_cpu(&fresh, q, k, 3);
            assert_eq!(key(&c), key(&f), "step {step}: differs from a fresh build");
        }
        let Some(gpu) = gpu.as_ref() else {
            continue;
        };
        // A small chunk size, so sets of a few thousand span several chunks that grow.
        match index.as_mut() {
            Some(ix) => ix.update(&gpu.device, &gpu.queue, &v),
            None => {
                index = Some(GpuIndex::with_chunk_blocks(&gpu.device, &gpu.queue, &v, 2).unwrap())
            }
        }
        let ix = index.as_ref().unwrap();
        assert_eq!(ix.len(), v.len());
        let fresh_ix = GpuIndex::new(&gpu.device, &gpu.queue, &fresh).unwrap();
        for (q, &k) in queries.iter().zip(&ks) {
            let g = ix.search(&gpu.device, &gpu.queue, q, k).unwrap();
            assert_eq!(
                g,
                search_cpu(&v, q, k, 3),
                "step {step}: GPU and CPU differ"
            );
            let fg = fresh_ix.search(&gpu.device, &gpu.queue, q, k).unwrap();
            assert_eq!(
                key(&g),
                key(&fg),
                "step {step}: differs from a fresh GPU build"
            );
        }
    }
    eprintln!(
        "40 steps: {} rows at the end ({} live), {compactions} compactions, {:?}",
        v.len(),
        v.live(),
        t0.elapsed()
    );
    assert!(compactions >= 1);
    if let Some(ix) = index {
        assert_eq!(
            ix.uploads(),
            1 + compactions,
            "appends never upload everything"
        );
    }
}

/// Appending to a large index writes only the rows added: timed against a fresh upload.
#[test]
fn appending_is_cheaper_than_uploading_again() {
    let Ok(gpu) = specular_gfx::Gpu::new() else {
        assert!(
            std::env::var_os("SPECULAR_REQUIRE_GPU").is_none(),
            "no adapter"
        );
        return;
    };
    let mut v = random_vectors(N, DIMS, 9);
    let mut ix = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
    let extra = random_vectors(100, DIMS, 10);
    for i in 0..extra.len() {
        v.push(1_000_000 + i as i64, &extra.get(i));
    }
    let q = extra.get(42);
    let t = Instant::now();
    ix.update(&gpu.device, &gpu.queue, &v);
    let hits = ix.search(&gpu.device, &gpu.queue, &q, K).unwrap();
    let append = t.elapsed();
    let t = Instant::now();
    let fresh = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
    let again = fresh.search(&gpu.device, &gpu.queue, &q, K).unwrap();
    let upload = t.elapsed();
    eprintln!("100 rows onto {N} x {DIMS}: append + search {append:?}, upload + search {upload:?}");
    assert_eq!(hits, again);
    assert_eq!(hits[0].id, 1_000_042);
    assert_eq!(ix.uploads(), 1);
}

/// On a lost device (a driver reset, as `Device::destroy` leaves it) a GPU search neither
/// panics nor hangs: it fails, and an index made on a new device searches as before.
#[test]
fn a_lost_device_fails_the_search_then_a_new_one_works() {
    let gpu = match specular_gfx::Gpu::new() {
        Ok(g) => g,
        Err(_) if std::env::var_os("SPECULAR_REQUIRE_GPU").is_none() => return,
        Err(e) => panic!("SPECULAR_REQUIRE_GPU is set but there is no adapter: {e}"),
    };
    let v = random_vectors(3000, 64, 9);
    let q = random_vectors(1, 64, 10).get(0);
    let want = scores_cpu(&v, &q, 2);
    let index = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
    gpu.device.destroy();
    let t = Instant::now();
    assert!(index.scores(&gpu.device, &gpu.queue, &q).is_err());
    // Made on the dead device: no panic (its parameters are not filled), no results.
    let late = GpuIndex::new(&gpu.device, &gpu.queue, &v).unwrap();
    assert!(late.scores(&gpu.device, &gpu.queue, &q).is_err());
    assert!(t.elapsed().as_secs() < 5, "{:?}", t.elapsed());
    let fresh = specular_gfx::Gpu::new().expect("a new device");
    let index = GpuIndex::new(&fresh.device, &fresh.queue, &v).unwrap();
    let got = index.scores(&fresh.device, &fresh.queue, &q).unwrap();
    assert!(
        got.iter()
            .zip(&want)
            .all(|(a, b)| a.to_bits() == b.to_bits())
    );
}
