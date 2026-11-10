//! GRIB edition 2 messages: sections 0 to 8, with sections 2 to 7 repeating.

use std::ops::Range;
use std::sync::Arc;

use crate::bits::{f32_, missing, sint, u8_, u16_, u32_, uint};
use crate::grid::{Earth, Grid, GridKind, Rotation};
use crate::tables;
use crate::{
    Ccsds, Complex, DateTime, Ensemble, Error, Field, Forecast, Level, Packing, Param, Result,
    Scale, TimeUnit, bad, unsupported,
};

/// The fields of the message at `at` and the offset just past it.
pub fn message(file: &[u8], at: usize) -> Result<(Vec<Field>, usize)> {
    let head = file.get(at..at + 16).ok_or(Error::Truncated)?;
    let discipline = head[6];
    let total = uint(head, 8, 8)? as usize;
    if total < 16 + 4 {
        return bad("message length");
    }
    let end = at.checked_add(total).ok_or(Error::Truncated)?;
    let msg = file.get(at..end).ok_or(Error::Truncated)?;
    if &msg[total - 4..] != b"7777" {
        return bad("no 7777 at the message end");
    }

    let mut out = Vec::new();
    let mut ident: Option<Ident> = None;
    let mut grid: Option<Arc<Grid>> = None;
    let mut product: Option<Product> = None;
    let mut repr: Option<(Packing, usize)> = None;
    let mut bitmap: Option<Range<usize>> = None;
    let mut previous_bitmap: Option<Range<usize>> = None;
    let mut pos = 16;
    while pos + 4 <= total - 4 {
        let len = u32_(msg, pos)? as usize;
        if len < 5 || pos + len > total - 4 {
            return bad("section length");
        }
        let s = &msg[pos..pos + len];
        match s[4] {
            1 => ident = Some(section1(s)?),
            2 => {}
            3 => grid = Some(Arc::new(section3(s)?)),
            4 => {
                product = Some(section4(
                    s,
                    discipline,
                    ident.as_ref().map_or(0, |i| i.centre),
                )?)
            }
            5 => repr = Some(section5(s)?),
            6 => {
                bitmap = match s[5] {
                    0 => {
                        let r = at + pos + 6..at + pos + len;
                        previous_bitmap = Some(r.clone());
                        Some(r)
                    }
                    254 => Some(
                        previous_bitmap
                            .clone()
                            .ok_or_else(|| Error::Malformed("no previous bitmap".into()))?,
                    ),
                    255 => None,
                    n => return unsupported(format!("predefined bitmap {n}")),
                }
            }
            7 => {
                let (Some(id), Some(g), Some(p), Some((packing, values))) =
                    (&ident, &grid, &product, &repr)
                else {
                    return bad("data section before its definitions");
                };
                if let Some(b) = &bitmap
                    && b.len() * 8 < g.points()
                {
                    return bad("bitmap shorter than the grid");
                }
                out.push(Field {
                    edition: 2,
                    message: at..end,
                    centre: id.centre,
                    sub_centre: id.sub_centre,
                    reference: id.reference,
                    param: p.param.clone(),
                    level: p.level.clone(),
                    forecast: p.forecast.clone(),
                    ensemble: p.ensemble,
                    grid: g.clone(),
                    packing: packing.clone(),
                    values: *values,
                    data: at + pos + 5..at + pos + len,
                    bitmap: bitmap.clone(),
                    product: p.template,
                });
            }
            n => return bad(format!("section {n}")),
        }
        pos += len;
    }
    if out.is_empty() {
        return bad("message without data");
    }
    Ok((out, end))
}

struct Ident {
    centre: u16,
    sub_centre: u16,
    reference: DateTime,
}

fn section1(s: &[u8]) -> Result<Ident> {
    Ok(Ident {
        centre: u16_(s, 5)?,
        sub_centre: u16_(s, 7)?,
        reference: DateTime {
            year: i32::from(u16_(s, 12)?),
            month: u8_(s, 14)?,
            day: u8_(s, 15)?,
            hour: u8_(s, 16)?,
            minute: u8_(s, 17)?,
            second: u8_(s, 18)?,
        },
    })
}

