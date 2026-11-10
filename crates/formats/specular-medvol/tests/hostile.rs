//! Headers naming hostile data files.

use std::path::Path;

/// A MetaImage header whose data file is `/dev/zero` (endless) or one file named a million
/// times over: refused, or read only as far as the volume needs.
#[cfg(unix)]
#[test]
fn hostile_data_files() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("medvol-hostile");
    std::fs::create_dir_all(&dir).unwrap();
    let head = "ObjectType = Image\nNDims = 3\nDimSize = 4 4 4\nElementType = MET_UCHAR\n";

    let zero = dir.join("zero.mhd");
    std::fs::write(&zero, format!("{head}ElementDataFile = /dev/zero\n")).unwrap();
    assert!(specular_medvol::open_path(&zero).is_err());

    std::fs::write(dir.join("big.raw"), vec![7u8; 1 << 20]).unwrap();
    let list = dir.join("list.mhd");
    let names = "big.raw\n".repeat(200_000);
    std::fs::write(&list, format!("{head}ElementDataFile = LIST\n{names}")).unwrap();
    let t = std::time::Instant::now();
    let v = specular_medvol::open_path(&list);
    assert!(v.is_ok(), "{:?}", v.err());
    assert!(t.elapsed().as_secs() < 20, "{:?}", t.elapsed());
}

/// Text, hex and detached voxel data reserve from the memory budget, for as long as the
/// volume lives, and only what the data can hold.
#[test]
fn voxel_data_reserved() {
    use specular_core::budget::{MemoryBudget, with_budget};
    use specular_medvol::Error;
    use std::sync::Arc;
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("medvol-budget");
    std::fs::create_dir_all(&dir).unwrap();
    let open = |name: &str, bytes: &[u8], budget: u64| {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        let b = Arc::new(MemoryBudget::fixed(budget));
        let v = with_budget(b.clone(), || specular_medvol::open_path(&p));
        (v, b)
    };

    // 16 GiB of ASCII floats declared, three written: refused by the text's size before
    // anything is reserved or allocated.
    let ascii =
        b"NRRD0004\ntype: float\ndimension: 3\nsizes: 2048 2048 1024\nencoding: ascii\n\n1 2 3\n";
    let (v, _) = open("ascii.nrrd", ascii, 1 << 20);
    assert!(matches!(v, Err(Error::Truncated(_))), "{:?}", v.err());

    // 1 MiB of hex voxels: refused by a budget a byte short, held while the volume lives.
    let mut hex =
        b"NRRD0004\ntype: uchar\ndimension: 3\nsizes: 1024 1024 1\nencoding: hex\n\n".to_vec();
    hex.extend(b"0a".repeat(1 << 20));
    let (v, _) = open("hex.nrrd", &hex, (1 << 20) - 1);
    assert!(matches!(v, Err(Error::NeedsMemory(_))), "{:?}", v.err());
    let (v, b) = open("hex.nrrd", &hex, 4 << 20);
    let v = v.unwrap();
    assert_eq!(b.reserved(), 1 << 20);
    drop(v);
    assert_eq!(b.reserved(), 0);
    // Declared larger than the digits: refused before reserving.
    let mut short =
        b"NRRD0004\ntype: uchar\ndimension: 3\nsizes: 4096 4096 64\nencoding: hex\n\n".to_vec();
    short.extend(b"0a".repeat(1000));
    let (v, _) = open("short.nrrd", &short, 1 << 20);
    assert!(matches!(v, Err(Error::Truncated(_))), "{:?}", v.err());

    // Detached parts, 4 × 1 MiB: refused under 2 MiB, held while the volume lives.
    std::fs::write(dir.join("part.raw"), vec![7u8; 1 << 20]).unwrap();
    let list = "ObjectType = Image\nNDims = 3\nDimSize = 1024 1024 4\nElementType = MET_UCHAR\nElementDataFile = LIST\npart.raw\npart.raw\npart.raw\npart.raw\n";
    let (v, _) = open("list.mhd", list.as_bytes(), 2 << 20);
    assert!(matches!(v, Err(Error::NeedsMemory(_))), "{:?}", v.err());
    let (v, b) = open("list.mhd", list.as_bytes(), 8 << 20);
    let v = v.unwrap();
    assert_eq!(b.reserved(), 4 << 20);
    drop(v);
    assert_eq!(b.reserved(), 0);
}

