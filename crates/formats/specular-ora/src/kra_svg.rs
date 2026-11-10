//! A Krita vector layer's SVG made to draw as Krita draws it with an ordinary SVG renderer.
//!
//! Krita paints shapes with Qt, which interpolates gradient colours premultiplied by their
//! opacity; SVG renderers interpolate them straight, so a stop fading to transparent tints
//! the gradient with its (invisible) colour. Gradients whose stop opacities differ get
//! intermediate stops holding the premultiplied interpolation. Qt also clips with shapes
//! nested in groups, which SVG does not allow inside a clip path: such groups are dissolved
//! into their shapes' transforms.

use quick_xml::Reader;
use quick_xml::Writer;
use quick_xml::events::{BytesStart, Event};

/// Intermediate stops inserted per segment whose two opacities differ.
const STEPS: usize = 16;

struct Stop {
    offset: f32,
    color: [f32; 3],
    opacity: f32,
}

fn attr(e: &BytesStart<'_>, k: &[u8]) -> Option<String> {
    e.attributes()
        .with_checks(false)
        .flatten()
        .find(|a| a.key.local_name().as_ref() == k)
        .map(|a| String::from_utf8_lossy(&a.value).into_owned())
}

fn hex(s: &str) -> Option<[f32; 3]> {
    let h = s.trim().strip_prefix('#')?;
    let n = match h.len() {
        6 => u32::from_str_radix(h, 16).ok()?,
        3 => {
            let v = u32::from_str_radix(h, 16).ok()?;
            let (r, g, b) = ((v >> 8) & 15, (v >> 4) & 15, v & 15);
            ((r * 17) << 16) | ((g * 17) << 8) | (b * 17)
        }
        _ => return None,
    };
    Some([(n >> 16) & 255, (n >> 8) & 255, n & 255].map(|c| c as f32 / 255.0))
}

fn stop(e: &BytesStart<'_>) -> Option<Stop> {
    let offset = attr(e, b"offset")?;
    let offset = match offset.trim().strip_suffix('%') {
        Some(p) => p.parse::<f32>().ok()? / 100.0,
        None => offset.trim().parse::<f32>().ok()?,
    };
    Some(Stop {
        offset: offset.clamp(0.0, 1.0),
        color: hex(&attr(e, b"stop-color")?)?,
        opacity: attr(e, b"stop-opacity")
            .map_or(Some(1.0), |v| v.trim().parse::<f32>().ok())?
            .clamp(0.0, 1.0),
    })
}

/// The stops between `a` and `b` (exclusive) interpolated premultiplied.
fn between(a: &Stop, b: &Stop) -> Vec<Stop> {
    (1..STEPS)
        .map(|k| {
            let t = k as f32 / STEPS as f32;
            let opacity = a.opacity + (b.opacity - a.opacity) * t;
            let color = std::array::from_fn(|c| {
                let p =
                    a.color[c] * a.opacity + (b.color[c] * b.opacity - a.color[c] * a.opacity) * t;
                if opacity > 0.0 { p / opacity } else { 0.0 }
            });
            Stop {
                offset: a.offset + (b.offset - a.offset) * t,
                color,
                opacity,
            }
        })
        .collect()
}

fn write_stop(w: &mut Writer<Vec<u8>>, s: &Stop) -> std::io::Result<()> {
    let c = s.color.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    let mut e = BytesStart::new("stop");
    e.push_attribute(("offset", format!("{}", s.offset).as_str()));
    e.push_attribute((
        "stop-color",
        format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2]).as_str(),
    ));
    e.push_attribute(("stop-opacity", format!("{}", s.opacity).as_str()));
    w.write_event(Event::Empty(e))
}

