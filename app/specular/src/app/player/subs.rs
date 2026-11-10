//! A movie's subtitles: the tracks it carries (closed captions in its video among them)
//! and the files beside it, the one chosen (decoded off the UI thread), and drawing its cues
//! over the picture.
//!
//! The track to start with follows the settings: the subtitle mode (off, forced only,
//! always), the subtitle language and the preference for subtitles for the deaf and hard
//! of hearing; forced subtitles in the sound's language show in every mode.
//!
//! Text cues are laid out with the viewer's text stack (bidirectional: Hebrew and Arabic
//! read correctly) in the look the settings give (`viewing.subtitle_*`: position, size,
//! colour, outline, box and font; white with a dark outline and a soft shadow by default).
//! Bitmap cues (PGS, VobSub) are uploaded as textures and scaled from their canvas to the
//! picture; those in its lower half move with the position setting.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc;

use specular_gfx::{DrawList, Gpu, Rect, TexId, Text, UiRenderer, wgpu};
use specular_settings::{
    Settings, SubtitleBackground, SubtitleColor, SubtitleMode, SubtitleOutline,
};
use specular_subs::{Bitmap, Body, Cue, Track};
use specular_video::extras::{Extras, TrackInfo, iso639_2};
use specular_widgets::subtitles::{DEFAULT_POSITION, Look, Stack};

/// Where a subtitle track comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Origin {
    /// A stream of the movie, by position in [`Extras::subtitles`].
    Embedded(usize),
    File(PathBuf),
}

/// A subtitle track on offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub origin: Origin,
    /// Language code as stored, when known.
    pub lang: Option<String>,
    pub name: Option<String>,
    pub forced: bool,
    /// For the deaf and hard of hearing (SDH, closed captions).
    pub sdh: bool,
    /// A closed caption channel or service in the video.
    pub caption: Option<Caption>,
}

/// A closed caption stream's channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Caption {
    /// CEA-608 CC1..CC4.
    Cc(u8),
    /// CEA-608 text service T1..T4.
    Text(u8),
    /// CEA-708 service 1..63.
    Service(u8),
}

impl Caption {
    fn of(codec: &str, private: &[u8]) -> Option<Caption> {
        let n = private.first().copied().unwrap_or(1);
        match codec {
            specular_subs::CEA_608 if n > 4 => Some(Caption::Text(n - 4)),
            specular_subs::CEA_608 => Some(Caption::Cc(n)),
            specular_subs::CEA_708 => Some(Caption::Service(n)),
            _ => None,
        }
    }

    fn label(self) -> String {
        match self {
            Caption::Cc(n) => specular_i18n::t!("player-track-cc", number = i64::from(n)),
            Caption::Text(n) => specular_i18n::t!("player-track-cc-text", number = i64::from(n)),
            Caption::Service(n) => {
                specular_i18n::t!("player-track-cc-service", number = i64::from(n))
            }
        }
    }
}

/// What picks the track to start with (from the settings).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Prefs<'a> {
    pub mode: SubtitleMode,
    /// A two-letter language code; empty: none.
    pub lang: &'a str,
    pub sdh: bool,
}

impl<'a> Prefs<'a> {
    pub fn of(s: &'a Settings) -> Prefs<'a> {
        Prefs {
            mode: s.viewing.subtitle_mode,
            lang: &s.viewing.subtitle_language,
            sdh: s.viewing.subtitle_prefer_sdh,
        }
    }
}

impl Source {
    /// A stable key for the resume store: the track id or the file's name.
    pub fn key(&self, extras: Option<&Extras>) -> String {
        match &self.origin {
            Origin::Embedded(i) => {
                let id = extras
                    .and_then(|x| x.subtitles.get(*i))
                    .map_or(0, |s| s.info.id);
                format!("track:{id}")
            }
            Origin::File(p) => format!(
                "file:{}",
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ),
        }
    }

    /// Two-letter language code, when known.
    pub fn lang2(&self) -> Option<String> {
        let l = self.lang.as_deref()?;
        let base = l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase();
        Some(iso639_2(&base).map_or(base, str::to_owned))
    }

