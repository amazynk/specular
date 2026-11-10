//! `specular-fcs`: Flow Cytometry Standard list-mode files (FCS 2.0, 3.0, 3.1 and 3.2).
//!
//! A file is one or more datasets, each a HEADER (version, then the byte offsets of its
//! segments as eight-character ASCII fields), a TEXT segment of keyword/value pairs (the
//! first byte is the delimiter; a doubled delimiter is one literal delimiter), a DATA
//! segment and an optional ANALYSIS segment. `$NEXTDATA` gives the next dataset's offset
//! from the current one's start.
//!
//! * Offsets past 99,999,999 are written as zero in the HEADER and given by `$BEGINDATA`,
//!   `$ENDDATA` (and `$BEGINSTEXT`/`$ENDSTEXT` for supplemental TEXT,
//!   `$BEGINANALYSIS`/`$ENDANALYSIS`). When the HEADER and TEXT disagree, the pair that
//!   holds exactly the events' bytes wins.
//! * List mode (`$MODE L`) only; the deprecated histogram modes are refused by name.
//! * `$DATATYPE I`: unsigned integers of each parameter's own width (`$PnB`, 8 to 64 bits),
//!   masked to the range (`$PnR`) rounded up to a power of two, as acquisition software
//!   leaves high bits set. `F` and `D`: IEEE floats and doubles. `A`: ASCII, fixed width
//!   per parameter or (`$PnB` `*`) separated by blanks or commas. `$BYTEORD` is any
//!   permutation (`1,2,3,4` little-endian, `4,3,2,1` big-endian, `3,4,1,2`…).
//! * Scale values ([`Dataset::read_events`]): `$PnE f1,f2` with `f1 > 0` is log amplified,
//!   `f2 · 10^(f1 · channel / $PnR)` (`f2 = 0` read as 1), then divided by the gain
//!   `$PnG`; the `Time` parameter is first multiplied by `$TIMESTEP` (seconds). This is
//!   what FlowIO and FlowKit do (and so for float data that states a `$PnE` too).
//! * Compensation: `$SPILLOVER` (3.1+), `SPILL` / `$SPILL` (earlier BD and others): `n`, the
//!   `n` parameter names (or numbers), then the spillover matrix by rows; compensated
//!   values are the measured ones times its inverse. `$COMP` (3.0) is reported, not applied.
//!
//! [`scale`] gives the plot axes (linear, log, logicle, arcsinh) and the density plots and
//! histograms drawn through them. Nothing here panics on malformed input; sizes are
//! checked before anything is allocated.

pub mod scale;

/// Most datasets read from one file.
pub const MAX_DATASETS: usize = 1024;
/// Most parameters of a dataset.
pub const MAX_PARAMS: usize = 4096;
/// Most values (events × parameters) decoded at once.
pub const MAX_VALUES: u64 = 1 << 28;
/// Longest TEXT segment parsed.
const MAX_TEXT: usize = 64 << 20;

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum Error {
    #[error("not an FCS file")]
    NotFcs,
    #[error("truncated: {0}")]
    Truncated(String),
    #[error("malformed: {0}")]
    Malformed(String),
    #[error("unsupported: {0}")]
    Unsupported(String),
    #[error("too large")]
    TooLarge,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Whether `head` starts like an FCS file: `FCS` and a version, then spaces.
pub fn sniff(head: &[u8]) -> bool {
    head.len() >= 10
        && head.starts_with(b"FCS")
        && head[3].is_ascii_digit()
        && head[4] == b'.'
        && head[5].is_ascii_digit()
        && head[6..10].iter().all(|&b| b == b' ')
}

/// How DATA stores a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    Integer,
    Float,
    Double,
    Ascii,
}

impl DataType {
    pub fn name(self) -> &'static str {
        match self {
            DataType::Integer => "integer",
            DataType::Float => "32-bit float",
            DataType::Double => "64-bit float",
            DataType::Ascii => "ASCII",
        }
    }
}

