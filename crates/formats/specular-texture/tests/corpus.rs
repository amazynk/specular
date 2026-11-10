//! Container tests on the fetched texture corpus (`tools/corpus/fetch-textures.sh`); each test
//! passes without checking anything when the corpus is absent.

use std::path::{Path, PathBuf};

use specular_texture::{DecodeOptions, Decoded, Texture};

fn corpus(sub: &str) -> Option<PathBuf> {
    let base = std::env::var_os("SPECULAR_LARGE_CORPUS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../tools/corpus/large")
        });
    let d = base.join("texture").join(sub);
    d.is_dir().then_some(d)
}

fn files(d: &Path) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(d)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file())
        .collect();
    v.sort();
    v
}

fn decode(t: &Texture, data: &[u8], scale: u32) -> Decoded {
    t.decode(
        data,
        &DecodeOptions {
            scale,
            ..Default::default()
        },
    )
    .unwrap_or_else(|e| panic!("{}: {e}", t.description()))
}

/// Files the decoder does not read (yet), by name.
const UNSUPPORTED: &[&str] = &[];

/// Every KTX / KTX2 test file of KTX-Software opens and decodes every image at full size and
/// at 1/2 and 1/8 (the mips, or a box filter).
#[test]
fn ktx_software_files_decode() {
    let mut n = 0;
    for sub in [
        "ktx-software/tests/resources/ktx",
        "ktx-software/tests/resources/ktx2",
    ] {
        let Some(d) = corpus(sub) else { return };
        for p in files(&d) {
            let name = p.file_name().unwrap().to_string_lossy().to_string();
            let data = std::fs::read(&p).unwrap();
            let mut t = match Texture::open(&data) {
                Ok(t) => t,
                Err(e) => {
                    assert!(UNSUPPORTED.contains(&name.as_str()), "{name}: {e}");
                    eprintln!("{name}: {e}");
                    continue;
                }
            };
            for i in 0..t.image_count() {
                assert!(t.select_image(i));
                let full = decode(&t, &data, 1);
                assert!(!full.truncated, "{name} image {i} truncated");
                assert_eq!((full.width, full.height), (t.width, t.height));
                for s in [2, 8] {
                    decode(&t, &data, s);
                }
            }
            eprintln!("{name}: {} ({} images)", t.description(), t.image_count());
            n += 1;
        }
    }
    eprintln!("{n} files decoded");
}

/// The same texture in KTX and KTX2 decodes to the same pixels, image by image.
#[test]
fn ktx1_matches_ktx2() {
    let (Some(k1), Some(k2)) = (
        corpus("ktx-software/tests/resources/ktx"),
        corpus("ktx-software/tests/resources/ktx2"),
    ) else {
        return;
    };
    for name in [
        "etc2_unorm_array_7",
        "astc_8x8_unorm_array_7",
        "bc3_unorm_array_7",
    ] {
        let a = std::fs::read(k1.join(format!("{name}.ktx"))).unwrap();
        let b = std::fs::read(k2.join(format!("{name}.ktx2"))).unwrap();
        let (mut ta, mut tb) = (Texture::open(&a).unwrap(), Texture::open(&b).unwrap());
        assert_eq!(ta.image_count(), tb.image_count(), "{name}");
        assert_eq!(ta.image_count(), 7, "{name}");
        let mut previous = Vec::new();
        for i in 0..ta.image_count() {
            ta.select_image(i);
            tb.select_image(i);
            for s in [1, 2] {
                let (da, db) = (decode(&ta, &a, s), decode(&tb, &b, s));
                assert!(da.data == db.data, "{name} image {i} scale {s}");
            }
            // The layers differ from each other.
            let d = decode(&ta, &a, 1).data;
            assert!(d != previous, "{name} image {i} repeats the previous one");
            previous = d;
        }
    }
}

