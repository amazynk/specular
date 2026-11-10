//! Orientation labels: the patient direction (L R A P S I, combined when oblique) of each
//! edge of the image as shown, from the frame's [`PatientAxes`] through the view, so they
//! follow rotation, flips, zoom and pan.

use specular_dicom::PatientAxes;
use specular_gfx::{Align, DrawList, Rect, Text, TextStyle};
use specular_render::View;
use specular_theme::Color;
use specular_theme::metrics::{FONT_BODY, RADIUS_SM};
use specular_widgets::accesskit;

/// Which edge of the image a label marks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    Left,
    Right,
    Top,
    Bottom,
}

/// A label at an edge's midpoint (logical pixels), inside the visible part of the image.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Edge {
    pub side: Side,
    pub text: String,
    pub at: (f32, f32),
}

/// The labels of the four edges: the direction of screen right, left, down and up in stored
/// pixels, named by `axes`, placed on the edges of the image's visible rectangle. `s`: the
/// display scale (physical per logical pixel).
pub(crate) fn edges(axes: &PatientAxes, view: &View, s: f32) -> Vec<Edge> {
    let a = view.stored_from_screen();
    let o = a.apply((0.0, 0.0));
    let r = a.apply((1.0, 0.0));
    let d = a.apply((0.0, 1.0));
    let right = (f64::from(r.0 - o.0), f64::from(r.1 - o.1));
    let down = (f64::from(d.0 - o.0), f64::from(d.1 - o.1));
    let img = view.image_screen_rect();
    let vp = view.viewport;
    let x0 = img.x.max(vp.x);
    let y0 = img.y.max(vp.y);
    let x1 = (img.x + img.w).min(vp.x + vp.w);
    let y1 = (img.y + img.h).min(vp.y + vp.h);
    if x1 <= x0 || y1 <= y0 {
        return Vec::new();
    }
    let (mx, my) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    [
        (Side::Left, (-right.0, -right.1), (x0, my)),
        (Side::Right, right, (x1, my)),
        (Side::Top, (-down.0, -down.1), (mx, y0)),
        (Side::Bottom, down, (mx, y1)),
    ]
    .into_iter()
    .filter_map(|(side, (dx, dy), p)| {
        Some(Edge {
            side,
            text: axes.label(dx, dy)?,
            at: (p.0 / s, p.1 / s),
        })
    })
    .collect()
}

/// Draws the labels just inside their edges on a dark backdrop. They name anatomy, so they
/// are not mirrored in right-to-left layouts.
pub(crate) fn draw(dl: &mut DrawList, text: &mut Text, edges: &[Edge], ts: f32) {
    let st = TextStyle::new(FONT_BODY * 1.15 * ts).weight(600.0);
    let pad = 3.0 * ts;
    let m = 6.0 * ts;
    for e in edges {
        let b = text.layout(&e.text, st, None, Align::Start);
        let (w, h) = (b.width + 2.0 * pad, b.height + 2.0 * pad);
        let (x, y) = match e.side {
            Side::Left => (e.at.0 + m, e.at.1 - h / 2.0),
            Side::Right => (e.at.0 - m - w, e.at.1 - h / 2.0),
            Side::Top => (e.at.0 - w / 2.0, e.at.1 + m),
            Side::Bottom => (e.at.0 - w / 2.0, e.at.1 - m - h),
        };
        dl.rect(
            Rect::new(x, y, w, h),
            RADIUS_SM,
            Color::rgba(0.0, 0.0, 0.0, 0.5),
        );
        text.draw(dl, &b, x + pad, y + pad, Color::WHITE);
    }
}

/// Node id of the labels' accessibility node.
const A11Y_ID: u64 = 0x5800_0000_0000_0001;

/// The labels' accessible name: each edge's letters, "–" for an edge without one.
pub(crate) fn a11y_label(edges: &[Edge]) -> String {
    let of = |side| {
        edges
            .iter()
            .find(|e| e.side == side)
            .map_or_else(|| "–".to_owned(), |e| e.text.clone())
    };
    specular_i18n::t!(
        "medical-orientation-a11y",
        left = of(Side::Left),
        right = of(Side::Right),
        top = of(Side::Top),
        bottom = of(Side::Bottom)
    )
}

