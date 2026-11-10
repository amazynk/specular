use std::path::{Path, PathBuf};

use super::*;

fn data(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/data")
        .join(name)
}

#[test]
fn opens_every_fixture() {
    for (name, shape, pixel, storage, be) in [
        (
            "cube_le.image",
            &[13, 11, 2, 5][..],
            Pixel::Float,
            "TiledCellStMan",
            false,
        ),
        (
            "cube_be.image",
            &[9, 8, 2, 3],
            Pixel::Double,
            "TiledCellStMan",
            true,
        ),
        (
            "cplx.image",
            &[6, 5, 3, 2],
            Pixel::Complex,
            "TiledCellStMan",
            false,
        ),
        (
            "dcplx.image",
            &[4, 3, 1, 2],
            Pixel::DComplex,
            "TiledCellStMan",
            false,
        ),
        (
            "flat.residual",
            &[7, 9],
            Pixel::Float,
            "TiledCellStMan",
            false,
        ),
        (
            "masked_be.image",
            &[10, 6, 1, 2],
            Pixel::Float,
            "TiledCellStMan",
            true,
        ),
        (
            "shape_le.model",
            &[7, 6, 1, 4],
            Pixel::Float,
            "TiledShapeStMan",
            false,
        ),
        (
            "shape_be.pb",
            &[5, 9, 1, 3],
            Pixel::Float,
            "TiledShapeStMan",
            true,
        ),
    ] {
        let im = CasaImage::open(&data(name)).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert_eq!(im.shape, shape, "{name}");
        assert_eq!(im.pixel, pixel, "{name}");
        assert_eq!(im.storage, storage, "{name}");
        assert_eq!(im.big_endian, be, "{name}");
        // The files inside open the directory too.
        let via = CasaImage::open(&data(name).join("table.dat")).unwrap();
        assert_eq!(via.dir, im.dir);
        assert!(is_image_dir(&data(name)));
    }
}

#[test]
fn image_types_from_names() {
    for (name, t) in [
        ("a.image", ImageType::Image),
        ("a.image.tt0", ImageType::Image),
        ("a.residual.tt1", ImageType::Residual),
        ("a.model", ImageType::Model),
        ("a.pb", ImageType::PrimaryBeam),
        ("x.mask", ImageType::Mask),
        ("x.psf", ImageType::Psf),
        ("x.image.pbcor", ImageType::Image),
        ("x.foo", ImageType::Other),
    ] {
        assert_eq!(ImageType::from_name(Path::new(name)), t, "{name}");
    }
}

#[test]
fn keywords_and_axes() {
    let im = CasaImage::open(&data("cube_le.image")).unwrap();
    assert_eq!(im.units.as_deref(), Some("Jy/beam"));
    assert_eq!(im.object.as_deref(), Some("Orion KL"));
    assert_eq!(im.coords.telescope.as_deref(), Some("ALMA"));
    assert_eq!(im.coords.projection.as_deref(), Some("SIN"));
    assert_eq!(im.mask_name.as_deref(), Some("mask0"));
    assert_eq!(im.coords.axes[2].stokes, ["I", "V"]);
    assert_eq!(im.coords.axes[3].kind, AxisKind::Spectral);
    let sp = im.coords.spectral.as_ref().unwrap();
    assert_eq!(sp.system, "LSRK");
    assert!((sp.rest - 230.538e9).abs() < 1.0);
    let b = im.beam.unwrap();
    assert!((b.major - 1.25).abs() < 1e-12 && (b.pa + 35.0).abs() < 1e-12);
    let pb = CasaImage::open(&data("cube_be.image")).unwrap();
    let beams = pb.plane_beams.as_ref().unwrap();
    assert_eq!((beams.channels, beams.stokes), (3, 2));
    assert_eq!(pb.beam_at(&[1, 2]).unwrap().major, 1.0 + 0.5 * 5.0);
    let flat = CasaImage::open(&data("flat.residual")).unwrap();
    assert_eq!(flat.image_type, ImageType::Residual);
    assert!((flat.beam.unwrap().minor - 36.0).abs() < 1e-9);
    assert!(
        flat.coords
            .cards
            .iter()
            .any(|(k, v)| k == "CTYPE1" && v == "'GLON-TAN'")
    );
}

