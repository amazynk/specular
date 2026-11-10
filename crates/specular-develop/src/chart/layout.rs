//! Chart layouts: where each patch of a chart sits, from the chart type and the patch names
//! of its reference file (never from hard-coded colours).
//!
//! Positions are in grid units: patch centres one unit apart, the first corner patch at
//! (0, 0), x to the right and y down as the chart is read. The four corner patches are the
//! ones the user marks, in this order: top left, top right, bottom right, bottom left.
//!
//! - **ColorChecker 24** (Classic, Passport): 6 × 4. Ids 1–24 row by row from dark skin, or
//!   a letter and a number (A1–D6 or A1–F4: the axis with six values is the columns), or
//!   X-Rite's patch names. Corners: dark skin, bluish green, black, white.
//! - **ColorChecker SG**: 14 × 10, a letter and a number (the axis with 14 values is the
//!   columns).
//! - **IT8.7/2** (ISO 12641): rows A–L, columns 1–22, and the grey scale GS0–GS23 in a row
//!   below, 24 steps at the grid's pitch, centred under the columns. Ids that are not on
//!   the grid (Dmin, Dmax) are left out.
//! - **SpyderCHECKR** is refused: its values are Datacolor's.

use super::cgats::Reference;
use super::colour::Lab;

/// Chart types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChartKind {
    ColorChecker24,
    ColorCheckerSg,
    It8,
    /// Refused ([`LayoutError::Datacolor`]).
    SpyderCheckr,
}

impl ChartKind {
    pub const ALL: [ChartKind; 4] = [
        ChartKind::ColorChecker24,
        ChartKind::ColorCheckerSg,
        ChartKind::It8,
        ChartKind::SpyderCheckr,
    ];

    /// The chart a reference file is most likely for, by its patch count.
    pub fn guess(r: &Reference) -> Option<ChartKind> {
        let grid = |p: &&super::cgats::RefPatch| split(&p.id).is_some();
        match r.patches.len() {
            24 => Some(ChartKind::ColorChecker24),
            140 => Some(ChartKind::ColorCheckerSg),
            n if n >= 200 && r.patches.iter().filter(grid).count() >= 264 => Some(ChartKind::It8),
            _ => None,
        }
    }

    /// The HueSatMap divisions (hue, saturation) the solver fits for this chart: about one
    /// patch per cell.
    pub fn table_divisions(self) -> (u32, u32) {
        match self {
            ChartKind::ColorChecker24 | ChartKind::SpyderCheckr => (12, 4),
            ChartKind::ColorCheckerSg => (24, 6),
            ChartKind::It8 => (36, 8),
        }
    }
}

/// One patch on the chart.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub id: String,
    /// Centre, grid units.
    pub at: (f64, f64),
    /// Spacing to the neighbours along x and y, grid units (1 on the main grid; the IT8
    /// grey scale is narrower).
    pub pitch: (f64, f64),
    /// Reference L*a*b* (D50).
    pub reference: Lab,
}

/// A chart's patches.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub kind: ChartKind,
    pub cols: u32,
    pub rows: u32,
    pub patches: Vec<Patch>,
    /// Indices into `patches` of the corners to mark: top left, top right, bottom right,
    /// bottom left.
    pub corners: [usize; 4],
    /// Reference ids not on the chart's grid (left out).
    pub unplaced: Vec<String>,
}

/// Why a reference file does not fit a chart.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LayoutError {
    /// SpyderCHECKR: Datacolor's values are not used until there is a decision on them.
    Datacolor,
    /// The file has `found` patches; the chart has `expected`.
    Count { found: usize, expected: usize },
    /// The patch names do not form the chart's grid.
    Names,
    /// Corner patch `id` is not in the file.
    Corner { id: String },
}

impl std::fmt::Display for LayoutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LayoutError::Datacolor => write!(f, "SpyderCHECKR values are Datacolor's"),
            LayoutError::Count { found, expected } => {
                write!(f, "the file has {found} patches, the chart {expected}")
            }
            LayoutError::Names => write!(f, "the patch names do not match the chart"),
            LayoutError::Corner { id } => write!(f, "corner patch {id} is missing"),
        }
    }
}

impl std::error::Error for LayoutError {}

/// X-Rite's ColorChecker patch names, row by row.
pub const COLORCHECKER_NAMES: [&str; 24] = [
    "dark skin",
    "light skin",
    "blue sky",
    "foliage",
    "blue flower",
    "bluish green",
    "orange",
    "purplish blue",
    "moderate red",
    "purple",
    "yellow green",
    "orange yellow",
    "blue",
    "green",
    "red",
    "yellow",
    "magenta",
    "cyan",
    "white",
    "neutral 8",
    "neutral 6.5",
    "neutral 5",
    "neutral 3.5",
    "black",
];

