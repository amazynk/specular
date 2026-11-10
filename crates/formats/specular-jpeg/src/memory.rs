//! Memory estimates (see [`crate::memory_estimate`]). The model mirrors the decoder's large
//! allocations; everything else (tables, per-thread scratch, task bookkeeping) is small and
//! covered by a fixed allowance.

use crate::geom::OutGeom;
use crate::marker::{Frame, Parsed, frame_from_info};
use crate::prog::{AC_MARGIN, AC_MIN_CHUNK};
use crate::sequential::Checkpoint;
use crate::spec::{MARGIN, MIN_CHUNK, SPARSE};
use crate::{Coding, ColorModel, JpegInfo, MemoryEstimate, Region, Scale};

/// Small allocations not modelled individually (tables, task and band bookkeeping, per-thread
/// scratch): for the streaming (single-scan) and arithmetic decoders, and the Huffman coefficient-
/// buffer decoders.
const SLACK: u64 = 64 << 10;
const SLACK_COEF: u64 = 384 << 10;
const SLACK_BUILD_PROG: u64 = 192 << 10;
/// Bytes per stored coefficient block: 2 (DC) + 128 (AC).
pub(crate) const COEF_BLOCK: u64 = 130;
/// Minimum MCUs for speculative parsing of a baseline scan (`sequential::SPECULATIVE_MIN_MCUS`).
const SPECULATIVE_MIN_MCUS: u64 = 4096;

const CP: u64 = std::mem::size_of::<Checkpoint>() as u64;

/// One scan of a script, as far as memory is concerned.
#[derive(Clone)]
pub(crate) struct ScanShape {
    comps: Vec<usize>,
    ss: u8,
    ah: u8,
    /// Entropy-coded bytes, if known.
    len: Option<u64>,
}

impl ScanShape {
    fn new(comps: &[usize], ss: u8, ah: u8) -> Self {
        ScanShape {
            comps: comps.to_vec(),
            ss,
            ah,
            len: None,
        }
    }
}

/// The scans of `p` (the actual script, with each scan's compressed size).
fn script_of(p: &Parsed) -> Vec<ScanShape> {
    p.scans
        .iter()
        .map(|s| ScanShape {
            comps: s.comps.iter().map(|c| c.ci).collect(),
            ss: s.ss,
            ah: s.ah,
            len: Some(s.data_end.saturating_sub(s.data_start) as u64),
        })
        .collect()
}

/// The libjpeg standard progression script (`jpeg_simple_progression`, also used by
/// `cjpeg -progressive` and `jpegtran -progressive`) for frame `f`.
fn standard_script(f: &Frame, info: &JpegInfo) -> Vec<ScanShape> {
    let n = f.comps.len();
    let all: Vec<usize> = (0..n).collect();
    let mut s = Vec::new();
    if n == 3 && info.color == ColorModel::YCbCr {
        s.push(ScanShape::new(&all, 0, 0));
        s.push(ScanShape::new(&[0], 1, 0));
        s.push(ScanShape::new(&[2], 1, 0));
        s.push(ScanShape::new(&[1], 1, 0));
        s.push(ScanShape::new(&[0], 6, 0));
        s.push(ScanShape::new(&[0], 1, 2));
        s.push(ScanShape::new(&all, 0, 1));
        s.push(ScanShape::new(&[2], 1, 1));
        s.push(ScanShape::new(&[1], 1, 1));
        s.push(ScanShape::new(&[0], 1, 1));
    } else {
        s.push(ScanShape::new(&all, 0, 0));
        for (ss, ah) in [(1, 0), (6, 0), (1, 2)] {
            s.extend((0..n).map(|c| ScanShape::new(&[c], ss, ah)));
        }
        s.push(ScanShape::new(&all, 0, 1));
        s.extend((0..n).map(|c| ScanShape::new(&[c], 1, 1)));
    }
    s
}

/// Rows (checkpoints) of a progressive scan: MCU rows if interleaved, else the component's block
/// rows.
fn scan_rows(f: &Frame, s: &ScanShape) -> u64 {
    if s.comps.len() > 1 {
        f.mcus_y as u64
    } else {
        f.comps[s.comps[0]].bh as u64
    }
}

