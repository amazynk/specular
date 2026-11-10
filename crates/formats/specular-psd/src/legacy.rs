//! Legacy layer effects (`lrFX`, Photoshop 5 to 6) as the effects descriptor later versions
//! write, for layers saved without one: drop and inner shadow, outer and inner glow, bevel
//! and colour fill. Sizes and angles are 16.16 fixed point, opacities 0..255, colours a
//! colour space and four 16-bit components. What the old records do not keep (spread,
//! contours, noise) takes the descriptor's defaults.

use crate::desc::{Desc, Value};
use crate::parse::Rd;

/// The descriptor names of blend modes, for [`crate::effects::mode_key`].
const MODES: &[&[u8]] = &[
    b"Nrml",
    b"Dslv",
    b"Drkn",
    b"Mltp",
    b"CBrn",
    b"linearBurn",
    b"darkerColor",
    b"Lghn",
    b"Scrn",
    b"CDdg",
    b"linearDodge",
    b"lighterColor",
    b"Ovrl",
    b"SftL",
    b"HrdL",
    b"vividLight",
    b"linearLight",
    b"pinLight",
    b"hardMix",
    b"Dfrn",
    b"Xclu",
    b"blendSubtraction",
    b"blendDivide",
    b"H   ",
    b"Strt",
    b"Clr ",
    b"Lmns",
];

fn item(k: &[u8], v: Value) -> (Vec<u8>, Value) {
    (k.to_vec(), v)
}

fn en(v: &[u8]) -> Value {
    Value::Enum(Vec::new(), v.to_vec())
}

/// A blend-mode signature and key (`8BIM` + `mul `…) as the descriptor's `Md  `.
fn mode(r: &mut Rd) -> Option<Value> {
    let _sig = r.bytes(4)?;
    let key: [u8; 4] = r.bytes(4)?.try_into().ok()?;
    let name = MODES
        .iter()
        .find(|&&m| crate::effects::mode_key(Some(m)) == key)
        .copied()
        .unwrap_or(b"Nrml");
    Some(en(name))
}

/// A colour (space, four 16-bit components) as a descriptor colour, `None` for spaces left
/// out (only RGB and grey are kept).
fn colour(r: &mut Rd) -> Option<Option<Desc>> {
    let space = r.u16()?;
    let c = [r.u16()?, r.u16()?, r.u16()?, r.u16()?];
    let num = |v: f64| Value::Num(v);
    Some(match space {
        0 => Some(Desc {
            class: b"RGBC".to_vec(),
            items: vec![
                item(b"Rd  ", num(f64::from(c[0]) / 257.0)),
                item(b"Grn ", num(f64::from(c[1]) / 257.0)),
                item(b"Bl  ", num(f64::from(c[2]) / 257.0)),
            ],
        }),
        // Grey: 0..10000 of black.
        8 => Some(Desc {
            class: b"Grsc".to_vec(),
            items: vec![item(b"Gry ", num(f64::from(c[0].min(10000)) / 100.0))],
        }),
        _ => None,
    })
}

fn fixed(r: &mut Rd) -> Option<f64> {
    Some(f64::from(r.i32()?) / 65536.0)
}

fn pct(v: u8) -> Value {
    Value::Num(f64::from(v) / 255.0 * 100.0)
}

/// Layer effects from an `lrFX` block body, as an effects descriptor (`None` when it cannot
/// be read). Effects in colour spaces left out are switched off.
pub(crate) fn effects(b: &[u8]) -> Option<Desc> {
    effects_and_dropped(b).map(|(d, _)| d)
}