/// Basis Universal files (ETC1S / BasisLZ and UASTC) decode through the container like the
/// basisu transcoder (references from `fetch-textures.sh oracle_basis`): every image at full
/// size, and level 1 at scale 2 when it is exactly half. Two-channel files are shown as
/// (R, G, 0, 255) with G from the alpha slice.
#[test]
fn basis_matches_basisu() {
    let Some(root) = corpus("") else { return };
    let Ok(dir) = std::fs::read_dir(root.join("basis/ref")) else {
        return;
    };
    let mut srcs: Vec<PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "src"))
        .collect();
    srcs.sort();
    let mut images = 0;
    for p in srcs {
        let stem = p.file_stem().unwrap().to_string_lossy().to_string();
        let src = std::fs::read_to_string(&p).unwrap();
        let data = std::fs::read(root.join(src.trim())).unwrap();
        let mut t = Texture::open(&data).unwrap_or_else(|e| panic!("{stem}: {e}"));
        let rg = matches!(
            t.format,
            specular_texture::Format::Etc1s { rg: true, .. }
                | specular_texture::Format::Uastc { rg: true, .. }
        );
        for i in 0..t.image_count() {
            t.select_image(i);
            let (layer, face) = (i / t.faces, i % t.faces);
            for (l, s) in [(0, 1), (1, 2)] {
                let refp = root.join(format!("basis/ref/{stem}.l{l}.a{layer}.f{face}.rgba"));
                let Ok(mut want) = std::fs::read(&refp) else {
                    continue;
                };
                let got = decode(&t, &data, s);
                if got.data.len() != want.len() {
                    assert!(l > 0, "{stem} image {i}: size");
                    continue;
                }
                if rg {
                    for px in want.chunks_exact_mut(4) {
                        px.copy_from_slice(&[px[0], px[3], 0, 255]);
                    }
                }
                assert!(got.data == want, "{stem} image {i} level {l}");
                images += 1;
            }
        }
    }
    eprintln!("{images} Basis images match basisu");
}

/// Corrupted and truncated Basis files (BasisLZ global data, slices, UASTC blocks) are
/// refused or decoded, never a panic.
#[test]
fn corrupt_basis_files_never_panic() {
    let Some(d) = corpus("basis/gen") else { return };
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        state
    };
    for p in files(&d) {
        let data = std::fs::read(&p).unwrap();
        for round in 0..200 {
            let mut f = data.clone();
            if round % 10 == 0 {
                f.truncate((next() % data.len() as u64) as usize);
            } else {
                for _ in 0..1 + next() % 8 {
                    let at = (next() % f.len() as u64) as usize;
                    f[at] ^= 1 << (next() % 8);
                }
            }
            let Ok(mut t) = Texture::open(&f) else {
                continue;
            };
            let o = DecodeOptions {
                scale: 1 + (next() % 3) as u32,
                ..Default::default()
            };
            if t.memory_estimate(&o) < 256 << 20 {
                t.select_image((next() % u64::from(t.image_count())) as u32);
                let _ = t.decode(&f, &o);
            }
        }
    }
}

/// Orientation metadata: "rd" / "S=r,T=d" stays, "ru" / "S=r,T=u" flips.
#[test]
fn orientation_metadata() {
    for (sub, ext) in [
        ("ktx-software/tests/resources/ktx", "ktx"),
        ("ktx-software/tests/resources/ktx2", "ktx2"),
    ] {
        let Some(d) = corpus(sub) else { return };
        let open = |n: &str| Texture::open(&std::fs::read(d.join(format!("{n}.{ext}"))).unwrap());
        assert!(!open("orient_down_metadata").unwrap().flip_y, "{ext}");
        assert!(open("orient_up_metadata").unwrap().flip_y, "{ext}");
    }
}

/// Basis Universal v2 files (`src_basisu2`): every KTX2 and `.basis` file opens and decodes
/// whole at full size, 1/2 and 1/8 through the public API (the blocks are checked in the
/// crate's unit tests).
#[test]
fn basisu2_files_decode() {
    let Some(d) = corpus("basisu2") else { return };
    for p in files(&d) {
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "ktx2" && ext != "basis" {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let t = Texture::open(&data).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        for scale in [1, 2, 8] {
            let out = decode(&t, &data, scale);
            assert!(!out.truncated, "{} at 1/{scale}", p.display());
        }
    }
}
