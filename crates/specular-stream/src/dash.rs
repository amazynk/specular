//! DASH manifests (ISO/IEC 23009-1 MPD): periods, adaptation sets and representations, and
//! the segments of a representation from a `SegmentTemplate` (by number or with a
//! `SegmentTimeline`), a `SegmentList`, or a `SegmentBase` whose index (`sidx`) is read by
//! [`parse_sidx`]. Static and dynamic (live) manifests.
//!
//! An adaptation set or representation with a `ContentProtection` element is encrypted
//! (Common Encryption, a DRM system): never played.

use crate::addr::resolve;

/// A manifest.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mpd {
    /// `type="dynamic"`: a live stream, read again every [`Mpd::min_update`].
    pub dynamic: bool,
    /// `mediaPresentationDuration`, seconds.
    pub duration: Option<f64>,
    /// `availabilityStartTime`, seconds since 1970.
    pub availability_start: Option<f64>,
    pub min_update: Option<f64>,
    pub time_shift: Option<f64>,
    pub suggested_delay: Option<f64>,
    pub periods: Vec<Period>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Period {
    /// Seconds from the presentation's start.
    pub start: f64,
    pub duration: Option<f64>,
    pub sets: Vec<Set>,
}

/// What an adaptation set carries.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    Video,
    Audio,
    Text,
    #[default]
    Other,
}

/// An adaptation set: the same content at several qualities.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Set {
    pub kind: Kind,
    pub lang: Option<String>,
    /// A `ContentProtection` element (on the set or any representation).
    pub protected: bool,
    pub reps: Vec<Rep>,
}

/// A representation: one quality of a set.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rep {
    pub id: String,
    pub bandwidth: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub codecs: Option<String>,
    pub mime: Option<String>,
    /// The resolved `BaseURL`.
    pub base: String,
    pub addressing: Addressing,
}

/// How a representation's segments are found.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Addressing {
    Template(Template),
    List(List),
    /// One file indexed by its `sidx` (`indexRange`), or played whole.
    Base {
        index: Option<(u64, u64)>,
        init: Option<(u64, u64)>,
    },
    /// Just a `BaseURL`: one file.
    #[default]
    Whole,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Template {
    pub media: Option<String>,
    pub init: Option<String>,
    pub start_number: Option<u64>,
    pub timescale: Option<u64>,
    pub duration: Option<u64>,
    pub pto: Option<u64>,
    /// `(t, d, r)` from `<S>`; `t` absent follows on.
    pub timeline: Option<Vec<(Option<u64>, u64, i64)>>,
}

impl Template {
    /// `self` with what it leaves out taken from `parent`.
    fn inherit(self, parent: &Template) -> Template {
        Template {
            media: self.media.or_else(|| parent.media.clone()),
            init: self.init.or_else(|| parent.init.clone()),
            start_number: self.start_number.or(parent.start_number),
            timescale: self.timescale.or(parent.timescale),
            duration: self.duration.or(parent.duration),
            pto: self.pto.or(parent.pto),
            timeline: self.timeline.or_else(|| parent.timeline.clone()),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct List {
    pub init: Option<(String, Option<(u64, u64)>)>,
    pub segments: Vec<(String, Option<(u64, u64)>)>,
    pub timescale: u64,
    pub duration: Option<u64>,
}

/// A segment to fetch.
#[derive(Clone, Debug, PartialEq)]
pub struct SegRef {
    pub url: String,
    /// `(offset, length)`.
    pub range: Option<(u64, u64)>,
    /// Seconds from the period's start.
    pub start: f64,
    pub duration: f64,
    /// Its number (template) or index (list).
    pub number: u64,
}

/// What went wrong reading a manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    Xml(String),
    NotMpd,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Xml(e) => write!(f, "the manifest is not valid XML: {e}"),
            Error::NotMpd => f.write_str("not a DASH manifest"),
        }
    }
}

