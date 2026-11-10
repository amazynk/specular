//! End to end: frames encoded by `specular-h264enc`, written by [`Mp4Writer`], read back by
//! the in-tree demuxer, and checked with ffmpeg/ffprobe (a test oracle only; skipped with a
//! note when they are not installed): the file decodes without a complaint, is tagged BT.709
//! limited range in both the `colr` box and the VUI, plays at the export's frame rate, and
//! the decoder's pictures are the encoder's reconstruction.

use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

use specular_h264enc::{Config, Encoder, Frame, Profile};
use specular_video::mp4::{self, Codec, Colour};
use specular_video::mp4write::{Mp4Writer, Track};

const W: usize = 192;
const H: usize = 108;

/// A moving gradient with a bar (BT.709 limited range 4:2:0).
fn picture(i: usize) -> [Vec<u8>; 3] {
    let y = (0..W * H)
        .map(|k| {
            let (x, yy) = (k % W, k / W);
            let bar = (x + 3 * i) % 64 < 8;
            if bar {
                200
            } else {
                (40 + (x + yy + 2 * i) % 150) as u8
            }
        })
        .collect();
    let c = (W / 2) * (H / 2);
    let u = (0..c).map(|k| (110 + (k % 37)) as u8).collect();
    let v = (0..c).map(|k| (140 - (k / 96 % 23)) as u8).collect();
    [y, u, v]
}

fn have(tool: &str) -> bool {
    Command::new(tool)
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join("mp4export");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// 45 frames at 30 fps with a hard cut at 30; returns (file, reconstruction, key flags).
fn movie(profile: Profile) -> (Vec<u8>, Vec<u8>, Vec<bool>) {
    let mut cfg = Config::new(W as u32, H as u32, 30, 1, 24);
    cfg.profile = profile;
    let mut enc = Encoder::new(cfg).unwrap();
    let track = Track {
        width: W as u32,
        height: H as u32,
        fps_num: 30,
        fps_den: 1,
        avcc: enc.avcc(),
        colour: Colour {
            primaries: 1,
            transfer: 1,
            matrix: 1,
            full_range: false,
        },
    };
    let mut w = Mp4Writer::new(Cursor::new(Vec::new()), track, 45).unwrap();
    let mut recon = Vec::new();
    let mut keys = Vec::new();
    for i in 0..45 {
        let [y, u, v] = picture(if i < 30 { i } else { i + 500 });
        let p = enc.encode(
            &Frame {
                y: &y,
                u: &u,
                v: &v,
                y_stride: W,
                c_stride: W / 2,
            },
            i == 30,
        );
        w.write_sample(&p.data, p.key).unwrap();
        keys.push(p.key);
        for plane in enc.reconstruction() {
            recon.extend_from_slice(&plane);
        }
    }
    (w.finish().unwrap().into_inner(), recon, keys)
}

#[test]
fn demuxer_reads_the_export_back() {
    let (data, _, keys) = movie(Profile::default());
    let m = mp4::parse(&data, true).unwrap();
    assert_eq!(m.duration_us(), Some(1_500_000));
    let v = m.video.unwrap();
    assert_eq!(v.codec, Some(Codec::H264));
    assert_eq!((v.width, v.height), (W as u32, H as u32));
    assert_eq!(v.samples.len(), 45);
    let sync: Vec<bool> = v.samples.iter().map(|s| s.sync).collect();
    assert_eq!(sync, keys);
    assert!(sync[0] && sync[30], "the start and the hard cut are IDR");
    assert_eq!(sync.iter().filter(|k| **k).count(), 2);
}

#[test]
fn ffmpeg_decodes_the_export_exactly() {
    if !have("ffmpeg") || !have("ffprobe") {
        eprintln!("ffmpeg/ffprobe not installed; oracle test skipped");
        return;
    }
    // The export's default, and its maximum-compatibility choice.
    for (profile, name) in [
        (Profile::High, "High"),
        (Profile::Baseline, "Constrained Baseline"),
    ] {
        check_with_ffmpeg(profile, name);
    }
}

fn check_with_ffmpeg(profile: Profile, profile_name: &str) {
    let (data, recon, _) = movie(profile);
    let path = scratch(&format!("e2e-{profile:?}.mp4"));
    std::fs::write(&path, &data).unwrap();
    let probe = Command::new("ffprobe")
        .args(["-v", "error", "-show_streams", "-show_format"])
        .arg(&path)
        .output()
        .expect("ffprobe runs");
    let dec = Command::new("ffmpeg")
        .args([
            "-v",
            "error",
            "-hide_banner",
            "-err_detect",
            "explode+crccheck+bitstream+buffer",
            "-i",
        ])
        .arg(&path)
        .args([
            "-fps_mode",
            "passthrough",
            "-f",
            "rawvideo",
            "-pix_fmt",
            "yuv420p",
            "-",
        ])
        .output()
        .expect("ffmpeg runs");
    std::fs::remove_file(&path).ok();
    let s = String::from_utf8_lossy(&probe.stdout);
    assert!(
        probe.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    let profile_line = format!("profile={profile_name}\n");
    for want in [
        "codec_name=h264\n",
        profile_line.as_str(),
        "width=192",
        "height=108",
        "pix_fmt=yuv420p",
        "color_range=tv",
        "color_space=bt709",
        "color_transfer=bt709",
        "color_primaries=bt709",
        "r_frame_rate=30/1",
        "avg_frame_rate=30/1",
        "nb_frames=45",
        "duration=1.500000",
        "sample_aspect_ratio=1:1",
    ] {
        assert!(s.contains(want), "ffprobe lacks {want}:\n{s}");
    }
    let err = String::from_utf8_lossy(&dec.stderr);
    assert!(err.trim().is_empty(), "ffmpeg reported: {err}");
    assert_eq!(dec.stdout.len(), recon.len());
    assert!(
        dec.stdout == recon,
        "decoded pictures differ from the reconstruction"
    );
}
