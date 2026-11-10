//! specular-corpus: generates the JPEG test corpus.
//!
//! Usage: specular-corpus [--small | --full] [--flip-set] [--out DIR] [--only SUBSTR] [--force]
//!                       [--no-network] [--dry-run]

mod content;
mod encode;
mod jpeginfo;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use std::time::Instant;

use anyhow::{Context, Result, bail};
use rayon::prelude::*;
use serde::Serialize;

use content::{Scene, orientation_chart, to_stored};
use encode::{
    CjpegOpts, Meta, cjpeg_buffer, cjpeg_stream, encode_12bit, exiftool, magick_cmyk,
    make_thumbnail, pillow_cmyk,
};

const ICC_DIR: &str = "/usr/share/color/icc";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Tier {
    Small,
    Full,
    Flip,
}

#[derive(Clone, Debug)]
enum Kind {
    /// Procedural photo through cjpeg.
    Photo { opts: CjpegOpts },
    /// Procedural photo converted to CMYK: ImageMagick writes YCCK, Pillow writes plain CMYK.
    Cmyk { ycck: bool },
    /// Built-in 12-bit encoder.
    TwelveBit,
    /// Orientation chart for EXIF orientation n.
    Orientation(u8),
    /// Derived from another (already generated) file by a byte-level mutation.
    Broken {
        base: &'static str,
        mutation: Mutation,
    },
    /// Read-only file supplied by the project owner in tools/corpus/real (never written or copied).
    External,
    /// Downloaded public-domain photo.
    Real {
        url: &'static str,
        licence: &'static str,
        credit: &'static str,
        page: &'static str,
    },
}

#[derive(Clone, Copy, Debug)]
enum Mutation {
    Truncate(u32),
    BadSoi,
    StripDht,
    MarkerInScan,
    EarlyEoi,
    WrongRst,
    Dims(u16, u16),
    Garbage,
    Empty,
}

#[derive(Clone, Debug)]
struct Spec {
    name: String,
    tier: Tier,
    w: u32,
    h: u32,
    seed: u32,
    kind: Kind,
    meta: Meta,
    icc_name: Option<&'static str>,
    gps_name: Option<&'static str>,
    expected_error: bool,
    expected_note: Option<&'static str>,
    purpose: String,
}

fn photo(name: &str, tier: Tier, w: u32, h: u32, opts: CjpegOpts, purpose: &str) -> Spec {
    Spec {
        name: name.to_string(),
        tier,
        w,
        h,
        seed: fxhash(name),
        kind: Kind::Photo { opts },
        meta: Meta::default(),
        icc_name: None,
        gps_name: None,
        expected_error: false,
        expected_note: None,
        purpose: purpose.to_string(),
    }
}

fn fxhash(s: &str) -> u32 {
    s.bytes().fold(0x811c_9dc5u32, |h, b| {
        (h ^ b as u32).wrapping_mul(0x0100_0193)
    })
}

fn q92() -> CjpegOpts {
    CjpegOpts {
        quality: 92,
        sample: Some("2x2"),
        ..Default::default()
    }
}

fn camera(model: &str) -> Meta {
    Meta {
        camera: Some(("SpeculaCorpus".into(), model.into())),
        strip_jfif: true,
        ..Default::default()
    }
}

fn broken(name: &str, base: &'static str, mutation: Mutation, purpose: &str) -> Spec {
    let mut s = photo(name, Tier::Small, 0, 0, CjpegOpts::default(), purpose);
    s.kind = Kind::Broken { base, mutation };
    s.expected_error = true;
    s
}

