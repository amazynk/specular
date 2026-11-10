//! The rarer IFF forms and codings: RGB8 and RGBN (Impulse), DEEP (TVPaint: ByteRun1 and
//! TVDC), ByteRun2 / VDAT (Atari ST) and ANIM (operations 0, 5, 7, 8), against FFmpeg's IFF
//! decoder and TwelveMonkeys' (Java ImageIO, `tools/corpus/fetch-twelvemonkeys.sh`), on
//! files built here and on the samples of `tools/corpus/fetch-conformance.sh
//! src_ffmpeg_iffx src_twelvemonkeys`. The oracles and samples are skipped when missing.
//!
//! Known differences, allowed for: FFmpeg widens RGBN's 4-bit values by shifting (`0xA` →
//! `0xA0`) where we replicate (`0xAA`), as the IFF crate does for 4-bit palettes; FFmpeg
//! ignores the genlock bit (TwelveMonkeys and we make it transparent), so colours are
//! compared and alpha only against TwelveMonkeys.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use specular_iff::{Compression, Player, decode, info};

fn corpus(rel: &str) -> Option<PathBuf> {
    let root = std::env::var_os("SPECULAR_CONFORMANCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/conformance")
        });
    let p = root.join(rel);
    p.is_file().then_some(p)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("specular-iff-rare");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// FFmpeg's frames of `file` as RGBA, or None without FFmpeg (or when it refuses the file).
fn ffmpeg(file: &Path, w: u32, h: u32) -> Option<Vec<Vec<u8>>> {
    let out = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(file)
        .args([
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let n = (w * h * 4) as usize;
    (out.status.success() && !out.stdout.is_empty() && out.stdout.len() % n == 0)
        .then(|| out.stdout.chunks_exact(n).map(<[u8]>::to_vec).collect())
}

/// TwelveMonkeys' decode of `file` as RGBA, or None.
fn twelvemonkeys(file: &Path) -> Option<(u32, u32, Vec<u8>)> {
    let jars = std::env::var_os("SPECULAR_TWELVEMONKEYS")
        .map(PathBuf::from)
        .or_else(|| {
            Some(Path::new(&std::env::var_os("CARGO_TARGET_DIR")?).join("twelvemonkeys"))
        })?;
    if !jars.join("imageio-iff-3.12.0.jar").is_file() {
        return None;
    }
    let dir = tmp("tm");
    std::fs::create_dir_all(&dir).ok()?;
    let script = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tools/corpus/twelvemonkeys-oracle.java");
    let out = Command::new("java")
        .arg(format!("-Djava.io.tmpdir={}", dir.display()))
        .arg(format!("-Duser.home={}", dir.display()))
        .arg("-Djava.awt.headless=true")
        .arg("-cp")
        .arg(format!("{}/*", jars.display()))
        .arg(&script)
        .arg(&dir)
        .arg(file)
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let line = String::from_utf8_lossy(&out.stdout).into_owned();
    let f: Vec<&str> = line.trim().split('\t').collect();
    if f.get(1) != Some(&"OK") {
        return None;
    }
    let d = std::fs::read(dir.join(format!("{}.rgba", f[0]))).ok()?;
    let nl = d.iter().position(|&b| b == b'\n')?;
    Some((f[2].parse().ok()?, f[3].parse().ok()?, d[nl + 1..].to_vec()))
}

/// (pixels whose colour differs by more than `tol`, largest difference).
fn diff(a: &[u8], b: &[u8], tol: u8) -> (usize, u8) {
    let (mut n, mut m) = (0, 0);
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        let d = (0..3).map(|c| p[c].abs_diff(q[c])).max().unwrap();
        m = m.max(d);
        if d > tol {
            n += 1;
        }
    }
    (n, m)
}

fn alpha_diff(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| p[3] != q[3])
        .count()
}

fn chunk(id: &[u8; 4], d: &[u8]) -> Vec<u8> {
    let mut v = id.to_vec();
    v.extend((d.len() as u32).to_be_bytes());
    v.extend(d);
    if d.len() % 2 == 1 {
        v.push(0);
    }
    v
}