/// Speculative-parsing records kept for a scan of `units` units (MCUs or blocks) and `len`
/// bytes split into up to `max_chunks` chunks of at least `min_chunk` bytes whose runs extend
/// `margin` bytes into the next chunk: every `spec::SPARSE`-th record, every record in the
/// synchronisation zones (`2 * margin + 48` bytes per chunk boundary) and a small allowance per
/// chunk (see `spec::expected_recs`). Without `len` the zones are assumed to hold half of their
/// largest possible share (`2 * margin / min_chunk` of the units).
fn kept_recs(
    units: u64,
    len: Option<u64>,
    max_chunks: u64,
    min_chunk: usize,
    margin: usize,
) -> u64 {
    let (min_chunk, zone) = (min_chunk as u64, 2 * margin as u64 + 48);
    let (zones, nchunks) = match len {
        Some(len) => {
            let n = max_chunks.min(len / min_chunk).max(1);
            let z = units as u128 * ((n - 1) * zone) as u128 / len.max(1) as u128;
            (z as u64, n)
        }
        None => (units * zone / (2 * min_chunk), max_chunks),
    };
    units / SPARSE + zones + 64 * nchunks
}

/// Blocks of every component in MCU rows `rows` (full width).
fn blocks_in_rows(f: &Frame, rows: u32) -> u64 {
    f.comps
        .iter()
        .map(|c| rows as u64 * c.v as u64 * c.bw_pad as u64)
        .sum()
}

/// Peak bytes of the speculative AC-first parsing of a progressive file (all such scans run
/// concurrently, each in up to `2 * threads` chunks, see `prog::decode`).
fn progressive_spec_bytes(f: &Frame, script: &[ScanShape], threads: u64) -> u64 {
    script
        .iter()
        .filter(|s| s.ss > 0 && s.ah == 0)
        .map(|s| {
            let c = &f.comps[s.comps[0]];
            let blocks = c.bw as u64 * c.bh as u64;
            kept_recs(blocks, s.len, threads * 2, AC_MIN_CHUNK, AC_MARGIN)
                * crate::prog::RUN_REC_BYTES
                + c.bh as u64 * CP
        })
        .sum()
}

/// Speculative parsing of the refinement scans long enough for it (all may run at once), and
/// the nonzero masks the parse reads: for decodes with output one per block of the largest
/// such component (computed from the coefficients); for index builds a snapshot per scan of
/// the bands queued when it starts, up to a whole component each depending on timing (the
/// worst case is counted).
fn refine_spec_bytes(f: &Frame, script: &[ScanShape], output: bool) -> u64 {
    let mut masks = 0;
    let mut snapshots = 0;
    let parse: u64 = script
        .iter()
        .filter(|s| {
            s.ss > 0 && s.ah > 0 && s.len.is_none_or(|l| l >= crate::prog::REFINE_MIN as u64)
        })
        .map(|s| {
            let c = &f.comps[s.comps[0]];
            let blocks = c.bw as u64 * c.bh as u64;
            masks = masks.max(blocks * 8);
            snapshots += blocks * 8;
            crate::prog::refine_spec_bytes(blocks, c.bh as u64)
        })
        .sum();
    parse + if output { masks } else { snapshots }
}

/// Blocks that get a nonzero mask in an index-only build: those of components with an AC scan
/// that is not parsed speculatively (refinement scans, or every AC scan without speculation).
fn masked_blocks(f: &Frame, script: &[ScanShape], spec: bool) -> u64 {
    f.comps
        .iter()
        .enumerate()
        .filter(|&(ci, _)| {
            script
                .iter()
                .any(|s| s.ss > 0 && s.comps[0] == ci && (s.ah > 0 || !spec))
        })
        .map(|(_, c)| f.mcus_y as u64 * c.v as u64 * c.bw_pad as u64)
        .sum()
}

/// Thread count of a [`crate::Parallelism::Auto`] decode of `f`.
fn auto_threads(f: &Frame) -> u64 {
    if (f.width as u64 * f.height as u64) < crate::AUTO_PARALLEL_MIN_PIXELS {
        1
    } else {
        rayon::current_num_threads() as u64
    }
}

/// [`crate::memory_estimate_for`]: the actual scan script and scan sizes.
pub(crate) fn estimate_parsed(p: &Parsed, scale: Scale, region: Option<Region>) -> MemoryEstimate {
    estimate(&p.info, scale, region, Some(script_of(p)))
}

