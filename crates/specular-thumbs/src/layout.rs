//! Grid geometry: pure maths, no drawing, so it is tested here and reused by the Library.
//!
//! Coordinates are logical pixels; y grows down from the top of the content (the scroll
//! offset is subtracted when drawing). A layout is rebuilt when the width, tile size, mode,
//! captions or the item list change: O(n), well under a millisecond for 100 000 items.
//! Everything per frame (visible range, hit test) is a binary search over rows.

use std::ops::Range;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Mode {
    /// Square tiles, image cropped to fill.
    #[default]
    Squares,
    /// Rows of equal height, tiles as wide as their image's aspect ratio (no crop).
    Justified,
    /// Square tiles, whole image fitted inside.
    Uncropped,
    /// One row per item with columns of facts.
    Details,
}

impl Mode {
    pub const ALL: [Mode; 4] = [
        Mode::Squares,
        Mode::Justified,
        Mode::Uncropped,
        Mode::Details,
    ];
    pub fn id(self) -> &'static str {
        match self {
            Mode::Squares => "squares",
            Mode::Justified => "justified",
            Mode::Uncropped => "uncropped",
            Mode::Details => "details",
        }
    }
    pub fn from_id(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.id() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Params {
    /// Content width.
    pub width: f32,
    /// Tile size (square side, or justified row height).
    pub tile: f32,
    pub gap: f32,
    /// Outer padding left/right/top/bottom.
    pub pad: f32,
    /// Caption height under each tile (0: none).
    pub caption: f32,
    /// Group header height.
    pub header: f32,
    /// Row height in the details view.
    pub detail_row: f32,
}

impl Default for Params {
    fn default() -> Params {
        Params {
            width: 800.0,
            tile: 160.0,
            gap: 6.0,
            pad: 12.0,
            caption: 0.0,
            header: 36.0,
            detail_row: 28.0,
        }
    }
}

/// What the layout needs to know about an item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Item {
    /// Width / height of the image as displayed; 1.0 while unknown.
    pub aspect: f32,
    /// Group number; items of a group must be contiguous. A change starts a header.
    pub group: u32,
}