/// The figure of the Earth (code table 3.2) from the octets at `at` (octet 15 of
/// templates 3.0 to 3.41).
pub fn earth(s: &[u8], at: usize) -> Result<Earth> {
    let shape = u8_(s, at)?;
    let scaled = |o: usize| -> Result<f64> {
        let f = u8_(s, at + o)?;
        let v = u32_(s, at + o + 1)?;
        Ok(f64::from(v) / 10f64.powi(i32::from(f)))
    };
    Ok(match shape {
        0 => Earth::sphere(6_367_470.0),
        1 => Earth::sphere(scaled(1)?),
        2 => Earth::oblate(
            6_378_160.0,
            6_378_160.0 * (1.0 - 1.0 / 297.0),
            Some("IAU 1965"),
        ),
        3 => Earth::oblate(scaled(6)? * 1e3, scaled(11)? * 1e3, None),
        4 => Earth::oblate(
            6_378_137.0,
            6_378_137.0 * (1.0 - 1.0 / 298.257_222_101),
            Some("GRS 80"),
        ),
        5 => {
            let mut e = Earth::oblate(
                6_378_137.0,
                6_378_137.0 * (1.0 - 1.0 / 298.257_223_563),
                Some("WGS 84"),
            );
            e.wgs84 = true;
            e
        }
        7 => Earth::oblate(scaled(6)?, scaled(11)?, None),
        8 => Earth::sphere(6_371_200.0),
        9 => Earth::oblate(6_377_563.396, 6_356_256.909, Some("Airy 1830")),
        _ => Earth::sphere(6_371_229.0),
    })
}

fn section3(s: &[u8]) -> Result<Grid> {
    if u8_(s, 5)? != 0 {
        return unsupported("predefined grid");
    }
    let pl_octets = u8_(s, 10)? as usize;
    let template = u16_(s, 12)?;
    let earth = earth(s, 14)?;
    match template {
        0 | 1 | 40 | 41 => {
            let ni_missing = missing(s, 30, 4);
            let ni = u32_(s, 30)?;
            let nj = u32_(s, 34)?;
            let basic = u32_(s, 38)?;
            let sub = u32_(s, 42)?;
            let unit = if basic == 0 || missing(s, 38, 4) || missing(s, 42, 4) || sub == 0 {
                1e-6
            } else {
                f64::from(basic) / f64::from(sub)
            };
            let ang = |o: usize| -> Result<f64> { Ok(sint(s, o, 4)? as f64 * unit) };
            let flags = u8_(s, 54)?;
            let lat1 = ang(46)?;
            let lon1 = ang(50)?;
            let lat2 = ang(55)?;
            let lon2 = ang(59)?;
            let di = (flags & 0x20 != 0 && !missing(s, 63, 4))
                .then(|| u32_(s, 63).ok())
                .flatten();
            let di = di.map(|v| f64::from(v) * unit);
            let n_or_dj = u32_(s, 67)?;
            let scan = u8_(s, 71)?;
            let rotation = if matches!(template, 1 | 41) {
                Some(Rotation {
                    pole_lat: ang(72)?,
                    pole_lon: ang(76)?,
                    angle: f64::from(f32_(s, 80)?),
                })
            } else {
                None
            };
            let pl = if ni_missing {
                if pl_octets == 0 || nj == 0 {
                    return bad("reduced grid without a list of row points");
                }
                let start = s
                    .len()
                    .checked_sub(nj as usize * pl_octets)
                    .ok_or(Error::Truncated)?;
                let mut v = Vec::with_capacity(nj as usize);
                for j in 0..nj as usize {
                    v.push(uint(s, start + j * pl_octets, pl_octets)? as u32);
                }
                Some(v)
            } else {
                None
            };
            let kind = if template >= 40 {
                GridKind::Gaussian {
                    lat1,
                    lon1,
                    lat2,
                    lon2,
                    n: n_or_dj,
                    di,
                }
            } else {
                let dj =
                    (flags & 0x10 != 0 && !missing(s, 67, 4)).then(|| f64::from(n_or_dj) * unit);
                GridKind::LatLon {
                    lat1,
                    lon1,
                    lat2,
                    lon2,
                    di,
                    dj,
                }
            };
            Ok(Grid {
                kind,
                rotation,
                ni: if ni_missing { 0 } else { ni },
                nj,
                pl,
                scan,
                earth,
            })
        }
        20 | 30 => {
            let ni = u32_(s, 30)?;
            let nj = u32_(s, 34)?;
            let deg = |o: usize| -> Result<f64> { Ok(sint(s, o, 4)? as f64 * 1e-6) };
            let lat1 = deg(38)?;
            let lon1 = deg(42)?;
            let lad = deg(47)?;
            let lov = deg(51)?;
            let dx = f64::from(u32_(s, 55)?) * 1e-3;
            let dy = f64::from(u32_(s, 59)?) * 1e-3;
            let south = u8_(s, 63)? & 0x80 != 0;
            let scan = u8_(s, 64)?;
            let kind = if template == 20 {
                GridKind::PolarStereographic {
                    lat1,
                    lon1,
                    lov,
                    lad,
                    dx,
                    dy,
                    south,
                }
            } else {
                GridKind::Lambert {
                    lat1,
                    lon1,
                    lov,
                    lad,
                    latin1: deg(65)?,
                    latin2: deg(69)?,
                    dx,
                    dy,
                    south,
                }
            };
            Ok(Grid {
                kind,
                rotation: None,
                ni,
                nj,
                pl: None,
                scan,
                earth,
            })
        }
        t => unsupported(format!("grid template 3.{t}")),
    }
}