/// An ISO 8601 duration (`PT1H2M3.5S`, `P1DT2H`), seconds.
pub fn duration(s: &str) -> Option<f64> {
    let s = s.trim();
    let neg = s.starts_with('-');
    let s = s.trim_start_matches('-').strip_prefix('P')?;
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let mut total = 0.0;
    let mut num = String::new();
    for c in date.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'Y' => total += num.parse::<f64>().ok()? * 365.0 * 86400.0,
            'M' => total += num.parse::<f64>().ok()? * 30.0 * 86400.0,
            'W' => total += num.parse::<f64>().ok()? * 7.0 * 86400.0,
            'D' => total += num.parse::<f64>().ok()? * 86400.0,
            _ => return None,
        }
        if c.is_ascii_alphabetic() {
            num.clear();
        }
    }
    for c in time.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'H' => total += num.parse::<f64>().ok()? * 3600.0,
            'M' => total += num.parse::<f64>().ok()? * 60.0,
            'S' => total += num.parse::<f64>().ok()?,
            _ => return None,
        }
        if c.is_ascii_alphabetic() {
            num.clear();
        }
    }
    Some(if neg { -total } else { total })
}

/// An `xs:dateTime` (`2024-03-01T12:00:00.5Z`, `…+01:00`, no zone = UTC), seconds since 1970.
pub fn date_time(s: &str) -> Option<f64> {
    let s = s.trim();
    let (date, time) = s.split_once('T')?;
    let mut d = date.split('-');
    let y: i64 = d.next()?.parse().ok()?;
    let m: i64 = d.next()?.parse().ok()?;
    let day: i64 = d.next()?.parse().ok()?;
    let (clock, offset) = if let Some(c) = time.strip_suffix('Z') {
        (c, 0.0)
    } else if let Some(i) = time.rfind(['+', '-']).filter(|&i| i >= 8) {
        let (c, z) = time.split_at(i);
        let sign = if z.starts_with('-') { -1.0 } else { 1.0 };
        let (h, mi) = z[1..].split_once(':').unwrap_or((&z[1..], "0"));
        (
            c,
            sign * (h.parse::<f64>().ok()? * 3600.0 + mi.parse::<f64>().ok()? * 60.0),
        )
    } else {
        (time, 0.0)
    };
    let mut c = clock.split(':');
    let h: f64 = c.next()?.parse().ok()?;
    let mi: f64 = c.next()?.parse().ok()?;
    let sec: f64 = c.next().unwrap_or("0").parse().ok()?;
    // Days from 1970-01-01 (proleptic Gregorian).
    let (yy, mm) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = yy.div_euclid(400);
    let yoe = yy - era * 400;
    let doy = (153 * mm + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days as f64 * 86400.0 + h * 3600.0 + mi * 60.0 + sec - offset)
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, name: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
}

fn children<'a, 'i: 'a>(
    n: roxmltree::Node<'a, 'i>,
    name: &'a str,
) -> impl Iterator<Item = roxmltree::Node<'a, 'i>> + 'a {
    n.children()
        .filter(move |c| c.is_element() && c.tag_name().name() == name)
}

fn base_of(n: roxmltree::Node<'_, '_>, parent: &str) -> String {
    match child(n, "BaseURL").and_then(|b| b.text()) {
        Some(t) => resolve(parent, t.trim()),
        None => parent.to_owned(),
    }
}

fn num<T: std::str::FromStr>(n: roxmltree::Node<'_, '_>, a: &str) -> Option<T> {
    n.attribute(a).and_then(|v| v.trim().parse().ok())
}

fn range_attr(s: Option<&str>) -> Option<(u64, u64)> {
    let (a, b) = s?.split_once('-')?;
    let (a, b): (u64, u64) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
    (b >= a).then_some((a, b - a + 1))
}