    /// The menu label: the language's name in the interface language, the track's own name,
    /// and whether it is forced, for the deaf and hard of hearing, or a file. Captions:
    /// "CC1 (English)", "Service 1".
    pub fn label(&self, n: usize) -> String {
        let lang = self.lang2().map(|l| language_name(&l));
        if let Some(c) = self.caption {
            let s = c.label();
            return match lang {
                Some(l) => specular_i18n::t!("player-track-with-language", track = s, language = l),
                None => s,
            };
        }
        let mut s = match (&lang, &self.name) {
            (Some(l), Some(n)) if !n.eq_ignore_ascii_case(l) => format!("{l} – {n}"),
            (Some(l), _) => l.clone(),
            (None, Some(n)) => n.clone(),
            (None, None) => specular_i18n::t!("player-track-number", number = n as i64 + 1),
        };
        if self.sdh && !self.name.as_deref().is_some_and(names_sdh) {
            s = specular_i18n::t!("player-track-sdh", track = s);
        }
        if self.forced {
            s = specular_i18n::t!("player-track-forced", track = s);
        }
        if let Origin::File(p) = &self.origin
            && self.lang.is_none()
            && self.name.is_none()
        {
            s = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or(s);
        }
        s
    }
}

/// Whether a track's own name already says it is SDH.
fn names_sdh(n: &str) -> bool {
    let n = n.to_ascii_lowercase();
    n.contains("sdh") || n.contains("hearing") || n.contains("deaf") || n.contains("caption")
}

/// A language code's name in the interface language ("Hebrew", "עברית").
pub(crate) fn language_name(code: &str) -> String {
    let i18n = specular_i18n::I18n::global();
    match code.parse::<unic_langid::LanguageIdentifier>() {
        Ok(id) => specular_i18n::language_name(&id, &i18n.language()),
        Err(_) => code.to_owned(),
    }
}

/// The tracks on offer: the movie's streams, then the files beside it.
pub(crate) fn sources(movie: &Path, extras: Option<&Extras>) -> Vec<Source> {
    let mut out: Vec<Source> = extras
        .map(|x| {
            x.subtitles
                .iter()
                .enumerate()
                .map(|(i, s)| Source {
                    origin: Origin::Embedded(i),
                    lang: s.info.lang.clone(),
                    name: s.info.name.clone(),
                    forced: s.info.forced,
                    sdh: s.info.hearing_impaired,
                    caption: Caption::of(&s.info.codec, &s.private),
                })
                .collect()
        })
        .unwrap_or_default();
    let stem = movie
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    for f in specular_subs::beside(movie) {
        let flags = specular_subs::file_flags(&f);
        out.push(Source {
            lang: specular_subs::file_language(stem, &f),
            name: None,
            forced: flags.forced,
            sdh: flags.sdh,
            caption: None,
            origin: Origin::File(f),
        });
    }
    out
}

/// The track to start with: the one remembered for this movie (by key); else, as the
/// settings say (`prefs`):
/// - always: a full track in the subtitle language (SDH or not as preferred; a subtitle
///   track before a caption channel; captions only when SDH is preferred); with SDH
///   preferred and no language match, an SDH track or caption channel in the sound's
///   language or of no stated language;
/// - forced only and always: then a forced track in the sound's language, the subtitle
///   language, or of no stated language;
/// - off: a forced track in the sound's language only.
pub(crate) fn pick(
    sources: &[Source],
    extras: Option<&Extras>,
    remembered: Option<&str>,
    prefs: Prefs,
    sound_lang: Option<&str>,
) -> Option<usize> {
    if let Some(k) = remembered {
        if k.is_empty() {
            return None;
        }
        if let Some(i) = sources.iter().position(|s| s.key(extras) == k) {
            return Some(i);
        }
    }
    let pref = prefs.lang.trim().to_ascii_lowercase();
    let pref = (!pref.is_empty() && pref != "off").then_some(pref);
    let in_lang = |s: &Source, l: &str| s.lang2().as_deref() == Some(l);
    // Among candidates: the SDH preference met first, subtitle tracks before captions.
    let best = |c: &mut dyn Iterator<Item = usize>| {
        c.min_by_key(|&i| {
            let s = &sources[i];
            (s.sdh != prefs.sdh, s.caption.is_some(), i)
        })
    };
    if prefs.mode == SubtitleMode::Always {
        if let Some(l) = pref.as_deref() {
            // Captions only for those who prefer them.
            let ok = |s: &Source| in_lang(s, l) && (prefs.sdh || s.caption.is_none());
            let full =
                best(&mut (0..sources.len()).filter(|&i| !sources[i].forced && ok(&sources[i])));
            if let Some(i) = full.or_else(|| sources.iter().position(ok)) {
                return Some(i);
            }
        }
        if prefs.sdh {
            let sdh = best(&mut (0..sources.len()).filter(|&i| {
                let s = &sources[i];
                s.sdh
                    && !s.forced
                    && (s.lang.is_none() || sound_lang.is_some_and(|l| in_lang(s, l)))
            }));
            if sdh.is_some() {
                return sdh;
            }
        }
    }
    let forced_in = |l: &str| sources.iter().position(|s| s.forced && in_lang(s, l));
    if let Some(i) = sound_lang.and_then(forced_in) {
        return Some(i);
    }
    if prefs.mode == SubtitleMode::Off {
        return None;
    }
    pref.as_deref()
        .and_then(forced_in)
        .or_else(|| sources.iter().position(|s| s.forced && s.lang.is_none()))
}

/// The sound track to start with: the one remembered (`remembered`, an id), else one in
/// the sound language (`pref`; empty: the movie's default track's language), with audio
/// description or without as `prefer_description` says, the default track first. `None`:
/// the movie's own choice.
pub(crate) fn pick_sound(
    audio: &[TrackInfo],
    remembered: Option<u64>,
    pref: &str,
    prefer_description: bool,
) -> Option<u64> {
    if let Some(id) = remembered.filter(|id| audio.iter().any(|a| a.id == *id)) {
        return Some(id);
    }
    if audio.len() < 2 {
        return None;
    }
    let default = audio.iter().find(|a| a.default).or(audio.first())?;
    let matches = |a: &TrackInfo, l: &str| {
        a.lang
            .as_deref()
            .is_some_and(|c| specular_video::extras::lang_matches(c, l))
    };
    let pref = pref.trim().to_ascii_lowercase();
    let lang = if !pref.is_empty() && audio.iter().any(|a| matches(a, &pref)) {
        Some(pref)
    } else {
        default.lang.as_deref().map(|l| {
            let base = l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase();
            iso639_2(&base).map_or(base, str::to_owned)
        })
    };
    audio
        .iter()
        .enumerate()
        .filter(|(_, a)| lang.as_deref().is_none_or(|l| matches(a, l)))
        .min_by_key(|(i, a)| (a.visual_impaired != prefer_description, !a.default, *i))
        .map(|(_, a)| a.id)
}

/// Whether a sound track's own name already says it has audio description.
pub(crate) fn names_description(n: &str) -> bool {
    let n = n.to_ascii_lowercase();
    n.contains("descri") || n.split(|c: char| !c.is_alphanumeric()).any(|w| w == "ad")
}

/// What a decode gives back.
pub(crate) type Decoded = Result<(Track, Option<&'static encoding_rs::Encoding>), String>;

/// Decodes `src` on a worker; `waker` wakes the event loop when done.
pub(crate) fn decode(
    src: &Source,
    extras: Option<Arc<Extras>>,
    charset: Option<&'static encoding_rs::Encoding>,
    waker: Option<specular_anim::Waker>,
) -> mpsc::Receiver<Decoded> {
    let (tx, rx) = mpsc::channel();
    let src = src.clone();
    let _ = specular_thread::builder("player-subs").spawn(move || {
        let r = match &src.origin {
            Origin::File(p) => {
                let hint = src.lang2();
                specular_subs::read_file(p, charset, hint.as_deref())
                    .map(|(t, read)| (t, read.map(|r| r.encoding)))
                    .map_err(|e| e.to_string())
            }
            Origin::Embedded(i) => match extras.as_ref().and_then(|x| x.subtitles.get(*i)) {
                Some(s) => specular_subs::decode_stream(
                    &s.info.codec,
                    &s.private,
                    s.packets
                        .iter()
                        .map(|p| (p.start_us, p.dur_us, p.data.as_slice())),
                )
                .map(|t| (t, None))
                .map_err(|e| e.to_string()),
                None => Err("no such track".into()),
            },
        };
        let _ = tx.send(r);
        if let Some(w) = waker {
            w();
        }
    });
    rx
}

/// The cues showing at movie time `t_us` (the player's time plus the picture's start) with
/// subtitles `delay_us` later.
pub(crate) fn active(track: &Track, t_us: i64, delay_us: i64) -> Vec<&Cue> {
    track.active(t_us - delay_us).collect()
}

/// Bitmap textures, by the cue's start and place (a cue shows for a while; its texture is
/// made once).
#[derive(Default)]
pub(crate) struct Bitmaps {
    held: Vec<((i64, u32, u32), wgpu::Texture, TexId)>,
}

impl Bitmaps {
    /// The GPU device was replaced: the cues are uploaded again as they show.
    pub(crate) fn gpu_reset(&mut self) {
        self.held.clear();
    }

    fn get(&mut self, gpu: &Gpu, ui: &mut UiRenderer, c: &Cue) -> Option<TexId> {
        let Body::Bitmap(b) = &c.body else {
            return None;
        };
        let key = (c.start_us, b.x, b.y);
        if let Some((_, _, id)) = self.held.iter().find(|h| h.0 == key) {
            return Some(*id);
        }
        let (tex, id) = upload_texture(gpu, ui, b.w, b.h, &b.rgba);
        self.held.push((key, tex, id));
        Some(id)
    }

    /// Lets go of textures no cue on screen uses.
    fn keep(&mut self, ui: &mut UiRenderer, on: &[(i64, u32, u32)]) {
        self.held.retain(|(k, _, id)| {
            let keep = on.contains(k);
            if !keep {
                ui.unregister_texture(*id);
            }
            keep
        });
    }

    pub(crate) fn clear(&mut self, ui: &mut UiRenderer) {
        self.keep(ui, &[]);
    }
}

pub(crate) fn upload_texture(
    gpu: &Gpu,
    ui: &mut UiRenderer,
    w: u32,
    h: u32,
    rgba: &[u8],
) -> (wgpu::Texture, TexId) {
    let size = wgpu::Extent3d {
        width: w.max(1),
        height: h.max(1),
        depth_or_array_layers: 1,
    };
    let tex = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("subtitle"),
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    // Straight alpha in, premultiplied for the UI's blending.
    let pm: Vec<u8> = rgba
        .chunks_exact(4)
        .flat_map(|p| {
            let a = u16::from(p[3]);
            [
                (u16::from(p[0]) * a / 255) as u8,
                (u16::from(p[1]) * a / 255) as u8,
                (u16::from(p[2]) * a / 255) as u8,
                p[3],
            ]
        })
        .collect();
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &pm,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4 * w.max(1)),
            rows_per_image: None,
        },
        size,
    );
    let view = tex.create_view(&Default::default());
    let id = ui.register_texture(gpu, &view);
    (tex, id)
}