/// [`crate::memory_estimate`] if `script` is `None` (the standard script is assumed).
pub(crate) fn estimate(
    info: &JpegInfo,
    scale: Scale,
    region: Option<Region>,
    script: Option<Vec<ScanShape>>,
) -> MemoryEstimate {
    let Ok(f) = frame_from_info(info) else {
        return MemoryEstimate::default();
    };
    let Ok(g) = OutGeom::new(&f, scale, region) else {
        return MemoryEstimate::default();
    };
    let planes = g.total_bytes();
    let rgba = g.width as u64 * g.height as u64 * 4;
    let cmyk = matches!(info.color, ColorModel::Cmyk | ColorModel::Ycck);
    let threads = auto_threads(&f);

    if info.coding == Coding::Lossless {
        // Native decoders: per-component sample planes padded to whole MCUs, then 8-bit RGBA.
        // Huffman (SOF3): 16-bit samples, written straight to the output (no full-size RGBA).
        // Arithmetic (SOF11): a 16-bit sample and a category byte, plus a full-size RGBA
        // unless the output is exactly that.
        let s = scale as u64;
        let (w, h) = (f.width as u64, f.height as u64);
        let arith = info.sof_marker == 0xCB;
        let samples = w * h * f.comps.len() as u64 * if arith { 3 } else { 2 };
        let (ow, oh) = match region {
            Some(r) => (r.w as u64, r.h as u64),
            None => (w.div_ceil(s), h.div_ceil(s)),
        };
        let out = ow.min(w.div_ceil(s)) * oh.min(h.div_ceil(s)) * 4;
        let full = w * h * 4;
        let working = if arith {
            samples + if out == full && s == 1 { 0 } else { full }
        } else {
            samples
        };
        return MemoryEstimate {
            output_bytes: out,
            working_bytes: working + SLACK_COEF,
            working_bytes_with_index: working + SLACK_COEF,
            index_bytes: 0,
            index_build_bytes: 0,
        };
    }

    if info.coding == Coding::Arithmetic {
        return arith_estimate(info, &f, &g, script, planes);
    }

    let progressive = info.coding == Coding::Progressive;
    let streaming =
        !progressive && info.precision == 8 && info.first_scan_components as usize == f.comps.len();
    let dc_only = g.n == 1 && f.precision == 8;
    let mcus_x = f.mcus_x as u64;
    let mcus_y = f.mcus_y as u64;
    let spec_on = info.restart_interval == 0 && threads > 1;

    // Decoder work (beyond the planes) without and with an index.
    let (work, work_ix, index_bytes, build) = if streaming {
        let index = mcus_y * CP;
        let len = script.as_ref().and_then(|s| s.first()).and_then(|s| s.len);
        let spec = kept_recs(mcus_x * mcus_y, len, threads * 4, MIN_CHUNK, MARGIN)
            * crate::spec::RUN_REC_BYTES
            + index;
        let work = if spec_on && (g.my1 - g.my0) as u64 * mcus_x >= SPECULATIVE_MIN_MCUS {
            spec
        } else {
            0
        };
        (work, 0, index, if spec_on { spec } else { index })
    } else if progressive {
        let script = script.unwrap_or_else(|| standard_script(&f, info));
        let per_block = if dc_only { 2 } else { COEF_BLOCK };
        let coefs = blocks_in_rows(&f, g.my1) * per_block;
        // With an index, big decodes run in stripes of bounded coefficient memory, each
        // decoded straight into its rows of the planes (`crate::coef_striped`).
        let stripe = crate::stripe_rows(&f, &g);
        let coefs_ix = if stripe < g.my1 - g.my0 {
            blocks_in_rows(&f, stripe) * per_block
        } else {
            blocks_in_rows(&f, g.my1 - g.my0) * per_block
        };
        let index = script.iter().map(|s| scan_rows(&f, s)).sum::<u64>() * CP;
        let spec = if spec_on {
            progressive_spec_bytes(&f, &script, threads)
        } else {
            0
        };
        // Index-only build: speculative AC-first parsing, then one nonzero mask per block of the
        // components that still have scans to parse.
        let masks = masked_blocks(&f, &script, spec_on) * 8;
        // Refinement scans parsed speculatively (no region restriction, see `prog::decode`).
        let full = g.my1 == f.mcus_y;
        let refine = |output| {
            if spec_on && full {
                refine_spec_bytes(&f, &script, output)
            } else {
                0
            }
        };
        let work = if dc_only {
            coefs
        } else {
            spec.max(coefs + refine(true))
        };
        let build = spec.max(
            masks
                + if spec_on {
                    refine_spec_bytes(&f, &script, false)
                } else {
                    0
                },
        );
        (work, coefs_ix, index, build + index)
    } else {
        // Multi-scan sequential or 12-bit: coefficient windows (full width unless DC-only).
        let per_block = if dc_only { 2 } else { COEF_BLOCK };
        let width_blocks = |c: &crate::marker::FrameComp| -> u64 {
            if dc_only {
                ((g.mx1 - g.mx0) * c.h) as u64
            } else {
                c.bw_pad as u64
            }
        };
        let rows = |r0: u32| -> u64 {
            f.comps
                .iter()
                .map(|c| width_blocks(c) * ((g.my1 - r0) * c.v) as u64)
                .sum()
        };
        let r0 = if dc_only { g.my0 } else { 0 };
        let index = mcus_y * f.comps.len() as u64 * CP;
        let build = f.total_blocks() * COEF_BLOCK + index;
        (rows(r0) * per_block, rows(g.my0) * per_block, index, build)
    };

    // CMYK / YCCK: the planes are converted into one RGBA plane at the end.
    let output = if cmyk { rgba } else { planes };
    let slack = if streaming { SLACK } else { SLACK_COEF };
    let peak = |w: u64| -> u64 {
        if cmyk {
            (planes + w).max(planes + rgba) - output
        } else {
            w
        }
    };
    MemoryEstimate {
        output_bytes: output,
        working_bytes: peak(work) + slack,
        working_bytes_with_index: peak(work_ix) + slack,
        index_bytes,
        // Index builds produce no output and need fewer of the small allocations.
        index_build_bytes: build + if progressive { SLACK_BUILD_PROG } else { SLACK },
    }
}