/// One parameter (a detector channel or a derived value).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Param {
    /// `$PnN`.
    pub name: String,
    /// `$PnS`, the stain or long name.
    pub label: String,
    /// `$PnB` (0 for ASCII values separated by delimiters).
    pub bits: u32,
    /// `$PnR`.
    pub range: f64,
    /// `$PnE` (decades, offset); `(0, 0)` linear.
    pub log: (f64, f64),
    /// `$PnG`.
    pub gain: Option<f64>,
    /// `$PnD`: the writer's preferred display (`Linear` / `Logarithmic`), as written.
    pub display: Option<String>,
    /// `$PnV` detector voltage.
    pub voltage: Option<f64>,
}

impl Param {
    /// Log amplified (`$PnE` decades above zero).
    pub fn log_amplified(&self) -> bool {
        self.log.0 > 0.0
    }

    /// The name shown: `$PnN`, with `$PnS` after it when it differs.
    pub fn title(&self) -> String {
        if self.label.trim().is_empty() || self.label.trim() == self.name.trim() {
            self.name.trim().to_owned()
        } else {
            format!("{} ({})", self.name.trim(), self.label.trim())
        }
    }

    /// The top of the parameter's scale in scale values: the range through its log
    /// amplification or gain.
    pub fn scale_top(&self) -> f64 {
        let top = if self.log_amplified() {
            let f2 = if self.log.1 > 0.0 { self.log.1 } else { 1.0 };
            f2 * 10f64.powf(self.log.0)
        } else {
            self.range
        };
        match self.gain {
            Some(g) if g > 0.0 && g != 1.0 => top / g,
            _ => top,
        }
    }
}

/// A spillover matrix and the parameters it applies to.
#[derive(Clone, Debug, PartialEq)]
pub struct Spillover {
    /// The keyword it came from (`$SPILLOVER`, `SPILL`, `$SPILL`).
    pub keyword: String,
    /// Parameter indices, in matrix order.
    pub params: Vec<usize>,
    /// Row-major, `params.len()` squared: row `i` is fluorochrome `i`'s spill into each
    /// detector.
    pub matrix: Vec<f64>,
}

/// One dataset of a file.
#[derive(Clone, Debug)]
pub struct Dataset {
    /// `FCS3.1`.
    pub version: String,
    /// Offset of the dataset in the file.
    pub base: usize,
    /// TEXT (and supplemental TEXT) keywords in file order, as written.
    pub text: Vec<(String, String)>,
    /// ANALYSIS keywords.
    pub analysis: Vec<(String, String)>,
    pub params: Vec<Param>,
    /// `$TOT` (or the events the DATA segment holds).
    pub events: usize,
    pub datatype: DataType,
    /// Significance (1 the least) of each stored byte, from `$BYTEORD`.
    pub byte_order: Vec<u8>,
    /// The DATA segment's bytes in the file.
    pub data: (usize, usize),
    pub spillover: Option<Spillover>,
    /// A compensation keyword present but not applied (`$COMP`), with why.
    pub compensation_note: Option<String>,
    /// The DATA segment holds fewer bytes than the events need.
    pub truncated: bool,
}

/// The events of a dataset: `count` rows of `params` values (event-major).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Events {
    pub params: usize,
    pub count: usize,
    pub values: Vec<f32>,
    /// Compensation was applied.
    pub compensated: bool,
}

impl Events {
    #[inline]
    pub fn get(&self, event: usize, param: usize) -> f32 {
        self.values[event * self.params + param]
    }

    /// Parameter `p`'s values.
    pub fn column(&self, p: usize) -> impl Iterator<Item = f32> + '_ {
        self.values
            .iter()
            .skip(p)
            .step_by(self.params.max(1))
            .copied()
    }
}

/// A parsed FCS file.
#[derive(Clone, Debug)]
pub struct Fcs {
    pub datasets: Vec<Dataset>,
}

fn field(h: &[u8], at: usize) -> Result<usize> {
    let f = h
        .get(at..at + 8)
        .ok_or_else(|| Error::Truncated("FCS header".into()))?;
    let s = std::str::from_utf8(f)
        .map_err(|_| Error::Malformed("FCS header offset".into()))?
        .trim();
    if s.is_empty() {
        return Ok(0);
    }
    s.parse::<i64>()
        .ok()
        .and_then(|v| usize::try_from(v.max(0)).ok())
        .ok_or_else(|| Error::Malformed(format!("FCS header offset {s:?}")))
}

