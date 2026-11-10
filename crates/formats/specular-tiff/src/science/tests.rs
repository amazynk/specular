use super::*;

#[test]
fn imagej_description() {
    let d = "ImageJ=1.54f\nimages=24\nchannels=2\nslices=3\nframes=4\nhyperstack=true\n\
             mode=composite\nunit=micron\nspacing=0.5\nfinterval=2.0\nmin=10.0\nmax=200.0\n\
             cf=0\nc0=-32768.0\nc1=1.0\nvunit=Gray Value\nloop=false\n";
    let ij = ImageJ::parse(d).unwrap();
    assert_eq!(
        (ij.channels, ij.slices, ij.frames, ij.images),
        (2, 3, 4, 24)
    );
    assert!(ij.hyperstack);
    assert_eq!(ij.unit.as_deref(), Some("µm"));
    assert_eq!(ij.spacing, Some(0.5));
    assert_eq!(ij.finterval, Some(2.0));
    assert_eq!((ij.min, ij.max), (Some(10.0), Some(200.0)));
    let cal = ij.calibration().unwrap();
    assert_eq!(cal.apply(32768.0), 0.0);
    assert_eq!(ij.extra, vec![("loop".into(), "false".into())]);
    assert!(ImageJ::parse("hello").is_none());
}

#[test]
fn imagej_plain_stack_and_pixel_unit() {
    let ij = ImageJ::parse("ImageJ=1.11a\nimages=5\nunit=pixel\n").unwrap();
    assert_eq!((ij.channels, ij.slices, ij.frames), (1, 5, 1));
    assert_eq!(ij.unit, None);
    assert_eq!(normalise_unit("\\u00B5m").as_deref(), Some("µm"));
    assert_eq!(normalise_unit("nm").as_deref(), Some("nm"));
}

#[test]
fn calibration_polynomial() {
    let c = Calibration {
        coefficients: vec![1.0, 2.0, 3.0],
    };
    assert_eq!(c.apply(2.0), 1.0 + 4.0 + 12.0);
    assert!(
        Calibration {
            coefficients: vec![0.0, 1.0]
        }
        .is_identity()
    );
}

fn ij_meta(le: bool) -> (Vec<u8>, Vec<u64>) {
    let w32 = |v: u32| if le { v.to_le_bytes() } else { v.to_be_bytes() };
    let utf16 = |s: &str| -> Vec<u8> {
        s.encode_utf16()
            .flat_map(|u| if le { u.to_le_bytes() } else { u.to_be_bytes() })
            .collect()
    };
    let mut d = Vec::new();
    d.extend_from_slice(if le { b"JIJI" } else { b"IJIJ" });
    for (t, n) in [(b"info", 1u32), (b"labl", 2), (b"rang", 1), (b"luts", 1)] {
        d.extend_from_slice(&w32(u32::from_be_bytes(*t)));
        d.extend_from_slice(&w32(n));
    }
    let mut counts = vec![d.len() as u64];
    for s in ["hello", "a", "bc"] {
        let b = utf16(s);
        counts.push(b.len() as u64);
        d.extend(b);
    }
    let mut r = Vec::new();
    for v in [1.0f64, 2.0, 3.0, 4.0] {
        r.extend_from_slice(&if le { v.to_le_bytes() } else { v.to_be_bytes() });
    }
    counts.push(r.len() as u64);
    d.extend(r);
    let lut: Vec<u8> = (0..3)
        .flat_map(|k| (0..=255u8).map(move |i| if k == 1 { i } else { 0 }))
        .collect();
    counts.push(768);
    d.extend(lut);
    (d, counts)
}

#[test]
fn ij_metadata_both_orders() {
    for le in [false, true] {
        let (d, c) = ij_meta(le);
        let m = IjMetadata::parse(&d, &c).unwrap();
        assert_eq!(m.info.as_deref(), Some("hello"));
        assert_eq!(m.labels, vec!["a".to_string(), "bc".into()]);
        assert_eq!(m.ranges, vec![(1.0, 2.0), (3.0, 4.0)]);
        assert_eq!(m.luts.len(), 1);
        assert_eq!(ramp_colour(&m.luts[0]), Some([0, 255, 0]));
    }
    assert!(IjMetadata::parse(b"XXXX", &[4]).is_none());
}

#[test]
fn ramp_rejects_inverted_grey() {
    let mut lut: Lut = Box::new([[0; 3]; 256]);
    for (i, e) in lut.iter_mut().enumerate() {
        *e = [(255 - i) as u8; 3];
    }
    assert_eq!(ramp_colour(&lut), None);
}

