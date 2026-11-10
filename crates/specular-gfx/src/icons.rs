//! Stroke icons in the Lucide style (24-unit grid, round caps), drawn analytically by the
//! UI shader, so they are crisp at any size and scale factor.

use specular_theme::Color;

use crate::draw::{DrawList, Rect};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Icon {
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    ZoomIn,
    ZoomOut,
    Fit,
    ActualSize,
    Info,
    Trash,
    RotateLeft,
    RotateRight,
    Settings,
    Copy,
    Folder,
    Close,
    Search,
    Fullscreen,
    ExitFullscreen,
    More,
    Keyboard,
    Undo,
    Navigator,
    Grid,
    Check,
    Image,
    Globe,
    Sun,
    Moon,
    Alert,
    /// Animation transport.
    Play,
    Pause,
    /// A strip of film (video export).
    Film,
    StepForward,
    StepBack,
    Minus,
    Plus,
    /// A drag handle (two columns of dots).
    Grip,
    /// Toolbar spacer: a gap between two bars.
    Spacer,
    /// Toolbar separator: a divider between two dots.
    Separator,
    /// A padlock: something encrypted.
    Lock,
    /// A speaker with sound waves (a movie's sound on).
    Volume,
    /// A speaker with a cross (muted).
    Muted,
    /// Two arrows chasing each other (loop playback).
    Repeat,
    /// An arrow down onto a tray (a download).
    Download,
    /// A house: the start screen.
    Home,
    /// Three sliders: the facets panel.
    Sliders,
    /// The Zynk app's mark in its own colours, not the given one: the white "Z" on its blue
    /// tile (Share and Send with Zynk).
    Zynk,
    /// A screen with two lines of text at its foot (subtitles).
    Subtitles,
    /// Bars of different heights (a movie's sound tracks).
    SoundTracks,
    /// A list with bullets (chapters).
    Chapters,
    /// A small window in the corner of a big one (the mini player).
    MiniPlayer,
    /// Two crossing arrows (shuffle).
    Shuffle,
    /// Two arrows chasing each other around a 1 (repeat one).
    RepeatOne,
    /// Two triangles and a bar: the next item (mirrored: the previous one).
    TrackNext,
    TrackPrev,
    /// Lines and a note: the play queue.
    Queue,
    /// Discord's symbol, filled from its own path data in the given colour, never mirrored
    /// (the button that opens the Specular Discord server; see `mark.PROVENANCE.md`).
    Discord,
}

#[derive(Clone, Copy)]
enum Seg {
    L(f32, f32, f32, f32),
    /// Centre, radius, start and sweep in degrees (clockwise on screen).
    A(f32, f32, f32, f32, f32),
    C(f32, f32, f32),
    /// Stroked rounded rect.
    R(f32, f32, f32, f32, f32),
    /// Filled dot.
    D(f32, f32, f32),
}

use Seg::*;

const FOLDER: &[Seg] = &[
    L(4.0, 3.0, 8.0, 3.0),
    L(8.0, 3.0, 10.0, 6.0),
    L(10.0, 6.0, 20.0, 6.0),
    L(22.0, 8.0, 22.0, 18.0),
    L(20.0, 20.0, 4.0, 20.0),
    L(2.0, 18.0, 2.0, 5.0),
    A(20.0, 8.0, 2.0, 270.0, 90.0),
    A(20.0, 18.0, 2.0, 0.0, 90.0),
    A(4.0, 18.0, 2.0, 90.0, 90.0),
    A(4.0, 5.0, 2.0, 180.0, 90.0),
];

const STEP: &[Seg] = &[
    L(6.0, 6.0, 14.0, 12.0),
    L(14.0, 12.0, 6.0, 18.0),
    L(6.0, 18.0, 6.0, 6.0),
    L(18.0, 6.0, 18.0, 18.0),
];

const TRACK: &[Seg] = &[
    L(3.0, 6.0, 10.0, 12.0),
    L(10.0, 12.0, 3.0, 18.0),
    L(3.0, 18.0, 3.0, 6.0),
    L(10.0, 6.0, 17.0, 12.0),
    L(17.0, 12.0, 10.0, 18.0),
    L(10.0, 18.0, 10.0, 6.0),
    L(20.0, 6.0, 20.0, 18.0),
];

