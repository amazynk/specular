//! GRIB edition 1 messages: indicator, product definition, grid description, bitmap and
//! binary data sections.

use std::sync::Arc;

use crate::bits::{ibm, missing, sint, u8_, u16_, uint};
use crate::grid::{Earth, Grid, GridKind, Rotation};
use crate::tables;
use crate::{DateTime, Error, Field, Forecast, Packing, Result, Scale, TimeUnit, bad, unsupported};

/// The field of the message at `at` and the offset just past it.
pub fn message(file: &[u8], at: usize) -> Result<(Vec<Field>, usize)> {
    let head = file.get(at..at + 8).ok_or(Error::Truncated)?;
    let mut total = uint(head, 4, 3)? as usize;
    // ECMWF's large messages: the length counts 120-octet units.
    let large = total & 0x80_0000 != 0;
    if large {
        total = (total & 0x7F_FFFF) * 120;
    }
    if total < 8 + 28 + 11 + 4 {
        return bad("message length");
    }
    let end = at.checked_add(total).ok_or(Error::Truncated)?;
    let msg = file.get(at..end).ok_or(Error::Truncated)?;
    if !large && &msg[total - 4..] != b"7777" {
        return bad("no 7777 at the message end");
    }

    let pds_len = uint(msg, 8, 3)? as usize;
    let p = msg.get(8..8 + pds_len).ok_or(Error::Truncated)?;
    if pds_len < 28 {
        return bad("product definition section length");
    }
    let table = p[3];
    let centre = u16::from(p[4]);
    let flags = p[7];
    let number = p[8];
    let level_kind = p[9];
    let century = i32::from(p[24]);
    let yy = i32::from(p[12]);
    let year = if yy == 0 {
        century * 100
    } else {
        (century - 1) * 100 + yy
    };
    let reference = DateTime {
        year,
        month: p[13],
        day: p[14],
        hour: p[15],
        minute: p[16],
        second: 0,
    };
    let (unit, mult) = time_unit(p[17])?;
    let (p1, p2) = (i64::from(p[18]), i64::from(p[19]));
    let range = p[20];
    let mut forecast = Forecast {
        unit,
        start: 0,
        end: None,
        statistic: None,
    };
    match range {
        0 | 1 => forecast.start = p1 * mult,
        10 => forecast.start = (p1 * 256 + p2) * mult,
        2..=5 => {
            forecast.start = p1 * mult;
            forecast.end = Some(p2 * mult);
            forecast.statistic = match range {
                3 => Some("average"),
                4 => Some("accumulation"),
                5 => Some("difference"),
                _ => None,
            };
        }
        _ => forecast.start = p1 * mult,
    }
    let decimal = sint(p, 26, 2)? as i32;
    let sub_centre = u16::from(p[25]);
    let level = if tables::level1_is_layer(level_kind) {
        tables::level1(level_kind, Some(f64::from(p[10])), Some(f64::from(p[11])))
    } else {
        tables::level1(level_kind, Some(f64::from(u16_(p, 10)?)), None)
    };
    let param = tables::param1(table, centre, number, &level);

    let mut pos = 8 + pds_len;
    if flags & 0x80 == 0 {
        return unsupported(format!("predefined grid {}", p[6]));
    }
    let gds_len = uint(msg, pos, 3)? as usize;
    let g = msg.get(pos..pos + gds_len).ok_or(Error::Truncated)?;
    let grid = gds(g)?;
    pos += gds_len;

    let mut bitmap = None;
    if flags & 0x40 != 0 {
        let len = uint(msg, pos, 3)? as usize;
        let b = msg.get(pos..pos + len).ok_or(Error::Truncated)?;
        if len < 6 || u16_(b, 4)? != 0 {
            return unsupported("predefined bitmap");
        }
        if (len - 6) * 8 < grid.points() {
            return bad("bitmap shorter than the grid");
        }
        bitmap = Some(at + pos + 6..at + pos + len);
        pos += len;
    }

    let mut bds_len = uint(msg, pos, 3)? as usize;
    if large && bds_len < 120 {
        bds_len = total - 4 - pos;
    }
    let d = msg.get(pos..pos + bds_len).ok_or(Error::Truncated)?;
    if bds_len < 11 {
        return bad("binary data section length");
    }
    let bflags = d[3];
    if bflags & 0x80 != 0 {
        return unsupported("spherical harmonics");
    }
    if bflags & 0x40 != 0 {
        return unsupported("second-order packing");
    }
    let scale = Scale {
        reference: ibm(d, 6)?,
        binary: sint(d, 4, 2)? as i32,
        decimal,
        bits: d[10],
    };
    let values = match &bitmap {
        Some(r) => {
            let bm = &file[r.clone()];
            let n = grid.points();
            let full = bm[..n / 8]
                .iter()
                .map(|b| b.count_ones() as usize)
                .sum::<usize>();
            let rest = if n % 8 != 0 {
                (bm[n / 8] >> (8 - n % 8)).count_ones() as usize
            } else {
                0
            };
            full + rest
        }
        None => grid.points(),
    };
    Ok((
        vec![Field {
            edition: 1,
            message: at..end,
            centre,
            sub_centre,
            reference,
            param,
            level,
            forecast,
            ensemble: None,
            grid: Arc::new(grid),
            packing: Packing::Simple(scale),
            values,
            data: at + pos + 11..at + pos + bds_len,
            bitmap,
            product: u16::from(range),
        }],
        end,
    ))
}

