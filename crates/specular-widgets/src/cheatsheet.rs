//! The keyboard cheat sheet (`?`): every listed key with what it does, grouped, one tab per
//! place with keys of its own (the viewer, the gallery, compare, cull mode, medical images).
//! It opens on the tab of what is shown; the others are a click, Tab or ←/→ away.
//!
//! The keys come from the live keymap, so rebinds show. The gallery's and compare's own keys
//! are fixed; they come from the tables their handlers run from
//! ([`specular_input::fixed`]), so the sheet and the handlers cannot drift apart.

use specular_gfx::Rect;
use specular_i18n::{I18n, t};
use specular_input::fixed::{self, FixedKey};
use specular_input::{Action, Group, Key, KeySeq, Keymap, Mode};
use specular_theme::metrics;

use crate::ctx::{Ctx, HAlign, Target};
use crate::{Chrome, ViewerState};

/// A line: its label and keys.
type Line = (String, Vec<String>);
/// A group title with its lines.
type GroupLines = (String, Vec<Line>);

/// A tab of the cheat sheet: a place with keys of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SheetMode {
    Viewer,
    Gallery,
    Compare,
    Cull,
    Medical,
    Science,
    Drawing,
    Mesh,
    Relight,
    Video,
    Map,
    Develop,
}

impl SheetMode {
    /// In tab order.
    pub const ALL: [SheetMode; 12] = [
        SheetMode::Viewer,
        SheetMode::Gallery,
        SheetMode::Compare,
        SheetMode::Cull,
        SheetMode::Medical,
        SheetMode::Science,
        SheetMode::Drawing,
        SheetMode::Mesh,
        SheetMode::Relight,
        SheetMode::Video,
        SheetMode::Map,
        SheetMode::Develop,
    ];

    /// The tab's name.
    pub fn label(self) -> String {
        match self {
            SheetMode::Viewer => t!("keys-mode-viewer"),
            SheetMode::Gallery => t!("cheatsheet-mode-gallery"),
            SheetMode::Compare => t!("cheatsheet-mode-compare"),
            SheetMode::Cull => t!("keys-mode-cull"),
            SheetMode::Medical => t!("keys-mode-medical"),
            SheetMode::Science => t!("keys-mode-science"),
            SheetMode::Drawing => t!("keys-mode-drawing"),
            SheetMode::Mesh => t!("keys-mode-mesh"),
            SheetMode::Relight => t!("keys-mode-relight"),
            SheetMode::Video => t!("keys-mode-video"),
            SheetMode::Map => t!("keys-mode-map"),
            SheetMode::Develop => t!("keys-mode-develop"),
        }
    }

    /// The tabs the profile shows ([`Keymap::profile`]): medical images' and 3D meshes' only
    /// while their keys work, drawings' only while a drawing is shown.
    pub fn shown(km: &Keymap) -> Vec<SheetMode> {
        SheetMode::ALL
            .into_iter()
            .filter(|m| *m != SheetMode::Medical || km.mode_shown(Mode::Medical))
            .filter(|m| *m != SheetMode::Drawing || km.active().contains(&Mode::Drawing))
            .filter(|m| *m != SheetMode::Mesh || km.mode_shown(Mode::Mesh))
            .filter(|m| *m != SheetMode::Relight || km.active().contains(&Mode::Relight))
            .filter(|m| *m != SheetMode::Video || km.active().contains(&Mode::Video))
            .filter(|m| *m != SheetMode::Map || km.active().contains(&Mode::Map))
            .filter(|m| *m != SheetMode::Develop || km.active().contains(&Mode::Develop))
            .collect()
    }
}

impl Chrome {
    /// The tab for what is shown: compare, the gallery, cull mode, a medical image, else the
    /// viewer.
    pub fn cheat_context(&self, km: &Keymap) -> SheetMode {
        if self.gallery && self.comparing {
            SheetMode::Compare
        } else if self.gallery {
            SheetMode::Gallery
        } else if km.active().contains(&Mode::Cull) {
            SheetMode::Cull
        } else if km.active().contains(&Mode::Develop) {
            SheetMode::Develop
        } else if km.active().contains(&Mode::Medical) && km.mode_shown(Mode::Medical) {
            SheetMode::Medical
        } else if km.active().contains(&Mode::Map) {
            SheetMode::Map
        } else if km.active().contains(&Mode::Science) {
            SheetMode::Science
        } else if km.active().contains(&Mode::Drawing) {
            SheetMode::Drawing
        } else if km.active().contains(&Mode::Mesh) && km.mode_shown(Mode::Mesh) {
            SheetMode::Mesh
        } else if km.active().contains(&Mode::Relight) {
            SheetMode::Relight
        } else if km.active().contains(&Mode::Video) {
            SheetMode::Video
        } else {
            SheetMode::Viewer
        }
    }

    /// The tab the cheat sheet shows.
    pub fn cheat_shown(&self, km: &Keymap) -> SheetMode {
        self.cheat_tab
            .filter(|m| SheetMode::shown(km).contains(m))
            .unwrap_or_else(|| self.cheat_context(km))
    }

    pub(crate) fn cheat_pick(&mut self, m: SheetMode) {
        if self.cheat_tab != Some(m) {
            self.cheat_scroll = 0.0;
        }
        self.cheat_tab = Some(m);
    }

    /// ←/→: the next or previous tab (round the ends); a focused tab's focus follows.
    pub(crate) fn cheat_step(&mut self, next: bool, st: &ViewerState) {
        let all = SheetMode::shown(st.keymap);
        let i = all
            .iter()
            .position(|m| *m == self.cheat_shown(st.keymap))
            .unwrap_or(0);
        let n = all.len();
        let m = all[if next { (i + 1) % n } else { (i + n - 1) % n }];
        self.cheat_pick(m);
        if matches!(self.modal_focus, Some(Target::CheatTab(_))) {
            self.modal_focus = Some(Target::CheatTab(m));
        }
    }
}