/// NRRD voxels in bzip2 (Python's `bz2` wrote the stream: bytes 0 to 23) open through the
/// in-tree bzip2 decoder; a damaged stream is refused, not misread.
#[test]
fn nrrd_bzip2_voxels() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("medvol-bzip2");
    std::fs::create_dir_all(&dir).unwrap();
    let bz: &[u8] = b"\x42\x5a\x68\x39\x31\x41\x59\x26\x53\x59\xf7\xcd\x3b\x61\x00\x00\x00\x60\x00\x7f\xff\xff\x80\x20\x00\x22\x80\x01\xa6\x86\x4c\x85\x06\x8d\x1a\x0c\x80\xd1\x5c\xf8\x46\x53\xa5\x6d\x7c\x64\x34\x3b\xe7\x48\xfc\x8b\xf1\x77\x24\x53\x85\x09\x0f\x7c\xd3\xb6\x10";
    let mut file =
        b"NRRD0004\ntype: uchar\ndimension: 3\nsizes: 2 3 4\nencoding: bzip2\n\n".to_vec();
    file.extend_from_slice(bz);
    let p = dir.join("v.nrrd");
    std::fs::write(&p, &file).unwrap();
    let v = specular_medvol::open_path(&p).unwrap();
    for k in 0..4 {
        for j in 0..3 {
            for i in 0..2 {
                assert_eq!(v.value(i, j, k, 0, 0), (i + 2 * j + 6 * k) as f64);
            }
        }
    }
    let n = file.len();
    file[n - 20] ^= 0x55;
    std::fs::write(&p, &file).unwrap();
    assert!(specular_medvol::open_path(&p).is_err());
}

/// Cut and damaged PAR/REC and ECAT 7 files are refused with an error, never a panic: every
/// prefix of the samples, every byte of an ECAT header flipped, a REC shorter than its PAR
/// says.
#[test]
fn cut_parrec_and_ecat_files() {
    let data = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("medvol-cut");
    std::fs::create_dir_all(&dir).unwrap();
    for name in ["ecat_i16.v", "ecat_frames.v", "ecat_le_f32.v"] {
        let full = std::fs::read(data.join(name)).unwrap();
        let p = dir.join(name);
        for n in (0..full.len()).step_by(97) {
            std::fs::write(&p, &full[..n]).unwrap();
            let _ = specular_medvol::open_path(&p);
        }
        for at in 0..1536 {
            let mut b = full.clone();
            b[at] ^= 0xA5;
            std::fs::write(&p, &b).unwrap();
            let _ = specular_medvol::open_path(&p);
        }
        // The last frame cut inside its samples (its 512-byte block holds 120 or more).
        std::fs::write(&p, &full[..full.len() - 512 + 60]).unwrap();
        assert!(specular_medvol::open_path(&p).is_err(), "{name}: cut data");
    }
    let par = std::fs::read(data.join("par_tra.PAR")).unwrap();
    let rec = std::fs::read(data.join("par_tra.REC")).unwrap();
    std::fs::write(dir.join("cut.PAR"), &par).unwrap();
    std::fs::write(dir.join("cut.REC"), &rec[..rec.len() / 2]).unwrap();
    let e = specular_medvol::open_path(&dir.join("cut.PAR")).unwrap_err();
    assert!(e.to_string().contains("ends early"), "{e}");
    for n in (0..par.len()).step_by(13) {
        std::fs::write(dir.join("cut.PAR"), &par[..n]).unwrap();
        let _ = specular_medvol::open_path(&dir.join("cut.PAR"));
    }
    // A header without its REC still reads; the voxels then fail by name.
    std::fs::write(dir.join("alone.PAR"), &par).unwrap();
    let h = specular_medvol::read_header(&par, &dir.join("alone.PAR")).unwrap();
    assert_eq!(h.layout.size, [5, 4, 3]);
    let e = specular_medvol::open_path(&dir.join("alone.PAR")).unwrap_err();
    assert!(e.to_string().contains("alone.REC"), "{e}");
}