fn template_of(n: Option<roxmltree::Node<'_, '_>>) -> Template {
    let Some(n) = n else {
        return Template::default();
    };
    Template {
        media: n.attribute("media").map(str::to_owned),
        init: n.attribute("initialization").map(str::to_owned),
        start_number: num(n, "startNumber"),
        timescale: num(n, "timescale"),
        duration: num(n, "duration"),
        pto: num(n, "presentationTimeOffset"),
        timeline: child(n, "SegmentTimeline").map(|tl| {
            children(tl, "S")
                .map(|s| {
                    (
                        num(s, "t"),
                        num(s, "d").unwrap_or(0),
                        num(s, "r").unwrap_or(0),
                    )
                })
                .collect()
        }),
    }
}

fn kind_of(set: roxmltree::Node<'_, '_>) -> Kind {
    let ct = set.attribute("contentType").unwrap_or("");
    let mime = set
        .attribute("mimeType")
        .or_else(|| children(set, "Representation").find_map(|r| r.attribute("mimeType")))
        .unwrap_or("");
    let probe = format!("{ct} {mime}").to_ascii_lowercase();
    if probe.contains("video") {
        Kind::Video
    } else if probe.contains("audio") {
        Kind::Audio
    } else if probe.contains("text") || probe.contains("application/ttml") {
        Kind::Text
    } else {
        Kind::Other
    }
}

/// Reads a manifest; addresses are resolved against `url` (the manifest's own address).
pub fn parse(text: &str, url: &str) -> Result<Mpd, Error> {
    let doc = roxmltree::Document::parse(text).map_err(|e| Error::Xml(e.to_string()))?;
    let root = doc.root_element();
    if root.tag_name().name() != "MPD" {
        return Err(Error::NotMpd);
    }
    let mut mpd = Mpd {
        dynamic: root.attribute("type") == Some("dynamic"),
        duration: root
            .attribute("mediaPresentationDuration")
            .and_then(duration),
        availability_start: root.attribute("availabilityStartTime").and_then(date_time),
        min_update: root.attribute("minimumUpdatePeriod").and_then(duration),
        time_shift: root.attribute("timeShiftBufferDepth").and_then(duration),
        suggested_delay: root
            .attribute("suggestedPresentationDelay")
            .and_then(duration),
        periods: Vec::new(),
    };
    let mpd_base = base_of(root, url);
    let mut next_start = 0.0;
    for p in children(root, "Period") {
        let start = p
            .attribute("start")
            .and_then(duration)
            .unwrap_or(next_start);
        let pdur = p.attribute("duration").and_then(duration);
        let pbase = base_of(p, &mpd_base);
        let ptpl = template_of(child(p, "SegmentTemplate"));
        let mut period = Period {
            start,
            duration: pdur,
            sets: Vec::new(),
        };
        for a in children(p, "AdaptationSet") {
            let abase = base_of(a, &pbase);
            let atpl = template_of(child(a, "SegmentTemplate")).inherit(&ptpl);
            let mut set = Set {
                kind: kind_of(a),
                lang: a.attribute("lang").map(str::to_owned),
                protected: child(a, "ContentProtection").is_some(),
                reps: Vec::new(),
            };
            for r in children(a, "Representation") {
                if child(r, "ContentProtection").is_some() {
                    set.protected = true;
                }
                let rbase = base_of(r, &abase);
                let rtpl_node = child(r, "SegmentTemplate");
                let has_tpl = rtpl_node.is_some()
                    || child(a, "SegmentTemplate").is_some()
                    || child(p, "SegmentTemplate").is_some();
                let list = child(r, "SegmentList").or_else(|| child(a, "SegmentList"));
                let sbase = child(r, "SegmentBase").or_else(|| child(a, "SegmentBase"));
                let addressing = if has_tpl {
                    Addressing::Template(template_of(rtpl_node).inherit(&atpl))
                } else if let Some(l) = list {
                    Addressing::List(List {
                        init: child(l, "Initialization").map(|i| {
                            (
                                i.attribute("sourceURL")
                                    .map_or(rbase.clone(), |u| resolve(&rbase, u)),
                                range_attr(i.attribute("range")),
                            )
                        }),
                        segments: children(l, "SegmentURL")
                            .map(|s| {
                                (
                                    s.attribute("media")
                                        .map_or(rbase.clone(), |u| resolve(&rbase, u)),
                                    range_attr(s.attribute("mediaRange")),
                                )
                            })
                            .collect(),
                        timescale: num(l, "timescale").unwrap_or(1),
                        duration: num(l, "duration"),
                    })
                } else if let Some(b) = sbase {
                    Addressing::Base {
                        index: range_attr(b.attribute("indexRange")),
                        init: child(b, "Initialization")
                            .and_then(|i| range_attr(i.attribute("range"))),
                    }
                } else {
                    Addressing::Whole
                };
                set.reps.push(Rep {
                    id: r.attribute("id").unwrap_or("").to_owned(),
                    bandwidth: num(r, "bandwidth").unwrap_or(0),
                    width: num(r, "width").or_else(|| num(a, "width")),
                    height: num(r, "height").or_else(|| num(a, "height")),
                    codecs: r
                        .attribute("codecs")
                        .or_else(|| a.attribute("codecs"))
                        .map(str::to_owned),
                    mime: r
                        .attribute("mimeType")
                        .or_else(|| a.attribute("mimeType"))
                        .map(str::to_owned),
                    base: rbase,
                    addressing,
                });
            }
            if !set.reps.is_empty() {
                period.sets.push(set);
            }
        }
        next_start = start + pdur.unwrap_or(0.0);
        mpd.periods.push(period);
    }
    // A single period runs as long as the presentation.
    if let (Some(d), [p]) = (mpd.duration, mpd.periods.as_mut_slice())
        && p.duration.is_none()
    {
        p.duration = Some((d - p.start).max(0.0));
    }
    Ok(mpd)
}