/// The tabs laid out from the start edge, wrapping onto more rows when they do not fit
/// (narrow windows, large text): (tab, rectangle before mirroring), and the rows' bottom.
fn tab_rects(
    cx: &mut Ctx,
    km: &Keymap,
    x0: f32,
    y0: f32,
    w: f32,
) -> (Vec<(SheetMode, Rect, String)>, f32) {
    let h = cx.u(30.0);
    let gap = 6.0;
    let style = cx.style(metrics::FONT_LABEL).weight(600.0);
    let (mut x, mut y) = (x0, y0);
    let mut out = Vec::new();
    for m in SheetMode::shown(km) {
        let label = m.label();
        let tw = (cx.layout(&label, style, None).width + 28.0).min(w);
        if x > x0 && x + tw > x0 + w {
            x = x0;
            y += h + gap;
        }
        out.push((m, Rect::new(x, y, tw, h), label));
        x += tw + gap;
    }
    (out, y + h)
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx, st: &ViewerState) {
    // Columns narrow as the text grows: three at the standard size in a wide window.
    let w = (cx.w - 48.0).min(cx.u(940.0));
    let g = cx.grow();
    let cols = if w > 820.0 * g {
        3
    } else if w > 520.0 * g {
        2
    } else {
        1
    };
    let line = cx.u(28.0);
    let head = cx.u(34.0);
    let shown = ch.cheat_shown(st.keymap);
    let groups = groups_for(shown, st.keymap);
    ch.cheat_tabs = SheetMode::shown(st.keymap);
    // Balance groups over the columns in order.
    let heights: Vec<f32> = groups
        .iter()
        .map(|(_, l)| head + l.len() as f32 * line + 14.0)
        .collect();
    let total: f32 = heights.iter().sum();
    let target = total / cols as f32;
    let mut col_of = Vec::new();
    let (mut c, mut acc) = (0usize, 0.0f32);
    for hgt in &heights {
        if acc > 0.0 && acc + hgt * 0.5 > target && c + 1 < cols {
            c += 1;
            acc = 0.0;
        }
        col_of.push(c);
        acc += hgt;
    }
    let mut col_h = vec![0.0f32; cols];
    for (i, hgt) in heights.iter().enumerate() {
        col_h[col_of[i]] += hgt;
    }
    let content_h = col_h.iter().copied().fold(0.0, f32::max);
    let title_h = cx.u(64.0);
    // The tabs, under the title (laid out at the top for now; moved once the sheet is placed).
    let (tabs, tabs_bottom) = tab_rects(cx, st.keymap, 24.0, 0.0, w - 48.0);
    let tabs_h = tabs_bottom + 14.0;
    let foot_h = cx.u(40.0);
    let h = (title_h + tabs_h + content_h + foot_h).min(cx.h - 48.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);

    let ts = cx.style(metrics::FONT_LARGE).weight(650.0);
    let tr = Rect::new(r.x + 24.0, r.y + 14.0, r.w - 80.0, title_h - 28.0);
    let tr = cx.m_in(tr, r);
    cx.label_fit(
        &t!("cheatsheet-title"),
        ts,
        tr,
        HAlign::Start,
        cx.pal.text,
        false,
    );
    let close = cx.m_in(Rect::new(r.right() - 48.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);

    // The tabs (clipped to the sheet when a tiny window cuts them off).
    cx.dl
        .push_clip(Rect::new(r.x, r.y, r.w, (h - foot_h).max(0.0)));
    for (m, tr, label) in &tabs {
        let tr = cx.m_in(
            Rect::new(r.x + tr.x, r.y + title_h + tr.y - 8.0, tr.w, tr.h),
            r,
        );
        let tgt = Target::CheatTab(*m);
        let on = *m == shown;
        let fill = if on {
            cx.pal.accent_soft
        } else if cx.is_hover(&tgt) {
            cx.pal.surface_hover
        } else {
            cx.pal.surface_raised
        };
        let border = if on { fill } else { cx.pal.separator };
        cx.dl
            .rect_border(tr, metrics::RADIUS_SM + 1.0, fill, border, 1.0);
        let style = cx
            .style(metrics::FONT_LABEL)
            .weight(if on { 600.0 } else { 450.0 });
        let color = if on { cx.pal.text } else { cx.pal.text_muted };
        cx.label_fit(
            label,
            style,
            tr.inset_xy(10.0, 0.0),
            HAlign::Center,
            color,
            false,
        );
        cx.ring_if(&tgt, tr, metrics::RADIUS_SM + 1.0);
        cx.hit(tr, tgt);
    }
    cx.dl.pop_clip();

    let body_y = r.y + title_h + tabs_h;
    let body = Rect::new(r.x, body_y, r.w, (r.bottom() - foot_h - body_y).max(0.0));
    let max = (content_h - body.h).max(0.0);
    ch.cheat_scroll = ch.cheat_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    ch.cheat_cut.clear();
    #[cfg(test)]
    ch.cheat_slots.clear();
    let col_w = (r.w - 48.0 - 32.0 * (cols - 1) as f32) / cols as f32;
    let mut ys = vec![body.y - ch.cheat_scroll; cols];
    let hs = cx.style(metrics::FONT_SMALL).weight(650.0);
    let ls = cx.style(metrics::FONT_BODY);
    for (gi, (title, lines)) in groups.iter().enumerate() {
        let c = col_of[gi];
        let x = r.x + 24.0 + c as f32 * (col_w + 32.0);
        let col = cx.m_in(Rect::new(x, body.y, col_w, body.h), r);
        let y = &mut ys[c];
        #[cfg(test)]
        ch.cheat_slots
            .push((Rect::new(col.x, *y, col.w, head - 8.0), title.clone(), None));
        cx.label_fit(
            title,
            hs,
            Rect::new(col.x, *y, col.w, head - 8.0),
            HAlign::Start,
            cx.pal.text_faint,
            false,
        );
        cx.dl.rect(
            Rect::new(col.x, *y + head - 6.0, col.w, 1.0),
            0.0,
            cx.pal.separator,
        );
        *y += head;
        for (li, (label, keys)) in lines.iter().enumerate() {
            let row = Rect::new(col.x, *y, col.w, line);
            // A line without keys is an action reached through the command palette: muted,
            // with a "Palette" tag in the accent instead of keycaps.
            let palette = keys.is_empty();
            let mut kw = 0.0;
            for k in keys.iter().rev() {
                kw += cx.keycap_end(k, row, kw) + 4.0;
            }
            if palette {
                kw = palette_tag(cx, row) + 4.0;
            }
            let lr = cx.m_in(
                Rect::new(row.x, row.y, (row.w - kw - 8.0).max(0.0), row.h),
                row,
            );
            let color = if palette {
                cx.pal.text_muted
            } else {
                cx.pal.text
            };
            #[cfg(test)]
            ch.cheat_slots.push((lr, label.clone(), Some((gi, li))));
            cx.dl.push_clip(lr);
            cx.label_fit(label, ls, lr, HAlign::Start, color, false);
            cx.dl.pop_clip();
            // Cut short ("…"): the whole line shows in a tooltip (screen readers get it
            // whole from the tree).
            if cx.text.measure(label, ls) > lr.w {
                let tgt = Target::CheatLine(shown, gi as u16, li as u16);
                let vis = lr.intersect(&body);
                if vis.h > 0.0 {
                    cx.hit(vis, tgt);
                }
                ch.cheat_cut.push((shown, gi, li));
            }
            *y += line;
        }
        *y += 14.0;
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(ch, cx, crate::scrollbar::Area::CheatSheet, body, r, max);
    let key = st
        .keymap
        .primary_key(Action::CheatSheet)
        .map(|k| k.to_string())
        .unwrap_or_else(|| "?".into());
    let fr = Rect::new(r.x, r.bottom() - foot_h, r.w, foot_h - 6.0);
    let fs = cx.style(metrics::FONT_SMALL);
    cx.label(
        &t!("cheatsheet-close-hint", key = key),
        fs,
        fr,
        HAlign::Center,
        cx.pal.text_faint,
    );
}

/// Tests: a group title (`None`) or line (group, line) drawn, with its slot.
#[cfg(test)]
pub(crate) type Slot = (Rect, String, Option<(usize, usize)>);

/// A line of tab `m` as a whole sentence ("Pick: P"), for its tooltip when it is cut short.
pub(crate) fn line_text(m: SheetMode, km: &Keymap, group: usize, line: usize) -> Option<String> {
    let groups = groups_for(m, km);
    let (label, keys) = groups.get(group)?.1.get(line)?;
    Some(spoken_line(km, label, keys))
}

/// The tag at the end of a palette-only line (where keycaps go): a rounded pill outlined in
/// the accent, so it does not read as a key. Returns its width.
fn palette_tag(cx: &mut Ctx, row: Rect) -> f32 {
    let style = cx.style(metrics::FONT_SMALL).weight(550.0);
    let b = cx.layout(&t!("cheatsheet-palette-only-tag"), style, None);
    let w = b.width + 14.0;
    let x = if cx.rtl { row.x } else { row.right() - w };
    let h = cx.u(20.0);
    let r = Rect::new(x, row.center().1 - h * 0.5, w, h);
    cx.dl
        .rect_border(r, h * 0.5, cx.pal.accent_soft, cx.pal.accent, 1.0);
    cx.text.draw(
        cx.dl,
        &b,
        r.x + 7.0,
        r.y + (r.h - b.height) * 0.5,
        cx.pal.text,
    );
    w
}

/// The shown tab's groups with their lines, in display order (drawn, and read out by screen
/// readers).
pub(crate) fn groups(ch: &Chrome, st: &ViewerState) -> Vec<GroupLines> {
    groups_for(ch.cheat_shown(st.keymap), st.keymap)
}

/// A tab's groups with their lines. The last group may hold the tab's important actions that
/// no key runs: their lines have no keys and read as "in the command palette".
pub(crate) fn groups_for(m: SheetMode, km: &Keymap) -> Vec<GroupLines> {
    let mut g = key_groups_for(m, km);
    g.extend(palette_group(m, km));
    g
}

/// Actions worth knowing on a tab that have no key by default. While nothing binds them the
/// tab lists them as reached through the command palette; a key bound to one moves it to its
/// usual group.
pub(crate) fn palette_only(m: SheetMode) -> &'static [Action] {
    match m {
        SheetMode::Medical => &[
            Action::Studies,
            Action::DicomServers,
            Action::ExportAnonymised,
            Action::ExportPatientDisc,
            Action::ExportTeachingFile,
            Action::TeachingQuiz,
            Action::SendToPacs,
        ],
        SheetMode::Video => &[Action::GenerateSubtitles, Action::SaveSubtitles],
        _ => &[],
    }
}

/// The keymap layer a tab lists.
fn sheet_layer(m: SheetMode) -> Mode {
    match m {
        SheetMode::Cull => Mode::Cull,
        SheetMode::Medical => Mode::Medical,
        SheetMode::Science => Mode::Science,
        SheetMode::Drawing => Mode::Drawing,
        SheetMode::Mesh => Mode::Mesh,
        SheetMode::Relight => Mode::Relight,
        SheetMode::Video => Mode::Video,
        SheetMode::Map => Mode::Map,
        SheetMode::Develop => Mode::Develop,
        SheetMode::Viewer | SheetMode::Gallery | SheetMode::Compare => Mode::Viewer,
    }
}

/// The key that opens the command palette, as shown.
pub(crate) fn palette_key(km: &Keymap) -> Option<String> {
    km.primary_key(Action::Palette).map(|k| k.to_string())
}

/// The tab's "in the command palette" group: [`palette_only`] actions the profile shows and
/// no key runs on the tab.
fn palette_group(m: SheetMode, km: &Keymap) -> Option<GroupLines> {
    let mode = sheet_layer(m);
    let lines: Vec<Line> = palette_only(m)
        .iter()
        .filter(|a| km.shows(**a) && mode_keys(km, mode, **a).is_empty())
        .map(|a| (a.name(), Vec::new()))
        .collect();
    if lines.is_empty() {
        return None;
    }
    let title = match palette_key(km) {
        Some(key) => t!("cheatsheet-palette-only-title", key = key),
        None => t!("cheatsheet-palette-only-title-nokey"),
    };
    Some((title, lines))
}

/// What a screen reader says for a line: "Pick: P", or for an action without a key, that it
/// is in the command palette.
pub(crate) fn spoken_line(km: &Keymap, label: &str, keys: &[String]) -> String {
    if !keys.is_empty() {
        return format!("{label}: {}", keys.join(", "));
    }
    match palette_key(km) {
        Some(key) => t!(
            "cheatsheet-palette-only-line",
            action = label.to_owned(),
            key = key
        ),
        None => t!(
            "cheatsheet-palette-only-line-nokey",
            action = label.to_owned()
        ),
    }
}

/// A tab's groups of keys.
fn key_groups_for(m: SheetMode, km: &Keymap) -> Vec<GroupLines> {
    match m {
        SheetMode::Viewer => viewer_groups(km),
        SheetMode::Gallery => gallery_groups(km),
        SheetMode::Compare => compare_groups(km),
        SheetMode::Cull => cull_groups(km),
        SheetMode::Medical => medical_groups(km),
        SheetMode::Science => science_groups(km),
        SheetMode::Drawing => drawing_groups(km),
        SheetMode::Mesh => mesh_groups(km),
        SheetMode::Relight => relight_groups(km),
        SheetMode::Video => video_groups(km),
        SheetMode::Map => map_groups(km),
        SheetMode::Develop => develop_groups(km),
    }
}

/// Develop's keys: before and after, the panel, speed edit.
fn develop_groups(km: &Keymap) -> Vec<GroupLines> {
    action_groups(
        km,
        Mode::Develop,
        &[
            (
                t!("keys-mode-develop"),
                &[
                    Action::ToggleDevelop,
                    Action::DevelopBeforeAfter,
                    Action::DevelopSplit,
                    Action::DevelopFocusPanel,
                    Action::DevelopNextRegion,
                    Action::DevelopPrevRegion,
                    Action::DevelopExport,
                    Action::DevelopCrop,
                    Action::DevelopSpotOutlines,
                    Action::DevelopSpotCrosshair,
                    Action::DevelopScopes,
                    Action::DevelopClipDisplay,
                    Action::DevelopClipScene,
                    Action::DevelopClipGamut,
                    Action::DevelopSoftProof,
                    Action::DevelopSnapshot,
                    Action::DevelopMaskOverlay,
                    Action::DevelopBrushSmaller,
                    Action::DevelopBrushLarger,
                    Action::DevelopBrushSofter,
                    Action::DevelopBrushHarder,
                    Action::DevelopBrushErase,
                    Action::Undo,
                    Action::Redo,
                ],
            ),
            (
                t!("cheatsheet-develop-speed"),
                &specular_input::DEVELOP_SPEED,
            ),
        ],
    )
}

/// Maps' keys: layers, coordinates, measuring.
fn map_groups(km: &Keymap) -> Vec<GroupLines> {
    action_groups(
        km,
        Mode::Map,
        &[
            (
                t!("cheatsheet-map-layers"),
                &[
                    Action::MapLayers,
                    Action::MapAddLayer,
                    Action::MapZoomToLayer,
                    Action::MapAttributeTable,
                    Action::MapOverview,
                ],
            ),
            (
                t!("cheatsheet-map-coordinates"),
                &[
                    Action::MapCoordinateFormat,
                    Action::MapCopyCoordinate,
                    Action::MapGoToCoordinate,
                    Action::MapUnits,
                ],
            ),
            (
                t!("cheatsheet-map-measure"),
                &[
                    Action::MapMeasureDistance,
                    Action::MapMeasureArea,
                    Action::MapElevationProfile,
                ],
            ),
            (
                t!("cheatsheet-map-terrain"),
                &[
                    Action::MapSlope,
                    Action::MapAspect,
                    Action::MapContours,
                    Action::MapTerrain3d,
                ],
            ),
            (
                t!("cheatsheet-map-time"),
                &[
                    Action::MapTime,
                    Action::MapPlay,
                    Action::MapWind,
                    Action::MapWeatherColours,
                ],
            ),
            (t!("cheatsheet-map-online"), &[Action::MapServices]),
            (
                t!("cheatsheet-map-photos"),
                &[Action::MapPhotosOnTrack, Action::MapGeotag],
            ),
            (
                t!("cheatsheet-map-output"),
                &[Action::MapExport, Action::MapPrint, Action::MapCopyExtent],
            ),
            (
                t!("cheatsheet-view"),
                &[Action::ZoomFit, Action::ZoomRectangle, Action::Measure],
            ),
        ],
    )
}

/// Movies' keys: playing, seeking and changing file, the sound, speed and looping.
fn video_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut groups = action_groups(
        km,
        Mode::Video,
        &[
            (
                t!("keys-mode-video"),
                &[
                    Action::PlayPauseVideo,
                    Action::SeekBack,
                    Action::SeekForward,
                    Action::SeekBackLong,
                    Action::SeekForwardLong,
                    Action::PrevFrame,
                    Action::NextFrame,
                    Action::PrevChapter,
                    Action::NextChapter,
                    Action::AbLoop,
                    Action::GoToTime,
                    Action::VideoFullscreen,
                    Action::MiniPlayer,
                    Action::CastMenu,
                    Action::PrevImage,
                    Action::NextImage,
                ],
            ),
            (
                t!("cheatsheet-video-sound"),
                &[
                    Action::VolumeUp,
                    Action::VolumeDown,
                    Action::ToggleMute,
                    Action::CycleSound,
                    Action::CycleSoundVisuals,
                    Action::StreamQuality,
                    Action::DialogueBoost,
                    Action::Equaliser,
                    Action::ToggleLyrics,
                ],
            ),
            (
                t!("cheatsheet-video-subtitles"),
                &[
                    Action::CycleSubtitles,
                    Action::SubtitleEarlier,
                    Action::SubtitleLater,
                    Action::SubtitleEarlierFine,
                    Action::SubtitleLaterFine,
                    Action::SubtitleStepPrev,
                    Action::SubtitleStepNext,
                    Action::SubtitleUp,
                    Action::SubtitleDown,
                    Action::SubtitleBigger,
                    Action::SubtitleSmaller,
                    Action::SubtitleResetPlace,
                    Action::SoundEarlier,
                    Action::SoundLater,
                    Action::SoundEarlierFine,
                    Action::SoundLaterFine,
                    Action::ResetDelays,
                ],
            ),
            (
                t!("cheatsheet-video-queue"),
                &[
                    Action::PrevTrack,
                    Action::NextTrack,
                    Action::CycleRepeat,
                    Action::ToggleShuffle,
                    Action::ToggleQueue,
                ],
            ),
            (
                t!("cheatsheet-video-picture"),
                &[
                    Action::CopyFrame,
                    Action::SaveFrame,
                    Action::CycleAspect,
                    Action::CycleCrop,
                    Action::VideoPicture,
                    Action::RotateLeft,
                    Action::RotateRight,
                    Action::FlipHorizontal,
                    Action::FlipVertical,
                    Action::ZoomIn,
                    Action::ZoomOut,
                    Action::ZoomFit,
                ],
            ),
            (
                t!("cheatsheet-video-speed"),
                &[
                    Action::SlowerPlayback,
                    Action::FasterPlayback,
                    Action::NormalPlayback,
                    Action::ToggleLoopForever,
                ],
            ),
        ],
    );
    // The jumps to a tenth of the movie as one run ("0–9").
    let keys: Vec<String> = (0..10u8)
        .map(|n| Action::SeekPercent(n * 10))
        .filter(|a| km.shows(*a))
        .filter_map(|a| mode_keys(km, Mode::Video, a).into_iter().next())
        .collect();
    if let Some(run) = digit_run(&keys)
        && let Some((_, lines)) = groups.first_mut()
    {
        lines.insert(3, (t!("cheatsheet-video-seek-percent"), run));
    }
    groups
}

