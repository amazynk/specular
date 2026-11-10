//! What an edit uses, in words the library searches (`module:`, `style:`): the modules
//! that change the picture and the styles applied.

use super::edit::{Edit, ModuleInstance};
use super::history::{Change, History};
use super::kind::Registry;

/// Whether `m` changes the picture: on, and moved off the registry's defaults (or blended
/// through a mask). Modules the panel adds at their identity are not "used".
pub fn is_used(m: &ModuleInstance, registry: &Registry) -> bool {
    if !m.enabled {
        return false;
    }
    if m.blend.is_some() {
        return true;
    }
    match registry.get(m.kind) {
        Some(spec) => m.params.to_fields() != (spec.defaults)(),
        // A kind this build does not know: kept, and counted.
        None => true,
    }
}

/// The stable names (`exposure`, `colour_balance`) of the modules `edit` uses, in stack
/// order, once each. Unknown kinds are left out (they have no name to search by).
pub fn used_modules(edit: &Edit, registry: &Registry) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for m in &edit.modules {
        if let Some(spec) = registry.get(m.kind)
            && is_used(m, registry)
            && !out.contains(&spec.name)
        {
            out.push(spec.name);
        }
    }
    out
}

/// The styles applied to the edit as its history records them (whole-stack steps named as
/// one of `styles`), oldest first, once each. Steps undone (past the history's position)
/// are left out.
pub fn styles_applied(history: &History, styles: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in history.steps.iter().take(history.position) {
        if let Change::Stack { what, .. } = &s.change
            && styles.contains(&what.as_str())
            && !out.contains(what)
        {
            out.push(what.clone());
        }
    }
    out
}

/// [`used_modules`] and [`styles_applied`] as the library keeps them: one per line.
pub fn summary_lines(
    edit: &Edit,
    history: &History,
    registry: &Registry,
    styles: &[&str],
) -> (String, String) {
    (
        used_modules(edit, registry).join("\n"),
        styles_applied(history, styles).join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        ApplyMode, Edit, History, ModuleInstance, ModuleKind, SourceKey, Style, Value,
    };

    #[test]
    fn modules_at_their_defaults_are_not_used() {
        let reg = Registry::builtin();
        let mut e = Edit::default_for(reg, SourceKey::default(), true);
        assert!(used_modules(&e, reg).is_empty());
        let exp = e.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
        let mut f = exp.params.to_fields();
        f.set("exposure_ev", Value::Float(0.7));
        exp.params = crate::model::Params::from_fields(ModuleKind::EXPOSURE, f);
        assert_eq!(used_modules(&e, reg), ["exposure"]);
        // Off: not used.
        e.module_mut(ModuleKind::EXPOSURE, 0).unwrap().enabled = false;
        assert!(used_modules(&e, reg).is_empty());
        // A second instance counts once.
        let mut d = ModuleInstance::new(reg, ModuleKind::DENOISE, 0);
        d.blend = Some(Default::default());
        e.insert(reg, d.clone());
        d.instance = 1;
        e.insert(reg, d);
        assert_eq!(used_modules(&e, reg), ["denoise"]);
    }

    #[test]
    fn styles_come_from_the_history() {
        let reg = Registry::builtin();
        let mut e = Edit::default_for(reg, SourceKey::default(), true);
        let mut h = History::default();
        let mut src = e.clone();
        let exp = src.module_mut(ModuleKind::EXPOSURE, 0).unwrap();
        let mut f = exp.params.to_fields();
        f.set("exposure_ev", Value::Float(1.0));
        exp.params = crate::model::Params::from_fields(ModuleKind::EXPOSURE, f);
        let style = Style::from_edit("Bright", &src, |m| m.kind == ModuleKind::EXPOSURE);
        assert!(style.paste(&mut e, &mut h, reg, ApplyMode::Replace, "Bright", 1));
        assert!(!style.paste(&mut e, &mut h, reg, ApplyMode::Replace, "Bright", 2));
        assert_eq!(styles_applied(&h, &["Bright", "Mono"]), ["Bright"]);
        // A paste is not a style.
        assert!(styles_applied(&h, &["Mono"]).is_empty());
        // Undone: gone.
        h.position = 0;
        assert!(styles_applied(&h, &["Bright"]).is_empty());
        let (m, s) = summary_lines(&e, &h, reg, &["Bright"]);
        assert_eq!(m, "exposure");
        assert_eq!(s, "");
    }
}