/// Keyword/value pairs of a TEXT (or ANALYSIS) segment.
pub fn parse_text(seg: &[u8]) -> Vec<(String, String)> {
    let Some((&delim, body)) = seg.split_first() else {
        return Vec::new();
    };
    let mut tokens: Vec<Vec<u8>> = Vec::new();
    let mut cur = Vec::new();
    let mut i = 0;
    while i < body.len() {
        let b = body[i];
        if b == delim {
            if body.get(i + 1) == Some(&delim) && !cur.is_empty() {
                // An escaped delimiter inside a keyword or value.
                cur.push(delim);
                i += 2;
                continue;
            }
            tokens.push(std::mem::take(&mut cur));
            i += 1;
            continue;
        }
        cur.push(b);
        i += 1;
    }
    if !cur.iter().all(|b| b.is_ascii_whitespace() || *b == 0) {
        tokens.push(cur);
    }
    let text = |b: &[u8]| -> String {
        match std::str::from_utf8(b) {
            Ok(s) => s.to_owned(),
            // Older files are Latin-1.
            Err(_) => b.iter().map(|&c| char::from(c)).collect(),
        }
    };
    tokens
        .chunks(2)
        .filter(|kv| kv.len() == 2)
        .map(|kv| (text(&kv[0]).trim().to_owned(), text(&kv[1])))
        .filter(|(k, _)| !k.is_empty())
        .collect()
}

/// Keyword lookup, case-insensitive (3.x keywords are; 2.0 writers vary).
struct Keys<'a>(&'a [(String, String)]);

impl<'a> Keys<'a> {
    fn get(&self, k: &str) -> Option<&'a str> {
        self.0
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.trim())
    }
    fn num(&self, k: &str) -> Option<f64> {
        self.get(k)?
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|v| v.is_finite())
    }
    fn uint(&self, k: &str) -> Option<usize> {
        let v = self.num(k)?;
        (0.0..1e18).contains(&v).then_some(v as usize)
    }
}

/// Parse every dataset of an FCS file.
pub fn parse(data: &[u8]) -> Result<Fcs> {
    if !sniff(data) {
        return Err(Error::NotFcs);
    }
    let mut datasets = Vec::new();
    let mut base = 0usize;
    loop {
        let ds = parse_dataset(data, base)?;
        let next = Keys(&ds.text).uint("$NEXTDATA").unwrap_or(0);
        datasets.push(ds);
        if next == 0 || datasets.len() >= MAX_DATASETS {
            break;
        }
        let Some(nb) = base.checked_add(next).filter(|&b| b < data.len()) else {
            break;
        };
        if !sniff(&data[nb..]) {
            break;
        }
        base = nb;
    }
    Ok(Fcs { datasets })
}

fn segment(data: &[u8], base: usize, a: usize, b: usize) -> Option<&[u8]> {
    if b < a || (a == 0 && b == 0) {
        return None;
    }
    let s = base.checked_add(a)?;
    let e = base.checked_add(b)?.checked_add(1)?.min(data.len());
    data.get(s..e)
}

