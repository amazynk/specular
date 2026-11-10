//! Screenshot tests of the sound page: a made-up 12 s stereo sound (tones and noise in
//! steps of level, clipping near the end) opened as a WAVE file, at its overview (with a
//! cover), its meters, everything with the spectrum as a curve and the time zoomed in, at 1×
//! and 1.5×, dark in English and light in Hebrew (right to left). The "looks" cases play it
//! through a live tap (the goniometer fills), with synced lyrics, the cover's colours and
//! the blurred cover behind. Rendered offscreen on lavapipe and compared with the PNGs in
//! `app/specular/tests/shots/`.
//!
//! To accept an intended change:
//! `SPECULAR_BLESS=1 cargo test -p specular --bin specular sound_page::shots`, then review
//! and commit the rewritten PNGs.

use std::sync::atomic::AtomicI64;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_decode::Format;
use specular_shots::{Goldens, Harness};
use specular_theme::{Mode, Palette};

use super::*;

const SIZE: (f32, f32) = (900.0, 560.0);
const RATE: u32 = 48_000;
const SECS: f64 = 12.0;

/// The sound as a 16-bit WAVE file.
fn wave() -> Vec<u8> {
    let n = (SECS * f64::from(RATE)) as usize;
    let mut pcm = Vec::with_capacity(n * 2);
    let mut seed = 1u32;
    let mut noise = move || {
        seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        f64::from(seed >> 8) / f64::from(1u32 << 24) * 2.0 - 1.0
    };
    for i in 0..n {
        let t = i as f64 / f64::from(RATE);
        // Level steps every 2 s, a fade over each step.
        let step = (t / 2.0).floor();
        let db = [-30.0, -18.0, -12.0, -24.0, -8.0, -3.0][step as usize % 6];
        let env =
            10f64.powf(db / 20.0) * (0.6 + 0.4 * (std::f64::consts::PI * (t % 2.0) / 2.0).sin());
        let tone = |f: f64| (2.0 * std::f64::consts::PI * f * t).sin();
        let l =
            env * (0.5 * tone(220.0) + 0.3 * tone(660.0) + 0.15 * tone(3_520.0) + 0.1 * noise());
        let r =
            env * (0.5 * tone(330.0) + 0.25 * tone(1_320.0) + 0.1 * tone(7_040.0) + 0.1 * noise());
        // Clipped at the end of the loudest step.
        let clip = |v: f64| {
            if t > 11.0 {
                (v * 3.0).clamp(-1.0, 1.0)
            } else {
                v
            }
        };
        for v in [clip(l), clip(r)] {
            pcm.push((v * 32767.0).round().clamp(-32768.0, 32767.0) as i16);
        }
    }
    let data = (pcm.len() * 2) as u32;
    let mut b = Vec::with_capacity(data as usize + 44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&RATE.to_le_bytes());
    b.extend_from_slice(&(RATE * 4).to_le_bytes());
    b.extend_from_slice(&4u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    for v in pcm {
        b.extend_from_slice(&v.to_le_bytes());
    }
    b
}

/// A cover: a soft diagonal gradient.
fn cover() -> (u32, u32, Vec<u8>) {
    let (w, h) = (96u32, 96u32);
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let t = (x + y) as f32 / (w + h) as f32;
            px.extend([
                (40.0 + 160.0 * t) as u8,
                (90.0 + 60.0 * (1.0 - t)) as u8,
                150,
                255,
            ]);
        }
    }
    (w, h, px)
}

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    visuals: &'static str,
    scale: f32,
    dark: bool,
    rtl: bool,
    cover: bool,
    /// Zoom in time: factor, around this fraction of the plot.
    zoom: Option<(f64, f64)>,
    /// Played through a live tap, with lyrics, cover colours and the backdrop.
    looks: bool,
    /// What has the keyboard (its focus ring shows).
    focus: Option<Focus>,
}

/// Synced lyrics (enhanced LRC on the line sung at 9.1 s).
const LRC_EN: &str = "[00:00.50]Low hum of the morning\n[00:02.00]Steps of light on the floor\n\
    [00:04.00]Counting tones in the hallway\n[00:06.00]Every one a little more\n\
    [00:07.50]\n[00:09.00]<00:09.00>Sing <00:09.40>along <00:09.80>with <00:10.20>me <00:10.80>\n\
    [00:11.00]Until the meters glow\n[00:12.00]End";
