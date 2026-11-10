//! Blackmagic RAW through a vendor SDK in the host, with the mock SDK library
//! (`examples/braw_mock.rs`, built by `cargo test` beside the host): loading, the 6.0 check,
//! decoding through the host's file object inside the full sandbox, scales, limits and the
//! failures. Linux only (the mock is); the real SDK is not on the build machine.
#![cfg(target_os = "linux")]
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use specular_sandbox::protocol::{ErrorKind, ImageLimits};
use specular_sandbox::{ClipSource, Config, Error, Host, SandboxLevel, VendorLibs};
use std::path::PathBuf;
use std::time::Duration;

/// The mock library: `target/<profile>/examples/libbraw_mock.so`, next to the host's folder.
fn mock() -> PathBuf {
    let host = PathBuf::from(env!("CARGO_BIN_EXE_specular-decode-host"));
    let p = host.parent().unwrap().join("examples/libbraw_mock.so");
    assert!(
        p.is_file(),
        "{} missing: run `cargo test -p specular-decode-host` (it builds the examples)",
        p.display()
    );
    p
}

fn host(braw: Option<PathBuf>, env: &[(&str, &str)]) -> Host {
    Host::new(Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        timeout: Duration::from_secs(20),
        pool_size: 1,
        test_hooks: true,
        env: env.iter().map(|(k, v)| (k.into(), v.into())).collect(),
        vendor: VendorLibs { braw },
        ..Config::default()
    })
}

/// A mock clip: `w`×`h`, `frames` frames at `rate`, recorded by `camera`.
fn clip(w: u32, h: u32, frames: u64, rate: f32, camera: &str) -> ClipSource {
    let mut b = b"MOCKBRAW".to_vec();
    b.extend_from_slice(&w.to_le_bytes());
    b.extend_from_slice(&h.to_le_bytes());
    b.extend_from_slice(&frames.to_le_bytes());
    b.extend_from_slice(&rate.to_bits().to_le_bytes());
    b.extend_from_slice(&(camera.len() as u32).to_le_bytes());
    b.extend_from_slice(camera.as_bytes());
    b.resize(b.len() + 100, 0);
    ClipSource::from_bytes(&b).unwrap()
}

#[test]
fn loads_and_reports_the_sdk() {
    let h = host(Some(mock()), &[]);
    let info = h.info().unwrap();
    assert_eq!(
        info.braw.as_deref(),
        Some("6.0 or later (cameras 6.0)"),
        "{info:?}"
    );
    assert!(info.braw_error.is_empty());
    assert!(info.sandbox >= SandboxLevel::Partial, "{info:?}");
    // A folder holding the library is accepted too.
    let h = host(Some(mock().parent().unwrap().to_path_buf()), &[]);
    let info = h.info().unwrap();
    // The folder has no libBlackmagicRawAPI.so: refused with the reason.
    assert!(info.braw.is_none());
    assert!(
        info.braw_error.contains("not the Blackmagic RAW SDK"),
        "{info:?}"
    );
}

#[test]
fn decodes_a_frame_inside_the_sandbox() {
    let h = host(Some(mock()), &[]);
    let c = clip(64, 48, 10, 23.976, "Mock URSA");
    let (info, img) = h.braw_decode(&c, 3, 1, ImageLimits::default()).unwrap();
    assert_eq!((info.width, info.height, info.frame_count), (64, 48, 10));
    assert!((info.frame_rate - 23.976).abs() < 1e-4);
    // The SDK running in the host could not open a file by path.
    assert_eq!(
        info.camera, "Mock URSA",
        "the sandbox let the SDK open a file"
    );
    assert_eq!((img.width, img.height, img.bit_depth), (64, 48, 12));
    assert!(!img.has_alpha);
    let px = |x: usize, y: usize| &img.rgba[(y * 64 + x) * 4..][..4];
    assert_eq!(px(0, 0), [0, 0, 3, 255]);
    assert_eq!(px(63, 47), [63, 47, 3, 255]);
    // Lower resolutions, as the SDK's half / quarter / eighth.
    for (s, w) in [(2u8, 32u32), (4, 16), (8, 8), (3, 32), (16, 8)] {
        let (_, img) = h.braw_decode(&c, 9, s, ImageLimits::default()).unwrap();
        assert_eq!((img.width, img.height * 4 / 3), (w, w), "scale {s}");
        assert_eq!(&img.rgba[4..8], &[64 / w as u8, 0, 9, 255]);
    }
}