/// `A01` → ("A", 1); `GS12` → ("GS", 12).
fn split(id: &str) -> Option<(String, u32)> {
    let id = id.trim();
    let n = id.chars().take_while(char::is_ascii_alphabetic).count();
    if n == 0 || n == id.len() {
        return None;
    }
    let num = id[n..].parse().ok()?;
    Some((id[..n].to_ascii_uppercase(), num))
}

fn letter_index(s: &str) -> Option<u32> {
    let mut c = s.chars();
    let ch = c.next()?;
    (c.next().is_none() && ch.is_ascii_uppercase()).then(|| u32::from(ch) - u32::from('A'))
}

impl Layout {
    /// The layout of `kind` with reference `r`.
    pub fn new(kind: ChartKind, r: &Reference) -> Result<Layout, LayoutError> {
        match kind {
            ChartKind::SpyderCheckr => Err(LayoutError::Datacolor),
            ChartKind::ColorChecker24 => grid(kind, r, 6, 4),
            ChartKind::ColorCheckerSg => grid(kind, r, 14, 10),
            ChartKind::It8 => it8(r),
        }
    }

    /// Whether patch `i` is a neutral (grey) patch: reference chroma below 3.
    pub fn is_neutral(&self, i: usize) -> bool {
        let l = self.patches[i].reference;
        l[1].hypot(l[2]) < 3.0
    }
}

/// A plain `cols × rows` chart.
fn grid(kind: ChartKind, r: &Reference, cols: u32, rows: u32) -> Result<Layout, LayoutError> {
    let n = (cols * rows) as usize;
    if r.patches.len() != n {
        return Err(LayoutError::Count {
            found: r.patches.len(),
            expected: n,
        });
    }
    // (col, row) of every patch.
    let mut cells: Vec<(u32, u32)> = Vec::with_capacity(n);
    let numbers: Option<Vec<u32>> = r.patches.iter().map(|p| p.id.trim().parse().ok()).collect();
    let names: Option<Vec<usize>> = (kind == ChartKind::ColorChecker24)
        .then(|| {
            r.patches
                .iter()
                .map(|p| {
                    COLORCHECKER_NAMES
                        .iter()
                        .position(|k| k.eq_ignore_ascii_case(p.id.trim()))
                })
                .collect::<Option<Vec<usize>>>()
        })
        .flatten();
    if let Some(nums) = numbers {
        for v in nums {
            if !(1..=n as u32).contains(&v) {
                return Err(LayoutError::Names);
            }
            cells.push(((v - 1) % cols, (v - 1) / cols));
        }
    } else if let Some(idx) = names {
        cells.extend(idx.iter().map(|&i| (i as u32 % cols, i as u32 / cols)));
    } else {
        let parts: Option<Vec<(u32, u32)>> = r
            .patches
            .iter()
            .map(|p| {
                let (l, num) = split(&p.id)?;
                Some((letter_index(&l)?, num.checked_sub(1)?))
            })
            .collect();
        let parts = parts.ok_or(LayoutError::Names)?;
        let lmax = parts.iter().map(|p| p.0).max().unwrap_or(0) + 1;
        let nmax = parts.iter().map(|p| p.1).max().unwrap_or(0) + 1;
        let letters_are_rows = if lmax == rows && nmax == cols {
            true
        } else if lmax == cols && nmax == rows {
            false
        } else {
            return Err(LayoutError::Names);
        };
        cells.extend(
            parts
                .iter()
                .map(|&(l, num)| if letters_are_rows { (num, l) } else { (l, num) }),
        );
    }
    let mut seen = vec![false; n];
    for &(c, rr) in &cells {
        let k = (rr * cols + c) as usize;
        if c >= cols || rr >= rows || std::mem::replace(&mut seen[k], true) {
            return Err(LayoutError::Names);
        }
    }
    let patches: Vec<Patch> = r
        .patches
        .iter()
        .zip(&cells)
        .map(|(p, &(c, rr))| Patch {
            id: p.id.clone(),
            at: (f64::from(c), f64::from(rr)),
            pitch: (1.0, 1.0),
            reference: p.lab,
        })
        .collect();
    let at = |c: u32, rr: u32| {
        cells
            .iter()
            .position(|&x| x == (c, rr))
            .ok_or(LayoutError::Names)
    };
    let corners = [
        at(0, 0)?,
        at(cols - 1, 0)?,
        at(cols - 1, rows - 1)?,
        at(0, rows - 1)?,
    ];
    Ok(Layout {
        kind,
        cols,
        rows,
        patches,
        corners,
        unplaced: Vec::new(),
    })
}

/// The IT8 grid, and its grey scale: a row of 24 steps at the main grid's pitch below
/// row L, centred under the 22 columns (GS0, the lightest, under column 1's side). Measured
/// on a Kodak Q-60 (the rawrest corpus card); the row's place is snapped per patch anyway
/// ([`super::sample::locate`]).
const IT8_ROWS: u32 = 12;
const IT8_COLS: u32 = 22;
const IT8_GS_ROW: f64 = 13.4;
const IT8_GS_STEPS: u32 = 24;

