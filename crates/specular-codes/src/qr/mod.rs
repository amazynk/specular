//! QR codes (ISO/IEC 18004), versions 1 to 40, all four error correction levels and
//! masks, mirrored symbols, and light-on-dark ones.

pub mod content;
pub(crate) mod detect;
pub mod matrix;
pub mod tables;

pub use tables::Level;

use crate::image::{Binary, Gray, Runs};

/// A decoded symbol before it is placed in the list of codes.
pub(crate) struct Symbol {
    pub version: usize,
    pub level: Level,
    pub content: content::Content,
    /// Corners in the image: top-left, top-right, bottom-right, bottom-left (as printed).
    pub corners: [(f32, f32); 4],
    pub mirrored: bool,
    pub corrected: usize,
}

/// Whether a grid's format information read (it is probably a symbol, misplaced a little).
fn plausible(g: &matrix::Grid) -> bool {
    [g.clone(), g.transposed()].iter().any(|x| {
        !matches!(
            matrix::read(x),
            Err(matrix::Fail::Format | matrix::Fail::Version)
        )
    })
}

fn decode_grid(g: &matrix::Grid) -> Option<(matrix::Codewords, content::Content, bool)> {
    for (mirrored, grid) in [(false, g.clone()), (true, g.transposed())] {
        if let Ok(cw) = matrix::read(&grid)
            && let Ok(c) = content::parse(&cw.data, cw.version)
        {
            return Some((cw, c, mirrored));
        }
    }
    None
}

