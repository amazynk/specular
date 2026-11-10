//! A movie's player controls beyond the transport's basics: the subtitle (CC), sound track,
//! chapter and More menus over their transport buttons, the speed menu over the speed, the
//! picture adjustments (More › Picture…), chapter and A–B marks on the seek bar, and the
//! time and preview frame shown while the pointer is over the seek bar.
//!
//! The application describes what it has in a [`PlayerView`] ([`crate::Chrome::set_player`]);
//! choices come back as [`Command::Player`](crate::Command::Player).

use specular_gfx::{Icon, Rect, TexId, draw_icon};
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::transport::clock;

/// One of the player's menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PlayerMenu {
    Subtitles,
    Sound,
    Chapters,
    /// The playback speeds, over the speed shown on the bar.
    Speed,
    /// Go to time…, Picture…, rotate, flip and fit.
    More,
    /// The picture adjustments (brightness, contrast, saturation, gamma, hue).
    Picture,
    /// The sound's output device and speakers (over Sound, which offers them).
    Output,
    /// "Play on…": this device and the TVs and speakers found on the network.
    Cast,
}

/// A choice made in the player's menus.
#[derive(Clone, Debug, PartialEq)]
pub enum PlayerCmd {
    /// A subtitle track by position in [`PlayerView::subtitles`], or off.
    Subtitle(Option<usize>),
    /// "Open subtitle file…".
    OpenSubtitle,
    /// "Generate subtitles…" (speech to text), or stop the generating.
    GenerateSubtitles,
    /// "Save subtitles…": the generated subtitles as a file.
    SaveSubtitles,
    /// The next character set for a text subtitle file.
    SubtitleCharset,
    /// "Subtitle style…": Settings at the subtitles' look.
    SubtitleStyle,
    /// Subtitles later (positive) or earlier, in milliseconds; 0 resets.
    SubtitleDelay(i64),
    /// A sound track by position in [`PlayerView::sounds`].
    Sound(usize),
    /// Sound later (positive) or earlier, in milliseconds; 0 resets.
    SoundDelay(i64),
    /// Dialogue boost on or off.
    DialogueBoost,
    /// Mono sound on or off.
    Mono,
    /// Opens the equaliser's panel.
    Equaliser,
    /// A chapter by position.
    Chapter(usize),
    /// A playback speed (one of [`SPEEDS`]).
    Speed(f64),
    /// An action of the viewer's (rotate, flip, fit), as its key does.
    Action(Action),
    /// "Go to time…": the field over the time (the chrome opens it).
    GoToTime,
    /// "Picture…": the picture adjustments (the chrome opens them).
    OpenPicture,
    /// A picture adjustment ([`PICTURE`] by position) a step up (1) or down (−1); 0 resets it.
    Picture(usize, i64),
    /// Every picture adjustment back to neutral.
    PictureReset,
    /// "Output…": the output device and speakers (the chrome opens them; the
    /// application lists the devices again).
    OpenOutput,
    /// An output device by position in [`PlayerView::outputs`] (0: the system's default).
    Output(usize),
    /// The speakers by position in [`PlayerView::speakers`].
    Speakers(usize),
    /// "Play on…": the devices to play on (the chrome opens the menu; the application
    /// looks for devices while it is open).
    OpenCast,
    /// Play on device `i` of [`CastView::devices`].
    CastTo(usize),
    /// Back to this device (stop casting).
    CastStop,
    /// Look for devices again.
    CastSearch,
    /// A row that only says something (why the subtitles stay here); does nothing.
    CastNote,
    /// A network stream's quality by position in [`PlayerView::qualities`], or automatic.
    Quality(Option<usize>),
}

/// Playing on another device: what the "Play on…" menu and the "Playing on" card show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CastView {
    /// The devices found, by their own names.
    pub devices: Vec<String>,
    /// Looking for devices now.
    pub searching: bool,
    /// The network is off: nothing is looked for.
    pub network_off: bool,
    /// The device playing, by position in `devices`, and its name (it stays named when it is
    /// no longer in the list).
    pub active: Option<usize>,
    pub playing_on: Option<String>,
    /// What the card says under the name: "Connecting…", "Paused", why it stopped.
    pub status: Option<String>,
    /// Why the chosen subtitles are not sent to devices, in words (picture subtitles).
    pub subtitles_note: Option<String>,
}

/// The delay step of the menus' − and + (milliseconds).
pub const DELAY_STEP_MS: i64 = 50;

/// The speed menu's choices.
pub const SPEEDS: [f64; 6] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

/// A picture adjustment: its name's message, its neutral value, its range and step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Knob {
    pub name: &'static str,
    pub neutral: f32,
    pub min: f32,
    pub max: f32,
    pub step: f32,
}

