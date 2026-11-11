//! Protocol version 2 end to end: the server with a stand-in window, driven through the Rust
//! client crate (`specular-client`), as scripts drive the viewer. Every command makes the round
//! trip; version 1 clients keep working; hostile requests are refused.
#![allow(clippy::disallowed_methods)] // Tests may start unnamed threads.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Value, json};
use specular_client::{Client, Request};
use specular_platform::ipc;

use super::protocol::{
    ArraySource, Channel, Lang, Levels, Nav, RoiFormat, Shape, ShapeKind, ViewSet, Window, Zoom,
};
use super::server::{Call, Server};

fn test_dir(name: &str) -> PathBuf {
    ipc::scratch_dir(&format!("specular-v2-{name}")).unwrap()
}

/// A stand-in window: answers each request with what it was asked (so the test sees the
/// request arrived whole), a screenshot with a small gradient, and remembers opened files.
fn fake_window(calls: crossbeam_channel::Receiver<Call>) -> std::thread::JoinHandle<Vec<Request>> {
    std::thread::spawn(move || {
        let mut seen = Vec::new();
        while let Ok(c) = calls.recv() {
            seen.push(c.req.clone());
            match &c.req {
                Request::Screenshot => {
                    let (w, h) = (6u32, 4u32);
                    let rgba: Vec<u8> = (0..w * h)
                        .flat_map(|i| [i as u8 * 10, 0, 255, 255])
                        .collect();
                    c.answer_pixels(Ok(json!({})), (w, h, rgba));
                }
                Request::Quit => {
                    c.answer(Ok(json!({})));
                    break;
                }
                Request::Open { files, .. } => {
                    let r = json!({ "opened": files.len(), "missing": [] });
                    c.answer(Ok(r));
                }
                r => {
                    let echo = json!({ "cmd": r.cmd(), "debug": format!("{r:?}") });
                    c.answer(Ok(echo));
                }
            }
        }
        seen
    })
}

fn start(name: &str) -> (PathBuf, Server, std::thread::JoinHandle<Vec<Request>>) {
    let dir = test_dir(name);
    let server = Server::start(&dir, Arc::new(|| {})).unwrap();
    let window = fake_window(server.calls.clone());
    (dir, server, window)
}

fn connect(server: &Server) -> Client {
    Client::connect(server.path()).unwrap()
}

fn npy_shape_and_data(path: &Path) -> (Vec<u64>, String, Vec<u8>) {
    let b = std::fs::read(path).unwrap();
    let h = specular_arrays::npy::parse(&b).unwrap();
    assert!(!h.fortran);
    (
        h.shape.clone(),
        h.descr.clone(),
        b[h.data_offset..].to_vec(),
    )
}