#[test]
fn lsm_info() {
    let mut file = vec![0u8; 1024];
    let mut tag = vec![0u8; 512];
    tag[0..4].copy_from_slice(&0x0400_494Cu32.to_le_bytes());
    tag[4..8].copy_from_slice(&512u32.to_le_bytes());
    for (o, v) in [(8, 64u32), (12, 32), (16, 5), (20, 2), (24, 3), (28, 1)] {
        tag[o..o + 4].copy_from_slice(&v.to_le_bytes());
    }
    tag[40..48].copy_from_slice(&0.2e-6f64.to_le_bytes());
    tag[48..56].copy_from_slice(&0.2e-6f64.to_le_bytes());
    tag[56..64].copy_from_slice(&1e-6f64.to_le_bytes());
    tag[112..120].copy_from_slice(&1.5f64.to_le_bytes());
    tag[108..112].copy_from_slice(&600u32.to_le_bytes());
    // Colours block at 600: size, colours, names, colours at +24, names at +32.
    for (o, v) in [(0, 64u32), (4, 2), (8, 2), (12, 24), (16, 32)] {
        file[600 + o..604 + o].copy_from_slice(&v.to_le_bytes());
    }
    file[624..628].copy_from_slice(&[255, 0, 0, 0]);
    file[628..632].copy_from_slice(&[0, 255, 0, 0]);
    let mut p = 632;
    for n in ["Ch1", "Ch2"] {
        file[p..p + 4].copy_from_slice(&4u32.to_le_bytes());
        file[p + 4..p + 7].copy_from_slice(n.as_bytes());
        p += 8;
    }
    tag[132..136].copy_from_slice(&800u32.to_le_bytes());
    file[804..808].copy_from_slice(&2u32.to_le_bytes());
    file[808..816].copy_from_slice(&10.0f64.to_le_bytes());
    file[816..824].copy_from_slice(&11.5f64.to_le_bytes());
    let i = LsmInfo::parse(&file, &tag).unwrap();
    assert_eq!(i.dims, [64, 32, 5, 2, 3]);
    assert_eq!(i.voxel, [0.2e-6, 0.2e-6, 1e-6]);
    assert_eq!(i.time_interval, 1.5);
    assert_eq!(i.channel_colours, vec![[255, 0, 0], [0, 255, 0]]);
    assert_eq!(i.channel_names, vec!["Ch1".to_string(), "Ch2".into()]);
    assert_eq!(i.timestamps, vec![10.0, 11.5]);
    assert!(LsmInfo::parse(&file, &[0; 16]).is_none());
}

#[test]
fn gel_values() {
    let g = MdGel {
        file_tag: 2,
        scale: 0.5,
        ..Default::default()
    };
    assert_eq!(g.value(4.0), 8.0);
    let g = MdGel { file_tag: 128, ..g };
    assert_eq!(g.value(4.0), 2.0);
}

#[test]
fn fei_text() {
    let t = "[User]\r\nDate=03/14/2019\r\nTime=10:12:13 AM\r\n[System]\r\nSystemType=Helios G4 UX\r\n\
             [Beam]\r\nHV=5000\r\nBeam=EBeam\r\n[EBeam]\r\nHV=5000\r\nWD=0.004\r\n[Scan]\r\n\
             PixelWidth=1.5e-09\r\nPixelHeight=1.5e-09\r\n[Detectors]\r\nName=TLD\r\nMode=SE\r\n";
    let i = Instrument::fei(t);
    assert_eq!(i.pixel, Some((1.5e-9, 1.5e-9)));
    assert_eq!(i.voltage, Some(5000.0));
    assert_eq!(i.model.as_deref(), Some("Helios G4 UX"));
    assert_eq!(i.detector.as_deref(), Some("TLD"));
    assert_eq!(i.mode.as_deref(), Some("SE"));
    assert_eq!(i.working_distance, Some(0.004));
    assert_eq!(i.date.as_deref(), Some("03/14/2019 10:12:13 AM"));
}