/// Brightness, contrast, saturation, gamma and hue, in [`PlayerView::picture`]'s order.
pub const PICTURE: [Knob; 5] = [
    Knob {
        name: "player-picture-brightness",
        neutral: 0.0,
        min: -1.0,
        max: 1.0,
        step: 0.05,
    },
    Knob {
        name: "player-picture-contrast",
        neutral: 1.0,
        min: 0.0,
        max: 2.0,
        step: 0.05,
    },
    Knob {
        name: "player-picture-saturation",
        neutral: 1.0,
        min: 0.0,
        max: 2.0,
        step: 0.1,
    },
    Knob {
        name: "player-picture-gamma",
        neutral: 1.0,
        min: 0.25,
        max: 4.0,
        step: 0.1,
    },
    Knob {
        name: "player-picture-hue",
        neutral: 0.0,
        min: -180.0,
        max: 180.0,
        step: 10.0,
    },
];

impl Knob {
    /// `v` moved `step` steps (0: back to neutral), within the range.
    pub fn stepped(&self, v: f32, step: i64) -> f32 {
        if step == 0 {
            return self.neutral;
        }
        let n = ((v - self.neutral) / self.step).round() + step as f32;
        (self.neutral + n * self.step).clamp(self.min, self.max)
    }

    /// The name, in the UI language.
    pub fn label(&self) -> String {
        I18n::global().tr(self.name)
    }
}

/// A picture value as shown: brightness, contrast and saturation as a signed percentage off
/// neutral ("+15 %"), gamma as a number ("1.2"), hue in degrees ("−30°").
pub fn picture_text(knob: usize, v: f32) -> String {
    let l = I18n::global().localizer();
    let signed = |x: f64| {
        let n = l.number(x.abs(), 0);
        if x > 1e-9 {
            format!("+{n}")
        } else if x < -1e-9 {
            format!("\u{2212}{n}")
        } else {
            n
        }
    };
    let off = f64::from(v - PICTURE[knob].neutral);
    // The sign and the unit stay with the number in right-to-left text.
    let text = match knob {
        3 => l.number(f64::from(v), 2),
        4 => t!("player-picture-degrees", value = signed(off.round())),
        _ => t!(
            "player-picture-percent",
            value = signed((off * 100.0).round())
        ),
    };
    specular_i18n::isolate_ltr(&text)
}

/// A row with −, a value (which resets it) and +: a delay, or a picture adjustment.
pub(crate) struct Stepper {
    /// "Subtitle delay: +50 ms", "Contrast: +10 %".
    pub text: String,
    /// The value alone (accessibility).
    pub value: String,
    /// What −, the reset and + do (accessibility).
    pub less: String,
    pub reset: String,
    pub more: String,
}

/// What the player offers, from the application.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlayerView {
    /// Subtitle tracks' labels ("English", "Hebrew (file)").
    pub subtitles: Vec<String>,
    pub subtitle: Option<usize>,
    pub subtitle_delay_ms: i64,
    /// The character set the chosen text subtitle file was read in.
    pub charset: Option<String>,
    /// Sound tracks' labels (more than one: the menu offers a choice).
    pub sounds: Vec<String>,
    pub sound: usize,
    pub sound_delay_ms: i64,
    /// Chapters: start as a fraction of the movie, start in seconds, name.
    pub chapters: Vec<(f64, f64, String)>,
    /// A–B repeat points as fractions of the movie.
    pub ab: (Option<f64>, Option<f64>),
    /// The frame for the seek bar's hover: its texture, size and the fraction it shows.
    pub preview: Option<(TexId, u32, u32, f64)>,
    /// The movie has sound (the sound menu shows, for its delay at least).
    pub has_sound: bool,
    /// Dialogue boost and mono are on (their rows are ticked).
    pub dialogue: bool,
    pub mono: bool,
    /// Mini player mode is on (its button is ticked).
    pub mini: bool,
    /// The subtitle menu's speech-to-text row ("Generate subtitles…", or the progress of
    /// the subtitles being generated); `None`: not offered.
    pub generate: Option<String>,
    /// The chosen subtitles were generated and can be saved as a file.
    pub save_subtitles: bool,
    /// The playback speed (the speed menu ticks it).
    pub speed: f64,
    /// The movie has a picture: More offers Picture…, rotate, flip and fit.
    pub picture_menu: bool,
    /// The picture adjustments ([`PICTURE`]'s order).
    pub picture: [f32; 5],
    /// The picture is zoomed or panned (More offers to fit it again).
    pub zoomed: bool,
    /// The output devices ("System default" first) and the one chosen.
    pub outputs: Vec<String>,
    pub output: usize,
    /// The speaker settings (automatic, stereo, 5.1, 7.1) and the one chosen.
    pub speakers: Vec<String>,
    pub speaker: usize,
    /// Playing on another device ("Play on…").
    pub cast: CastView,
    /// A network stream's qualities, lowest first (more than one: More offers them).
    pub qualities: Vec<String>,
    /// The quality the user chose (`None`: automatic).
    pub quality: Option<usize>,
}

impl PlayerView {
    /// Any picture adjustment is off neutral.
    pub fn picture_adjusted(&self) -> bool {
        self.picture
            .iter()
            .zip(PICTURE)
            .any(|(v, k)| (v - k.neutral).abs() > 1e-4)
    }
}