struct Product {
    template: u16,
    param: Param,
    level: Level,
    forecast: Forecast,
    ensemble: Option<Ensemble>,
}

/// Time unit and multiplier for code table 4.4.
fn time_unit(code: u8) -> Result<(TimeUnit, i64)> {
    Ok(match code {
        0 => (TimeUnit::Minute, 1),
        1 => (TimeUnit::Hour, 1),
        2 => (TimeUnit::Day, 1),
        3 => (TimeUnit::Month, 1),
        4 => (TimeUnit::Year, 1),
        5 => (TimeUnit::Year, 10),
        6 => (TimeUnit::Year, 30),
        7 => (TimeUnit::Year, 100),
        10 => (TimeUnit::Hour, 3),
        11 => (TimeUnit::Hour, 6),
        12 => (TimeUnit::Hour, 12),
        13 => (TimeUnit::Second, 1),
        n => return unsupported(format!("time unit {n}")),
    })
}

/// A fixed surface: type, scale factor and scaled value at `at`.
fn surface(s: &[u8], at: usize) -> Result<(u8, Option<f64>)> {
    let kind = u8_(s, at)?;
    if kind == 255 || missing(s, at + 1, 1) || missing(s, at + 2, 4) {
        return Ok((kind, None));
    }
    let scale = sint(s, at + 1, 1)? as i32;
    let v = sint(s, at + 2, 4)? as f64;
    Ok((kind, Some(v / 10f64.powi(scale))))
}

