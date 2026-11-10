//! NI TDMS measurement files through `specular-tdms`, on the array session
//! ([`crate::array_dec`]).
//!
//! * A group with two or more numeric channels of one length is a page of its own: an image
//!   with a row per channel and the samples across (binned to [`MAX_STRIP`] columns by their
//!   mean), the channel under the pointer plotted in full.
//! * Every numeric channel is a page too: a strip of its samples whose plot is the channel,
//!   against time when the channel has waveform properties (`wf_increment`).
//! * Properties of the file, the group and the channel are the header keywords; text
//!   channels list their first values there.
//!
//! A `.tdms_index` file is opened as the `.tdms` beside it.

use std::path::PathBuf;
use std::sync::Arc;

use specular_arrays::{Part, PlaneSpec};
use specular_core::Source;
use specular_tdms::{self as tdms, Channel, Tdms};

use crate::array_dec::{self, Convention, MAX_STRIP, Page, PlaneRead, Shared};
use crate::science::Linear;
use crate::{DecodeError, Decoder, Format, Result, Session};

fn map_err(e: tdms::Error) -> DecodeError {
    match e {
        tdms::Error::NotTdms => DecodeError::UnsupportedFormat,
        tdms::Error::Truncated(s) => DecodeError::Truncated(s),
        tdms::Error::Malformed(s) => DecodeError::Malformed(s),
        tdms::Error::Unsupported(s) => DecodeError::Unsupported(s),
        tdms::Error::TooLarge => DecodeError::TooLarge,
    }
}

/// Samples of a channel read for one plot or plane at most.
const MAX_SAMPLES: u64 = 1 << 28;
/// Values listed from a text channel.
const MAX_STRINGS: usize = 8;

pub struct TdmsDecoder;

impl Decoder for TdmsDecoder {
    fn format(&self) -> Format {
        Format::Tdms
    }
    fn probe(&self, head: &[u8]) -> bool {
        tdms::sniff(head)
    }
    fn extensions(&self) -> &'static [&'static str] {
        &["tdms"]
    }
    fn open(&self, source: Arc<Source>) -> Result<Arc<dyn Session>> {
        let source = if source.bytes().starts_with(b"TDSh") {
            // An index file: the data is in the .tdms beside it.
            let p = source.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let data = name
                .strip_suffix("_index")
                .map(|n| p.with_file_name(n))
                .filter(|d: &PathBuf| d.is_file())
                .ok_or_else(|| {
                    DecodeError::Unsupported(
                        "this is a TDMS index file; its .tdms file is missing".into(),
                    )
                })?;
            Arc::new(Source::open(data)?)
        } else {
            source
        };
        open(source)
    }
}

/// A channel, or the equal-length channels of a group, read as values.
struct ChannelsRead {
    source: Arc<Source>,
    file: Arc<Tdms>,
    group: usize,
    channels: Vec<usize>,
    len: u64,
    /// Samples per column of a group page's image (1 for a channel page).
    bin: u64,
}

impl ChannelsRead {
    fn channel(&self, k: usize) -> Result<&Channel> {
        self.channels
            .get(k)
            .and_then(|&c| self.file.groups[self.group].channels.get(c))
            .ok_or_else(|| DecodeError::Malformed("no such channel".into()))
    }

    /// The whole channel `k` in `part`.
    fn series(&self, k: usize, part: Part) -> Result<Vec<f32>> {
        let ch = self.channel(k)?;
        if self.len > MAX_SAMPLES {
            return Err(DecodeError::TooLarge);
        }
        let n = self.len as usize;
        let _r = specular_core::budget::reserve(self.len.saturating_mul(16))?;
        let data = self.source.bytes();
        let mut v = vec![0f64; n];
        if matches!(
            ch.dtype,
            tdms::DataType::ComplexF32 | tdms::DataType::ComplexF64
        ) {
            let mut im = vec![0f64; n];
            ch.read_complex(data, 0, &mut v, &mut im).map_err(map_err)?;
            return Ok(v.iter().zip(&im).map(|(&r, &i)| part.of(r, i)).collect());
        }
        ch.read(data, 0, &mut v).map_err(map_err)?;
        Ok(v.into_iter().map(|x| x as f32).collect())
    }
}