/// The keys that run `a` in `mode` (its own layer, then its parents' where the inner layers
/// leave the key free), at most two, as shown.
pub(crate) fn mode_keys(km: &Keymap, mode: Mode, a: Action) -> Vec<String> {
    let chain = mode.chain();
    let mut v: Vec<KeySeq> = Vec::new();
    for (i, m) in chain.iter().enumerate() {
        for s in km.keys_in(*m, a) {
            let free = |o: &Mode| km.get(*o, &s).is_none_or(|b| !km.shows(b));
            if chain[..i].iter().all(free) && !v.contains(&s) {
                v.push(s);
            }
        }
    }
    let keys = v.iter().take(2).map(|k| k.to_string());
    if a.is_hold() {
        keys.map(|k| t!("cheatsheet-hold", key = k)).collect()
    } else {
        keys.collect()
    }
}

/// An action's line in `mode`, when something runs it there and the profile shows it.
fn action_line(km: &Keymap, mode: Mode, a: Action) -> Option<(String, Vec<String>)> {
    if !km.shows(a) {
        return None;
    }
    let keys = mode_keys(km, mode, a);
    (!keys.is_empty()).then(|| (a.name(), keys))
}

/// Groups of actions in `mode`, leaving out empty groups.
fn action_groups(km: &Keymap, mode: Mode, groups: &[(String, &[Action])]) -> Vec<GroupLines> {
    groups
        .iter()
        .map(|(title, acts)| {
            let lines = acts
                .iter()
                .filter_map(|a| action_line(km, mode, *a))
                .collect();
            (title.clone(), lines)
        })
        .filter(|(_, l): &GroupLines| !l.is_empty())
        .collect()
}