/// [`effects`], and whether an effect that is on was switched off (its colour space is left
/// out).
pub(crate) fn effects_and_dropped(b: &[u8]) -> Option<(Desc, bool)> {
    let mut r = Rd::new(b, 0);
    let _version = r.u16()?;
    let count = r.u16()?.min(16);
    let mut items = Vec::new();
    let mut visible = true;
    let mut dropped = false;
    for _ in 0..count {
        let sig = r.bytes(4)?;
        if sig != b"8BIM" {
            break;
        }
        let key: [u8; 4] = r.bytes(4)?.try_into().ok()?;
        let size = r.u32()? as usize;
        let start = r.pos;
        let end = start.checked_add(size).filter(|&e| e <= b.len())?;
        let mut e = Rd::new(&b[..end], start);
        let fx = match &key {
            b"cmnS" => {
                let _v = e.u32()?;
                visible = e.u8()? != 0;
                None
            }
            b"dsdw" | b"isdw" => shadow(&mut e).map(|(d, lost)| {
                let k: &[u8] = if &key == b"dsdw" { b"DrSh" } else { b"IrSh" };
                (k, d, lost)
            }),
            b"oglw" | b"iglw" => glow(&mut e, &key == b"iglw").map(|(d, lost)| {
                let k: &[u8] = if &key == b"oglw" { b"OrGl" } else { b"IrGl" };
                (k, d, lost)
            }),
            b"bevl" => bevel(&mut e).map(|d| (&b"ebbl"[..], d, false)),
            b"sofi" => fill(&mut e).map(|(d, lost)| (&b"SoFi"[..], d, lost)),
            _ => None,
        };
        if let Some((k, d, lost)) = fx {
            dropped |= lost;
            items.push(item(k, Value::Obj(d)));
        }
        r.pos = end;
    }
    items.push(item(b"masterFXSwitch", Value::Bool(visible)));
    Some((
        Desc {
            class: b"null".to_vec(),
            items,
        },
        visible && dropped,
    ))
}

fn shadow(e: &mut Rd) -> Option<(Desc, bool)> {
    let _v = e.u32()?;
    let blur = fixed(e)?;
    let _intensity = fixed(e)?;
    let angle = fixed(e)?;
    let distance = fixed(e)?;
    let clr = colour(e)?;
    let md = mode(e)?;
    let on = e.u8()? != 0;
    let global = e.u8()? != 0;
    let opacity = e.u8()?;
    let lost = on && clr.is_none();
    let mut items = vec![
        item(b"enab", Value::Bool(on && !lost)),
        item(b"Md  ", md),
        item(b"Opct", pct(opacity)),
        item(b"uglg", Value::Bool(global)),
        item(b"lagl", Value::Num(angle)),
        item(b"Dstn", Value::Num(distance)),
        item(b"Ckmt", Value::Num(0.0)),
        item(b"blur", Value::Num(blur)),
    ];
    if let Some(c) = clr {
        items.push(item(b"Clr ", Value::Obj(c)));
    }
    Some((
        Desc {
            class: b"DrSh".to_vec(),
            items,
        },
        lost,
    ))
}

fn glow(e: &mut Rd, inner: bool) -> Option<(Desc, bool)> {
    let v = e.u32()?;
    let blur = fixed(e)?;
    let _intensity = fixed(e)?;
    let clr = colour(e)?;
    let md = mode(e)?;
    let on = e.u8()? != 0;
    let opacity = e.u8()?;
    let lost = on && clr.is_none();
    let mut items = vec![
        item(b"enab", Value::Bool(on && !lost)),
        item(b"Md  ", md),
        item(b"Opct", pct(opacity)),
        item(b"GlwT", en(b"SfBL")),
        item(b"Ckmt", Value::Num(0.0)),
        item(b"blur", Value::Num(blur)),
        item(b"Inpr", Value::Num(50.0)),
    ];
    if inner {
        // Version 2 says whether it glows from the edge (the default) or the centre.
        let edge = if v >= 2 {
            e.u8().unwrap_or(1) != 0
        } else {
            true
        };
        items.push(item(b"glwS", en(if edge { b"SrcE" } else { b"SrcC" })));
    }
    if let Some(c) = clr {
        items.push(item(b"Clr ", Value::Obj(c)));
    }
    Some((
        Desc {
            class: b"OrGl".to_vec(),
            items,
        },
        lost,
    ))
}

