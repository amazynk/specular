//! A text subtitle drawn over a movie's picture: laid out with the viewer's text stack (one
//! bidirectional layout per cue, so Hebrew and Arabic read right to left with numbers and
//! Latin words in place), in the user's colour and font (white on a dark outline and a soft
//! shadow by default, optionally on a translucent box) so it reads over any picture; spans
//! keep their own colours unless the user overrides them. Cues placed at the bottom stack
//! upwards from the user's position, cues at the top downwards.

use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_subs::TextCue;
use specular_theme::Color;

/// Where bottom cues sit by default: their bottom 5 % of the picture's height above its
/// bottom edge.
pub const DEFAULT_POSITION: f32 = 0.05;

/// How text subtitles look, from the settings.
#[derive(Clone, Debug, PartialEq)]
pub struct Look {
    /// 1 = about 5 % of the picture's height a line.
    pub size: f32,
    /// Bottom cues' distance from the picture's bottom, as a share of its height.
    pub position: f32,
    /// The text's colour (straight RGBA).
    pub fill: [u8; 4],
    /// The outline's colour; `None`: no outline.
    pub outline: Option<[u8; 4]>,
    /// A translucent dark box behind each cue.
    pub boxed: bool,
    /// A font family by its installed name; empty: the interface's.
    pub family: String,
    /// The colours and size above apply to every cue, over the file's own.
    pub override_styles: bool,
}

impl Default for Look {
    fn default() -> Look {
        Look {
            size: 1.0,
            position: DEFAULT_POSITION,
            fill: [255, 255, 255, 255],
            outline: Some([0, 0, 0, 255]),
            boxed: false,
            family: String::new(),
            override_styles: false,
        }
    }
}

impl Look {
    /// How far bottom cues placed by their own margin move: the position's distance from
    /// the default (a share of the picture's height; negative: down).
    pub fn lift(&self) -> f32 {
        self.position.clamp(0.0, 0.9) - DEFAULT_POSITION
    }
}

/// Where the next bottom and top cues go.
#[derive(Clone, Copy, Debug)]
pub struct Stack {
    bottom: f32,
    top: f32,
}

impl Stack {
    /// For bottom cues starting at `position` (a share of the picture's height) above the
    /// picture's bottom.
    pub fn new(pic: Rect, position: f32) -> Stack {
        Stack {
            bottom: pic.y + pic.h * (1.0 - position.clamp(0.0, 0.9)),
            top: pic.y + pic.h * 0.05,
        }
    }
}

/// A cue's whole text, the colour from each byte on, and whether most of it is italic or
/// bold, its size and its outline colour.
struct Flat {
    text: String,
    colours: Vec<(usize, Option<[u8; 4]>)>,
    italic: bool,
    bold: bool,
    scale: f32,
    outline: Option<[u8; 4]>,
}

fn flat(c: &TextCue, override_styles: bool) -> Flat {
    let mut text = String::new();
    let mut colours = Vec::new();
    let (mut italic, mut bold, mut chars) = (0usize, 0usize, 0usize);
    let mut scale = 0.0f32;
    let mut outline = None;
    for (i, line) in c.lines.iter().enumerate() {
        if i > 0 {
            text.push('\n');
        }
        for sp in line {
            // Plain white is what styled files spell out for "the default": the user's
            // colour shows instead.
            let own = sp
                .colour
                .filter(|c| !override_styles && c[..3] != [255, 255, 255]);
            colours.push((text.len(), own));
            text.push_str(&sp.text);
            let n = sp.text.chars().count();
            chars += n;
            if sp.italic {
                italic += n;
            }
            if sp.bold {
                bold += n;
            }
            scale = scale.max(sp.scale);
            if !override_styles {
                outline = outline.or(sp.outline);
            }
        }
    }
    // A cue mostly in italic (or bold) is shown so: the bidirectional layout stays whole.
    Flat {
        text,
        colours,
        italic: italic * 2 > chars,
        bold: bold * 2 > chars,
        scale: if scale > 0.0 && !override_styles {
            scale
        } else {
            1.0
        },
        outline,
    }
}

fn rgba(c: [u8; 4], a: f32) -> Color {
    Color::rgba(
        f32::from(c[0]) / 255.0,
        f32::from(c[1]) / 255.0,
        f32::from(c[2]) / 255.0,
        f32::from(c[3]) / 255.0 * a,
    )
}