fn parse_dataset(data: &[u8], base: usize) -> Result<Dataset> {
    let h = &data[base..];
    let version = String::from_utf8_lossy(&h[..6]).into_owned();
    if !matches!(&h[3..6], b"2.0" | b"3.0" | b"3.1" | b"3.2") {
        return Err(Error::Unsupported(format!("{version} files")));
    }
    let (t0, t1) = (field(h, 10)?, field(h, 18)?);
    let (d0, d1) = (field(h, 26)?, field(h, 34)?);
    let (a0, a1) = (field(h, 42).unwrap_or(0), field(h, 50).unwrap_or(0));
    if t1 < t0 || t1 - t0 > MAX_TEXT {
        return Err(Error::Malformed("FCS TEXT segment offsets".into()));
    }
    let text_seg =
        segment(data, base, t0, t1).ok_or_else(|| Error::Truncated("FCS TEXT segment".into()))?;
    let mut text = parse_text(text_seg);
    // Supplemental TEXT.
    let k = Keys(&text);
    if let (Some(s0), Some(s1)) = (k.uint("$BEGINSTEXT"), k.uint("$ENDSTEXT"))
        && s1 > s0
        && s1 - s0 <= MAX_TEXT
        && let Some(seg) = segment(data, base, s0, s1)
        && (s0 > t1 || s1 < t0)
    {
        let extra = parse_text(seg);
        text.extend(extra);
    }
    let k = Keys(&text);
    let mode = k.get("$MODE").unwrap_or("L").to_ascii_uppercase();
    if mode != "L" {
        return Err(Error::Unsupported(format!(
            "FCS histogram data ($MODE {mode}); only list mode is read"
        )));
    }
    let npar = k
        .uint("$PAR")
        .ok_or_else(|| Error::Malformed("FCS without $PAR".into()))?;
    if npar == 0 || npar > MAX_PARAMS {
        return Err(Error::Unsupported(format!("{npar} FCS parameters")));
    }
    let datatype = match k
        .get("$DATATYPE")
        .map(|s| s.to_ascii_uppercase())
        .as_deref()
    {
        Some("I") => DataType::Integer,
        Some("F") => DataType::Float,
        Some("D") => DataType::Double,
        Some("A") => DataType::Ascii,
        other => {
            return Err(Error::Unsupported(format!(
                "FCS data type {}",
                other.unwrap_or("(none)")
            )));
        }
    };
    let mut params = Vec::with_capacity(npar);
    for n in 1..=npar {
        let key = |s: &str| format!("$P{n}{s}");
        let bits_s = k.get(&key("B")).unwrap_or("");
        let bits = if bits_s == "*" {
            0
        } else {
            bits_s
                .parse::<u32>()
                .map_err(|_| Error::Malformed(format!("FCS $P{n}B {bits_s:?}")))?
        };
        let log = k
            .get(&key("E"))
            .map(|e| {
                let mut it = e.split(',').map(|v| v.trim().parse::<f64>().unwrap_or(0.0));
                (it.next().unwrap_or(0.0), it.next().unwrap_or(0.0))
            })
            .unwrap_or((0.0, 0.0));
        let range = k.num(&key("R")).unwrap_or(0.0);
        params.push(Param {
            name: k.get(&key("N")).unwrap_or("").to_owned(),
            label: k.get(&key("S")).unwrap_or("").to_owned(),
            bits,
            range,
            log: if log.0.is_finite() && log.1.is_finite() && log.0 > 0.0 {
                log
            } else {
                (0.0, 0.0)
            },
            gain: k.num(&key("G")).filter(|g| *g > 0.0),
            display: k.get(&key("D")).map(str::to_owned),
            voltage: k.num(&key("V")),
        });
        let p = params.last_mut().unwrap_or_else(|| unreachable!());
        if p.name.is_empty() {
            p.name = format!("P{n}");
        }
        match datatype {
            DataType::Integer if !matches!(bits, 8 | 16 | 24 | 32 | 64) => {
                return Err(Error::Unsupported(format!(
                    "{bits}-bit packed FCS integers"
                )));
            }
            DataType::Float if bits != 32 => {
                return Err(Error::Malformed(format!("FCS float with $P{n}B {bits}")));
            }
            DataType::Double if bits != 64 => {
                return Err(Error::Malformed(format!("FCS double with $P{n}B {bits}")));
            }
            _ => {}
        }
    }
    let byte_order: Vec<u8> = k
        .get("$BYTEORD")
        .unwrap_or("1,2,3,4")
        .split(',')
        .filter_map(|v| v.trim().parse::<u8>().ok())
        .collect();
    let event_bytes: Option<usize> = match datatype {
        DataType::Ascii => None,
        _ => params
            .iter()
            .try_fold(0usize, |a, p| a.checked_add(p.bits as usize / 8)),
    };
    // DATA: the HEADER's offsets, else (or when they do not hold the events) TEXT's.
    let tot = k.uint("$TOT");
    let header_pair = (d0, d1);
    let text_pair = (
        k.uint("$BEGINDATA").unwrap_or(0),
        k.uint("$ENDDATA").unwrap_or(0),
    );
    let holds = |(a, b): (usize, usize)| -> bool {
        match (event_bytes, tot) {
            (Some(eb), Some(t)) if b >= a && (a, b) != (0, 0) => {
                eb.checked_mul(t).is_some_and(|n| n == b - a + 1)
            }
            _ => false,
        }
    };
    let (da, db) = if holds(header_pair) {
        header_pair
    } else if holds(text_pair) {
        text_pair
    } else if header_pair != (0, 0) {
        header_pair
    } else {
        text_pair
    };
    let seg_len = if db >= da && (da, db) != (0, 0) {
        db - da + 1
    } else {
        0
    };
    let avail = data.len().saturating_sub(base.saturating_add(da));
    let events = match (tot, event_bytes) {
        (Some(t), _) => t,
        (None, Some(eb)) if eb > 0 => seg_len.min(avail) / eb,
        _ => 0,
    };
    let truncated = event_bytes
        .and_then(|eb| eb.checked_mul(events))
        .is_some_and(|need| need > seg_len.min(avail));
    let analysis = {
        let (aa, ab) = if (a0, a1) != (0, 0) {
            (a0, a1)
        } else {
            (
                k.uint("$BEGINANALYSIS").unwrap_or(0),
                k.uint("$ENDANALYSIS").unwrap_or(0),
            )
        };
        if ab > aa && ab - aa <= MAX_TEXT {
            segment(data, base, aa, ab)
                .map(parse_text)
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    };
    let (spillover, compensation_note) = spill(&k, &params);
    Ok(Dataset {
        version,
        base,
        text,
        analysis,
        params,
        events,
        datatype,
        byte_order,
        data: (base.saturating_add(da), seg_len),
        spillover,
        compensation_note,
        truncated,
    })
}

/// The spillover matrix of a dataset, or a note on a compensation keyword not applied.
fn spill(k: &Keys, params: &[Param]) -> (Option<Spillover>, Option<String>) {
    for key in ["$SPILLOVER", "SPILL", "$SPILL", "SPILLOVER"] {
        let Some(v) = k.get(key) else { continue };
        let parts: Vec<&str> = v.split(',').map(str::trim).collect();
        let Some(n) = parts.first().and_then(|n| n.parse::<usize>().ok()) else {
            return (None, Some(format!("{key} is malformed")));
        };
        if n == 0 || n > params.len() || parts.len() != 1 + n + n * n {
            return (None, Some(format!("{key} is malformed")));
        }
        let mut idx = Vec::with_capacity(n);
        for name in &parts[1..=n] {
            let found = params
                .iter()
                .position(|p| p.name.trim().eq_ignore_ascii_case(name))
                .or_else(|| {
                    params
                        .iter()
                        .position(|p| p.label.trim().eq_ignore_ascii_case(name))
                })
                .or_else(|| {
                    name.parse::<usize>()
                        .ok()
                        .filter(|&i| i >= 1 && i <= params.len())
                        .map(|i| i - 1)
                });
            match found {
                Some(i) if !idx.contains(&i) => idx.push(i),
                _ => {
                    return (
                        None,
                        Some(format!("{key} names an unknown parameter {name}")),
                    );
                }
            }
        }
        let matrix: Option<Vec<f64>> = parts[1 + n..]
            .iter()
            .map(|v| v.parse::<f64>().ok().filter(|v| v.is_finite()))
            .collect();
        let Some(matrix) = matrix else {
            return (None, Some(format!("{key} is malformed")));
        };
        return (
            Some(Spillover {
                keyword: key.to_owned(),
                params: idx,
                matrix,
            }),
            None,
        );
    }
    if k.get("$COMP").is_some() {
        return (None, Some("$COMP (FCS 3.0) is not applied".into()));
    }
    (None, None)
}

/// The inverse of an `n`×`n` row-major matrix (Gauss–Jordan, partial pivoting); `None` when
/// singular.
pub fn invert(m: &[f64], n: usize) -> Option<Vec<f64>> {
    let mut a = m.to_vec();
    let mut inv = vec![0.0; n * n];
    for i in 0..n {
        inv[i * n + i] = 1.0;
    }
    for c in 0..n {
        let p = (c..n).max_by(|&x, &y| a[x * n + c].abs().total_cmp(&a[y * n + c].abs()))?;
        if a[p * n + c].abs() < 1e-12 {
            return None;
        }
        if p != c {
            for j in 0..n {
                a.swap(p * n + j, c * n + j);
                inv.swap(p * n + j, c * n + j);
            }
        }
        let d = a[c * n + c];
        for j in 0..n {
            a[c * n + j] /= d;
            inv[c * n + j] /= d;
        }
        for r in 0..n {
            if r != c {
                let f = a[r * n + c];
                if f != 0.0 {
                    for j in 0..n {
                        a[r * n + j] -= f * a[c * n + j];
                        inv[r * n + j] -= f * inv[c * n + j];
                    }
                }
            }
        }
    }
    Some(inv)
}

impl Dataset {
    /// A keyword's value (case-insensitive).
    pub fn keyword(&self, k: &str) -> Option<&str> {
        Keys(&self.text).get(k)
    }

    /// The stored channel values (no amplification or gain), event-major.
    pub fn read_channels(&self, file: &[u8]) -> Result<Events> {
        let n = self.params.len();
        let total = (n as u64)
            .checked_mul(self.events as u64)
            .ok_or(Error::TooLarge)?;
        if total > MAX_VALUES {
            return Err(Error::TooLarge);
        }
        let seg = file
            .get(self.data.0..)
            .map(|s| &s[..s.len().min(self.data.1)])
            .unwrap_or(&[]);
        let mut values = Vec::new();
        values
            .try_reserve_exact(total as usize)
            .map_err(|_| Error::TooLarge)?;
        match self.datatype {
            DataType::Ascii => self.read_ascii(seg, &mut values)?,
            _ => self.read_binary(seg, &mut values),
        }
        // Events the segment does not hold read as zero.
        values.resize(total as usize, 0.0);
        Ok(Events {
            params: n,
            count: self.events,
            values,
            compensated: false,
        })
    }

    fn read_binary(&self, seg: &[u8], out: &mut Vec<f32>) {
        let widths: Vec<usize> = self.params.iter().map(|p| p.bits as usize / 8).collect();
        let eb: usize = widths.iter().sum();
        if eb == 0 {
            return;
        }
        // Byte permutation per width: for the order's own width the order itself, for
        // narrower values little- or big-endian as the order starts.
        let little = self.byte_order.first().is_none_or(|&b| b == 1);
        let perms: Vec<Vec<usize>> = widths
            .iter()
            .map(|&w| {
                if self.byte_order.len() == w {
                    let mut p: Vec<usize> = (0..w).collect();
                    for (stored, &sig) in self.byte_order.iter().enumerate() {
                        if let Some(slot) = p.get_mut(usize::from(sig).wrapping_sub(1)) {
                            *slot = stored;
                        }
                    }
                    p
                } else if little {
                    (0..w).collect()
                } else {
                    (0..w).rev().collect()
                }
            })
            .collect();
        let masks: Vec<u64> = self
            .params
            .iter()
            .map(|p| {
                let full = if p.bits >= 64 {
                    u64::MAX
                } else {
                    (1u64 << p.bits) - 1
                };
                if self.datatype == DataType::Integer && p.range >= 1.0 && p.range < 1.8e19 {
                    let r = (p.range.ceil() as u64).max(1);
                    let m = r.checked_next_power_of_two().map_or(u64::MAX, |v| v - 1);
                    m.min(full)
                } else {
                    full
                }
            })
            .collect();
        for ev in seg.chunks_exact(eb).take(self.events) {
            let mut at = 0;
            for (k, &w) in widths.iter().enumerate() {
                let b = &ev[at..at + w];
                at += w;
                let mut le = [0u8; 8];
                for (sig, &i) in perms[k].iter().enumerate() {
                    le[sig] = b[i];
                }
                let v = match self.datatype {
                    DataType::Float => f32::from_le_bytes([le[0], le[1], le[2], le[3]]),
                    DataType::Double => f64::from_le_bytes(le) as f32,
                    _ => (u64::from_le_bytes(le) & masks[k]) as f32,
                };
                out.push(v);
            }
        }
    }

    fn read_ascii(&self, seg: &[u8], out: &mut Vec<f32>) -> Result<()> {
        let n = self.params.len();
        let want = n * self.events;
        if self.params.iter().all(|p| p.bits > 0) {
            // Fixed width: each value its `$PnB` characters.
            let widths: Vec<usize> = self.params.iter().map(|p| p.bits as usize).collect();
            let eb: usize = widths.iter().sum();
            for ev in seg.chunks_exact(eb).take(self.events) {
                let mut at = 0;
                for &w in &widths {
                    let s = std::str::from_utf8(&ev[at..at + w]).unwrap_or("");
                    at += w;
                    out.push(s.trim().parse::<f32>().unwrap_or(0.0));
                }
            }
        } else {
            for tok in seg
                .split(|b| b.is_ascii_whitespace() || *b == b',')
                .filter(|t| !t.is_empty())
                .take(want)
            {
                let s = std::str::from_utf8(tok).unwrap_or("");
                out.push(s.parse::<f32>().unwrap_or(0.0));
            }
        }
        Ok(())
    }

    /// The scale values: channel values through `$PnE` log amplification or `$PnG` gain,
    /// compensated when `compensate` and the dataset has a spillover matrix.
    pub fn read_events(&self, file: &[u8], compensate: bool) -> Result<Events> {
        let mut ev = self.read_channels(file)?;
        let n = ev.params;
        if n == 0 {
            return Ok(ev);
        }
        let timestep = Keys(&self.text).num("$TIMESTEP").filter(|t| *t > 0.0);
        for (k, p) in self.params.iter().enumerate() {
            // As FlowIO and FlowKit do: the time parameter in seconds, then $PnE (also on
            // float data that states it), then the gain.
            let time = p.name.trim().eq_ignore_ascii_case("time");
            if let Some(ts) = timestep.filter(|_| time) {
                for row in ev.values.chunks_exact_mut(n) {
                    row[k] = (f64::from(row[k]) * ts) as f32;
                }
            }
            if p.log_amplified() && p.range > 0.0 {
                let f2 = if p.log.1 > 0.0 { p.log.1 } else { 1.0 };
                let (f1, r) = (p.log.0, p.range);
                for row in ev.values.chunks_exact_mut(n) {
                    row[k] = (f2 * 10f64.powf(f1 * f64::from(row[k]) / r)) as f32;
                }
            }
            // A gain on the time channel is no amplifier's: ignored.
            if let Some(g) = p.gain.filter(|g| *g != 1.0 && !time) {
                for row in ev.values.chunks_exact_mut(n) {
                    row[k] = (f64::from(row[k]) / g) as f32;
                }
            }
        }
        if compensate && let Some(s) = &self.spillover {
            let m = s.params.len();
            let inv = invert(&s.matrix, m)
                .ok_or_else(|| Error::Malformed(format!("{} is singular", s.keyword)))?;
            let mut x = vec![0f64; m];
            for row in ev.values.chunks_exact_mut(n) {
                for (i, &p) in s.params.iter().enumerate() {
                    x[i] = f64::from(row[p]);
                }
                for (j, &p) in s.params.iter().enumerate() {
                    let mut acc = 0.0;
                    for i in 0..m {
                        acc += x[i] * inv[i * m + j];
                    }
                    row[p] = acc as f32;
                }
            }
            ev.compensated = true;
        }
        Ok(ev)
    }
}

/// An FCS file of one dataset (test fixtures and the oracle generator's twin): `text`
/// keywords (delimiter `/`), `data` the DATA segment.
pub fn write_dataset(text: &[(&str, String)], data: &[u8]) -> Vec<u8> {
    let mut pairs: Vec<(String, String)> = text
        .iter()
        .map(|(k, v)| ((*k).to_owned(), v.clone()))
        .collect();
    pairs.push(("$BEGINDATA".into(), String::new()));
    pairs.push(("$ENDDATA".into(), String::new()));
    // Offsets are fixed width so the TEXT length does not depend on them.
    let render = |pairs: &[(String, String)], d0: usize, d1: usize| {
        let mut s = String::from("/");
        for (k, v) in pairs {
            let v = match k.as_str() {
                "$BEGINDATA" => format!("{d0:0>12}"),
                "$ENDDATA" => format!("{d1:0>12}"),
                _ => v.replace('/', "//"),
            };
            s += &format!("{k}/{v}/");
        }
        s
    };

    let probe = render(&pairs, 0, 0);
    let t0 = 58;
    let t1 = t0 + probe.len() - 1;
    let d0 = t1 + 1;
    let d1 = d0 + data.len().max(1) - 1;
    let text_s = render(&pairs, d0, d1);
    let mut out = format!("FCS3.1    {t0:>8}{t1:>8}{d0:>8}{d1:>8}{:>8}{:>8}", 0, 0).into_bytes();
    out.extend_from_slice(text_s.as_bytes());
    out.extend_from_slice(data);
    out
}

#[cfg(test)]
mod tests;
