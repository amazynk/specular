//! A list of measurements on one image, with undo.

use crate::calib::{Calibration, Quantity};
use crate::clinical::{self, Outcome, Template};
use crate::geom::{P, Shape};
use crate::stats::ChannelStats;

/// One measurement.
#[derive(Clone, Debug, PartialEq)]
pub struct Measurement {
    /// Unique within its list (1, 2, …).
    pub id: u32,
    pub shape: Shape,
    pub label: String,
    /// The frame, slice or page it was made on.
    pub frame: Option<u32>,
    pub cal: Calibration,
    /// The names of the statistics' channels (`R`, `G`, `B`, `Y`, `HU`, …).
    pub channels: Vec<String>,
    /// Region statistics per channel, once computed.
    pub stats: Option<Vec<ChannelStats>>,
    /// The statistics came from a reduced-resolution copy of the image.
    pub approximate: bool,
    /// A geometry template's measurement: the shape is a polyline through its points in the
    /// template's order, and its values are the template's readouts, not the path's.
    pub template: Option<Template>,
    /// An inspection probe's measurement (radiograph or crack): the shape is the line,
    /// region or point it was drawn as, and [`Measurement::reading`] holds what it found.
    pub probe: Option<crate::inspect::Probe>,
    pub reading: Option<crate::inspect::Reading>,
}

impl Measurement {
    pub fn new(shape: Shape, cal: Calibration) -> Measurement {
        Measurement {
            id: 0,
            shape,
            label: String::new(),
            frame: None,
            cal,
            channels: Vec::new(),
            stats: None,
            approximate: false,
            template: None,
            probe: None,
            reading: None,
        }
    }

    /// A geometry template's measurement through its points.
    pub fn clinical(template: Template, points: Vec<P>, cal: Calibration) -> Measurement {
        let mut m = Measurement::new(Shape::Polyline(points), cal);
        m.template = Some(template);
        m
    }

    /// A geometry template's readouts and construction lines.
    pub fn outcome(&self) -> Option<Outcome> {
        clinical::compute(self.template?, &self.shape.points(), &self.cal)
    }

    /// An inspection probe's measurement.
    pub fn probed(probe: crate::inspect::Probe, shape: Shape, cal: Calibration) -> Measurement {
        let mut m = Measurement::new(shape, cal);
        m.probe = Some(probe);
        m
    }

    /// The shape's own values, which a template's or a probe's measurement does not have.
    fn own(&self) -> Option<&Shape> {
        (self.template.is_none() && self.probe.is_none()).then_some(&self.shape)
    }

    pub fn length(&self) -> Option<Quantity> {
        self.own()?.length(&self.cal)
    }

    pub fn perimeter(&self) -> Option<Quantity> {
        self.own()?.perimeter(&self.cal)
    }

    pub fn area(&self) -> Option<Quantity> {
        self.own()?.area(&self.cal)
    }

    pub fn angle(&self) -> Option<Quantity> {
        self.own()?.angle(&self.cal)
    }

    /// The headline value: a path's length, an angle, or an area.
    pub fn primary(&self) -> Option<Quantity> {
        self.length()
            .or_else(|| self.angle())
            .or_else(|| self.area())
    }
}

#[derive(Clone, Debug)]
enum Undo {
    Added(Vec<u32>),
    Removed(usize, Box<Measurement>),
    Cleared(Vec<Measurement>),
    Replaced(Box<Measurement>),
}

/// Measurements in the order they were made.
#[derive(Clone, Debug, Default)]
pub struct List {
    items: Vec<Measurement>,
    next: u32,
    undo: Vec<Undo>,
}

/// Undo steps kept.
const UNDO_DEPTH: usize = 200;

impl List {
    pub fn new() -> List {
        List::default()
    }

    pub fn items(&self) -> &[Measurement] {
        &self.items
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn get(&self, id: u32) -> Option<&Measurement> {
        self.items.iter().find(|m| m.id == id)
    }

    /// For derived data (statistics) only: changes here are not undone.
    pub fn get_mut(&mut self, id: u32) -> Option<&mut Measurement> {
        self.items.iter_mut().find(|m| m.id == id)
    }

    pub fn index_of(&self, id: u32) -> Option<usize> {
        self.items.iter().position(|m| m.id == id)
    }

    fn record(&mut self, u: Undo) {
        if self.undo.len() == UNDO_DEPTH {
            self.undo.remove(0);
        }
        self.undo.push(u);
    }

    fn assign(&mut self, mut m: Measurement) -> Measurement {
        self.next += 1;
        m.id = self.next;
        m
    }

    /// Adds a measurement, giving it the next id, which is returned.
    pub fn push(&mut self, m: Measurement) -> u32 {
        let m = self.assign(m);
        let id = m.id;
        self.items.push(m);
        self.record(Undo::Added(vec![id]));
        id
    }

    /// Adds several (an import) as one undo step.
    pub fn extend(&mut self, ms: impl IntoIterator<Item = Measurement>) -> Vec<u32> {
        let ids: Vec<u32> = ms
            .into_iter()
            .map(|m| {
                let m = self.assign(m);
                let id = m.id;
                self.items.push(m);
                id
            })
            .collect();
        if !ids.is_empty() {
            self.record(Undo::Added(ids.clone()));
        }
        ids
    }

    /// Replaces a measurement's shape, label or calibration (undoable).
    pub fn replace(&mut self, m: Measurement) -> bool {
        let Some(i) = self.index_of(m.id) else {
            return false;
        };
        let old = std::mem::replace(&mut self.items[i], m);
        self.record(Undo::Replaced(Box::new(old)));
        true
    }

    pub fn remove(&mut self, id: u32) -> Option<&Measurement> {
        let i = self.index_of(id)?;
        let m = self.items.remove(i);
        self.record(Undo::Removed(i, Box::new(m)));
        match self.undo.last() {
            Some(Undo::Removed(_, m)) => Some(m),
            _ => None,
        }
    }

    pub fn clear(&mut self) {
        if !self.items.is_empty() {
            let all = std::mem::take(&mut self.items);
            self.record(Undo::Cleared(all));
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Reverts the last change; false when there is none.
    pub fn undo(&mut self) -> bool {
        let Some(u) = self.undo.pop() else {
            return false;
        };
        match u {
            Undo::Added(ids) => self.items.retain(|m| !ids.contains(&m.id)),
            Undo::Removed(i, m) => self.items.insert(i.min(self.items.len()), *m),
            Undo::Cleared(all) => self.items = all,
            Undo::Replaced(m) => {
                if let Some(i) = self.index_of(m.id) {
                    self.items[i] = *m;
                }
            }
        }
        true
    }
}
