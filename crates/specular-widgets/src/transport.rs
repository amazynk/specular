//! The animation transport bar: a small glass bar above the toolbar with step back,
//! play/pause, step forward (a movie's jump 5 seconds), the frame readout and the playback speed. It fades with the
//! toolbar. Media controls keep their left-to-right order in right-to-left languages (as
//! platform media players do): "forward" in time is not a reading direction.
//!
//! A movie's bar adds a seek bar across its top (drag to scrub), shows the time instead of
//! the frame number, and has a loop toggle, a mute button and volume slider when the movie
//! has sound (`volume.rs`; the slider goes when the window is too narrow), and a
//! full-screen play button when it has a picture. In full-screen play mode the bar sits at
//! the bottom of the screen, over the picture. A movie's speed opens the speed menu, and its
//! time "Go to time…" (the field takes the time's place).

use specular_gfx::{Icon, Rect};
use specular_i18n::{I18n, t};
use specular_input::Action;
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::line_edit::LineEdit;
use crate::player::{self, PlayerMenu, PlayerView};
use crate::{Playback, VideoPlayback, ViewerState};

/// Height of the seek bar's row at the top of a movie's bar.
const SEEK_ROW: f32 = 14.0;

/// "1.5", "0.25" in the UI language's digits.
pub(crate) fn speed_number(speed: f64) -> String {
    I18n::global().localizer().number(speed, 2)
}

/// "Frame 3 / 12" (or "Frame 3" while the count is still unknown).
pub(crate) fn frame_text(pb: &Playback) -> String {
    let index = pb.frame as i64 + 1;
    match pb.count {
        Some(n) => t!("transport-frame", index = index, count = n as i64),
        None => t!("transport-frame-unknown", index = index),
    }
}

/// A time as a clock reading: "0:07", "12:30", "1:02:03" (whole seconds, rounded down).
pub(crate) fn clock(secs: f64) -> String {
    let s = if secs.is_finite() {
        secs.max(0.0) as u64
    } else {
        0
    };
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    let l = I18n::global().localizer();
    let two = |v: u64| {
        let d = l.number(v as f64, 0);
        if v < 10 { l.number(0.0, 0) + &d } else { d }
    };
    if h > 0 {
        format!("{}:{}:{}", l.number(h as f64, 0), two(m), two(s))
    } else {
        format!("{}:{}", l.number(m as f64, 0), two(s))
    }
}

/// "0:12 / 1:30".
pub(crate) fn time_text(v: &VideoPlayback) -> String {
    t!(
        "transport-time",
        position = clock(v.position),
        duration = clock(v.duration)
    )
}

/// What the readout says: the time of a movie, else the frame.
pub(crate) fn readout(pb: &Playback) -> String {
    match &pb.video {
        Some(v) => time_text(v),
        None => frame_text(pb),
    }
}

/// The play/pause button's label for the current state.
pub(crate) fn play_label(pb: &Playback) -> String {
    if pb.playing {
        t!("transport-pause")
    } else {
        t!("transport-play")
    }
}

/// Label of a transport button.
pub(crate) fn label(a: Action, st: &ViewerState) -> String {
    let video = st.playback.as_ref().and_then(|p| p.video);
    match a {
        Action::PlayPause => st.playback.as_ref().map(play_label).unwrap_or_default(),
        Action::PrevFrame => t!("transport-prev-frame"),
        Action::NextFrame => t!("transport-next-frame"),
        Action::SlowerPlayback => t!("transport-slower"),
        Action::FasterPlayback => t!("transport-faster"),
        Action::ToggleLoopForever => match video {
            Some(v) if v.looping => t!("transport-loop-off"),
            _ => t!("transport-loop-on"),
        },
        Action::ToggleMute => match video.and_then(|v| v.sound) {
            Some((_, true)) => t!("transport-unmute"),
            _ => t!("transport-mute"),
        },
        Action::CycleRepeat => t!("queue-repeat"),
        Action::ToggleShuffle => t!("queue-shuffle"),
        Action::PrevTrack => t!("transport-prev-track"),
        Action::NextTrack => t!("transport-next-track"),
        Action::ToggleQueue => t!("queue-title"),
        Action::VideoFullscreen if st.fullscreen => t!("toolbar-fullscreen-exit"),
        Action::VideoFullscreen => t!("toolbar-fullscreen-enter"),
        _ => a.name(),
    }
}

