//! A shot's camera and lens names to Lensfun profiles.
//!
//! - **Camera**: the Exif maker and model as the database has them, case and spacing aside
//!   ([`crate::names::plain`]); a model with or without the maker's name in front matches
//!   too, and so does a maker known by its first word ("NIKON" for "Nikon Corporation").
//!   Its mount says which lenses fit (its own and its `<compat>` mounts), its crop factor
//!   scales the profile. An unknown camera fits every lens; its crop factor then comes from
//!   the 35 mm equivalent focal length when the file has one.
//! - **Lens**: the Exif lens name split into words as Lensfun's documentation says
//!   ([`crate::names::words`]). A lens **matches** when every word of the Exif name is in its
//!   model or maker and every number of its model is in the Exif name (so "24-70mm" never
//!   matches a 24-105). Matches rank by the share of words the two names have in common;
//!   two different lenses ranked first are **ambiguous** and none is picked (the caller asks).
//!   Lenses outside the shot's focal length do not fit; only calibrated lenses count.
//! - **Fixed-lens cameras** (a mount name starting with a lower-case letter, as the
//!   database's convention is): the lens of that mount, whatever the file calls it.
//! - The same lens calibrated on several cameras is one choice: the calibration whose crop
//!   factor is closest to the camera's without being larger (Lensfun's advice: models get
//!   imprecise on crop factors smaller than the calibration's), else the closest.

use std::collections::HashMap;

use crate::names::{plain, words};
use crate::{Db, Lens};

/// How many lenses the picker offers at most.
pub const MAX_CANDIDATES: usize = 24;

/// What is known of a shot.
#[derive(Clone, Copy, Debug, Default)]
pub struct Query<'a> {
    pub maker: &'a str,
    pub model: &'a str,
    /// The Exif lens name.
    pub lens: Option<&'a str>,
    /// Focal length in mm.
    pub focal: Option<f64>,
    /// The 35 mm equivalent focal length (Exif FocalLengthIn35mmFilm).
    pub focal35: Option<f64>,
}

/// The outcome of a lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// The first candidate is the lens.
    One,
    /// Two or more lenses match equally well: the first ones in the candidates.
    Ambiguous,
    /// The file names a lens the database does not have (for this camera and focal length).
    NoMatch,
    /// The file does not name the lens (and the camera's lens is not fixed).
    NoLensName,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Match {
    /// Index into [`Db::cameras`].
    pub camera: Option<usize>,
    /// The camera's crop factor, when known.
    pub crop: Option<f64>,
    /// Indices into [`Db::lenses`], best first, at most [`MAX_CANDIDATES`].
    pub candidates: Vec<usize>,
    pub pick: Pick,
    /// How many candidates match equally well (with [`Pick::One`]: 1).
    pub tied: usize,
}

impl Match {
    /// The picked lens, if one.
    pub fn picked(&self) -> Option<usize> {
        (self.pick == Pick::One)
            .then(|| self.candidates.first().copied())
            .flatten()
    }
}

/// The camera of `maker` and `model` (see the module documentation).
pub fn find_camera(db: &Db, maker: &str, model: &str) -> Option<usize> {
    let pm = plain(maker);
    let pmodel = plain(model);
    if pmodel.is_empty() {
        return None;
    }
    let first = |s: &str| s.split(' ').next().unwrap_or("").to_string();
    let mut pool: Vec<usize> = db.camera_indices(maker).to_vec();
    if pool.is_empty() {
        let f = first(&pm);
        if f.is_empty() {
            return None;
        }
        pool = db
            .camera_makers()
            .filter(|k| first(k) == f)
            .flat_map(|k| db.camera_indices(k).iter().copied())
            .collect();
        pool.sort_unstable();
        pool.dedup();
    }
    let strip = |model: &str, maker: &str| -> String {
        let f = first(maker);
        model
            .strip_prefix(&f)
            .map_or(model, str::trim_start)
            .to_string()
    };
    let same = |c: &crate::Camera| {
        let cm = plain(&c.model);
        let ce = plain(&c.model_en);
        cm == pmodel
            || ce == pmodel
            || strip(&cm, &plain(&c.maker)) == strip(&pmodel, &pm)
            || ce == strip(&pmodel, &pm)
    };
    // Without a variant first (variants cannot be told apart from the file).
    let mut best: Option<usize> = None;
    for &i in &pool {
        let c = &db.cameras[i];
        if same(c) && best.is_none_or(|b| db.cameras[b].variant.is_some() && c.variant.is_none()) {
            best = Some(i);
        }
    }
    best
}

/// Whether the lens fits the shot's focal length (2 % or half a millimetre of slack).
fn fits_focal(l: &Lens, focal: Option<f64>) -> bool {
    match (focal, l.focal) {
        (Some(f), Some((lo, hi))) => {
            let slack = (0.02 * f).max(0.5);
            f >= lo - slack && f <= hi + slack
        }
        _ => true,
    }
}

fn is_number(w: &str) -> bool {
    w.bytes().all(|b| b.is_ascii_digit())
}

/// Whether a lens name says nothing (empty, dashes, "0mm f/0").
fn says_nothing(w: &[String]) -> bool {
    w.iter().all(|w| {
        (is_number(w) && w.trim_start_matches('0').is_empty())
            || w == "mm"
            || w.chars().all(|c| !c.is_alphanumeric())
    })
}

