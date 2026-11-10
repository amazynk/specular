//! Damaged input must never panic or hang: every truncation and a
//! deterministic set of byte mutations of the small committed HEIC
//! fixtures (specular-decode-host test data: 4:2:0, 10-bit, lossless,
//! alpha) go through `open` / `info` / `decode_tile` / `decode`, with
//! both HEVC cores. The heic-rs core must also give the same result
//! serial and threaded on every damaged case.

use std::path::Path;
use std::sync::atomic::AtomicBool;

use specular_heic::{DecodeOptions, HeicFile, HeicRs, HevcCore};

/// The cores under test: the default (oxideav) and heic-rs.
const CORES: &[Option<&dyn HevcCore>] = &[None, Some(&HeicRs)];

const FIXTURES: &[&str] = &["grad.heic", "grad10.heic", "lossless.heic", "alpha.heic"];

fn fixture(name: &str) -> Vec<u8> {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../specular-decode-host/tests/data")
        .join(name);
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// One item holding its picture 2000 times over (each repeat a new IDR picture, a few
/// hundred bytes): the Oxideav core keeps the first output picture, not all of them.
#[test]
fn repeated_pictures_in_one_item() {
    use specular_heic::{CoreInput, Oxideav};
    let f = HeicFile::open(fixture("grad.heic")).unwrap();
    let node = f.graph();
    let node = node.inputs.first().unwrap_or(node);
    let hvcc = &node.properties.hvcc().expect("hvcC").raw;
    let once = f.container().item_data(node.item.id).unwrap().into_owned();
    let input = |data| CoreInput {
        hvcc,
        data,
        crop: None,
        threads: 1,
        cancel: None,
    };
    let one = Oxideav.decode(&input(&once)).unwrap();
    let many = Oxideav.decode(&input(&once.repeat(2000))).unwrap();
    assert_eq!(one.frame, many.frame);
}

fn exercise(data: Vec<u8>, parallel: bool, core: Option<&dyn HevcCore>) -> bool {
    let Ok(f) = HeicFile::open(data) else {
        return false;
    };
    let _ = f.info();
    let _ = f.exif();
    let _ = f.xmp();
    let opts = DecodeOptions {
        parallel,
        core,
        ..Default::default()
    };
    for i in 0..f.tile_count() {
        let _ = f.decode_tile(i, &opts);
    }
    f.decode(&opts).is_ok_and(|img| {
        img.to_rgba8()
            .is_ok_and(|rgba| rgba.len() == img.width() as usize * img.height() as usize * 4)
    })
}

#[test]
fn fixtures_decode() {
    for core in CORES {
        for name in FIXTURES {
            assert!(exercise(fixture(name), true, *core), "{name}");
        }
    }
}

#[test]
fn truncations() {
    for name in FIXTURES {
        let data = fixture(name);
        for n in 0..data.len() {
            for core in CORES {
                exercise(data[..n].to_vec(), n % 2 == 0, *core);
            }
        }
    }
}

/// 600 deterministic mutations (1–8 byte edits) of each fixture.
fn mutated() -> Vec<(&'static str, Vec<Vec<u8>>)> {
    let mut seed = 0x9e37_79b9_7f4a_7c15u64;
    let mut rnd = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    FIXTURES
        .iter()
        .map(|name| {
            let data = fixture(name);
            let cases = (0..600)
                .map(|_| {
                    let mut v = data.clone();
                    for _ in 0..1 + rnd() % 8 {
                        let pos = (rnd() % v.len() as u64) as usize;
                        v[pos] = match rnd() % 4 {
                            0 => 0,
                            1 => 0xff,
                            2 => v[pos] ^ (1 << (rnd() % 8)),
                            _ => rnd() as u8,
                        };
                    }
                    v
                })
                .collect();
            (*name, cases)
        })
        .collect()
}

#[test]
fn mutations() {
    for core in CORES {
        let cname = core.map_or("oxideav", |c| c.name());
        for (name, cases) in mutated() {
            let mut ok = 0;
            for (k, v) in cases.into_iter().enumerate() {
                ok += exercise(v, k % 2 == 0, *core) as usize;
            }
            eprintln!("{cname} {name}: {ok}/600 mutations decode");
        }
    }
}

/// The heic-rs core conceals damage deterministically: serial and
/// threaded decodes of every coded item agree, success or failure.
#[test]
fn heic_rs_serial_equals_threaded() {
    let opts = |parallel| DecodeOptions {
        parallel,
        core: Some(&HeicRs),
        ..Default::default()
    };
    let (serial, threaded) = (opts(false), opts(true));
    for (name, cases) in mutated() {
        let mut damaged = 0;
        for (k, v) in cases.into_iter().enumerate() {
            let Ok(f) = HeicFile::open(v) else { continue };
            let Ok(meta) = f.container().meta() else {
                continue;
            };
            let ids: Vec<u32> = meta.items.iter().map(|i| i.id).collect();
            for id in ids {
                match (f.decode_item(id, &serial), f.decode_item(id, &threaded)) {
                    (Ok(a), Ok(b)) => {
                        assert_eq!(a.frame, b.frame, "{name} case {k} item {id}");
                        assert_eq!(a.concealed_ctus, b.concealed_ctus);
                        damaged += (a.concealed_ctus > 0) as usize;
                    }
                    (Err(_), Err(_)) => {}
                    (a, b) => panic!("{name} case {k} item {id}: {:?} vs {:?}", a.err(), b.err()),
                }
            }
        }
        eprintln!("heic-rs {name}: {damaged} concealed items, serial == threaded");
    }
}

#[test]
fn cancellation() {
    let f = HeicFile::open(fixture("grad.heic")).unwrap();
    let flag = AtomicBool::new(true);
    for core in CORES {
        for parallel in [false, true] {
            let opts = DecodeOptions {
                cancel: Some(&flag),
                parallel,
                core: *core,
            };
            assert!(matches!(
                f.decode(&opts),
                Err(specular_heic::Error::Cancelled)
            ));
            assert!(matches!(
                f.decode_tile(0, &opts),
                Err(specular_heic::Error::Cancelled)
            ));
        }
    }
}

/// A core wrapper: the driver goes through the `HevcCore` boundary.
#[derive(Debug, Default)]
struct Counting(std::sync::atomic::AtomicUsize);

impl specular_heic::HevcCore for Counting {
    fn name(&self) -> &'static str {
        "counting"
    }
    fn decode(
        &self,
        input: &specular_heic::CoreInput<'_>,
    ) -> Result<specular_heic::CoreOutput, specular_heic::CoreError> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        specular_heic::HevcCore::decode(&specular_heic::Oxideav, input)
    }
}

