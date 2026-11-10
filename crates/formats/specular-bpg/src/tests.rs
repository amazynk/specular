use super::*;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(format!("{name}.bpg")),
    )
    .unwrap()
}

/// FNV-1a 64 of the samples as an RGBA PAM stores them (8-bit, or 16-bit big-endian).
fn pam_hash(img: &Image) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    let mut eat = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100_0000_01b3);
    };
    match img.format {
        PixelFormat::Rgba8 => img.data.iter().for_each(|&b| eat(b)),
        PixelFormat::Gray8 => img
            .data
            .iter()
            .for_each(|&g| [g, g, g, 255].into_iter().for_each(&mut eat)),
        PixelFormat::Rgba16 | PixelFormat::Gray16 => {
            for c in img.data.chunks_exact(2) {
                let v = u16::from_ne_bytes([c[0], c[1]]);
                let px: &[u16] = if img.format == PixelFormat::Gray16 {
                    &[v, v, v, 65535]
                } else {
                    &[v]
                };
                for s in px {
                    s.to_be_bytes().into_iter().for_each(&mut eat);
                }
            }
        }
    }
    h
}

/// The committed fixtures decode to libbpg's pixels, frame for frame, with its durations.
#[test]
fn fixtures_match_libbpg() {
    let refs = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/refs.txt"),
    )
    .unwrap();
    let mut by_file: Vec<(String, Vec<Vec<String>>)> = Vec::new();
    for l in refs.lines().filter(|l| !l.starts_with('#')) {
        let f: Vec<String> = l.split(' ').map(str::to_owned).collect();
        match by_file.last_mut() {
            Some((n, v)) if *n == f[0] => v.push(f),
            _ => by_file.push((f[0].clone(), vec![f])),
        }
    }
    assert!(by_file.len() >= 10);
    for (name, frames) in by_file {
        let data: Arc<[u8]> = fixture(&name).into();
        let mut fr = match Frames::new(data.clone(), 1) {
            Ok(f) => f,
            Err(e) => panic!("{name}: {e}"),
        };
        assert_eq!(fr.frame_count() as usize, frames.len(), "{name}");
        for f in &frames {
            let (img, d) = fr.next_frame(None).unwrap().unwrap();
            assert_eq!(img.width.to_string(), f[2], "{name}");
            assert_eq!(img.height.to_string(), f[3], "{name}");
            assert_eq!(
                format!("{:016x}", pam_hash(&img)),
                f[5],
                "{name} frame {}",
                f[1]
            );
            let (num, den): (f64, f64) = (f[6].parse().unwrap(), f[7].parse().unwrap());
            if num > 0.0 {
                assert!((d.as_secs_f64() - num / den).abs() < 1e-9, "{name}");
            }
            assert!(!img.truncated);
        }
        assert!(fr.next_frame(None).is_none());
        // The still path gives the first frame.
        let still = decode(&data, &Options::default()).unwrap();
        assert_eq!(format!("{:016x}", pam_hash(&still)), frames[0][5], "{name}");
    }
}

#[test]
fn header_facts() {
    let i = read_info(&fixture("meta-exif")).unwrap();
    assert_eq!((i.width, i.height, i.bit_depth), (333, 211, 8));
    assert_eq!(
        (i.chroma, i.colour_space),
        (Chroma::C420, ColourSpace::YCbCr601)
    );
    assert!(
        i.exif.as_ref().is_some_and(|e| e.starts_with(b"Exif")
            || e.starts_with(b"II")
            || e.starts_with(b"MM"))
    );
    assert!(
        i.xmp
            .as_ref()
            .is_some_and(|x| x.windows(9).any(|w| w == b"x:xmpmeta"))
    );
    assert!(i.icc.is_none() && i.animation.is_none());
    let i = read_info(&fixture("photo-422video")).unwrap();
    assert_eq!((i.chroma, i.video_siting), (Chroma::C422, true));
    let i = read_info(&fixture("cmyk")).unwrap();
    assert!(i.cmyk && i.alpha == AlphaKind::None);
    let i = read_info(&fixture("alpha-premul")).unwrap();
    assert_eq!(i.alpha, AlphaKind::Premultiplied);
    let i = read_info(&fixture("anim-alpha-delays")).unwrap();
    let a = i.animation.unwrap();
    assert_eq!(
        (a.loop_count, a.frame_period_num, a.frame_period_den),
        (0, 1, 25)
    );
    let i = read_info(&fixture("grey-10")).unwrap();
    assert_eq!((i.chroma, i.bit_depth), (Chroma::Gray, 10));
    assert_eq!(
        decode(&fixture("grey-10"), &Options::default())
            .unwrap()
            .format,
        PixelFormat::Gray16
    );
    assert_eq!(
        decode(&fixture("logo-ycgco-420"), &Options::default())
            .unwrap()
            .format,
        PixelFormat::Rgba8
    );
}

fn ue7_bytes(mut v: u32) -> Vec<u8> {
    let mut out = vec![(v & 0x7f) as u8];
    v >>= 7;
    while v > 0 {
        out.insert(0, 0x80 | (v & 0x7f) as u8);
        v >>= 7;
    }
    out
}