/// A template with `$RepresentationID$`, `$Number$` (`%0Nd`), `$Bandwidth$` and `$Time$`
/// filled in.
pub fn fill(tpl: &str, rep: &Rep, number: u64, time: u64) -> String {
    let mut out = String::with_capacity(tpl.len() + 16);
    let mut rest = tpl;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let Some(j) = after.find('$') else {
            out.push_str(&rest[i..]);
            return out;
        };
        let ident = &after[..j];
        let (name, fmt) = ident.split_once('%').unwrap_or((ident, ""));
        let width: usize = fmt
            .trim_start_matches('0')
            .trim_end_matches('d')
            .parse()
            .unwrap_or(0);
        match name {
            "" => out.push('$'),
            "RepresentationID" => out.push_str(&rep.id),
            "Number" => out.push_str(&format!("{number:0width$}")),
            "Bandwidth" => out.push_str(&format!("{:0width$}", rep.bandwidth)),
            "Time" => out.push_str(&format!("{time:0width$}")),
            _ => {
                out.push('$');
                out.push_str(ident);
                out.push('$');
            }
        }
        rest = &after[j + 1..];
    }
    out.push_str(rest);
    out
}

impl Rep {
    /// The initialisation segment, when there is one apart from the media.
    pub fn init(&self) -> Option<SegRef> {
        let (url, range) = match &self.addressing {
            Addressing::Template(t) => (
                resolve(&self.base, &fill(t.init.as_ref()?, self, 0, 0)),
                None,
            ),
            Addressing::List(l) => l.init.clone()?,
            Addressing::Base { init, .. } => (self.base.clone(), Some((*init)?)),
            Addressing::Whole => return None,
        };
        Some(SegRef {
            url,
            range,
            start: 0.0,
            duration: 0.0,
            number: 0,
        })
    }