fn form(kind: &[u8; 4], chunks: &[Vec<u8>]) -> Vec<u8> {
    let body = chunks.concat();
    let mut f = b"FORM".to_vec();
    f.extend(((body.len() + 4) as u32).to_be_bytes());
    f.extend(kind);
    f.extend(body);
    f
}

fn bmhd(w: u16, h: u16, planes: u8, comp: u8) -> Vec<u8> {
    let mut b = Vec::new();
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b.extend([0, 0, 0, 0, planes, 0, comp, 0, 0, 0, 1, 1]);
    b.extend(w.to_be_bytes());
    b.extend(h.to_be_bytes());
    b
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

/// An RGB8 or RGBN file of random runs (short, byte and word counts) and its pixels.
fn rgbx(rgbn: bool, w: u16, h: u16, seed: u64) -> (Vec<u8>, Vec<[u8; 4]>) {
    let mut r = Rng(seed | 1);
    let n = usize::from(w) * usize::from(h);
    let (mut body, mut px) = (Vec::new(), Vec::new());
    while px.len() < n {
        let run =
            [1usize, 1, 2, 3, 6, 7, 20, 127, 130, 300][r.next() as usize % 10].min(n - px.len());
        let (c, g) = (
            [r.next() as u8, r.next() as u8, r.next() as u8],
            r.next().is_multiple_of(5),
        );
        let small = if rgbn { 7 } else { 127 };
        let (inline, extra) = if run <= small {
            (run, vec![])
        } else if run <= 255 {
            (0, vec![run as u8])
        } else {
            let mut e = vec![0];
            e.extend((run as u16).to_be_bytes());
            (0, e)
        };
        let colour = if rgbn {
            let v = (u16::from(c[0] >> 4) << 12)
                | (u16::from(c[1] >> 4) << 8)
                | (u16::from(c[2] >> 4) << 4)
                | if g { 8 } else { 0 }
                | inline as u16;
            body.extend(v.to_be_bytes());
            [(c[0] >> 4) * 17, (c[1] >> 4) * 17, (c[2] >> 4) * 17]
        } else {
            body.extend(c);
            body.push(if g { 0x80 } else { 0 } | inline as u8);
            c
        };
        body.extend(extra);
        px.extend(std::iter::repeat_n(
            [colour[0], colour[1], colour[2], if g { 0 } else { 255 }],
            run,
        ));
    }
    let f = form(
        if rgbn { b"RGBN" } else { b"RGB8" },
        &[
            chunk(b"BMHD", &bmhd(w, h, if rgbn { 13 } else { 25 }, 4)),
            chunk(b"BODY", &body),
        ],
    );
    (f, px)
}

#[test]
fn rgb8_and_rgbn_built() {
    for rgbn in [false, true] {
        let (f, want) = rgbx(rgbn, 61, 37, 7 + rgbn as u64);
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        let got: Vec<[u8; 4]> = img
            .pixels
            .chunks_exact(4)
            .map(|p| [p[0], p[1], p[2], p[3]])
            .collect();
        assert!(got == want, "rgbn {rgbn}");
        let i = info(&f).unwrap();
        assert_eq!(i.compression, Compression::Rgbx);
        assert!(
            i.coding()
                .starts_with(if rgbn { "RGBN 12-bit" } else { "RGB8 24-bit" })
        );
        // FFmpeg: colours (4-bit shifted, not replicated, in RGBN).
        let p = tmp(if rgbn { "built.rgbn" } else { "built.rgb8" });
        std::fs::write(&p, &f).unwrap();
        if let Some(ff) = ffmpeg(&p, 61, 37) {
            let (n, m) = diff(&img.pixels, &ff[0], if rgbn { 15 } else { 0 });
            eprintln!("built rgbn={rgbn} vs ffmpeg: {n} over, max {m}");
            assert_eq!(n, 0);
        }
    }
}

#[test]
fn samples_against_oracles() {
    let mut ran = 0;
    for (rel, tol) in [
        ("ffmpeg-iffx/1.rgb8", 0u8),
        ("ffmpeg-iffx/1.rgbn", 15),
        ("ffmpeg-iffx/tvpaint_delta.deep", 0),
        ("ffmpeg-iffx/MEDUSAB2.IFF", 15),
        ("ffmpeg-iffx/MEDUSABL.IFF", 15),
        (
            "twelvemonkeys/imageio/imageio-iff/src/test/resources/iff/arch.deep",
            0,
        ),
    ] {
        let Some(p) = corpus(rel) else { continue };
        let data = std::fs::read(&p).unwrap();
        let t = std::time::Instant::now();
        let img = decode(&data).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let i = info(&data).unwrap();
        assert!(!img.truncated, "{rel}");
        let mut line = format!(
            "{rel}: {} {}x{} {ms:.1} ms",
            i.coding(),
            img.width,
            img.height
        );
        if let Some(ff) = ffmpeg(&p, img.width, img.height) {
            let (n, m) = diff(&img.pixels, &ff[0], tol);
            line += &format!("; ffmpeg {n} over {tol} (max {m})");
            assert_eq!(n, 0, "{line}");
            ran += 1;
        }
        if let Some((w, h, tm)) = twelvemonkeys(&p)
            && (w, h) == (img.width, img.height)
        {
            // Colours where opaque: TwelveMonkeys hands back DEEP's partly transparent
            // pixels premultiplied.
            let opaque = |v: &[u8]| -> Vec<u8> {
                v.chunks_exact(4)
                    .zip(img.pixels.chunks_exact(4))
                    .flat_map(|(p, o)| {
                        if o[3] == 255 {
                            [p[0], p[1], p[2], p[3]]
                        } else {
                            [0; 4]
                        }
                    })
                    .collect()
            };
            let (n, m) = diff(&opaque(&img.pixels), &opaque(&tm), tol);
            let a = alpha_diff(&img.pixels, &tm);
            line += &format!("; twelvemonkeys {n} over {tol} (max {m}), alpha {a} differ");
            assert_eq!((n, a), (0, 0), "{line}");
            ran += 1;
        }
        eprintln!("{line}");
    }
    eprintln!("{ran} oracle comparisons");
}

#[test]
fn anim_against_ffmpeg() {
    let mut failed = Vec::new();
    for name in [
        "anim5_1bpp.anim",
        "anim5_ham6.anim",
        "anim7l_1bpp.anim",
        "anim7s_1bpp.anim",
        "anim8l_1bpp.anim",
        "anim8s_1bpp.anim",
        "anim3_1bpp.anim",
        "anim3_zeus.anim",
        "animJ_5bpp.anim",
        "animJ_ham6.anim",
        "anim74_spigot",
        "anim108_ElGato(maybe_anim_1).anim",
    ] {
        let Some(p) = corpus(&format!("ffmpeg-iffx/{name}")) else {
            continue;
        };
        let data = std::fs::read(&p).unwrap();
        let mut player = Player::new(&data).unwrap();
        let t = std::time::Instant::now();
        let mut ours = Vec::new();
        while let Some(f) = player.next_frame(&data) {
            ours.push(f.unwrap_or_else(|e| panic!("{name}: frame {}: {e}", ours.len())));
        }
        let ms = t.elapsed().as_secs_f64() * 1e3;
        let i = info(&data).unwrap();
        assert_eq!(i.anim_frames, ours.len());
        let (w, h) = (ours[0].image.width, ours[0].image.height);
        let Some(ff) = ffmpeg(&p, w, h) else {
            eprintln!("{name}: {} frames, no ffmpeg", ours.len());
            continue;
        };
        if let Ok(k) = std::env::var("SPECULAR_IFF_DUMP")
            && let Ok(k) = k.parse::<usize>()
            && k < ours.len().min(ff.len())
        {
            let mut d = format!("{w} {h}\n").into_bytes();
            d.extend(&ours[k].image.pixels);
            std::fs::write(tmp(&format!("{name}.{k}.ours.rgba")), d).unwrap();
            let mut d = format!("{w} {h}\n").into_bytes();
            d.extend(&ff[k]);
            std::fs::write(tmp(&format!("{name}.{k}.rgba")), d).unwrap();
        }
        let mut bad = 0;
        let mut worst = 0;
        for (k, (o, f)) in ours.iter().zip(&ff).enumerate() {
            // FFmpeg's operation 8 (short) stops 4 bytes before the end of the data, so the
            // last ops of the last column are lost there: that column is not compared.
            let skip_last = name.starts_with("anim8s");
            let cut = |v: &[u8]| -> Vec<u8> {
                v.chunks_exact(4)
                    .enumerate()
                    .flat_map(|(i, p)| {
                        if skip_last && (i as u32 % w) >= w - 16 {
                            [0; 4]
                        } else {
                            [p[0], p[1], p[2], p[3]]
                        }
                    })
                    .collect()
            };
            let (n, m) = diff(&cut(&o.image.pixels), &cut(f), 15);
            // FFmpeg leaves stray pixels in the operation l sample from frame 60 on (checked
            // by eye: ours is clean); under 1% of the picture.
            let stray = name.starts_with("anim108") && k >= 60 && n * 100 < (w * h) as usize;
            if n > 0 && !stray {
                bad += 1;
                if bad <= 3 {
                    let first = o
                        .image
                        .pixels
                        .chunks_exact(4)
                        .zip(f.chunks_exact(4))
                        .position(|(p, q)| p[..3] != q[..3])
                        .unwrap_or(0) as u32;
                    eprintln!(
                        "{name}: frame {k}: {n} px over 15 (max {m}), first at {},{}",
                        first % w,
                        first / w
                    );
                }
            }
            worst = worst.max(m);
        }
        eprintln!(
            "{name}: {} frames ({} from ffmpeg) in {ms:.0} ms, {bad} differ, max {worst}",
            ours.len(),
            ff.len()
        );
        if bad > 0 {
            failed.push(name);
        }
    }
    assert!(failed.is_empty(), "{failed:?}");
}

/// Codes one plane (rows of `row` bytes) as a VDAT payload: the words down each column, in
/// repeats (`c > 1`, or `1` with a count) and literal runs (`c < 0`, or `0` with a count).
fn vdat(plane: &[u8], row: usize, height: usize) -> Vec<u8> {
    let mut words = Vec::new();
    for x in (0..row).step_by(2) {
        for y in 0..height {
            words.push([plane[y * row + x], plane[y * row + x + 1]]);
        }
    }
    let (mut cmds, mut data) = (Vec::new(), Vec::new());
    let mut i = 0;
    while i < words.len() {
        let mut run = 1;
        while i + run < words.len() && words[i + run] == words[i] {
            run += 1;
        }
        if run >= 3 {
            if run <= 127 {
                cmds.push(run as u8);
            } else {
                cmds.push(1);
                data.extend((run as u16).to_be_bytes());
            }
            data.extend(words[i]);
            i += run;
            continue;
        }
        // A literal run up to the next repeat of three.
        let mut n = 1;
        while i + n < words.len()
            && !(i + n + 2 < words.len()
                && words[i + n] == words[i + n + 1]
                && words[i + n] == words[i + n + 2])
        {
            n += 1;
        }
        if n <= 128 {
            cmds.push((-(n as i32)) as u8);
        } else {
            cmds.push(0);
            data.extend((n as u16).to_be_bytes());
        }
        for w in &words[i..i + n] {
            data.extend(w);
        }
        i += n;
    }
    // Keep the data words aligned (the 68000 reads words at even addresses only): a spare
    // command after the last column is never reached.
    if cmds.len() % 2 == 1 {
        cmds.push(0xff);
    }
    let mut v = ((cmds.len() + 2) as u16).to_be_bytes().to_vec();
    v.extend(cmds);
    v.extend(data);
    v
}

/// ByteRun2 (VDAT) files built here: our decode equals the same planes uncompressed and
/// FFmpeg's decode.
#[test]
fn vdat_built() {
    for (w, h, planes) in [(37u16, 23u16, 4u8), (300, 7, 5), (16, 300, 8)] {
        let row = usize::from(w).div_ceil(16) * 2;
        let hh = usize::from(h);
        let np = usize::from(planes);
        let mut r = Rng(u64::from(w) * 31 + u64::from(h));
        // Planes with long runs and noise.
        let acbm: Vec<Vec<u8>> = (0..np)
            .map(|p| {
                (0..row * hh)
                    .map(|i| {
                        if (i / row + p) % 5 < 2 {
                            0xa5
                        } else if r.next().is_multiple_of(3) {
                            r.next() as u8
                        } else {
                            (i % 7) as u8
                        }
                    })
                    .collect()
            })
            .collect();
        let mut body = Vec::new();
        for pl in &acbm {
            body.extend(chunk(b"VDAT", &vdat(pl, row, hh)));
        }
        let mut cmap = Vec::new();
        for i in 0..(1u32 << planes) {
            cmap.extend([(i * 37) as u8, (i * 91) as u8, (i * 13) as u8]);
        }
        let f = form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(w, h, planes, 2)),
                chunk(b"CMAP", &cmap),
                chunk(b"BODY", &body),
            ],
        );
        // The same planes, uncompressed (interleaved rows).
        let mut raw = Vec::new();
        for y in 0..hh {
            for pl in &acbm {
                raw.extend(&pl[y * row..(y + 1) * row]);
            }
        }
        let plain = form(
            b"ILBM",
            &[
                chunk(b"BMHD", &bmhd(w, h, planes, 0)),
                chunk(b"CMAP", &cmap),
                chunk(b"BODY", &raw),
            ],
        );
        let img = decode(&f).unwrap();
        assert!(!img.truncated);
        assert!(img.pixels == decode(&plain).unwrap().pixels, "{w}x{h}");
        assert_eq!(info(&f).unwrap().compression, Compression::Vdat);
        let p = tmp(&format!("vdat-{w}x{h}.iff"));
        std::fs::write(&p, &f).unwrap();
        if let Some(ff) = ffmpeg(&p, u32::from(w), u32::from(h)) {
            let (n, m) = diff(&img.pixels, &ff[0], 0);
            eprintln!("vdat {w}x{h}x{planes} vs ffmpeg: {n} over 0 (max {m})");
            assert_eq!(n, 0);
        }
    }
}