impl Default for Item {
    fn default() -> Item {
        Item {
            aspect: 1.0,
            group: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Cell {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Cell {
    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
    pub fn center(&self) -> (f32, f32) {
        (self.x + self.w * 0.5, self.y + self.h * 0.5)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// Top of the row's tiles.
    pub y: f32,
    /// Tile height (the caption goes below).
    pub h: f32,
    pub first: usize,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Header {
    pub group: u32,
    pub y: f32,
    /// First item of the group.
    pub first: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Nav {
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
}

#[derive(Clone, Debug, Default)]
pub struct Layout {
    pub mode: Mode,
    pub params: Option<Params>,
    pub cells: Vec<Cell>,
    pub rows: Vec<Row>,
    pub headers: Vec<Header>,
    /// Total content height.
    pub height: f32,
    /// Room to scroll past the content, for something floating over the view's end (the
    /// gallery's selection bar): not laid out, kept by the caller across rebuilds.
    pub tail: f32,
    /// Columns of the square modes (0 for the others).
    pub columns: usize,
}

/// Aspect ratios are clamped so panoramas and slivers stay usable tiles.
const MIN_ASPECT: f32 = 0.33;
const MAX_ASPECT: f32 = 3.0;

impl Layout {
    pub fn build(items: &[Item], p: &Params, mode: Mode) -> Layout {
        let mut l = Layout {
            mode,
            params: Some(*p),
            cells: Vec::with_capacity(items.len()),
            ..Layout::default()
        };
        let inner = (p.width - 2.0 * p.pad).max(1.0);
        let grouped = items
            .first()
            .is_some_and(|f| items.iter().any(|i| i.group != f.group));
        let mut y = p.pad;
        let mut i = 0;
        if mode != Mode::Details && mode != Mode::Justified {
            let cols = (((inner + p.gap) / (p.tile + p.gap)).floor() as usize).max(1);
            l.columns = cols;
        }
        while i < items.len() {
            let group = items[i].group;
            let end = items[i..]
                .iter()
                .position(|it| it.group != group)
                .map_or(items.len(), |n| i + n);
            if grouped {
                l.headers.push(Header { group, y, first: i });
                y += p.header;
            }
            match mode {
                Mode::Details => {
                    for k in i..end {
                        l.rows.push(Row {
                            y,
                            h: p.detail_row,
                            first: k,
                            count: 1,
                        });
                        l.cells.push(Cell {
                            x: p.pad,
                            y,
                            w: inner,
                            h: p.detail_row,
                        });
                        y += p.detail_row;
                    }
                }
                Mode::Squares | Mode::Uncropped => {
                    let cols = l.columns;
                    let side = ((inner - (cols - 1) as f32 * p.gap) / cols as f32).max(1.0);
                    for row_start in (i..end).step_by(cols) {
                        let n = cols.min(end - row_start);
                        l.rows.push(Row {
                            y,
                            h: side,
                            first: row_start,
                            count: n,
                        });
                        for c in 0..n {
                            l.cells.push(Cell {
                                x: p.pad + c as f32 * (side + p.gap),
                                y,
                                w: side,
                                h: side,
                            });
                        }
                        y += side + p.caption + p.gap;
                    }
                }
                Mode::Justified => {
                    let mut k = i;
                    while k < end {
                        // Fill a row at the target height until it is at least as wide as
                        // the space, then scale it to fit exactly.
                        let mut sum = 0.0;
                        let mut n = 0;
                        while k + n < end {
                            sum += items[k + n].aspect.clamp(MIN_ASPECT, MAX_ASPECT);
                            n += 1;
                            if sum * p.tile + (n - 1) as f32 * p.gap >= inner {
                                break;
                            }
                        }
                        let gaps = (n - 1) as f32 * p.gap;
                        let full = sum * p.tile + gaps >= inner;
                        // The last row of a group keeps the target height (not stretched),
                        // unless it would overflow.
                        let h = if full {
                            ((inner - gaps) / sum).max(1.0)
                        } else {
                            p.tile
                        };
                        l.rows.push(Row {
                            y,
                            h,
                            first: k,
                            count: n,
                        });
                        let mut x = p.pad;
                        for j in 0..n {
                            let w = items[k + j].aspect.clamp(MIN_ASPECT, MAX_ASPECT) * h;
                            // Round the last tile so the row's right edge is exact.
                            let w = if full && j == n - 1 {
                                p.pad + inner - x
                            } else {
                                w
                            };
                            l.cells.push(Cell { x, y, w, h });
                            x += w + p.gap;
                        }
                        y += h + p.caption + p.gap;
                        k += n;
                    }
                }
            }
            i = end;
        }
        l.height = y + p.pad;
        l
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    /// Rows overlapping `y0..y1` (content coordinates).
    pub fn rows_in(&self, y0: f32, y1: f32) -> Range<usize> {
        let cap = self.params.map_or(0.0, |p| p.caption);
        let a = self.rows.partition_point(|r| r.y + r.h + cap < y0);
        let b = self.rows.partition_point(|r| r.y < y1);
        a..b.max(a)
    }

    /// Items overlapping `y0..y1`, in order.
    pub fn items_in(&self, y0: f32, y1: f32) -> Range<usize> {
        let r = self.rows_in(y0, y1);
        if r.is_empty() {
            return 0..0;
        }
        let first = self.rows[r.start].first;
        let last = &self.rows[r.end - 1];
        first..last.first + last.count
    }

    /// The row holding item `i`.
    pub fn row_of(&self, i: usize) -> Option<usize> {
        let r = self.rows.partition_point(|r| r.first + r.count <= i);
        (r < self.rows.len()).then_some(r)
    }

    /// The item under a point (tile or its caption), if any.
    pub fn hit(&self, x: f32, y: f32) -> Option<usize> {
        let cap = self.params.map_or(0.0, |p| p.caption);
        let r = self.rows.partition_point(|r| r.y + r.h + cap < y);
        let row = self.rows.get(r)?;
        if y < row.y || y >= row.y + row.h + cap {
            return None;
        }
        (row.first..row.first + row.count).find(|&i| {
            let c = &self.cells[i];
            x >= c.x && x < c.x + c.w
        })
    }

    /// Items whose tiles intersect a rectangle (rubber-band selection).
    pub fn items_in_rect(&self, x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<usize> {
        let (x0, x1) = (x0.min(x1), x0.max(x1));
        let (y0, y1) = (y0.min(y1), y0.max(y1));
        self.items_in(y0, y1)
            .filter(|&i| {
                let c = &self.cells[i];
                c.x < x1 && c.x + c.w > x0 && c.y < y1 && c.y + c.h > y0
            })
            .collect()
    }

    /// Where keyboard focus goes from item `i`. `view_h` is the viewport height (pages).
    pub fn nav(&self, i: usize, nav: Nav, view_h: f32) -> usize {
        let n = self.cells.len();
        if n == 0 {
            return 0;
        }
        let i = i.min(n - 1);
        match nav {
            Nav::Left => i.saturating_sub(1),
            Nav::Right => (i + 1).min(n - 1),
            Nav::Home => 0,
            Nav::End => n - 1,
            Nav::Up | Nav::Down => {
                let Some(r) = self.row_of(i) else { return i };
                let target = if nav == Nav::Up {
                    match r.checked_sub(1) {
                        Some(t) => t,
                        None => return i,
                    }
                } else if r + 1 < self.rows.len() {
                    r + 1
                } else {
                    return i;
                };
                self.nearest_in_row(target, self.cells[i].center().0)
            }
            Nav::PageUp | Nav::PageDown => {
                let c = self.cells[i];
                let dy = (view_h - c.h).max(c.h);
                let y = if nav == Nav::PageUp {
                    c.y - dy
                } else {
                    c.y + dy
                };
                let y = y.clamp(0.0, self.height);
                let r = self
                    .rows
                    .partition_point(|r| r.y + r.h <= y)
                    .min(self.rows.len() - 1);
                // At least one row, so short views still move.
                let r = match nav {
                    Nav::PageUp => r.min(self.row_of(i).unwrap_or(0).saturating_sub(1)),
                    _ => r.max((self.row_of(i).unwrap_or(0) + 1).min(self.rows.len() - 1)),
                };
                self.nearest_in_row(r, c.center().0)
            }
        }
    }

    /// [`nav`](Self::nav), going round at the ends: Left from the first item to the last, Right
    /// from the last to the first, Up from the top row to the bottom one and Down back (keeping
    /// the column). Home, End and the page keys stop at the ends as before.
    pub fn nav_wrapping(&self, i: usize, nav: Nav, view_h: f32) -> usize {
        let j = self.nav(i, nav, view_h);
        let n = self.cells.len();
        if n < 2 || j != i.min(n - 1) {
            return j;
        }
        let x = self.cells[j].center().0;
        match nav {
            Nav::Left => n - 1,
            Nav::Right => 0,
            Nav::Up => self.nearest_in_row(self.rows.len() - 1, x),
            Nav::Down => self.nearest_in_row(0, x),
            _ => j,
        }
    }

    fn nearest_in_row(&self, r: usize, x: f32) -> usize {
        let row = self.rows[r];
        (row.first..row.first + row.count)
            .min_by(|&a, &b| {
                let da = (self.cells[a].center().0 - x).abs();
                let db = (self.cells[b].center().0 - x).abs();
                da.total_cmp(&db)
            })
            .unwrap_or(row.first)
    }

    /// The scroll offset that shows item `i` (unchanged when it is already fully visible),
    /// keeping `margin` around it and room for a sticky header at the top.
    pub fn scroll_to_show(&self, i: usize, scroll: f32, view_h: f32, top_inset: f32) -> f32 {
        let Some(c) = self.cells.get(i) else {
            return scroll;
        };
        let cap = self.params.map_or(0.0, |p| p.caption);
        let margin = self.params.map_or(0.0, |p| p.gap);
        let top = c.y - margin - top_inset;
        let bottom = c.y + c.h + cap + margin;
        let s = if top < scroll {
            top
        } else if bottom > scroll + view_h {
            bottom - view_h
        } else {
            scroll
        };
        self.clamp_scroll(s, view_h)
    }

    pub fn clamp_scroll(&self, s: f32, view_h: f32) -> f32 {
        s.clamp(0.0, (self.extent() - view_h).max(0.0))
    }

    /// How far it scrolls: the content and the room past it.
    pub fn extent(&self) -> f32 {
        self.height + self.tail
    }

    /// The item under (or nearest to) screen y `py` and where in it, for keeping it in place
    /// across a relayout ("the tile under the pointer stays put").
    pub fn anchor(&self, scroll: f32, px: f32, py: f32) -> Option<(usize, f32)> {
        if self.cells.is_empty() {
            return None;
        }
        let y = scroll + py;
        let i = self.hit(px, y).or_else(|| {
            let r = self
                .rows
                .partition_point(|r| r.y + r.h < y)
                .min(self.rows.len() - 1);
            Some(self.nearest_in_row(r, px))
        })?;
        let c = self.cells[i];
        Some((i, (y - c.y) / c.h.max(1.0)))
    }

    /// The scroll offset that puts the anchor back under screen y `py`.
    pub fn scroll_for_anchor(&self, anchor: (usize, f32), py: f32, view_h: f32) -> f32 {
        let Some(c) = self.cells.get(anchor.0) else {
            return 0.0;
        };
        self.clamp_scroll(c.y + anchor.1 * c.h - py, view_h)
    }

    /// The group header pinned at the top of the view and how far it is pushed up by the
    /// next one (`0.0` = fully shown).
    pub fn sticky_header(&self, scroll: f32) -> Option<(usize, f32)> {
        let h = self.params.map_or(0.0, |p| p.header);
        let k = self.headers.partition_point(|hd| hd.y <= scroll);
        let cur = k.checked_sub(1)?;
        let push = self
            .headers
            .get(cur + 1)
            .map_or(0.0, |next| (scroll + h - next.y).max(0.0));
        Some((cur, push))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn items(n: usize) -> Vec<Item> {
        vec![Item::default(); n]
    }

    fn p(width: f32, tile: f32) -> Params {
        Params {
            width,
            tile,
            gap: 10.0,
            pad: 10.0,
            caption: 0.0,
            header: 30.0,
            detail_row: 20.0,
        }
    }

    #[test]
    fn squares_fill_the_width() {
        // inner 780: (780+10)/(150+10) = 4.9 → 4 columns of (780-30)/4 = 187.5.
        let l = Layout::build(&items(10), &p(800.0, 150.0), Mode::Squares);
        assert_eq!(l.columns, 4);
        assert_eq!(l.rows.len(), 3);
        let c = l.cells[3];
        assert!((c.x + c.w - 790.0).abs() < 0.01);
        assert_eq!(c.w, 187.5);
        assert_eq!(l.cells[4].y, 10.0 + 187.5 + 10.0);
        assert_eq!(l.height, 10.0 + 3.0 * 197.5 + 10.0);
        // Never zero columns.
        assert_eq!(
            Layout::build(&items(3), &p(50.0, 150.0), Mode::Uncropped).columns,
            1
        );
    }

    #[test]
    fn justified_rows_are_exact_and_keep_aspect() {
        let mut it = items(7);
        for (k, a) in [1.5, 0.66, 1.0, 2.0, 1.33, 1.0, 0.75]
            .into_iter()
            .enumerate()
        {
            it[k].aspect = a;
        }
        let l = Layout::build(&it, &p(800.0, 150.0), Mode::Justified);
        for row in &l.rows[..l.rows.len() - 1] {
            let last = l.cells[row.first + row.count - 1];
            assert!((last.x + last.w - 790.0).abs() < 0.01, "{row:?}");
            assert!(row.h <= 150.0 * 1.01 && row.h > 50.0);
            let range = row.first..row.first + row.count - 1;
            for (c, item) in l.cells[range.clone()].iter().zip(&it[range]) {
                assert!((c.w / c.h - item.aspect).abs() < 0.01);
            }
        }
        // The last row is not stretched.
        let lr = l.rows.last().unwrap();
        let end = l.cells[lr.first + lr.count - 1];
        assert!(end.x + end.w <= 790.0 + 0.01);
        assert_eq!(l.cells.len(), 7);
    }

    #[test]
    fn groups_start_rows_and_headers() {
        let mut it = items(7);
        for item in &mut it[3..7] {
            item.group = 1;
        }
        let l = Layout::build(&it, &p(800.0, 150.0), Mode::Squares);
        assert_eq!(l.headers.len(), 2);
        assert_eq!(l.headers[1].first, 3);
        assert_eq!(l.rows[1].first, 3);
        assert_eq!(l.cells[0].y, 10.0 + 30.0);
        // Sticky header: at the top, then pushed up by the next.
        assert_eq!(l.sticky_header(0.0), None);
        assert_eq!(l.sticky_header(10.0), Some((0, 0.0)));
        let y1 = l.headers[1].y;
        assert_eq!(l.sticky_header(y1 - 10.0), Some((0, 20.0)));
        assert_eq!(l.sticky_header(y1 + 1.0), Some((1, 0.0)));
        // No headers without groups.
        assert!(
            Layout::build(&items(5), &p(800.0, 150.0), Mode::Squares)
                .headers
                .is_empty()
        );
    }

    #[test]
    fn visible_range_and_hit_test() {
        let l = Layout::build(&items(100_000), &p(1000.0, 100.0), Mode::Squares);
        assert_eq!(l.len(), 100_000);
        let cols = l.columns;
        let row_h = l.rows[1].y - l.rows[0].y;
        let r = l.items_in(row_h * 500.0, row_h * 500.0 + 300.0);
        assert!(r.start >= 499 * cols && r.end <= 504 * cols, "{r:?}");
        let c = l.cells[1234];
        assert_eq!(l.hit(c.x + 1.0, c.y + 1.0), Some(1234));
        assert_eq!(l.hit(c.x - 5.0, c.y + 1.0), None); // in the gap
        assert_eq!(l.hit(5.0, 5.0), None); // padding
        let sel = l.items_in_rect(0.0, 0.0, 1000.0, 1.0 + 10.0);
        assert_eq!(sel.len(), cols);
        let d = Layout::build(&items(10), &p(500.0, 100.0), Mode::Details);
        assert_eq!(d.hit(200.0, 10.0 + 20.0 * 3.5), Some(3));
    }

    #[test]
    fn keyboard_navigation() {
        let l = Layout::build(&items(10), &p(800.0, 150.0), Mode::Squares); // 4 columns
        assert_eq!(l.nav(5, Nav::Up, 400.0), 1);
        assert_eq!(l.nav(5, Nav::Down, 400.0), 9);
        assert_eq!(l.nav(7, Nav::Down, 400.0), 9); // short last row: nearest
        assert_eq!(l.nav(1, Nav::Up, 400.0), 1);
        assert_eq!(l.nav(0, Nav::Left, 400.0), 0);
        assert_eq!(l.nav(9, Nav::Right, 400.0), 9);
        assert_eq!(l.nav(3, Nav::Right, 400.0), 4); // wraps to the next row
        assert_eq!(l.nav(4, Nav::End, 400.0), 9);
        assert_eq!(l.nav_wrapping(0, Nav::Left, 400.0), 9);
        assert_eq!(l.nav_wrapping(9, Nav::Right, 400.0), 0);
        assert_eq!(l.nav_wrapping(1, Nav::Up, 400.0), 9); // short last row: nearest
        assert_eq!(l.nav_wrapping(9, Nav::Down, 400.0), 1);
        assert_eq!(l.nav_wrapping(3, Nav::Right, 400.0), 4); // inside: as `nav`
        assert_eq!(l.nav_wrapping(9, Nav::End, 400.0), 9);
        assert_eq!(l.nav(1, Nav::PageDown, 200.0), 5);
        assert_eq!(l.nav(9, Nav::PageUp, 1000.0), 1);
        let big = Layout::build(&items(400), &p(800.0, 150.0), Mode::Squares);
        let row_h = big.rows[1].y - big.rows[0].y;
        let k = big.nav(2, Nav::PageDown, row_h * 4.0);
        assert_eq!(k % 4, 2);
        assert!(k / 4 >= 3);
    }

    #[test]
    fn scrolling_and_anchoring() {
        let l = Layout::build(&items(200), &p(800.0, 150.0), Mode::Squares);
        let view = 600.0;
        // Visible: unchanged; below: scrolled so its bottom shows; above: its top.
        assert_eq!(l.scroll_to_show(0, 0.0, view, 0.0), 0.0);
        let c = l.cells[40];
        let s = l.scroll_to_show(40, 0.0, view, 0.0);
        assert!((s - (c.y + c.h + 10.0 - view)).abs() < 0.01);
        assert!(l.scroll_to_show(0, 5000.0, view, 0.0) <= 0.0 + 0.01);
        assert_eq!(l.clamp_scroll(1e9, view), l.height - view);
        // Zoom: the tile under the pointer stays at the same screen y.
        let scroll = 3000.0;
        let (px, py) = (300.0, 250.0);
        let a = l.anchor(scroll, px, py).unwrap();
        let l2 = Layout::build(&items(200), &p(800.0, 90.0), Mode::Squares);
        let s2 = l2.scroll_for_anchor(a, py, view);
        let c2 = l2.cells[a.0];
        let screen_y = c2.y + a.1 * c2.h - s2;
        assert!((screen_y - py).abs() < 0.01, "{screen_y}");
    }
}