fn section4(s: &[u8], discipline: u8, centre: u16) -> Result<Product> {
    let template = u16_(s, 7)?;
    let category = u8_(s, 9)?;
    let number = u8_(s, 10)?;
    // (shift of the octets after 11, octet of the ensemble facts and of the statistics
    // block before that shift) for the templates sharing 4.0's layout.
    let layout: Option<(usize, Option<usize>, Option<usize>)> = match template {
        0 | 2 | 5 | 6 | 15 => Some((0, None, None)),
        1 => Some((0, Some(35), None)),
        8 => Some((0, None, Some(35))),
        9 => Some((0, None, Some(48))),
        10 => Some((0, None, Some(36))),
        11 => Some((0, Some(35), Some(38))),
        12 => Some((0, None, Some(37))),
        40 => Some((2, None, None)),
        41 => Some((2, Some(35), None)),
        42 => Some((2, None, Some(35))),
        43 => Some((2, Some(35), Some(38))),
        _ => None,
    };
    let Some((sh, ens_at, stat_at)) = layout else {
        let level = tables::level2(255, None, None);
        return Ok(Product {
            template,
            param: tables::param2(centre, discipline, category, number, &level),
            level,
            forecast: Forecast {
                unit: TimeUnit::Hour,
                start: 0,
                end: None,
                statistic: None,
            },
            ensemble: None,
        });
    };
    // 1-based octet → index.
    let o = |octet: usize| octet - 1 + sh;
    let (unit, mult) = time_unit(u8_(s, o(18))?)?;
    let start = sint(s, o(19), 4)? * mult;
    let (k1, v1) = surface(s, o(23))?;
    let (k2, v2) = surface(s, o(29))?;
    let v2 = if k2 == 255 || k2 != k1 { None } else { v2 };
    let level = tables::level2(k1, v1, v2);
    let ensemble = match ens_at {
        Some(e) => Some(Ensemble {
            member: u32::from(u8_(s, o(e + 1))?),
            size: Some(u32::from(u8_(s, o(e + 2))?)).filter(|&n| n != 255),
        }),
        None => None,
    };
    let mut forecast = Forecast {
        unit,
        start,
        end: None,
        statistic: None,
    };
    if template == 15 {
        forecast.statistic = tables::statistic(u8_(s, 34)?);
    }
    if let Some(b) = stat_at {
        forecast.statistic = tables::statistic(u8_(s, o(b + 12))?);
        let (lu, lmult) = time_unit(u8_(s, o(b + 14))?)?;
        let length = sint(s, o(b + 15), 4)? * lmult;
        let secs = length * lu.seconds();
        let per = unit.seconds();
        if lu == unit {
            forecast.end = Some(start + length);
        } else if secs % per == 0 {
            forecast.end = Some(start + secs / per);
        } else {
            // Express both in the range's unit.
            forecast.start = start * unit.seconds() / lu.seconds();
            forecast.unit = lu;
            forecast.end = Some(forecast.start + length);
        }
    }
    let param = tables::param2(centre, discipline, category, number, &level);
    Ok(Product {
        template,
        param,
        level,
        forecast,
        ensemble,
    })
}

fn section5(s: &[u8]) -> Result<(Packing, usize)> {
    let values = u32_(s, 5)? as usize;
    let template = u16_(s, 9)?;
    let scale = || -> Result<Scale> {
        Ok(Scale {
            reference: f64::from(f32_(s, 11)?),
            binary: sint(s, 15, 2)? as i32,
            decimal: sint(s, 17, 2)? as i32,
            bits: u8_(s, 19)?,
        })
    };
    let packing = match template {
        0 => Packing::Simple(scale()?),
        2 | 3 => {
            let c = Complex {
                missing: u8_(s, 22)?,
                groups: u32_(s, 31)?,
                width_ref: u8_(s, 35)?,
                width_bits: u8_(s, 36)?,
                length_ref: u32_(s, 37)?,
                length_inc: u8_(s, 41)?,
                last_length: u32_(s, 42)?,
                length_bits: u8_(s, 46)?,
                order: if template == 3 { u8_(s, 47)? } else { 0 },
                octets: if template == 3 { u8_(s, 48)? } else { 0 },
            };
            if c.order > 2 {
                return unsupported(format!("spatial differencing order {}", c.order));
            }
            Packing::Complex(scale()?, c)
        }
        4 => match u8_(s, 11)? {
            1 => Packing::Ieee(4),
            2 => Packing::Ieee(8),
            p => return unsupported(format!("IEEE precision {p}")),
        },
        40 => Packing::Jpeg2000(scale()?),
        41 => Packing::Png(scale()?),
        42 => Packing::Ccsds(
            scale()?,
            Ccsds {
                flags: u8_(s, 21)?,
                block: u8_(s, 22)?,
                rsi: u16_(s, 23)?,
            },
        ),
        t => return unsupported(format!("data representation template 5.{t}")),
    };
    Ok((packing, values))
}