/// "+350 ms", "−120 ms", "0 ms".
pub fn delay_text(ms: i64) -> String {
    let n = specular_i18n::I18n::global()
        .localizer()
        .number(ms.unsigned_abs() as f64, 0);
    let sign = match ms.signum() {
        1 => "+",
        -1 => "\u{2212}",
        _ => "",
    };
    t!("player-delay-ms", value = format!("{sign}{n}"))
}

/// Which buttons the transport gets for `v`.
pub(crate) fn buttons(v: &PlayerView, picture: bool) -> Vec<PlayerMenu> {
    let mut b = Vec::new();
    if picture {
        b.push(PlayerMenu::Subtitles);
    }
    if v.has_sound || v.sounds.len() > 1 {
        b.push(PlayerMenu::Sound);
    }
    if !v.chapters.is_empty() {
        b.push(PlayerMenu::Chapters);
    }
    b.push(PlayerMenu::More);
    b
}

pub(crate) fn icon(m: PlayerMenu) -> Icon {
    match m {
        PlayerMenu::Subtitles => Icon::Subtitles,
        PlayerMenu::Sound => Icon::SoundTracks,
        PlayerMenu::Chapters => Icon::Chapters,
        PlayerMenu::Speed => Icon::Repeat,
        PlayerMenu::More => Icon::More,
        PlayerMenu::Picture => Icon::Sliders,
        PlayerMenu::Output => Icon::SoundTracks,
        PlayerMenu::Cast => Icon::More,
    }
}

pub(crate) fn label(m: PlayerMenu) -> String {
    match m {
        PlayerMenu::Subtitles => t!("player-subtitles"),
        PlayerMenu::Sound => t!("player-sound-tracks"),
        PlayerMenu::Chapters => t!("player-chapters"),
        PlayerMenu::Speed => t!("player-speed"),
        PlayerMenu::More => t!("player-more"),
        PlayerMenu::Picture => t!("player-picture"),
        PlayerMenu::Output => t!("player-output"),
        PlayerMenu::Cast => t!("cast-menu"),
    }
}

/// Chapter ticks and the A–B span on the seek bar's track `track`.
pub(crate) fn seek_marks(cx: &mut Ctx, v: &PlayerView, track: Rect) {
    if let (Some(a), b) = v.ab {
        let b = b.unwrap_or(a);
        let (x0, x1) = (track.x + track.w * a as f32, track.x + track.w * b as f32);
        let span = Rect::new(
            x0.min(x1),
            track.y - 2.0,
            (x1 - x0).abs().max(2.0),
            track.h + 4.0,
        );
        cx.dl.rect(span, 2.0, cx.pal.accent.with_alpha(0.45));
        for x in [Some(x0), v.ab.1.map(|_| x1)].into_iter().flatten() {
            cx.dl.rect(
                Rect::new(x - 1.0, track.y - 4.0, 2.0, track.h + 8.0),
                1.0,
                cx.pal.accent,
            );
        }
    }
    for (f, _, _) in &v.chapters {
        if *f <= 0.0 {
            continue;
        }
        let x = track.x + track.w * *f as f32;
        cx.dl.rect(
            Rect::new(x - 0.75, track.y - 2.0, 1.5, track.h + 4.0),
            0.5,
            cx.pal.text.with_alpha(0.8),
        );
    }
}

/// The chapter at `secs`.
pub(crate) fn chapter_at(v: &PlayerView, secs: f64) -> Option<&str> {
    v.chapters
        .iter()
        .rev()
        .find(|(_, s, _)| *s <= secs + 1e-6)
        .map(|(_, _, n)| n.as_str())
}

