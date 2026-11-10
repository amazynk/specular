//! Lensfun profiles for raws without lens data of their own (Canon, Nikon, Pentax and
//! others).
//!
//! A raw whose file has no corrections of its own (no maker tables in
//! [`specular_raw::SensorMeta::lens`], no DNG `OpcodeList3` warp or vignette) is looked up in
//! Lensfun's database ([`specular_lensfun`]) by its camera and lens names. [`Shot`] holds the
//! outcome: the lenses to offer, each with its profile at the shot's focal length, aperture
//! and focus distance, and whether one was picked. The lens module's `lensfun` field chooses:
//! empty for the automatic pick, [`NONE`] for none, else a lens's [`Candidate::key`]. Two
//! lenses that match equally well are never chosen between: none applies until the person
//! picks one.
//!
//! The lookup runs where the raw is read (off the UI thread); the database is loaded once,
//! on first use ([`specular_lensfun::bundled`]).

use std::sync::Arc;

use specular_lensfun::lookup::{self, Pick, Query};
use specular_lensfun::model::Profile;

/// The `lensfun` field's value for "no profile".
pub const NONE: &str = "none";

/// Focus distance when the file does not say (metres; the database's "far").
const FAR: f64 = 1000.0;

/// A lens the picker offers.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// What the lens module's `lensfun` field stores ([`specular_lensfun::Lens::key`]).
    pub key: String,
    /// "Canon EF 24-70mm f/2.8L II USM".
    pub name: String,
    /// The profile at this shot.
    pub profile: Arc<Profile>,
}

/// One raw's Lensfun lookup (see the module documentation).
#[derive(Clone, Debug, PartialEq)]
pub struct Shot {
    /// The lens name the file gives, if any.
    pub lens_name: Option<String>,
    /// Best first.
    pub candidates: Vec<Candidate>,
    pub pick: Pick,
    /// How many of the first candidates match equally well ([`Pick::Ambiguous`]).
    pub tied: usize,
}

impl Shot {
    /// The lens the `lensfun` field `setting` chooses, if any.
    pub fn chosen(&self, setting: &str) -> Option<&Candidate> {
        match setting {
            "" => (self.pick == Pick::One)
                .then(|| self.candidates.first())
                .flatten(),
            NONE => None,
            key => self.candidates.iter().find(|c| c.key == key),
        }
    }
}

/// The facts of a raw file the lookup needs: camera maker and model, lens name, focal
/// length, f-number, 35 mm equivalent focal length and focus distance.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    pub maker: String,
    pub model: String,
    pub lens: Option<String>,
    pub focal: Option<f64>,
    pub aperture: Option<f64>,
    pub focal35: Option<f64>,
    pub distance: Option<f64>,
}

impl Facts {
    /// Reads them from a raw file (its head is enough).
    pub fn of_raw(data: &[u8]) -> Option<Facts> {
        let f = specular_raw::parse(data).ok()?;
        let i = f.info();
        let mut out = Facts {
            maker: i.make.clone(),
            model: i.model.clone(),
            lens: i.lens.clone().filter(|l| !l.trim().is_empty()),
            focal: i.focal_length.filter(|v| *v > 0.0),
            aperture: i.f_number.filter(|v| *v > 0.0),
            focal35: None,
            distance: None,
        };
        if let Some(exif) = specular_convert::source_meta(data).exif {
            let entry = |tag: u16| {
                exif.exif
                    .iter()
                    .chain(&exif.ifd0)
                    .find(|e| e.tag == tag)
                    .map(|e| e.data.as_slice())
            };
            let u32_at = |d: &[u8], i: usize| -> Option<u32> {
                Some(u32::from_le_bytes(d.get(i..i + 4)?.try_into().ok()?))
            };
            // FocalLengthIn35mmFilm (SHORT).
            out.focal35 = entry(0xA405)
                .and_then(|d| Some(f64::from(u16::from_le_bytes([*d.first()?, *d.get(1)?]))))
                .filter(|v| *v > 0.0);
            // SubjectDistance (RATIONAL, metres; 0xFFFFFFFF: infinity, 0: unknown).
            out.distance = entry(0x9206).and_then(|d| {
                let (n, dn) = (u32_at(d, 0)?, u32_at(d, 4)?);
                if n == u32::MAX {
                    Some(FAR)
                } else {
                    (n != 0 && dn != 0).then(|| f64::from(n) / f64::from(dn))
                }
            });
        }
        Some(out)
    }
}