fn it8(r: &Reference) -> Result<Layout, LayoutError> {
    let mut patches = Vec::new();
    let mut unplaced = Vec::new();
    let gs_first = (f64::from(IT8_COLS) - f64::from(IT8_GS_STEPS)) / 2.0;
    for p in &r.patches {
        let place = split(&p.id).and_then(|(l, num)| {
            if l == "GS" {
                (num < IT8_GS_STEPS).then(|| ((gs_first + f64::from(num), IT8_GS_ROW), (1.0, 1.0)))
            } else {
                let row = letter_index(&l)?;
                (row < IT8_ROWS && (1..=IT8_COLS).contains(&num))
                    .then(|| ((f64::from(num - 1), f64::from(row)), (1.0, 1.0)))
            }
        });
        match place {
            Some((at, pitch)) => {
                if patches.iter().any(|q: &Patch| q.at == at) {
                    return Err(LayoutError::Names);
                }
                patches.push(Patch {
                    id: p.id.clone(),
                    at,
                    pitch,
                    reference: p.lab,
                });
            }
            None => unplaced.push(p.id.clone()),
        }
    }
    if patches.len() < 200 {
        return Err(LayoutError::Count {
            found: patches.len(),
            expected: (IT8_ROWS * IT8_COLS + IT8_GS_STEPS) as usize,
        });
    }
    let find = |c: f64, rr: f64, id: &str| {
        patches
            .iter()
            .position(|p| p.at == (c, rr))
            .ok_or(LayoutError::Corner { id: id.to_owned() })
    };
    let (c1, r1) = (f64::from(IT8_COLS - 1), f64::from(IT8_ROWS - 1));
    let corners = [
        find(0.0, 0.0, "A01")?,
        find(c1, 0.0, "A22")?,
        find(c1, r1, "L22")?,
        find(0.0, r1, "L01")?,
    ];
    Ok(Layout {
        kind: ChartKind::It8,
        cols: IT8_COLS,
        rows: IT8_ROWS,
        patches,
        corners,
        unplaced,
    })
}

#[cfg(test)]
mod tests {
    use super::super::cgats::RefPatch;
    use super::*;

    fn refs(ids: impl IntoIterator<Item = String>) -> Reference {
        Reference {
            title: String::new(),
            patches: ids
                .into_iter()
                .enumerate()
                .map(|(i, id)| RefPatch {
                    id,
                    lab: [50.0, i as f64, 0.0],
                })
                .collect(),
        }
    }

    #[test]
    fn colorchecker_ids_in_every_style() {
        let numeric = refs((1..=24).map(|i| i.to_string()));
        let rows_first = refs((0..24).map(|i| format!("{}{}", (b'A' + i / 6) as char, i % 6 + 1)));
        let cols_first = refs((0..24).map(|i| format!("{}{}", (b'A' + i % 6) as char, i / 6 + 1)));
        let names = refs(COLORCHECKER_NAMES.iter().map(|s| s.to_string()));
        for r in [numeric, rows_first, cols_first, names] {
            let l = Layout::new(ChartKind::ColorChecker24, &r).unwrap();
            assert_eq!(l.patches[5].at, (5.0, 0.0), "bluish green");
            assert_eq!(l.patches[18].at, (0.0, 3.0), "white");
            assert_eq!(l.corners, [0, 5, 23, 18]);
        }
        assert_eq!(
            Layout::new(
                ChartKind::ColorChecker24,
                &refs((1..=23).map(|i| i.to_string()))
            ),
            Err(LayoutError::Count {
                found: 23,
                expected: 24
            })
        );
        assert_eq!(
            Layout::new(
                ChartKind::SpyderCheckr,
                &refs((1..=24).map(|i| i.to_string()))
            ),
            Err(LayoutError::Datacolor)
        );
    }

    #[test]
    fn sg_and_it8_layouts_from_their_ids() {
        let sg = refs((0..140).map(|i| format!("{}{}", (b'A' + i % 14) as char, i / 14 + 1)));
        assert_eq!(ChartKind::guess(&sg), Some(ChartKind::ColorCheckerSg));
        let l = Layout::new(ChartKind::ColorCheckerSg, &sg).unwrap();
        assert_eq!(l.patches[l.corners[2]].at, (13.0, 9.0));
        let mut ids: Vec<String> = (0..12u8)
            .flat_map(|r| (1..=22).map(move |c| format!("{}{c:02}", (b'A' + r) as char)))
            .collect();
        ids.extend((0..24).map(|g| format!("GS{g}")));
        ids.push("Dmax".into());
        let it8 = refs(ids);
        assert_eq!(ChartKind::guess(&it8), Some(ChartKind::It8));
        let l = Layout::new(ChartKind::It8, &it8).unwrap();
        assert_eq!(l.patches.len(), 288);
        assert_eq!(l.unplaced, ["Dmax"]);
        assert_eq!(l.patches[l.corners[1]].id, "A22");
        assert_eq!(l.patches[l.corners[3]].id, "L01");
    }
}
