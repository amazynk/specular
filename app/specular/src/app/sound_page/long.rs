//! Sound longer than [`EXACT_MAX_US`], made at test time (no big fixture): the page shows
//! its sampled analysis first and says the loudness is being measured; the whole-file
//! loudness follows and joins the numbers and the Info panel. An ignored check times the
//! page on a 4.4-hour sound: first draw, seeks, wheel zoom to the deepest span, and a long
//! synced lyrics column.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use specular_core::Source;
use specular_decode::{Format, Session};
use specular_theme::{Mode, Palette};

use super::*;

/// A mono 16-bit WAVE of `secs` seconds at `rate`: a 440 Hz tone at −20 dBFS with a quieter
/// 3 kHz one every other 10 s.
fn long_wave(secs: u64, rate: u32) -> Vec<u8> {
    let n = secs as usize * rate as usize;
    let data = (n * 2) as u32;
    let mut b = Vec::with_capacity(n * 2 + 44);
    b.extend_from_slice(b"RIFF");
    b.extend_from_slice(&(36 + data).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt ");
    b.extend_from_slice(&16u32.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&1u16.to_le_bytes());
    b.extend_from_slice(&rate.to_le_bytes());
    b.extend_from_slice(&(rate * 2).to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&data.to_le_bytes());
    let w = |f: f64| 2.0 * std::f64::consts::PI * f / f64::from(rate);
    let (w1, w2) = (w(440.0), w(3_000.0));
    for i in 0..n {
        let t = i as f64;
        let high = (i / (rate as usize * 10)) % 2 == 1;
        let v = 0.1 * (w1 * t).sin() + if high { 0.03 * (w2 * t).sin() } else { 0.0 };
        b.extend_from_slice(&((v * 32767.0).round() as i16).to_le_bytes());
    }
    b
}

/// The page of a made-up sound of `secs` seconds, entered; the channel its worker wakes.
fn entered(secs: u64, rate: u32) -> (SoundPage, mpsc::Receiver<()>) {
    let src = Arc::new(Source::from_vec("long.wav", long_wave(secs, rate)));
    let s: Arc<dyn Session> = specular_video::session::open_any(src, Format::Wave).unwrap();
    let mut page = SoundPage::default();
    let rx = enter(&mut page, &s);
    (page, rx)
}

/// Enters `page` for `s`; the channel its worker wakes.
fn enter(page: &mut SoundPage, s: &Arc<dyn Session>) -> mpsc::Receiver<()> {
    let (tx, rx) = mpsc::channel();
    let wake: crate::loader::Wake = Arc::new(move || {
        let _ = tx.send(());
    });
    page.enter(1, s, wake);
    rx
}

/// Waits for the worker's next message.
fn next(page: &mut SoundPage, rx: &mpsc::Receiver<()>) {
    loop {
        rx.recv_timeout(Duration::from_secs(900))
            .expect("the sound page's worker");
        if page.poll() {
            return;
        }
    }
}

#[test]
fn a_long_sound_shows_before_its_loudness() {
    // A minute past the sampling limit, at 8 kHz (30 MB).
    let secs = EXACT_MAX_US / 1_000_000 + 60;
    let t0 = Instant::now();
    let (mut page, rx) = entered(secs, 8_000);
    assert!(!page.done, "reading");
    next(&mut page, &rx);
    let first = t0.elapsed();
    // The page left "Reading the sound…" with the sampled analysis, before any loudness.
    assert!(page.done);
    let a = page.analysis.as_ref().expect("an analysis");
    assert!(!a.exact, "sampled");
    assert!((a.duration_s - secs as f64).abs() < 1.0);
    assert!(page.loudness_of(1).is_none());
    assert!(page.measuring);
    let rows = numbers(&page);
    assert!(
        rows.iter()
            .any(|(l, v)| *l == t!("sound-loudness-measuring") && v.is_empty()),
        "{rows:?}"
    );
    assert!(!rows.iter().any(|(l, _)| *l == t!("sound-integrated")));
    // Then the loudness: the numbers and Info take it.
    next(&mut page, &rx);
    let second = t0.elapsed();
    assert!(!page.measuring);
    let l = page.loudness_of(1).expect("the whole-file loudness");
    let i = l.integrated.expect("integrated loudness");
    assert!((-27.0..-20.0).contains(&i), "{i}");
    let info = info_items(l);
    assert!(info.iter().any(|r| r.key == "sound-loudness"), "{info:?}");
    assert!(
        numbers(&page)
            .iter()
            .any(|(l, _)| *l == t!("sound-integrated"))
    );
    eprintln!(
        "{} min sound: analysis after {:.1} s, loudness after {:.1} s",
        secs / 60,
        first.as_secs_f64(),
        second.as_secs_f64()
    );
    // Zoomed past its 4096 sampled columns: the part in view is analysed again.
    let min = page.min_span();
    assert!((min * secs as f64 - DETAIL_MIN_SECS).abs() < 1e-3);
    page.view.zoom(1000.0, 0.5, min);
    let v = page.view;
    page.want_detail(1000.0);
    let t1 = Instant::now();
    while !page.poll_detail() {
        rx.recv_timeout(Duration::from_secs(300))
            .expect("the detail");
    }
    let d = page.detail.as_ref().expect("a detail");
    assert!(d.from <= v.start && v.start + v.span <= d.from + d.span);
    assert!(d.analysis.cols as f64 * v.span / d.span >= 500.0);
    // The tone's peak (0.1, or 0.13 with the 3 kHz one) in every column.
    assert!(
        d.analysis.wave.iter().all(|w| (0.09..0.14).contains(&w[1])),
        "{:?}",
        &d.analysis.wave[..4]
    );
    eprintln!(
        "detail of {:.1} s in {} columns after {:.2} s",
        d.span * secs as f64,
        d.analysis.cols,
        t1.elapsed().as_secs_f64()
    );
    // Held: nothing more is asked for.
    page.want_detail(1000.0);
    assert!(page.detail_job.is_none());
}

/// One frame of the page at `px` physical size: drawn twice (uploads, then the GPU panels)
/// and rendered. Its time.
fn frame(h: &mut specular_shots::Harness, page: &mut SoundPage, size: (f32, f32)) -> Duration {
    frame_on(&h.gpu, &mut h.ui, &mut h.text, page, size)
}

fn frame_on(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    text: &mut Text,
    page: &mut SoundPage,
    size: (f32, f32),
) -> Duration {
    let pal = Palette::for_mode(Mode::Dark);
    let area = Rect::new(0.0, 0.0, size.0, size.1);
    let px = (size.0 as u32, size.1 as u32);
    let t = Instant::now();
    let mut dl = DrawList::new(1.0);
    draw(&mut dl, text, &pal, page, area, false, 1.0, 1.0);
    page.prepare(gpu, ui, px);
    let mut dl = DrawList::new(1.0);
    draw(&mut dl, text, &pal, page, area, false, 1.0, 1.0);
    let mut enc = gpu.device.create_command_encoder(&Default::default());
    page.render(gpu, &mut enc);
    gpu.queue.submit([enc.finish()]);
    let _ = gpu.wait();
    t.elapsed()
}

/// The page drawn on a device that is then lost (destroyed, as a driver reset leaves it)
/// is made again on a new one: nothing of the old device is used.
#[test]
fn the_page_is_made_again_on_a_new_device() {
    let Some(mut h) = specular_shots::Harness::new() else {
        return;
    };
    let old = Gpu::new().expect("a second device");
    let comp = specular_gfx::Compositor::new(&old, specular_gfx::HDR, 16, 640, 400);
    let mut old_ui = UiRenderer::new(&old, &comp);
    let size = (640.0, 400.0);
    let src = Arc::new(Source::from_vec("short.wav", long_wave(20, 8_000)));
    let s: Arc<dyn Session> = specular_video::session::open_any(src, Format::Wave).unwrap();
    let mut page = SoundPage::default();
    let rx = enter(&mut page, &s);
    next(&mut page, &rx);
    frame_on(&old, &mut old_ui, &mut h.text, &mut page, size);
    let before = page.tex.expect("the panels' texture");
    assert!(page.gpu.is_some());
    old.device.destroy();
    let _ = old.wait();
    page.gpu_reset();
    assert!(page.gpu.is_none() && page.tex.is_none() && !page.active());
    // Entered again on the next frame (the session kept its analysis).
    let rx = enter(&mut page, &s);
    next(&mut page, &rx);
    frame(&mut h, &mut page, size);
    let after = page.tex.expect("made again");
    assert_ne!(before, after);
}

/// The 4.4-hour case of the user's report, timed: first draw, seeks, wheel zoom to the
/// deepest span and synced lyrics scrolling. `cargo test -p specular --bin specular
/// sound_page::long -- --ignored --nocapture`
#[test]
#[ignore = "a 4.4-hour sound (127 MB in memory); run by hand"]
fn a_multi_hour_sound_page_is_quick() {
    let Some(mut h) = specular_shots::Harness::new() else {
        return;
    };
    let secs = 4 * 3600 + 24 * 60;
    let t0 = Instant::now();
    let (mut page, rx) = entered(secs, 4_000);
    let made = t0.elapsed();
    next(&mut page, &rx);
    let analysed = t0.elapsed();
    let size = (1200.0, 700.0);
    page.tick(0.0, false, Instant::now(), false);
    let first = frame(&mut h, &mut page, size);
    eprintln!(
        "4.4 h: made {:.1} s, analysis after {:.1} s, first draw {:.1} ms (time to first draw {:.1} s)",
        made.as_secs_f64(),
        (analysed - made).as_secs_f64(),
        first.as_secs_f64() * 1e3,
        (analysed - made + first).as_secs_f64()
    );
    // Seeks across the whole length, as clicks would.
    let mut worst = Duration::ZERO;
    for k in 0..20 {
        let t = (k as f64 * 0.37).fract() * secs as f64;
        page.tick(t, false, Instant::now(), false);
        worst = worst.max(frame(&mut h, &mut page, size));
    }
    eprintln!("seeks: worst frame {:.1} ms", worst.as_secs_f64() * 1e3);
    // The wheel down to the deepest span, then out again.
    let min = page.min_span();
    let mut worst = Duration::ZERO;
    let mut steps = 0;
    while page.view.span > min * 1.0001 && steps < 200 {
        page.view.zoom(1.25, 0.5, min);
        worst = worst.max(frame(&mut h, &mut page, size));
        steps += 1;
    }
    eprintln!(
        "zoom in: {steps} steps to {:.1} s shown, worst frame {:.1} ms",
        page.view.span * secs as f64,
        worst.as_secs_f64() * 1e3
    );
    // Synced lyrics, a line every 10 s, scrolled through while playing.
    let mut lrc = String::new();
    for k in 0..secs / 10 {
        let t = k * 10;
        lrc.push_str(&format!("[{:02}:{:02}.00]Line {k}\n", t / 60, t % 60));
    }
    page.lyrics = Lyrics::from_text(&lrc, specular_video::lyrics::From::Sidecar).map(Arc::new);
    page.set_looks(false, false, true);
    let mut worst = Duration::ZERO;
    let at = Instant::now();
    for k in 0..60 {
        let t = 3600.0 + f64::from(k) * 0.5;
        page.tick(t, true, at + Duration::from_millis(500 * k as u64), false);
        worst = worst.max(frame(&mut h, &mut page, size));
    }
    eprintln!(
        "lyrics ({} lines): worst frame {:.1} ms",
        secs / 10,
        worst.as_secs_f64() * 1e3
    );
}