    /// The media segments, in order. A live manifest (`now`: seconds since 1970) gives those
    /// available now; `SegmentBase` gives nothing here (its `sidx` must be read first).
    pub fn segments(&self, mpd: &Mpd, period: &Period, now: f64) -> Vec<SegRef> {
        match &self.addressing {
            Addressing::Template(t) => self.template_segments(t, mpd, period, now),
            Addressing::List(l) => {
                let d = l
                    .duration
                    .map_or(0.0, |d| d as f64 / l.timescale.max(1) as f64);
                l.segments
                    .iter()
                    .enumerate()
                    .map(|(i, (url, range))| SegRef {
                        url: url.clone(),
                        range: *range,
                        start: i as f64 * d,
                        duration: d,
                        number: i as u64,
                    })
                    .collect()
            }
            Addressing::Base { .. } => Vec::new(),
            Addressing::Whole => vec![SegRef {
                url: self.base.clone(),
                range: None,
                start: 0.0,
                duration: period.duration.unwrap_or(0.0),
                number: 0,
            }],
        }
    }

    fn template_segments(&self, t: &Template, mpd: &Mpd, period: &Period, now: f64) -> Vec<SegRef> {
        let Some(media) = t.media.as_deref() else {
            return Vec::new();
        };
        let ts = t.timescale.unwrap_or(1).max(1) as f64;
        let start_number = t.start_number.unwrap_or(1);
        let pto = t.pto.unwrap_or(0);
        let make = |number: u64, time: u64, dur: u64| SegRef {
            url: resolve(&self.base, &fill(media, self, number, time)),
            range: None,
            start: (time.saturating_sub(pto)) as f64 / ts,
            duration: dur as f64 / ts,
            number,
        };
        if let Some(tl) = &t.timeline {
            let end = period
                .duration
                .map(|d| pto + (d * ts) as u64)
                .unwrap_or(u64::MAX);
            let mut out = Vec::new();
            let mut time = 0u64;
            let mut number = start_number;
            for (i, &(st, d, r)) in tl.iter().enumerate() {
                if let Some(st) = st {
                    time = st;
                }
                if d == 0 {
                    continue;
                }
                let count = if r >= 0 {
                    r as u64 + 1
                } else {
                    // Repeat until the next S's start, or the period's end.
                    let until = tl.get(i + 1).and_then(|n| n.0).unwrap_or(if mpd.dynamic {
                        live_time(mpd, period, now, ts, pto)
                    } else {
                        end
                    });
                    until.saturating_sub(time).div_ceil(d)
                };
                for _ in 0..count.min(1_000_000) {
                    out.push(make(number, time, d));
                    time += d;
                    number += 1;
                }
            }
            return out;
        }
        let Some(d) = t.duration.filter(|&d| d > 0) else {
            return Vec::new();
        };
        let seg_s = d as f64 / ts;
        if mpd.dynamic {
            let Some(ast) = mpd.availability_start else {
                return Vec::new();
            };
            let elapsed = now - ast - period.start;
            // Segment k is whole once its end has passed.
            let available = (elapsed / seg_s).floor() as i64;
            if available < 1 {
                return Vec::new();
            }
            let window = mpd
                .time_shift
                .map_or(5, |s| (s / seg_s).floor().max(1.0) as i64);
            let first = (available - window).max(0);
            return (first..available)
                .map(|k| make(start_number + k as u64, pto + k as u64 * d, d))
                .collect();
        }
        let total = period.duration.or(mpd.duration).unwrap_or(0.0);
        let count = (total / seg_s - 1e-9).ceil().max(0.0) as u64;
        (0..count.min(1_000_000))
            .map(|k| make(start_number + k, pto + k * d, d))
            .collect()
    }

    /// A label for the menu: `1280×720, 2.8 Mbit/s` or `128 kbit/s`.
    pub fn label(&self) -> String {
        let rate = rate_label(self.bandwidth);
        match (self.width, self.height) {
            (Some(w), Some(h)) => format!("{w}×{h}, {rate}"),
            _ => rate,
        }
    }
}