/// Viewer keys that are not palette commands: panning, Esc and reload.
const VIEWER_EXTRA: [Action; 10] = [
    Action::PanLeft,
    Action::PanRight,
    Action::PanUp,
    Action::PanDown,
    Action::PanPageLeft,
    Action::PanPageRight,
    Action::PanPageUp,
    Action::PanPageDown,
    Action::Reload,
    Action::Back,
];

/// The viewer's keys: every listed action the viewer's own layer binds, by group, and the
/// keys that are not commands (panning, Esc). The other modes' layers have their own tabs;
/// their actions show here when the viewer binds them too (`b` debayers wherever a colour
/// camera's frame is shown, Ctrl+Shift+K enters cull mode).
fn viewer_groups(km: &Keymap) -> Vec<GroupLines> {
    let i18n = I18n::global();
    Group::ALL
        .into_iter()
        .map(|g| {
            let lines = Action::LISTED
                .iter()
                .chain(VIEWER_EXTRA.iter())
                .filter(|a| a.group() == g)
                .filter_map(|a| {
                    let (label, keys) = action_line(km, Mode::Viewer, *a)?;
                    // Esc's palette name is a whole sentence, too long for the sheet.
                    let label = if *a == Action::Back {
                        t!("cheatsheet-back")
                    } else {
                        label
                    };
                    Some((label, keys))
                })
                .collect();
            (i18n.tr(g.message_id()), lines)
        })
        .filter(|(_, l): &GroupLines| !l.is_empty())
        .collect()
}

/// A fixed line's keys as shown: a counting run of digits as one ("1–5"), else the listed
/// keys; "Hold Q" for keys that work while held.
fn fixed_keys<C: Copy>(l: &FixedKey<C>) -> Vec<String> {
    let all: Vec<String> = l.combos().map(|c| c.to_string()).collect();
    let keys = match digit_run(&all) {
        Some(run) if run.len() == 1 && all.len() > 1 => run,
        _ => l.listed_combos().map(|c| c.to_string()).collect(),
    };
    if l.hold {
        keys.into_iter()
            .map(|k| t!("cheatsheet-hold", key = k))
            .collect()
    } else {
        keys
    }
}

