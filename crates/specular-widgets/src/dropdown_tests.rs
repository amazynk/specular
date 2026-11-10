//! Settings dropdowns open on the frame of the click: the work of that frame stays small.

use crate::ctx::Target;
use crate::settings::SettingsTarget;
use crate::{Button, Chrome, HudStats, Input, Modal, Overlays, ViewerState};
use specular_gfx::{DrawList, Text};
use specular_input::{ActionState, ImageState, Keymap};
use specular_settings::{Settings, SettingsStore};
use specular_theme::Palette;
use std::time::{Duration, Instant};

fn with_state(f: impl FnOnce(&ViewerState)) {
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let st = ViewerState {
        actions: ActionState {
            image: ImageState::Ready,
            count: 4,
            ..ActionState::default()
        },
        has_image: true,
        index: 1,
        count: 4,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: Some("beach.jpg"),
        dims: Some((4000, 3000)),
        loading: false,
        error: None,
        truncated: false,
        metadata: None,
        hud: &hud,
        keymap: &keymap,
        store: &store,
        settings: &settings,
        overlays: Overlays::default(),
        playback: None,
        motion: None,
        spread: None,
        pages: None,
    };
    f(&st)
}

struct Harness {
    c: Chrome,
    text: Text,
    dl: DrawList,
    now: f64,
}

impl Harness {
    fn new(text: Text) -> Harness {
        let mut c = Chrome::new();
        c.set_size(1000.0, 700.0);
        c.set_accessibility(true);
        c.poke(1.0);
        Harness {
            c,
            text,
            dl: DrawList::new(1.0),
            now: 1.0,
        }
    }

    /// One frame as the app draws it (text frame, chrome, accessibility tree when on).
    fn frame(&mut self, st: &ViewerState) {
        self.now += 0.016;
        self.text.begin_frame(1.0);
        self.dl.clear(1.0);
        self.c
            .draw(&mut self.dl, &mut self.text, st, &Palette::dark(), self.now);
        self.text.end_frame();
        let _ = self.c.accessibility_tree(st, "Specular", 1.0);
    }

    fn rect_of(&self, t: &Target) -> Option<crate::Rect> {
        self.c
            .hits
            .iter()
            .rev()
            .find(|(_, h)| h == t)
            .map(|(r, _)| *r)
    }

    fn pointer(&mut self, st: &ViewerState, (x, y): (f32, f32), down: bool) {
        self.c.input(
            &Input::PointerButton {
                x,
                y,
                down,
                button: Button::Primary,
            },
            st,
            self.now,
        );
    }

    /// The dropdown of `k` on the current page, opened by a pointer click; returns the time
    /// from the press to the end of the frame that shows the list.
    fn open(&mut self, st: &ViewerState, k: &'static str) -> Duration {
        let t = Target::Settings(SettingsTarget::Dropdown(k));
        let r = self.rect_of(&t).unwrap_or_else(|| panic!("{k} on screen"));
        let p = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        let t0 = Instant::now();
        self.pointer(st, p, true);
        self.pointer(st, p, false);
        self.frame(st);
        let d = t0.elapsed();
        assert_eq!(self.c.settings.dropdown, Some(k), "{k} open");
        d
    }

    fn close(&mut self, st: &ViewerState) {
        self.c.close_dropdown();
        self.frame(st);
        self.frame(st);
    }

    /// Settings open on section `i` and drawn (`frames` frames).
    fn settings(&mut self, st: &ViewerState, i: usize, frames: usize) {
        self.c.open_modal(Modal::Settings);
        self.frame(st);
        let r = self
            .rect_of(&Target::Settings(SettingsTarget::Section(i)))
            .expect("section in the sidebar");
        let p = (r.x + r.w * 0.5, r.y + r.h * 0.5);
        self.pointer(st, p, true);
        self.pointer(st, p, false);
        for _ in 0..frames {
            self.frame(st);
        }
    }
}

const GENERAL: [&str; 4] = [
    "general.language",
    "general.region_formats",
    "general.theme",
    "general.on_start",
];

/// A text system as the app has it a moment after start: system fonts in, the UI locale
/// set, the warm-up of the language names done.
fn app_text() -> Text {
    let mut text = Text::new();
    text.set_locale(Some("en".into()));
    text.set_warm_up(std::sync::Arc::new(crate::warm_up_texts));
    let t0 = Instant::now();
    while (text.fonts_pending() || text.warming_up()) && t0.elapsed() < Duration::from_secs(30) {
        std::thread::sleep(Duration::from_millis(5));
        text.begin_frame(1.0);
    }
    text
}

/// Not a check: prints the time from a click to the end of the frame that shows each
/// General dropdown, on its first open (three new text systems) and again. `cargo test -p
/// specular-widgets dropdown_open_times -- --ignored --nocapture`.
#[test]
#[ignore]
fn dropdown_open_times() {
    with_state(|st| {
        for &k in &GENERAL {
            let mut first = Vec::new();
            let mut again = Duration::MAX;
            for _ in 0..3 {
                // A new text system with the Settings page drawn a few frames.
                let mut h = Harness::new(app_text());
                h.settings(st, 0, 3);
                first.push(h.open(st, k));
                h.close(st);
                for _ in 0..3 {
                    again = again.min(h.open(st, k));
                    h.close(st);
                }
            }
            let ms = |d: &Duration| format!("{:.2}", d.as_secs_f64() * 1e3);
            eprintln!(
                "{k}: first open {} ms, again {} ms",
                first.iter().map(ms).collect::<Vec<_>>().join(" / "),
                ms(&again)
            );
        }
    });
}

/// The language dropdown, the first time it opens, is laid out in the frame of the click,
/// and that frame neither finds nor loads fonts: the languages' names (a script each) were
/// shaped ahead on another thread. Loading a CJK fallback font in that frame took 120-135
/// ms; now it takes a few. The best of three, and not judged when the machine is
/// overloaded (`specular_log::timing`).
#[test]
fn language_dropdown_opens_at_once() {
    let k = "general.language";
    with_state(|st| {
        let mut best = Duration::MAX;
        for _ in 0..3 {
            let mut h = Harness::new(app_text());
            h.settings(st, 0, 3);
            best = best.min(h.open(st, k));
            // Laid out in the click's frame: its options are there to point at.
            let options = h
                .c
                .hits
                .iter()
                .filter(|(_, t)| {
                    matches!(t, Target::Settings(SettingsTarget::Choose(kk, _)) if *kk == k)
                })
                .count();
            assert!(options > 5, "{options} options laid out");
        }
        specular_log::timing::assert_within(
            "first open of the language dropdown",
            best,
            Duration::from_millis(40),
        );
    });
}