/// The live edge in the template's timescale.
fn live_time(mpd: &Mpd, period: &Period, now: f64, ts: f64, pto: u64) -> u64 {
    let ast = mpd.availability_start.unwrap_or(now);
    pto + ((now - ast - period.start).max(0.0) * ts) as u64
}

/// `2.8 Mbit/s`, `128 kbit/s`.
pub fn rate_label(bits_per_s: u64) -> String {
    if bits_per_s >= 1_000_000 {
        format!("{:.1} Mbit/s", bits_per_s as f64 / 1e6)
    } else {
        format!("{} kbit/s", bits_per_s / 1000)
    }
}

/// The subsegments a `sidx` box lists: `(offset, length, seconds)` in the file, given the
/// box's own offset in the file. Only media references are taken (a hierarchical index's
/// references to other `sidx` boxes are not followed).
pub fn parse_sidx(data: &[u8], box_offset: u64) -> Option<Vec<(u64, u64, f64)>> {
    let size = u32::from_be_bytes(data.get(0..4)?.try_into().ok()?) as usize;
    if data.get(4..8)? != b"sidx" || size < 32 || size > data.len() {
        return None;
    }
    let version = data[8];
    let mut at = 12 + 4; // full-box header, reference_ID
    let timescale = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as f64;
    at += 4;
    let first_offset = if version == 0 {
        at += 4;
        let v = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?) as u64;
        at += 4;
        v
    } else {
        at += 8;
        let v = u64::from_be_bytes(data.get(at..at + 8)?.try_into().ok()?);
        at += 8;
        v
    };
    at += 2;
    let count = u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?) as usize;
    at += 2;
    let mut offset = box_offset + size as u64 + first_offset;
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let a = u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?);
        let dur = u32::from_be_bytes(data.get(at + 4..at + 8)?.try_into().ok()?);
        at += 12;
        let len = u64::from(a & 0x7FFF_FFFF);
        if a & 0x8000_0000 == 0 {
            out.push((offset, len, f64::from(dur) / timescale.max(1.0)));
        }
        offset += len;
    }
    Some(out)
}

impl Mpd {
    /// Why it can't be played, if it can't: every video or sound set is protected.
    pub fn protected(&self) -> bool {
        self.periods.iter().any(|p| {
            let av: Vec<&Set> = p
                .sets
                .iter()
                .filter(|s| matches!(s.kind, Kind::Video | Kind::Audio))
                .collect();
            !av.is_empty() && av.iter().all(|s| s.protected)
        })
    }
}

impl Period {
    /// The first clear set of `kind`.
    pub fn set(&self, kind: Kind) -> Option<&Set> {
        self.sets.iter().find(|s| s.kind == kind && !s.protected)
    }
}

impl Set {
    /// Representations, lowest bandwidth first.
    pub fn by_bandwidth(&self) -> Vec<usize> {
        let mut v: Vec<usize> = (0..self.reps.len()).collect();
        v.sort_by_key(|&i| self.reps[i].bandwidth);
        v
    }

