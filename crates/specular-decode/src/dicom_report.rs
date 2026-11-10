//! DICOM files without an image that are read as documents: a structured report opens as its
//! content tree set as text (the application browses the same tree, with jumps to the images
//! it points at, in its tag browser), and a waveform (ECG, haemodynamic…) as a summary page
//! of its channels and annotations (the application draws the traces on its plot sheet).
//! Both are wrapped as encapsulated documents, so the info panel gets the kind and title and
//! the study summary, never the patient. Person names in a report are left out of the page.

use std::sync::Arc;

use specular_core::Source;
use specular_dicom as dcm;
use specular_dicom::sr::{self, Report};
use specular_dicom::waveform::Waveforms;
use specular_pdf::textdoc::{self, Style};

use crate::dicom_doc::EncapsulatedFacts;
use crate::{DecodeError, Format, Result, Session};

/// The kind a report's facts carry ([`EncapsulatedFacts::kind`]).
pub const SR_KIND: &str = "SR";
/// The kind a waveform's facts carry.
pub const WAVEFORM_KIND: &str = "Waveform";

/// Opens `d` as a report or waveform document, or `None` when it is neither.
pub(crate) fn open(source: &Arc<Source>, d: &dcm::Dicom) -> Option<Result<Arc<dyn Session>>> {
    let data = source.bytes();
    if let Ok(r) = Report::read(d, data) {
        let pdf = build(&report_text(&r));
        let facts = EncapsulatedFacts {
            kind: SR_KIND,
            title: Some(format!("{} ({})", r.title(), r.kind.name())),
            mime: None,
            summary: d.summary(data),
        };
        return Some(wrap(source, pdf, facts));
    }
    if Waveforms::present(d) {
        let w = match Waveforms::read(d, data) {
            Ok(w) => w,
            Err(e) => return Some(Err(DecodeError::Malformed(format!("DICOM waveform: {e}")))),
        };
        let pdf = build(&waveform_text(&w));
        let facts = EncapsulatedFacts {
            kind: WAVEFORM_KIND,
            title: Some(w.kind.name().to_owned()),
            mime: None,
            summary: d.summary(data),
        };
        return Some(wrap(source, pdf, facts));
    }
    None
}

fn wrap(source: &Arc<Source>, pdf: Vec<u8>, facts: EncapsulatedFacts) -> Result<Arc<dyn Session>> {
    let path = source.path().to_path_buf();
    let s = Arc::new(Source::from_entry(path, source, pdf));
    let inner = crate::pdf_dec::open_pdf(s, Format::Pdf)?;
    Ok(crate::dicom_doc::wrap(
        source.clone(),
        inner,
        Arc::new(facts),
    ))
}

fn build(lines: &[(Style, String)]) -> Vec<u8> {
    let refs: Vec<(Style, &str)> = lines.iter().map(|(s, t)| (*s, t.as_str())).collect();
    textdoc::build(&refs)
}

/// The report's tree as indented lines: the title, then each item two spaces deeper than its
/// parent, containers with content as headings. Person names are hidden.
pub fn report_text(r: &Report) -> Vec<(Style, String)> {
    let mut out = vec![(Style::Title, format!("{} ({})", r.title(), r.kind.name()))];
    let walk = r.walk();
    for ((_, n), (depth, line)) in walk.iter().zip(r.lines(false)).skip(1) {
        let indent = "  ".repeat(depth.saturating_sub(1));
        let style = if n.value_type == "CONTAINER" && !n.children.is_empty() {
            Style::Heading
        } else {
            Style::Body
        };
        out.push((style, format!("{indent}{line}")));
    }
    let mut flags = Vec::new();
    if !r.completion.is_empty() {
        flags.push(r.completion.clone());
    }
    if !r.verification.is_empty() {
        flags.push(r.verification.clone());
    }
    if !flags.is_empty() {
        out.push((Style::Body, String::new()));
        out.push((Style::Body, flags.join(" · ")));
    }
    if r.dropped > 0 {
        out.push((Style::Body, format!("(+{} items not read)", r.dropped)));
    }
    out
}