const LRC_HE: &str = "[00:00.50]בוקר שקט בעיר\n[00:02.00]אור על הרצפה\n\
    [00:04.00]צלילים במסדרון\n[00:06.00]עוד אחד ועוד אחד\n\
    [00:09.00]<00:09.00>שירו <00:09.40>איתי <00:10.20>עכשיו <00:10.80>\n[00:11.00]עד שהמחוגים זוהרים";

/// The sound's samples (interleaved stereo, as the WAVE holds them).
fn pcm() -> Vec<f32> {
    let w = wave();
    w[44..]
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
        .collect()
}

fn shoot(
    h: &mut Harness,
    goldens: &mut Goldens,
    data: &(Arc<Analysis>, Option<Arc<Report>>),
    c: &Case,
) {
    let pal = Palette::for_mode(if c.dark { Mode::Dark } else { Mode::Light });
    let mut page = SoundPage {
        id: Some(1),
        focus: c.focus,
        ..SoundPage::default()
    };
    page.set_data(Some(Arc::clone(&data.0)), data.1.clone());
    page.visuals = Visuals::parse(c.visuals);
    if c.cover {
        page.cover = Some(cover());
    }
    if let Some((f, x)) = c.zoom {
        page.view.zoom(f, x, 0.0);
    }
    let tap = Arc::new(Tap::new());
    let samples = pcm();
    if c.looks {
        page.set_looks(true, true, true);
        let text = if c.rtl { LRC_HE } else { LRC_EN };
        page.lyrics = Lyrics::from_text(text, specular_video::lyrics::From::Sidecar).map(Arc::new);
        page.set_tap(Some((Arc::clone(&tap), Arc::new(AtomicI64::new(0)))));
    }
    // Play from 6.0 s to 9.1 s: the bars and peaks have moved, the clip has not come. With
    // the tap, each 100 ms is heard before the frame that shows it.
    let hear = |t: f64| {
        let (a, b) = (
            ((t - 0.1) * f64::from(RATE)) as usize,
            (t * f64::from(RATE)) as usize,
        );
        tap.write(
            &samples[a * 2..b * 2],
            2,
            RATE,
            ((t - 0.1) * 1e9) as i64,
            1.0,
        );
    };
    let t0 = Instant::now();
    page.tick(6.0, false, t0, false);
    for k in 1..=31 {
        let t = 6.0 + k as f64 * 0.1;
        if c.looks {
            hear(t);
        }
        page.tick(t, true, t0 + Duration::from_millis(100 * k), false);
    }
    // The second half of the last step clipped (seen by the meters at its time).
    if c.name.contains("meters") {
        page.tick(11.5, false, t0 + Duration::from_secs(5), false);
    }
    let area = Rect::new(0.0, 0.0, SIZE.0, SIZE.1);
    let px = (
        (SIZE.0 * c.scale).round() as u32,
        (SIZE.1 * c.scale).round() as u32,
    );
    // A first draw sets what the uploads depend on (the goniometer's panel colour).
    let mut dl = DrawList::new(c.scale);
    draw(
        &mut dl,
        &mut h.text,
        &pal,
        &mut page,
        area,
        c.rtl,
        1.0,
        c.scale,
    );
    page.prepare(&h.gpu, &mut h.ui, px);
    // The GPU panels: queued by drawing, rendered before the UI pass.
    let mut dl = DrawList::new(c.scale);
    draw(
        &mut dl,
        &mut h.text,
        &pal,
        &mut page,
        area,
        c.rtl,
        1.0,
        c.scale,
    );
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    page.render(&h.gpu, &mut enc);
    h.gpu.queue.submit([enc.finish()]);
    let rtl = c.rtl;
    let scale = c.scale;
    let shot = h.render(
        SIZE,
        c.scale,
        |_| {},
        |dl, text| draw(dl, text, &pal, &mut page, area, rtl, 1.0, scale),
    );
    goldens.check(&format!("{}-{}x", c.name, c.scale), &shot);
    super::super::a11y_audit::audit_screen(c.name, SIZE, rtl, |t| a11y(t, &page, 1.0));
    // Nothing left queued.
    let mut enc = h.gpu.device.create_command_encoder(&Default::default());
    page.render(&h.gpu, &mut enc);
    h.gpu.queue.submit([enc.finish()]);
    page.leave();
    page.prepare(&h.gpu, &mut h.ui, px);
}