/// `svg` with premultiplied gradient interpolation spelled out; `None` when nothing needs
/// changing (or the document cannot be read here: it is then drawn as it is).
#[doc(hidden)]
pub fn fixup(svg: &[u8]) -> Option<Vec<u8>> {
    let mut r = Reader::from_reader(svg);
    let mut w = Writer::new(Vec::with_capacity(svg.len() + 1024));
    let mut buf = Vec::new();
    // Inside a gradient: its stops so far (`None` once one cannot be read).
    let mut stops: Option<Option<Vec<Stop>>> = None;
    let mut held: Vec<Event<'static>> = Vec::new();
    let mut changed = false;
    // Open elements inside a `<clipPath>`, the clip path first: for a dissolved group its
    // transform, `None` for anything else.
    let mut clip: Vec<Option<String>> = Vec::new();
    loop {
        let ev = r.read_event_into(&mut buf).ok()?;
        match &ev {
            Event::Eof => break,
            Event::Start(e)
                if matches!(
                    e.local_name().as_ref(),
                    b"linearGradient" | b"radialGradient"
                ) =>
            {
                stops = Some(Some(Vec::new()));
                held.clear();
                w.write_event(ev.borrow()).ok()?;
            }
            Event::Empty(e) | Event::Start(e)
                if stops.is_some() && e.local_name().as_ref() == b"stop" =>
            {
                held.push(ev.clone().into_owned());
                if let Some(Some(v)) = &mut stops {
                    match stop(e) {
                        Some(s) => v.push(s),
                        None => stops = Some(None),
                    }
                }
            }
            Event::End(e)
                if stops.is_some()
                    && matches!(
                        e.local_name().as_ref(),
                        b"linearGradient" | b"radialGradient"
                    ) =>
            {
                match stops.take() {
                    Some(Some(v)) if v.windows(2).any(|p| p[0].opacity != p[1].opacity) => {
                        changed = true;
                        for (k, s) in v.iter().enumerate() {
                            write_stop(&mut w, s).ok()?;
                            if let Some(n) = v.get(k + 1)
                                && s.opacity != n.opacity
                            {
                                for m in between(s, n) {
                                    write_stop(&mut w, &m).ok()?;
                                }
                            }
                        }
                    }
                    _ => {
                        for h in held.drain(..) {
                            w.write_event(h).ok()?;
                        }
                    }
                }
                held.clear();
                w.write_event(ev.borrow()).ok()?;
            }
            // Anything else inside a gradient (whitespace, `</stop>`) is kept with the stops.
            _ if stops.is_some() => held.push(ev.clone().into_owned()),
            // Inside a clip path Krita nests the shapes in groups, which SVG does not allow
            // there (renderers then clip everything away), and repeats the clip path's id on
            // the first group: the groups are dissolved, their transforms moved onto the
            // shapes, and ids inside clip paths dropped.
            Event::Start(e) | Event::Empty(e) if !clip.is_empty() => {
                let start = matches!(ev, Event::Start(_));
                let group = e.local_name().as_ref() == b"g";
                if group {
                    changed = true;
                    if start {
                        clip.push(Some(attr(e, b"transform").unwrap_or_default()));
                    }
                    buf.clear();
                    continue;
                }
                if start {
                    clip.push(None);
                }
                let outer: Vec<&str> = clip
                    .iter()
                    .filter_map(|t| t.as_deref())
                    .filter(|t| !t.trim().is_empty())
                    .collect();
                let mut n =
                    BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());
                let mut own = String::new();
                for a in e.attributes().with_checks(false).flatten() {
                    match a.key.as_ref() {
                        b"id" => changed = true,
                        b"transform" => own = String::from_utf8_lossy(&a.value).into_owned(),
                        _ => n.push_attribute(a),
                    }
                }
                let mut t = outer.join(" ");
                if !own.is_empty() {
                    t = format!("{t} {own}");
                }
                if !t.trim().is_empty() {
                    n.push_attribute(("transform", t.trim()));
                }
                w.write_event(if start {
                    Event::Start(n)
                } else {
                    Event::Empty(n)
                })
                .ok()?;
            }
            Event::End(_) if clip.len() > 1 => {
                // A dissolved group's end is dropped.
                if clip.pop().flatten().is_none() {
                    w.write_event(ev.borrow()).ok()?;
                }
            }
            Event::End(e) if !clip.is_empty() && e.local_name().as_ref() == b"clipPath" => {
                clip.clear();
                w.write_event(ev.borrow()).ok()?;
            }
            Event::Start(e) if e.local_name().as_ref() == b"clipPath" => {
                clip.push(None);
                w.write_event(ev.borrow()).ok()?;
            }
            _ => w.write_event(ev.borrow()).ok()?,
        }
        buf.clear();
    }
    changed.then(|| w.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fading_stops_interpolate_premultiplied() {
        let svg = br##"<svg><defs><linearGradient id="g"><stop stop-color="#ffffff" offset="0" stop-opacity="1"/><stop stop-color="#d00000" offset="1" stop-opacity="0"/></linearGradient></defs><rect fill="url(#g)"/></svg>"##;
        let out = String::from_utf8(fixup(svg).unwrap()).unwrap();
        assert_eq!(out.matches("<stop").count(), 2 + STEPS - 1);
        // Half way: white at half opacity, no red.
        assert!(out.contains(r##"offset="0.5" stop-color="#ffffff" stop-opacity="0.5""##));
        assert!(out.ends_with(r#"<rect fill="url(#g)"/></svg>"#));
        let plain = br##"<svg><linearGradient id="g"><stop stop-color="#fff" offset="0"/><stop stop-color="#000" offset="1"/></linearGradient></svg>"##;
        assert!(fixup(plain).is_none());
    }
}