/// The hover over the seek bar at `x` (`fraction` of the movie of `duration` seconds): the
/// preview frame when it is for about this place, the time and the chapter.
pub(crate) fn hover(cx: &mut Ctx, v: &PlayerView, bar: Rect, x: f32, fraction: f64, duration: f64) {
    let secs = fraction * duration;
    let mut text = clock(secs);
    if let Some(c) = chapter_at(v, secs) {
        text = specular_i18n::dot_join::<&str>(&[&text, c]);
    }
    let style = cx.style(metrics::FONT_SMALL).tabular();
    let b = cx.layout(&text, style, Some(cx.u(260.0)));
    let near = |f: f64| duration <= 0.0 || ((f - fraction) * duration).abs() < 10.0;
    let pic = v.preview.filter(|p| near(p.3) && p.1 > 0 && p.2 > 0);
    let pw = cx.u(160.0);
    let ph = pic.map_or(0.0, |p| pw * p.2 as f32 / p.1 as f32);
    let w = b.width.max(if pic.is_some() { pw } else { 0.0 }) + 12.0;
    let h = ph + b.height + if pic.is_some() { 16.0 } else { 10.0 };
    let bx = (x - w * 0.5).clamp(8.0, (cx.w - w - 8.0).max(8.0));
    let r = Rect::new(bx.round(), (bar.y - h - 8.0).round(), w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_SM, cx.pal.shadow, 12.0, 3.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_SM,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.popovers.push(r);
    if let Some((tex, _, _, _)) = pic {
        let ir = Rect::new(r.x + (w - pw) * 0.5, r.y + 6.0, pw, ph);
        cx.dl.image(ir, [0.0, 0.0, 1.0, 1.0], tex, 3.0);
    }
    let tr = Rect::new(r.x, r.y + h - b.height - 5.0, w, b.height);
    cx.block_in(&b, tr, HAlign::Center, cx.pal.text);
}

/// A menu's rows: (label, ticked, the command).
type Rows = Vec<(String, bool, PlayerCmd)>;

/// A delay's stepper.
fn delay_stepper(m: PlayerMenu, ms: i64) -> Stepper {
    let name = if m == PlayerMenu::Sound {
        t!("player-sound-delay")
    } else {
        t!("player-subtitle-delay")
    };
    let value = delay_text(ms);
    Stepper {
        text: format!("{name}: {value}"),
        value,
        less: t!("player-delay-earlier"),
        reset: t!("player-delay-reset"),
        more: t!("player-delay-later"),
    }
}

/// The menu's steppers, under its rows.
pub(crate) fn steppers(m: PlayerMenu, v: &PlayerView) -> Vec<Stepper> {
    match m {
        PlayerMenu::Subtitles => vec![delay_stepper(m, v.subtitle_delay_ms)],
        PlayerMenu::Sound => vec![delay_stepper(m, v.sound_delay_ms)],
        PlayerMenu::Picture => PICTURE
            .iter()
            .enumerate()
            .map(|(i, k)| {
                let name = k.label();
                let value = picture_text(i, v.picture[i]);
                Stepper {
                    text: format!("{name}: {value}"),
                    value,
                    less: t!("player-picture-less", name = name.clone()),
                    reset: t!("player-picture-reset-one", name = name.clone()),
                    more: t!("player-picture-more", name = name),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// The command of stepper `row`'s − (−1), reset (0) or + (1).
pub(crate) fn step_cmd(m: PlayerMenu, row: usize, step: i64) -> Option<PlayerCmd> {
    Some(match m {
        PlayerMenu::Sound => PlayerCmd::SoundDelay(step * DELAY_STEP_MS),
        PlayerMenu::Subtitles => PlayerCmd::SubtitleDelay(step * DELAY_STEP_MS),
        PlayerMenu::Picture if row < PICTURE.len() => PlayerCmd::Picture(row, step),
        _ => return None,
    })
}

/// A command that leaves its menu open (the steppers, and the picture's reset).
pub(crate) fn keeps_open(cmd: &PlayerCmd) -> bool {
    matches!(
        cmd,
        PlayerCmd::SubtitleDelay(_)
            | PlayerCmd::SoundDelay(_)
            | PlayerCmd::Picture(..)
            | PlayerCmd::PictureReset
            | PlayerCmd::CastSearch
            | PlayerCmd::CastNote
    )
}

fn rows(m: PlayerMenu, v: &PlayerView) -> Rows {
    match m {
        PlayerMenu::Subtitles => {
            let mut r = vec![(
                t!("player-subtitles-off"),
                v.subtitle.is_none(),
                PlayerCmd::Subtitle(None),
            )];
            for (i, s) in v.subtitles.iter().enumerate() {
                r.push((
                    s.clone(),
                    v.subtitle == Some(i),
                    PlayerCmd::Subtitle(Some(i)),
                ));
            }
            r.push((t!("player-subtitles-open"), false, PlayerCmd::OpenSubtitle));
            if let Some(g) = &v.generate {
                r.push((g.clone(), false, PlayerCmd::GenerateSubtitles));
            }
            if v.save_subtitles {
                r.push((t!("player-subtitles-save"), false, PlayerCmd::SaveSubtitles));
            }
            if let Some(c) = &v.charset {
                r.push((
                    t!("player-subtitles-charset", charset = c.clone()),
                    false,
                    PlayerCmd::SubtitleCharset,
                ));
            }
            r.push((
                t!("player-subtitles-style"),
                false,
                PlayerCmd::SubtitleStyle,
            ));
            r
        }
        PlayerMenu::Sound => {
            let mut r: Vec<_> = if v.sounds.len() > 1 {
                v.sounds
                    .iter()
                    .enumerate()
                    .map(|(i, s)| (s.clone(), v.sound == i, PlayerCmd::Sound(i)))
                    .collect()
            } else {
                Vec::new()
            };
            if v.has_sound {
                r.push((
                    t!("player-dialogue-boost"),
                    v.dialogue,
                    PlayerCmd::DialogueBoost,
                ));
                r.push((t!("player-mono"), v.mono, PlayerCmd::Mono));
                r.push((t!("player-equaliser"), false, PlayerCmd::Equaliser));
                if let Some(o) = v.outputs.get(v.output) {
                    r.push((
                        t!("player-output-open", device = o.clone()),
                        false,
                        PlayerCmd::OpenOutput,
                    ));
                }
            }
            r
        }
        PlayerMenu::Output => {
            let mut r: Rows = v
                .outputs
                .iter()
                .enumerate()
                .map(|(i, o)| (o.clone(), v.output == i, PlayerCmd::Output(i)))
                .collect();
            for (i, s) in v.speakers.iter().enumerate() {
                r.push((
                    t!("player-speakers", speakers = s.clone()),
                    v.speaker == i,
                    PlayerCmd::Speakers(i),
                ));
            }
            r
        }
        PlayerMenu::Chapters => v
            .chapters
            .iter()
            .enumerate()
            .map(|(i, (_, s, n))| (format!("{}  {n}", clock(*s)), false, PlayerCmd::Chapter(i)))
            .collect(),
        PlayerMenu::Speed => SPEEDS
            .iter()
            .map(|&s| {
                (
                    t!("transport-speed", speed = crate::transport::speed_number(s)),
                    (v.speed - s).abs() < 1e-6,
                    PlayerCmd::Speed(s),
                )
            })
            .collect(),
        PlayerMenu::More => {
            let mut r = vec![(t!("player-go-to-time"), false, PlayerCmd::GoToTime)];
            if v.qualities.len() > 1 {
                r.push((
                    t!("player-quality-auto"),
                    v.quality.is_none(),
                    PlayerCmd::Quality(None),
                ));
                for (i, q) in v.qualities.iter().enumerate() {
                    r.push((
                        t!("player-quality", quality = q.clone()),
                        v.quality == Some(i),
                        PlayerCmd::Quality(Some(i)),
                    ));
                }
            }
            if v.picture_menu {
                r.push((
                    t!("player-picture-open"),
                    v.picture_adjusted(),
                    PlayerCmd::OpenPicture,
                ));
                for a in [
                    Action::RotateLeft,
                    Action::RotateRight,
                    Action::FlipHorizontal,
                    Action::FlipVertical,
                ] {
                    r.push((a.name(), false, PlayerCmd::Action(a)));
                }
                if v.zoomed {
                    r.push((
                        Action::ZoomFit.name(),
                        false,
                        PlayerCmd::Action(Action::ZoomFit),
                    ));
                }
            }
            r.push((
                t!("cast-play-on"),
                v.cast.playing_on.is_some(),
                PlayerCmd::OpenCast,
            ));
            r
        }
        PlayerMenu::Picture => vec![(t!("player-picture-reset"), false, PlayerCmd::PictureReset)],
        PlayerMenu::Cast => cast_rows(&v.cast),
    }
}

/// "Play on…": this device, the devices found, then the search (or why there is none).
fn cast_rows(c: &CastView) -> Rows {
    let mut r = vec![(
        t!("cast-this-device"),
        c.playing_on.is_none(),
        PlayerCmd::CastStop,
    )];
    for (i, d) in c.devices.iter().enumerate() {
        r.push((
            specular_i18n::isolate(d),
            c.active == Some(i),
            PlayerCmd::CastTo(i),
        ));
    }
    let status = if c.network_off {
        t!("cast-network-off")
    } else if c.searching {
        t!("cast-searching")
    } else if c.devices.is_empty() {
        t!("cast-none-found")
    } else {
        t!("cast-search-again")
    };
    r.push((status, false, PlayerCmd::CastSearch));
    if let Some(n) = &c.subtitles_note {
        r.push((n.clone(), false, PlayerCmd::CastNote));
    }
    r
}

/// The label of row `i` and whether it is ticked (accessibility).
pub(crate) fn row_label(m: PlayerMenu, v: &PlayerView, i: usize) -> Option<(String, bool)> {
    let r = rows(m, v);
    r.get(i).map(|(l, t, _)| (l.clone(), *t))
}

/// How many keyboard stops a menu has (its rows, then each stepper's −, reset and +).
pub(crate) fn stops(m: PlayerMenu, v: &PlayerView) -> usize {
    rows(m, v).len() + 3 * steppers(m, v).len()
}

/// The command of keyboard stop `i`.
pub(crate) fn stop_cmd(m: PlayerMenu, v: &PlayerView, i: usize) -> Option<PlayerCmd> {
    let r = rows(m, v);
    if let Some(row) = r.get(i) {
        return Some(row.2.clone());
    }
    let k = i - r.len();
    if k / 3 >= steppers(m, v).len() {
        return None;
    }
    step_cmd(m, k / 3, (k % 3) as i64 - 1)
}

/// Draws menu `m` above `anchor`; `sel`: the keyboard's stop.
pub(crate) fn menu(cx: &mut Ctx, m: PlayerMenu, v: &PlayerView, anchor: Rect, sel: Option<usize>) {
    let items = rows(m, v);
    let steps = steppers(m, v);
    let style = cx.style(metrics::FONT_BODY);
    let row = cx.u(30.0);
    let tick = 16.0;
    let blocks: Vec<_> = items
        .iter()
        .map(|(s, _, _)| {
            let s = cx.name(s);
            cx.layout(&s, style, None)
        })
        .collect();
    let step_labels: Vec<_> = steps
        .iter()
        .map(|s| cx.layout(&s.text, style, None))
        .collect();
    let b = metrics::BUTTON;
    let w = blocks
        .iter()
        .map(|b| b.width)
        .chain(step_labels.iter().map(|l| l.width + 2.0 * b + 8.0))
        .fold(cx.u(160.0), f32::max)
        + 28.0
        + tick;
    let w = w.min(cx.w - 16.0);
    let n = items.len() + steps.len();
    let max_rows = ((anchor.y - 30.0) / row).floor().max(3.0) as usize;
    // The row of the keyboard's stop (a stepper's three stops share its row).
    let sel_row = sel.map(|s| {
        if s < items.len() {
            s
        } else {
            items.len() + (s - items.len()) / 3
        }
    });
    // A long chapter list keeps the rows around the current one.
    let first = sel_row
        .unwrap_or(0)
        .saturating_sub(max_rows.saturating_sub(1))
        .min(n.saturating_sub(max_rows));
    let shown = n.min(max_rows);
    let h = row * shown as f32 + 8.0;
    let x = (anchor.center().0 - w * 0.5).clamp(8.0, (cx.w - w - 8.0).max(8.0));
    let r = Rect::new(x.round(), (anchor.y - h - 14.0).max(8.0).round(), w, h);
    cx.dl
        .shadow(r, metrics::RADIUS_MD, cx.pal.shadow, 20.0, 6.0);
    cx.dl.glass(
        r,
        metrics::RADIUS_MD,
        cx.pal.glass,
        cx.pal.glass_border,
        1.0,
    );
    cx.hit(r, Target::ToolbarBg);
    cx.popovers.push(r);
    let row_rect = |k: usize| {
        Rect::new(
            r.x + 4.0,
            r.y + 4.0 + (k - first) as f32 * row,
            w - 8.0,
            row,
        )
    };
    for (k, ((_, ticked, _), bl)) in items
        .iter()
        .zip(&blocks)
        .enumerate()
        .skip(first)
        .take(shown)
    {
        let rr = row_rect(k);
        let t = Target::PlayerItem(m, k);
        if cx.is_hover(&t) || sel == Some(k) {
            cx.dl.rect(rr, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        let tr = rr.inset_xy(10.0, 0.0);
        // The tick leads the row (its right end right to left), as in every menu.
        let lr = cx.m_in(
            Rect::new(tr.x + tick + 4.0, tr.y, tr.w - tick - 4.0, tr.h),
            tr,
        );
        cx.block_in(bl, lr, HAlign::Start, cx.pal.text);
        if *ticked {
            let ir = Rect::new(tr.x, rr.center().1 - tick * 0.5, tick, tick);
            let ir = cx.m_in(ir, tr);
            draw_icon(cx.dl, Icon::Check, ir, cx.pal.accent, 2.0, false);
        }
        if sel == Some(k) {
            cx.focus_ring(rr, metrics::RADIUS_SM);
        }
        cx.hit(rr, t);
    }
    for (j, lb) in step_labels.iter().enumerate() {
        let k = items.len() + j;
        if k < first || k >= first + shown {
            continue;
        }
        let rr = row_rect(k);
        // The first stepper is set off from the rows above it.
        if j == 0 && !items.is_empty() {
            let sep = Rect::new(rr.x + 6.0, rr.y - 0.5, rr.w - 12.0, 1.0);
            cx.dl.rect(sep, 0.0, cx.pal.separator);
        }
        let stop = items.len() + 3 * j;
        let bs = (row - 4.0).min(b);
        let (minus, plus) = (
            Rect::new(rr.x + 4.0, rr.center().1 - bs * 0.5, bs, bs),
            Rect::new(rr.right() - bs - 4.0, rr.center().1 - bs * 0.5, bs, bs),
        );
        // − and + keep their places right to left (time runs one way); the reset between
        // them.
        let mid = Rect::new(
            minus.right() + 4.0,
            rr.y,
            plus.x - minus.right() - 8.0,
            rr.h,
        );
        let step_button = |cx: &mut Ctx, br: Rect, ic: Icon, step: i64, at: usize| {
            let t = Target::PlayerDelay(m, j, step);
            cx.icon_button_e(br, ic, t, cx.pal.text, false, false, true);
            if sel == Some(at) {
                cx.focus_ring(br, metrics::RADIUS_SM);
            }
        };
        step_button(cx, minus, Icon::Minus, -1, stop);
        let t = Target::PlayerDelay(m, j, 0);
        if cx.is_hover(&t) || sel == Some(stop + 1) {
            cx.dl.rect(mid, metrics::RADIUS_SM, cx.pal.surface_hover);
        }
        cx.block_in(lb, mid, HAlign::Center, cx.pal.text);
        if sel == Some(stop + 1) {
            cx.focus_ring(mid, metrics::RADIUS_SM);
        }
        cx.hit(mid, t);
        step_button(cx, plus, Icon::Plus, 1, stop + 2);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delays_read_with_a_sign() {
        let s = |ms| specular_i18n::strip_isolates(&delay_text(ms));
        assert_eq!(s(350), "+350 ms");
        assert_eq!(s(-120), "\u{2212}120 ms");
        assert_eq!(s(0), "0 ms");
    }

    #[test]
    fn menu_stops() {
        let v = PlayerView {
            subtitles: vec!["English".into(), "Hebrew".into()],
            subtitle: Some(1),
            has_sound: true,
            ..PlayerView::default()
        };
        // Off, two tracks, Open…, Subtitle style…, then −, reset, +.
        assert_eq!(stops(PlayerMenu::Subtitles, &v), 8);
        assert_eq!(
            stop_cmd(PlayerMenu::Subtitles, &v, 0),
            Some(PlayerCmd::Subtitle(None))
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Subtitles, &v, 3),
            Some(PlayerCmd::OpenSubtitle)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Subtitles, &v, 4),
            Some(PlayerCmd::SubtitleStyle)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Subtitles, &v, 7),
            Some(PlayerCmd::SubtitleDelay(50))
        );
        // Dialogue boost, mono, the equaliser, then the delay's −, reset, +.
        assert_eq!(
            stop_cmd(PlayerMenu::Sound, &v, 0),
            Some(PlayerCmd::DialogueBoost)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Sound, &v, 2),
            Some(PlayerCmd::Equaliser)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Sound, &v, 4),
            Some(PlayerCmd::SoundDelay(0))
        );
        assert_eq!(
            buttons(&v, true),
            [PlayerMenu::Subtitles, PlayerMenu::Sound, PlayerMenu::More]
        );
    }

    /// Sound › Output… lists the devices, then the speakers, each ticked as chosen.
    #[test]
    fn output_menu() {
        let v = PlayerView {
            has_sound: true,
            outputs: vec!["System default".into(), "USB DAC".into()],
            output: 1,
            speakers: vec!["Automatic".into(), "Stereo".into()],
            speaker: 0,
            ..PlayerView::default()
        };
        // Dialogue boost, mono, the equaliser, then Output….
        assert_eq!(
            stop_cmd(PlayerMenu::Sound, &v, 3),
            Some(PlayerCmd::OpenOutput)
        );
        let r = rows(PlayerMenu::Output, &v);
        assert_eq!(r.len(), 4);
        let ticked: Vec<_> = r.iter().filter(|x| x.1).map(|x| x.2.clone()).collect();
        assert_eq!(ticked, [PlayerCmd::Output(1), PlayerCmd::Speakers(0)]);
        assert_eq!(
            stop_cmd(PlayerMenu::Output, &v, 3),
            Some(PlayerCmd::Speakers(1))
        );
        // No sound: no Output….
        let silent = PlayerView {
            has_sound: false,
            ..v
        };
        assert!(
            !rows(PlayerMenu::Sound, &silent)
                .iter()
                .any(|x| x.2 == PlayerCmd::OpenOutput)
        );
    }

    #[test]
    fn speed_menu_ticks_the_speed() {
        let v = PlayerView {
            speed: 1.5,
            ..PlayerView::default()
        };
        let r = rows(PlayerMenu::Speed, &v);
        assert_eq!(r.len(), SPEEDS.len());
        let ticked: Vec<_> = r.iter().filter(|x| x.1).map(|x| x.2.clone()).collect();
        assert_eq!(ticked, [PlayerCmd::Speed(1.5)]);
        assert_eq!(stops(PlayerMenu::Speed, &v), 6);
        assert_eq!(
            stop_cmd(PlayerMenu::Speed, &v, 0),
            Some(PlayerCmd::Speed(0.5))
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Speed, &v, 5),
            Some(PlayerCmd::Speed(2.0))
        );
    }

    #[test]
    fn more_and_picture_menus() {
        let sound_only = PlayerView::default();
        assert_eq!(
            stop_cmd(PlayerMenu::More, &sound_only, 0),
            Some(PlayerCmd::GoToTime)
        );
        // Go to time…, Play on….
        assert_eq!(stops(PlayerMenu::More, &sound_only), 2);
        assert_eq!(
            stop_cmd(PlayerMenu::More, &sound_only, 1),
            Some(PlayerCmd::OpenCast)
        );
        let v = PlayerView {
            picture_menu: true,
            picture: PICTURE.map(|k| k.neutral),
            ..PlayerView::default()
        };
        // Go to time…, Picture…, rotate left and right, flip both ways, Play on….
        assert_eq!(stops(PlayerMenu::More, &v), 7);
        assert_eq!(
            stop_cmd(PlayerMenu::More, &v, 1),
            Some(PlayerCmd::OpenPicture)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::More, &v, 3),
            Some(PlayerCmd::Action(Action::RotateRight))
        );
        // A stream's qualities: automatic, then each, ticked as chosen.
        let q = PlayerView {
            qualities: vec!["640×360, 1 Mbit/s".into(), "1280×720, 3 Mbit/s".into()],
            quality: Some(1),
            ..PlayerView::default()
        };
        let r = rows(PlayerMenu::More, &q);
        assert_eq!(r.len(), 5, "go to time, automatic, two, play on");
        assert_eq!(r[1].2, PlayerCmd::Quality(None));
        assert!(!r[1].1 && r[3].1);
        assert!(r[3].0.contains("1280×720"), "{}", r[3].0);
        assert_eq!(
            stops(
                PlayerMenu::More,
                &PlayerView {
                    qualities: vec!["x".into()],
                    ..PlayerView::default()
                }
            ),
            2,
            "one quality: no choice"
        );
        let zoomed = PlayerView {
            zoomed: true,
            ..v.clone()
        };
        assert_eq!(
            stop_cmd(PlayerMenu::More, &zoomed, 6),
            Some(PlayerCmd::Action(Action::ZoomFit))
        );
        // Reset all, then five steppers of three stops.
        assert_eq!(stops(PlayerMenu::Picture, &v), 16);
        assert_eq!(
            stop_cmd(PlayerMenu::Picture, &v, 0),
            Some(PlayerCmd::PictureReset)
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Picture, &v, 1),
            Some(PlayerCmd::Picture(0, -1))
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Picture, &v, 9),
            Some(PlayerCmd::Picture(2, 1))
        );
        assert_eq!(
            stop_cmd(PlayerMenu::Picture, &v, 14),
            Some(PlayerCmd::Picture(4, 0))
        );
        assert_eq!(stop_cmd(PlayerMenu::Picture, &v, 16), None);
        assert!(!v.picture_adjusted());
    }

    #[test]
    fn cast_menu() {
        let mut v = PlayerView::default();
        v.cast.searching = true;
        // This device (ticked), then "Looking for devices…".
        let r = rows(PlayerMenu::Cast, &v);
        assert_eq!(r.len(), 2);
        assert!(r[0].1);
        assert_eq!(r[0].2, PlayerCmd::CastStop);
        assert_eq!(r[1].2, PlayerCmd::CastSearch);
        v.cast.subtitles_note = Some("why".into());
        let r = rows(PlayerMenu::Cast, &v);
        assert_eq!(r.len(), 3, "the note last");
        assert_eq!((r[2].0.as_str(), &r[2].2), ("why", &PlayerCmd::CastNote));
        assert!(keeps_open(&PlayerCmd::CastNote));
        v.cast.subtitles_note = None;
        v.cast.devices = vec!["Living Room TV".into(), "Kitchen".into()];
        v.cast.active = Some(1);
        v.cast.playing_on = Some("Kitchen".into());
        let r = rows(PlayerMenu::Cast, &v);
        let ticked: Vec<_> = r.iter().filter(|x| x.1).map(|x| x.2.clone()).collect();
        assert_eq!(ticked, [PlayerCmd::CastTo(1)]);
        assert_eq!(stops(PlayerMenu::Cast, &v), 4);
        assert_eq!(
            stop_cmd(PlayerMenu::Cast, &v, 1),
            Some(PlayerCmd::CastTo(0))
        );
        // More ticks Play on… while casting.
        let more = rows(PlayerMenu::More, &v);
        assert_eq!(
            more.last().map(|x| (x.1, x.2.clone())),
            Some((true, PlayerCmd::OpenCast))
        );
    }

    #[test]
    fn knobs_step_within_their_range() {
        let b = PICTURE[0];
        assert!((b.stepped(0.0, 1) - 0.05).abs() < 1e-6);
        assert!((b.stepped(0.97, 1) - 1.0).abs() < 1e-6);
        assert_eq!(b.stepped(0.4, 0), 0.0);
        let g = PICTURE[3];
        assert!((g.stepped(1.0, -1) - 0.9).abs() < 1e-6);
        assert_eq!(g.stepped(0.3, -3), 0.25);
        let h = PICTURE[4];
        assert_eq!(h.stepped(170.0, 2), 180.0);
        let s = |k, v| specular_i18n::strip_isolates(&picture_text(k, v));
        assert_eq!(s(0, 0.15), "+15%");
        assert_eq!(s(1, 0.9), "\u{2212}10%");
        assert_eq!(s(3, 1.2), "1.2");
        assert_eq!(s(4, -30.0), "\u{2212}30°");
    }

    #[test]
    fn chapter_under_a_time() {
        let v = PlayerView {
            chapters: vec![(0.0, 0.0, "One".into()), (0.5, 60.0, "Two".into())],
            ..PlayerView::default()
        };
        assert_eq!(chapter_at(&v, 59.0), Some("One"));
        assert_eq!(chapter_at(&v, 60.0), Some("Two"));
    }
}