/// How text subtitles look, from the settings.
pub(crate) fn look(s: &Settings) -> Look {
    let v = &s.viewing;
    let fill = match v.subtitle_color {
        SubtitleColor::White => [255, 255, 255, 255],
        SubtitleColor::Yellow => [255, 230, 0, 255],
        SubtitleColor::Cyan => [0, 230, 255, 255],
        SubtitleColor::Green => [60, 230, 90, 255],
        SubtitleColor::Custom => {
            let c = v.subtitle_custom_color;
            // A colour that can't be seen is no choice: opaque.
            [c.r, c.g, c.b, c.a.max(64)]
        }
    };
    let outline = match v.subtitle_outline {
        SubtitleOutline::Black => Some([0, 0, 0, 255]),
        SubtitleOutline::Grey => Some([90, 90, 90, 255]),
        SubtitleOutline::White => Some([255, 255, 255, 255]),
        SubtitleOutline::None => None,
    };
    Look {
        size: v.subtitle_size.clamp(50, 250) as f32 / 100.0,
        position: v.subtitle_position.clamp(0, 90) as f32 / 100.0,
        fill,
        outline,
        boxed: v.subtitle_background == SubtitleBackground::Box,
        family: v.subtitle_font.trim().to_owned(),
        override_styles: v.subtitle_override_styles,
    }
}