    /// The highest representation within `budget` bits per second, else the lowest.
    pub fn pick(&self, budget: Option<f64>) -> usize {
        let order = self.by_bandwidth();
        let Some(b) = budget else { return order[0] };
        order
            .iter()
            .copied()
            .rev()
            .find(|&i| self.reps[i].bandwidth as f64 <= b)
            .unwrap_or(order[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_and_dates_read() {
        assert_eq!(duration("PT1H2M3.5S"), Some(3723.5));
        assert_eq!(duration("P1DT2H"), Some(93600.0));
        assert_eq!(duration("PT0S"), Some(0.0));
        assert_eq!(duration("PT634.566S"), Some(634.566));
        assert_eq!(duration("1H"), None);
        assert_eq!(date_time("1970-01-01T00:00:00Z"), Some(0.0));
        assert_eq!(date_time("2000-03-01T00:00:00Z"), Some(951_868_800.0));
        assert_eq!(date_time("2024-02-29T12:30:15.5Z"), Some(1_709_209_815.5));
        assert_eq!(
            date_time("2024-02-29T13:30:15.5+01:00"),
            Some(1_709_209_815.5)
        );
        assert_eq!(date_time("2024-02-29T12:30:15.5"), Some(1_709_209_815.5));
    }

    const STATIC: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="static" mediaPresentationDuration="PT0H0M10.0S" minBufferTime="PT2S">
  <BaseURL>media/</BaseURL>
  <Period id="0">
    <AdaptationSet contentType="video" mimeType="video/mp4" segmentAlignment="true">
      <SegmentTemplate timescale="1000" duration="4000" initialization="$RepresentationID$/init.mp4" media="$RepresentationID$/seg-$Number%05d$.m4s" startNumber="1"/>
      <Representation id="v1" bandwidth="800000" width="640" height="360" codecs="avc1.4d401e"/>
      <Representation id="v2" bandwidth="2500000" width="1280" height="720" codecs="avc1.64001f"/>
    </AdaptationSet>
    <AdaptationSet contentType="audio" mimeType="audio/mp4" lang="en">
      <Representation id="a1" bandwidth="128000" codecs="mp4a.40.2">
        <SegmentTemplate timescale="48000" initialization="a/init.mp4" media="a/$Time$.m4s">
          <SegmentTimeline>
            <S t="0" d="192000" r="1"/>
            <S d="96000"/>
          </SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet>
    <AdaptationSet contentType="video" mimeType="video/mp4">
      <ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" value="cenc"/>
      <Representation id="drm" bandwidth="1"/>
    </AdaptationSet>
  </Period>
</MPD>"#;

    #[test]
    fn a_static_manifest_lists_its_segments() {
        let mpd = parse(STATIC, "https://h.example/show/manifest.mpd").unwrap();
        assert!(!mpd.dynamic);
        assert_eq!(mpd.duration, Some(10.0));
        let p = &mpd.periods[0];
        assert_eq!(p.duration, Some(10.0));
        assert_eq!(p.sets.len(), 3);
        assert!(!mpd.protected(), "clear sets remain");
        assert!(p.sets[2].protected);
        let v = p.set(Kind::Video).unwrap();
        assert_eq!(v.reps.len(), 2);
        assert_eq!(v.pick(Some(3e6)), 1);
        assert_eq!(v.pick(Some(1e6)), 0);
        assert_eq!(v.pick(None), 0);
        let r = &v.reps[1];
        assert_eq!(r.label(), "1280×720, 2.5 Mbit/s");
        assert_eq!(
            r.init().unwrap().url,
            "https://h.example/show/media/v2/init.mp4"
        );
        let segs = r.segments(&mpd, p, 0.0);
        assert_eq!(segs.len(), 3, "10 s in 4 s segments");
        assert_eq!(segs[0].url, "https://h.example/show/media/v2/seg-00001.m4s");
        assert_eq!(segs[2].url, "https://h.example/show/media/v2/seg-00003.m4s");
        assert_eq!(segs[2].start, 8.0);
        let a = p.set(Kind::Audio).unwrap();
        assert_eq!(a.lang.as_deref(), Some("en"));
        let segs = a.reps[0].segments(&mpd, p, 0.0);
        let urls: Vec<&str> = segs.iter().map(|s| s.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://h.example/show/media/a/0.m4s",
                "https://h.example/show/media/a/192000.m4s",
                "https://h.example/show/media/a/384000.m4s"
            ]
        );
        assert_eq!(segs[2].duration, 2.0);
    }

    #[test]
    fn a_live_manifest_gives_what_is_available_now() {
        let text = r#"<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" type="dynamic" availabilityStartTime="2024-01-01T00:00:00Z" minimumUpdatePeriod="PT2S" timeShiftBufferDepth="PT20S">
  <Period start="PT0S">
    <AdaptationSet mimeType="audio/mp4">
      <SegmentTemplate timescale="1" duration="2" media="live-$Number$.m4s" initialization="init.mp4" startNumber="10"/>
      <Representation id="a" bandwidth="64000"/>
    </AdaptationSet>
  </Period>
</MPD>"#;
        let mpd = parse(text, "http://h/live/m.mpd").unwrap();
        assert!(mpd.dynamic);
        assert_eq!(mpd.min_update, Some(2.0));
        let p = &mpd.periods[0];
        let r = &p.sets[0].reps[0];
        let ast = date_time("2024-01-01T00:00:00Z").unwrap();
        // 101 s in: segments 0..=49 (of 2 s) are whole; the window keeps 10.
        let segs = r.segments(&mpd, p, ast + 101.0);
        assert_eq!(segs.len(), 10);
        assert_eq!(segs.last().unwrap().url, "http://h/live/live-59.m4s");
        assert_eq!(segs[0].url, "http://h/live/live-50.m4s");
        assert!(r.segments(&mpd, p, ast + 1.0).is_empty());
    }

    #[test]
    fn segment_lists_bases_and_sidx() {
        let text = r#"<MPD type="static" mediaPresentationDuration="PT6S">
  <Period>
    <AdaptationSet mimeType="audio/mp4">
      <Representation id="l" bandwidth="1">
        <BaseURL>a.mp4</BaseURL>
        <SegmentList timescale="10" duration="30">
          <Initialization range="0-99"/>
          <SegmentURL mediaRange="100-199"/>
          <SegmentURL mediaRange="200-299"/>
        </SegmentList>
      </Representation>
      <Representation id="b" bandwidth="2">
        <BaseURL>b.mp4</BaseURL>
        <SegmentBase indexRange="800-899"><Initialization range="0-799"/></SegmentBase>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;
        let mpd = parse(text, "http://h/x/m.mpd").unwrap();
        let p = &mpd.periods[0];
        let l = &p.sets[0].reps[0];
        assert_eq!(l.init().unwrap().range, Some((0, 100)));
        let s = l.segments(&mpd, p, 0.0);
        assert_eq!(s[1].range, Some((200, 100)));
        assert_eq!(s[1].url, "http://h/x/a.mp4");
        assert_eq!(s[1].start, 3.0);
        let b = &p.sets[0].reps[1];
        assert_eq!(
            b.addressing,
            Addressing::Base {
                index: Some((800, 100)),
                init: Some((0, 800))
            }
        );
        // A sidx at 800 (52 bytes) listing two subsegments after a gap of 8.
        let mut sidx = Vec::new();
        sidx.extend_from_slice(&52u32.to_be_bytes());
        sidx.extend_from_slice(b"sidx");
        sidx.extend_from_slice(&[0, 0, 0, 0]);
        sidx.extend_from_slice(&1u32.to_be_bytes());
        sidx.extend_from_slice(&1000u32.to_be_bytes());
        sidx.extend_from_slice(&0u32.to_be_bytes());
        sidx.extend_from_slice(&8u32.to_be_bytes());
        sidx.extend_from_slice(&[0, 0, 0, 2]);
        for (len, dur) in [(5000u32, 2000u32), (3000, 1500)] {
            sidx.extend_from_slice(&len.to_be_bytes());
            sidx.extend_from_slice(&dur.to_be_bytes());
            sidx.extend_from_slice(&0x9000_0000u32.to_be_bytes());
        }
        assert_eq!(
            parse_sidx(&sidx, 800).unwrap(),
            vec![(860, 5000, 2.0), (5860, 3000, 1.5)]
        );
        assert_eq!(
            fill(
                "$RepresentationID$-$Number%03d$-$Bandwidth$-$$-$Time$",
                b,
                7,
                99
            ),
            "b-007-2-$-99"
        );
        assert!(parse("<html/>", "http://h/").is_err());
    }
}
