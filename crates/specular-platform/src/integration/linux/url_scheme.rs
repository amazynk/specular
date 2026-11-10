//! `specular://` links (DESIGN.md §11, "URL scheme"): `x-scheme-handler/specular` in our
//! `.desktop` file's `MimeType=` and Specular as its default in `$XDG_CONFIG_HOME/mimeapps.list`
//! (through `xdg-mime default` when installed, else edited directly). Opt-in like every
//! integration option; the value replaced is recorded and put back on removal unless the user
//! picked another handler since. What a link may do is decided by
//! [`crate::url_scheme`]: local files only, unusual ones after a confirmation.

use specular_log::{SETTINGS, info, warn};

use super::default_viewer::{
    DEFAULTS, desktop_id, effective_default, first_is_ours, mimeapps_path, ours_first, read_keyfile,
};
use super::{
    Record, backup, run_tool, sync_desktop_file, sync_desktop_file_after_removal, write_atomic,
};
use crate::integration::{Env, Labels, OptionId, Result, State};

/// The MIME type desktops use for URL scheme handlers.
pub(crate) const SCHEME_TYPE: &str = "x-scheme-handler/specular";

fn record_name() -> &'static str {
    OptionId::UrlScheme.key()
}

pub(super) fn apply(env: &Env, labels: &Labels) -> Result<()> {
    let path = mimeapps_path(env);
    let (kf, mode) = read_keyfile(&path)?;
    let mut rec = Record::load(env, record_name()).unwrap_or_default();
    if !rec
        .rows
        .iter()
        .any(|r| r.first().is_some_and(|k| k == "prev"))
    {
        let prev = kf
            .get(DEFAULTS, SCHEME_TYPE)
            .filter(|v| !first_is_ours(Some(v)) && !v.contains(['\t', '\n']))
            .unwrap_or("-");
        rec.rows
            .push(vec!["prev".into(), SCHEME_TYPE.into(), prev.into()]);
    }
    // Recorded before anything changes, so an interrupted apply can still be undone.
    rec.save(env, record_name())?;
    sync_desktop_file(env, labels)?;
    if first_is_ours(kf.get(DEFAULTS, SCHEME_TYPE)) {
        return Ok(());
    }
    backup(env, &path)?;
    let id = desktop_id();
    match run_tool(
        env,
        "xdg-mime",
        &["default".as_ref(), id.as_ref(), SCHEME_TYPE.as_ref()],
    ) {
        Ok(true) => info!(target: SETTINGS, "integration: xdg-mime default {id} {SCHEME_TYPE}"),
        Ok(false) => {}
        Err(e) => warn!(target: SETTINGS, "integration: {e}; editing {} directly", path.display()),
    }
    let (mut kf, _) = read_keyfile(&path)?;
    let cur = kf.get(DEFAULTS, SCHEME_TYPE).map(str::to_owned);
    if !first_is_ours(cur.as_deref()) {
        kf.set(DEFAULTS, SCHEME_TYPE, &ours_first(cur.as_deref()));
        write_atomic(&path, kf.to_text().as_bytes(), mode)?;
    }
    Ok(())
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let rec = Record::load(env, record_name()).unwrap_or_default();
    let prev = rec
        .rows
        .iter()
        .find(|r| r.len() == 3 && r[0] == "prev" && r[1] == SCHEME_TYPE)
        .and_then(|r| (r[2] != "-").then(|| r[2].clone()));
    let path = mimeapps_path(env);
    let (mut kf, mode) = read_keyfile(&path)?;
    // Not ours (any more): the user's choice stays.
    if first_is_ours(kf.get(DEFAULTS, SCHEME_TYPE)) {
        match prev {
            Some(p) => kf.set(DEFAULTS, SCHEME_TYPE, &p),
            None => {
                kf.remove(DEFAULTS, SCHEME_TYPE);
            }
        }
        backup(env, &path)?;
        write_atomic(&path, kf.to_text().as_bytes(), mode)?;
        info!(target: SETTINGS, "integration: restored the {SCHEME_TYPE} handler in {}", path.display());
    }
    Record::delete(env, record_name());
    sync_desktop_file_after_removal(env)?;
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let applied = Record::load(env, record_name()).is_some();
    let ours = effective_default(env, SCHEME_TYPE).as_deref() == Some(desktop_id().as_str());
    let state = match (ours, applied) {
        (true, true) => State::On,
        (true, false) | (false, true) => State::Partial,
        (false, false) => State::Off,
    };
    (state, Vec::new())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::{local_desktop_file, test_env};
    use super::*;

    #[test]
    fn apply_and_remove() {
        let env = test_env::env("url-scheme");
        fs::create_dir_all(&env.config_home).unwrap();
        let list = "[Default Applications]\nx-scheme-handler/specular=other.desktop;\n";
        fs::write(mimeapps_path(&env), list).unwrap();
        assert_eq!(status(&env).0, State::Off);

        apply(&env, &Labels::default()).unwrap();
        let entry = fs::read_to_string(local_desktop_file(&env)).unwrap();
        let mimes = entry
            .lines()
            .find_map(|l| l.strip_prefix("MimeType="))
            .unwrap();
        assert!(mimes.contains("image/jpeg;") && mimes.contains("x-scheme-handler/specular;"));
        assert!(entry.contains("DBusActivatable=false"));
        let text = fs::read_to_string(mimeapps_path(&env)).unwrap();
        assert!(
            text.contains("x-scheme-handler/specular=app.specular.Specular.desktop;other.desktop;")
        );
        assert_eq!(status(&env).0, State::On);
        // Idempotent.
        apply(&env, &Labels::default()).unwrap();
        assert_eq!(fs::read_to_string(mimeapps_path(&env)).unwrap(), text);

        remove(&env).unwrap();
        assert_eq!(fs::read_to_string(mimeapps_path(&env)).unwrap(), list);
        assert!(!local_desktop_file(&env).exists());
        assert_eq!(status(&env).0, State::Off);
        test_env::cleanup(&env);
    }
}