fn bevel(e: &mut Rd) -> Option<Desc> {
    let _v = e.u32()?;
    let angle = fixed(e)?;
    // The old dialog's strength does not follow the later depth: left at its default.
    let _strength = fixed(e)?;
    let blur = fixed(e)?;
    let hm = mode(e)?;
    let sm = mode(e)?;
    let hc = colour(e)?;
    let sc = colour(e)?;
    let style = e.u8()?;
    let ho = e.u8()?;
    let so = e.u8()?;
    let on = e.u8()? != 0;
    let global = e.u8()? != 0;
    let down = e.u8()? != 0;
    let half = matches!(style, 3 | 4);
    let style: &[u8] = match style {
        1 => b"OtrB",
        3 => b"Embs",
        4 => b"PlEb",
        5 => b"strokeEmboss",
        _ => b"InrB",
    };
    let mut items = vec![
        item(b"enab", Value::Bool(on)),
        item(b"hglM", hm),
        item(b"hglO", pct(ho)),
        item(b"sdwM", sm),
        item(b"sdwO", pct(so)),
        item(b"bvlT", en(b"SfBL")),
        item(b"bvlS", en(style)),
        item(b"uglg", Value::Bool(global)),
        item(b"lagl", Value::Num(angle)),
        item(b"srgR", Value::Num(100.0)),
        // Embossing styles keep half the size.
        item(b"blur", Value::Num(blur * if half { 2.0 } else { 1.0 })),
        item(b"bvlD", en(if down { b"Out " } else { b"In  " })),
    ];
    if let Some(c) = hc {
        items.push(item(b"hglC", Value::Obj(c)));
    }
    if let Some(c) = sc {
        items.push(item(b"sdwC", Value::Obj(c)));
    }
    Some(Desc {
        class: b"ebbl".to_vec(),
        items,
    })
}