const ROTATE: &[Seg] = &[
    A(12.0, 12.0, 9.0, 270.0, 270.0),
    A(12.0, 12.0, 9.0, 225.0, 45.0),
    L(5.64, 5.64, 3.0, 8.0),
    L(3.0, 3.0, 3.0, 8.0),
    L(3.0, 8.0, 8.0, 8.0),
];

fn segs(icon: Icon) -> (&'static [Seg], bool) {
    let s: &'static [Seg] = match icon {
        Icon::ChevronLeft => &[L(15.0, 18.0, 9.0, 12.0), L(9.0, 12.0, 15.0, 6.0)],
        Icon::ChevronRight => &[L(9.0, 18.0, 15.0, 12.0), L(15.0, 12.0, 9.0, 6.0)],
        Icon::ChevronUp => &[L(6.0, 15.0, 12.0, 9.0), L(12.0, 9.0, 18.0, 15.0)],
        Icon::ChevronDown => &[L(6.0, 9.0, 12.0, 15.0), L(12.0, 15.0, 18.0, 9.0)],
        Icon::ZoomIn => &[
            C(11.0, 11.0, 7.5),
            L(21.0, 21.0, 16.4, 16.4),
            L(11.0, 8.0, 11.0, 14.0),
            L(8.0, 11.0, 14.0, 11.0),
        ],
        Icon::ZoomOut => &[
            C(11.0, 11.0, 7.5),
            L(21.0, 21.0, 16.4, 16.4),
            L(8.0, 11.0, 14.0, 11.0),
        ],
        Icon::Search => &[C(11.0, 11.0, 7.5), L(21.0, 21.0, 16.4, 16.4)],
        Icon::Fit => &[
            L(3.0, 8.0, 3.0, 3.0),
            L(3.0, 3.0, 8.0, 3.0),
            L(16.0, 3.0, 21.0, 3.0),
            L(21.0, 3.0, 21.0, 8.0),
            L(21.0, 16.0, 21.0, 21.0),
            L(21.0, 21.0, 16.0, 21.0),
            L(8.0, 21.0, 3.0, 21.0),
            L(3.0, 21.0, 3.0, 16.0),
            R(8.0, 8.0, 8.0, 8.0, 1.5),
        ],
        Icon::ActualSize => &[
            L(4.5, 9.0, 6.5, 7.0),
            L(6.5, 7.0, 6.5, 17.0),
            D(12.0, 10.0, 1.2),
            D(12.0, 14.5, 1.2),
            L(15.5, 9.0, 17.5, 7.0),
            L(17.5, 7.0, 17.5, 17.0),
        ],
        Icon::Info => &[
            C(12.0, 12.0, 9.5),
            L(12.0, 16.5, 12.0, 11.5),
            D(12.0, 8.0, 1.25),
        ],
        Icon::Alert => &[
            C(12.0, 12.0, 9.5),
            L(12.0, 7.5, 12.0, 12.5),
            D(12.0, 16.0, 1.25),
        ],
        Icon::Trash => &[
            L(3.0, 6.0, 21.0, 6.0),
            R(5.0, 6.0, 14.0, 16.0, 2.0),
            R(9.0, 2.0, 6.0, 4.0, 1.5),
            L(10.0, 11.0, 10.0, 17.0),
            L(14.0, 11.0, 14.0, 17.0),
        ],
        Icon::RotateLeft => ROTATE,
        Icon::RotateRight => return (ROTATE, true),
        Icon::Settings => &[
            L(21.0, 4.0, 14.0, 4.0),
            L(10.0, 4.0, 3.0, 4.0),
            L(21.0, 12.0, 12.0, 12.0),
            L(8.0, 12.0, 3.0, 12.0),
            L(21.0, 20.0, 16.0, 20.0),
            L(12.0, 20.0, 3.0, 20.0),
            L(14.0, 2.0, 14.0, 6.0),
            L(8.0, 10.0, 8.0, 14.0),
            L(16.0, 18.0, 16.0, 22.0),
        ],
        Icon::Copy => &[
            R(8.0, 8.0, 14.0, 14.0, 2.0),
            L(2.0, 4.0, 2.0, 14.0),
            L(4.0, 2.0, 14.0, 2.0),
            A(4.0, 4.0, 2.0, 180.0, 90.0),
            A(4.0, 14.0, 2.0, 90.0, 90.0),
            A(14.0, 4.0, 2.0, 270.0, 90.0),
        ],
        Icon::Folder => FOLDER,
        Icon::Close => &[L(18.0, 6.0, 6.0, 18.0), L(6.0, 6.0, 18.0, 18.0)],
        Icon::Fullscreen => &[
            L(15.0, 3.0, 21.0, 3.0),
            L(21.0, 3.0, 21.0, 9.0),
            L(9.0, 21.0, 3.0, 21.0),
            L(3.0, 21.0, 3.0, 15.0),
            L(21.0, 3.0, 14.0, 10.0),
            L(3.0, 21.0, 10.0, 14.0),
        ],
        Icon::ExitFullscreen => &[
            L(4.0, 14.0, 10.0, 14.0),
            L(10.0, 14.0, 10.0, 20.0),
            L(20.0, 10.0, 14.0, 10.0),
            L(14.0, 10.0, 14.0, 4.0),
            L(14.0, 10.0, 21.0, 3.0),
            L(3.0, 21.0, 10.0, 14.0),
        ],
        Icon::More => &[D(5.0, 12.0, 1.4), D(12.0, 12.0, 1.4), D(19.0, 12.0, 1.4)],
        Icon::Keyboard => &[
            R(2.0, 4.0, 20.0, 16.0, 2.0),
            D(6.0, 8.5, 0.9),
            D(10.0, 8.5, 0.9),
            D(14.0, 8.5, 0.9),
            D(18.0, 8.5, 0.9),
            D(6.0, 12.0, 0.9),
            D(10.0, 12.0, 0.9),
            D(14.0, 12.0, 0.9),
            D(18.0, 12.0, 0.9),
            L(7.0, 16.0, 17.0, 16.0),
        ],
        Icon::Undo => &[
            L(9.0, 14.0, 4.0, 9.0),
            L(4.0, 9.0, 9.0, 4.0),
            L(4.0, 9.0, 14.5, 9.0),
            A(14.5, 14.5, 5.5, 270.0, 180.0),
            L(14.5, 20.0, 11.0, 20.0),
        ],
        Icon::Navigator => &[R(3.0, 3.0, 18.0, 18.0, 2.0), R(11.0, 11.0, 7.0, 7.0, 1.0)],
        Icon::Grid => &[
            R(3.0, 3.0, 18.0, 18.0, 2.0),
            L(3.0, 9.0, 21.0, 9.0),
            L(3.0, 15.0, 21.0, 15.0),
            L(9.0, 3.0, 9.0, 21.0),
            L(15.0, 3.0, 15.0, 21.0),
        ],
        Icon::Check => &[L(20.0, 6.0, 9.0, 17.0), L(9.0, 17.0, 4.0, 12.0)],
        Icon::Image => &[
            R(3.0, 3.0, 18.0, 18.0, 2.0),
            C(9.0, 9.0, 2.0),
            L(21.0, 15.0, 16.0, 10.0),
            L(16.0, 10.0, 5.0, 21.0),
        ],
        Icon::Globe => &[
            C(12.0, 12.0, 10.0),
            L(2.0, 12.0, 22.0, 12.0),
            A(2.0, 12.0, 14.0, 315.0, 90.0),
            A(22.0, 12.0, 14.0, 135.0, 90.0),
        ],
        Icon::Sun => &[
            C(12.0, 12.0, 4.0),
            L(12.0, 2.0, 12.0, 4.0),
            L(12.0, 20.0, 12.0, 22.0),
            L(2.0, 12.0, 4.0, 12.0),
            L(20.0, 12.0, 22.0, 12.0),
            L(4.93, 4.93, 6.34, 6.34),
            L(17.66, 17.66, 19.07, 19.07),
            L(4.93, 19.07, 6.34, 17.66),
            L(17.66, 6.34, 19.07, 4.93),
        ],
        Icon::Play => &[
            L(8.0, 5.0, 19.0, 12.0),
            L(19.0, 12.0, 8.0, 19.0),
            L(8.0, 19.0, 8.0, 5.0),
        ],
        Icon::Pause => &[L(9.0, 5.0, 9.0, 19.0), L(15.0, 5.0, 15.0, 19.0)],
        Icon::Film => &[
            R(3.0, 3.0, 18.0, 18.0, 2.0),
            L(7.0, 3.0, 7.0, 21.0),
            L(17.0, 3.0, 17.0, 21.0),
            L(3.0, 12.0, 21.0, 12.0),
            L(3.0, 7.5, 7.0, 7.5),
            L(3.0, 16.5, 7.0, 16.5),
            L(17.0, 7.5, 21.0, 7.5),
            L(17.0, 16.5, 21.0, 16.5),
        ],
        Icon::StepForward => STEP,
        Icon::StepBack => return (STEP, true),
        Icon::Minus => &[L(6.0, 12.0, 18.0, 12.0)],
        Icon::Plus => &[L(6.0, 12.0, 18.0, 12.0), L(12.0, 6.0, 12.0, 18.0)],
        Icon::Grip => &[
            D(9.0, 6.0, 1.4),
            D(15.0, 6.0, 1.4),
            D(9.0, 12.0, 1.4),
            D(15.0, 12.0, 1.4),
            D(9.0, 18.0, 1.4),
            D(15.0, 18.0, 1.4),
        ],
        Icon::Spacer => &[
            L(3.0, 6.0, 3.0, 18.0),
            L(21.0, 6.0, 21.0, 18.0),
            L(7.0, 12.0, 17.0, 12.0),
            L(7.0, 12.0, 10.0, 9.0),
            L(7.0, 12.0, 10.0, 15.0),
            L(17.0, 12.0, 14.0, 9.0),
            L(17.0, 12.0, 14.0, 15.0),
        ],
        Icon::Lock => &[
            R(3.0, 11.0, 18.0, 11.0, 2.0),
            L(7.0, 11.0, 7.0, 7.0),
            L(17.0, 11.0, 17.0, 7.0),
            A(12.0, 7.0, 5.0, 180.0, 180.0),
        ],
        Icon::Volume => &[
            L(11.0, 5.0, 6.0, 9.0),
            L(6.0, 9.0, 2.0, 9.0),
            L(2.0, 9.0, 2.0, 15.0),
            L(2.0, 15.0, 6.0, 15.0),
            L(6.0, 15.0, 11.0, 19.0),
            L(11.0, 19.0, 11.0, 5.0),
            A(12.0, 12.0, 5.0, 315.0, 90.0),
            A(12.0, 12.0, 9.0, 315.0, 90.0),
        ],
        Icon::Muted => &[
            L(11.0, 5.0, 6.0, 9.0),
            L(6.0, 9.0, 2.0, 9.0),
            L(2.0, 9.0, 2.0, 15.0),
            L(2.0, 15.0, 6.0, 15.0),
            L(6.0, 15.0, 11.0, 19.0),
            L(11.0, 19.0, 11.0, 5.0),
            L(16.0, 9.0, 22.0, 15.0),
            L(22.0, 9.0, 16.0, 15.0),
        ],
        Icon::Repeat => &[
            L(17.0, 2.0, 21.0, 6.0),
            L(21.0, 6.0, 17.0, 10.0),
            L(7.0, 6.0, 21.0, 6.0),
            A(7.0, 10.0, 4.0, 180.0, 90.0),
            L(3.0, 10.0, 3.0, 11.0),
            L(7.0, 22.0, 3.0, 18.0),
            L(3.0, 18.0, 7.0, 14.0),
            L(3.0, 18.0, 17.0, 18.0),
            A(17.0, 14.0, 4.0, 0.0, 90.0),
            L(21.0, 13.0, 21.0, 14.0),
        ],
        Icon::Subtitles => &[
            R(2.0, 4.0, 20.0, 16.0, 3.0),
            L(6.0, 12.0, 11.0, 12.0),
            L(13.0, 12.0, 18.0, 12.0),
            L(6.0, 16.0, 15.0, 16.0),
        ],
        Icon::SoundTracks => &[
            L(4.0, 10.0, 4.0, 14.0),
            L(8.0, 6.0, 8.0, 18.0),
            L(12.0, 3.0, 12.0, 21.0),
            L(16.0, 7.0, 16.0, 17.0),
            L(20.0, 10.0, 20.0, 14.0),
        ],
        Icon::Chapters => &[
            D(5.0, 6.0, 1.3),
            L(9.0, 6.0, 20.0, 6.0),
            D(5.0, 12.0, 1.3),
            L(9.0, 12.0, 20.0, 12.0),
            D(5.0, 18.0, 1.3),
            L(9.0, 18.0, 20.0, 18.0),
        ],
        Icon::MiniPlayer => &[R(2.0, 4.0, 20.0, 16.0, 2.0), R(12.0, 12.0, 7.0, 5.0, 1.0)],
        Icon::Shuffle => &[
            L(3.0, 18.0, 7.0, 18.0),
            L(7.0, 18.0, 15.0, 6.0),
            L(15.0, 6.0, 21.0, 6.0),
            L(18.0, 3.0, 21.0, 6.0),
            L(21.0, 6.0, 18.0, 9.0),
            L(3.0, 6.0, 7.0, 6.0),
            L(7.0, 6.0, 9.6, 9.9),
            L(12.4, 14.1, 15.0, 18.0),
            L(15.0, 18.0, 21.0, 18.0),
            L(18.0, 15.0, 21.0, 18.0),
            L(21.0, 18.0, 18.0, 21.0),
        ],
        Icon::RepeatOne => &[
            L(17.0, 2.0, 21.0, 6.0),
            L(21.0, 6.0, 17.0, 10.0),
            L(7.0, 6.0, 21.0, 6.0),
            A(7.0, 10.0, 4.0, 180.0, 90.0),
            L(3.0, 10.0, 3.0, 11.0),
            L(7.0, 22.0, 3.0, 18.0),
            L(3.0, 18.0, 7.0, 14.0),
            L(3.0, 18.0, 17.0, 18.0),
            A(17.0, 14.0, 4.0, 0.0, 90.0),
            L(21.0, 13.0, 21.0, 14.0),
            L(11.0, 10.5, 12.5, 9.5),
            L(12.5, 9.5, 12.5, 14.5),
        ],
        Icon::TrackNext => TRACK,
        Icon::TrackPrev => return (TRACK, true),
        Icon::Queue => &[
            L(3.0, 6.0, 15.0, 6.0),
            L(3.0, 12.0, 12.0, 12.0),
            L(3.0, 18.0, 11.0, 18.0),
            C(17.0, 17.5, 3.0),
            L(20.0, 17.5, 20.0, 4.0),
            L(20.0, 4.0, 22.5, 5.0),
        ],
        Icon::Download => &[
            L(12.0, 3.0, 12.0, 15.0),
            L(7.0, 10.0, 12.0, 15.0),
            L(17.0, 10.0, 12.0, 15.0),
            L(4.0, 15.0, 4.0, 20.0),
            L(4.0, 20.0, 20.0, 20.0),
            L(20.0, 20.0, 20.0, 15.0),
        ],
        Icon::Home => &[
            L(3.0, 10.0, 12.0, 3.0),
            L(12.0, 3.0, 21.0, 10.0),
            L(21.0, 10.0, 21.0, 19.0),
            A(19.0, 19.0, 2.0, 0.0, 90.0),
            L(19.0, 21.0, 5.0, 21.0),
            A(5.0, 19.0, 2.0, 90.0, 90.0),
            L(3.0, 19.0, 3.0, 10.0),
            L(9.0, 21.0, 9.0, 13.0),
            L(9.0, 13.0, 15.0, 13.0),
            L(15.0, 13.0, 15.0, 21.0),
        ],
        Icon::Sliders => &[
            L(21.0, 4.0, 14.0, 4.0),
            L(10.0, 4.0, 3.0, 4.0),
            L(21.0, 12.0, 12.0, 12.0),
            L(8.0, 12.0, 3.0, 12.0),
            L(21.0, 20.0, 16.0, 20.0),
            L(12.0, 20.0, 3.0, 20.0),
            L(14.0, 2.0, 14.0, 6.0),
            L(8.0, 10.0, 8.0, 14.0),
            L(16.0, 18.0, 16.0, 22.0),
        ],
        // Drawn by `draw_zynk`, in colour.
        Icon::Zynk => &[],
        // A filled mark (`DrawList::mark`).
        Icon::Discord => &[],
        Icon::Separator => &[
            L(12.0, 4.0, 12.0, 20.0),
            D(6.0, 12.0, 1.2),
            D(18.0, 12.0, 1.2),
        ],
        Icon::Moon => &[
            A(12.0, 12.0, 9.0, 300.0, 300.0),
            A(17.0, 7.0, 7.5, 90.0, 120.0),
        ],
    };
    (s, false)
}