#[test]
fn every_command_round_trips() {
    let (dir, server, window) = start("all");
    let mut c = connect(&server);
    // Version 2 as it was (a version 3 viewer answers it unchanged).
    c.set_version(2);

    // Capabilities.
    let hello = c.hello().unwrap();
    assert_eq!(hello["protocol"], 2);
    assert_eq!(hello["protocols"], json!([1, 2, 3]));
    assert!(hello.get("events").is_none(), "events are version 3's");
    let caps = &hello["capabilities"];
    assert_eq!(caps["max_chunk"], specular_client::protocol::MAX_CHUNK);
    assert!(
        caps["colormaps"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "viridis")
    );
    let shared = PathBuf::from(caps["shared_dir"].as_str().unwrap());
    assert!(shared.starts_with(&dir));
    let commands: Vec<&str> = hello["commands"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    for cmd in specular_client::protocol::COMMANDS_V2 {
        assert!(commands.contains(cmd), "{cmd}");
    }

    // Commands the window answers: each arrives as sent.
    let mut text = Shape::new(ShapeKind::Text, vec![[1.0, 2.0]]);
    text.text = "nucleus".into();
    let window_reqs = vec![
        Request::ViewGet,
        Request::ViewSet(ViewSet {
            zoom: Some(Zoom::Percent(300.0)),
            center: Some([12.5, 8.0]),
            levels: Some(Levels::Set {
                center: -600.0,
                width: 1500.0,
            }),
            colormap: Some("cividis".into()),
            channel: Some(Channel::Index(1)),
        }),
        Request::ViewSet(ViewSet {
            zoom: Some(Zoom::Fit),
            levels: Some(Levels::Auto),
            channel: Some(Channel::All),
            ..ViewSet::default()
        }),
        Request::Metadata,
        Request::OverlayAdd {
            layer: "cells".into(),
            shapes: vec![
                Shape::new(ShapeKind::Polygon, vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]]),
                text,
            ],
        },
        Request::OverlayList,
        Request::OverlayClear {
            layer: Some("cells".into()),
        },
        Request::OverlayClear { layer: None },
        Request::Rois {
            format: RoiFormat::GeoJson,
        },
        Request::Rois {
            format: RoiFormat::Json,
        },
        Request::ViewCommand { lang: Lang::Python },
        Request::Navigate(Nav::Next),
        Request::Navigate(Nav::Prev),
        Request::Navigate(Nav::Index(3)),
        Request::State,
        Request::Marked,
        Request::Action {
            name: "zoom-in".into(),
        },
        Request::Activate,
        Request::OpenUrl {
            url: "specular:///x.png".into(),
        },
        Request::Open {
            files: vec![dir.join("a.png")],
            window: Window::Current,
            view: Some(ViewSet {
                zoom: Some(Zoom::Percent(50.0)),
                ..ViewSet::default()
            }),
            compare: None,
        },
    ];
    for r in &window_reqs {
        let v = c.call(r).unwrap();
        if !matches!(r, Request::Open { .. }) {
            assert_eq!(v["cmd"], r.cmd(), "{r:?}");
        }
    }

    // Blobs: up and down in chunks, bigger than one chunk.
    let data: Vec<u8> = (0..1_300_000u32).map(|i| (i * 7 % 251) as u8).collect();
    let blob = c.upload(&data).unwrap();
    assert_eq!(c.download(blob, data.len() as u64).unwrap(), data);
    let e = c.call(&Request::BlobFree { blob }).unwrap_err();
    assert_eq!(e.code, "not-found", "download frees the blob");

    // Arrays: C order, and a Fortran-order view gathered into C order.
    let a: Vec<u8> = (0u16..12).flat_map(|v| v.to_le_bytes()).collect();
    let r = c
        .show_array(&a, &[3, 4], "<u2", None, Some("ramp"))
        .unwrap();
    let file = PathBuf::from(r["file"].as_str().unwrap());
    assert_eq!(file.file_name().unwrap(), "ramp.npy");
    assert_eq!(
        npy_shape_and_data(&file),
        (vec![3, 4], "<u2".into(), a.clone())
    );
    let f: Vec<u8> = [0u16, 4, 8, 1, 5, 9, 2, 6, 10, 3, 7, 11]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    let r = c
        .show_array(&f, &[3, 4], "<u2", Some(&[2, 6]), None)
        .unwrap();
    let file2 = PathBuf::from(r["file"].as_str().unwrap());
    assert_eq!(npy_shape_and_data(&file2).2, a);

    // Through the shared folder: the viewer takes the file.
    std::fs::write(shared.join("frame.raw"), &a).unwrap();
    let r = c
        .call(&Request::ArrayShow {
            source: ArraySource::File("frame.raw".into()),
            shape: vec![12],
            dtype: "<u2".into(),
            strides: None,
            name: Some("frame".into()),
        })
        .unwrap();
    assert_eq!(
        npy_shape_and_data(Path::new(r["file"].as_str().unwrap())).2,
        a
    );
    assert!(!shared.join("frame.raw").exists());

    // Screenshot: PNG bytes of what the window drew.
    let png = c.screenshot().unwrap();
    let r = png::Decoder::new(std::io::Cursor::new(&png))
        .read_info()
        .unwrap();
    assert_eq!((r.info().width, r.info().height), (6, 4));

    c.call(&Request::Quit).unwrap();
    let seen = window.join().unwrap();
    for r in &window_reqs {
        assert!(seen.contains(r), "{r:?} did not arrive whole");
    }
    // The arrays were opened in the window, as files.
    let opened = seen
        .iter()
        .filter(|r| matches!(r, Request::Open { files, .. } if files[0].extension().is_some_and(|e| e == "npy")))
        .count();
    assert_eq!(opened, 3);
    drop(server);
    assert!(!file.exists(), "the viewer's arrays go with it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn version_1_clients_keep_working() {
    let (dir, server, window) = start("v1");
    let mut c = connect(&server);
    c.set_version(1);
    let hello = c.hello().unwrap();
    assert_eq!(hello["protocol"], 1);
    assert!(hello.get("capabilities").is_none());
    assert_eq!(
        hello["commands"].as_array().unwrap().len(),
        specular_client::protocol::COMMANDS_V1.len()
    );
    assert_eq!(c.call(&Request::State).unwrap()["cmd"], "state");
    // A v1 `open` with a `view` member: ignored, as unknown members are in version 1.
    let line = json!({"v": 1, "id": 1, "cmd": "open", "files": [crate::remote::test_abs("/a.png")], "view": {"zoom": "bogus"}});
    let v: Value =
        serde_json::from_slice(&c.exchange(line.to_string().as_bytes()).unwrap()).unwrap();
    assert_eq!((v["v"].clone(), v["ok"].clone()), (json!(1), json!(true)));
    let e = c.call(&Request::ViewGet).unwrap_err();
    assert_eq!(e.code, "version");
    let e = c.call(&Request::Screenshot).unwrap_err();
    assert_eq!(e.code, "version");
    c.set_version(2);
    c.call(&Request::Quit).unwrap();
    let seen = window.join().unwrap();
    assert!(
        seen.iter()
            .any(|r| matches!(r, Request::Open { view: None, .. }))
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn hostile_requests_are_refused() {
    let dir = test_dir("hostile");
    // A small memory budget: big blobs are refused before anything is allocated.
    let budget = Arc::new(specular_core::budget::MemoryBudget::fixed(8 << 20));
    let server = Server::start_with(&dir, Arc::new(|| {}), Some(budget)).unwrap();
    let window = fake_window(server.calls.clone());
    let mut c = connect(&server);
    #[cfg_attr(not(unix), allow(unused_variables))] // checked by the Unix part below
    let shared = PathBuf::from(
        c.hello().unwrap()["capabilities"]["shared_dir"]
            .as_str()
            .unwrap(),
    );

    let e = c.call(&Request::BlobNew { bytes: 1 << 30 }).unwrap_err();
    assert_eq!(e.code, "no-memory", "{}", e.message);
    assert!(e.message.contains("MiB"));
    let b = c.call(&Request::BlobNew { bytes: 16 }).unwrap()["blob"]
        .as_u64()
        .unwrap();
    let e = c
        .call(&Request::BlobWrite {
            blob: b,
            offset: 10,
            data: vec![0; 10],
        })
        .unwrap_err();
    assert_eq!(e.code, "bad-arguments");
    let e = c
        .call(&Request::BlobRead {
            blob: 999,
            offset: 0,
            length: 1,
        })
        .unwrap_err();
    assert_eq!(e.code, "not-found");
    // An array bigger than its data, or strides past it.
    let e = c
        .call(&Request::ArrayShow {
            source: ArraySource::Blob(b),
            shape: vec![4, 4],
            dtype: "<f8".into(),
            strides: None,
            name: None,
        })
        .unwrap_err();
    assert_eq!(e.code, "bad-arguments");
    // Blobs per connection are capped; another connection has its own.
    let mut held = Vec::new();
    loop {
        match c.call(&Request::BlobNew { bytes: 1 }) {
            Ok(v) => held.push(v["blob"].as_u64().unwrap()),
            Err(e) => {
                assert_eq!(e.code, "busy");
                break;
            }
        }
        assert!(held.len() <= super::arrays::MAX_BLOBS);
    }
    let mut c2 = connect(&server);
    assert!(c2.call(&Request::BlobNew { bytes: 1 }).is_ok());
    // A blob of another connection is not this one's (c2 holds blob 1 only).
    assert!(held.contains(&5));
    let e = c2.call(&Request::BlobFree { blob: 5 }).unwrap_err();
    assert_eq!(e.code, "not-found");

    // The shared folder: only bare names of regular files that are nobody else's.
    let line =
        br#"{"v":2,"id":9,"cmd":"array-show","file":"../../etc/passwd","shape":[1],"dtype":"u1"}"#;
    let v: Value = serde_json::from_slice(&c2.exchange(line).unwrap()).unwrap();
    assert_eq!(v["error"]["code"], "bad-arguments");
    #[cfg(unix)]
    {
        let secret = dir.join("secret.raw");
        std::fs::write(&secret, [1u8; 8]).unwrap();
        std::os::unix::fs::symlink(&secret, shared.join("link.raw")).unwrap();
        std::fs::hard_link(&secret, shared.join("hard.raw")).unwrap();
        std::fs::create_dir(shared.join("dir.raw")).unwrap();
        for name in ["link.raw", "hard.raw", "dir.raw"] {
            let e = c2
                .call(&Request::ArrayShow {
                    source: ArraySource::File(name.into()),
                    shape: vec![8],
                    dtype: "u1".into(),
                    strides: None,
                    name: None,
                })
                .unwrap_err();
            assert_eq!(e.code, "refused", "{name}");
        }
        let e = c2
            .call(&Request::ArrayShow {
                source: ArraySource::File("missing.raw".into()),
                shape: vec![8],
                dtype: "u1".into(),
                strides: None,
                name: None,
            })
            .unwrap_err();
        assert_eq!(e.code, "not-found");
        // The folders are private.
        use std::os::unix::fs::PermissionsExt;
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&shared), 0o700);
        assert_eq!(mode(&dir.join("arrays")), 0o700);
    }
    c2.call(&Request::Quit).unwrap();
    window.join().unwrap();
    drop(server);
    let _ = std::fs::remove_dir_all(&dir);
}