/// Looks `facts` up in `db` and makes the candidates' profiles.
pub fn lookup(db: &specular_lensfun::Db, facts: &Facts) -> Shot {
    let m = lookup::find(
        db,
        &Query {
            maker: &facts.maker,
            model: &facts.model,
            lens: facts.lens.as_deref(),
            focal: facts.focal,
            focal35: facts.focal35,
        },
    );
    let candidates = m
        .candidates
        .iter()
        .map(|&i| {
            let l = &db.lenses[i];
            // A shot without a focal length: the lens's shortest (a prime's only one).
            let focal = facts.focal.or(l.focal.map(|f| f.0)).unwrap_or(50.0);
            let aperture = facts.aperture.or(l.aperture.map(|a| a.0)).unwrap_or(8.0);
            let crop = m.crop.unwrap_or(l.crop);
            Candidate {
                key: l.key(),
                name: l.display_name(),
                profile: Arc::new(l.profile(focal, aperture, facts.distance.unwrap_or(FAR), crop)),
            }
        })
        .collect();
    Shot {
        lens_name: facts.lens.clone(),
        candidates,
        pick: m.pick,
        tied: m.tied,
    }
}

/// The lookup for a raw (`data` the file) whose sensor data has no lens corrections of its
/// own; `None` for one that has them, or when the database cannot be read. Loads the
/// database on first use: call off the UI thread.
pub fn shot_of_raw(sensor: &specular_raw::SensorData, data: &[u8]) -> Option<Arc<Shot>> {
    if !needs_profile(sensor) {
        return None;
    }
    let facts = Facts::of_raw(data)?;
    let db = specular_lensfun::bundled()?;
    Some(Arc::new(lookup(db, &facts)))
}

/// Whether a raw has no lens corrections of its own (maker tables, DNG warps or vignettes).
pub fn needs_profile(sensor: &specular_raw::SensorData) -> bool {
    sensor.meta.lens.as_ref().is_none_or(|l| l.is_empty())
        && super::dng_steps(&sensor.meta.opcodes[2]).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped database: a Canon body and its Exif lens name pick the profile; the
    /// profile is evaluated at the shot (35 mm between the 28 and 50 mm calibrations).
    #[test]
    fn lookup_in_the_shipped_database() {
        let db = specular_lensfun::bundled().expect("database");
        let facts = Facts {
            maker: "Canon".into(),
            model: "Canon EOS 5D Mark III".into(),
            lens: Some("EF24-70mm f/2.8L II USM".into()),
            focal: Some(40.0),
            aperture: Some(4.0),
            focal35: None,
            distance: None,
        };
        let shot = lookup(db, &facts);
        assert_eq!(shot.pick, Pick::One);
        let c = shot.chosen("").expect("picked");
        assert_eq!(c.name, "Canon EF 24-70mm f/2.8L II USM");
        assert!(c.profile.distortion.is_some() && c.profile.vignetting.is_some());
        assert!(shot.chosen(NONE).is_none());
        // Full frame on full frame: √(1 + aspect²) of the calibration frame.
        let g = c.profile.geometry_scale;
        assert!((1.7..1.9).contains(&g), "{g}");
        // No lens name: nothing is guessed; lenses are offered.
        let shot = lookup(
            db,
            &Facts {
                lens: None,
                ..facts
            },
        );
        assert_eq!(shot.pick, Pick::NoLensName);
        assert!(shot.chosen("").is_none());
        assert!(!shot.candidates.is_empty());
    }
}