fn plan(flip: bool) -> Vec<Spec> {
    let mut v = Vec::new();
    // --- camera-like files (dimensions of real bodies) ---
    let mut s = photo(
        "cam/24mp-420-q92.jpg",
        Tier::Small,
        6000,
        4000,
        q92(),
        "24 MP baseline 4:2:0 q92 (Sony/Nikon 24 MP); §2.3 row 1",
    );
    s.meta = camera("Synthetic 24MP");
    v.push(s);
    let mut s = photo(
        "cam/24mp-444-q92.jpg",
        Tier::Full,
        6000,
        4000,
        CjpegOpts {
            sample: Some("1x1"),
            ..q92()
        },
        "24 MP baseline 4:4:4 q92 (no chroma subsampling)",
    );
    s.meta = camera("Synthetic 24MP 444");
    v.push(s);
    let mut s = photo(
        "cam/24mp-422-q92.jpg",
        Tier::Full,
        6000,
        4000,
        CjpegOpts {
            sample: Some("2x1"),
            ..q92()
        },
        "24 MP baseline 4:2:2 q92 (Canon-style subsampling)",
    );
    s.meta = camera("Synthetic 24MP 422");
    v.push(s);
    let mut s = photo(
        "cam/45mp-420-q92.jpg",
        Tier::Full,
        8256,
        5504,
        q92(),
        "45 MP baseline 4:2:0 (Nikon Z7/D850 size)",
    );
    s.meta = camera("Synthetic 45MP");
    v.push(s);
    let mut s = photo(
        "cam/50mp-progressive-q92.jpg",
        Tier::Full,
        8688,
        5792,
        CjpegOpts {
            progressive: true,
            ..q92()
        },
        "50 MP progressive 4:2:0 (Canon 5DS size); §2.3 row 2",
    );
    s.meta = camera("Synthetic 50MP");
    v.push(s);
    let mut s = photo(
        "cam/61mp-420-q92.jpg",
        Tier::Full,
        9504,
        6336,
        q92(),
        "61 MP baseline 4:2:0 (Sony A7R IV size)",
    );
    s.meta = camera("Synthetic 61MP");
    v.push(s);
    let mut s = photo(
        "cam/100mp-norst-q92.jpg",
        Tier::Full,
        11648,
        8736,
        q92(),
        "100 MP baseline, NO restart markers (Fujifilm GFX100 size); §2.3 row 3 — forces sequential Huffman decode unless indexed",
    );
    s.meta = camera("Synthetic 100MP");
    v.push(s);
    let mut s = photo(
        "cam/100mp-rst1-q92.jpg",
        Tier::Full,
        11648,
        8736,
        CjpegOpts {
            restart_rows: Some(1),
            ..q92()
        },
        "100 MP baseline with a restart marker every MCU row (parallel-decodable)",
    );
    s.meta = camera("Synthetic 100MP RST");
    v.push(s);
    let mut s = photo(
        "cam/200mp-16384x12288-q92.jpg",
        Tier::Full,
        16384,
        12288,
        q92(),
        "200 MP pixel-shift composite size (16384 x 12288); §2.3 row 4; peak-memory target < 1 GB",
    );
    s.meta = camera("Synthetic 200MP pixel-shift");
    v.push(s);
    let mut s = photo(
        "pano/65000x10000-q92.jpg",
        Tier::Full,
        65000,
        10000,
        q92(),
        "650 MP 65000 x 10000 panorama near the JPEG width limit; §2.3 row 5",
    );
    s.meta = camera("Synthetic panorama");
    v.push(s);
    // Small-tier stand-ins so --quick benchmarks cover progressive and restart paths.
    v.push(photo(
        "cam/12mp-progressive-q92.jpg",
        Tier::Small,
        4000,
        3000,
        CjpegOpts {
            progressive: true,
            ..q92()
        },
        "12 MP progressive (quick/CI stand-in for the 50 MP progressive file)",
    ));
    v.push(photo(
        "cam/12mp-rst1-q92.jpg",
        Tier::Small,
        4000,
        3000,
        CjpegOpts {
            restart_rows: Some(1),
            ..q92()
        },
        "12 MP with restart marker every MCU row (CI stand-in; base for wrong-RST mutation)",
    ));
    // --- colour models / coding variants ---
    v.push(photo(
        "variant/greyscale-24mp.jpg",
        Tier::Small,
        6000,
        4000,
        CjpegOpts {
            grayscale: true,
            quality: 92,
            ..Default::default()
        },
        "Greyscale (1 component) 24 MP",
    ));
    v.push(photo(
        "variant/rgb-nontransformed-6mp.jpg",
        Tier::Small,
        3000,
        2000,
        CjpegOpts {
            rgb: true,
            quality: 92,
            ..Default::default()
        },
        "RGB JPEG without YCbCr transform (Adobe transform=0)",
    ));
    let mut s = photo(
        "variant/ycck-6mp.jpg",
        Tier::Small,
        3000,
        2000,
        q92(),
        "YCCK JPEG (Adobe APP14 transform=2, inverted CMYK), via ImageMagick",
    );
    s.kind = Kind::Cmyk { ycck: true };
    v.push(s);
    let mut s = photo(
        "variant/cmyk-6mp.jpg",
        Tier::Small,
        3000,
        2000,
        q92(),
        "Plain CMYK JPEG (Adobe APP14 transform=0, inverted), via Python Pillow",
    );
    s.kind = Kind::Cmyk { ycck: false };
    v.push(s);
    let mut s = photo(
        "variant/12bit-ycbcr444-2mp.jpg",
        Tier::Small,
        1600,
        1200,
        q92(),
        "12-bit extended sequential (SOF1) 4:4:4",
    );
    s.kind = Kind::TwelveBit;
    s.expected_error = true;
    s.expected_note = Some(
        "decode or graceful error (libjpeg-turbo 2.1.5 and most Rust decoders do not support 12-bit)",
    );
    v.push(s);
    let mut s = photo(
        "variant/arithmetic-12mp.jpg",
        Tier::Small,
        4000,
        3000,
        CjpegOpts {
            arithmetic: true,
            ..q92()
        },
        "Arithmetic-coded baseline (SOF9)",
    );
    s.expected_error = true;
    s.expected_note =
        Some("decode or graceful error (djpeg decodes it; zune-jpeg/jpeg-decoder refuse)");
    v.push(s);
    let mut s = photo(
        "variant/arithmetic-progressive-6mp.jpg",
        Tier::Small,
        3000,
        2000,
        CjpegOpts {
            arithmetic: true,
            progressive: true,
            ..q92()
        },
        "Arithmetic-coded progressive (SOF10)",
    );
    s.expected_error = true;
    s.expected_note = Some("decode or graceful error");
    v.push(s);
    v.push(photo(
        "variant/24mp-q75.jpg",
        Tier::Full,
        6000,
        4000,
        q92().clone_with_quality(75),
        "24 MP q75 (typical web/phone re-save; less entropy data)",
    ));
    v.push(photo(
        "variant/24mp-q100-444.jpg",
        Tier::Full,
        6000,
        4000,
        CjpegOpts {
            quality: 100,
            sample: Some("1x1"),
            ..Default::default()
        },
        "24 MP q100 4:4:4 (worst-case entropy decode load)",
    ));
    // --- EXIF orientation ---
    for o in 1..=8u8 {
        let mut s = photo(
            &format!("orientation/orientation-{o}.jpg"),
            Tier::Small,
            800,
            600,
            q92(),
            "",
        );
        s.kind = Kind::Orientation(o);
        s.meta = Meta {
            orientation: Some(o),
            camera: Some(("SpeculaCorpus".into(), "Orientation chart".into())),
            ..Default::default()
        };
        s.purpose = format!(
            "EXIF orientation {o}: correctly oriented display shows an upright F, red square top-left"
        );
        v.push(s);
    }
    // --- ICC ---
    for (file, label, icc) in [
        (
            "icc/srgb-v2.jpg",
            "sRGB (v2, Compact-ICC-Profiles sRGB-v2-magic, CC0)",
            "sRGB-v2-magic.icc",
        ),
        (
            "icc/srgb-v4.jpg",
            "sRGB (v4, Compact-ICC-Profiles sRGB-v4, CC0)",
            "sRGB-v4.icc",
        ),
        (
            "icc/display-p3.jpg",
            "Display P3 (v4, Compact-ICC-Profiles DisplayP3-v4, CC0)",
            "DisplayP3-v4.icc",
        ),
        (
            "icc/adobe-rgb.jpg",
            "Adobe RGB (1998)-compatible (Compact-ICC-Profiles AdobeCompat-v4, CC0)",
            "AdobeCompat-v4.icc",
        ),
        (
            "icc/prophoto.jpg",
            "ProPhoto RGB / ROMM (Compact-ICC-Profiles ProPhoto-v4, CC0)",
            "ProPhoto-v4.icc",
        ),
    ] {
        let mut s = photo(
            file,
            Tier::Small,
            2000,
            1500,
            q92(),
            &format!("Embedded ICC profile: {label}"),
        );
        s.seed = 777; // same pixels in every ICC file so the colour difference is the profile alone
        s.meta = Meta {
            icc: Some(PathBuf::from(ICC_DIR).join(icc)),
            ..camera("ICC test")
        };
        s.icc_name = Some(label_static(label));
        v.push(s);
    }
    // --- GPS ---
    for (file, place, lat, lon) in [
        ("gps/lisbon.jpg", "Lisbon, Portugal", 38.7223, -9.1393),
        ("gps/tokyo.jpg", "Tokyo, Japan", 35.6762, 139.6503),
        (
            "gps/buenos-aires.jpg",
            "Buenos Aires, Argentina",
            -34.6037,
            -58.3816,
        ),
        (
            "gps/mid-ocean-point-nemo.jpg",
            "South Pacific, Point Nemo (no land nearby)",
            -48.8767,
            -123.3933,
        ),
    ] {
        let mut s = photo(
            file,
            Tier::Small,
            1600,
            1200,
            q92(),
            &format!("GPS: {place}"),
        );
        s.meta = Meta {
            gps: Some((lat, lon)),
            ..camera("GPS test")
        };
        s.gps_name = Some(label_static(place));
        v.push(s);
    }
    let mut s = photo(
        "gps/no-gps.jpg",
        Tier::Small,
        1600,
        1200,
        q92(),
        "EXIF present but no GPS IFD",
    );
    s.meta = camera("GPS test");
    v.push(s);
    // --- broken ---
    let base = "cam/12mp-progressive-q92.jpg";
    let base_bl = "cam/24mp-420-q92.jpg";
    for pct in [10, 50, 90] {
        v.push(broken(
            &format!("broken/truncated-{pct}pct-baseline.jpg"),
            base_bl,
            Mutation::Truncate(pct),
            &format!("24 MP baseline truncated at {pct}% (show partial image or graceful error, no crash)"),
        ));
        v.push(broken(
            &format!("broken/truncated-{pct}pct-progressive.jpg"),
            base,
            Mutation::Truncate(pct),
            &format!("12 MP progressive truncated at {pct}%"),
        ));
    }
    v.push(broken(
        "broken/bad-soi.jpg",
        base_bl,
        Mutation::BadSoi,
        "First marker is not SOI",
    ));
    v.push(broken(
        "broken/missing-dht.jpg",
        base_bl,
        Mutation::StripDht,
        "All DHT segments removed",
    ));
    v.push(broken(
        "broken/bogus-marker-in-scan.jpg",
        base_bl,
        Mutation::MarkerInScan,
        "Unexpected 0xFFE5 marker in the middle of entropy data",
    ));
    v.push(broken(
        "broken/early-eoi.jpg",
        base_bl,
        Mutation::EarlyEoi,
        "EOI at 30% of the scan; rest of file is garbage",
    ));
    v.push(broken(
        "broken/wrong-rst-sequence.jpg",
        "cam/12mp-rst1-q92.jpg",
        Mutation::WrongRst,
        "RST markers out of sequence (RSTn numbers shuffled)",
    ));
    v.push(broken(
        "broken/absurd-65535x65535.jpg",
        "orientation/orientation-1.jpg",
        Mutation::Dims(65535, 65535),
        "SOF claims 65535 x 65535 with ~100 KB of data: must be refused / bounded, never allocate 12 GB up front",
    ));
    v.push(broken(
        "broken/zero-height.jpg",
        "orientation/orientation-1.jpg",
        Mutation::Dims(800, 0),
        "SOF height 0 (DNL) without a DNL marker",
    ));
    v.push(broken(
        "broken/random-garbage.jpg",
        base_bl,
        Mutation::Garbage,
        "64 KB of random bytes with .jpg extension",
    ));
    v.push(broken(
        "broken/empty.jpg",
        base_bl,
        Mutation::Empty,
        "Zero-byte file",
    ));
    // --- real public-domain photos ---
    for (file, url, credit, page) in [
        (
            "real/aldrin-apollo11-15mp.jpg",
            "https://upload.wikimedia.org/wikipedia/commons/9/98/Aldrin_Apollo_11_original.jpg",
            "NASA / Neil A. Armstrong (AS11-40-5903)",
            "https://commons.wikimedia.org/wiki/File:Aldrin_Apollo_11_original.jpg",
        ),
        (
            "real/sts134-iss-12mp.jpg",
            "https://upload.wikimedia.org/wikipedia/commons/c/c9/STS-134_International_Space_Station_after_undocking.jpg",
            "NASA (STS-134 crew)",
            "https://commons.wikimedia.org/wiki/File:STS-134_International_Space_Station_after_undocking.jpg",
        ),
        (
            "real/pillars-of-creation-48mp.jpg",
            "https://upload.wikimedia.org/wikipedia/commons/6/68/Pillars_of_creation_2014_HST_WFC3-UVIS_full-res_denoised.jpg",
            "NASA, ESA/Hubble and the Hubble Heritage Team",
            "https://commons.wikimedia.org/wiki/File:Pillars_of_creation_2014_HST_WFC3-UVIS_full-res_denoised.jpg",
        ),
    ] {
        let mut s = photo(
            file,
            Tier::Full,
            0,
            0,
            q92(),
            "Real public-domain photograph (NASA via Wikimedia Commons)",
        );
        s.kind = Kind::Real {
            url,
            licence: "Public domain (NASA work; Commons LicenseShortName = Public domain)",
            credit,
            page,
        };
        v.push(s);
    }
    // --- flip set ---
    if flip {
        for i in 0..1000 {
            let mut s = photo(
                &format!("flip/flip-{i:04}.jpg"),
                Tier::Flip,
                4000,
                3000,
                q92(),
                "Flip-test folder: 1,000 x 12 MP (12 MP instead of 24 MP to halve disk use, ~6 GB)",
            );
            s.meta = camera("Synthetic 12MP flip");
            v.push(s);
        }
    }
    v
}