/// Where bitmap cue `b` goes on the picture `pic`: scaled from its canvas; in the lower half,
/// raised by the position setting's distance from the default (`lift`, a share of the
/// picture's height) and kept inside the picture. Cues in the upper half (signs, captions
/// placed at the top) stay.
pub(crate) fn bitmap_rect(b: &Bitmap, pic: Rect, lift: f32) -> Rect {
    let (cw, ch) = (b.canvas.0.max(1) as f32, b.canvas.1.max(1) as f32);
    let (sx, sy) = (pic.w / cw, pic.h / ch);
    let (w, h) = (b.w as f32 * sx, b.h as f32 * sy);
    let mut y = pic.y + b.y as f32 * sy;
    if y + h * 0.5 > pic.y + pic.h * 0.5 {
        y = (y - lift * pic.h).min(pic.y + pic.h - h).max(pic.y);
    }
    Rect::new(pic.x + b.x as f32 * sx, y, w, h)
}

/// Draws the cues over the picture at `pic` (logical px) in `look`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    dl: &mut DrawList,
    text: &mut Text,
    gpu: &Gpu,
    ui: &mut UiRenderer,
    bitmaps: &mut Bitmaps,
    cues: &[&Cue],
    pic: Rect,
    look: &Look,
) {
    let mut on = Vec::new();
    let mut stack = Stack::new(pic, look.position);
    for c in cues {
        match &c.body {
            Body::Bitmap(b) => {
                if let Some(id) = bitmaps.get(gpu, ui, c) {
                    let r = bitmap_rect(b, pic, look.lift());
                    dl.image(r, [0.0, 0.0, 1.0, 1.0], id, 0.0);
                    on.push((c.start_us, b.x, b.y));
                }
            }
            Body::Text(t) => {
                specular_widgets::subtitles::draw_text(dl, text, t, pic, look, &mut stack)
            }
        }
    }
    bitmaps.keep(ui, &on);
}