/// Arithmetic-coded frames (`crate::arith`): full-width coefficient windows (from the top for
/// progressive files without an index, else the needed rows only); the index holds one coder
/// state plus the statistics of the scan's tables per scan row. An index build of a
/// progressive file keeps every coefficient, one of a sequential file none.
fn arith_estimate(
    info: &JpegInfo,
    f: &Frame,
    g: &OutGeom,
    script: Option<Vec<ScanShape>>,
    planes: u64,
) -> MemoryEstimate {
    let progressive = info.sof_marker == 0xCA;
    let dc_only = g.n == 1 && f.precision == 8;
    let per_block = if dc_only { 2 } else { COEF_BLOCK };
    let r0 = if progressive { 0 } else { g.my0 };
    let work = blocks_in_rows(f, g.my1 - r0) * per_block;
    let work_ix = blocks_in_rows(f, g.my1 - g.my0) * per_block;
    let script = script.unwrap_or_else(|| {
        if progressive {
            standard_script(f, info)
        } else {
            vec![ScanShape::new(
                &(0..f.comps.len()).collect::<Vec<_>>(),
                0,
                0,
            )]
        }
    });
    let cp = std::mem::size_of::<crate::arith::ArithCp>() as u64;
    let index: u64 = script
        .iter()
        .map(|s| {
            // Up to two tables of each kind (luma, chroma) per scan.
            let tables = s.comps.len().min(2) as u64;
            let dc = if s.ss == 0 && s.ah == 0 { 64 } else { 0 };
            let ac = if s.ss > 0 || !progressive { 256 } else { 0 };
            scan_rows(f, s) * (cp + tables * (dc + ac))
        })
        .sum();
    let build = if progressive {
        f.total_blocks() * COEF_BLOCK
    } else {
        0
    };
    let cmyk = matches!(info.color, ColorModel::Cmyk | ColorModel::Ycck);
    let rgba = g.width as u64 * g.height as u64 * 4;
    let output = if cmyk { rgba } else { planes };
    let peak = |w: u64| -> u64 {
        if cmyk {
            (planes + w).max(planes + rgba) - output
        } else {
            w
        }
    };
    MemoryEstimate {
        output_bytes: output,
        working_bytes: peak(work) + SLACK,
        working_bytes_with_index: peak(work_ix) + SLACK,
        index_bytes: index,
        index_build_bytes: build + index + SLACK,
    }
}