#[test]
fn large_frames_travel_in_shared_memory() {
    let h = host(Some(mock()), &[]);
    let c = clip(1024, 512, 1, 25.0, "");
    let (info, img) = h.braw_decode(&c, 0, 1, ImageLimits::default()).unwrap();
    assert_eq!(info.camera, "");
    assert_eq!(img.rgba.len(), 1024 * 512 * 4);
    assert_eq!(&img.rgba[(511 * 1024 + 1023) * 4..], &[255, 255, 0, 255]);
}

#[test]
fn failures_are_plain_errors() {
    let h = host(Some(mock()), &[]);
    // Past the last frame.
    let c = clip(8, 8, 2, 24.0, "x");
    match h.braw_decode(&c, 2, 1, ImageLimits::default()) {
        Err(Error::Remote { kind, message }) => {
            assert_eq!(kind, ErrorKind::Unsupported);
            assert!(message.contains("2 frames"), "{message}");
        }
        r => panic!("{r:?}"),
    }
    // Not a clip.
    let junk = ClipSource::from_bytes(&[7u8; 100]).unwrap();
    match h.braw_decode(&junk, 0, 1, ImageLimits::default()) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::Malformed),
        r => panic!("{r:?}"),
    }
    // Larger than the limits.
    let big = clip(4000, 3000, 1, 24.0, "x");
    let small = ImageLimits {
        max_pixels: 1_000_000,
        max_side: 65_535,
    };
    match h.braw_decode(&big, 0, 1, small) {
        Err(Error::Remote { kind, .. }) => assert_eq!(kind, ErrorKind::TooLarge),
        r => panic!("{r:?}"),
    }
    // The same host still decodes afterwards.
    assert!(h.braw_decode(&c, 1, 1, ImageLimits::default()).is_ok());
}

#[test]
fn an_older_sdk_is_refused_with_the_reason() {
    let h = host(Some(mock()), &[("SPECULAR_BRAW_MOCK", "old")]);
    let info = h.info().unwrap();
    assert!(info.braw.is_none());
    assert!(info.braw_error.contains("6.0 or later"), "{info:?}");
    let c = clip(8, 8, 1, 24.0, "x");
    match h.braw_decode(&c, 0, 1, ImageLimits::default()) {
        Err(Error::NotInstalled(m)) => assert!(m.contains("too old"), "{m}"),
        r => panic!("{r:?}"),
    }
}

#[test]
fn without_the_sdk() {
    let h = host(None, &[]);
    let info = h.info().unwrap();
    assert!(info.braw.is_none() && info.braw_error.is_empty());
    let c = clip(8, 8, 1, 24.0, "x");
    assert!(matches!(
        h.braw_decode(&c, 0, 1, ImageLimits::default()),
        Err(Error::NotInstalled(_))
    ));
    let h = host(Some("/nonexistent/libBlackmagicRawAPI.so".into()), &[]);
    let info = h.info().unwrap();
    assert!(info.braw.is_none());
    assert!(!info.braw_error.is_empty());
}

/// Control for the probe above: without the sandbox the mock SDK can open a file by path.
#[test]
fn the_file_probe_works_without_the_sandbox() {
    let h = Host::new(Config {
        exe: Some(env!("CARGO_BIN_EXE_specular-decode-host").into()),
        pool_size: 1,
        test_hooks: true,
        disable_sandbox: true,
        require_sandbox: false,
        vendor: VendorLibs { braw: Some(mock()) },
        ..Config::default()
    });
    let c = clip(8, 8, 1, 24.0, "Mock");
    let (info, _) = h.braw_decode(&c, 0, 1, ImageLimits::default()).unwrap();
    assert_eq!(info.camera, "Mock [opened a file]");
}

#[test]
fn changing_the_sdk_restarts_the_host() {
    let h = host(None, &[]);
    assert!(h.info().unwrap().braw.is_none());
    let before = h.pids();
    h.set_vendor_libs(VendorLibs { braw: Some(mock()) });
    assert!(h.pids().is_empty(), "the old host stopped");
    assert!(h.info().unwrap().braw.is_some());
    assert_ne!(h.pids(), before);
    let c = clip(8, 8, 1, 24.0, "x");
    assert!(h.braw_decode(&c, 0, 8, ImageLimits::default()).is_ok());
    // Setting the same again keeps the running host.
    let now = h.pids();
    h.set_vendor_libs(VendorLibs { braw: Some(mock()) });
    assert_eq!(h.pids(), now);
}