/// Offers the font families to the subtitle font setting, once the platform's fonts are in
/// (`offered` keeps it to once).
pub(crate) fn offer_fonts(
    offered: &mut bool,
    chrome: &mut specular_widgets::Chrome,
    text: &mut Text,
) {
    if *offered || text.fonts_pending() {
        return;
    }
    *offered = true;
    let pairs = text
        .family_names()
        .into_iter()
        .map(|f| {
            let label = specular_i18n::isolate_ltr(&f);
            (f, label)
        })
        .collect();
    chrome.set_setting_choices("viewing.subtitle_font", Some(pairs));
}

/// The position after a step of `by` percentage points: within 0-90 %.
pub(crate) fn step_position(now: i64, by: i64) -> i64 {
    (now + by).clamp(0, 90)
}

/// The size one 10 % step larger (`up`) or smaller: within 50-250 %, onto the steps from a
/// size in between.
pub(crate) fn step_size(now: i64, up: bool) -> i64 {
    let n = if up {
        now.div_euclid(10) * 10 + 10
    } else {
        (now + 9).div_euclid(10) * 10 - 10
    };
    n.clamp(50, 250)
}

/// The default position in whole percent.
pub(crate) const DEFAULT_POSITION_PERCENT: i64 = (DEFAULT_POSITION * 100.0 + 0.5) as i64;

#[cfg(test)]
mod tests {
    use super::*;

    fn src(lang: Option<&str>, forced: bool, origin: Origin) -> Source {
        Source {
            origin,
            lang: lang.map(str::to_owned),
            name: None,
            forced,
            sdh: false,
            caption: None,
        }
    }

