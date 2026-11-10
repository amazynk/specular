//! SMV ("Simple Molecular Viewer"), the `.img` format of ADSC CCDs and of Rigaku d*TREK /
//! CrystalClear (Saturn, HyPix, R-AXIS IV++ in SMV mode): a `{`, `KEY=value;` lines, `}`,
//! padded with spaces or NULs to `HEADER_BYTES`; uncompressed samples follow.
//!
//! ADSC headers give lengths in mm and the beam centre in mm (`BEAM_CENTER_X` along the slow
//! axis, `BEAM_CENTER_Y` along the fast one, the convention DIALS and XDS use for these
//! detectors). d*TREK headers put detector items under a prefix (`DETECTOR_NAMES=PXD_`): the
//! beam in pixels (`PXD_SPATIAL_BEAM_POSITION`), pixel size in `PXD_SPATIAL_DISTORTION_INFO`,
//! distance and 2θ among the goniometer values; `ROTATION` is start, end and increment.

use crate::geometry::Geometry;
use crate::{Coding, Error, Frame, Image, Kind, Result, Sample, bad, finite, positive, text};

/// Whether the file starts with an SMV header (`{`, then `HEADER_BYTES=` first).
pub fn sniff(head: &[u8]) -> bool {
    let Some(rest) = head.strip_prefix(b"{") else {
        return false;
    };
    let s = rest
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .unwrap_or(rest.len());
    rest[s..].starts_with(b"HEADER_BYTES=")
}

/// The header's `KEY=value` pairs, file order.
fn pairs(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim().trim_end_matches(';').trim_end();
            let (k, v) = l.split_once('=')?;
            let k = k.trim();
            (!k.is_empty() && !k.contains('{')).then(|| (k.to_owned(), v.trim().to_owned()))
        })
        .collect()
}