impl PlaneRead for ChannelsRead {
    fn read(&self, spec: &PlaneSpec, part: Part, out: &mut Vec<f32>) -> Result<bool> {
        match spec.y_dim {
            // A whole series: a channel page's line, or one row of a group page (the plot).
            None => {
                let k = if self.channels.len() > 1 {
                    spec.index.first().copied().unwrap_or(0) as usize
                } else {
                    0
                };
                *out = self.series(k, part)?;
            }
            Some(_) => {
                let w = self.len.div_ceil(self.bin.max(1)) as usize;
                out.clear();
                for k in 0..self.channels.len() {
                    let s = self.series(k, part)?;
                    let bin = self.bin.max(1) as usize;
                    for c in 0..w {
                        let run = &s[(c * bin).min(s.len())..((c + 1) * bin).min(s.len())];
                        let (sum, n) = run
                            .iter()
                            .filter(|x| x.is_finite())
                            .fold((0f64, 0u32), |(a, n), &x| (a + f64::from(x), n + 1));
                        out.push(if n == 0 {
                            f32::NAN
                        } else {
                            (sum / f64::from(n)) as f32
                        });
                    }
                }
            }
        }
        Ok(out.iter().any(|v| v.is_nan()) && self.file.truncated)
    }
}

fn kw(k: impl Into<String>, v: impl Into<String>) -> (String, String, String) {
    (k.into(), v.into(), String::new())
}

/// The time axis of a channel's samples: (calibration, name, unit).
fn time_axis(ch: &Channel) -> Option<(Linear, String, String)> {
    let (start, step, unit) = ch.waveform()?;
    (step.is_finite() && step != 0.0 && start.is_finite()).then(|| {
        (
            Linear {
                offset: start,
                step,
            },
            ch.property("wf_xname")
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_owned(),
            unit.unwrap_or_else(|| "s".to_owned()),
        )
    })
}

fn complex(ch: &Channel) -> bool {
    matches!(
        ch.dtype,
        tdms::DataType::ComplexF32 | tdms::DataType::ComplexF64
    )
}

fn bits(ch: &Channel) -> u32 {
    use tdms::DataType as D;
    match ch.dtype {
        D::I8 | D::U8 | D::Bool => 8,
        D::I16 | D::U16 => 16,
        D::I32 | D::U32 | D::F32 | D::ComplexF32 => 32,
        _ => 64,
    }
}