/// Time unit and multiplier for GRIB1 code table 4.
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
        13 => (TimeUnit::Minute, 15),
        14 => (TimeUnit::Minute, 30),
        254 => (TimeUnit::Second, 1),
        n => return unsupported(format!("time unit {n}")),
    })
}

/// A 24-bit sign-and-magnitude angle in millidegrees.
fn milli(g: &[u8], at: usize) -> Result<f64> {
    Ok(sint(g, at, 3)? as f64 * 1e-3)
}

fn gds(g: &[u8]) -> Result<Grid> {
    if g.len() < 32 {
        return bad("grid description section length");
    }
    let nv = usize::from(g[3]);
    let pv = usize::from(g[4]);
    let kind = g[5];
    match kind {
        0 | 4 | 10 | 14 => {
            let ni_missing = missing(g, 6, 2);
            let ni = u32::from(u16_(g, 6)?);
            let nj = u32::from(u16_(g, 8)?);
            let lat1 = milli(g, 10)?;
            let lon1 = milli(g, 13)?;
            let res = g[16];
            let lat2 = milli(g, 17)?;
            let lon2 = milli(g, 20)?;
            let given = res & 0x80 != 0;
            let di =
                (given && !missing(g, 23, 2)).then(|| f64::from(u16_(g, 23).unwrap_or(0)) * 1e-3);
            let n_or_dj = u16_(g, 25)?;
            let scan = g[27];
            let earth = earth(res);
            let rotation = if matches!(kind, 10 | 14) {
                Some(Rotation {
                    pole_lat: milli(g, 32)?,
                    pole_lon: milli(g, 35)?,
                    angle: ibm(g, 38)?,
                })
            } else {
                None
            };
            let pl = if ni_missing {
                if pv == 0 || pv == 255 {
                    return bad("reduced grid without a list of row points");
                }
                let start = pv - 1 + 4 * nv;
                let mut v = Vec::with_capacity(nj as usize);
                for j in 0..nj as usize {
                    v.push(u32::from(u16_(g, start + 2 * j)?));
                }
                Some(v)
            } else {
                None
            };
            let gk = if matches!(kind, 4 | 14) {
                GridKind::Gaussian {
                    lat1,
                    lon1,
                    lat2,
                    lon2,
                    n: u32::from(n_or_dj),
                    di,
                }
            } else {
                GridKind::LatLon {
                    lat1,
                    lon1,
                    lat2,
                    lon2,
                    di,
                    dj: (given && !missing(g, 25, 2)).then(|| f64::from(n_or_dj) * 1e-3),
                }
            };
            Ok(Grid {
                kind: gk,
                rotation,
                ni: if ni_missing { 0 } else { ni },
                nj,
                pl,
                scan,
                earth,
            })
        }
        3 | 5 => {
            let ni = u32::from(u16_(g, 6)?);
            let nj = u32::from(u16_(g, 8)?);
            let lat1 = milli(g, 10)?;
            let lon1 = milli(g, 13)?;
            let earth = earth(g[16]);
            let lov = milli(g, 17)?;
            let dx = uint(g, 20, 3)? as f64;
            let dy = uint(g, 23, 3)? as f64;
            let south = u8_(g, 26)? & 0x80 != 0;
            let scan = g[27];
            let gk = if kind == 5 {
                GridKind::PolarStereographic {
                    lat1,
                    lon1,
                    lov,
                    lad: if south { -60.0 } else { 60.0 },
                    dx,
                    dy,
                    south,
                }
            } else {
                let latin1 = milli(g, 28)?;
                GridKind::Lambert {
                    lat1,
                    lon1,
                    lov,
                    lad: latin1,
                    latin1,
                    latin2: milli(g, 31)?,
                    dx,
                    dy,
                    south,
                }
            };
            Ok(Grid {
                kind: gk,
                rotation: None,
                ni,
                nj,
                pl: None,
                scan,
                earth,
            })
        }
        t => unsupported(format!("GRIB1 grid type {t}")),
    }
}

/// The Earth of the resolution and component flags (octet 17).
fn earth(flags: u8) -> Earth {
    if flags & 0x40 != 0 {
        Earth::oblate(
            6_378_160.0,
            6_378_160.0 * (1.0 - 1.0 / 297.0),
            Some("IAU 1965"),
        )
    } else {
        Earth::sphere(6_367_470.0)
    }
}