/// The top-left corner of a cue's block (`w` x `h`, lines `px` high) on the picture `pic`:
/// at the cue's own anchor when it has one; else by its alignment, bottom cues raised by
/// `lift` (see [`Look::lift`]) and stacked upwards, top ones downwards. Bottom cues stay
/// inside the picture.
pub fn place(
    cue: &TextCue,
    pic: Rect,
    (w, h): (f32, f32),
    px: f32,
    lift: f32,
    stack: &mut Stack,
) -> (f32, f32) {
    let col = (cue.align.clamp(1, 9) - 1) % 3;
    if let Some((ax, ay)) = cue.pos {
        // The anchor point is where the alignment says on the block.
        let (ax, ay) = (pic.x + ax * pic.w, pic.y + ay * pic.h);
        let x = match col {
            0 => ax,
            2 => ax - w,
            _ => ax - w * 0.5,
        };
        let y = match cue.align {
            7..=9 => ay,
            4..=6 => ay - h * 0.5,
            _ => ay - h,
        };
        return (x, y);
    }
    let x = match col {
        0 => pic.x + pic.w * 0.05,
        2 => pic.x + pic.w * 0.95 - w,
        _ => pic.x + (pic.w - w) * 0.5,
    };
    let y = match cue.align {
        7..=9 => {
            let margin = cue.margin_v.map_or(pic.h * 0.05, |m| m * pic.h);
            let y = stack.top.max(pic.y + margin);
            stack.top = y + h + px * 0.2;
            y
        }
        4..=6 => pic.y + (pic.h - h) * 0.5,
        _ => {
            // The cue's own margin moves with the user's position, so the cues of a styled
            // file keep their places relative to each other.
            let margin = cue
                .margin_v
                .map_or(DEFAULT_POSITION + lift, |m| m + lift)
                .clamp(0.0, 0.95);
            let y = (stack.bottom.min(pic.y + pic.h - margin * pic.h) - h).max(pic.y);
            stack.bottom = y - px * 0.2;
            y
        }
    };
    (x, y)
}

/// The height of a cue's lines (logical px): `size` from the settings, `scale` the cue's own.
pub fn line_px(pic: Rect, size: f32, scale: f32) -> f32 {
    (pic.h * 0.048 * size * scale).clamp(11.0, 120.0)
}