/// A fixed table's lines under their headings, in table order.
fn fixed_groups<C: Copy>(
    table: &'static [FixedKey<C>],
    mut extra: impl FnMut(&FixedKey<C>) -> Vec<String>,
) -> Vec<GroupLines> {
    let i18n = I18n::global();
    let mut out: Vec<(&str, Vec<Line>)> = Vec::new();
    for l in table {
        let mut keys = fixed_keys(l);
        for k in extra(l) {
            if !keys.contains(&k) && keys.len() < 3 {
                keys.push(k);
            }
        }
        let item = (i18n.tr(l.label), keys);
        match out.iter_mut().find(|(g, _)| *g == l.group) {
            Some((_, lines)) => lines.push(item),
            None => out.push((l.group, vec![item])),
        }
    }
    out.into_iter()
        .map(|(g, lines)| (i18n.tr(g), lines))
        .collect()
}

/// The viewer action the gallery also runs for a line of its own keys: its keymap keys
/// (rebinds included) work in the gallery too, so they are listed with the fixed ones.
fn gallery_action(cmd: fixed::GalleryCmd) -> Option<Action> {
    use fixed::GalleryCmd as G;
    Some(match cmd {
        G::Viewer => Action::ToggleGallery,
        G::Back => Action::Back,
        G::Trash => Action::MoveToTrash,
        G::CopyImage => Action::CopyImage,
        G::CopyPaths => Action::CopyPath,
        G::Refresh => Action::Reload,
        G::Bigger => Action::ZoomIn,
        G::Smaller => Action::ZoomOut,
        _ => return None,
    })
}

/// Keymap keys of `a` that reach it in the gallery (the line `cmd` does the same): not those
/// the gallery's own keys take for something else, nor plain letters (they type a name to
/// jump to).
fn gallery_keymap_keys(km: &Keymap, a: Action, cmd: fixed::GalleryCmd) -> Vec<String> {
    km.keys_in(Mode::Viewer, a)
        .into_iter()
        .filter(|s| {
            let c = s.first();
            let typed =
                matches!(c.key, Key::Char(_)) && !c.mods.ctrl && !c.mods.alt && !c.mods.meta;
            !typed && fixed::find(fixed::GALLERY_KEYS, &c).is_none_or(|l| l.cmd == cmd)
        })
        .map(|s| s.to_string())
        .collect()
}

/// The gallery's keys: its own, with the viewer's that also work there.
fn gallery_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut groups = fixed_groups(fixed::GALLERY_KEYS, |l| {
        gallery_action(l.cmd)
            .map(|a| gallery_keymap_keys(km, a, l.cmd))
            .unwrap_or_default()
    });
    // The library and undo come from the keymap.
    let i18n = I18n::global();
    let browse = i18n.tr("cheatsheet-gallery-browse");
    let file = i18n.tr("cheatsheet-file");
    for (group, a) in [(browse, Action::ToggleLibrary), (file, Action::Undo)] {
        if let Some(l) = action_line(km, Mode::Viewer, a)
            && let Some((_, lines)) = groups.iter_mut().find(|(g, _)| *g == group)
        {
            lines.push(l);
        }
    }
    groups
}

/// Compare's keys, and the channel views (the viewer's keys).
fn compare_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut groups = fixed_groups(fixed::COMPARE_KEYS, |_| Vec::new());
    groups.extend(action_groups(
        km,
        Mode::Viewer,
        &[(
            t!("cheatsheet-channels"),
            &[
                Action::ChannelRed,
                Action::ChannelGreen,
                Action::ChannelBlue,
                Action::ChannelAlpha,
                Action::ChannelLuma,
            ],
        )],
    ));
    groups
}

/// Cull mode's keys: in and out, the marks, the folder slots, bursts, following new files.
fn cull_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut lines: Vec<_> = action_line(km, Mode::Cull, Action::ToggleCull)
        .into_iter()
        .collect();
    lines.extend(cull_lines(km));
    lines.extend(action_line(km, Mode::Cull, Action::Undo));
    let mut groups = vec![(t!("cheatsheet-cull"), lines)];
    groups.extend(action_groups(
        km,
        Mode::Cull,
        &[(
            t!("cheatsheet-navigation"),
            &[
                Action::NextImage,
                Action::PrevImage,
                Action::FirstImage,
                Action::LastImage,
            ],
        )],
    ));
    groups.retain(|(_, l)| !l.is_empty());
    groups
}

/// Medical images' keys: window/level and its presets, measuring and overlays, frames and
/// series, and the views that go with them.
fn medical_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut window = vec![Action::WindowLevelTool, Action::InvertGrey];
    window.extend((0..=9).map(Action::WindowPreset));
    action_groups(
        km,
        Mode::Medical,
        &[
            (t!("cheatsheet-medical-window"), &window),
            (
                t!("cheatsheet-medical-measure"),
                &[
                    Action::MeasureLength,
                    Action::MeasureAngle,
                    Action::MeasureEllipse,
                    Action::MeasureRectangle,
                    Action::ClearMeasurements,
                    Action::ToggleOverlays,
                ],
            ),
            (
                t!("cheatsheet-medical-frames"),
                &[
                    Action::NextSlice,
                    Action::PrevSlice,
                    Action::NextPage,
                    Action::PrevPage,
                    Action::FirstPage,
                    Action::LastPage,
                    Action::Cine,
                    Action::NextSeries,
                    Action::PrevSeries,
                    Action::CompareSeries,
                    Action::CompareLayout,
                    Action::NextViewport,
                    Action::PrevViewport,
                    Action::ReferenceLines,
                    Action::SyncWindow,
                    Action::SyncZoom,
                    Action::LinkSeries,
                    Action::AlignSeries,
                ],
            ),
            (
                t!("cheatsheet-medical-3d"),
                &[
                    Action::Mpr,
                    Action::MprSlab,
                    Action::MprTractSlab,
                    Action::MprPreset,
                    Action::MprRender,
                    Action::MprClip,
                    Action::MprMaximise,
                    Action::MprExportPlanes,
                    Action::MprCurve,
                ],
            ),
            (
                t!("cheatsheet-medical-analysis"),
                &[
                    Action::Segments,
                    Action::SegmentOpacity,
                    Action::Fusion,
                    Action::FusionColormap,
                    Action::FusionBlend,
                    Action::FusionNudge,
                    Action::Tmtv,
                    Action::TmtvThreshold,
                    Action::TimeCurve,
                    Action::ExportCurve,
                ],
            ),
            (
                t!("cheatsheet-view"),
                &[
                    Action::ToggleOrientationLabels,
                    Action::NextPresentationState,
                    Action::DicomTags,
                    Action::ZoomFit,
                    Action::ZoomTo(100.0),
                    Action::ZoomRectangle,
                ],
            ),
            // Without keys by default (then listed as in the command palette); here once
            // something binds them.
            (
                t!("cheatsheet-medical-studies"),
                palette_only(SheetMode::Medical),
            ),
        ],
    )
}

/// Reflectance transformation images' keys: the light, the shading and the widget.
fn relight_groups(km: &Keymap) -> Vec<GroupLines> {
    action_groups(
        km,
        Mode::Relight,
        &[
            (
                t!("keys-mode-relight"),
                &[
                    Action::RelightLeft,
                    Action::RelightRight,
                    Action::RelightUp,
                    Action::RelightDown,
                    Action::RelightReset,
                ],
            ),
            (
                t!("cheatsheet-view"),
                &[Action::RelightMode, Action::RelightWidget],
            ),
            (
                t!("cheatsheet-zoom-pan"),
                &[Action::ZoomIn, Action::ZoomOut, Action::ZoomFit],
            ),
        ],
    )
}