/// The seek bar's position under `x` (0 at its start, 1 at its end) when `r` is its hit
/// rectangle.
pub(crate) fn seek_fraction(r: Rect, x: f32) -> f64 {
    let (x0, w) = (r.x + SEEK_INSET, (r.w - 2.0 * SEEK_INSET).max(1.0));
    f64::from(((x - x0) / w).clamp(0.0, 1.0))
}

/// Height of the bar (`video`: a movie's, with the seek bar).
pub(crate) fn height(video: bool) -> f32 {
    metrics::BUTTON + 12.0 + if video { SEEK_ROW } else { 0.0 }
}

/// Space between the seek bar's hit rectangle and its track, at either end.
const SEEK_INSET: f32 = 6.0;

/// Draws the bar; returns its top edge (where toasts start stacking) and where the player
/// menus' buttons went. `dock`: how far it has slid down into the toolbar's place (0: above
/// the toolbar, 1: at the bottom of the screen with no toolbar under it, as in full-screen
/// play mode). `pv`: the movie's player menus; `goto`: the "Go to time…" field while it is
/// open; `pointer`: for the seek bar's hover; `order`: repeat and shuffle, for a sound file's
/// buttons (previous and next item, repeat, shuffle, the queue).
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    cx: &mut Ctx,
    st: &ViewerState,
    pb: &Playback,
    alpha: f32,
    dock: f32,
    pv: Option<&PlayerView>,
    goto: Option<&LineEdit>,
    pointer: Option<(f32, f32)>,
    order: Option<crate::PlayOrder>,
) -> (f32, Vec<(PlayerMenu, Rect)>) {
    let menus: Vec<PlayerMenu> = match (&pb.video, pv) {
        (Some(v), Some(p)) => player::buttons(p, v.picture),
        _ => Vec::new(),
    };
    let mut anchors = Vec::new();
    let style = cx.style(metrics::FONT_LABEL).tabular().weight(500.0);
    let frame = cx.layout(&readout(pb), style, None);
    // As wide as the widest readout, so the bar keeps its size while playing.
    let widest = match (&pb.video, pb.count) {
        (Some(v), _) => time_text(&VideoPlayback {
            position: v.duration,
            ..*v
        }),
        (None, Some(n)) => t!("transport-frame", index = n as i64, count = n as i64),
        (None, None) => t!("transport-frame-unknown", index = 8888),
    };
    let frame_w = frame.width.max(cx.layout(&widest, style, None).width) + 16.0;
    let speed = cx.layout(
        &t!("transport-speed", speed = speed_number(pb.speed)),
        style,
        None,
    );
    let speed_w = speed.width.max(
        cx.layout(
            &t!("transport-speed", speed = speed_number(0.25)),
            style,
            None,
        )
        .width,
    ) + 12.0;

    let b = metrics::BUTTON;
    let pad = 6.0;
    let gap = 2.0;
    // A sound file: previous and next item, shuffle and the queue too.
    let sound_file = pb.video.as_ref().is_some_and(|v| !v.picture);
    let extra = match &pb.video {
        Some(v) => {
            1 + usize::from(v.sound.is_some())
                + usize::from(v.picture)
                + menus.len()
                + if sound_file { 4 } else { 0 }
        }
        None => 0,
    };
    let buttons = 5 + extra;
    let total = pad * 2.0 + b * buttons as f32 + gap * (buttons + 1) as f32 + frame_w + speed_w;
    // The volume slider beside the mute button, when the window has room for it.
    let volume_w = crate::volume::WIDTH + gap;
    let slider = pb.video.as_ref().is_some_and(|v| v.sound.is_some())
        && total + volume_w <= cx.w - 2.0 * metrics::TOOLBAR_MARGIN;
    let total = if slider { total + volume_w } else { total };
    let seek_h = if pb.video.is_some() { SEEK_ROW } else { 0.0 };
    let h = b + 12.0 + seek_h;
    let slide = (1.0 - alpha) * 10.0;
    let lift = (metrics::TOOLBAR_HEIGHT + 8.0) * (1.0 - dock.clamp(0.0, 1.0));
    let bottom = (cx.h - metrics::TOOLBAR_MARGIN - lift).round();
    let bar = Rect::new(((cx.w - total) * 0.5).round(), bottom - h + slide, total, h);
    let radius = (b + 12.0) * 0.5;
    cx.dl.push_opacity(alpha);
    cx.dl.shadow(
        bar,
        radius,
        cx.pal.shadow,
        metrics::SHADOW_BLUR,
        metrics::SHADOW_Y,
    );
    cx.dl
        .glass(bar, radius, cx.pal.glass, cx.pal.glass_border, 1.0);
    cx.hit(bar, Target::ToolbarBg);

    if let Some(v) = &pb.video {
        // The seek bar: the track, the part played, and a knob at the playhead.
        let hit = Rect::new(
            bar.x + radius - SEEK_INSET,
            bar.y + 4.0,
            bar.w - 2.0 * radius + 2.0 * SEEK_INSET,
            seek_h,
        );
        let track = Rect::new(
            hit.x + SEEK_INSET,
            hit.y + hit.h * 0.5 - 1.5,
            hit.w - 2.0 * SEEK_INSET,
            3.0,
        );
        let f = if v.duration > 0.0 {
            (v.position / v.duration).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        cx.dl.rect(track, 1.5, cx.pal.text_faint.with_alpha(0.5));
        let played = Rect::new(track.x, track.y, track.w * f, track.h);
        cx.dl.rect(played, 1.5, cx.pal.accent);
        if let Some(p) = pv {
            player::seek_marks(cx, p, track);
        }
        let knob = if cx.is_hover(&Target::SeekBar) || cx.is_pressed(&Target::SeekBar) {
            6.0
        } else {
            4.5
        };
        cx.dl
            .circle((track.x + track.w * f, track.y + 1.5), knob, cx.pal.accent);
        cx.hit(hit, Target::SeekBar);
        if let (Some(p), Some((px, py))) = (pv, pointer)
            && (cx.is_hover(&Target::SeekBar) || hit.contains((px, py)))
            && !cx.is_pressed(&Target::SeekBar)
            && alpha > 0.5
        {
            let f = seek_fraction(hit, px);
            player::hover(cx, p, bar, px, f, v.duration);
        }
    }

    let by = bar.y + seek_h + (b + 12.0 - b) * 0.5;
    let mut x = bar.x + pad;
    let button = |cx: &mut Ctx, x: &mut f32, icon: Icon, a: Action, active: bool| {
        let r = Rect::new(*x, by, b, b);
        let t = Target::Transport(a);
        let enabled = t.disabled(st).is_none();
        cx.icon_button_e(r, icon, t, cx.pal.text, false, active, enabled);
        *x += b + gap;
    };
    // A movie's step buttons jump 5 seconds (a frame is too little to see); `,` and `.`
    // still step a frame.
    let (back, forward) = if pb.video.is_some() {
        (Action::SeekBack, Action::SeekForward)
    } else {
        (Action::PrevFrame, Action::NextFrame)
    };
    if sound_file {
        button(cx, &mut x, Icon::TrackPrev, Action::PrevTrack, false);
    }
    button(cx, &mut x, Icon::StepBack, back, false);
    let play = if pb.playing { Icon::Pause } else { Icon::Play };
    button(cx, &mut x, play, Action::PlayPause, false);
    button(cx, &mut x, Icon::StepForward, forward, false);
    if sound_file {
        button(cx, &mut x, Icon::TrackNext, Action::NextTrack, false);
    }
    let r = Rect::new(x, by, frame_w, b);
    match goto {
        Some(edit) if pb.video.is_some() => {
            crate::goto_time::draw_field(cx, r, edit, style);
            cx.hit(r, Target::Position);
        }
        _ if pb.video.is_some() && pv.is_some() => {
            // A movie's time: "Go to time…".
            let t = Target::Transport(Action::GoToTime);
            if cx.is_hover(&t) || cx.is_focused(&t) {
                let bg = if cx.is_pressed(&t) {
                    cx.pal.surface_pressed
                } else {
                    cx.pal.surface_hover
                };
                cx.dl.rect(r, metrics::RADIUS_SM, bg);
            }
            cx.block_in(&frame, r, HAlign::Center, cx.pal.text_muted);
            cx.hit(r, t);
        }
        _ => cx.block_in(&frame, r, HAlign::Center, cx.pal.text_muted),
    }
    x += frame_w + gap;
    button(cx, &mut x, Icon::Minus, Action::SlowerPlayback, false);
    let r = Rect::new(x, by, speed_w, b);
    let c = if (pb.speed - 1.0).abs() < 1e-6 {
        cx.pal.text_muted
    } else {
        cx.pal.accent
    };
    // A movie's speed opens the speed menu; an animation's goes back to 1× (as 1:1 does for
    // the zoom).
    let target = if pb.video.is_some() && pv.is_some() {
        anchors.push((PlayerMenu::Speed, r));
        Target::PlayerButton(PlayerMenu::Speed)
    } else {
        Target::Transport(Action::NormalPlayback)
    };
    if cx.is_hover(&target) || cx.is_focused(&target) {
        let bg = if cx.is_pressed(&target) {
            cx.pal.surface_pressed
        } else {
            cx.pal.surface_hover
        };
        cx.dl.rect(r, metrics::RADIUS_SM, bg);
    }
    cx.block_in(&speed, r, HAlign::Center, c);
    cx.hit(r, target);
    x += speed_w + gap;
    button(cx, &mut x, Icon::Plus, Action::FasterPlayback, false);
    if let Some(v) = &pb.video {
        if sound_file {
            // Repeat off, all or one, and shuffle (`queue_panel.rs`).
            let o = order.unwrap_or_default();
            let icon = crate::queue_panel::repeat_icon(o.repeat);
            let on = o.repeat != specular_settings::PlayRepeat::Off;
            button(cx, &mut x, icon, Action::CycleRepeat, on);
            button(cx, &mut x, Icon::Shuffle, Action::ToggleShuffle, o.shuffle);
        } else {
            button(
                cx,
                &mut x,
                Icon::Repeat,
                Action::ToggleLoopForever,
                v.looping,
            );
        }
        if let Some((volume, muted)) = v.sound {
            let icon = if muted { Icon::Muted } else { Icon::Volume };
            button(cx, &mut x, icon, Action::ToggleMute, false);
            if slider {
                let r = Rect::new(x, by, crate::volume::WIDTH, b);
                crate::volume::draw(cx, r, volume, v.volume_max, muted, alpha);
                x += volume_w;
            }
        }
        if sound_file {
            button(cx, &mut x, Icon::Queue, Action::ToggleQueue, false);
        }
        if v.picture {
            let icon = if st.fullscreen {
                Icon::ExitFullscreen
            } else {
                Icon::Fullscreen
            };
            button(cx, &mut x, icon, Action::VideoFullscreen, false);
        }
        for m in menus {
            let r = Rect::new(x, by, b, b);
            let t = Target::PlayerButton(m);
            let active = pv.is_some_and(|p| match m {
                PlayerMenu::Subtitles => p.subtitle.is_some(),
                PlayerMenu::More => p.picture_adjusted(),
                _ => false,
            });
            cx.icon_button_e(r, player::icon(m), t, cx.pal.text, false, active, true);
            anchors.push((m, r));
            x += b + gap;
        }
    }
    cx.dl.pop_opacity();
    (bar.y, anchors)
}