fn open(source: Arc<Source>) -> Result<Arc<dyn Session>> {
    let file = Arc::new(tdms::parse(source.bytes()).map_err(map_err)?);
    let mut shared = Shared::new(Format::Tdms);
    shared.kw("TDMS version", file.version.to_string());
    shared.kw("segments", file.segments.to_string());
    let mut layout = Vec::new();
    if file.interleaved {
        layout.push("interleaved");
    }
    if file.big_endian {
        layout.push("big-endian");
    }
    if file.daqmx {
        layout.push("DAQmx raw data");
    }
    if !layout.is_empty() {
        shared.kw("layout", layout.join(", "));
    }
    if file.truncated {
        shared.kw("note", "the file ends early; missing samples are blank");
    }
    for (k, v) in &file.properties {
        shared.kw(k.clone(), v.text());
    }
    for (gi, g) in file.groups.iter().enumerate() {
        let gkw: Vec<_> = g
            .properties
            .iter()
            .map(|(k, v)| kw(format!("{}: {k}", g.name), v.text()))
            .collect();
        let numeric: Vec<usize> = (0..g.channels.len())
            .filter(|&c| g.channels[c].is_numeric() && g.channels[c].len >= 1)
            .collect();
        // Text and other channels: their first values as keywords.
        for ch in &g.channels {
            if ch.dtype == tdms::DataType::String {
                if let Ok(s) = ch.strings(source.bytes(), MAX_STRINGS) {
                    shared.kw(format!("{}/{}", g.name, ch.name), s.join(", "));
                }
            } else if !ch.is_numeric() && ch.len > 0 {
                shared.kw(
                    format!("{}/{}", g.name, ch.name),
                    format!("not shown: {} channel", ch.dtype.describe()),
                );
            }
        }
        // The group image: its channels of the most common length (two or more).
        let mut by_len: Vec<(u64, Vec<usize>)> = Vec::new();
        for &c in &numeric {
            let l = g.channels[c].len;
            match by_len.iter_mut().find(|(n, _)| *n == l) {
                Some((_, v)) => v.push(c),
                None => by_len.push((l, vec![c])),
            }
        }
        by_len.sort_by_key(|(_, v)| std::cmp::Reverse(v.len()));
        if let Some((len, chans)) = by_len.first().filter(|(l, v)| v.len() >= 2 && *l >= 2) {
            let bin = len.div_ceil(MAX_STRIP as u64).max(1);
            let w = len.div_ceil(bin);
            let first = &g.channels[chans[0]];
            let reader = Arc::new(ChannelsRead {
                source: source.clone(),
                file: file.clone(),
                group: gi,
                channels: chans.clone(),
                len: *len,
                bin,
            });
            let mut p = Page::new(
                g.name.clone(),
                vec![chans.len() as u64, w],
                Convention::LastIsX,
                reader,
            );
            p.class = format!("{} channels × {len} samples", chans.len());
            p.complex = chans.iter().any(|&c| complex(&g.channels[c]));
            p.bits = chans
                .iter()
                .map(|&c| bits(&g.channels[c]))
                .max()
                .unwrap_or(64);
            p.sample = first.dtype.describe().to_owned();
            if bin > 1 {
                p.sample += &format!(", {bin} samples per column (mean)");
            }
            let t = time_axis(first);
            p.dim_cal[1] = t.as_ref().map(|(l, _, u)| {
                (
                    Linear {
                        offset: l.offset,
                        step: l.step * bin as f64,
                    },
                    u.clone(),
                )
            });
            p.plot_cal = t.as_ref().map(|(l, _, _)| *l);
            p.row_plot = Some(
                t.as_ref()
                    .map_or((String::new(), String::new()), |(_, n, u)| {
                        (n.clone(), u.clone())
                    }),
            );
            p.dim_labels[0] = chans.iter().map(|&c| g.channels[c].name.clone()).collect();
            p.unit = first.unit().map(str::to_owned);
            p.keywords = gkw.clone();
            p.keywords.insert(
                0,
                kw(
                    "channels",
                    chans
                        .iter()
                        .map(|&c| g.channels[c].name.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            shared.pages.push(p);
        }
        for &c in &numeric {
            let ch = &g.channels[c];
            if ch.len < 2 {
                let mut v = [0f64];
                if ch.read(source.bytes(), 0, &mut v).is_ok() {
                    shared.kw(format!("{}/{}", g.name, ch.name), v[0].to_string());
                }
                continue;
            }
            let reader = Arc::new(ChannelsRead {
                source: source.clone(),
                file: file.clone(),
                group: gi,
                channels: vec![c],
                len: ch.len,
                bin: 1,
            });
            let mut p = Page::new(
                format!("{}/{}", g.name, ch.name),
                vec![ch.len],
                Convention::LastIsX,
                reader,
            );
            p.class = format!("{} × {}", ch.dtype.describe(), ch.len);
            p.complex = complex(ch);
            p.bits = bits(ch);
            p.sample = ch.dtype.describe().to_owned();
            if ch.is_scaled() {
                p.sample += ", scaled";
            }
            if let Some((l, n, u)) = time_axis(ch) {
                p.dim_cal[0] = Some((l, u));
                p.dim_names[0] = Some(n).filter(|n| !n.is_empty());
            }
            p.unit = ch.unit().map(str::to_owned);
            p.title = Some(ch.name.clone());
            p.keywords = ch
                .properties
                .iter()
                .map(|(k, v)| kw(k.clone(), v.text()))
                .collect();
            if let Some(why) = ch.scaling_problem() {
                p.keywords
                    .push(kw("scaling", format!("not applied: {why}")));
            }
            p.keywords.extend(gkw.iter().cloned());
            shared.pages.push(p);
        }
    }
    array_dec::session(source, shared, "the file holds no numeric channels")
}
