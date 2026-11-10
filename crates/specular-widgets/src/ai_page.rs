//! Settings → Local AI (DESIGN.md "Local AI pack"): the pack's page from what the
//! application knows about it ([`AiPage`]).
//!
//! - "Local AI pack" (`ai.pack`): a state line, and under it one row per feature this build
//!   offers (what it does, download size and model licence) with Download, Cancel or
//!   Remove; "Remove pack" with a confirm step once anything is installed. With nothing
//!   offered (the built-in manifest is empty until the models are chosen) the page says "No
//!   models available in this build".
//! - Per feature this build offers (or has installed): its switch (`ai.search` …
//!   `ai.people`; switching on installs it) and, when it comes in more than one size, its
//!   "Model size" (`ai.search_size` …): Recommended (labelled with the size it means here,
//!   "Default (Standard)") or one of the sizes the feature comes in by name, the
//!   recommendation's line ("Default for your <GPU>: Standard"), and under it a row per
//!   size saying what is on disk (in use, the one in use until a switch has indexed,
//!   downloaded but not in use, downloading, failed, or the chosen size not downloaded;
//!   never inferred from the setting), its download and memory, and why it may not run here
//!   ("Needs 6 GB of memory, more than the 4 GB Specular allows for AI on this computer": shown, never hidden). Every
//!   size not in use has a button (Switch when downloaded, else "Use this size") that sets
//!   the size setting, and a named size gets "Back to default"; then a row per licence
//!   (a button opens its page). Only a size that needs the GPU mentions it; where a model
//!   runs is the feature row's "Runs on" line. A size change that downloads and re-indexes,
//!   choosing a size that may not run here, or a recommendation that moved (only while the
//!   setting is Recommended) asks first there: Switch (or "Use anyway"), or keep the size.
//! - Each size names its model ("SigLIP 2 B/16") and says what it reads (languages), a hint
//!   ("Faster, for slower graphics cards"), its training data and whether the publisher
//!   states the data's terms, and the host it downloads from: the model manager (the size
//!   setting is the model choice). Models still
//!   coming are listed with the same facts and no button; a feature with only coming
//!   models shows "Coming in a later version" and no switch.
//! - Under People: grouping faces into people, off until turned on for this library; the
//!   people found, each with Forget; and "Delete all face data" (asked first).
//!
//! The rows come back as [`Command::SettingPanel`](crate::Command::SettingPanel) with the
//! ids below; the application acts on them.

use specular_i18n::{I18n, t};

use crate::settings::SettingStatus;
use crate::{Chrome, PanelAction, PanelRow, SettingPanel};

/// The pack's setting (its state line and the features' panel).
pub const PACK: &str = "ai.pack";
/// People's switch (the people panel sits under it).
pub const PEOPLE: &str = "ai.people";
/// Install from file… and from folder… (sideloading model files).
pub const SIDELOAD: [&str; 2] = ["ai.install_from_file", "ai.install_from_folder"];
/// The features, in page order, by key (`ai.<key>`, `settings-ai-<key>`).
pub const FEATURES: [&str; 9] = [
    "search", "text", "culling", "people", "speech", "masks", "denoise", "remove", "depth",
];
/// The model size settings, one per feature in [`FEATURES`] order (shown only for a feature
/// that comes in more than one size).
pub const SIZES: [&str; 9] = [
    "ai.search_size",
    "ai.text_size",
    "ai.culling_size",
    "ai.people_size",
    "ai.speech_size",
    "ai.masks_size",
    "ai.denoise_size",
    "ai.remove_size",
    "ai.depth_size",
];

/// The model size setting of `feature` (one of [`FEATURES`]), if it has one.
pub fn size_key(feature: &str) -> Option<&'static str> {
    SIZES
        .into_iter()
        .find(|k| k.strip_prefix("ai.").and_then(|k| k.strip_suffix("_size")) == Some(feature))
}

/// One feature's row.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiFeatureRow {
    /// One of [`FEATURES`].
    pub feature: &'static str,
    pub state: AiRowState,
    /// Download size in bytes (of the size it would install).
    pub size: u64,
    /// The models' licences ("Apache-2.0, MIT").
    pub licences: String,
    /// The models, for the detail line ("clip-vit-b32 1").
    pub models: String,
    /// The sizes offered, smallest first (fewer than two: no size choice).
    pub sizes: Vec<AiSize>,
    /// Each licence of the feature's models, with its page.
    pub licence_links: Vec<AiLicence>,
    /// The size installed (`"standard"`), the one downloading or that failed to, and the one
    /// still in use until the new one has indexed.
    pub tier: Option<&'static str>,
    pub target: Option<&'static str>,
    pub previous: Option<&'static str>,
    /// What this machine suits (None: nothing fits).
    pub recommended: Option<&'static str>,
    /// The size setting as it is: `"recommended"` or a size by name (`"large"`).
    pub choice: &'static str,
    /// The size the size setting means here (it may not be installed).
    pub chosen: Option<&'static str>,
    /// A switch waiting for the user's yes.
    pub question: Option<AiQuestion>,
    /// What its model runs on now (installed and loaded; None: not known, nothing shown).
    pub runs_on: Option<AiEngine>,
    /// Models listed but not installable yet.
    pub coming: Vec<AiComing>,
    /// Add-ons installed beside the feature's size (Text's script recognisers).
    pub addons: Vec<AiAddon>,
}

/// An add-on: a model installed beside a feature's size (a script's text recogniser),
/// downloaded and removed on its own.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiAddon {
    /// The manifest's id (`arabic`).
    pub id: String,
    pub info: AiModelInfo,
    /// Bytes of its files.
    pub size: u64,
    /// SPDX name.
    pub licence: String,
    /// `Available`, `Downloading`, `Failed`, `NetworkOff`, `Installed` or `Removing`.
    pub state: AiRowState,
}

/// What the model manager says about a model besides its size: its name and the facts the
/// user picks by.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiModelInfo {
    /// The model's own name ("SigLIP 2 B/16"); empty: the size's name stands for it.
    pub name: String,
    /// An `ai-choice-languages` selector (`multilingual`, `english`, a script); empty: none.
    pub languages: String,
    /// An `ai-choice-hint` selector (`faster`, …); empty: none.
    pub hint: String,
    /// The training data as the publisher names it; empty: not named.
    pub data: String,
    /// `unstated`, `stated` or `research`.
    pub data_terms: &'static str,
    /// The host it downloads from ("huggingface.co"); empty: not known.
    pub host: String,
}

/// A model listed but not installable yet (a later version runs it).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiComing {
    pub info: AiModelInfo,
    /// Bytes it will download (approximate).
    pub size: u64,
    /// SPDX name and page.
    pub licence: String,
    pub licence_url: String,
}

/// What a feature's model runs on, as the app's runtime and path selector say.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AiEngine {
    /// The graphics card (its name when known).
    Gpu(Option<String>),
    /// The processor, and why (an `ai-runs-on` selector: `"no-gpu"`, `"differed"`,
    /// `"failed"`, `"setting"`, `"checking"`, or `"cpu"` when the measurements chose it).
    Cpu(&'static str),
}

/// One size of a feature.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiSize {
    pub tier: &'static str,
    /// Bytes to download for it (its files not installed yet; 0 when all are).
    pub download: u64,
    /// Bytes of all its files.
    pub size: u64,
    /// All its files are installed (as `installed.toml` lists them, and present).
    pub on_disk: bool,
    /// Memory it needs, MB.
    pub memory_mb: u32,
    pub needs_gpu: bool,
    /// Why it may not run on this machine (None: it fits). Shown, and asked about before
    /// it is used, never hidden.
    pub unfit: Option<AiUnfit>,
    pub info: AiModelInfo,
}

/// Why a size may not run on this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiUnfit {
    /// It needs a GPU the engine cannot use here.
    Gpu,
    /// It needs more memory (MB) than this machine has for it.
    Memory { need_mb: u64, have_mb: u64 },
}