/// 3D meshes' keys: turning and moving the model, the standard views, and how it is drawn.
fn mesh_groups(km: &Keymap) -> Vec<GroupLines> {
    let mut groups = action_groups(
        km,
        Mode::Mesh,
        &[
            (
                t!("keys-mode-mesh"),
                &[
                    Action::MeshOrbitLeft,
                    Action::MeshOrbitRight,
                    Action::MeshOrbitUp,
                    Action::MeshOrbitDown,
                    Action::MeshViewFront,
                    Action::MeshViewTop,
                    Action::MeshViewSide,
                    Action::MeshViewIso,
                ],
            ),
            (
                t!("cheatsheet-zoom-pan"),
                &[
                    Action::PanLeft,
                    Action::PanRight,
                    Action::PanUp,
                    Action::PanDown,
                    Action::ZoomIn,
                    Action::ZoomOut,
                    Action::ZoomFit,
                ],
            ),
            (
                t!("cheatsheet-view"),
                &[
                    Action::MeshShading,
                    Action::MeshProjection,
                    Action::MeshGrid,
                ],
            ),
            (
                t!("cheatsheet-points"),
                &[
                    Action::MeshPointsBigger,
                    Action::MeshPointsSmaller,
                    Action::MeshPointColour,
                    Action::MeshEdl,
                ],
            ),
            (
                t!("cheatsheet-molecules"),
                &[Action::MeshMolStyle, Action::MeshMolColour],
            ),
            (
                t!("cheatsheet-review"),
                &[
                    Action::MeshWalk,
                    Action::MeshFly,
                    Action::MeshViewpoints,
                    Action::MeshSaveViewpoint,
                    Action::MeshSectionPlane,
                    Action::MeshSectionBox,
                    Action::MeshSectionFlip,
                    Action::MeshSectionClear,
                    Action::MeshCutMeasure,
                    Action::MeshBcf,
                    Action::MeshChecks,
                    Action::MeshModels,
                    Action::MeshPmi,
                ],
            ),
        ],
    );
    // A tractogram's streamlines take the point keys, under their own names.
    let tracts: Vec<Line> = [
        Action::MeshPointsBigger,
        Action::MeshPointsSmaller,
        Action::MeshPointColour,
        Action::MeshTractOpacity,
    ]
    .into_iter()
    .filter_map(|a| {
        let (_, keys) = action_line(km, Mode::Mesh, a)?;
        Some((a.tract_name(), keys))
    })
    .collect();
    if !tracts.is_empty() {
        groups.push((t!("cheatsheet-tracts"), tracts));
    }
    groups
}

/// Scientific images' keys: the look (stretch, limits), the planes and channels, and the
/// views that go with them.
fn science_groups(km: &Keymap) -> Vec<GroupLines> {
    let planes = [
        Action::ScienceNextPlane,
        Action::SciencePrevPlane,
        Action::ScienceNextAxis,
        Action::ScienceComposite,
        // A z-stack or cube in three planes.
        Action::Mpr,
    ];
    let mut groups = action_groups(
        km,
        Mode::Science,
        &[
            (
                t!("cheatsheet-science-look"),
                &[
                    Action::ScienceStretch,
                    Action::ScienceLimits,
                    Action::ScienceLevel,
                    Action::ScienceHillshade,
                    Action::ScienceFft,
                    Action::ScienceReset,
                ],
            ),
            (t!("cheatsheet-science-planes"), &planes),
            (
                t!("cheatsheet-view"),
                &[
                    Action::ScienceGrid,
                    Action::ScienceWiggle,
                    Action::ScienceWireframe,
                    Action::ScienceIonImage,
                    Action::ScienceTemperatureUnit,
                    Action::ScienceIsotherm,
                    Action::ScienceThermalParameters,
                    Action::ScienceExportTemperatures,
                    Action::ScienceThermalPlay,
                    Action::ScienceThermalSeriesCsv,
                    Action::ThermalReport,
                    Action::ScienceCatalogue,
                    Action::ScienceClearCatalogues,
                    Action::ScienceLoadRegions,
                    Action::ScienceExportRegions,
                    Action::PlateSolve,
                    Action::SkySurvey,
                    Action::SampSendImage,
                    Action::SampSendPosition,
                    Action::TogglePixelInspector,
                    Action::ZoomFit,
                    Action::ZoomTo(100.0),
                    Action::ZoomRectangle,
                ],
            ),
        ],
    );
    // The channel keys as two runs ("1–9", "Alt+0–6") rather than sixteen lines.
    let run = |label: String, channels: std::ops::RangeInclusive<u8>| {
        let keys: Vec<String> = channels
            .map(Action::ScienceChannel)
            .filter(|a| km.shows(*a))
            .filter_map(|a| mode_keys(km, Mode::Science, a).into_iter().next())
            .collect();
        Some((label, digit_run(&keys)?))
    };
    let channels = [
        run(t!("cheatsheet-science-channels-1-9"), 1..=9),
        run(t!("cheatsheet-science-channels-10-16"), 10..=16),
    ];
    let planes_title = t!("cheatsheet-science-planes");
    match groups.iter_mut().find(|(t, _)| *t == planes_title) {
        Some((_, lines)) => lines.extend(channels.into_iter().flatten()),
        None => groups.push((planes_title, channels.into_iter().flatten().collect())),
    }
    groups.retain(|(_, l)| !l.is_empty());
    groups
}

/// Drawings' keys: layers and paper, then the views that go with them.
fn drawing_groups(km: &Keymap) -> Vec<GroupLines> {
    action_groups(
        km,
        Mode::Drawing,
        &[
            (
                t!("cheatsheet-drawing"),
                &[
                    Action::DrawingLayers,
                    Action::DrawingAllLayers,
                    Action::DrawingPaper,
                    Action::DrawingLineweights,
                ],
            ),
            (
                t!("cheatsheet-view"),
                &[
                    Action::ZoomFit,
                    Action::ZoomRectangle,
                    Action::NextPage,
                    Action::PrevPage,
                    Action::Measure,
                ],
            ),
        ],
    )
}