#[test]
fn swappable_core() {
    let core = Counting::default();
    let f = HeicFile::open(fixture("alpha.heic")).unwrap();
    let a = f
        .decode(&DecodeOptions {
            core: Some(&core),
            ..Default::default()
        })
        .unwrap();
    let b = f.decode(&DecodeOptions::default()).unwrap();
    assert_eq!(a.frame.planes, b.frame.planes);
    // Colour + alpha items.
    assert_eq!(core.0.load(std::sync::atomic::Ordering::Relaxed), 2);
}

/// The fallback core takes over only when the primary reports
/// `Unsupported`, and decodes like the default.
#[test]
fn fallback_core_on_unsupported_only() {
    use specular_heic::{CoreError, CoreInput, CoreOutput, Fallback};
    #[derive(Debug)]
    struct Refuses(CoreErrorKind);
    #[derive(Debug, Clone, Copy)]
    enum CoreErrorKind {
        Unsupported,
        Invalid,
    }
    impl HevcCore for Refuses {
        fn name(&self) -> &'static str {
            "refuses"
        }
        fn decode(&self, _: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
            Err(match self.0 {
                CoreErrorKind::Unsupported => CoreError::Unsupported("test".into()),
                CoreErrorKind::Invalid => CoreError::Invalid("test".into()),
            })
        }
    }
    let f = HeicFile::open(fixture("alpha.heic")).unwrap();
    let reference = f.decode(&DecodeOptions::default()).unwrap();
    let core = Fallback {
        primary: Refuses(CoreErrorKind::Unsupported),
        fallback: HeicRs,
    };
    let a = f
        .decode(&DecodeOptions {
            core: Some(&core),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(a.frame.planes, reference.frame.planes);
    let core = Fallback {
        primary: Refuses(CoreErrorKind::Invalid),
        fallback: HeicRs,
    };
    assert!(
        f.decode(&DecodeOptions {
            core: Some(&core),
            ..Default::default()
        })
        .is_err()
    );
}

/// Cancellation inside the heic-rs core (serial and threaded): a core
/// wrapper hands it a raised flag the driver itself never saw.
#[test]
fn heic_rs_cancels_inside_the_core() {
    use specular_heic::{CoreError, CoreInput, CoreOutput};
    #[derive(Debug)]
    struct Probe;
    impl HevcCore for Probe {
        fn name(&self) -> &'static str {
            "probe"
        }
        fn decode(&self, input: &CoreInput<'_>) -> Result<CoreOutput, CoreError> {
            let raised = AtomicBool::new(true);
            for threads in [1, 4] {
                let i = CoreInput {
                    threads,
                    cancel: Some(&raised),
                    ..*input
                };
                assert!(matches!(HeicRs.decode(&i), Err(CoreError::Cancelled)));
            }
            HeicRs.decode(input)
        }
    }
    for name in FIXTURES {
        let f = HeicFile::open(fixture(name)).unwrap();
        f.decode(&DecodeOptions {
            core: Some(&Probe),
            ..Default::default()
        })
        .unwrap();
    }
}