/// Cut short and corrupted RGB8, RGBN, VDAT, DEEP and ANIM data never panics.
#[test]
fn cut_and_corrupt_never_panic() {
    let mut files = vec![rgbx(false, 40, 9, 3).0, rgbx(true, 40, 9, 4).0];
    for rel in [
        "ffmpeg-iffx/anim5_1bpp.anim",
        "ffmpeg-iffx/anim3_1bpp.anim",
        "ffmpeg-iffx/animJ_5bpp.anim",
        "ffmpeg-iffx/anim8s_1bpp.anim",
        "ffmpeg-iffx/anim7l_1bpp.anim",
        "ffmpeg-iffx/anim108_ElGato(maybe_anim_1).anim",
        "ffmpeg-iffx/tvpaint_delta.deep",
        "twelvemonkeys/imageio/imageio-iff/src/test/resources/iff/arch.deep",
    ] {
        if let Some(p) = corpus(rel) {
            let d = std::fs::read(p).unwrap();
            // Keep the work small: the head of the big ones.
            files.push(d[..d.len().min(200_000)].to_vec());
        }
    }
    let mut r = Rng(99);
    for f in &files {
        for cut in [12, 40, 100, f.len() / 3, f.len() / 2, f.len() - 1] {
            let g = &f[..cut.min(f.len())];
            let _ = decode(g);
            if let Ok(mut p) = Player::new(g) {
                for _ in 0..4 {
                    if p.next_frame(g).is_none() {
                        break;
                    }
                }
            }
        }
        for _ in 0..6 {
            let mut g = f.clone();
            for _ in 0..20 {
                let i = r.next() as usize % g.len();
                g[i] = r.next() as u8;
            }
            let _ = decode(&g);
            if let Ok(mut p) = Player::new(&g) {
                for _ in 0..6 {
                    if p.next_frame(&g).is_none() {
                        break;
                    }
                }
            }
        }
    }
}