/// One accessibility node naming the four labels, over the visible part of the image they
/// mark (`scale`: physical pixels per logical pixel).
pub(crate) fn a11y(tree: &mut accesskit::TreeUpdate, edges: &[Edge], scale: f64) {
    use accesskit::{Node, NodeId, Role};
    if edges.is_empty() {
        return;
    }
    let xs = edges.iter().map(|e| f64::from(e.at.0));
    let ys = edges.iter().map(|e| f64::from(e.at.1));
    let (x0, x1) = (
        xs.clone().fold(f64::MAX, f64::min),
        xs.fold(f64::MIN, f64::max),
    );
    let (y0, y1) = (
        ys.clone().fold(f64::MAX, f64::min),
        ys.fold(f64::MIN, f64::max),
    );
    let mut n = Node::new(Role::Label);
    n.set_label(a11y_label(edges));
    n.set_bounds(accesskit::Rect {
        x0: x0 * scale,
        y0: y0 * scale,
        x1: x1 * scale,
        y1: y1 * scale,
    });
    if let Some((_, root)) = tree.nodes.iter_mut().find(|(id, _)| *id == NodeId(1)) {
        root.push_child(NodeId(A11Y_ID));
    }
    tree.nodes.push((NodeId(A11Y_ID), n));
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_core::Orientation;
    use specular_render::PxRect;

    fn axial() -> PatientAxes {
        PatientAxes {
            row: [1.0, 0.0, 0.0],
            col: [0.0, 1.0, 0.0],
            from_letters: false,
            anatomy: Default::default(),
        }
    }

    fn texts(o: Orientation) -> Vec<(Side, String)> {
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
        };
        let v = View::fixed((128, 128), o, vp, 2.0, (64.0, 64.0));
        edges(&axial(), &v, 1.0)
            .into_iter()
            .map(|e| (e.side, e.text))
            .collect()
    }

    #[test]
    fn follow_rotation_and_flip() {
        let s = |v: &[(Side, &str)]| -> Vec<(Side, String)> {
            v.iter().map(|(a, b)| (*a, b.to_string())).collect()
        };
        // Axial: the patient's right on the screen's left, anterior at the top.
        assert_eq!(
            texts(Orientation::NORMAL),
            s(&[
                (Side::Left, "R"),
                (Side::Right, "L"),
                (Side::Top, "A"),
                (Side::Bottom, "P")
            ])
        );
        // A quarter turn clockwise: what was on the left is at the top.
        assert_eq!(
            texts(Orientation::from_rotation_flip(1, false)),
            s(&[
                (Side::Left, "P"),
                (Side::Right, "A"),
                (Side::Top, "R"),
                (Side::Bottom, "L")
            ])
        );
        // Mirrored left to right.
        assert_eq!(
            texts(Orientation::from_rotation_flip(0, true)),
            s(&[
                (Side::Left, "L"),
                (Side::Right, "R"),
                (Side::Top, "A"),
                (Side::Bottom, "P")
            ])
        );
    }

    #[test]
    fn oblique_and_offscreen() {
        let a = PatientAxes {
            row: [0.8, -0.6, 0.0],
            col: [0.0, 0.0, -1.0],
            from_letters: false,
            anatomy: Default::default(),
        };
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
        };
        let v = View::fixed((128, 128), Orientation::NORMAL, vp, 2.0, (64.0, 64.0));
        let e = edges(&a, &v, 2.0);
        assert_eq!(e[1].text, "LA");
        assert_eq!(e[0].text, "RP");
        assert_eq!(e[2].text, "S");
        // Labels sit on the visible part of the image, in logical pixels.
        assert_eq!(e[1].at, ((200.0 + 128.0) / 2.0, 75.0));
        // Panned off screen: none.
        let v = View::fixed((128, 128), Orientation::NORMAL, vp, 2.0, (1000.0, 64.0));
        assert!(edges(&a, &v, 1.0).is_empty());
    }

    /// One node under the window, named by the four letters, over the labelled image.
    #[test]
    fn accessibility_node_names_the_edges() {
        let vp = PxRect {
            x: 0.0,
            y: 0.0,
            w: 400.0,
            h: 300.0,
        };
        let v = View::fixed((128, 128), Orientation::NORMAL, vp, 2.0, (64.0, 64.0));
        let e = edges(&axial(), &v, 1.0);
        let mut tree = accesskit::TreeUpdate {
            nodes: vec![(
                accesskit::NodeId(1),
                accesskit::Node::new(accesskit::Role::Window),
            )],
            tree: None,
            tree_id: accesskit::TreeId::ROOT,
            focus: accesskit::NodeId(1),
        };
        a11y(&mut tree, &e, 2.0);
        assert_eq!(tree.nodes.len(), 2);
        assert_eq!(tree.nodes[0].1.children(), &[accesskit::NodeId(A11Y_ID)]);
        let n = &tree.nodes[1].1;
        assert_eq!(n.role(), accesskit::Role::Label);
        let name = n.label().unwrap();
        for l in ["R", "L", "A", "P"] {
            assert!(name.contains(l), "{name}");
        }
        let b = n.bounds().unwrap();
        assert_eq!((b.x0, b.y0, b.x1, b.y1), (144.0, 44.0, 656.0, 556.0));
        a11y(&mut tree, &[], 2.0);
        assert_eq!(tree.nodes.len(), 2);
    }
}