/// Draws `icon` centred in `r` (logical px). `mirror` flips it horizontally (RTL arrows).
pub fn draw_icon(dl: &mut DrawList, icon: Icon, r: Rect, color: Color, stroke: f32, mirror: bool) {
    if icon == Icon::Zynk {
        draw_zynk(dl, r, color.a);
        return;
    }
    if icon == Icon::Discord {
        dl.mark(crate::Mark::Discord, r, color);
        return;
    }
    let (segs, m) = segs(icon);
    let mirror = mirror ^ m;
    let size = r.w.min(r.h);
    let k = size / 24.0;
    let ox = r.x + (r.w - size) * 0.5;
    let oy = r.y + (r.h - size) * 0.5;
    let tx = |x: f32| {
        if mirror {
            ox + (24.0 - x) * k
        } else {
            ox + x * k
        }
    };
    let ty = |y: f32| oy + y * k;
    let ang = |a: f32, sw: f32| -> f32 {
        let a = if mirror { 180.0 - a - sw } else { a };
        a.to_radians()
    };
    for s in segs {
        match *s {
            L(x0, y0, x1, y1) => dl.line((tx(x0), ty(y0)), (tx(x1), ty(y1)), stroke, color),
            A(cx, cy, rad, a0, sw) => dl.arc(
                (tx(cx), ty(cy)),
                rad * k,
                ang(a0, sw),
                sw.to_radians(),
                stroke,
                color,
            ),
            C(cx, cy, rad) => dl.arc(
                (tx(cx), ty(cy)),
                rad * k,
                0.0,
                std::f32::consts::TAU,
                stroke,
                color,
            ),
            R(x, y, w, h, rad) => {
                let x0 = if mirror { tx(x + w) } else { tx(x) };
                dl.stroke_rect(
                    Rect::new(
                        x0 - stroke * 0.5,
                        ty(y) - stroke * 0.5,
                        w * k + stroke,
                        h * k + stroke,
                    ),
                    rad * k + stroke * 0.5,
                    color,
                    stroke,
                );
            }
            D(cx, cy, rad) => dl.circle((tx(cx), ty(cy)), rad * k, color),
        }
    }
}

