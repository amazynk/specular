//! Advanced B-reps as a STEP AP214 file for a tessellator: IFC's B-rep and NURBS entities
//! are ISO 10303-42's own, renamed (`IFCADVANCEDFACE` → `ADVANCED_FACE`) and given the
//! name STEP's representation items carry first; rational B-splines become STEP's complex
//! instances, surface curves their 3D curve. Each B-rep is a product named `B<number>`
//! with a shape representation of its own, its length unit declared as a metre (the
//! tessellator's output is scaled back by the caller).

use std::collections::{HashMap, HashSet};
use std::fmt::Write;

use super::Ctx;

/// STEP name of an IFC entity, and whether it takes a leading name.
fn step_name(ty: &str) -> Option<(&'static str, bool)> {
    Some(match ty {
        "IFCADVANCEDBREP" | "IFCADVANCEDBREPWITHVOIDS" => ("MANIFOLD_SOLID_BREP", true),
        "IFCCLOSEDSHELL" => ("CLOSED_SHELL", true),
        "IFCOPENSHELL" => ("OPEN_SHELL", true),
        "IFCADVANCEDFACE" => ("ADVANCED_FACE", true),
        "IFCFACESURFACE" => ("FACE_SURFACE", true),
        "IFCFACE" => ("FACE", true),
        "IFCFACEOUTERBOUND" => ("FACE_OUTER_BOUND", true),
        "IFCFACEBOUND" => ("FACE_BOUND", true),
        "IFCEDGELOOP" => ("EDGE_LOOP", true),
        "IFCPOLYLOOP" => ("POLY_LOOP", true),
        "IFCVERTEXLOOP" => ("VERTEX_LOOP", true),
        "IFCORIENTEDEDGE" => ("ORIENTED_EDGE", true),
        "IFCEDGECURVE" => ("EDGE_CURVE", true),
        "IFCEDGE" => ("EDGE", true),
        "IFCVERTEXPOINT" => ("VERTEX_POINT", true),
        "IFCCARTESIANPOINT" => ("CARTESIAN_POINT", true),
        "IFCDIRECTION" => ("DIRECTION", true),
        "IFCVECTOR" => ("VECTOR", true),
        "IFCAXIS2PLACEMENT3D" => ("AXIS2_PLACEMENT_3D", true),
        "IFCAXIS2PLACEMENT2D" => ("AXIS2_PLACEMENT_2D", true),
        "IFCAXIS1PLACEMENT" => ("AXIS1_PLACEMENT", true),
        "IFCPLANE" => ("PLANE", true),
        "IFCCYLINDRICALSURFACE" => ("CYLINDRICAL_SURFACE", true),
        "IFCCONICALSURFACE" => ("CONICAL_SURFACE", true),
        "IFCSPHERICALSURFACE" => ("SPHERICAL_SURFACE", true),
        "IFCTOROIDALSURFACE" => ("TOROIDAL_SURFACE", true),
        "IFCBSPLINESURFACEWITHKNOTS" => ("B_SPLINE_SURFACE_WITH_KNOTS", true),
        "IFCBSPLINECURVEWITHKNOTS" => ("B_SPLINE_CURVE_WITH_KNOTS", true),
        "IFCRECTANGULARTRIMMEDSURFACE" => ("RECTANGULAR_TRIMMED_SURFACE", true),
        "IFCLINE" => ("LINE", true),
        "IFCCIRCLE" => ("CIRCLE", true),
        "IFCELLIPSE" => ("ELLIPSE", true),
        "IFCPOLYLINE" => ("POLYLINE", true),
        "IFCTRIMMEDCURVE" => ("TRIMMED_CURVE", true),
        "IFCCOMPOSITECURVE" => ("COMPOSITE_CURVE", true),
        "IFCCOMPOSITECURVESEGMENT" => ("COMPOSITE_CURVE_SEGMENT", false),
        _ => return None,
    })
}

