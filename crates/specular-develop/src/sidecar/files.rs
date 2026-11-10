//! Develop's own files in the user's config folder: styles, module presets and
//! auto-apply rules.
//!
//! - **Style:** one small XMP file each, the sidecar form of its modules with `spd:name`
//!   (also what Import and Export read and write).
//! - **Presets and rules:** plain text, one `[preset]` or `[rule]` block each, `key = value`
//!   lines; field values in the sidecar's text form. Unknown keys and broken blocks are
//!   skipped, so a hand-edited file never stops the rest from loading.
//!
//! ```text
//! [preset]
//! module = exposure
//! name = Bright
//! version = 1
//! exposure_ev = 1.0
//!
//! [rule]
//! style = Night
//! maker = Canon
//! iso_min = 3200
//! type = raw
//! ```

use crate::model::migrate::MigrationReport;
use crate::model::{
    AutoRule, Blend, Edit, FileType, MaskPreset, ModuleInstance, ModuleKind, Preset, Registry,
    SourceKey, Style,
};

use super::{Error, from_xmp, module_kind, module_name, parse_value, to_xmp_named, value_text};

/// A style as a file.
pub fn style_to_xmp(style: &Style) -> String {
    let mut e = Edit::empty(SourceKey::default());
    e.modules.clone_from(&style.items);
    e.masks.clone_from(&style.masks);
    to_xmp_named(&e, None, Some(&style.name))
}

/// A style file read back; `fallback` names it when the file has no name.
pub fn style_from_xmp(
    text: &str,
    reg: &Registry,
    fallback: &str,
) -> Result<(Style, MigrationReport), Error> {
    let l = from_xmp(text, reg)?;
    let name = l
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    Ok((
        Style {
            name,
            items: l.edit.modules,
            masks: l.edit.masks,
        },
        l.report,
    ))
}

/// A file name for a style's name: letters, digits, spaces, `-` and `_` kept, the rest `_`.
pub fn style_file_name(name: &str) -> String {
    let mut s: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    if s.is_empty() {
        s.push_str("style");
    }
    s.push_str(".xmp");
    s
}

/// The `[kind]` blocks of `text` as key/value lists.
fn blocks<'a>(text: &'a str, kind: &str) -> Vec<Vec<(&'a str, &'a str)>> {
    let mut out: Vec<Vec<(&str, &str)>> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('#') {
            continue;
        }
        if let Some(h) = l.strip_prefix('[').and_then(|h| h.strip_suffix(']')) {
            inside = h.trim() == kind;
            if inside {
                out.push(Vec::new());
            }
            continue;
        }
        if inside
            && let Some((k, v)) = l.split_once('=')
            && let Some(b) = out.last_mut()
        {
            b.push((k.trim(), v.trim()));
        }
    }
    out
}

fn line(o: &mut String, k: &str, v: &str) {
    // One line per value: line breaks would start a new key.
    let v = v.replace(['\n', '\r'], " ");
    o.push_str(&format!("{k} = {v}\n"));
}

/// Presets as text.
pub fn presets_to_text(presets: &[Preset]) -> String {
    let reg = Registry::builtin();
    let mut o = String::from("# Specular Develop presets\n");
    for p in presets {
        o.push_str("\n[preset]\n");
        line(&mut o, "module", &module_name(reg, p.kind));
        line(&mut o, "name", &p.name);
        line(&mut o, "version", &p.version.to_string());
        for (n, v) in &p.fields.0 {
            line(&mut o, n, &value_text(v));
        }
    }
    o
}