/// `file` (without extensions) with an extension block of `exts` (tag, bytes) added.
fn with_extensions(file: &[u8], exts: &[(u32, &[u8])]) -> Vec<u8> {
    let mut at = 6;
    let mut fields = Vec::new();
    for _ in 0..3 {
        let (v, n) = header::ue7(file, at).unwrap();
        fields.push(v);
        at += n;
    }
    assert_eq!(file[5] & 0x08, 0);
    let mut body = Vec::new();
    for (tag, b) in exts {
        body.extend(ue7_bytes(*tag));
        body.extend(ue7_bytes(b.len() as u32));
        body.extend_from_slice(b);
    }
    let mut out = file[..6].to_vec();
    out[5] |= 0x08;
    for v in &fields {
        out.extend(ue7_bytes(*v));
    }
    out.extend(ue7_bytes(body.len() as u32));
    out.extend(body);
    out.extend_from_slice(&file[at..]);
    out
}

#[test]
fn thumbnail_and_icc_extensions() {
    let thumb = fixture("grad-12-420");
    let file = with_extensions(
        &fixture("grey-10"),
        &[(2, b"icc!"), (4, &thumb), (99, b"skip")],
    );
    let i = read_info(&file).unwrap();
    assert!(i.has_thumbnail);
    assert_eq!(i.icc.as_deref(), Some(&b"icc!"[..]));
    let t = decode_thumbnail(&file).unwrap().unwrap();
    assert_eq!((t.width, t.height), (257, 129));
    let main = decode(&file, &Options::default()).unwrap();
    assert_eq!(
        pam_hash(&main),
        pam_hash(&decode(&fixture("grey-10"), &Options::default()).unwrap())
    );
    assert!(decode_thumbnail(&fixture("grey-10")).is_none());
}

#[test]
fn refusals_are_plain() {
    // 14-bit stills decode; a 14-bit animation is refused before decoding.
    let still = fixture("grad-14-444");
    assert!(decode(&still, &Options::default()).is_ok());
    let mut anim = fixture("anim-alpha-delays");
    anim[4] = (anim[4] & 0xf0) | 6;
    match Frames::new(anim.into(), 1) {
        Err(Error::Unsupported(m)) => assert_eq!(m, "BPG animation at 14 bits per sample"),
        Err(e) => panic!("{e}"),
        Ok(_) => panic!("14-bit animation accepted"),
    }
    let mut f = fixture("grey-10");
    f[4] = (f[4] & 0xf0) | 7;
    assert_eq!(
        read_info(&f).unwrap_err(),
        Error::Unsupported("BPG at 15 bits per sample (the BPG specification stops at 14)".into())
    );
    let mut f = fixture("photo-422video");
    f[4] = (f[4] & 0x1f) | 6 << 5;
    assert!(matches!(read_info(&f), Err(Error::Unsupported(_))));
    let mut f = fixture("photo-422video");
    f[5] = (f[5] & 0x0f) | 5 << 4;
    assert_eq!(
        read_info(&f).unwrap_err(),
        Error::Unsupported(
            "BPG BT.2020 constant-luminance colour (reserved by the BPG specification)".into()
        )
    );
}

#[test]
fn cancel_and_memory_budget() {
    let c = AtomicBool::new(true);
    let r = decode(
        &fixture("logo-ycgco-420"),
        &Options {
            threads: 1,
            cancel: Some(&c),
        },
    );
    assert_eq!(r.unwrap_err(), Error::Cancelled);
    let r = specular_core::budget::with_limit(1 << 10, || {
        decode(&fixture("logo-ycgco-420"), &Options::default())
    });
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
}

#[test]
fn rewind_replays_the_animation() {
    let mut f = Frames::new(fixture("anim-alpha-delays").into(), 1).unwrap();
    let first = f.next_frame(None).unwrap().unwrap().0;
    let _ = f.next_frame(None).unwrap().unwrap();
    f.rewind().unwrap();
    let again = f.next_frame(None).unwrap().unwrap().0;
    assert_eq!(first.data, again.data);
}

/// Cut and corrupted files: no panic, and decodes stay bounded.
#[test]
fn damaged_files_fail_cleanly() {
    let mut seed = 0x9e37_79b9u32;
    for name in [
        "alpha-premul",
        "anim-alpha-delays",
        "meta-exif",
        "photo-422video",
        "cmyk",
    ] {
        let good = fixture(name);
        let try_one = |b: Vec<u8>| {
            let _ = read_info(&b);
            let _ = decode(&b, &Options::default());
            let _ = decode_thumbnail(&b);
            if let Ok(mut f) = Frames::new(b.into(), 1) {
                while let Some(r) = f.next_frame(None) {
                    if r.is_err() {
                        break;
                    }
                }
            }
        };
        for n in (0..good.len()).step_by((good.len() / 97).max(1)) {
            try_one(good[..n].to_vec());
        }
        for _ in 0..120 {
            let mut b = good.clone();
            for _ in 0..4 {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                // Mostly the header and parameter bytes, sometimes the slices.
                let span = if seed & 3 == 0 {
                    b.len()
                } else {
                    b.len().min(64)
                };
                let at = (seed >> 3) as usize % span;
                b[at] = (seed >> 11) as u8;
            }
            try_one(b);
        }
    }
    // Implausible sizes are refused before any allocation.
    let mut f = b"BPG\xfb\x20\x00".to_vec();
    f.extend(ue7_bytes(70_000));
    f.extend(ue7_bytes(10));
    f.extend([0, 1, 2, 3]);
    assert!(matches!(
        decode(&f, &Options::default()),
        Err(Error::Unsupported(_))
    ));
    let mut f = b"BPG\xfb\x20\x00".to_vec();
    f.extend(ue7_bytes(65_000));
    f.extend(ue7_bytes(65_000));
    f.extend([0, 1, 2, 3]);
    assert_eq!(
        decode(&f, &Options::default()).unwrap_err(),
        Error::TooLarge
    );
}