pub(crate) fn parse(data: &[u8]) -> Result<Image> {
    let head = &data[..data.len().min(1 << 16)];
    let hb_text = String::from_utf8_lossy(&head[..head.len().min(64)]);
    let hb = hb_text
        .split("HEADER_BYTES=")
        .nth(1)
        .and_then(|r| r.split(';').next())
        .and_then(|n| n.trim().parse::<usize>().ok())
        .ok_or_else(|| bad("HEADER_BYTES"))?;
    if hb > data.len() {
        return Err(Error::Truncated);
    }
    // HEADER_BYTES=0 would make the range run backwards.
    let raw = data.get(1..hb).ok_or_else(|| bad("HEADER_BYTES"))?;
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    let txt = String::from_utf8_lossy(&raw[..end]);
    let header = pairs(&txt);
    let get = |k: &str| {
        header
            .iter()
            .rev()
            .find(|(hk, _)| hk.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    };
    let int = |k: &str| get(k).and_then(|v| v.trim().parse::<u64>().ok());
    let w = int("SIZE1").ok_or_else(|| bad("no SIZE1"))?;
    let h = int("SIZE2").ok_or_else(|| bad("no SIZE2"))?;
    let w = u32::try_from(w).map_err(|_| bad("SIZE1"))?;
    let h = u32::try_from(h).map_err(|_| bad("SIZE2"))?;
    let big = get("BYTE_ORDER").is_some_and(|b| b.to_ascii_lowercase().starts_with("big"));
    let sample = match get("Data_type").map(str::to_ascii_lowercase).as_deref() {
        Some("unsigned short int") | None => match get("TYPE").map(str::to_ascii_lowercase) {
            None => Sample::U16,
            Some(t) => match t.as_str() {
                "unsigned_short" | "unsigned short" => Sample::U16,
                "signed_short" | "short" => Sample::I16,
                "unsigned_long" | "unsigned_int" | "unsigned long" => Sample::U32,
                "signed_long" | "long" | "int" | "signed_int" => Sample::I32,
                "float" | "float_ieee" => Sample::F32,
                "unsigned_char" | "unsigned_byte" => Sample::U8,
                _ => return Err(Error::Unsupported(format!("SMV TYPE \"{t}\""))),
            },
        },
        Some("short int" | "signed short int") => Sample::I16,
        Some("long int" | "signed long int") => Sample::I32,
        Some("unsigned long int") => Sample::U32,
        Some("float ieee" | "float") => Sample::F32,
        Some("unsigned char") => Sample::U8,
        Some(t) => return Err(Error::Unsupported(format!("SMV data type \"{t}\""))),
    };
    if let Some(c) = get("COMPRESSION")
        && !c.eq_ignore_ascii_case("none")
    {
        return Err(Error::Unsupported(format!("SMV compression \"{c}\"")));
    }
    let num = |k: &str| get(k).and_then(crate::first_number);
    let nums = |k: &str| -> Vec<f64> {
        get(k)
            .map(|v| crate::numbers(v).collect())
            .unwrap_or_default()
    };
    let mut g = Geometry::default();
    let detector;
    if let Some(p) = get("DETECTOR_NAMES").and_then(|v| v.split_whitespace().next()) {
        // d*TREK.
        let p = p.to_owned();
        let key = |s: &str| format!("{p}{s}");
        let beam = nums(&key("SPATIAL_BEAM_POSITION"));
        if beam.len() >= 2 {
            g.beam = Some((beam[0], beam[1]));
        }
        let dist = nums(&key("SPATIAL_DISTORTION_INFO"));
        if dist.len() >= 4 && dist[2] > 0.0 && dist[3] > 0.0 {
            g.pixel = Some((dist[2], dist[3]));
            if g.beam.is_none() {
                g.beam = Some((dist[0], dist[1]));
            }
        }
        let names: Vec<String> = get(&key("GONIO_NAMES"))
            .map(|v| v.split_whitespace().map(str::to_ascii_lowercase).collect())
            .unwrap_or_default();
        let values = nums(&key("GONIO_VALUES"));
        for (n, v) in names.iter().zip(&values) {
            match n.as_str() {
                "distance" => g.distance = positive(Some(*v)),
                "2theta" => g.two_theta = finite(Some(*v)),
                _ => {}
            }
        }
        detector = get(&key("DETECTOR_DESCRIPTION")).and_then(text);
        let rot = nums("ROTATION");
        if rot.len() >= 3 && (rot[1] != rot[0] || rot[2] != 0.0) {
            g.osc_start = Some(rot[0]);
            g.osc_width = Some(rot[1] - rot[0]);
        }
        g.osc_axis = get("ROTATION_AXIS_NAME")
            .or(get("SCAN_ROTATION_AXIS"))
            .and_then(text);
        g.exposure = positive(num("EXPOSURE_TIME")).or(positive(num("TIME")));
        // `CRYSTAL_GONIO_NAMES` / `VALUES`: omega, chi, phi, kappa.
        let cn: Vec<String> = get("CRYSTAL_GONIO_NAMES")
            .map(|v| v.split_whitespace().map(str::to_ascii_lowercase).collect())
            .unwrap_or_default();
        for (n, v) in cn.iter().zip(nums("CRYSTAL_GONIO_VALUES")) {
            match n.as_str() {
                "omega" => g.omega = Some(v),
                "chi" => g.chi = Some(v),
                "phi" => g.phi = Some(v),
                "kappa" => g.kappa = Some(v),
                _ => {}
            }
        }
    } else {
        // ADSC (and its imitators).
        let px = positive(num("PIXEL_SIZE"));
        g.pixel = px.map(|p| (p, p));
        if let (Some(p), Some(bx), Some(by)) = (
            px,
            finite(num("BEAM_CENTER_X")),
            finite(num("BEAM_CENTER_Y")),
        ) {
            g.beam = Some((by / p, bx / p));
        }
        g.distance = positive(num("DISTANCE"));
        g.two_theta = finite(num("TWOTHETA")).or(finite(num("TWO_THETA")));
        g.osc_start = finite(num("OSC_START"));
        g.osc_width = finite(num("OSC_RANGE"));
        g.osc_axis = get("AXIS").and_then(text);
        g.phi = finite(num("PHI"));
        g.omega = finite(num("OMEGA"));
        g.kappa = finite(num("KAPPA"));
        g.chi = finite(num("CHI"));
        g.exposure = positive(num("TIME")).or(positive(num("EXPOSURE_TIME")));
        detector = get("DETECTOR_SN")
            .and_then(text)
            .map(|sn| format!("ADSC S/N {sn}"));
    }
    // `WAVELENGTH=0.979` (ADSC) or `WAVELENGTH=n λ1 …` (d*TREK: a count first).
    let wl = |k: &str| {
        let v = nums(k);
        match v[..] {
            [w] => positive(Some(w)),
            [n, w, ..] if n.fract() == 0.0 && n >= 1.0 => positive(Some(w)),
            [w, ..] => positive(Some(w)),
            [] => None,
        }
    };
    g.wavelength = wl("WAVELENGTH")
        .or_else(|| wl("SOURCE_WAVELENGTH"))
        .or_else(|| positive(num("SCAN_WAVELENGTH")));
    g.overload = positive(num("SATURATED_VALUE")).or(positive(num("CCD_IMAGE_SATURATION")));
    // ADSC `BIN=2x2` (or `none`).
    g.binning = get("BIN")
        .or(get("BINNING"))
        .and_then(crate::edf::binning_text);
    let date = get("DATE").or(get("DTREK_DATE_TIME")).and_then(text);
    let title = get("SAMPLE").or(get("FILENAME")).and_then(text);
    Ok(Image {
        kind: Kind::Smv,
        width: w,
        height: h,
        sample,
        frames: vec![Frame {
            coding: Coding::Raw {
                offset: hb,
                sample,
                big,
            },
            header: Vec::new(),
        }],
        header,
        geometry: g,
        detector,
        date,
        mask_negative: false,
        dummy: None,
        title,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn smv(lines: &str, data: &[u8]) -> Vec<u8> {
        let mut t = format!("{{\nHEADER_BYTES=  512;\n{lines}}}\n").into_bytes();
        t.resize(512, b' ');
        t.extend(data);
        t
    }

    #[test]
    fn adsc() {
        let mut px = Vec::new();
        for v in [1u16, 2, 3, 4, 5, 60000] {
            px.extend(v.to_be_bytes());
        }
        let d = smv(
            "DIM=2;\nBYTE_ORDER=big_endian;\nTYPE=unsigned_short;\nSIZE1=3;\nSIZE2=2;\nPIXEL_SIZE=0.1;\nDISTANCE=200;\nWAVELENGTH=0.9794;\nBEAM_CENTER_X=0.1;\nBEAM_CENTER_Y=0.25;\nOSC_START=85;\nOSC_RANGE=1;\nTWOTHETA=0.0;\nDETECTOR_SN=901;\n",
            &px,
        );
        assert!(sniff(&d));
        assert!(!crate::edf::sniff(&d));
        let im = Image::parse(&d).unwrap();
        assert_eq!(im.kind, Kind::Smv);
        assert_eq!((im.width, im.height), (3, 2));
        let g = &im.geometry;
        let (bx, by) = g.beam.unwrap();
        assert!((bx - 2.5).abs() < 1e-9 && (by - 1.0).abs() < 1e-9);
        assert_eq!(g.distance, Some(200.0));
        assert_eq!(g.wavelength, Some(0.9794));
        assert_eq!(g.osc_width, Some(1.0));
        assert_eq!(im.detector.as_deref(), Some("ADSC S/N 901"));
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, 2.0, 3.0, 4.0, 5.0, 60000.0]);
    }

    #[test]
    fn dtrek() {
        let mut px = Vec::new();
        for v in [1i32, -2, 3, 4] {
            px.extend(v.to_le_bytes());
        }
        let d = smv(
            "ASC_NAMES={ Z X Y };\nBYTE_ORDER=little_endian;\nCOMPRESSION=None;\nDETECTOR_NAMES=PXD_;\nData_type=long int;\nPXD_DETECTOR_DESCRIPTION=HyPix-3000;\nPXD_GONIO_NAMES=RotAboutBeam 2Theta RotY XShift YShift Distance;\nPXD_GONIO_VALUES=0 10.0 0 0 0 490.3;\nPXD_SPATIAL_BEAM_POSITION=1.5 0.5;\nPXD_SPATIAL_DISTORTION_INFO=1.5 0.5 0.1 0.1;\nROTATION=10 10.5 0.5 5.4e+003 0 0 0 100 0 0;\nSATURATED_VALUE= 2147483648.0;\nSIZE1=2;\nSIZE2=2;\nWAVELENGTH=1.0 1.541870;\n",
            &px,
        );
        let im = Image::parse(&d).unwrap();
        let g = &im.geometry;
        assert_eq!(g.beam, Some((1.5, 0.5)));
        assert_eq!(g.pixel, Some((0.1, 0.1)));
        assert_eq!(g.distance, Some(490.3));
        assert_eq!(g.two_theta, Some(10.0));
        assert_eq!(g.wavelength, Some(1.54187));
        assert_eq!(g.osc_start, Some(10.0));
        assert_eq!(g.osc_width, Some(0.5));
        assert_eq!(g.overload, Some(2147483648.0));
        assert_eq!(im.detector.as_deref(), Some("HyPix-3000"));
        assert_eq!(im.frame(&d, 0).unwrap(), [1.0, -2.0, 3.0, 4.0]);
    }

    mod hostile {
        use super::*;

        #[test]
        fn zero_header_bytes() {
            let d = b"{\nHEADER_BYTES=0;\nSIZE1=2;\nSIZE2=2;\n}\n".to_vec();
            assert!(parse(&d).is_err());
        }
    }
}