    fn prefs(lang: &str) -> Prefs<'_> {
        Prefs {
            mode: SubtitleMode::Always,
            lang,
            sdh: false,
        }
    }

    #[test]
    fn the_track_to_start_with() {
        let s = vec![
            src(Some("eng"), true, Origin::Embedded(0)),
            src(Some("eng"), false, Origin::Embedded(1)),
            src(Some("heb"), false, Origin::Embedded(2)),
            src(Some("he"), false, Origin::File("/v/m.he.srt".into())),
        ];
        assert_eq!(pick(&s, None, None, prefs("he"), None), Some(2));
        assert_eq!(
            pick(&s, None, None, prefs("en"), None),
            Some(1),
            "a full track before a forced one"
        );
        assert_eq!(
            pick(&s, None, None, prefs(""), Some("en")),
            Some(0),
            "forced in the sound's language"
        );
        assert_eq!(pick(&s, None, None, prefs(""), None), None);
        assert_eq!(
            pick(&s, None, Some("file:m.he.srt"), prefs("en"), None),
            Some(3),
            "remembered"
        );
        assert_eq!(
            pick(&s, None, Some(""), prefs("en"), None),
            None,
            "remembered off"
        );
    }

    #[test]
    fn modes_sdh_and_captions() {
        let sdh = |mut s: Source| {
            s.sdh = true;
            s
        };
        let cc = |n: u8, lang: Option<&str>| {
            let mut s = src(lang, false, Origin::Embedded(10 + usize::from(n)));
            s.sdh = true;
            s.caption = Some(Caption::Cc(n));
            s
        };
        let s = vec![
            src(Some("eng"), false, Origin::Embedded(0)),
            sdh(src(Some("eng"), false, Origin::Embedded(1))),
            src(Some("fre"), true, Origin::Embedded(2)),
            src(None, true, Origin::Embedded(3)),
            cc(1, None),
        ];
        let p = |mode, lang, sdh| Prefs { mode, lang, sdh };
        use SubtitleMode::{Always, Forced, Off};
        assert_eq!(pick(&s, None, None, p(Always, "en", false), None), Some(0));
        assert_eq!(
            pick(&s, None, None, p(Always, "en", true), None),
            Some(1),
            "SDH preferred"
        );
        // No subtitle language: captions only when SDH is preferred.
        assert_eq!(
            pick(&s, None, None, p(Always, "", true), Some("de")),
            Some(4)
        );
        assert_eq!(
            pick(&s, None, None, p(Always, "", false), Some("de")),
            Some(3)
        );
        // Forced in the sound's language shows in every mode.
        for mode in [Off, Forced, Always] {
            assert_eq!(
                pick(&s, None, None, p(mode, "", false), Some("fr")),
                Some(2)
            );
        }
        // Off: nothing else; forced only: a forced track of no stated language.
        assert_eq!(pick(&s, None, None, p(Off, "en", true), Some("de")), None);
        assert_eq!(
            pick(&s, None, None, p(Forced, "en", true), Some("de")),
            Some(3)
        );
        // A subtitle track before a caption channel, both SDH.
        let s2 = vec![
            cc(1, Some("eng")),
            sdh(src(Some("eng"), false, Origin::Embedded(1))),
        ];
        assert_eq!(pick(&s2, None, None, p(Always, "en", true), None), Some(1));
        // Captions in the subtitle language stay off unless SDH is preferred.
        let s3 = vec![cc(1, Some("eng"))];
        assert_eq!(pick(&s3, None, None, p(Always, "en", false), None), None);
        assert_eq!(pick(&s3, None, None, p(Always, "en", true), None), Some(0));
    }

    #[test]
    fn the_sound_to_start_with() {
        let a = |id: u64, lang: &str, default: bool, ad: bool| TrackInfo {
            id,
            lang: Some(lang.into()),
            default,
            visual_impaired: ad,
            ..TrackInfo::default()
        };
        let tracks = [
            a(1, "eng", true, false),
            a(2, "eng", false, true),
            a(3, "fre", false, false),
            a(4, "fre", false, true),
        ];
        assert_eq!(
            pick_sound(&tracks, Some(3), "en", true),
            Some(3),
            "remembered"
        );
        assert_eq!(pick_sound(&tracks, None, "", false), Some(1));
        assert_eq!(
            pick_sound(&tracks, None, "", true),
            Some(2),
            "description preferred"
        );
        assert_eq!(pick_sound(&tracks, None, "fr", false), Some(3));
        assert_eq!(pick_sound(&tracks, None, "fr", true), Some(4));
        // A language the movie lacks: the default's.
        assert_eq!(pick_sound(&tracks, None, "de", true), Some(2));
        assert_eq!(pick_sound(&tracks[..1], None, "fr", true), None);
        assert!(names_description("English (Audio Description)"));
        assert!(names_description("English AD"));
        assert!(!names_description("Adventure mix"));
    }

    #[test]
    fn caption_labels() {
        let plain = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
        let mut c = src(Some("eng"), false, Origin::Embedded(0));
        c.caption = Some(Caption::Cc(1));
        assert_eq!(plain(c.label(0)), "CC1 (English)");
        c.lang = None;
        c.caption = Some(Caption::Service(2));
        assert_eq!(plain(c.label(0)), "Service 2");
        c.caption = Some(Caption::Text(1));
        assert_eq!(plain(c.label(0)), "Text 1");
        let mut s = src(Some("eng"), false, Origin::Embedded(0));
        s.sdh = true;
        assert_eq!(plain(s.label(0)), "English (SDH)");
        s.name = Some("English SDH".into());
        assert_eq!(plain(s.label(0)), "English – English SDH");
        assert_eq!(
            Caption::of(specular_subs::CEA_608, &[6]),
            Some(Caption::Text(2))
        );
    }

    #[test]
    fn keys_and_languages() {
        let f = src(Some("he"), false, Origin::File("/v/Film.he.srt".into()));
        assert_eq!(f.key(None), "file:Film.he.srt");
        assert_eq!(f.lang2().as_deref(), Some("he"));
        assert_eq!(
            src(Some("heb"), false, Origin::Embedded(0))
                .lang2()
                .as_deref(),
            Some("he")
        );
        assert_eq!(
            src(Some("pt-BR"), false, Origin::Embedded(0))
                .lang2()
                .as_deref(),
            Some("pt")
        );
    }

    fn bitmap(y: u32, h: u32) -> Bitmap {
        Bitmap {
            x: 100,
            y,
            w: 400,
            h,
            canvas: (1920, 1080),
            rgba: Arc::from(vec![0u8; 4].into_boxed_slice()),
        }
    }

    #[test]
    fn bitmap_cues_move_with_the_position() {
        let pic = Rect::new(0.0, 0.0, 960.0, 540.0);
        // A PGS line near the bottom: scaled by half, raised by the lift.
        let b = bitmap(960, 80);
        let r = bitmap_rect(&b, pic, 0.0);
        assert_eq!((r.x, r.y, r.w, r.h), (50.0, 480.0, 200.0, 40.0));
        let up = bitmap_rect(&b, pic, 0.2);
        assert_eq!((up.x, up.y, up.w), (50.0, 480.0 - 108.0, 200.0));
        // Down: no lower than the picture's bottom; up: no higher than its top.
        assert_eq!(bitmap_rect(&b, pic, -0.05).y, 500.0);
        let tall = bitmap(800, 270);
        assert_eq!(bitmap_rect(&tall, pic, 0.85).y, 0.0);
        // A sign at the top stays.
        let sign = bitmap(40, 80);
        assert_eq!(bitmap_rect(&sign, pic, 0.3).y, 20.0);
    }

    #[test]
    fn the_look_follows_the_settings() {
        let mut s = Settings::default();
        let l = look(&s);
        assert_eq!(l, Look::default());
        assert_eq!(DEFAULT_POSITION_PERCENT, s.viewing.subtitle_position);
        s.viewing.subtitle_position = 30;
        s.viewing.subtitle_size = 150;
        s.viewing.subtitle_color = SubtitleColor::Custom;
        s.viewing.subtitle_custom_color = specular_settings::Color::rgb(0x12, 0x34, 0x56);
        s.viewing.subtitle_outline = SubtitleOutline::None;
        s.viewing.subtitle_background = SubtitleBackground::Box;
        s.viewing.subtitle_font = " DejaVu Serif ".into();
        s.viewing.subtitle_override_styles = true;
        let l = look(&s);
        assert!((l.position - 0.3).abs() < 1e-6 && (l.size - 1.5).abs() < 1e-6);
        assert_eq!(l.fill, [0x12, 0x34, 0x56, 255]);
        assert_eq!(l.outline, None);
        assert!(l.boxed && l.override_styles);
        assert_eq!(l.family, "DejaVu Serif");
        // Out of range in a hand-edited file: clamped.
        s.viewing.subtitle_position = 400;
        assert!((look(&s).position - 0.9).abs() < 1e-6);
    }

    #[test]
    fn live_steps_stay_in_range() {
        assert_eq!(step_position(5, 1), 6);
        assert_eq!(step_position(0, -1), 0);
        assert_eq!(step_position(90, 1), 90);
        assert_eq!(step_size(100, true), 110);
        assert_eq!(step_size(100, false), 90);
        assert_eq!(step_size(250, true), 250);
        assert_eq!(step_size(50, false), 50);
        // A hand-set 125 % steps onto the grid.
        assert_eq!(step_size(125, true), 130);
        assert_eq!(step_size(125, false), 120);
    }

    #[test]
    fn delay_moves_the_cues() {
        let t = specular_subs::srt::parse("1\n00:00:01,000 --> 00:00:02,000\nhi\n").unwrap();
        assert_eq!(active(&t, 1_500_000, 0).len(), 1);
        // Subtitles 1 s later: at 1.5 s nothing yet; at 2.5 s the cue shows.
        assert_eq!(active(&t, 1_500_000, 1_000_000).len(), 0);
        assert_eq!(active(&t, 2_500_000, 1_000_000).len(), 1);
    }
}