/// The waveform's summary: kind, then per group its rate, length and channels, then the
/// annotations.
pub fn waveform_text(w: &Waveforms) -> Vec<(Style, String)> {
    let mut out = vec![(Style::Title, w.kind.name().to_owned())];
    for g in &w.groups {
        let head = if g.label.is_empty() {
            format!("{} Hz · {:.2} s", fmt(g.frequency), g.duration())
        } else {
            format!(
                "{} · {} Hz · {:.2} s",
                g.label,
                fmt(g.frequency),
                g.duration()
            )
        };
        out.push((Style::Heading, head));
        for c in &g.channels {
            let unit = c.unit_symbol();
            let range = c
                .extent(0..c.samples.len())
                .map(|(lo, hi)| {
                    format!(" · {} – {} {unit}", fmt(f64::from(lo)), fmt(f64::from(hi)))
                })
                .unwrap_or_default();
            out.push((Style::Body, format!("{}{range}", c.display_label())));
        }
    }
    if !w.annotations.is_empty() {
        out.push((Style::Heading, String::from("Annotations")));
        for a in &w.annotations {
            let name = a
                .concept
                .as_ref()
                .map(|c| c.label().to_owned())
                .unwrap_or_default();
            let line = match (name.is_empty(), a.text.is_empty()) {
                (false, false) => format!("{name}: {}", a.text),
                (true, _) => a.text.clone(),
                (false, true) => name,
            };
            out.push((Style::Body, line));
        }
    }
    out
}

fn fmt(v: f64) -> String {
    sr::format_number((v * 1000.0).round() / 1000.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Registry;

    fn fixture(rel: &str) -> Arc<Source> {
        let p = format!(
            "{}/../formats/specular-dicom/tests/data/dicomrt/{rel}",
            env!("CARGO_MANIFEST_DIR")
        );
        let data = std::fs::read(&p).unwrap_or_else(|e| panic!("{p}: {e}"));
        Arc::new(Source::from_vec(rel, data))
    }

    /// A report opens as its tree on a page, with its kind and title as encapsulated facts;
    /// neither the patient nor the observer is named on the page.
    #[test]
    fn report_opens_as_its_tree() {
        for (f, kind) in [
            ("study/sr_basic.dcm", "Basic Text SR"),
            ("study/sr_enhanced.dcm", "Enhanced SR"),
            ("study/sr_comprehensive.dcm", "Comprehensive SR"),
            ("study/sr_tid1500.dcm", "Comprehensive 3D SR"),
            ("study/sr_tid1500_3d.dcm", "Comprehensive 3D SR"),
        ] {
            let src = fixture(f);
            let s = Registry::default().open(src.clone()).expect(f);
            let e = s.encapsulated().expect(f);
            assert_eq!(e.kind, SR_KIND);
            assert!(
                e.title.as_deref().unwrap().ends_with(&format!("({kind})")),
                "{f}"
            );
            assert!(s.info().coding.starts_with("DICOM encapsulated SR"), "{f}");
            assert!(s.info().width > 0);
            let d = dcm::Dicom::parse(src.bytes()).unwrap();
            let r = Report::read(&d, src.bytes()).unwrap();
            let text = report_text(&r);
            let items = text.iter().take_while(|(_, l)| !l.is_empty()).count();
            assert_eq!(items, r.walk().len(), "{f}: title and one line per item");
            for (_, l) in &text {
                assert!(!l.contains("Curie") && !l.contains("Phantom"), "{f}: {l}");
            }
        }
    }

    /// A waveform opens as its summary page; ECG channels are listed with their leads.
    #[test]
    fn waveform_opens_as_its_summary() {
        for f in [
            "ecg/ecg_12lead.dcm",
            "ecg/ecg_general_8bit.dcm",
            "ecg/hemodynamic.dcm",
            "ecg/audio_mulaw.dcm",
        ] {
            let src = fixture(f);
            let s = Registry::default().open(src.clone()).expect(f);
            assert_eq!(s.encapsulated().expect(f).kind, WAVEFORM_KIND, "{f}");
        }
        let src = fixture("ecg/ecg_12lead.dcm");
        let d = dcm::Dicom::parse(src.bytes()).unwrap();
        let w = Waveforms::read(&d, src.bytes()).unwrap();
        let text = waveform_text(&w);
        assert_eq!(text[0].1, "12-lead ECG");
        for lead in ["I", "aVR", "V6"] {
            assert!(
                text.iter()
                    .any(|(_, l)| l.starts_with(&format!("{lead} · "))),
                "{lead}: {text:?}"
            );
        }
    }
}