/// Presets read back; a block without a known module or a name is skipped. Fields migrate
/// to the module's current version.
pub fn presets_from_text(text: &str, reg: &Registry) -> Vec<Preset> {
    let mut out = Vec::new();
    for b in blocks(text, "preset") {
        let get = |k: &str| b.iter().find(|(n, _)| *n == k).map(|(_, v)| *v);
        let (Some(kind), Some(name)) =
            (get("module").and_then(|m| module_kind(reg, m)), get("name"))
        else {
            continue;
        };
        if name.is_empty() {
            continue;
        }
        let mut version: u16 = get("version").and_then(|v| v.parse().ok()).unwrap_or(1);
        let defaults = reg.get(kind).map(|s| (s.defaults)()).unwrap_or_default();
        let mut fields = crate::model::Fields::new();
        for (k, v) in &b {
            if matches!(*k, "module" | "name" | "version") {
                continue;
            }
            fields.set(k, parse_value(v, defaults.get(k)));
        }
        if let Some(spec) = reg.get(kind)
            && crate::model::migrate::run(spec, version, spec.current, &mut fields).is_ok()
        {
            version = spec.current;
        }
        out.push(Preset {
            kind,
            name: name.to_owned(),
            version,
            fields,
        });
    }
    out
}

/// Rules as text.
pub fn rules_to_text(rules: &[AutoRule]) -> String {
    let mut o = String::from("# Specular Develop auto-apply rules: the first that matches wins\n");
    for r in rules {
        o.push_str("\n[rule]\n");
        line(&mut o, "style", &r.style);
        for (k, v) in [("maker", &r.maker), ("model", &r.model), ("lens", &r.lens)] {
            if !v.is_empty() {
                line(&mut o, k, v);
            }
        }
        if let Some(v) = r.iso_min {
            line(&mut o, "iso_min", &v.to_string());
        }
        if let Some(v) = r.iso_max {
            line(&mut o, "iso_max", &v.to_string());
        }
        let t = match r.file_type {
            FileType::Any => "any",
            FileType::Raw => "raw",
            FileType::Other => "other",
        };
        line(&mut o, "type", t);
    }
    o
}

/// Rules read back; a block without a style is skipped.
pub fn rules_from_text(text: &str) -> Vec<AutoRule> {
    let mut out = Vec::new();
    for b in blocks(text, "rule") {
        let get = |k: &str| {
            b.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| (*v).to_owned())
                .unwrap_or_default()
        };
        let style = get("style");
        if style.is_empty() {
            continue;
        }
        out.push(AutoRule {
            style,
            maker: get("maker"),
            model: get("model"),
            lens: get("lens"),
            iso_min: get("iso_min").parse().ok(),
            iso_max: get("iso_max").parse().ok(),
            file_type: match get("type").as_str() {
                "raw" => FileType::Raw,
                "other" => FileType::Other,
                _ => FileType::Any,
            },
        });
    }
    out
}

/// A mask preset as a file: the sidecar form of one switched-off Exposure item that
/// carries the mask as its blend, the shapes upright, and `spd:name`.
pub fn mask_preset_to_xmp(p: &MaskPreset) -> String {
    let reg = Registry::builtin();
    let mut m = ModuleInstance::new(reg, ModuleKind::EXPOSURE, 0);
    m.enabled = false;
    m.blend = Some(Blend {
        mask: p.mask.clone(),
        invert: p.invert,
        refine: p.refine.clone(),
        ..Blend::default()
    });
    let mut e = Edit::empty(SourceKey::default());
    e.modules.push(m);
    e.masks.clone_from(&p.masks);
    to_xmp_named(&e, None, Some(&p.name))
}

/// A mask preset file read back; `fallback` names it when the file has no name. A file
/// with no mask in it is an error.
pub fn mask_preset_from_xmp(
    text: &str,
    reg: &Registry,
    fallback: &str,
) -> Result<MaskPreset, Error> {
    let l = from_xmp(text, reg)?;
    let name = l
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| fallback.to_owned());
    let b = l
        .edit
        .modules
        .iter()
        .find_map(|m| m.blend.clone())
        .ok_or_else(|| super::bad("a mask preset without a mask"))?;
    Ok(MaskPreset {
        name,
        mask: b.mask,
        invert: b.invert,
        refine: b.refine,
        masks: l.edit.masks,
    })
}