/// Typed values IFC writes inside geometry.
fn typed_name(t: &[u8]) -> Option<&'static str> {
    Some(match t {
        b"IFCPARAMETERVALUE" => "PARAMETER_VALUE",
        b"IFCLENGTHMEASURE" => "LENGTH_MEASURE",
        b"IFCPOSITIVELENGTHMEASURE" => "POSITIVE_LENGTH_MEASURE",
        b"IFCPLANEANGLEMEASURE" => "PLANE_ANGLE_MEASURE",
        b"IFCREAL" => "REAL",
        _ => return None,
    })
}

/// The argument list of `#id` from `(` to its `)`.
fn args_text<'a>(ctx: &Ctx<'a>, id: u32) -> Option<&'a [u8]> {
    let raw = ctx.s.raw(id)?;
    let mut depth = 0usize;
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'(' => depth += 1,
            b')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(&raw[..=i]);
                }
            }
            b'\'' => {
                i += 1;
                while i < raw.len() {
                    if raw[i] == b'\'' {
                        if raw.get(i + 1) == Some(&b'\'') {
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// The top-level arguments of an argument list (without its parentheses).
fn split(args: &[u8]) -> Vec<&[u8]> {
    let inner = &args[1..args.len() - 1];
    let mut out = Vec::new();
    let (mut depth, mut start, mut i) = (0usize, 0usize, 0usize);
    while i < inner.len() {
        match inner[i] {
            b'(' => depth += 1,
            b')' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                out.push(&inner[start..i]);
                start = i + 1;
            }
            b'\'' => {
                i += 1;
                while i < inner.len() {
                    if inner[i] == b'\'' {
                        if inner.get(i + 1) == Some(&b'\'') {
                            i += 1;
                        } else {
                            break;
                        }
                    }
                    i += 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    out.push(&inner[start..]);
    out
}

/// `text` with references substituted and IFC typed values renamed; `None` when a typed
/// value is not one STEP has.
fn rewrite(text: &[u8], subst: &HashMap<u32, u32>, refs: &mut Vec<u32>) -> Option<String> {
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if c == b'\'' {
            let start = i;
            i += 1;
            while i < text.len() {
                if text[i] == b'\'' {
                    if text.get(i + 1) == Some(&b'\'') {
                        i += 1;
                    } else {
                        break;
                    }
                }
                i += 1;
            }
            i = (i + 1).min(text.len());
            out.push_str(&String::from_utf8_lossy(&text[start..i]));
            continue;
        }
        if c == b'#' {
            let start = i + 1;
            let mut j = start;
            while j < text.len() && text[j].is_ascii_digit() {
                j += 1;
            }
            let n: u32 = std::str::from_utf8(&text[start..j]).ok()?.parse().ok()?;
            let n = subst.get(&n).copied().unwrap_or(n);
            refs.push(n);
            let _ = write!(out, "#{n}");
            i = j;
            continue;
        }
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < text.len() && (text[i].is_ascii_alphanumeric() || text[i] == b'_') {
                i += 1;
            }
            let word = &text[start..i];
            if text.get(i) == Some(&b'(') {
                out.push_str(typed_name(word)?);
            } else {
                out.push_str(&String::from_utf8_lossy(word));
            }
            continue;
        }
        out.push(c as char);
        i += 1;
    }
    Some(out)
}

/// The STEP instances of one B-rep (all it references), or `None` when it uses entities
/// STEP does not share.
fn instances(ctx: &Ctx, brep: u32, done: &mut HashSet<u32>, out: &mut String) -> Option<()> {
    let mut lines: Vec<String> = Vec::new();
    let mut stack = vec![brep];
    let mut local: HashSet<u32> = HashSet::new();
    let subst: HashMap<u32, u32> = HashMap::new();
    while let Some(id) = stack.pop() {
        if done.contains(&id) || !local.insert(id) {
            continue;
        }
        let ty = ctx.s.ty(id)?;
        let args = args_text(ctx, id)?;
        let mut refs = Vec::new();
        let line = match ty {
            "IFCSURFACECURVE" | "IFCSEAMCURVE" | "IFCINTERSECTIONCURVE" => {
                // The 3D curve stands in: written under this number.
                let parts = split(args);
                let c3 = rewrite(parts.first()?, &subst, &mut refs)?;
                let target: u32 = c3.strip_prefix('#')?.parse().ok()?;
                let tty = ctx.s.ty(target)?;
                let (name, named) = step_name(tty)?;
                let targs = args_text(ctx, target)?;
                let mut trefs = Vec::new();
                let body = rewrite(&targs[1..targs.len() - 1], &subst, &mut trefs)?;
                stack.extend(trefs);
                refs.clear();
                if named {
                    format!("#{id}={name}('',{body});")
                } else {
                    format!("#{id}={name}({body});")
                }
            }
            "IFCRATIONALBSPLINESURFACEWITHKNOTS" => {
                let p: Vec<String> = split(args)
                    .iter()
                    .map(|x| rewrite(x, &subst, &mut refs))
                    .collect::<Option<_>>()?;
                if p.len() < 13 {
                    return None;
                }
                format!(
                    "#{id}=(BOUNDED_SURFACE()B_SPLINE_SURFACE({})B_SPLINE_SURFACE_WITH_KNOTS({})GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_SURFACE({})REPRESENTATION_ITEM('')SURFACE());",
                    p[0..7].join(","),
                    p[7..12].join(","),
                    p[12]
                )
            }
            "IFCRATIONALBSPLINECURVEWITHKNOTS" => {
                let p: Vec<String> = split(args)
                    .iter()
                    .map(|x| rewrite(x, &subst, &mut refs))
                    .collect::<Option<_>>()?;
                if p.len() < 9 {
                    return None;
                }
                format!(
                    "#{id}=(BOUNDED_CURVE()B_SPLINE_CURVE({})B_SPLINE_CURVE_WITH_KNOTS({})CURVE()GEOMETRIC_REPRESENTATION_ITEM()RATIONAL_B_SPLINE_CURVE({})REPRESENTATION_ITEM(''));",
                    p[0..5].join(","),
                    p[5..8].join(","),
                    p[8]
                )
            }
            "IFCADVANCEDBREPWITHVOIDS" => {
                // The outer shell only.
                let parts = split(args);
                let outer = rewrite(parts.first()?, &subst, &mut refs)?;
                format!("#{id}=MANIFOLD_SOLID_BREP('',{outer});")
            }
            _ => {
                let (name, named) = step_name(ty)?;
                let body = rewrite(&args[1..args.len() - 1], &subst, &mut refs)?;
                if named {
                    if body.trim().is_empty() {
                        format!("#{id}={name}('');")
                    } else {
                        format!("#{id}={name}('',{body});")
                    }
                } else {
                    format!("#{id}={name}({body});")
                }
            }
        };
        lines.push(line);
        stack.extend(refs);
    }
    done.extend(local);
    for l in lines {
        out.push_str(&l);
        out.push('\n');
    }
    Some(())
}

/// A STEP file of the B-reps `items` and how many could not be translated; `None` when
/// none could.
pub(crate) fn translate(ctx: &Ctx, items: &[u32]) -> Option<(Vec<u8>, usize)> {
    let angle_degrees = (ctx.angle - std::f64::consts::PI / 180.0).abs() < 1e-9;
    let mut data = String::new();
    let mut done = HashSet::new();
    let mut ok = Vec::new();
    let mut skipped = 0;
    for &b in items {
        let mut text = String::new();
        let mut d = done.clone();
        if instances(ctx, b, &mut d, &mut text).is_some() {
            done = d;
            data.push_str(&text);
            ok.push(b);
        } else {
            skipped += 1;
        }
    }
    if ok.is_empty() {
        return None;
    }
    let mut n = ctx.s.max_id.max(1) as u64 + 1;
    let mut id = || {
        let k = n;
        n += 1;
        k
    };
    let (app, apd, pc, pdc) = (id(), id(), id(), id());
    let (len_u, ang_u, sol_u, unc, ctx_id, origin, place) =
        (id(), id(), id(), id(), id(), id(), id());
    let mut head = String::new();
    let _ = writeln!(head, "#{app}=APPLICATION_CONTEXT('automotive design');");
    let _ = writeln!(
        head,
        "#{apd}=APPLICATION_PROTOCOL_DEFINITION('international standard','automotive_design',2000,#{app});"
    );
    let _ = writeln!(head, "#{pc}=PRODUCT_CONTEXT('',#{app},'mechanical');");
    let _ = writeln!(
        head,
        "#{pdc}=PRODUCT_DEFINITION_CONTEXT('part definition',#{app},'design');"
    );
    let _ = writeln!(
        head,
        "#{len_u}=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT($,.METRE.));"
    );
    if angle_degrees {
        let (rad, dim, conv) = (id(), id(), id());
        let _ = writeln!(
            head,
            "#{rad}=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));"
        );
        let _ = writeln!(head, "#{dim}=DIMENSIONAL_EXPONENTS(0.,0.,0.,0.,0.,0.,0.);");
        let _ = writeln!(
            head,
            "#{conv}=PLANE_ANGLE_MEASURE_WITH_UNIT(PLANE_ANGLE_MEASURE(0.0174532925199433),#{rad});"
        );
        let _ = writeln!(
            head,
            "#{ang_u}=(CONVERSION_BASED_UNIT('DEGREE',#{conv})NAMED_UNIT(#{dim})PLANE_ANGLE_UNIT());"
        );
    } else {
        let _ = writeln!(
            head,
            "#{ang_u}=(NAMED_UNIT(*)PLANE_ANGLE_UNIT()SI_UNIT($,.RADIAN.));"
        );
    }
    let _ = writeln!(
        head,
        "#{sol_u}=(NAMED_UNIT(*)SI_UNIT($,.STERADIAN.)SOLID_ANGLE_UNIT());"
    );
    let _ = writeln!(
        head,
        "#{unc}=UNCERTAINTY_MEASURE_WITH_UNIT(LENGTH_MEASURE({:e}),#{len_u},'distance_accuracy_value','');",
        ctx.eps.max(1e-9)
    );
    let _ = writeln!(
        head,
        "#{ctx_id}=(GEOMETRIC_REPRESENTATION_CONTEXT(3)GLOBAL_UNCERTAINTY_ASSIGNED_CONTEXT((#{unc}))GLOBAL_UNIT_ASSIGNED_CONTEXT((#{len_u},#{ang_u},#{sol_u}))REPRESENTATION_CONTEXT('',''));"
    );
    let _ = writeln!(head, "#{origin}=CARTESIAN_POINT('',(0.,0.,0.));");
    let _ = writeln!(head, "#{place}=AXIS2_PLACEMENT_3D('',#{origin},$,$);");
    for b in &ok {
        let (p, f, d, s, r, sdr) = (id(), id(), id(), id(), id(), id());
        let _ = writeln!(head, "#{p}=PRODUCT('B{b}','B{b}','',(#{pc}));");
        let _ = writeln!(head, "#{f}=PRODUCT_DEFINITION_FORMATION('','',#{p});");
        let _ = writeln!(head, "#{d}=PRODUCT_DEFINITION('design','',#{f},#{pdc});");
        let _ = writeln!(head, "#{s}=PRODUCT_DEFINITION_SHAPE('','',#{d});");
        let _ = writeln!(
            head,
            "#{r}=ADVANCED_BREP_SHAPE_REPRESENTATION('',(#{b},#{place}),#{ctx_id});"
        );
        let _ = writeln!(head, "#{sdr}=SHAPE_DEFINITION_REPRESENTATION(#{s},#{r});");
    }
    let mut file = String::new();
    file.push_str("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('IFC advanced B-reps'),'2;1');\n");
    file.push_str("FILE_NAME('breps.stp','',(''),(''),'Specular','Specular','');\n");
    file.push_str(
        "FILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\nENDSEC;\nDATA;\n",
    );
    file.push_str(&head);
    file.push_str(&data);
    file.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    Some((file.into_bytes(), skipped))
}