fn fill(e: &mut Rd) -> Option<(Desc, bool)> {
    let _v = e.u32()?;
    let md = mode(e)?;
    let clr = colour(e)?;
    let opacity = e.u8()?;
    let on = e.u8()? != 0;
    let lost = on && clr.is_none();
    let mut items = vec![
        item(b"enab", Value::Bool(on && !lost)),
        item(b"Md  ", md),
        item(b"Opct", pct(opacity)),
    ];
    if let Some(c) = clr {
        items.push(item(b"Clr ", Value::Obj(c)));
    }
    Some((
        Desc {
            class: b"SoFi".to_vec(),
            items,
        },
        lost,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(key: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = b"8BIM".to_vec();
        v.extend_from_slice(key);
        v.extend_from_slice(&(body.len() as u32).to_be_bytes());
        v.extend_from_slice(body);
        v
    }

    fn fx(v: f64) -> [u8; 4] {
        ((v * 65536.0) as i32).to_be_bytes()
    }

    #[test]
    fn a_legacy_drop_shadow_reads_as_its_descriptor() {
        let mut ds = 2u32.to_be_bytes().to_vec();
        for v in [65.0, 0.0, 90.0, 45.0] {
            ds.extend_from_slice(&fx(v));
        }
        // RGB (12, 41, 214).
        ds.extend_from_slice(&[0, 0, 12, 12, 41, 41, 214, 214, 0, 0]);
        ds.extend_from_slice(b"8BIMmul ");
        ds.extend_from_slice(&[1, 1, 133]);
        let mut b = vec![0, 0, 0, 2];
        b.extend(rec(b"cmnS", &[0, 0, 0, 0, 1, 0, 0]));
        b.extend(rec(b"dsdw", &ds));
        let d = effects(&b).unwrap();
        assert_eq!(d.boolean(b"masterFXSwitch"), Some(true));
        let s = d.obj(b"DrSh").unwrap();
        assert_eq!(s.boolean(b"enab"), Some(true));
        assert_eq!(s.num(b"blur"), Some(65.0));
        assert_eq!(s.num(b"lagl"), Some(90.0));
        assert_eq!(s.num(b"Dstn"), Some(45.0));
        assert!((s.num(b"Opct").unwrap() - 52.16).abs() < 0.01);
        assert_eq!(s.enum_value(b"Md  "), Some(&b"Mltp"[..]));
        let c = s.obj(b"Clr ").unwrap();
        assert_eq!(c.num(b"Bl  "), Some(214.0));
    }

    #[test]
    fn truncated_records_are_refused_or_cut_short() {
        assert!(effects(&[0, 0]).is_none());
        let mut b = vec![0, 0, 0, 1];
        b.extend(rec(b"dsdw", &[0, 0, 0, 2, 0]));
        let d = effects(&b).unwrap();
        assert!(d.obj(b"DrSh").is_none());
    }

    /// Oracle: every layer of the fidelity corpus that keeps both the legacy records and
    /// Photoshop's descriptor; the legacy effects read as a descriptor must agree with it on
    /// what is on and on every setting the old records keep.
    #[test]
    #[ignore = "oracle: needs the fidelity corpus"]
    fn legacy_effects_agree_with_the_descriptor() {
        let Some(root) = std::env::var_os("SPECULAR_FIDELITY_CORPUS") else {
            return;
        };
        let mut files = Vec::new();
        let mut stack = vec![std::path::PathBuf::from(root).join("psd")];
        while let Some(d) = stack.pop() {
            for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "psd" || x == "psb") {
                    files.push(p);
                }
            }
        }
        let (mut layers, mut fields, mut off) = (0, 0, 0);
        for f in files {
            let Ok(data) = std::fs::read(&f) else {
                continue;
            };
            let Ok(psd) = crate::parse(&data) else {
                continue;
            };
            for l in psd.layers() {
                let Some(legacy) = l.block(b"lrFX").and_then(|r| data.get(r)) else {
                    continue;
                };
                let Some(real) = [b"lmfx", b"lfx2", b"lfxs"].iter().find_map(|k| {
                    l.block(k)
                        .and_then(|r| data.get(r))
                        .and_then(crate::desc::versioned2)
                }) else {
                    continue;
                };
                let ours = effects(legacy).expect("legacy effects");
                layers += 1;
                for key in [&b"DrSh"[..], b"OrGl", b"IrGl", b"ebbl"] {
                    let (Some(a), Some(b)) = (ours.obj(key), real.obj(key)) else {
                        continue;
                    };
                    let on = |d: &Desc| d.boolean(b"enab") != Some(false);
                    let name = key.escape_ascii().to_string();
                    // Colour spaces left out switch the effect off (noted as not drawn).
                    let dropped = key != b"ebbl" && a.obj(b"Clr ").is_none();
                    if on(a) != on(b) && !dropped {
                        off += 1;
                        println!("# {}: {:?} {name} on differs", f.display(), l.name);
                    }
                    if !on(b) {
                        continue;
                    }
                    for k in [
                        &b"blur"[..],
                        b"lagl",
                        b"Dstn",
                        b"Opct",
                        b"hglO",
                        b"sdwO",
                        b"srgR",
                    ] {
                        if let (Some(x), Some(y)) = (a.num(k), b.num(k)) {
                            fields += 1;
                            // The old dialog's sizes stop at 50 pixels.
                            let capped = k == b"blur" && x == 50.0 && y > 50.0;
                            if (x - y).abs() > 0.5 && !capped {
                                off += 1;
                                println!("# {:?} {name} {}: {x} vs {y}", l.name, k.escape_ascii());
                            }
                        }
                    }
                    for k in [&b"Md  "[..], b"hglM", b"sdwM", b"bvlS"] {
                        if let (Some(x), Some(y)) = (a.enum_value(k), b.enum_value(k)) {
                            fields += 1;
                            if x != y {
                                off += 1;
                                println!(
                                    "# {:?} {name} {}: {} vs {}",
                                    l.name,
                                    k.escape_ascii(),
                                    x.escape_ascii(),
                                    y.escape_ascii()
                                );
                            }
                        }
                    }
                }
            }
        }
        println!("# lrFX vs descriptor: {layers} layers, {fields} settings, {off} differ");
        assert_eq!(off, 0);
    }
}