/// A licence of a feature's models.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiLicence {
    /// SPDX name ("MIT").
    pub name: String,
    /// Its page (opened by the row's button).
    pub url: String,
    /// The models under it ("clip-vit-b-32 1").
    pub models: String,
}

/// A size switch asked before it starts (it downloads and re-indexes), or a size that may
/// not run here, chosen while the feature is not installed (asked before it is kept).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiQuestion {
    pub tier: &'static str,
    /// Bytes it downloads (0: its files are here already).
    pub download: u64,
    /// The recommendation moved (rather than the user changing the size).
    pub suggested: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum AiRowState {
    #[default]
    Available,
    Downloading {
        done: u64,
        total: u64,
    },
    /// The last download failed (the reason, as the error said it).
    Failed(String),
    /// Not installed, and the network is off (Settings › Privacy): Download installs only
    /// from model files already on this computer.
    NetworkOff,
    /// Installed; images indexed and readable images, when known.
    Installed {
        indexed: Option<(u64, u64)>,
    },
    /// Being removed (in the background): no button until it ends.
    Removing,
    /// Its models are listed but none can be installed yet.
    Coming,
}

/// Someone People found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiPerson {
    pub id: i64,
    /// Empty: not named.
    pub name: String,
    pub faces: u64,
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AiPage {
    /// The features offered or installed (others are left out).
    pub features: Vec<AiFeatureRow>,
    pub people: Vec<AiPerson>,
    /// The graphics card's name, for the recommendation's line (None: not known).
    pub gpu: Option<String>,
    /// Whether faces are grouped into people in this library (the user turns it on per
    /// library); None: not known (People not installed, no library).
    pub face_grouping: Option<bool>,
    /// Whether the text in this library's photos is read for search (the user turns it on
    /// per library); None: not known (Text not installed, no library).
    pub text_indexing: Option<bool>,
}