#[test]
fn truncated_headers_never_panic() {
    for name in ["cube_le.image", "shape_le.model", "cube_be.image"] {
        let dat = std::fs::read(data(name).join("table.dat")).unwrap();
        assert!(Table::parse(&dat).is_ok());
        for n in (0..dat.len()).step_by(7) {
            assert!(Table::parse(&dat[..n]).is_err(), "{name} cut at {n}");
        }
        let f0 = std::fs::read(data(name).join("table.f0")).unwrap();
        assert!(tsm::Tsm::parse(&f0).is_ok());
        for n in 0..f0.len() {
            assert!(tsm::Tsm::parse(&f0[..n]).is_err(), "{name} f0 cut at {n}");
        }
        // Flipped bytes: any result but a panic.
        for i in (0..dat.len()).step_by(5) {
            let mut d = dat.clone();
            d[i] ^= 0x5a;
            let _ = Table::parse(&d);
        }
        for i in 0..f0.len() {
            let mut d = f0.clone();
            d[i] ^= 0xa5;
            let _ = tsm::Tsm::parse(&d);
        }
    }
}

/// A plane and its mask are reserved from the memory budget before they are allocated, the
/// plane still counted while the mask (a byte out for each bit stored) is read.
#[test]
fn planes_and_masks_reserved() {
    use specular_core::budget::with_limit;
    let im = CasaImage::open(&data("masked_be.image")).unwrap();
    assert!(im.has_mask());
    // 10 × 6 floats (240 bytes), then 60 mask bytes beside them.
    let r = with_limit(239, || im.plane(&[0, 1]));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
    let r = with_limit(299, || im.plane(&[0, 1]));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
    assert!(with_limit(300, || im.plane(&[0, 1])).is_ok());
    let r = with_limit(59, || im.plane_mask(&[0, 1]));
    assert!(matches!(r, Err(Error::NeedsMemory(_))), "{r:?}");
    assert_eq!(
        with_limit(60, || im.plane_mask(&[0, 1]))
            .unwrap()
            .unwrap()
            .len(),
        60
    );
}

#[test]
fn not_casa() {
    assert!(matches!(
        CasaImage::open(Path::new(env!("CARGO_MANIFEST_DIR"))),
        Err(Error::NotCasa)
    ));
    assert!(!sniff(b"SIMPLE  =                    T"));
}

#[test]
fn bad_plane_index() {
    let im = CasaImage::open(&data("cube_le.image")).unwrap();
    assert!(im.plane(&[2, 0]).is_err());
    assert!(im.plane(&[0]).is_err());
    assert!(im.line(&[13, 0, 0, 0], 3).is_err());
    assert_eq!(im.line(&[3, 4, 1, 0], 3).unwrap().len(), 5);
}

/// A FIFO named `table.dat` is not opened (that blocks until a writer comes): the folder is
/// not a CASA image, at once.
#[cfg(unix)]
#[test]
fn a_fifo_table_is_not_opened() {
    let d = std::env::temp_dir().join(format!("specular-casa-fifo-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    let ok = std::process::Command::new("mkfifo")
        .arg(d.join("table.dat"))
        .status()
        .is_ok_and(|s| s.success());
    assert!(ok, "mkfifo");
    let (tx, rx) = std::sync::mpsc::channel();
    let dir = d.clone();
    #[allow(clippy::disallowed_methods)]
    std::thread::spawn(move || {
        let _ = tx.send(is_image_dir(&dir));
    });
    let is = rx
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("blocked on a FIFO");
    assert!(!is);
    let _ = std::fs::remove_dir_all(&d);
}

mod hostile {
    use crate::record::{Array, ArrayData, Record, Value};

    fn floats(n: usize) -> Value {
        Value::Array(Array {
            shape: vec![n],
            data: ArrayData::Float(vec![0.0; n]),
        })
    }

    #[test]
    fn linear_coordinate_with_huge_axis_count() {
        // 50,000 axes once asked for a 50,000² pc matrix (20 GB).
        let n = 50_000;
        let linear = Record {
            fields: vec![
                ("crval".into(), floats(n)),
                ("crpix".into(), floats(n)),
                ("cdelt".into(), floats(n)),
            ],
        };
        let coords = Record {
            fields: vec![
                ("linear0".into(), Value::Record(linear)),
                (
                    "pixelmap0".into(),
                    Value::Array(Array {
                        shape: vec![1],
                        data: ArrayData::Int(vec![0]),
                    }),
                ),
            ],
        };
        let c = crate::coords::parse(&coords, 1);
        assert_eq!(c.axes.len(), 1);
    }
}