/// Cull mode's keys from the keymap's cull layer: the marks (the gallery's too), the folder
/// slots, bursts and following new files. Numbered runs read as one line ("1–5", "Alt+1–3").
fn cull_lines(km: &Keymap) -> Vec<(String, Vec<String>)> {
    let key = |a: Action| km.keys_in(Mode::Cull, a).first().map(|k| k.to_string());
    let one = |label: String, a: Action| Some((label, vec![key(a)?]));
    let run = |label: String, acts: Vec<Action>| {
        let keys: Vec<String> = acts.into_iter().filter_map(key).collect();
        Some((label, digit_run(&keys)?))
    };
    [
        one(t!("gallery-pick"), Action::CullPick),
        one(t!("gallery-reject"), Action::CullReject),
        one(t!("gallery-unflag"), Action::CullUnflag),
        run(
            t!("gallery-facet-rating"),
            (1..=5).map(Action::CullRate).collect(),
        ),
        one(t!("gallery-rate-none"), Action::CullRate(0)),
        run(
            t!("gallery-facet-label"),
            (1..=4).map(Action::CullLabel).collect(),
        ),
        run(
            t!("cull-key-copy"),
            (1..=3).map(Action::CullCopyTo).collect(),
        ),
        run(
            t!("cull-key-move"),
            (1..=3).map(Action::CullMoveTo).collect(),
        ),
        one(t!("cull-key-burst"), Action::CullBurst),
        one(t!("cull-key-best"), Action::CullKeepBest),
        one(t!("cull-key-follow"), Action::CullFollow),
        one(t!("cull-key-subframes"), Action::CullSubframes),
        one(t!("cull-key-sort"), Action::CullSortNext),
        one(t!("cull-key-sort-reverse"), Action::CullSortReverse),
        one(t!("cull-key-metric"), Action::CullMetricNext),
        one(t!("cull-key-sigma-down"), Action::CullSigmaDown),
        one(t!("cull-key-sigma-up"), Action::CullSigmaUp),
        one(t!("cull-key-auto-reject"), Action::CullAutoReject),
        one(t!("cull-key-move-rejects"), Action::CullMoveRejects),
        one(t!("cull-key-same-stretch"), Action::CullSameStretch),
        one(Action::DevelopSync.name(), Action::DevelopSync),
        one(
            Action::DevelopSyncSelective.name(),
            Action::DevelopSyncSelective,
        ),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// Keys that differ only in a last digit counting up by one ("Alt+1", "Alt+2", "Alt+3") as
/// "Alt+1–3"; any other set as its first two keys. `None` when there are none.
pub fn digit_run(keys: &[String]) -> Option<Vec<String>> {
    let first = keys.first()?;
    let split = |k: &str| {
        let d = k.chars().last().filter(char::is_ascii_digit)?;
        Some((k[..k.len() - 1].to_owned(), d as u32))
    };
    let parts: Option<Vec<_>> = keys.iter().map(|k| split(k)).collect();
    let counts_up = parts.is_some_and(|p| {
        p.windows(2)
            .all(|w| w[0].0 == w[1].0 && w[1].1 == w[0].1 + 1)
    });
    Some(match keys.last().and_then(|k| k.chars().last()) {
        Some(d) if counts_up && keys.len() > 1 => vec![format!("{first}–{d}")],
        _ => keys.iter().take(2).cloned().collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(k: &[&str]) -> Vec<String> {
        k.iter().map(|s| (*s).to_owned()).collect()
    }

    /// `k`, written as Linux and Windows show it (a digit run `Alt+0–6` allowed), as this
    /// platform shows it: macOS uses its symbols (`⌥0–6`).
    fn d(k: &str) -> String {
        let (head, run) = match k.split_once('–') {
            Some((h, r)) => (h, Some(r)),
            None => (k, None),
        };
        let shown = specular_input::KeyCombo::parse(head)
            .unwrap_or_else(|| panic!("{head}"))
            .to_string();
        match run {
            Some(r) => format!("{shown}–{r}"),
            None => shown,
        }
    }

    /// The line labelled `label` in a tab's groups.
    fn keys_of(groups: &[GroupLines], label: &str) -> Option<Vec<String>> {
        groups
            .iter()
            .flat_map(|(_, l)| l)
            .find(|(l, _)| l == label)
            .map(|(_, k)| k.clone())
    }

    #[test]
    fn numbered_keys_read_as_a_run() {
        assert_eq!(digit_run(&v(&["1", "2", "3"])), Some(v(&["1–3"])));
        assert_eq!(digit_run(&v(&["Alt+1", "Alt+2"])), Some(v(&["Alt+1–2"])));
        assert_eq!(digit_run(&v(&["Alt+7"])), Some(v(&["Alt+7"])));
        assert_eq!(digit_run(&v(&["1", "3", "4"])), Some(v(&["1", "3"])));
        assert_eq!(digit_run(&v(&["Alt+1", "2"])), Some(v(&["Alt+1", "2"])));
        assert_eq!(digit_run(&v(&["a", "b"])), Some(v(&["a", "b"])));
        assert_eq!(digit_run(&[]), None);
        // The lines follow the keymap.
        let mut km = Keymap::viewer_default();
        let lines = cull_lines(&km);
        assert!(
            lines.iter().any(|(_, k)| k == &vec![d("Alt+Shift+1–3")]),
            "{lines:?}"
        );
        assert!(km.apply_toml("[cull]\n\"Alt+2\" = \"none\"\n").is_empty());
        assert!(
            cull_lines(&km)
                .iter()
                .any(|(_, k)| k == &vec![d("Alt+1"), d("Alt+3")])
        );
    }

    /// The science tab shows the sixteen channel keys as two runs, and the catalogue keys.
    #[test]
    fn science_channels_read_as_runs() {
        let km = Keymap::viewer_default();
        let g = groups_for(SheetMode::Science, &km);
        assert_eq!(
            keys_of(&g, &t!("cheatsheet-science-channels-1-9")),
            Some(v(&["1–9"]))
        );
        assert_eq!(
            keys_of(&g, &t!("cheatsheet-science-channels-10-16")),
            Some(vec![d("Alt+0–6")])
        );
        assert_eq!(
            keys_of(&g, &t!("palette-science-catalogue")),
            Some(vec![d("Shift+O")])
        );
    }

    /// Every tab has keys, and every line at least one key, but for the tab's palette-only
    /// actions.
    #[test]
    fn every_tab_lists_keys() {
        let km = Keymap::viewer_default();
        for m in SheetMode::ALL {
            let g = groups_for(m, &km);
            assert!(!g.is_empty(), "{m:?}");
            let palette: Vec<String> = palette_only(m).iter().map(|a| a.name()).collect();
            for (title, lines) in &g {
                assert!(!lines.is_empty(), "{m:?} {title}");
                for (label, keys) in lines {
                    assert!(!keys.is_empty() || palette.contains(label), "{m:?} {label}");
                }
            }
        }
    }

    /// Studies has no key in medical mode: the tab lists it, last, as in the command palette
    /// (naming the palette's key), read out as such; once a key runs it, it is an ordinary
    /// line with that key and the palette group goes when nothing is left in it.
    #[test]
    fn palette_only_actions_name_the_palette() {
        let mut km = Keymap::viewer_default();
        let studies = t!("palette-studies");
        let g = groups_for(SheetMode::Medical, &km);
        let (title, lines) = g.last().unwrap();
        let key = palette_key(&km).expect("the palette has a key");
        assert_eq!(
            *title,
            t!("cheatsheet-palette-only-title", key = key.clone())
        );
        let line = lines.iter().find(|(l, _)| *l == studies).expect("studies");
        assert!(line.1.is_empty());
        let spoken = spoken_line(&km, &line.0, &line.1);
        assert!(
            spoken.contains(&studies) && spoken.contains(&key),
            "{spoken}"
        );
        assert_eq!(spoken_line(&km, "Pick", &v(&["P"])), "Pick: P");
        // Only the medical and video tabs have such a group (for now).
        for m in SheetMode::ALL {
            let n = groups_for(m, &km)
                .iter()
                .flat_map(|(_, l)| l)
                .filter(|(_, k)| k.is_empty())
                .count();
            assert_eq!(
                n > 0,
                matches!(m, SheetMode::Medical | SheetMode::Video),
                "{m:?}"
            );
        }

        // Bound (in the viewer, which medical mode inherits): an ordinary line.
        let combo = specular_input::KeyCombo::parse("Ctrl+Alt+S").unwrap();
        km.bind(combo, Action::Studies);
        let g = groups_for(SheetMode::Medical, &km);
        assert_eq!(
            keys_of(&g, &studies),
            Some(vec![KeySeq::one(combo).to_string()])
        );
        let palette_lines: Vec<&String> = g
            .iter()
            .flat_map(|(_, l)| l)
            .filter(|(_, k)| k.is_empty())
            .map(|(l, _)| l)
            .collect();
        assert!(!palette_lines.contains(&&studies));

        // Hidden by the profile: not listed at all.
        let mut km = Keymap::viewer_default();
        km.set_profile(specular_input::Profile::of(
            specular_input::Capabilities::GENERAL,
        ));
        let g = groups_for(SheetMode::Medical, &km);
        assert!(keys_of(&g, &studies).is_none());
    }

    /// The gallery and compare list every line of their tables (the keys their handlers
    /// run), with the digits as runs.
    #[test]
    fn gallery_and_compare_list_their_tables() {
        let km = Keymap::viewer_default();
        let i18n = I18n::global();
        let gallery = groups_for(SheetMode::Gallery, &km);
        for l in fixed::GALLERY_KEYS {
            assert!(
                keys_of(&gallery, &i18n.tr(l.label)).is_some(),
                "{:?}",
                l.cmd
            );
        }
        let compare = groups_for(SheetMode::Compare, &km);
        for l in fixed::COMPARE_KEYS {
            assert!(
                keys_of(&compare, &i18n.tr(l.label)).is_some(),
                "{:?}",
                l.cmd
            );
        }
        assert_eq!(
            keys_of(&gallery, &t!("gallery-facet-rating")),
            Some(v(&["1–5"]))
        );
        assert_eq!(
            keys_of(&gallery, &t!("gallery-facet-label")),
            Some(v(&["6–9"]))
        );
        assert_eq!(
            keys_of(&gallery, &t!("cheatsheet-gallery-quick-look")),
            Some(vec![t!("cheatsheet-hold", key = "Q")])
        );
        // Compare's channel views are the viewer's keys.
        assert_eq!(
            keys_of(&compare, &Action::ChannelRed.name()),
            Some(vec![d("Shift+1")])
        );
    }

    /// Medical images list the newer keys (measuring, overlays, cine, the presets) from the
    /// medical layer, and the viewer's keys under them where the layer leaves them free.
    #[test]
    fn medical_tab_follows_the_medical_layer() {
        let mut km = Keymap::viewer_default();
        let tr = |a: Action| a.name();
        let g = groups_for(SheetMode::Medical, &km);
        for (a, k) in [
            (Action::Cine, "P"),
            (Action::MeasureLength, "M"),
            (Action::MeasureAngle, "A"),
            (Action::MeasureEllipse, "E"),
            (Action::MeasureRectangle, "R"),
            (Action::ToggleOverlays, "O"),
            (Action::WindowPreset(1), "1"),
            (Action::WindowPreset(0), "0"),
        ] {
            let keys = keys_of(&g, &tr(a)).unwrap_or_default();
            assert!(keys.contains(&k.to_owned()), "{a:?}: {keys:?}");
        }
        let clear = keys_of(&g, &tr(Action::ClearMeasurements)).unwrap_or_default();
        assert_eq!(clear.len(), 1, "{clear:?}");
        // The page keys step the frames of a multi-frame image (the viewer's keys).
        for (a, k) in [
            (Action::NextPage, "Alt+PgDn"),
            (Action::PrevPage, "Alt+PgUp"),
            (Action::FirstPage, "Alt+Home"),
            (Action::LastPage, "Alt+End"),
        ] {
            assert_eq!(keys_of(&g, &tr(a)), Some(vec![d(k)]), "{a:?}");
        }
        // A rebind shows.
        assert!(
            km.apply_toml("[medical]\n\"p\" = \"none\"\n\"Ctrl+Shift+p\" = \"cine\"\n")
                .is_empty()
        );
        let g = groups_for(SheetMode::Medical, &km);
        assert_eq!(
            keys_of(&g, &tr(Action::Cine)),
            Some(vec![d("Ctrl+Shift+P")])
        );
        // The viewer tab keeps the viewer's keys only.
        let viewer = groups_for(SheetMode::Viewer, &km);
        assert!(keys_of(&viewer, &tr(Action::Cine)).is_none());
    }

    /// A gallery line lists the keymap's keys of the same action that reach the gallery: a
    /// rebind with Ctrl shows, a plain letter (it types a name to jump to) does not.
    #[test]
    fn gallery_lines_follow_the_keymap() {
        let mut km = Keymap::viewer_default();
        assert!(
            km.apply_toml("[viewer]\n\"Ctrl+Shift+F5\" = \"reload\"\n\"z\" = \"trash\"\n")
                .is_empty()
        );
        let g = groups_for(SheetMode::Gallery, &km);
        let refresh = keys_of(&g, &t!("cheatsheet-gallery-refresh")).unwrap_or_default();
        assert!(refresh.contains(&d("Ctrl+Shift+F5")), "{refresh:?}");
        let trash = keys_of(&g, &t!("palette-move-to-trash")).unwrap_or_default();
        assert!(!trash.contains(&"z".to_owned()), "{trash:?}");
    }

    /// Every key a mode's own layer binds (and the profile shows) is on that mode's tab,
    /// under the action's name or the line that gathers it ("1–5" for the ratings).
    #[test]
    fn every_bound_key_is_on_its_tab() {
        let km = Keymap::viewer_default();
        let line = |a: Action| match a {
            Action::Back => t!("cheatsheet-back"),
            Action::CullRate(0) => t!("gallery-rate-none"),
            Action::CullRate(_) => t!("gallery-facet-rating"),
            Action::CullLabel(_) => t!("gallery-facet-label"),
            Action::CullCopyTo(_) => t!("cull-key-copy"),
            Action::CullMoveTo(_) => t!("cull-key-move"),
            Action::CullPick => t!("gallery-pick"),
            Action::CullReject => t!("gallery-reject"),
            Action::CullUnflag => t!("gallery-unflag"),
            Action::CullBurst => t!("cull-key-burst"),
            Action::CullKeepBest => t!("cull-key-best"),
            Action::CullFollow => t!("cull-key-follow"),
            Action::CullSubframes => t!("cull-key-subframes"),
            Action::CullSortNext => t!("cull-key-sort"),
            Action::CullSortReverse => t!("cull-key-sort-reverse"),
            Action::CullMetricNext => t!("cull-key-metric"),
            Action::CullSigmaDown => t!("cull-key-sigma-down"),
            Action::CullSigmaUp => t!("cull-key-sigma-up"),
            Action::CullAutoReject => t!("cull-key-auto-reject"),
            Action::CullMoveRejects => t!("cull-key-move-rejects"),
            Action::CullSameStretch => t!("cull-key-same-stretch"),
            Action::ScienceChannel(1..=9) => t!("cheatsheet-science-channels-1-9"),
            Action::ScienceChannel(_) => t!("cheatsheet-science-channels-10-16"),
            Action::SeekPercent(_) => t!("cheatsheet-video-seek-percent"),
            a => a.name(),
        };
        let mut missing = Vec::new();
        for (sheet, mode) in [
            (SheetMode::Viewer, Mode::Viewer),
            (SheetMode::Cull, Mode::Cull),
            (SheetMode::Medical, Mode::Medical),
            (SheetMode::Science, Mode::Science),
            (SheetMode::Drawing, Mode::Drawing),
            (SheetMode::Mesh, Mode::Mesh),
            (SheetMode::Relight, Mode::Relight),
            (SheetMode::Video, Mode::Video),
            (SheetMode::Map, Mode::Map),
            (SheetMode::Develop, Mode::Develop),
        ] {
            let g = groups_for(sheet, &km);
            for (k, a) in km.bindings(mode) {
                if km.shows(*a) && keys_of(&g, &line(*a)).is_none() {
                    missing.push(format!("{sheet:?}: {k} {a:?}"));
                }
            }
        }
        assert!(missing.is_empty(), "not on the cheat sheet: {missing:#?}");
        // Esc reads short; the viewer tab has the pan keys.
        let viewer = groups_for(SheetMode::Viewer, &km);
        assert!(keys_of(&viewer, &t!("cheatsheet-back")).is_some());
        assert!(keys_of(&viewer, &Action::PanUp.name()).is_some());
    }
}