/// Every QR symbol found in a binarised image (and its dark runs); `grey`: the picture it
/// was binarised from, and whether `b` is inverted from it.
pub(crate) fn find(b: &Binary, runs: &Runs, grey: Option<(&Gray, bool)>) -> Vec<Symbol> {
    let mut finders = detect::finders(b, runs);
    finders.sort_by_key(|f| std::cmp::Reverse(f.count));
    // confirmed by two or more scan lines, when there are enough of those
    if finders.iter().filter(|f| f.count >= 2).count() >= 3 {
        finders.retain(|f| f.count >= 2);
    }
    finders.truncate(16);
    let n = finders.len();
    let mut used = vec![false; n];
    let mut out: Vec<Symbol> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            for k in j + 1..n {
                if used[i] || used[j] || used[k] {
                    continue;
                }
                let Some((fs, dim)) = detect::arrange(b, &finders[i], &finders[j], &finders[k])
                else {
                    continue;
                };
                let mut done = None;
                let mut promising = false;
                let mut sizes = vec![
                    dim,
                    dim + 4,
                    dim.saturating_sub(4),
                    dim + 8,
                    dim.saturating_sub(8),
                ];
                let mut at = 0;
                'tries: while at < sizes.len() {
                    let d = sizes[at];
                    at += 1;
                    if !(21..=177).contains(&d) {
                        continue;
                    }
                    for align in [true, false] {
                        let Some(loc) = detect::locate(b, &fs[0], &fs[1], &fs[2], d, align) else {
                            continue;
                        };
                        // the version information names the size outright
                        if let Some(v) = matrix::version_hint(&loc.grid) {
                            let n = tables::size(v);
                            if n != d && !sizes[..at].contains(&n) {
                                sizes.insert(at, n);
                            }
                        }
                        if let Some((cw, c, mirrored)) = decode_grid(&loc.grid) {
                            done = Some(symbol(&loc, d, cw, c, mirrored));
                            break 'tries;
                        }
                        if d == dim && !align {
                            promising = plausible(&loc.grid);
                        }
                    }
                }
                if done.is_none() && promising {
                    // no alignment pattern to go by: look around the corner estimate
                    'nudge: for dy in -2..=2 {
                        for dx in -2..=2 {
                            if (dx, dy) == (0, 0) {
                                continue;
                            }
                            let nudge = (f64::from(dx) * 0.75, f64::from(dy) * 0.75);
                            let Some(loc) =
                                detect::locate_nudged(b, &fs[0], &fs[1], &fs[2], dim, false, nudge)
                            else {
                                continue;
                            };
                            if let Some((cw, c, mirrored)) = decode_grid(&loc.grid) {
                                done = Some(symbol(&loc, dim, cw, c, mirrored));
                                break 'nudge;
                            }
                        }
                    }
                }
                // Strong perspective: the alignment pattern found from the symbol's outer
                // edges, else those edges' corners.
                let outlined: Vec<detect::Located> = if done.is_none() {
                    detect::outline(b, &fs[0], &fs[1], &fs[2], dim)
                        .map(|o| o.aligned(b).into_iter().chain(o.cornered(b)).collect())
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                // Each grid also read from its module centres alone (small modules bleed
                // into the outer taps).
                'outline: for loc in &outlined {
                    for grid in [loc.grid.clone(), loc.centres(b)] {
                        if let Some((cw, c, mirrored)) = decode_grid(&grid) {
                            done = Some(symbol(loc, dim, cw, c, mirrored));
                            break 'outline;
                        }
                    }
                }
                if done.is_none() {
                    // A bent symbol: columns and rows where the timing patterns put them.
                    // The bend stretches the finders apart, so nearby sizes too.
                    let placed = [dim, dim.saturating_sub(4), dim + 4, dim.saturating_sub(8)]
                        .into_iter()
                        .filter(|d| (21..=177).contains(d))
                        .flat_map(|d| {
                            [true, false].into_iter().filter_map(move |align| {
                                detect::place(b, &fs[0], &fs[1], &fs[2], d, align)
                            })
                        });
                    'bent: for loc in outlined.into_iter().chain(placed) {
                        for w in loc.warps(b) {
                            if let Some((cw, c, mirrored)) =
                                loc.timed(b, &w).as_ref().and_then(decode_grid)
                            {
                                let d = loc.grid.size;
                                done = Some(symbol(&loc, d, cw, c, mirrored));
                                break 'bent;
                            }
                        }
                    }
                }
                if done.is_none()
                    && let Some((g, inverted)) = grey
                {
                    // Small blurred modules a binarisation loses: each module's own shade.
                    let placed = [dim, dim + 4, dim.saturating_sub(4)]
                        .into_iter()
                        .filter(|d| (21..=177).contains(d))
                        .flat_map(|d| {
                            [true, false].into_iter().filter_map(move |align| {
                                detect::place(b, &fs[0], &fs[1], &fs[2], d, align)
                            })
                        });
                    'grey: for loc in placed {
                        let warps = loc.warps(b);
                        for w in std::iter::once(None).chain(warps.iter().map(Some)) {
                            if let Some(grid) = loc.grey_grid(g, inverted, w)
                                && let Some((cw, c, mirrored)) = decode_grid(&grid)
                            {
                                done = Some(symbol(&loc, loc.grid.size, cw, c, mirrored));
                                break 'grey;
                            }
                        }
                    }
                }
                if let Some(s) = done {
                    used[i] = true;
                    used[j] = true;
                    used[k] = true;
                    if !out
                        .iter()
                        .any(|o| o.content == s.content && near(o.corners[0], s.corners[0]))
                    {
                        out.push(s);
                    }
                }
            }
        }
    }
    out
}

fn symbol(
    loc: &detect::Located,
    d: usize,
    cw: matrix::Codewords,
    c: content::Content,
    mirrored: bool,
) -> Symbol {
    let dd = d as f64;
    let corner = |x: f64, y: f64| {
        let (a, b) = loc.to_image.map(x, y);
        (a as f32, b as f32)
    };
    Symbol {
        version: cw.version,
        level: cw.level,
        content: c,
        corners: [
            corner(0.0, 0.0),
            corner(dd, 0.0),
            corner(dd, dd),
            corner(0.0, dd),
        ],
        mirrored,
        corrected: cw.corrected,
    }
}

fn near(a: (f32, f32), b: (f32, f32)) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) < 8.0
}