#[test]
fn zeiss_text() {
    let t = "\0\r\nAP_PIXEL_SIZE\r\nPixel Size = 2.5 nm\r\nAP_MAG\r\nMag = 25.00 K X\r\n\
             AP_ACTUALKV\r\nEHT = 5.00 kV\r\nDP_DETECTOR_CHANNEL\r\nSignal A = InLens\r\n\
             AP_WD\r\nWD = 3.9 mm\r\nAP_DATE\r\nDate :14 Mar 2019\r\n";
    let i = Instrument::zeiss(t);
    assert_eq!(i.pixel, Some((2.5e-9, 2.5e-9)));
    assert_eq!(i.magnification, Some(25000.0));
    assert_eq!(i.voltage, Some(5000.0));
    assert_eq!(i.detector.as_deref(), Some("InLens"));
    assert!((i.working_distance.unwrap() - 3.9e-3).abs() < 1e-15);
    assert_eq!(i.date.as_deref(), Some("14 Mar 2019"));
    // Stored at half the scan resolution: the image's pixel, not the scan's.
    let t = "AP_PIXEL_SIZE\r\nPixel Size = 11.16 nm\r\nAP_IMAGE_PIXEL_SIZE\r\n\
             Image Pixel Size = 22.33 nm\r\n";
    let p = Instrument::zeiss(t).pixel.unwrap();
    assert!((p.0 - 22.33e-9).abs() < 1e-18 && p.0 == p.1);
}

#[test]
fn tescan_text() {
    let t = "[MAIN]\r\nDevice=MIRA3\r\nHV=15000\r\nMagnification=5000\r\nPixelSizeX=4e-08\r\n\
             PixelSizeY=4e-08\r\nDetector=SE\r\nWD=0.01\r\n";
    let i = Instrument::tescan(t);
    assert_eq!(i.pixel, Some((4e-8, 4e-8)));
    assert_eq!(i.voltage, Some(15000.0));
    assert_eq!(i.magnification, Some(5000.0));
    assert_eq!(i.model.as_deref(), Some("MIRA3"));
}

#[test]
fn sidecars() {
    let h = "[SemImageFile]\r\nInstructName=S-4800\r\nPixelSize=12.4\r\n\
             AcceleratingVoltage=15000 Volt\r\nMagnification=10000\r\nSignalName=SE\r\n";
    let i = Instrument::hitachi(h).unwrap();
    assert!((i.pixel.unwrap().0 - 12.4e-9).abs() < 1e-18);
    assert_eq!(i.voltage, Some(15000.0));
    assert_eq!(i.magnification, Some(10000.0));
    assert!(Instrument::hitachi("nothing").is_none());

    let j = "$CM_FORMAT JEOL/SEM\n$CM_INSTRUMENT JSM-7800F\n$CM_MAG 5000\n$CM_ACCEL_VOLT 15.0\n\
             $CM_SIGNAL SEI\n$CM_FULL_SIZE 1280 960\n$$SM_MICRON_BAR 200\n$$SM_MICRON_MARKER 1um\n";
    let i = Instrument::jeol(j, 640).unwrap();
    assert!((i.pixel.unwrap().0 - 1e-8).abs() < 1e-18);
    assert_eq!(i.voltage, Some(15000.0));
    assert_eq!(i.model.as_deref(), Some("JSM-7800F"));
}

#[test]
fn numbers_with_units() {
    assert_eq!(number_unit("5.00 kV"), Some((5.0, "kV")));
    assert_eq!(number_unit("1.5e-09"), Some((1.5e-9, "")));
    assert_eq!(magnification("1.2 M X"), Some(1.2e6));
    assert_eq!(length("1um"), Some(1e-6));
}

#[test]
fn lut_files() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/scitiff/");
    let json = std::fs::read_to_string(format!("{dir}luts.json")).unwrap();
    for name in ["raw.lut", "icol.lut", "text.lut"] {
        let t = parse_lut(&std::fs::read(format!("{dir}{name}")).unwrap()).unwrap();
        // luts.json: {"name": [[r, g, b], ...], ...}.
        let at = json.find(&format!("\"{name}\":")).unwrap() + name.len() + 3;
        let body = &json[at..];
        let body = &body[..body.find("]]").unwrap() + 2];
        let want: Vec<u8> = body
            .split(|c: char| !c.is_ascii_digit())
            .filter(|c| !c.is_empty())
            .map(|c| c.parse().unwrap())
            .collect();
        let got: Vec<u8> = t.iter().flatten().copied().collect();
        assert_eq!(got, want, "{name}");
    }
    // Four rows stretched over 256, ImageJ-style.
    let t = parse_lut(b"r g b\n0 0 0\n255 0 0\n0 255 0\n0 0 255\n").unwrap();
    assert_eq!(
        (t[0], t[64], t[128], t[255]),
        ([0; 3], [255, 0, 0], [0, 255, 0], [0, 0, 255])
    );
    assert_eq!(t[32], [128, 0, 0]);
    assert!(parse_lut(b"hello").is_none());
    assert!(parse_lut(b"1 2\n3 4\n").is_none());
}