/// Draws `cue` over the picture `pic` (logical px) as `look` says.
pub fn draw_text(
    dl: &mut DrawList,
    text: &mut Text,
    cue: &TextCue,
    pic: Rect,
    look: &Look,
    stack: &mut Stack,
) {
    let f = flat(cue, look.override_styles);
    let px = line_px(pic, look.size, f.scale);
    let mut style = TextStyle::new(px);
    style.italic = f.italic;
    style.weight = if f.bold { 700.0 } else { 600.0 };
    let col = (cue.align.clamp(1, 9) - 1) % 3;
    let align = match col {
        0 => Align::Start,
        2 => Align::End,
        _ => Align::Center,
    };
    let block = if look.family.is_empty() {
        text.layout(&f.text, style, Some(pic.w * 0.9), align)
    } else {
        text.layout_family(&look.family, &f.text, style, Some(pic.w * 0.9), align)
    };
    let (w, h) = (block.width, block.height);
    let (x, y) = place(cue, pic, (w, h), px, look.lift(), stack);
    let o = (px * 0.07).clamp(1.0, 4.0);
    if look.boxed {
        // A box a little larger than the text, instead of the shadow.
        let (pw, ph) = (px * 0.3, px * 0.08);
        let r = Rect::new(x - pw, y - ph, w + 2.0 * pw, h + 2.0 * ph);
        dl.rect(r, px * 0.18, Color::rgba(0.0, 0.0, 0.0, 0.62));
    } else {
        // A soft shadow.
        text.draw(
            dl,
            &block,
            x + o * 0.8,
            y + o * 1.6,
            Color::rgba(0.0, 0.0, 0.0, 0.45),
        );
    }
    // The outline (eight offsets), then the fill.
    if let Some(oc) = f.outline.or(look.outline) {
        let oc = rgba(oc, 0.92);
        for (dx, dy) in [
            (-1.0, -1.0),
            (0.0, -1.0),
            (1.0, -1.0),
            (-1.0, 0.0),
            (1.0, 0.0),
            (-1.0, 1.0),
            (0.0, 1.0),
            (1.0, 1.0),
        ] {
            text.draw(dl, &block, x + dx * o, y + dy * o, oc);
        }
    }
    let fill = rgba(look.fill, 1.0);
    text.draw_colored(dl, &block, x, y, &mut |byte| {
        let c = f
            .colours
            .iter()
            .rev()
            .find(|(at, _)| *at <= byte)
            .and_then(|(_, c)| *c);
        c.map_or(fill, |c| rgba(c, 1.0))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mostly_italic_cues_are_italic() {
        let c = specular_subs::markup::parse("<i>most of it</i> no");
        let f = flat(&c, false);
        assert!(f.italic && !f.bold);
        assert_eq!(f.text, "most of it no");
        let c = specular_subs::markup::parse("a\n<font color=\"red\">b</font>");
        let f = flat(&c, false);
        assert_eq!(f.text, "a\nb");
        assert_eq!(f.colours.last(), Some(&(2, Some([255, 0, 0, 255]))));
    }

    #[test]
    fn overriding_drops_the_files_colours_and_sizes() {
        let mut c = specular_subs::markup::parse("<font color=\"red\">a</font> b");
        c.lines[0][0].outline = Some([0, 0, 255, 255]);
        c.lines[0][0].scale = 2.0;
        let own = flat(&c, false);
        assert_eq!(own.colours[0].1, Some([255, 0, 0, 255]));
        assert_eq!(own.outline, Some([0, 0, 255, 255]));
        assert_eq!(own.scale, 2.0);
        let ours = flat(&c, true);
        assert!(ours.colours.iter().all(|c| c.1.is_none()));
        assert_eq!((ours.outline, ours.scale), (None, 1.0));
        // Plain white from a styled file counts as the default colour.
        let mut w = specular_subs::markup::parse("a");
        w.lines[0][0].colour = Some([255, 255, 255, 255]);
        assert_eq!(flat(&w, false).colours[0].1, None);
    }

    const PIC: Rect = Rect {
        x: 10.0,
        y: 20.0,
        w: 1000.0,
        h: 500.0,
    };

    fn at(c: &TextCue, position: f32) -> (f32, f32) {
        let look = Look {
            position,
            ..Look::default()
        };
        let mut st = Stack::new(PIC, position);
        place(c, PIC, (200.0, 40.0), 24.0, look.lift(), &mut st)
    }

    #[test]
    fn bottom_cues_move_with_the_position() {
        let c = specular_subs::markup::parse("hi");
        // Default: the block's bottom 5 % above the picture's bottom (520 - 25), centred.
        assert_eq!(at(&c, DEFAULT_POSITION), (410.0, 455.0));
        // 20 %: 100 px above the bottom.
        let (x, y) = at(&c, 0.2);
        assert_eq!(x, 410.0);
        assert!((y - (520.0 - 100.0 - 40.0)).abs() < 0.01, "{y}");
        // At the bottom edge, and at the most: inside the picture.
        assert!((at(&c, 0.0).1 - 480.0).abs() < 0.01);
        let top = at(&c, 0.9).1;
        assert!((top - (520.0 - 450.0 - 40.0)).abs() < 0.01 && top >= PIC.y);
        // Past the top it stays inside the picture.
        let tall = {
            let mut st = Stack::new(PIC, 0.9);
            place(&c, PIC, (200.0, 120.0), 24.0, 0.85, &mut st).1
        };
        assert_eq!(tall, PIC.y);
    }

    #[test]
    fn a_second_bottom_cue_stacks_above_the_first() {
        let c = specular_subs::markup::parse("hi");
        let look = Look {
            position: 0.3,
            ..Look::default()
        };
        let mut st = Stack::new(PIC, look.position);
        let a = place(&c, PIC, (200.0, 40.0), 24.0, look.lift(), &mut st);
        let b = place(&c, PIC, (200.0, 40.0), 24.0, look.lift(), &mut st);
        assert!((b.1 - (a.1 - 40.0 - 24.0 * 0.2)).abs() < 0.01);
    }

    #[test]
    fn cues_placed_by_the_file_keep_their_place() {
        // ASS \pos, and an ASS top cue: the same at any position.
        let mut p = specular_subs::markup::parse("sign");
        p.pos = Some((0.25, 0.5));
        p.align = 5;
        let mut t = specular_subs::markup::parse("top");
        t.align = 8;
        t.margin_v = Some(0.1);
        let mut m = specular_subs::markup::parse("middle");
        m.align = 5;
        for c in [&p, &t, &m] {
            assert_eq!(at(c, DEFAULT_POSITION), at(c, 0.4), "{:?}", c.plain_text());
        }
        assert_eq!(at(&p, 0.4), (10.0 + 250.0 - 100.0, 20.0 + 250.0 - 20.0));
        assert_eq!(at(&t, 0.4).1, 20.0 + 50.0);
        // An ASS bottom cue with its own margin (8 %) moves by as much as the default does:
        // two cues of the file stay as far apart.
        let mut b = specular_subs::markup::parse("bottom");
        b.margin_v = Some(0.08);
        let d0 = at(&b, DEFAULT_POSITION).1 - at(&c_default(), DEFAULT_POSITION).1;
        let d1 = at(&b, 0.3).1 - at(&c_default(), 0.3).1;
        assert!((d0 - d1).abs() < 0.01 && d0 < 0.0, "{d0} {d1}");
    }

    fn c_default() -> TextCue {
        specular_subs::markup::parse("plain")
    }
}