/// Zynk's mark as on its app icon and favicon (`zynk-landing/public/favicon.svg`, 512 units):
/// a white "Z" on a tile shading left to right from blue to navy. `alpha` fades it (a greyed
/// out row).
fn draw_zynk(dl: &mut DrawList, r: Rect, alpha: f32) {
    const FROM: Color = Color::hex(0x0557AB);
    const TO: Color = Color::hex(0x06165A);
    // The navy is reached 85% of the way across.
    const NAVY_AT: f32 = 0.85;
    let size = r.w.min(r.h) * (20.0 / 24.0);
    let tile = r.centered(size, size);
    let radius = size * 0.22;
    // The gradient as bands: the two outer ones (wider than the corners' radius) carry the
    // rounded corners, the middle is cut finer. Each overlaps the next, so no seam shows.
    const EDGES: [f32; 7] = [0.0, 0.25, 0.375, 0.5, 0.625, 0.75, 1.0];
    let n = EDGES.len() - 1;
    for i in 0..n {
        let (f0, f1) = (EDGES[i], EDGES[i + 1]);
        let k = ((f0 + f1) * 0.5 / NAVY_AT).min(1.0);
        let c = Color::rgba(
            FROM.r + (TO.r - FROM.r) * k,
            FROM.g + (TO.g - FROM.g) * k,
            FROM.b + (TO.b - FROM.b) * k,
            alpha,
        );
        let (first, last) = (i == 0, i + 1 == n);
        let x0 = tile.x + f0 * size;
        let x1 = if last {
            tile.right()
        } else {
            tile.x + f1 * size + 1.0
        };
        let left = if first { radius } else { 0.0 };
        let right = if last { radius } else { 0.0 };
        dl.rect_full(
            Rect::new(x0, tile.y, x1 - x0, tile.h),
            [left, right, right, left],
            c,
            Color::TRANSPARENT,
            0.0,
        );
    }
    // The Z: its two bars and the diagonal between their ends, as round-capped strokes as
    // thick as the logo's (96 of 512).
    let at = |x: f32, y: f32| (tile.x + x / 512.0 * size, tile.y + y / 512.0 * size);
    let width = size * 96.0 / 512.0;
    let white = Color::rgba(1.0, 1.0, 1.0, alpha);
    dl.line(at(187.0, 154.0), at(322.0, 154.0), width, white);
    dl.line(at(322.0, 154.0), at(185.0, 358.0), width, white);
    dl.line(at(185.0, 358.0), at(327.0, 358.0), width, white);
}
