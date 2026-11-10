//! The protocol against the in-memory camera: sessions, capture, events, object transfers
//! at every packet boundary, properties, and the errors a camera answers with.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use specular_ptp::codes::{ev, op, prop, rc};
use specular_ptp::mock::{Makes, MockCamera, pattern};
use specular_ptp::tether::{Arrivals, DownloadError, Happening, download};
use specular_ptp::{Camera, Error, TransportError, Value};

fn scratch(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("ptp")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn open() -> (Camera<MockCamera>, MockCamera) {
    let m = MockCamera::new();
    let h = m.handle();
    (Camera::open(m).expect("session"), h)
}

#[test]
fn session_opens_with_device_info_and_counts_transactions() {
    let (mut cam, m) = open();
    assert_eq!(cam.info().name(), "Mock M-1");
    assert!(cam.supports(op::INITIATE_CAPTURE));
    assert_eq!(m.state().session, Some(1));
    cam.storage_ids().unwrap();
    cam.storage_ids().unwrap();
    {
        let s = m.state();
        let tids: Vec<(u16, u32)> = s.log.iter().map(|(o, t, _)| (*o, *t)).collect();
        // GetDeviceInfo and OpenSession outside the session (id 0), then 1, 2.
        assert_eq!(
            tids,
            [
                (op::GET_DEVICE_INFO, 0),
                (op::OPEN_SESSION, 0),
                (op::GET_STORAGE_IDS, 1),
                (op::GET_STORAGE_IDS, 2)
            ]
        );
        assert_eq!(s.bad_reads, 0);
    }
    drop(cam);
    assert_eq!(
        m.state().session,
        None,
        "dropping the camera closes the session"
    );
}

#[test]
fn a_session_left_open_is_closed_and_opened_again() {
    let m = MockCamera::new();
    m.state().session = Some(7);
    let h = m.handle();
    let _cam = Camera::open(m).expect("reopened");
    let s = h.state();
    assert_eq!(s.session, Some(1));
    let ops: Vec<u16> = s.log.iter().map(|l| l.0).collect();
    assert_eq!(
        ops,
        [
            op::GET_DEVICE_INFO,
            op::OPEN_SESSION,
            op::CLOSE_SESSION,
            op::OPEN_SESSION
        ]
    );
}

#[test]
fn storages_battery_and_old_shots() {
    let (mut cam, _m) = open();
    let ids = cam.storage_ids().unwrap();
    assert_eq!(ids, [0x0001_0001]);
    let st = cam.storage_info(ids[0]).unwrap();
    assert_eq!(st.volume_label, "CARD");
    assert_eq!(st.free_images, 898);
    assert_eq!(cam.battery(), Some(80));
    // A folder and two shots of two files each.
    assert_eq!(cam.all_handles().unwrap().len(), 5);
    let e = cam.storage_info(0x0002_0001).unwrap_err();
    assert_eq!(e.code(), Some(rc::INVALID_STORAGE_ID));
}

#[test]
fn capture_events_and_downloads() {
    let dir = scratch("capture");
    let (mut cam, _m) = open();
    let t0 = Instant::now();
    let mut arr = Arrivals::start(&mut cam, t0).unwrap();
    assert_eq!(arr.known(), 5);
    cam.initiate_capture().unwrap();
    let mut got = Vec::new();
    for _ in 0..5 {
        got.extend(arr.poll(&mut cam, Duration::ZERO, t0).unwrap());
    }
    assert_eq!(
        got,
        [
            Happening::Object(0x105),
            Happening::Object(0x106),
            Happening::CaptureDone
        ]
    );
    let mut paths = Vec::new();
    for (i, h) in [0x105, 0x106].into_iter().enumerate() {
        let mut last = (0, 0);
        let d = download(
            &mut cam,
            h,
            &dir,
            &mut |info| format!("shoot-{}-{}", i + 1, info.filename),
            &mut |a, b| last = (a, b),
        )
        .unwrap()
        .expect("a file");
        assert_eq!(
            std::fs::read(&d.path).unwrap(),
            pattern(h, d.bytes as usize)
        );
        assert_eq!(last.0, d.bytes);
        assert_eq!(last.1, d.bytes);
        paths.push(d.path.file_name().unwrap().to_string_lossy().into_owned());
    }
    assert_eq!(paths, ["shoot-1-IMG_0003.RAW", "shoot-2-IMG_0003.JPG"]);
    // No part files left.
    let left: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().collect();
    assert_eq!(left.len(), 2);
    // The same name again gets a free variant.
    let d = download(
        &mut cam,
        0x105,
        &dir,
        &mut |i| i.filename.clone(),
        &mut |_, _| {},
    )
    .unwrap();
    let d2 = download(
        &mut cam,
        0x105,
        &dir,
        &mut |i| i.filename.clone(),
        &mut |_, _| {},
    )
    .unwrap();
    assert_eq!(d.unwrap().path.file_name().unwrap(), "IMG_0003.RAW");
    assert_eq!(d2.unwrap().path.file_name().unwrap(), "IMG_0003 (2).RAW");
    // A folder object downloads nothing.
    assert!(
        download(
            &mut cam,
            0x100,
            &dir,
            &mut |i| i.filename.clone(),
            &mut |_, _| {}
        )
        .unwrap()
        .is_none()
    );
}

/// Fed files are what the next shots make, one per shot, by name and bytes; then the
/// camera's own files again.
#[test]
fn fed_files_are_the_next_shots() {
    let (mut cam, m) = open();
    m.state().feed.extend([
        ("DSC0001.JPG".to_owned(), vec![0xFF, 0xD8, 1, 2, 3]),
        ("DSC0002.DNG".to_owned(), vec![b'I'; 700]),
    ]);
    for (name, len, jpeg) in [("DSC0001.JPG", 5, true), ("DSC0002.DNG", 700, false)] {
        let hs = m.press_shutter();
        assert_eq!(hs.len(), 1);
        let info = cam.object_info(hs[0]).unwrap();
        assert_eq!(info.filename, name);
        assert_eq!(info.format == specular_ptp::codes::format::EXIF_JPEG, jpeg);
        let mut got = Vec::new();
        cam.get_object(hs[0], &mut got, &mut |_, _| {}).unwrap();
        assert_eq!(got.len(), len);
    }
    assert_eq!(
        m.press_shutter().len(),
        2,
        "a RAW and a JPEG once the feed is empty"
    );
}

#[test]
fn shutter_on_the_camera_without_events_is_found_by_listing() {
    let (mut cam, m) = open();
    m.state().object_events = false;
    let t0 = Instant::now();
    let mut arr = Arrivals::start(&mut cam, t0).unwrap();
    let hs = m.press_shutter();
    // Not due yet.
    assert!(arr.poll(&mut cam, Duration::ZERO, t0).unwrap().is_empty());
    let later = t0 + arr.poll_every;
    let got = arr.poll(&mut cam, Duration::ZERO, later).unwrap();
    assert_eq!(
        got,
        hs.iter().map(|&h| Happening::Object(h)).collect::<Vec<_>>()
    );
    // Once an ObjectAdded event came, listing stops.
    m.state().object_events = true;
    let hs = m.press_shutter();
    let got = arr.poll(&mut cam, Duration::ZERO, later).unwrap();
    assert_eq!(got, [Happening::Object(hs[0])]);
    let n = m.state().log.len();
    let got = arr
        .poll(&mut cam, Duration::ZERO, later + arr.poll_every * 3)
        .unwrap();
    assert_eq!(got, [Happening::Object(hs[1])]);
    assert_eq!(m.state().log.len(), n, "no more listing");
}

/// Objects of every length around the packet and chunk sizes, with 64- and 512-byte
/// packets, the header in a transfer of its own, and a zero-length packet after a data
/// phase that fills its last packet.
#[test]
fn object_transfers_at_packet_boundaries() {
    let dir = scratch("sizes");
    for packet in [64usize, 512] {
        let m = MockCamera::new();
        m.state().packet = packet;
        let h = m.handle();
        let mut cam = Camera::open(m).unwrap();
        let mb = 1 << 20;
        for (k, n) in [
            0usize,
            1,
            packet - 12,
            packet - 13,
            packet - 11,
            2 * packet - 12,
            mb - 12,
            mb - 11,
            mb,
            3 * mb + 77,
        ]
        .into_iter()
        .enumerate()
        {
            {
                let mut s = h.state();
                s.makes = vec![Makes {
                    ext: "BIN",
                    format: 0x3000,
                    bytes: n,
                }];
                s.split_header = k % 3 == 1;
            }
            let hs = h.press_shutter();
            let d = download(
                &mut cam,
                hs[0],
                &dir,
                &mut |_| format!("{packet}-{n}"),
                &mut |_, _| {},
            )
            .unwrap()
            .unwrap();
            assert_eq!(d.bytes as usize, n, "packet {packet}, {n} bytes");
            assert_eq!(std::fs::read(&d.path).unwrap(), pattern(hs[0], n));
            // The session goes on (no zero-length packet taken for a response).
            cam.storage_ids().unwrap();
        }
        assert_eq!(h.state().bad_reads, 0, "reads are whole packets");
    }
}

#[test]
fn exposure_properties_step_and_set() {
    let (mut cam, m) = open();
    let ex = cam.exposure();
    let codes: Vec<u16> = ex.iter().map(|d| d.code).collect();
    assert_eq!(
        codes,
        [
            prop::EXPOSURE_PROGRAM_MODE,
            prop::EXPOSURE_TIME,
            prop::F_NUMBER,
            prop::EXPOSURE_INDEX,
            prop::EXPOSURE_BIAS_COMPENSATION,
            prop::WHITE_BALANCE
        ]
    );
    let iso = ex.iter().find(|d| d.code == prop::EXPOSURE_INDEX).unwrap();
    let up = iso.step(1).unwrap();
    assert_eq!(up, Value::UInt(200));
    cam.set_prop_value(iso.code, iso.data_type, &up).unwrap();
    assert_eq!(
        cam.prop_value(iso.code, iso.data_type).unwrap(),
        Value::UInt(200)
    );
    let e = cam.poll_event(Duration::ZERO).unwrap().unwrap();
    assert_eq!(
        (e.code, e.param(0)),
        (ev::DEVICE_PROP_CHANGED, Some(prop::EXPOSURE_INDEX as u32))
    );
    // Bias in whole stops on this camera.
    let bias = ex
        .iter()
        .find(|d| d.code == prop::EXPOSURE_BIAS_COMPENSATION)
        .unwrap();
    let down = bias.step(-1).unwrap();
    assert_eq!(down, Value::Int(-1000));
    cam.set_prop_value(bias.code, bias.data_type, &down)
        .unwrap();
    assert_eq!(m.state().props[&bias.code].current, Value::Int(-1000));
    // A value the camera doesn't offer, and a read-only property.
    let e = cam
        .set_prop_value(iso.code, iso.data_type, &Value::UInt(123))
        .unwrap_err();
    assert_eq!(e.code(), Some(rc::INVALID_DEVICE_PROP_VALUE));
    let e = cam
        .set_prop_value(prop::EXPOSURE_PROGRAM_MODE, 4, &Value::UInt(2))
        .unwrap_err();
    assert_eq!(e.code(), Some(rc::ACCESS_DENIED));
    let e = cam.prop_desc(0x5099).unwrap_err();
    assert_eq!(e.code(), Some(rc::DEVICE_PROP_NOT_SUPPORTED));
}

#[test]
fn camera_errors() {
    let dir = scratch("errors");
    let (mut cam, m) = open();
    // Busy, then fine.
    m.state().busy_captures = 1;
    let e = cam.initiate_capture().unwrap_err();
    assert!(e.is_busy(), "{e}");
    cam.initiate_capture().unwrap();
    // Card full.
    m.state().storages[0].1.free_images = 0;
    let e = cam.initiate_capture().unwrap_err();
    assert_eq!(e.code(), Some(rc::STORE_FULL));
    // A camera that doesn't list InitiateCapture isn't asked.
    let mut no_capture = MockCamera::new();
    no_capture
        .state()
        .info
        .operations
        .retain(|&o| o != op::INITIATE_CAPTURE);
    let n = no_capture.state().log.len();
    let mut c2 = Camera::open(no_capture.handle()).unwrap();
    assert!(matches!(
        c2.initiate_capture(),
        Err(Error::Unsupported(op::INITIATE_CAPTURE))
    ));
    assert_eq!(no_capture.state().log.len(), n + 2);
    let _ = &mut no_capture;
    // An operation answered with an error code.
    m.state()
        .fail
        .insert(op::GET_STORAGE_IDS, rc::GENERAL_ERROR);
    let e = cam.storage_ids().unwrap_err();
    assert_eq!(e.code(), Some(rc::GENERAL_ERROR));
    assert!(e.to_string().contains("General_Error"));
    // A bad handle.
    let e = cam.object_info(0xDEAD).unwrap_err();
    assert_eq!(e.code(), Some(rc::INVALID_OBJECT_HANDLE));
    // A transfer that stops: a timeout, the part file removed, the session goes on.
    m.state().cut_next_object = Some(100);
    let r = download(
        &mut cam,
        0x101,
        &dir,
        &mut |_| "cut.RAW".into(),
        &mut |_, _| {},
    );
    assert!(matches!(
        r,
        Err(DownloadError::Camera(Error::Transport(
            TransportError::Timeout
        )))
    ));
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    cam.storage_ids().unwrap();
    // Unplugged.
    m.state().unplugged = true;
    let e = cam.storage_ids().unwrap_err();
    assert!(e.is_gone());
    let mut arr_cam = cam;
    assert!(arr_cam.poll_event(Duration::ZERO).unwrap_err().is_gone());
}

#[test]
fn no_card_still_starts() {
    let (mut cam, m) = open();
    m.state().storages.clear();
    let arr = Arrivals::start(&mut cam, Instant::now()).unwrap();
    assert_eq!(arr.known(), 0);
    let e = cam.initiate_capture().unwrap_err();
    assert_eq!(e.code(), Some(rc::STORE_NOT_AVAILABLE));
}

#[test]
fn store_and_prop_events() {
    let (mut cam, m) = open();
    let mut arr = Arrivals::start(&mut cam, Instant::now()).unwrap();
    m.send_event(ev::STORE_FULL, &[0x0001_0001]);
    m.send_event(ev::DEVICE_PROP_CHANGED, &[prop::F_NUMBER as u32]);
    m.send_event(ev::OBJECT_REMOVED, &[0x101]);
    m.send_event(ev::STORE_REMOVED, &[0x0001_0001]);
    m.send_event(0xC101, &[]);
    let now = Instant::now();
    let mut got = Vec::new();
    for _ in 0..6 {
        got.extend(arr.poll(&mut cam, Duration::ZERO, now).unwrap());
    }
    assert_eq!(
        got,
        [
            Happening::StoreFull,
            Happening::PropChanged(prop::F_NUMBER),
            Happening::Storage
        ]
    );
    assert_eq!(arr.known(), 4);
}