/// Owner-supplied real JPEGs in `<corpus>/real` (sibling of the default output directory).
/// They are referenced in place: the generator only reads them to describe them in the manifest.
fn external_specs(out: &Path) -> Vec<Spec> {
    let real = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/real");
    let Ok(real) = real.canonicalize() else {
        return Vec::new();
    };
    let sibling = out.parent().map(|p| p.join("real")) == Some(real.clone());
    let mut v = Vec::new();
    let Ok(rd) = std::fs::read_dir(&real) else {
        return v;
    };
    let mut names: Vec<String> = rd
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| {
            n.to_ascii_lowercase().ends_with(".jpg") || n.to_ascii_lowercase().ends_with(".jpeg")
        })
        .collect();
    names.sort();
    for n in names {
        let name = if sibling {
            format!("../real/{n}")
        } else {
            real.join(&n).display().to_string()
        };
        let mut s = photo(
            &name,
            Tier::Small,
            0,
            0,
            CjpegOpts::default(),
            "Real heavy JPEG supplied by the project owner (see tools/corpus/real/README.md)",
        );
        s.kind = Kind::External;
        v.push(s);
    }
    v
}

fn label_static(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

trait WithQuality {
    fn clone_with_quality(&self, q: u8) -> CjpegOpts;
}
impl WithQuality for CjpegOpts {
    fn clone_with_quality(&self, q: u8) -> CjpegOpts {
        CjpegOpts {
            quality: q,
            ..self.clone()
        }
    }
}

/// Rough output size estimate (bytes) for the disk check.
fn estimate(s: &Spec) -> u64 {
    let px = s.w as u64 * s.h as u64;
    match &s.kind {
        Kind::Photo { opts } => {
            let bpp = if opts.grayscale {
                2.5
            } else if opts.sample == Some("1x1") || opts.rgb || opts.sample.is_none() {
                7.0
            } else {
                4.5
            };
            (px as f64 * bpp / 8.0) as u64
        }
        Kind::Cmyk { .. } => px * 2,
        Kind::TwelveBit => px * 2,
        Kind::Orientation(_) => 200_000,
        Kind::Broken { .. } => 10_000_000,
        Kind::Real { .. } => 25_000_000,
        Kind::External => 0,
    }
}

#[derive(Serialize)]
struct Entry {
    path: String,
    category: String,
    file_size: u64,
    width: Option<u32>,
    height: Option<u32>,
    megapixels: Option<f64>,
    bits_per_pixel: Option<f64>,
    sof: Option<String>,
    precision: Option<u8>,
    sampling: Option<String>,
    progressive: Option<bool>,
    arithmetic: Option<bool>,
    restart_interval_mcus: Option<u16>,
    colour_model: Option<String>,
    exif_orientation: Option<u8>,
    has_gps: bool,
    gps: Option<Gps>,
    has_exif_thumbnail: bool,
    icc_profile: Option<String>,
    expected_error: bool,
    expected_note: Option<String>,
    purpose: String,
    tier: String,
    source_url: Option<String>,
    source_page: Option<String>,
    licence: Option<String>,
    credit: Option<String>,
}

#[derive(Serialize)]
struct Gps {
    place: Option<String>,
    lat: f64,
    lon: f64,
}

#[derive(Serialize)]
struct Manifest {
    generator: String,
    mode: String,
    encoder: String,
    icc_source: String,
    notes: Vec<String>,
    files: Vec<Entry>,
}

struct Args {
    full: bool,
    flip: bool,
    out: PathBuf,
    only: Option<String>,
    force: bool,
    network: bool,
    dry_run: bool,
}

fn parse_args() -> Result<Args> {
    let mut a = Args {
        full: false,
        flip: false,
        out: PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus/out"),
        only: None,
        force: false,
        network: true,
        dry_run: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(x) = it.next() {
        match x.as_str() {
            "--small" => a.full = false,
            "--full" => a.full = true,
            "--flip-set" => a.flip = true,
            "--out" => a.out = PathBuf::from(it.next().context("--out needs a value")?),
            "--only" => a.only = Some(it.next().context("--only needs a value")?),
            "--force" => a.force = true,
            "--no-network" => a.network = false,
            "--dry-run" => a.dry_run = true,
            "-h" | "--help" => {
                println!(
                    "specular-corpus [--small|--full] [--flip-set] [--out DIR] [--only SUBSTR] [--force] [--no-network] [--dry-run]\n\
                     --small      quick CI subset (default)\n\
                     --full       all heavy files (24..200 MP, 650 MP panorama, real photos)\n\
                     --flip-set   also 1,000 x 12 MP files in flip/ (~6 GB)\n\
                     --only S     only files whose path contains S (plus the bases they derive from)\n\
                     --force      regenerate files that already exist\n\
                     --no-network skip downloading real photos"
                );
                std::process::exit(0);
            }
            other => bail!("unknown argument {other}"),
        }
    }
    Ok(a)
}

#[cfg(unix)]
fn free_bytes(p: &Path) -> Result<u64> {
    let st = rustix::fs::statvfs(p).with_context(|| format!("statvfs {}", p.display()))?;
    Ok(st.f_bavail * st.f_frsize)
}

/// Free space is not checked off Unix.
#[cfg(not(unix))]
fn free_bytes(_: &Path) -> Result<u64> {
    Ok(u64::MAX)
}

fn gib(b: u64) -> String {
    format!("{:.2} GiB", b as f64 / (1u64 << 30) as f64)
}

fn main() -> Result<()> {
    let args = parse_args()?;
    for tool in ["cjpeg", "magick", "exiftool"] {
        if Command::new(tool).arg("-version").output().is_err() {
            bail!("required tool `{tool}` not found in PATH");
        }
    }
    let max_tier = if args.full { Tier::Full } else { Tier::Small };
    let mut specs: Vec<Spec> = plan(args.flip)
        .into_iter()
        .filter(|s| s.tier <= max_tier || (args.flip && s.tier == Tier::Flip))
        .collect();
    if !args.network {
        specs.retain(|s| !matches!(s.kind, Kind::Real { .. }));
    }
    if let Some(only) = &args.only {
        let wanted: Vec<String> = specs
            .iter()
            .filter(|s| s.name.contains(only.as_str()))
            .map(|s| s.name.clone())
            .collect();
        let bases: Vec<&str> = specs
            .iter()
            .filter(|s| wanted.contains(&s.name))
            .filter_map(|s| {
                if let Kind::Broken { base, .. } = s.kind {
                    Some(base)
                } else {
                    None
                }
            })
            .collect();
        specs.retain(|s| wanted.contains(&s.name) || bases.contains(&s.name.as_str()));
    }
    std::fs::create_dir_all(&args.out)?;
    let out = args.out.canonicalize()?;
    let todo: Vec<&Spec> = specs
        .iter()
        .filter(|s| {
            !matches!(s.kind, Kind::External) && (args.force || !out.join(&s.name).exists())
        })
        .collect();
    let need: u64 = todo.iter().map(|s| estimate(s)).sum();
    let free = free_bytes(&out)?;
    println!(
        "specular-corpus: mode={} files={} to-generate={} expected size ~{} (free on {}: {})",
        if args.full { "full" } else { "small" },
        specs.len(),
        todo.len(),
        gib(need),
        out.display(),
        gib(free)
    );
    let reserve = 2u64 << 30;
    if need + reserve > free {
        bail!(
            "not enough free disk space: need ~{} + {} reserve, have {}",
            gib(need),
            gib(reserve),
            gib(free)
        );
    }
    if args.dry_run {
        for s in &todo {
            println!(
                "  {:<48} {:>6}x{:<6} ~{:>8.1} MB",
                s.name,
                s.w,
                s.h,
                estimate(s) as f64 / 1e6
            );
        }
        return Ok(());
    }
    let scratch = out.join(".scratch");
    std::fs::create_dir_all(&scratch)?;
    let t0 = Instant::now();
    let failures = Mutex::new(Vec::new());
    // Phase 1: everything that isn't derived. Largest first so the long cjpeg runs start early.
    let mut phase1: Vec<&Spec> = todo
        .iter()
        .copied()
        .filter(|s| !matches!(s.kind, Kind::Broken { .. }))
        .collect();
    phase1.sort_by_key(|s| std::cmp::Reverse(s.w as u64 * s.h as u64));
    phase1.par_iter().with_max_len(1).for_each(|s| {
        let t = Instant::now();
        match generate(s, &out, &scratch) {
            Ok(()) => println!("  ok   {:<48} {:>7.1}s", s.name, t.elapsed().as_secs_f64()),
            Err(e) => {
                eprintln!("  FAIL {}: {e:#}", s.name);
                failures.lock().unwrap().push(s.name.clone());
            }
        }
    });
    // Phase 2: broken files derived from phase-1 outputs.
    for s in todo
        .iter()
        .filter(|s| matches!(s.kind, Kind::Broken { .. }))
    {
        match generate(s, &out, &scratch) {
            Ok(()) => println!("  ok   {}", s.name),
            Err(e) => {
                eprintln!("  FAIL {}: {e:#}", s.name);
                failures.lock().unwrap().push(s.name.clone());
            }
        }
    }
    std::fs::remove_dir_all(&scratch).ok();
    let mut all = plan(true);
    all.extend(external_specs(&out));
    write_manifest(&all, &out, args.full)?;
    println!(
        "generated in {:.1}s; manifest: {}",
        t0.elapsed().as_secs_f64(),
        out.join("manifest.json").display()
    );
    let f = failures.into_inner().unwrap();
    if !f.is_empty() {
        bail!("{} file(s) failed: {:?}", f.len(), f);
    }
    Ok(())
}

fn generate(s: &Spec, out: &Path, scratch: &Path) -> Result<()> {
    let path = out.join(&s.name);
    std::fs::create_dir_all(path.parent().unwrap())?;
    match &s.kind {
        Kind::Photo { opts } => {
            let scene = Scene::new(s.w, s.h, s.seed);
            cjpeg_stream(&path, s.w, s.h, opts, |y, row| scene.render_row(y, row))?;
            let mut meta = s.meta.clone();
            if meta.camera.is_some() && s.w * s.h >= 4_000_000 {
                let t = scratch.join(format!("{}.thumb.jpg", s.seed));
                make_thumbnail(&scene, &t)?;
                meta.thumbnail = Some(t);
            }
            exiftool(&path, &meta)?;
            if let Some(t) = meta.thumbnail {
                std::fs::remove_file(t).ok();
            }
        }
        Kind::Cmyk { ycck } => {
            let scene = Scene::new(s.w, s.h, s.seed);
            let mut rgb = vec![0u8; (s.w * s.h * 3) as usize];
            rgb.par_chunks_mut(s.w as usize * 3)
                .enumerate()
                .for_each(|(y, r)| scene.render_row(y as u32, r));
            if *ycck {
                magick_cmyk(&path, s.w, s.h, &rgb, scratch)?;
            } else {
                pillow_cmyk(&path, s.w, s.h, &rgb, scratch)?;
            }
        }
        Kind::TwelveBit => encode_12bit(&path, &Scene::new(s.w, s.h, s.seed))?,
        Kind::Orientation(o) => {
            let stored = to_stored(&orientation_chart(*o), *o);
            cjpeg_buffer(
                &path,
                stored.w,
                stored.h,
                &stored.px,
                &CjpegOpts {
                    quality: 92,
                    ..Default::default()
                },
            )?;
            exiftool(&path, &s.meta)?;
        }
        Kind::Broken { base, mutation } => {
            let src =
                std::fs::read(out.join(base)).with_context(|| format!("reading base {base}"))?;
            let bytes = mutate(&src, *mutation, s.seed)?;
            std::fs::write(&path, bytes)?;
        }
        Kind::External => {}
        Kind::Real { url, .. } => {
            let tmp = scratch.join(format!("{}.download", s.seed));
            encode::run(
                Command::new("curl")
                    .args([
                        "-fsSL",
                        "--max-time",
                        "600",
                        "-A",
                        "specular-corpus/0.1 (test corpus generator)",
                        "-o",
                    ])
                    .arg(&tmp)
                    .arg(url),
            )?;
            let head = std::fs::read(&tmp).map(|b| b.get(..2) == Some(&[0xFF, 0xD8][..]))?;
            if !head {
                bail!("download is not a JPEG");
            }
            std::fs::rename(&tmp, &path)?;
        }
    }
    Ok(())
}

fn mutate(src: &[u8], m: Mutation, seed: u32) -> Result<Vec<u8>> {
    let segs = jpeginfo::segments(src).context("base is not a parsable JPEG")?;
    let sos = *segs.last().unwrap();
    let scan_start = sos.offset + sos.len;
    let mut d = src.to_vec();
    match m {
        Mutation::Truncate(p) => d.truncate(src.len() * p as usize / 100),
        Mutation::BadSoi => d[1] = 0x00,
        Mutation::StripDht => {
            for s in segs.iter().rev().filter(|s| s.marker == 0xC4) {
                d.drain(s.offset..s.offset + s.len);
            }
        }
        Mutation::MarkerInScan => {
            let mut at = scan_start + (src.len() - scan_start) / 2;
            while d[at] == 0xFF || d[at - 1] == 0xFF {
                at += 1;
            }
            d.splice(at..at, [0xFF, 0xE5, 0x00, 0x04, 0x12, 0x34]);
        }
        Mutation::EarlyEoi => {
            let mut at = scan_start + (src.len() - scan_start) * 3 / 10;
            while d[at - 1] == 0xFF {
                at += 1;
            }
            d.splice(at..at, [0xFF, 0xD9]);
        }
        Mutation::WrongRst => {
            let mut n = 0;
            let mut i = scan_start;
            while i + 1 < d.len() {
                if d[i] == 0xFF && (0xD0..=0xD7).contains(&d[i + 1]) {
                    n += 1;
                    if n % 7 == 3 {
                        d[i + 1] = 0xD0 + ((d[i + 1] - 0xD0 + 3) & 7);
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if n == 0 {
                bail!("base has no restart markers");
            }
        }
        Mutation::Dims(w, h) => {
            let sof = segs
                .iter()
                .find(|s| matches!(s.marker, 0xC0..=0xC2))
                .context("no SOF")?;
            d[sof.offset + 5..sof.offset + 7].copy_from_slice(&h.to_be_bytes());
            d[sof.offset + 7..sof.offset + 9].copy_from_slice(&w.to_be_bytes());
        }
        Mutation::Garbage => {
            let mut x = seed as u64 | 1;
            d = (0..65536)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 7;
                    x ^= x << 17;
                    (x >> 24) as u8
                })
                .collect();
        }
        Mutation::Empty => d.clear(),
    }
    Ok(d)
}

fn write_manifest(specs: &[Spec], out: &Path, full: bool) -> Result<()> {
    // Read back EXIF facts with one exiftool call so the manifest reflects the files on disk.
    let present: Vec<&Spec> = specs
        .iter()
        .filter(|s| out.join(&s.name).exists())
        .collect();
    let mut exif: std::collections::HashMap<String, serde_json::Value> = Default::default();
    let list = out.join(".exiftool-args");
    std::fs::write(
        &list,
        present
            .iter()
            .map(|s| format!("{}\n", out.join(&s.name).display()))
            .collect::<String>(),
    )?;
    let res = Command::new("exiftool")
        .args([
            "-j",
            "-n",
            "-q",
            "-q",
            "-Orientation",
            "-GPSLatitude",
            "-GPSLongitude",
            "-ICC_Profile:ProfileDescription",
            "-ThumbnailLength",
            "-@",
        ])
        .arg(&list)
        .output()?;
    std::fs::remove_file(&list).ok();
    if let Ok(serde_json::Value::Array(a)) =
        serde_json::from_slice::<serde_json::Value>(&res.stdout)
    {
        for v in a {
            if let Some(src) = v.get("SourceFile").and_then(|s| s.as_str()) {
                exif.insert(src.to_string(), v.clone());
            }
        }
    }
    let mut files = Vec::new();
    for s in present {
        let p = out.join(&s.name);
        let data = std::fs::read(&p)?;
        let hi = jpeginfo::parse(&data);
        let ex = exif.get(&p.display().to_string());
        let num = |k: &str| ex.and_then(|e| e.get(k)).and_then(|v| v.as_f64());
        let (lat, lon) = (num("GPSLatitude"), num("GPSLongitude"));
        let (w, h) = match (&hi, &s.kind) {
            (_, Kind::Broken { .. }) => {
                (hi.as_ref().map(|h| h.width), hi.as_ref().map(|h| h.height))
            }
            (Some(h), _) => (Some(h.width), Some(h.height)),
            (None, _) => (None, None),
        };
        let mp = match (w, h) {
            (Some(w), Some(h)) => Some(((w as f64 * h as f64) / 1e6 * 100.0).round() / 100.0),
            _ => None,
        };
        let bpp = match (w, h) {
            (Some(w), Some(h)) if w * h > 0 && !s.expected_error => {
                Some(((data.len() as f64 * 8.0 / (w as f64 * h as f64)) * 100.0).round() / 100.0)
            }
            _ => None,
        };
        let (url, page, licence, credit) = match &s.kind {
            Kind::Real {
                url,
                licence,
                credit,
                page,
            } => (
                Some(url.to_string()),
                Some(page.to_string()),
                Some(licence.to_string()),
                Some(credit.to_string()),
            ),
            Kind::External => (
                None,
                None,
                Some(
                    "Owner-supplied; not redistributable without the owner's permission"
                        .to_string(),
                ),
                None,
            ),
            _ => (None, None, None, None),
        };
        let icc_desc = ex
            .and_then(|e| e.get("ProfileDescription"))
            .and_then(|v| v.as_str())
            .map(String::from);
        files.push(Entry {
            path: s.name.clone(),
            category: if matches!(s.kind, Kind::External) {
                "real".into()
            } else {
                s.name.split('/').next().unwrap_or("").to_string()
            },
            file_size: data.len() as u64,
            width: w,
            height: h,
            megapixels: mp,
            bits_per_pixel: bpp,
            sof: hi.as_ref().map(|h| h.sof.clone()),
            precision: hi.as_ref().map(|h| h.precision),
            sampling: hi.as_ref().map(|h| h.sampling.clone()),
            progressive: hi.as_ref().map(|h| h.progressive),
            arithmetic: hi.as_ref().map(|h| h.arithmetic),
            restart_interval_mcus: hi.as_ref().map(|h| h.restart_interval),
            colour_model: hi.as_ref().map(|h| h.colour_model.clone()),
            exif_orientation: num("Orientation").map(|o| o as u8),
            has_gps: lat.is_some(),
            gps: lat.zip(lon).map(|(lat, lon)| Gps {
                place: s.gps_name.map(String::from),
                lat,
                lon,
            }),
            has_exif_thumbnail: num("ThumbnailLength").is_some(),
            icc_profile: match (s.icc_name, icc_desc) {
                (Some(n), Some(d)) => Some(format!("{n} [desc: {d}]")),
                (Some(n), None) => Some(n.to_string()),
                (None, d) => d,
            },
            expected_error: s.expected_error,
            expected_note: s.expected_note.map(String::from),
            purpose: s.purpose.clone(),
            tier: if matches!(s.kind, Kind::External) {
                "real-owner".into()
            } else {
                format!("{:?}", s.tier).to_lowercase()
            },
            source_url: url,
            source_page: page,
            licence,
            credit,
        });
    }
    let encoder = Command::new("cjpeg")
        .arg("-version")
        .output()
        .map(|o| String::from_utf8_lossy(&o.stderr).trim().to_string())
        .unwrap_or_default();
    let m = Manifest {
        generator: format!("specular-corpus {}", env!("CARGO_PKG_VERSION")),
        mode: if full { "full".into() } else { "small".into() },
        encoder: format!("cjpeg ({encoder}); YCCK via ImageMagick; CMYK via Pillow; 12-bit via built-in SOF1 encoder; metadata via exiftool"),
        icc_source: "Debian package icc-profiles-free, /usr/share/color/icc (Compact-ICC-Profiles by Clinton Ingram, CC0-1.0)".into(),
        notes: vec![
            "Content is procedural photo-like (sky/clouds, ridged horizon, fBm foliage, buildings with window grids, signs with bitmap text, vignette, sensor noise); bits_per_pixel shows the resulting entropy load.".into(),
            "Camera-like files carry EXIF (Make/Model/Date/exposure) and a 160 px IFD1 thumbnail, JFIF APP0 stripped, like real camera output.".into(),
            "Paths are relative to the directory holding this manifest; owner-supplied files are referenced in place as ../real/<name> (read-only, tier real-owner).".into(),
            "Orientation charts are stored pre-transformed so that honouring the EXIF tag shows an upright F with the red square top-left.".into(),
            "expected_error=true: the file must be refused or partially shown gracefully; never crash, hang, or allocate per the claimed dimensions before validating data.".into(),
        ],
        files,
    };
    std::fs::write(out.join("manifest.json"), serde_json::to_string_pretty(&m)?)?;
    Ok(())
}