/// How many words two sorted, deduplicated lists share.
fn common(a: &[String], b: &[String]) -> usize {
    let (mut i, mut j, mut n) = (0, 0, 0);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                n += 1;
                i += 1;
                j += 1;
            }
        }
    }
    n
}

/// Keeps one calibration per lens model (see the module documentation), in order.
fn one_per_model(db: &Db, v: Vec<usize>, crop: Option<f64>) -> Vec<usize> {
    let rank = |i: usize| -> (u8, f64) {
        let Some(c) = crop else { return (0, 0.0) };
        let x = db.lenses[i].crop;
        if x <= c + 1e-6 {
            (0, c - x)
        } else {
            (1, x - c)
        }
    };
    let mut out: Vec<usize> = Vec::new();
    let mut at: HashMap<&str, usize> = HashMap::new();
    for i in v {
        match at.get(db.lenses[i].plain_key.as_str()) {
            Some(&p) => {
                let (a, b) = (rank(i), rank(out[p]));
                if a.0 < b.0 || (a.0 == b.0 && a.1 < b.1) {
                    out[p] = i;
                }
            }
            None => {
                at.insert(&db.lenses[i].plain_key, out.len());
                out.push(i);
            }
        }
    }
    out
}

/// Looks up a shot (see the module documentation).
pub fn find(db: &Db, q: &Query) -> Match {
    let camera = find_camera(db, q.maker, q.model);
    let cam = camera.map(|i| &db.cameras[i]);
    let crop = cam.map(|c| c.crop).or_else(|| match (q.focal, q.focal35) {
        (Some(f), Some(f35)) if f > 0.0 && f35 > 0.0 => Some(f35 / f),
        _ => None,
    });
    let fixed = cam.is_some_and(|c| c.mount.chars().next().is_some_and(char::is_lowercase));
    let pool: Vec<usize> = match cam {
        Some(c) if fixed => db.lenses_on(&[plain(&c.mount)]),
        Some(c) => db.lenses_on(&db.fitting_mounts(&c.mount)),
        None => (0..db.lenses.len()).collect(),
    };
    let pool: Vec<usize> = pool
        .into_iter()
        .filter(|&i| db.lenses[i].has_calibration() && fits_focal(&db.lenses[i], q.focal))
        .collect();
    let pool = one_per_model(db, pool, crop);
    let done = |candidates: Vec<usize>, pick: Pick, tied: usize| Match {
        camera,
        crop,
        candidates,
        pick,
        tied,
    };
    let name_words: Vec<String> = q.lens.map(words).unwrap_or_default();
    let named = !says_nothing(&name_words);
    if fixed && (!named || pool.len() == 1) {
        let n = pool.len();
        let pick = match n {
            0 => Pick::NoMatch,
            1 => Pick::One,
            _ => Pick::Ambiguous,
        };
        let mut c = pool;
        c.truncate(MAX_CANDIDATES);
        return done(c, pick, n.min(MAX_CANDIDATES));
    }
    if !named {
        // Ask: the camera's maker's lenses first; with the shot's focal length known, the
        // narrowest range around it first (a prime of that length, then short zooms), so
        // the lens is near the top rather than cut off by name order; then by name.
        let cmaker = cam.map(|c| plain(&c.maker_en));
        let span = |l: &Lens| -> u64 {
            match (q.focal, l.focal) {
                (Some(_), Some((lo, hi))) => ((hi - lo).max(0.0) * 10.0) as u64,
                _ => u64::MAX,
            }
        };
        let mut c = pool;
        c.sort_by_cached_key(|&i| {
            let l = &db.lenses[i];
            (
                cmaker.as_deref() != Some(plain(&l.maker_en).as_str()),
                span(l),
                l.sort_name.clone(),
            )
        });
        c.truncate(MAX_CANDIDATES);
        return done(c, Pick::NoLensName, 0);
    }
    let mut e = name_words;
    e.sort_unstable();
    e.dedup();
    let mut strict: Vec<(usize, f64)> = Vec::new();
    let mut loose: Vec<(usize, f64)> = Vec::new();
    for &i in &pool {
        let l = &db.lenses[i];
        let n = common(&e, &l.words);
        if n == 0 {
            continue;
        }
        let s = n as f64 / (e.len() + l.words.len() - n) as f64;
        let covered = n == e.len()
            || e.iter().all(|w| {
                l.words.binary_search(w).is_ok() || l.maker_words.binary_search(w).is_ok()
            });
        let numbers = l
            .words
            .iter()
            .filter(|w| is_number(w))
            .all(|w| e.binary_search(w).is_ok());
        if covered && numbers {
            strict.push((i, s));
        } else {
            loose.push((i, s));
        }
    }
    let order = |a: &(usize, f64), b: &(usize, f64)| {
        b.1.total_cmp(&a.1)
            .then_with(|| db.lenses[a.0].sort_name.cmp(&db.lenses[b.0].sort_name))
    };
    strict.sort_by(order);
    loose.sort_by(order);
    let tied = strict
        .iter()
        .take_while(|x| (x.1 - strict[0].1).abs() < 1e-9)
        .count();
    let pick = match tied {
        0 => Pick::NoMatch,
        1 => Pick::One,
        _ => Pick::Ambiguous,
    };
    let mut c: Vec<usize> = strict.iter().chain(&loose).map(|x| x.0).collect();
    c.truncate(MAX_CANDIDATES);
    done(c, pick, tied.min(MAX_CANDIDATES))
}