/// What a row's button asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiRowAction<'a> {
    Download(&'a str),
    Cancel(&'a str),
    Remove(&'a str),
    Forget(i64),
    /// Yes to the feature's question: switch to that size.
    Switch(&'a str, &'a str),
    /// No to it: keep the size in use.
    Keep(&'a str),
    /// Set the feature's size setting: `"recommended"` or a size by name. Changing the size
    /// of an installed feature then asks first, as the setting itself does.
    Use(&'a str, &'a str),
    /// Open a licence's page.
    Licence(&'a str),
    /// Turn grouping faces into people on or off for this library.
    Grouping(bool),
    /// Download an add-on (its id).
    AddonGet(&'a str),
    /// Stop an add-on's download.
    AddonCancel(&'a str),
    /// Remove an add-on (the app asks first).
    AddonRemove(&'a str),
    /// Turn reading the text in this library's photos on or off.
    TextIndexing(bool),
}

const TIERS: [&str; 3] = ["small", "standard", "large"];
/// The size setting's values: Recommended, then the sizes by name.
const CHOICES: [&str; 4] = ["recommended", "small", "standard", "large"];

impl AiRowAction<'_> {
    /// Reads a [`PanelRow::id`] of the Local AI page.
    pub fn parse(id: &str) -> Option<AiRowAction<'_>> {
        let (verb, arg) = id.split_once(':')?;
        let feature = |a: &str| FEATURES.into_iter().find(|f| *f == a);
        Some(match verb {
            "download" => AiRowAction::Download(feature(arg)?),
            "cancel" => AiRowAction::Cancel(feature(arg)?),
            "remove" => AiRowAction::Remove(feature(arg)?),
            "forget" => AiRowAction::Forget(arg.parse().ok()?),
            "switch" => {
                let (f, t) = arg.split_once(':')?;
                AiRowAction::Switch(feature(f)?, TIERS.into_iter().find(|x| *x == t)?)
            }
            "keep" => AiRowAction::Keep(feature(arg)?),
            "use" => {
                let (f, c) = arg.split_once(':')?;
                AiRowAction::Use(feature(f)?, CHOICES.into_iter().find(|x| *x == c)?)
            }
            "licence" if arg.starts_with("https://") => AiRowAction::Licence(arg),
            "grouping" => AiRowAction::Grouping(match arg {
                "on" => true,
                "off" => false,
                _ => return None,
            }),
            "addon-get" if is_id(arg) => AiRowAction::AddonGet(arg),
            "addon-cancel" if is_id(arg) => AiRowAction::AddonCancel(arg),
            "addon-remove" if is_id(arg) => AiRowAction::AddonRemove(arg),
            "textindex" => AiRowAction::TextIndexing(match arg {
                "on" => true,
                "off" => false,
                _ => return None,
            }),
            _ => return None,
        })
    }
}

fn is_id(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// An add-on's row: its name and state, what it reads, its size and licence, and Download,
/// Cancel or Remove (Download only once the feature itself is installed).
fn addon_row(f: &AiFeatureRow, a: &AiAddon) -> PanelRow {
    let loc = I18n::global().localizer();
    let installed = matches!(f.state, AiRowState::Installed { .. });
    let (value, button, verb) = match &a.state {
        AiRowState::Installed { .. } => (
            t!("ai-state", state = "installed"),
            Some(t!("ai-button", action = "remove")),
            "addon-remove",
        ),
        AiRowState::Downloading { done, total } => (
            t!(
                "ai-state",
                state = "downloading",
                percent = loc.percent(*done as f64 / (*total).max(1) as f64, 0)
            ),
            Some(t!("ai-button", action = "cancel")),
            "addon-cancel",
        ),
        AiRowState::Removing => (t!("ai-removing"), None, "addon"),
        AiRowState::Failed(_) => (
            t!("ai-state", state = "failed"),
            installed.then(|| t!("ai-button", action = "download")),
            "addon-get",
        ),
        _ => (
            t!("ai-state", state = "available"),
            installed.then(|| t!("ai-button", action = "download")),
            "addon-get",
        ),
    };
    let mut lines = Vec::new();
    if let AiRowState::Failed(why) = &a.state {
        lines.push(why.clone());
    }
    if a.state == AiRowState::NetworkOff {
        lines.push(t!("network-off"));
    }
    if !installed && !matches!(a.state, AiRowState::Installed { .. }) {
        let feature = I18n::global().tr(&format!("settings-ai-{}", f.feature));
        lines.push(t!("ai-addon-first", feature = feature));
    }
    lines.extend(info_lines(&a.info));
    PanelRow {
        id: format!("{verb}:{}", a.id),
        label: a.info.name.clone(),
        value,
        lines,
        detail: specular_i18n::dot_join(&[
            loc.bytes(a.size, specular_i18n::ByteUnits::Decimal),
            a.licence.clone(),
        ]),
        attention: matches!(a.state, AiRowState::Failed(_)),
        button,
        ..Default::default()
    }
}

/// Text's per-library switch: whether the text in this library's photos is read for search.
fn text_indexing_row(on: bool) -> PanelRow {
    PanelRow {
        id: format!("textindex:{}", if on { "off" } else { "on" }),
        label: t!("ai-text-indexing"),
        value: t!("ai-text-indexing-state", on = if on { "yes" } else { "no" }),
        lines: vec![t!("ai-text-indexing-help")],
        button: Some(t!("ai-grouping-button", on = if on { "yes" } else { "no" })),
        ..Default::default()
    }
}

fn setting_key(feature: &str) -> &'static str {
    match feature {
        "search" => "ai.search",
        "text" => "ai.text",
        "culling" => "ai.culling",
        "speech" => "ai.speech",
        "masks" => "ai.masks",
        "denoise" => "ai.denoise",
        "remove" => "ai.remove",
        "depth" => "ai.depth",
        _ => PEOPLE,
    }
}

fn feature_row(f: &AiFeatureRow) -> PanelRow {
    let i18n = I18n::global();
    let loc = i18n.localizer();
    let size = loc.bytes(f.size, specular_i18n::ByteUnits::Decimal);
    let (value, button, attention, id) = match &f.state {
        AiRowState::Removing => (t!("ai-removing"), None, false, "removing"),
        AiRowState::Coming => (t!("ai-coming"), None, false, "coming"),
        AiRowState::Available => (
            t!("ai-state", state = "available"),
            Some(t!("ai-button", action = "download")),
            false,
            "download",
        ),
        AiRowState::Downloading { done, total } => (
            t!(
                "ai-state",
                state = "downloading",
                percent = loc.percent(*done as f64 / (*total).max(1) as f64, 0)
            ),
            Some(t!("ai-button", action = "cancel")),
            false,
            "cancel",
        ),
        AiRowState::NetworkOff => (
            t!("ai-state", state = "available"),
            Some(t!("ai-button", action = "download")),
            false,
            "download",
        ),
        AiRowState::Failed(_) => (
            t!("ai-state", state = "failed"),
            Some(t!("ai-button", action = "download")),
            true,
            "download",
        ),
        AiRowState::Installed { indexed } => (
            match indexed {
                Some((done, total)) if f.previous.is_some() => t!(
                    "ai-switching",
                    size = size_label(f, f.tier.unwrap_or("standard")),
                    done = loc.integer(*done as i64),
                    total = loc.integer(*total as i64)
                ),
                Some((done, total)) if done < total => t!(
                    "ai-state",
                    state = "indexing",
                    done = loc.integer(*done as i64),
                    total = loc.integer(*total as i64)
                ),
                _ => t!("ai-state", state = "installed"),
            },
            Some(t!("ai-button", action = "remove")),
            false,
            "remove",
        ),
    };
    let mut lines = Vec::new();
    if let AiRowState::Failed(why) = &f.state {
        lines.push(why.clone());
    }
    if f.state == AiRowState::NetworkOff {
        lines.push(t!("network-off"));
    }
    // Where it runs is the page's one answer to "does it use my GPU": right under the
    // feature's name, before what the feature does.
    if matches!(f.state, AiRowState::Installed { .. })
        && let Some(e) = &f.runs_on
    {
        lines.push(runs_on(e));
    }
    lines.push(i18n.tr(&format!("settings-ai-{}-help", f.feature)));
    if f.state == AiRowState::Coming {
        lines.extend(f.coming.iter().map(coming_line));
    }
    PanelRow {
        id: format!("{id}:{}", f.feature),
        label: i18n.tr(&format!("settings-ai-{}", f.feature)),
        value,
        lines,
        // Size and licence are what the user decides by: always shown (a coming feature's
        // are on its models' lines).
        detail: if f.state == AiRowState::Coming {
            String::new()
        } else {
            specular_i18n::dot_join(&[size.as_str(), &f.licences, &f.models])
        },
        attention,
        button,
        ..Default::default()
    }
}

fn runs_on(e: &AiEngine) -> String {
    match e {
        AiEngine::Gpu(Some(name)) => t!("ai-runs-on", engine = "gpu", gpu = name.as_str()),
        AiEngine::Gpu(None) => t!("ai-runs-on", engine = "gpu-unnamed", gpu = ""),
        AiEngine::Cpu(why) => t!("ai-runs-on", engine = *why, gpu = ""),
    }
}

fn tier_name(tier: &str) -> String {
    t!("ai-tier", tier = tier)
}

/// A size as the page names it: its model's name, else the size's ("Standard").
fn size_label(f: &AiFeatureRow, tier: &str) -> String {
    f.sizes
        .iter()
        .find(|z| z.tier == tier && !z.info.name.is_empty())
        .map_or_else(|| tier_name(tier), |z| z.info.name.clone())
}

/// What a model reads and a hint, its training data and its host: one line each.
fn info_lines(i: &AiModelInfo) -> Vec<String> {
    let mut out = Vec::new();
    let mut what = Vec::new();
    if !i.languages.is_empty() {
        what.push(t!("ai-choice-languages", languages = i.languages.as_str()));
    }
    if !i.hint.is_empty() {
        what.push(t!("ai-choice-hint", hint = i.hint.as_str()));
    }
    if !what.is_empty() {
        out.push(specular_i18n::dot_join(&what));
    }
    if !i.data_terms.is_empty() {
        let named = if i.data.is_empty() { "no" } else { "yes" };
        out.push(t!(
            "ai-choice-data",
            terms = i.data_terms,
            named = named,
            source = i.data.as_str()
        ));
    }
    if !i.host.is_empty() {
        out.push(t!("ai-choice-host", host = i.host.as_str()));
    }
    out
}

/// A model still coming, on one line: name, size, licence, then its facts.
fn coming_line(c: &AiComing) -> String {
    let loc = I18n::global().localizer();
    let mut parts = vec![t!(
        "ai-choice-coming",
        name = c.info.name.as_str(),
        size = loc.bytes(c.size, specular_i18n::ByteUnits::Decimal),
        licence = c.licence.as_str()
    )];
    parts.extend(info_lines(&c.info));
    specular_i18n::dot_join(&parts)
}

/// Recommended as the dropdown and the "Back to recommended" row name it: with the size it
/// means here ("Recommended (Standard)"), or plainly when nothing fits.
fn recommended_label(f: &AiFeatureRow) -> String {
    match f.recommended {
        Some(r) => t!("ai-size-recommended-option", size = size_label(f, r)),
        None => I18n::global().tr(&format!("settings-ai-{}-size-recommended", f.feature)),
    }
}

/// The size setting's choices as the dropdown labels them: "Recommended (Standard)", then
/// every size the feature comes in by name, those that may not run here included.
fn choice_labels(f: &AiFeatureRow) -> Vec<(String, String)> {
    let mut out = vec![("recommended".to_owned(), recommended_label(f))];
    out.extend(
        f.sizes
            .iter()
            .map(|z| (z.tier.to_owned(), size_label(f, z.tier))),
    );
    out
}

/// The recommendation's line over the size setting.
fn recommendation(f: &AiFeatureRow, gpu: Option<&str>) -> String {
    match (f.recommended, gpu) {
        (None, _) => t!("ai-recommended-none"),
        (Some(r), Some(g)) if !g.is_empty() => {
            t!("ai-recommended-for", gpu = g, size = size_label(f, r))
        }
        (Some(r), _) => t!("ai-recommended", size = size_label(f, r)),
    }
}

/// The line over the size setting: the recommendation, or, while the setting names a size
/// other than the one in use (and nothing asks about it), which one runs.
fn size_status(f: &AiFeatureRow, gpu: Option<&str>) -> SettingStatus {
    let settled = f.target.is_none() && f.previous.is_none() && f.question.is_none();
    match (f.tier, f.chosen) {
        (Some(cur), Some(chosen)) if settled && cur != chosen && f.choice != "recommended" => {
            SettingStatus {
                state: t!(
                    "ai-size-not-in-use",
                    current = size_label(f, cur),
                    chosen = size_label(f, chosen)
                ),
                attention: true,
                button: None,
            }
        }
        _ => SettingStatus {
            state: recommendation(f, gpu),
            attention: f.recommended.is_none(),
            button: None,
        },
    }
}

/// Why a size may not run here: "Needs 6 GB of memory, more than the 4 GB Specular allows for AI on this computer".
fn unfit_line(u: AiUnfit) -> String {
    let loc = I18n::global().localizer();
    let mb = |m: u64| {
        loc.bytes(
            m.saturating_mul(1_000_000),
            specular_i18n::ByteUnits::Decimal,
        )
    };
    match u {
        AiUnfit::Gpu => t!("ai-size-unfit", why = "gpu", need = "", have = ""),
        AiUnfit::Memory { need_mb, have_mb } => t!(
            "ai-size-unfit",
            why = "memory",
            need = mb(need_mb),
            have = mb(have_mb)
        ),
    }
}

/// The panel under a feature's size setting: its question, its sizes, its licences.
fn size_panel(f: &AiFeatureRow) -> SettingPanel {
    let loc = I18n::global().localizer();
    let bytes = |b: u64| loc.bytes(b, specular_i18n::ByteUnits::Decimal);
    let feature = I18n::global().tr(&format!("settings-ai-{}", f.feature));
    let unfit_of = |tier: &str| {
        f.sizes
            .iter()
            .find(|z| z.tier == tier)
            .and_then(|z| z.unfit)
    };
    let mut rows = Vec::new();
    if let Some(q) = &f.question {
        let download = if q.download == 0 {
            "none".to_owned()
        } else {
            bytes(q.download)
        };
        let (size, feature, download) =
            (size_label(f, q.tier), feature.as_str(), download.as_str());
        // Installed: a switch (download, re-index). Not installed: only a size that may not
        // run here is asked about, before the setting keeps it.
        let installed = f.tier.is_some();
        let mut lines = vec![if q.suggested {
            t!(
                "ai-suggest-question",
                size = size.as_str(),
                feature = feature,
                download = download
            )
        } else if installed {
            t!(
                "ai-switch-question",
                size = size.as_str(),
                feature = feature,
                download = download
            )
        } else {
            t!("ai-unfit-question", size = size.as_str(), feature = feature)
        }];
        lines.extend(unfit_of(q.tier).map(unfit_line));
        rows.push(PanelRow {
            id: format!("switch:{}:{}", f.feature, q.tier),
            label: size_label(f, q.tier),
            lines,
            attention: true,
            button: Some(if installed || q.suggested {
                t!("ai-size-button", action = "switch")
            } else {
                t!("ai-size-action", action = "anyway")
            }),
            ..Default::default()
        });
        rows.push(PanelRow {
            id: format!("keep:{}", f.feature),
            label: f
                .tier
                .map_or_else(|| feature.to_owned(), |t| size_label(f, t)),
            button: Some(if installed {
                t!("ai-size-button", action = "keep")
            } else {
                t!("ai-button", action = "cancel")
            }),
            ..Default::default()
        });
    }
    let failed = matches!(f.state, AiRowState::Failed(_));
    let memory = |z: &AiSize| bytes(u64::from(z.memory_mb) * 1_000_000);
    let gpu = |z: &AiSize| if z.needs_gpu { "yes" } else { "no" };
    // The size buttons wait while something is under way: a question, a download, a switch
    // still indexing, a removal.
    let busy = f.question.is_some()
        || f.previous.is_some()
        || matches!(
            f.state,
            AiRowState::Downloading { .. } | AiRowState::Removing
        );
    // A named size: the way back to following the machine, first under the setting.
    if f.choice != "recommended" && !busy {
        rows.push(PanelRow {
            id: format!("use:{}:recommended", f.feature),
            label: recommended_label(f),
            button: Some(t!("ai-size-action", action = "back")),
            ..Default::default()
        });
    }
    // A row per size. What a size is comes from what is installed, never from the setting.
    for z in &f.sizes {
        let state = if f.target == Some(z.tier) && failed {
            "failed"
        } else if f.target == Some(z.tier) {
            "target"
        } else if f.previous == Some(z.tier) {
            "previous"
        } else if f.tier == Some(z.tier) {
            "current"
        } else if z.on_disk {
            "downloaded"
        } else if f.chosen == Some(z.tier) {
            "missing"
        } else {
            "other"
        };
        let mut lines = Vec::new();
        // Chosen but not here while another size is: say which one is used meanwhile (a
        // question about it says that itself).
        if state == "missing"
            && let Some(cur) = f.tier
            && f.question.as_ref().is_none_or(|q| q.tier != z.tier)
        {
            lines.push(t!("ai-size-chosen-missing", current = size_label(f, cur)));
        }
        lines.extend(z.unfit.map(unfit_line));
        lines.extend(info_lines(&z.info));
        let here = matches!(state, "current" | "previous" | "downloaded");
        let (place, amount) = if here {
            ("disk", z.size)
        } else {
            ("download", z.download)
        };
        // Every size not in use can be chosen: the one installed, or, with nothing installed,
        // the one switching on would install, is in use.
        let in_use = f.tier.map_or(f.chosen == Some(z.tier), |cur| cur == z.tier);
        let choosable = !busy && !in_use && matches!(state, "downloaded" | "missing" | "other");
        let button = choosable.then(|| {
            if z.on_disk {
                t!("ai-size-button", action = "switch")
            } else {
                t!("ai-size-action", action = "use")
            }
        });
        rows.push(PanelRow {
            id: if choosable {
                format!("use:{}:{}", f.feature, z.tier)
            } else {
                format!("size:{}", z.tier)
            },
            label: size_label(f, z.tier),
            value: t!("ai-size-state", state = state),
            lines,
            button,
            detail: t!(
                "ai-size-detail",
                place = place,
                bytes = bytes(amount),
                memory = memory(z),
                gpu = gpu(z)
            ),
            // A failed download, or the size chosen although it may not run here.
            attention: state == "failed" || z.unfit.is_some() && f.chosen == Some(z.tier),
            ..Default::default()
        });
    }
    rows.extend(f.addons.iter().map(|a| addon_row(f, a)));
    for (i, c) in f.coming.iter().enumerate() {
        rows.push(PanelRow {
            id: format!("coming:{i}"),
            label: c.info.name.clone(),
            value: t!("ai-coming"),
            lines: info_lines(&c.info),
            detail: specular_i18n::dot_join(&[bytes(c.size).as_str(), &c.licence]),
            ..Default::default()
        });
    }
    for l in &f.licence_links {
        rows.push(PanelRow {
            id: format!("licence:{}", l.url),
            label: l.name.clone(),
            detail: l.models.clone(),
            button: l.url.starts_with("https://").then(|| t!("ai-licence")),
            ..Default::default()
        });
    }
    SettingPanel {
        rows,
        keywords: "model size small standard large recommended default licence GPU memory training data languages add-on script OCR".into(),
        ..Default::default()
    }
}

fn confirm(what: &str) -> PanelAction {
    PanelAction {
        label: t!("ai-button", action = what),
        question: t!("ai-confirm", action = what),
        confirm: t!("ai-button", action = what),
        cancel: t!("ai-button", action = "cancel"),
    }
}

/// The pack's panel, state line and which switches show.
pub(crate) fn build(page: &AiPage) -> (SettingPanel, SettingStatus, Option<SettingPanel>) {
    let installed = |f: &str| {
        page.features
            .iter()
            .any(|r| r.feature == f && matches!(r.state, AiRowState::Installed { .. }))
    };
    let any = FEATURES.iter().any(|f| installed(f));
    // A removal running: the pack can't be removed again until it ends.
    let removing = page
        .features
        .iter()
        .any(|r| r.state == AiRowState::Removing);
    let mut pack = SettingPanel {
        keywords: "AI ML models offline privacy faces OCR CLIP".into(),
        ..Default::default()
    };
    // Without models the panel's one row says so (and is read out); an empty status keeps the
    // line from repeating it and the setting from showing a button.
    let state = if page.features.is_empty() {
        String::new()
    } else if any {
        t!("ai-state", state = "installed")
    } else if removing {
        t!("ai-removing")
    } else {
        t!("ai-state", state = "available")
    };
    if page.features.is_empty() {
        pack.rows.push(PanelRow {
            id: "none".into(),
            label: t!("ai-state", state = "none"),
            ..Default::default()
        });
    }
    for f in FEATURES {
        if let Some(r) = page.features.iter().find(|r| r.feature == f) {
            pack.rows.push(feature_row(r));
        }
    }
    if any && !removing {
        pack.action = Some(confirm("pack"));
    }
    let status = SettingStatus {
        state,
        attention: false,
        button: None,
    };
    let people = installed("people").then(|| {
        let loc = I18n::global().localizer();
        let grouping = page.face_grouping.map(|on| PanelRow {
            id: format!("grouping:{}", if on { "off" } else { "on" }),
            label: t!("ai-grouping"),
            value: t!("ai-grouping-state", on = if on { "yes" } else { "no" }),
            lines: vec![t!("ai-grouping-help")],
            button: Some(t!("ai-grouping-button", on = if on { "yes" } else { "no" })),
            ..Default::default()
        });
        SettingPanel {
            rows: grouping
                .into_iter()
                .chain(page.people.iter().map(|p| PanelRow {
                    id: format!("forget:{}", p.id),
                    label: if p.name.is_empty() {
                        format!("#{}", p.id)
                    } else {
                        p.name.clone()
                    },
                    value: loc.integer(p.faces as i64),
                    button: Some(t!("ai-button", action = "forget")),
                    ..Default::default()
                }))
                .collect(),
            action: Some(confirm("faces")),
            keywords: "faces face recognition".into(),
            ..Default::default()
        }
    });
    (pack, status, people)
}

impl Chrome {
    /// Shows the Local AI page for `page`. Returns whether anything changed (redraw then).
    pub fn set_ai_page(&mut self, page: &AiPage) -> bool {
        let (pack, status, people) = build(page);
        let mut changed = self.set_setting_panel(PACK, Some(pack));
        changed |= self.set_setting_panel(PEOPLE, people);
        changed |= self.set_ai_status(PACK, Some(status));
        // Install from file…: only with models to match the files against.
        for k in SIDELOAD {
            changed |= self.set_setting_hidden(k, page.features.is_empty());
        }
        for f in FEATURES {
            let row = page.features.iter().find(|r| r.feature == f);
            // A feature being removed is no longer offered: its switch goes until it ends; one
            // whose models are all still coming has none.
            let shown =
                row.is_some_and(|r| !matches!(r.state, AiRowState::Removing | AiRowState::Coming));
            changed |= self.set_setting_hidden(setting_key(f), !shown);
            let Some(size) = size_key(f) else {
                continue;
            };
            let sized = row.filter(|r| shown && r.sizes.len() > 1);
            changed |= self.set_setting_hidden(size, sized.is_none());
            changed |= self.set_setting_panel(
                size,
                sized.map(|r| {
                    let mut p = size_panel(r);
                    if r.feature == "text"
                        && matches!(r.state, AiRowState::Installed { .. })
                        && let Some(on) = page.text_indexing
                    {
                        p.rows.insert(0, text_indexing_row(on));
                    }
                    p
                }),
            );
            changed |= self.set_setting_choices(size, sized.map(choice_labels));
            changed |= self.set_ai_status(size, sized.map(|r| size_status(r, page.gpu.as_deref())));
        }
        changed
    }

    fn set_ai_status(&mut self, key: &'static str, status: Option<SettingStatus>) -> bool {
        if self.ai_status.get(key) == status.as_ref() {
            return false;
        }
        match status {
            Some(s) => self.ai_status.insert(key, s),
            None => self.ai_status.remove(key),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Without Fluent's bidi isolation marks.
    fn plain(s: &str) -> String {
        s.chars()
            .filter(|c| !matches!(c, '\u{2068}' | '\u{2069}'))
            .collect()
    }

    fn row(feature: &'static str, state: AiRowState) -> AiFeatureRow {
        AiFeatureRow {
            feature,
            state,
            size: 88_000_000,
            licences: "Apache-2.0".into(),
            models: format!("{feature}-model 1"),
            ..Default::default()
        }
    }

    /// Three sizes; Large needs more memory than the machine has (and the GPU). Nothing
    /// installed, Recommended (Standard) chosen.
    fn sized(feature: &'static str, state: AiRowState) -> AiFeatureRow {
        let size = |tier, download, memory_mb, needs_gpu, unfit| AiSize {
            tier,
            download,
            size: download,
            on_disk: false,
            memory_mb,
            needs_gpu,
            unfit,
            info: Default::default(),
        };
        let too_big = AiUnfit::Memory {
            need_mb: 3_000,
            have_mb: 2_000,
        };
        AiFeatureRow {
            sizes: vec![
                size("small", 40_000_000, 300, false, None),
                size("standard", 160_000_000, 900, false, None),
                size("large", 900_000_000, 3_000, true, Some(too_big)),
            ],
            licence_links: vec![AiLicence {
                name: "MIT".into(),
                url: "https://example.org/licence".into(),
                models: "clip 1".into(),
            }],
            recommended: Some("standard"),
            choice: "recommended",
            chosen: Some("standard"),
            ..row(feature, state)
        }
    }

    /// Search installed at Standard, the recommendation, on Recommended.
    fn installed_standard() -> AiFeatureRow {
        AiFeatureRow {
            tier: Some("standard"),
            ..sized("search", AiRowState::Installed { indexed: None })
        }
    }

    /// The size panel's rows but the licences: id, value, detail, button ("" for none),
    /// then the lines.
    fn rows(f: &AiFeatureRow) -> Vec<Vec<String>> {
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![f.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        ch.setting_panels[size_key(f.feature).unwrap()]
            .rows
            .iter()
            .filter(|r| !r.id.starts_with("licence:"))
            .map(|r| {
                let mut v = vec![
                    r.id.clone(),
                    plain(&r.value),
                    plain(&r.detail),
                    r.button.as_deref().map(plain).unwrap_or_default(),
                ];
                v.extend(r.lines.iter().map(|l| plain(l)));
                v
            })
            .collect()
    }

    fn info(
        name: &str,
        languages: &str,
        hint: &str,
        data: &str,
        terms: &'static str,
    ) -> AiModelInfo {
        AiModelInfo {
            name: name.into(),
            languages: languages.into(),
            hint: hint.into(),
            data: data.into(),
            data_terms: terms,
            host: "huggingface.co".into(),
        }
    }

    /// The model manager: sizes named by their models with what they read, their training
    /// data and host; coming models listed without a button; a feature with only coming
    /// models has no switch; grouping faces is turned on per library.
    #[test]
    fn models_show_names_data_and_host_and_coming_ones_wait() {
        let mut f = sized("search", AiRowState::Available);
        f.sizes[0].info = info(
            "SigLIP 2 B/32",
            "multilingual",
            "faster",
            "WebLI",
            "unstated",
        );
        f.sizes[1].info = info("SigLIP 2 B/16", "multilingual", "", "WebLI", "unstated");
        f.coming = vec![AiComing {
            info: info("PE Core S16", "english", "", "", "unstated"),
            size: 348_852_712,
            licence: "Apache-2.0".into(),
            licence_url: "https://example.org/l".into(),
        }];
        let r = rows(&f);
        let small = r.iter().find(|r| r[0] == "use:search:small").unwrap();
        assert_eq!(
            small[4..],
            [
                "Many languages · Faster, for slower graphics cards",
                "Training data: WebLI, terms not stated",
                "Downloads from huggingface.co",
            ]
        );
        let coming = r.iter().find(|r| r[0] == "coming:0").unwrap();
        assert_eq!(coming[1], "Coming in a later version");
        assert_eq!(coming[2], "349 MB · Apache-2.0");
        assert_eq!(coming[3], "", "no button");
        assert_eq!(
            coming[4..],
            [
                "English only",
                "Training data: not named, terms not stated",
                "Downloads from huggingface.co"
            ]
        );

        let mut people = row("people", AiRowState::Coming);
        people.coming = vec![AiComing {
            info: info("BlazeFace + SFace", "", "", "CASIA-WebFace", "research"),
            size: 39_780_139,
            licence: "Apache-2.0".into(),
            licence_url: "https://example.org/l".into(),
        }];
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![f.clone(), people],
            ..AiPage::default()
        });
        // The dropdown names the models; Default says which one it means.
        assert_eq!(
            crate::settings::choices("ai.search_size").unwrap()[..3]
                .iter()
                .map(|c| plain(&c.1))
                .collect::<Vec<_>>(),
            ["Default (SigLIP 2 B/16)", "SigLIP 2 B/32", "SigLIP 2 B/16"]
        );
        assert!(ch.setting_hidden(PEOPLE), "nothing to turn on yet");
        let p = &ch.setting_panels[PACK];
        let cr = p.rows.iter().find(|r| r.id == "coming:people").unwrap();
        assert_eq!(plain(&cr.value), "Coming in a later version");
        assert!(cr.button.is_none() && cr.detail.is_empty());
        assert_eq!(
            plain(cr.lines.last().unwrap()),
            "BlazeFace + SFace: 40 MB, Apache-2.0 · Training data: CASIA-WebFace (licensed for research only) · Downloads from huggingface.co"
        );

        // Grouping, under an installed People: off until turned on.
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![row("people", AiRowState::Installed { indexed: None })],
            face_grouping: Some(false),
            text_indexing: None,
            ..AiPage::default()
        });
        let g = &ch.setting_panels[PEOPLE].rows[0];
        assert_eq!(g.id, "grouping:on");
        assert_eq!(
            (plain(&g.value), g.button.as_deref()),
            ("Off".into(), Some("Turn on"))
        );
        assert_eq!(AiRowAction::parse(&g.id), Some(AiRowAction::Grouping(true)));
        assert_eq!(
            AiRowAction::parse("grouping:off"),
            Some(AiRowAction::Grouping(false))
        );
        assert_eq!(AiRowAction::parse("grouping:maybe"), None);
        let a = ch.setting_panels[PEOPLE].action.as_ref().unwrap();
        assert_eq!(
            plain(&a.question),
            "Delete all face data? Every face and person found is deleted. Your photos stay."
        );
    }

    #[test]
    fn empty_build_says_no_models_and_hides_every_switch() {
        let mut ch = Chrome::new();
        assert!(ch.set_ai_page(&AiPage::default()));
        assert!(!ch.set_ai_page(&AiPage::default()));
        let p = &ch.setting_panels[PACK];
        assert_eq!(p.rows.len(), 1);
        assert_eq!(p.rows[0].label, "No models available in this build.");
        assert!(p.rows[0].button.is_none());
        assert!(p.action.is_none());
        let s = ch.status_of(PACK).unwrap();
        assert!(
            s.state.is_empty() && s.button.is_none(),
            "said once, no button"
        );
        for f in FEATURES {
            assert!(ch.setting_hidden(setting_key(f)), "{f}");
        }
        assert!(!ch.setting_panels.contains_key(PEOPLE));
    }

    #[test]
    fn offers_show_size_licence_and_opt_in() {
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![
                row("people", AiRowState::Available),
                row("search", AiRowState::Downloading { done: 1, total: 4 }),
                row("text", AiRowState::Failed("hash mismatch".into())),
            ],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels[PACK];
        // Feature order, not offer order.
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["cancel:search", "download:text", "download:people"]);
        assert_eq!(p.rows[2].label, "People");
        assert_eq!(p.rows[2].button.as_deref(), Some("Download"));
        let detail = plain(&p.rows[2].detail);
        assert!(detail.contains("88 MB"), "{detail}");
        assert!(p.rows[2].detail.contains("Apache-2.0"));
        assert!(p.rows[0].value.contains("25"), "{}", p.rows[0].value);
        assert!(p.rows[1].attention);
        assert_eq!(p.rows[1].lines[0], "hash mismatch");
        assert!(p.action.is_none(), "nothing installed: nothing to remove");
        // Offered features get their switch (switching on installs them); others don't.
        assert!(!ch.setting_hidden("ai.people"));
        assert!(ch.setting_hidden("ai.culling"));
        // One size each: no size choice.
        for k in SIZES {
            assert!(ch.setting_hidden(k), "{k}");
        }
        assert_eq!(
            AiRowAction::parse(&p.rows[0].id),
            Some(AiRowAction::Cancel("search"))
        );
        assert_eq!(AiRowAction::parse("download:weather"), None);
        assert_eq!(
            AiRowAction::parse("forget:12"),
            Some(AiRowAction::Forget(12))
        );
    }

    #[test]
    fn installed_features_get_switches_progress_and_people_controls() {
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![
                row(
                    "search",
                    AiRowState::Installed {
                        indexed: Some((30, 40)),
                    },
                ),
                row("people", AiRowState::Installed { indexed: None }),
                row("culling", AiRowState::Available),
            ],
            people: vec![
                AiPerson {
                    id: 3,
                    name: "Ana".into(),
                    faces: 12,
                },
                AiPerson {
                    id: 4,
                    name: String::new(),
                    faces: 2,
                },
            ],
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        assert!(!ch.setting_hidden("ai.search"));
        assert!(!ch.setting_hidden(PEOPLE));
        assert!(!ch.setting_hidden("ai.culling"), "offered");
        assert!(ch.setting_hidden("ai.text"));
        let p = &ch.setting_panels[PACK];
        assert_eq!(
            plain(&p.rows[0].value),
            "Installed · 30 of 40 images indexed"
        );
        assert_eq!(p.rows[0].button.as_deref(), Some("Remove"));
        assert_eq!(p.action.as_ref().unwrap().label, "Remove pack");
        let people = &ch.setting_panels[PEOPLE];
        assert_eq!(people.rows[0].label, "Ana");
        assert_eq!(people.rows[1].label, "#4");
        assert_eq!(people.rows[0].id, "forget:3");
        assert_eq!(people.rows[0].button.as_deref(), Some("Forget"));
        assert_eq!(
            people.action.as_ref().unwrap().label,
            "Delete all face data"
        );
        // Removing People takes its panel and switch away again.
        assert!(ch.set_ai_page(&AiPage::default()));
        assert!(!ch.setting_panels.contains_key(PEOPLE));
        assert!(ch.setting_hidden(PEOPLE));
    }

    #[test]
    fn installed_features_say_what_their_model_runs_on() {
        let lines = |f: AiFeatureRow| {
            let mut ch = Chrome::new();
            ch.set_ai_page(&AiPage {
                features: vec![f],
                people: Vec::new(),
                gpu: None,
                face_grouping: None,
                text_indexing: None,
            });
            ch.setting_panels[PACK].rows[0]
                .lines
                .iter()
                .map(|l| plain(l))
                .collect::<Vec<_>>()
        };
        let installed = AiRowState::Installed { indexed: None };
        let with = |state: AiRowState, e: AiEngine| AiFeatureRow {
            runs_on: Some(e),
            ..row("search", state)
        };
        let gpu = AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into()));
        // First, right under the feature's name.
        assert_eq!(
            lines(with(installed.clone(), gpu.clone()))[0],
            "Runs on: GPU (AMD Radeon 860M Graphics)"
        );
        let cases = [
            (AiEngine::Gpu(None), "Runs on: GPU"),
            (AiEngine::Cpu("cpu"), "Runs on: CPU"),
            (AiEngine::Cpu("no-gpu"), "Runs on: CPU (no GPU found)"),
            (
                AiEngine::Cpu("differed"),
                "Runs on: CPU — the GPU result differed from the CPU's",
            ),
            (
                AiEngine::Cpu("failed"),
                "Runs on: CPU — the model failed on the GPU",
            ),
            (
                AiEngine::Cpu("setting"),
                "Runs on: CPU (processing is set to “CPU only”)",
            ),
            (
                AiEngine::Cpu("checking"),
                "Runs on: CPU — checking the GPU against it",
            ),
        ];
        for (e, want) in cases {
            assert_eq!(lines(with(installed.clone(), e))[0], want);
        }
        // Not known, or not installed: no line.
        let help = lines(row("search", installed.clone()));
        assert_eq!(help.len(), 1);
        assert_eq!(lines(with(AiRowState::Available, gpu)), help);
    }

    #[test]
    fn a_removal_shows_without_buttons_until_it_ends() {
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![
                row("search", AiRowState::Installed { indexed: None }),
                row("people", AiRowState::Removing),
            ],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels[PACK];
        assert_eq!(plain(&p.rows[0].value), "Installed");
        assert_eq!(plain(&p.rows[1].value), "Removing…");
        // Not twice: no button on the row, no pack removal while it runs.
        assert_eq!(p.rows[1].button, None);
        assert!(p.action.is_none());
        // A feature being removed is no longer offered (its switch and People's panel go).
        assert!(ch.setting_hidden(PEOPLE));
        assert!(!ch.setting_panels.contains_key(PEOPLE));
        // The whole pack going: the state line says so.
        ch.set_ai_page(&AiPage {
            features: vec![
                row("search", AiRowState::Removing),
                row("people", AiRowState::Removing),
            ],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        assert_eq!(plain(&ch.ai_status[PACK].state), "Removing…");
        assert!(ch.setting_panels[PACK].action.is_none());
    }

    #[test]
    fn every_size_shows_with_its_facts_and_the_recommendation_is_named() {
        let mut ch = Chrome::new();
        let mut search = installed_standard();
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: Some("Radeon 780M".into()),
            face_grouping: None,
            text_indexing: None,
        });
        assert!(!ch.setting_hidden("ai.search_size"));
        assert_eq!(size_key("people"), Some("ai.people_size"));
        // One size each: no size choice for the others.
        for k in &SIZES[1..] {
            assert!(ch.setting_hidden(k), "{k}");
        }
        let st = ch.status_of("ai.search_size").unwrap();
        assert_eq!(plain(&st.state), "Default for your Radeon 780M: Standard");
        assert!(st.button.is_none() && !st.attention);
        // Recommended with what it means here, then every size by name, Large included.
        let labels = crate::settings::choices("ai.search_size").unwrap();
        let labels: Vec<(String, String)> =
            labels.into_iter().map(|(k, l)| (k, plain(&l))).collect();
        assert_eq!(
            labels,
            [
                ("recommended".into(), "Default (Standard)".into()),
                ("small".into(), "Small".into()),
                ("standard".into(), "Standard".into()),
                ("large".into(), "Large".into()),
            ]
        );
        let p = &ch.setting_panels["ai.search_size"];
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        // A row per size; the ones not in use can be chosen. No "Other sizes" summary, and
        // no way back to Recommended while it is chosen.
        assert_eq!(
            ids,
            [
                "use:search:small",
                "size:standard",
                "use:search:large",
                "licence:https://example.org/licence"
            ]
        );
        assert_eq!(plain(&p.rows[1].value), "In use");
        assert_eq!(p.rows[1].button, None);
        assert_eq!(plain(&p.rows[1].detail), "160 MB on disk · 900 MB memory");
        assert_eq!(p.rows[0].button.as_deref(), Some("Use this size"));
        assert_eq!(plain(&p.rows[0].detail), "40 MB download · 300 MB memory");
        assert!(p.rows[0].lines.is_empty());
        // Large doesn't fit: shown with why, not hidden; choosing it asks (the app's part).
        let large = &p.rows[2];
        assert_eq!(plain(&large.label), "Large");
        assert_eq!(large.button.as_deref(), Some("Use this size"));
        assert_eq!(
            plain(&large.detail),
            "900 MB download · 3 GB memory · needs the GPU"
        );
        assert_eq!(
            large.lines.iter().map(|l| plain(l)).collect::<Vec<_>>(),
            ["Needs 3 GB of memory, more than the 2 GB Specular allows for AI on this computer."]
        );
        assert!(!large.attention, "not chosen: a fact, not a problem");
        assert_eq!(
            AiRowAction::parse(&large.id),
            Some(AiRowAction::Use("search", "large"))
        );
        assert_eq!(
            AiRowAction::parse("use:search:recommended"),
            Some(AiRowAction::Use("search", "recommended"))
        );
        assert_eq!(AiRowAction::parse("use:search:larger"), None);
        assert_eq!(AiRowAction::parse("use:weather:large"), None);
        // Sizes that don't need the GPU don't mention it (the feature's row says where its
        // model runs).
        assert!(p.rows[..2].iter().all(|r| !r.detail.contains("GPU")));
        assert_eq!(p.rows[3].button.as_deref(), Some("View licence"));
        assert_eq!(
            AiRowAction::parse(&p.rows[3].id),
            Some(AiRowAction::Licence("https://example.org/licence"))
        );
        assert_eq!(AiRowAction::parse("licence:file:///etc/passwd"), None);
        // A size needing a GPU the machine can't use says that instead.
        search.sizes[2].unfit = Some(AiUnfit::Gpu);
        assert_eq!(
            rows(&search)[2][4],
            "Needs a GPU, and this computer has none it can use."
        );

        // No GPU name: the plain line. Nothing fits: says so, with attention, and
        // Recommended is offered plainly.
        search.recommended = None;
        ch.set_ai_page(&AiPage {
            features: vec![search],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let st = ch.status_of("ai.search_size").unwrap();
        assert!(st.attention);
        assert_eq!(
            plain(&crate::settings::choices("ai.search_size").unwrap()[0].1),
            "Default"
        );
        // Leaving the page empty clears the size's panel, status and choices.
        ch.set_ai_page(&AiPage::default());
        assert!(ch.setting_hidden("ai.search_size"));
        assert!(!ch.setting_panels.contains_key("ai.search_size"));
        assert!(ch.status_of("ai.search_size").is_none());
        assert!(crate::settings::choices("ai.search_size").is_none());
    }

    /// Any feature that comes in more than one size gets the choice, not only Search.
    #[test]
    fn every_multi_size_feature_gets_its_size_setting() {
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![
                sized("people", AiRowState::Available),
                row("text", AiRowState::Available),
            ],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        assert!(!ch.setting_hidden("ai.people_size"));
        assert!(ch.setting_hidden("ai.text_size"));
        assert!(ch.setting_hidden("ai.search_size"));
        let p = &ch.setting_panels["ai.people_size"];
        assert_eq!(p.rows[0].id, "use:people:small");
        assert_eq!(
            AiRowAction::parse(&p.rows[0].id),
            Some(AiRowAction::Use("people", "small"))
        );
        for (f, k) in FEATURES.into_iter().zip(SIZES) {
            assert_eq!(size_key(f), Some(k));
        }
    }

    /// Nothing installed: the size switching on would install has no button; the others
    /// can be chosen, Large too (with its warning).
    #[test]
    fn nothing_installed_offers_every_other_size() {
        let mut search = sized("search", AiRowState::Available);
        search.chosen = Some("standard");
        let r = rows(&search);
        assert_eq!(
            r,
            [
                vec![
                    "use:search:small",
                    "",
                    "40 MB download · 300 MB memory",
                    "Use this size"
                ],
                vec![
                    "size:standard",
                    "Not downloaded",
                    "160 MB download · 900 MB memory",
                    ""
                ],
                vec![
                    "use:search:large",
                    "",
                    "900 MB download · 3 GB memory · needs the GPU",
                    "Use this size",
                    "Needs 3 GB of memory, more than the 2 GB Specular allows for AI on this computer."
                ],
            ]
        );
        // While it downloads: no size buttons.
        search.state = AiRowState::Downloading { done: 1, total: 4 };
        search.target = Some("standard");
        assert!(rows(&search).iter().all(|r| r[3].is_empty()), "{r:?}");
    }

    /// One size installed, another downloaded, the third not: Switch for the downloaded one,
    /// "Use this size" for the other.
    #[test]
    fn a_downloaded_size_offers_switch() {
        let mut search = installed_standard();
        search.sizes[0].on_disk = true;
        search.sizes[0].download = 0;
        let r = rows(&search);
        assert_eq!(
            r[0],
            [
                "use:search:small",
                "Downloaded, not in use",
                "40 MB on disk · 300 MB memory",
                "Switch"
            ]
        );
        assert_eq!(r[2][3], "Use this size");
    }

    /// A named size: the setting's line says which size runs while another one is named,
    /// and "Back to recommended" leads back. Recommended with another size in use (a declined
    /// suggestion) is no problem: just the recommendation.
    #[test]
    fn a_named_size_offers_the_way_back_to_recommended() {
        let mut search = installed_standard();
        search.choice = "small";
        search.chosen = Some("small");
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let st = &ch.ai_status["ai.search_size"];
        assert_eq!(plain(&st.state), "Standard is in use, not Small.");
        assert!(st.attention && st.button.is_none());
        let p = &ch.setting_panels["ai.search_size"];
        let ids: Vec<&str> = p.rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "use:search:recommended",
                "use:search:small",
                "size:standard",
                "use:search:large",
                "licence:https://example.org/licence"
            ]
        );
        assert_eq!(
            plain(&p.rows[1].lines[0]),
            "Chosen, but not downloaded yet: Standard stays in use."
        );
        // The way back comes first, under the setting.
        let back = &p.rows[0];
        assert_eq!(plain(&back.label), "Default (Standard)");
        assert_eq!(back.button.as_deref(), Some("Back to default"));
        assert_eq!(
            AiRowAction::parse(&back.id),
            Some(AiRowAction::Use("search", "recommended"))
        );
        // The named size is the one in use: the plain recommendation line, still the way back.
        search.choice = "standard";
        search.chosen = Some("standard");
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        assert_eq!(
            plain(&ch.ai_status["ai.search_size"].state),
            "Default for this computer: Standard"
        );
        assert!(
            ch.setting_panels["ai.search_size"]
                .rows
                .iter()
                .any(|r| r.id == "use:search:recommended")
        );
        // Recommended, and a declined suggestion left another size in use: no alarm, no way
        // back (it is there), Large can still be chosen.
        search.choice = "recommended";
        search.chosen = Some("large");
        search.recommended = Some("large");
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let st = &ch.ai_status["ai.search_size"];
        assert_eq!(plain(&st.state), "Default for this computer: Large");
        assert!(!st.attention);
        let r = rows(&search);
        assert_eq!(r.len(), 3, "{r:?}");
        assert_eq!(r[2][0], "use:search:large");
    }

    #[test]
    fn a_size_switch_asks_first_and_shows_both_sizes_while_it_runs() {
        let mut ch = Chrome::new();
        let mut search = installed_standard();
        search.choice = "small";
        search.chosen = Some("small");
        search.question = Some(AiQuestion {
            tier: "small",
            download: 40_000_000,
            suggested: false,
        });
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels["ai.search_size"];
        assert_eq!(p.rows[0].id, "switch:search:small");
        assert!(p.rows[0].attention);
        let q = plain(&p.rows[0].lines[0]);
        assert!(q.contains("40 MB") && q.contains("Small"), "{q}");
        assert_eq!(p.rows[0].lines.len(), 1, "fits: no warning");
        assert_eq!(p.rows[0].button.as_deref(), Some("Switch"));
        assert_eq!(p.rows[1].id, "keep:search");
        assert_eq!(p.rows[1].button.as_deref(), Some("Keep current size"));
        assert_eq!(
            AiRowAction::parse(&p.rows[0].id),
            Some(AiRowAction::Switch("search", "small"))
        );
        assert_eq!(
            AiRowAction::parse(&p.rows[1].id),
            Some(AiRowAction::Keep("search"))
        );
        assert_eq!(AiRowAction::parse("switch:search:huge"), None);
        // Asked: no size buttons and no way back until it is answered, and the size's row
        // doesn't repeat the question.
        assert!(
            p.rows[2..]
                .iter()
                .all(|r| r.button.is_none() || r.id.starts_with("licence:"))
        );
        assert!(!p.rows.iter().any(|r| r.id == "use:search:recommended"));
        let small = p.rows.iter().find(|r| r.id == "size:small").unwrap();
        assert!(small.lines.is_empty());
        // The status line leaves it to the question.
        assert!(!ch.ai_status["ai.search_size"].attention);

        // Asked about a size that may not run here: the question warns.
        search.question = Some(AiQuestion {
            tier: "large",
            download: 900_000_000,
            suggested: false,
        });
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let q = &ch.setting_panels["ai.search_size"].rows[0];
        assert_eq!(
            q.lines.iter().map(|l| plain(l)).collect::<Vec<_>>(),
            [
                "Switch Search by description to Large? This downloads 900 MB and indexes \
                 your library again; the current size stays in use until then.",
                "Needs 3 GB of memory, more than the 2 GB Specular allows for AI on this computer."
            ]
        );

        // Switched: the new size indexes while the old one still answers searches.
        search.question = None;
        search.tier = Some("small");
        search.previous = Some("standard");
        search.state = AiRowState::Installed {
            indexed: Some((5, 10)),
        };
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let pack = &ch.setting_panels[PACK];
        assert_eq!(
            plain(&pack.rows[0].value),
            "Switching to Small · 5 of 10 images indexed"
        );
        let p = &ch.setting_panels["ai.search_size"];
        assert_eq!(plain(&p.rows[0].value), "In use");
        assert_eq!(
            plain(&p.rows[1].value),
            "In use until the new size has indexed your library"
        );
        // Mid-switch: nothing to choose until it is done.
        assert!(
            p.rows
                .iter()
                .all(|r| r.button.is_none() || r.id.starts_with("licence:"))
        );

        // A failed switch: marked on the size it tried; the others can still be chosen.
        search.previous = None;
        search.tier = Some("standard");
        search.target = Some("small");
        search.state = AiRowState::Failed("hash mismatch".into());
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels["ai.search_size"];
        assert_eq!(p.rows[0].id, "use:search:recommended");
        let small = &p.rows[1];
        assert!(small.attention);
        assert_eq!(plain(&small.value), "Download failed");
        assert_eq!(small.button, None);
        let r = rows(&search);
        assert_eq!(r[2][3], "", "in use");
        assert_eq!(r[3][3], "Use this size");
    }

    /// A size that may not run here, chosen before the feature is installed: asked before
    /// the setting keeps it, with the warning; Cancel puts the setting back.
    #[test]
    fn choosing_a_size_that_does_not_fit_asks_with_the_warning() {
        let mut search = sized("search", AiRowState::Available);
        search.choice = "large";
        search.chosen = Some("large");
        search.question = Some(AiQuestion {
            tier: "large",
            download: 900_000_000,
            suggested: false,
        });
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![search.clone()],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels["ai.search_size"];
        let q = &p.rows[0];
        assert_eq!(q.id, "switch:search:large");
        assert!(q.attention);
        assert_eq!(
            q.lines.iter().map(|l| plain(l)).collect::<Vec<_>>(),
            [
                "Use Large for Search by description anyway? It may not run on this \
                 computer, or only very slowly.",
                "Needs 3 GB of memory, more than the 2 GB Specular allows for AI on this computer."
            ]
        );
        assert_eq!(q.button.as_deref(), Some("Use anyway"));
        let cancel = &p.rows[1];
        assert_eq!(cancel.id, "keep:search");
        assert_eq!(cancel.label, "Search by description");
        assert_eq!(cancel.button.as_deref(), Some("Cancel"));
        // Kept: the chosen size is marked, with its warning, and the way back is offered.
        search.question = None;
        let r = rows(&search);
        assert_eq!(r[3][0], "size:large");
        assert_eq!(r[3][1], "Not downloaded");
        let mut ch = Chrome::new();
        ch.set_ai_page(&AiPage {
            features: vec![search],
            people: Vec::new(),
            gpu: None,
            face_grouping: None,
            text_indexing: None,
        });
        let p = &ch.setting_panels["ai.search_size"];
        assert_eq!(p.rows[0].id, "use:search:recommended");
        assert!(p.rows[3].attention);
    }
}