fn sound() -> (Arc<Analysis>, Option<Arc<Report>>) {
    let src = Arc::new(Source::from_vec("shots.wav", wave()));
    let s = specular_video::session::open_any(src, Format::Wave).unwrap();
    let snd = s
        .as_any()
        .and_then(|a| a.downcast_ref::<SoundSession>())
        .unwrap();
    let cancel = CancelToken::new();
    let a = snd.analysis(&cancel).unwrap().unwrap();
    let l = snd.loudness(&cancel).unwrap();
    (a, l)
}

#[test]
fn sound_page_matches_goldens() {
    let Some(mut h) = Harness::new() else { return };
    let data = sound();
    let l = data.1.as_ref().unwrap();
    // The made-up sound's measurement, as the numbers show it.
    assert!(
        l.integrated.is_some_and(|i| (-25.0..-5.0).contains(&i)),
        "{l:?}"
    );
    assert!(l.max_true_peak_db() > 0.0);
    let mut goldens = Goldens::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/tests/shots"),
        "specular-sound",
    );
    let base = Case {
        name: "",
        visuals: "waveform,spectrogram",
        scale: 1.0,
        dark: true,
        rtl: false,
        cover: false,
        zoom: None,
        looks: false,
        focus: None,
    };
    let en = [
        Case {
            name: "sound-overview",
            cover: true,
            ..base
        },
        Case {
            name: "sound-meters",
            visuals: "spectrum,meters,loudness",
            ..base
        },
        Case {
            name: "sound-all-curve-zoomed",
            visuals: "waveform,spectrogram,spectrum,meters,loudness,curve",
            scale: 1.5,
            zoom: Some((3.0, 0.5)),
            ..base
        },
        Case {
            name: "sound-looks",
            visuals: "waveform,spectrum,meters,stereo",
            cover: true,
            looks: true,
            ..base
        },
        Case {
            name: "sound-looks-meters",
            visuals: "spectrum,meters,loudness,stereo",
            scale: 1.5,
            cover: true,
            looks: true,
            ..base
        },
        // The keyboard on a choice: its focus ring.
        Case {
            name: "sound-focus-chip",
            visuals: "spectrum,meters,loudness",
            focus: Some(Focus::Chip(1)),
            ..base
        },
        // Spectrum, stereo and lyrics side by side: the spectrum's labels thin out.
        Case {
            name: "sound-spectrum-lyrics",
            visuals: "spectrum,meters,stereo",
            looks: true,
            ..base
        },
        Case {
            name: "sound-spectrum-lyrics",
            visuals: "spectrum,meters,stereo",
            scale: 1.5,
            looks: true,
            ..base
        },
    ];
    for c in &en {
        shoot(&mut h, &mut goldens, &data, c);
    }
    specular_i18n::I18n::with_thread_language(specular_i18n::langid!("he"), || {
        let he = [
            Case {
                name: "sound-all-he",
                visuals: "waveform,spectrogram,spectrum,meters,loudness",
                dark: false,
                rtl: true,
                cover: true,
                ..base
            },
            // The keyboard on a line of the lyrics, right to left.
            Case {
                name: "sound-focus-lyrics-he",
                visuals: "spectrum,meters,stereo",
                dark: false,
                rtl: true,
                looks: true,
                focus: Some(Focus::Line(1)),
                ..base
            },
            Case {
                name: "sound-overview-he",
                dark: false,
                rtl: true,
                scale: 1.5,
                ..base
            },
            Case {
                name: "sound-looks-he",
                visuals: "waveform,spectrum,meters,stereo",
                dark: false,
                rtl: true,
                cover: true,
                looks: true,
                ..base
            },
            Case {
                name: "sound-looks-meters-he",
                visuals: "spectrum,meters,loudness,stereo",
                dark: false,
                rtl: true,
                scale: 1.5,
                cover: true,
                looks: true,
                ..base
            },
            Case {
                name: "sound-spectrum-lyrics-he",
                visuals: "spectrum,meters,stereo",
                dark: false,
                rtl: true,
                looks: true,
                ..base
            },
            Case {
                name: "sound-spectrum-lyrics-he",
                visuals: "spectrum,meters,stereo",
                dark: false,
                rtl: true,
                scale: 1.5,
                looks: true,
                ..base
            },
        ];
        for c in &he {
            shoot(&mut h, &mut goldens, &data, c);
        }
    });
    goldens.finish();
}
