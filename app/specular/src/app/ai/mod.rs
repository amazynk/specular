//! Local AI: the pack ([`specular_ai::Pack`]) behind Settings › Local AI
//! ([`specular_widgets::AiPage`]), the choice of model sizes ([`plan`]), Search's background
//! indexing ([`index`]) and search by description ([`semantic`]).
//!
//! The pack is made on an `ai-page` worker the first time it is needed: Settings opens, a
//! feature's setting changes, or a while after startup when a feature is on (so Search's
//! model loads and indexing resumes). The page is rebuilt every [`POLL`] while Settings
//! stays open (download progress, indexing counts), after a button and when the pack's
//! background work ends.
//!
//! None of it touches the catalogue on the UI thread: the pack is made, the machine looked
//! at ([`MachineFacts::gather`]) and the page built (indexing counts and people come from the
//! catalogue, which another writer may hold for seconds) on the worker, which also applies
//! what changed ([`plan::decide`]: install a size, ask before switching, remember the
//! machine) and hands the page back. The buttons only start the pack's own threads; a
//! feature shows "Removing…", without a button, until its removal ends.
//!
//! Sizes: each feature's size setting is Recommended or a size by name. Switching a feature
//! on installs the size its setting picks; a size change that downloads and re-indexes, or
//! names a size that may not run here, is asked first on the page (whose size rows set the
//! setting, [`Follow::Use`]); after the path selector's first calibration ([`recheck`]) and
//! on another GPU the recommendation is looked at again while the setting is Recommended,
//! and a new one offered with a notice (never downloaded unasked). The old relative values
//! (Smaller, Larger) are rewritten to the size they mean here by the first build.
//!
//! Downloads go through the pack's [`specular_ai::transport::Transports`], made in
//! [`make_pack`]: `file://` and HTTPS ([`https`], specular-remote's client).
//! `SPECULAR_AI_MANIFEST` names another manifest and `SPECULAR_AI_PLACEHOLDERS=1` offers its
//! placeholder (`file://`) entries, for development.

pub(crate) mod device;
pub(crate) mod faces;
pub(crate) mod https;
pub(crate) mod index;
pub(crate) mod plan;
pub(crate) mod semantic;
pub(crate) mod sideload;
pub(crate) mod text_index;
pub(crate) mod text_search;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use specular_ai::choose::{self, MachineFacts, SizeChoice, Unfit};
use specular_ai::runtime::{CpuWhy, RunsOn};
use specular_ai::{Feature, FeatureState, Manifest, Pack, Tier};
use specular_gfx::wgpu;
use specular_i18n::{I18n, t};
use specular_settings::{ModelSize, Origin, Settings};
use specular_widgets::{
    AiAddon, AiComing, AiEngine, AiFeatureRow, AiLicence, AiModelInfo, AiPage, AiPerson,
    AiQuestion, AiRowAction, AiRowState, AiSize, AiUnfit, Modal, PanelEvent, ai_keys,
};

use crate::app::App;
use crate::loader::Wake;
use plan::{Ask, Step, Why};

/// How often the open page is rebuilt.
const POLL: Duration = Duration::from_secs(1);
/// How long after startup the pack is made when a feature is on.
const STARTUP: Duration = Duration::from_secs(5);
/// The suggestion notice's button key ([`App::ai_notice`]).
const NOTICE_KEY: u64 = (1 << 62) | (1 << 61);

/// What the page's worker is told: the machine, the settings, and what to apply.
#[derive(Clone, Default)]
pub(crate) struct Ctx {
    /// The app's graphics adapter (its name and memory).
    pub(crate) adapter: Option<wgpu::AdapterInfo>,
    /// Each feature's switch, in [`Feature::ALL`] order.
    pub(crate) on: [bool; Feature::ALL.len()],
    /// Each feature's size setting, in [`Feature::ALL`] order (the build migrates old
    /// relative values, [`plan::migrate`]).
    pub(crate) sizes: [ModelSize; Feature::ALL.len()],
    /// What to look at.
    pub(crate) work: Vec<Work>,
    /// The questions waiting (shown on the page).
    pub(crate) questions: BTreeMap<Feature, Ask>,
}

/// A feature's place in [`Feature::ALL`] (and in [`Ctx`]'s arrays).
fn index_of(f: Feature) -> usize {
    Feature::ALL.iter().position(|x| *x == f).unwrap_or(0)
}

impl Ctx {
    fn on(&self, f: Feature) -> bool {
        self.on[index_of(f)]
    }

    fn from_settings(s: &Settings) -> Ctx {
        Ctx {
            on: [
                s.ai.search,
                s.ai.text,
                s.ai.culling,
                s.ai.people,
                s.ai.speech,
                s.ai.masks,
                s.ai.denoise,
                s.ai.remove,
                s.ai.depth,
            ],
            sizes: Feature::ALL.map(|f| size_setting(s, f)),
            ..Ctx::default()
        }
    }
}

/// A feature's size setting.
fn size_setting(s: &Settings, f: Feature) -> ModelSize {
    match f {
        Feature::Search => s.ai.search_size,
        Feature::Text => s.ai.text_size,
        Feature::Culling => s.ai.culling_size,
        Feature::People => s.ai.people_size,
        Feature::Speech => s.ai.speech_size,
        Feature::Masks => s.ai.masks_size,
        Feature::Denoise => s.ai.denoise_size,
        Feature::Remove => s.ai.remove_size,
        Feature::Depth => s.ai.depth_size,
    }
}

fn set_size(s: &mut Settings, f: Feature, m: ModelSize) {
    match f {
        Feature::Search => s.ai.search_size = m,
        Feature::Text => s.ai.text_size = m,
        Feature::Culling => s.ai.culling_size = m,
        Feature::People => s.ai.people_size = m,
        Feature::Speech => s.ai.speech_size = m,
        Feature::Masks => s.ai.masks_size = m,
        Feature::Denoise => s.ai.denoise_size = m,
        Feature::Remove => s.ai.remove_size = m,
        Feature::Depth => s.ai.depth_size = m,
    }
}

/// Something for the worker to apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Work {
    Look(Feature, Why),
    /// Yes to a question: switch to that size (not installed, a size kept anyway: nothing
    /// to install, the setting applies when it is switched on).
    Switch(Feature, Tier),
    /// No to a suggestion: keep the size, remember the machine.
    Keep(Feature),
}

/// What a build reports besides the page.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Outcome {
    /// New questions (Some) and questions that no longer apply (None).
    pub(crate) asks: Vec<(Feature, Option<Ask>)>,
    /// Features installed before sizes existed (their switches go on), from the build that
    /// made the pack.
    pub(crate) upgraded: Vec<Feature>,
    /// Old relative size settings (Smaller, Larger) and the absolute choice each became on
    /// this machine ([`plan::migrate`]): the settings are rewritten to them.
    pub(crate) migrated: Vec<(Feature, SizeChoice)>,
}

/// What a page build hands back: the pack (made the first time), its page and what it did.
type Built = (Arc<Pack>, AiPage, Outcome);

/// The pack and its page, made off the UI thread.
#[derive(Default)]
pub(crate) struct Pages {
    /// `None` until the first build has made it (and in screenshot mode).
    pub(crate) pack: Option<Arc<Pack>>,
    /// A build is running on the worker (finished without a page: it panicked, and the
    /// next request builds again).
    building: Option<std::thread::JoinHandle<()>>,
    /// What the worker made: the pack (the first time) and the page.
    built: Arc<Mutex<Option<Built>>>,
    /// The pack's [`Pack::changes`] when its page was last asked for.
    changes: u64,
    /// A removal or People change ended in a panic since the app last said so.
    pub(crate) internal_failure: bool,
}

impl Pages {
    /// A page the worker finished, with what its build did (the pack adopted with the first).
    pub(crate) fn take_all(&mut self) -> Option<(AiPage, Outcome)> {
        let (pack, page, out) = self.built.lock().take()?;
        self.building = None;
        self.pack.get_or_insert(pack);
        Some((page, out))
    }

    /// Whether the pack's background work ended since the last call (its page is stale),
    /// logging how it went.
    pub(crate) fn pack_changed(&mut self) -> bool {
        let Some(pack) = &self.pack else {
            return false;
        };
        for (what, r) in pack.take_finished() {
            match r {
                Ok(()) => specular_log::info!(target: specular_log::JOBS, "ai: {what}"),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: {what}: {e}");
                    self.internal_failure |= e == specular_ai::pack::INTERNAL_ERROR;
                }
            }
        }
        let c = pack.changes();
        std::mem::replace(&mut self.changes, c) != c
    }

    /// Builds the page on an `ai-page` worker (making the pack with `make` the first time),
    /// applying `ctx`'s work first, then `wake`s; [`Pages::take_all`] has it. False (nothing
    /// started) while one is running.
    pub(crate) fn request_with(
        &mut self,
        make: impl FnOnce() -> Pack + Send + 'static,
        wake: Wake,
        ctx: Ctx,
    ) -> bool {
        if let Some(b) = &self.building {
            if !b.is_finished() || self.built.lock().is_some() {
                return false;
            }
            specular_log::warn!(target: specular_log::UI, "ai: the page build stopped; building again");
            self.building = None;
        }
        let pack = self.pack.clone();
        let built = self.built.clone();
        let handle = specular_thread::builder("ai-page")
            .spawn(move || {
                let made = pack.is_none();
                let pack = pack.unwrap_or_else(|| Arc::new(make()));
                let (page, mut out) = build(&pack, &ctx);
                if made {
                    out.upgraded = pack.upgraded().to_vec();
                }
                *built.lock() = Some((pack, page, out));
                wake();
            })
            .ok();
        let started = handle.is_some();
        self.building = handle;
        started
    }
}

/// Applies `ctx`'s work to the pack, then builds the page (on the worker).
pub(crate) fn build(pack: &Pack, ctx: &Ctx) -> (AiPage, Outcome) {
    let facts = MachineFacts::gather(ctx.adapter.as_ref());
    let gpu = ctx
        .adapter
        .as_ref()
        .map(|a| a.name.trim().to_owned())
        .filter(|n| !n.is_empty());
    let now = choose::basis(gpu.as_deref().unwrap_or(""), &facts);
    let mut out = Outcome::default();
    let mut questions = ctx.questions.clone();
    // The size choices, old relative values migrated to what they mean here.
    let mut sizes = [SizeChoice::Recommended; Feature::ALL.len()];
    let mut offers = None;
    for (i, f) in Feature::ALL.into_iter().enumerate() {
        let m = ctx.sizes[i];
        sizes[i] = plan::choice(m).unwrap_or_else(|| {
            let status = offers.get_or_insert_with(|| pack.status());
            let offer = status
                .iter()
                .find(|s| s.feature == f)
                .and_then(|s| s.offer.as_ref());
            let c = plan::migrate(m, offer, &facts).unwrap_or_default();
            out.migrated.push((f, c));
            c
        });
    }
    let size = |f: Feature| sizes[index_of(f)];
    if !ctx.work.is_empty() {
        let status = pack.status();
        for w in &ctx.work {
            match *w {
                Work::Look(f, why) => {
                    let Some(s) = status.iter().find(|s| s.feature == f) else {
                        continue;
                    };
                    let step = plan::decide(s, &facts, &now, ctx.on(f), size(f), why, |t| {
                        pack.download_size(f, t)
                    });
                    match step {
                        Step::Nothing => {}
                        Step::Install(t) => {
                            pack.install_for(f, t, Some(now.clone()));
                        }
                        Step::Ask(a) => {
                            if questions.get(&f) != Some(&a) {
                                questions.insert(f, a);
                                out.asks.push((f, Some(a)));
                            }
                        }
                        Step::Record => record(pack, f, &now),
                        Step::Forget => forget(f, &mut questions, &mut out),
                    }
                }
                Work::Switch(f, t) => {
                    forget(f, &mut questions, &mut out);
                    if status.iter().any(|s| s.feature == f && s.tier.is_some()) {
                        pack.install_for(f, t, Some(now.clone()));
                    }
                }
                Work::Keep(f) => {
                    forget(f, &mut questions, &mut out);
                    record(pack, f, &now);
                }
            }
        }
    }
    let status = pack.status();
    // A question answered elsewhere (the size installed or on its way) goes, and a switch
    // question once nothing is installed (one about a size that may not run here stays).
    for s in &status {
        if let Some(a) = questions.get(&s.feature)
            && (s.tier == Some(a.tier)
                || s.target == Some(a.tier) && matches!(s.state, FeatureState::Downloading { .. })
                || s.tier.is_none() && a.unfit.is_none())
        {
            forget(s.feature, &mut questions, &mut out);
        }
    }
    let page = page(pack, status, &facts, gpu, &sizes, &questions);
    (page, out)
}

fn forget(f: Feature, q: &mut BTreeMap<Feature, Ask>, out: &mut Outcome) {
    if q.remove(&f).is_some() {
        out.asks.push((f, None));
    }
}

fn record(pack: &Pack, f: Feature, now: &str) {
    if let Err(e) = pack.store().set_basis(f, Some(now.to_owned())) {
        specular_log::warn!(target: specular_log::JOBS, "ai: {f}: {e}");
    }
}

fn row_state(pack: &Pack, s: &specular_ai::FeatureStatus) -> Option<AiRowState> {
    Some(match &s.state {
        FeatureState::Unavailable => return None,
        FeatureState::Available => AiRowState::Available,
        FeatureState::Downloading { done, total } => AiRowState::Downloading {
            done: *done,
            total: *total,
        },
        FeatureState::Failed(e) if e == specular_ai::pack::INTERNAL_ERROR => {
            AiRowState::Failed(t!("ai-failed-internal"))
        }
        // Refused by the no-network switch: not a failure, the network is off.
        FeatureState::Failed(e) if e.contains(specular_core::net::off::MESSAGE) => {
            AiRowState::NetworkOff
        }
        FeatureState::Failed(e) => AiRowState::Failed(e.clone()),
        FeatureState::Installed => AiRowState::Installed {
            indexed: pack.counts(s.feature).map(|(total, done)| (done, total)),
        },
        FeatureState::Removing => AiRowState::Removing,
    })
}

fn model_list<'a>(models: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    let mut v: Vec<String> = Vec::new();
    for (id, version) in models {
        let s = format!("{id} {version}");
        if !v.contains(&s) {
            v.push(s);
        }
    }
    v.join(", ")
}

/// What the page says a model runs on.
fn engine(r: RunsOn, gpu: Option<&str>) -> AiEngine {
    match r {
        RunsOn::Gpu => AiEngine::Gpu(gpu.map(str::to_owned)),
        RunsOn::Cpu(why) => AiEngine::Cpu(match why {
            CpuWhy::NoGpu => "no-gpu",
            CpuWhy::Differed => "differed",
            CpuWhy::Failed => "failed",
            CpuWhy::Setting => "setting",
            CpuWhy::Checking => "checking",
            CpuWhy::Chosen => "cpu",
        }),
    }
}

/// What the model manager says about a model ([`specular_ai::manifest::Choice`]): its name,
/// languages, hint, training data and host.
fn model_info(c: Option<&specular_ai::manifest::Choice>, host: Option<&str>) -> AiModelInfo {
    let Some(c) = c else {
        return AiModelInfo {
            host: host.unwrap_or_default().to_owned(),
            ..AiModelInfo::default()
        };
    };
    AiModelInfo {
        name: c.name.clone(),
        languages: c.languages.clone().unwrap_or_default(),
        hint: c.hint.clone().unwrap_or_default(),
        data: c.data.clone(),
        data_terms: c.data_terms.id(),
        host: c.host.as_deref().or(host).unwrap_or_default().to_owned(),
    }
}

/// A model listed but not installable yet.
fn coming(c: &specular_ai::manifest::Choice) -> AiComing {
    let (licence, licence_url) = c.licence.clone().unwrap_or_default();
    AiComing {
        info: model_info(Some(c), None),
        size: c.size.unwrap_or(0),
        licence,
        licence_url,
    }
}

/// Why the page says a size may not run here.
fn unfit(u: Unfit) -> AiUnfit {
    match u {
        Unfit::Gpu => AiUnfit::Gpu,
        Unfit::Memory { need_mb, budget_mb } => AiUnfit::Memory {
            need_mb,
            have_mb: budget_mb,
        },
    }
}

fn page(
    pack: &Pack,
    status: Vec<specular_ai::FeatureStatus>,
    facts: &MachineFacts,
    gpu: Option<String>,
    sizes: &[SizeChoice; Feature::ALL.len()],
    questions: &BTreeMap<Feature, Ask>,
) -> AiPage {
    let installed = pack.installed_models();
    let features = status
        .into_iter()
        .filter_map(|s| {
            let f = s.feature;
            let listed: Vec<AiComing> = pack
                .manifest()
                .choices(f)
                .iter()
                .filter(|c| c.coming)
                .map(coming)
                .collect();
            // A feature with nothing to install yet still lists the models it will have.
            let state = match row_state(pack, &s) {
                Some(st) => st,
                None if !listed.is_empty() => AiRowState::Coming,
                None => return None,
            };
            let mut row = AiFeatureRow {
                feature: ai_keys::FEATURES.into_iter().find(|x| *x == f.id())?,
                state,
                tier: s.tier.map(Tier::id),
                target: s.target.map(Tier::id),
                previous: s.previous.map(Tier::id),
                question: questions.get(&f).map(|a| AiQuestion {
                    tier: a.tier.id(),
                    download: a.download,
                    suggested: a.suggested,
                }),
                coming: listed,
                ..AiFeatureRow::default()
            };
            let choice = sizes[index_of(f)];
            row.choice = choice.id();
            let off = specular_core::net::off::is_off();
            match &s.offer {
                Some(o) => {
                    let chosen = choose::choose(o, facts, choice);
                    let shown = chosen.and_then(|t| o.with_tier(t));
                    let o2 = shown.as_ref().unwrap_or(o);
                    // What it would download; the size installed: its files.
                    row.size = chosen
                        .filter(|t| s.tier != Some(*t))
                        .and_then(|t| pack.download_size(f, t))
                        .unwrap_or_else(|| o2.size());
                    row.chosen = chosen.map(Tier::id);
                    // Network off: say so, unless its files are here to install.
                    if off
                        && row.state == AiRowState::Available
                        && chosen.is_some_and(|t| pack.needs_network(f, t))
                    {
                        row.state = AiRowState::NetworkOff;
                    }
                    row.licences = o2
                        .licences()
                        .iter()
                        .map(|(l, _)| *l)
                        .collect::<Vec<_>>()
                        .join(", ");
                    row.models = model_list(
                        o2.models
                            .iter()
                            .map(|m| (m.id.as_str(), m.version.as_str())),
                    );
                    let fits = choose::assess(o, facts);
                    row.sizes = o
                        .tiers
                        .iter()
                        .map(|t| {
                            // Files not installed yet (none: all of them are, and present).
                            let download =
                                pack.download_size(f, t.tier).unwrap_or_else(|| t.size());
                            AiSize {
                                tier: t.tier.id(),
                                download,
                                // Converted models take less than they download.
                                size: pack.disk_size(f, t.tier).unwrap_or_else(|| t.size()),
                                on_disk: download == 0,
                                memory_mb: t.memory_mb(),
                                needs_gpu: t.needs_gpu(),
                                unfit: fits
                                    .iter()
                                    .find(|x| x.tier == t.tier)
                                    .and_then(|x| x.unfit)
                                    .map(unfit),
                                info: model_info(o.choice(t.tier), o.host(t.tier)),
                            }
                        })
                        .collect();
                    row.recommended = choose::recommend(o, facts).map(Tier::id);
                    row.licence_links = o
                        .all_licences()
                        .into_iter()
                        .map(|(name, url)| AiLicence {
                            name: name.to_owned(),
                            url: url.to_owned(),
                            models: model_list(
                                o.tiers
                                    .iter()
                                    .flat_map(|t| &t.models)
                                    .filter(|m| m.licence == name)
                                    .map(|m| (m.id.as_str(), m.version.as_str())),
                            ),
                        })
                        .collect();
                    row.addons = addons(pack, f, o, off);
                }
                None => {
                    let mine: Vec<_> = installed.iter().filter(|m| m.feature == f).collect();
                    row.size = mine.iter().map(|m| m.size).sum();
                    row.models =
                        model_list(mine.iter().map(|m| (m.id.as_str(), m.version.as_str())));
                }
            }
            if f == Feature::Search && matches!(row.state, AiRowState::Installed { .. }) {
                row.runs_on = index::search_runs_on().map(|r| engine(r, gpu.as_deref()));
            }
            Some(row)
        })
        .collect();
    let people = pack
        .people()
        .into_iter()
        .map(|p| AiPerson {
            id: p.id,
            name: p.name.unwrap_or_default(),
            faces: p.faces,
        })
        .collect();
    AiPage {
        features,
        people,
        gpu,
        face_grouping: pack.face_grouping(),
        text_indexing: pack.text_indexing(),
    }
}

/// A feature's add-ons (Text's script recognisers) as the page shows them.
fn addons(pack: &Pack, f: Feature, o: &specular_ai::Offer, off: bool) -> Vec<AiAddon> {
    pack.addon_status(f)
        .into_iter()
        .map(|a| {
            let slot = specular_ai::manifest::Slot::Addon(a.id.clone());
            let choice = o.choices.iter().find(|c| c.slot == slot);
            let models = pack.manifest().addon_models(f, &a.id, true);
            let host = models
                .iter()
                .max_by_key(|m| m.size)
                .and_then(|m| m.urls.first())
                .and_then(|u| u.strip_prefix("https://"))
                .and_then(|u| u.split('/').next())
                .map(str::to_owned);
            let mut licences: Vec<&str> = models.iter().map(|m| m.licence.as_str()).collect();
            licences.dedup();
            let state = match a.state {
                specular_ai::FeatureState::Installed => AiRowState::Installed { indexed: None },
                specular_ai::FeatureState::Downloading { done, total } => {
                    AiRowState::Downloading { done, total }
                }
                specular_ai::FeatureState::Failed(e) if e == specular_ai::pack::INTERNAL_ERROR => {
                    AiRowState::Failed(t!("ai-failed-internal"))
                }
                specular_ai::FeatureState::Failed(e)
                    if e.contains(specular_core::net::off::MESSAGE) =>
                {
                    AiRowState::NetworkOff
                }
                specular_ai::FeatureState::Failed(e) => AiRowState::Failed(e),
                specular_ai::FeatureState::Removing => AiRowState::Removing,
                _ if off => AiRowState::NetworkOff,
                _ => AiRowState::Available,
            };
            AiAddon {
                id: a.id.clone(),
                info: model_info(choice, host.as_deref()),
                size: a.size,
                licence: licences.join(", "),
                state,
            }
        })
        .collect()
}

/// The add-on waiting for "Remove …?": its feature and id.
static ADDON_REMOVE: Mutex<Option<(Feature, String)>> = Mutex::new(None);

/// The feature that offers add-on `id`.
fn addon_feature(pack: &Pack, id: &str) -> Option<Feature> {
    Feature::ALL
        .into_iter()
        .find(|f| !pack.manifest().addon_models(*f, id, true).is_empty())
}

/// What a button asks of the app besides the pack's own work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Follow {
    Nothing,
    /// Download from the page: the feature's switch goes on (which installs it).
    TurnOn(Feature),
    /// For the page's worker.
    Work(Work),
    /// Keep the size in use after a size change: the setting goes back.
    Revert(Feature),
    /// A size row's button: the size setting becomes this (asked about first when it means
    /// a download, or the size may not run here).
    Use(Feature, SizeChoice),
    /// Open a licence's page.
    Open(String),
    /// Ask "Remove <add-on>?" (its name) before removing it.
    ConfirmRemoveAddon(String),
}

/// A Local AI panel's button: starts the pack's work and returns at once (nothing here
/// waits for the catalogue); what else it means comes back.
pub(crate) fn act(pack: &Pack, key: &str, e: &PanelEvent) -> Follow {
    let feature = Feature::from_id;
    match e {
        PanelEvent::Row(id) => match AiRowAction::parse(id) {
            Some(AiRowAction::Download(f)) => feature(f).map_or(Follow::Nothing, Follow::TurnOn),
            Some(AiRowAction::Cancel(f)) => {
                if let Some(f) = feature(f) {
                    pack.cancel(f);
                }
                Follow::Nothing
            }
            Some(AiRowAction::Remove(f)) => {
                if let Some(f) = feature(f) {
                    pack.remove(f);
                }
                Follow::Nothing
            }
            Some(AiRowAction::Forget(id)) => {
                pack.forget_person(id);
                Follow::Nothing
            }
            Some(AiRowAction::Switch(f, t)) => match (feature(f), Tier::from_id(t)) {
                (Some(f), Some(t)) => Follow::Work(Work::Switch(f, t)),
                _ => Follow::Nothing,
            },
            Some(AiRowAction::Keep(f)) => feature(f).map_or(Follow::Nothing, Follow::Revert),
            Some(AiRowAction::Use(f, c)) => match (feature(f), SizeChoice::from_id(c)) {
                (Some(f), Some(c)) => Follow::Use(f, c),
                _ => Follow::Nothing,
            },
            Some(AiRowAction::Licence(url)) => Follow::Open(url.to_owned()),
            Some(AiRowAction::Grouping(on)) => {
                pack.set_face_grouping(on);
                Follow::Nothing
            }
            Some(AiRowAction::AddonGet(id)) => {
                if let Some(f) = addon_feature(pack, id) {
                    pack.install_addon(f, id);
                }
                Follow::Nothing
            }
            Some(AiRowAction::AddonCancel(id)) => {
                if let Some(f) = addon_feature(pack, id) {
                    pack.cancel_addon(f, id);
                }
                Follow::Nothing
            }
            Some(AiRowAction::AddonRemove(id)) => match addon_feature(pack, id) {
                Some(f) => {
                    let name = pack
                        .manifest()
                        .choices(f)
                        .into_iter()
                        .find(|c| c.slot == specular_ai::manifest::Slot::Addon(id.to_owned()))
                        .map_or_else(|| id.to_owned(), |c| c.name);
                    *ADDON_REMOVE.lock() = Some((f, id.to_owned()));
                    Follow::ConfirmRemoveAddon(name)
                }
                None => Follow::Nothing,
            },
            Some(AiRowAction::TextIndexing(on)) => {
                pack.set_text_indexing(on);
                Follow::Nothing
            }
            None => Follow::Nothing,
        },
        PanelEvent::Action if key == ai_keys::PEOPLE => {
            pack.forget_all_faces();
            Follow::Nothing
        }
        PanelEvent::Action => {
            pack.remove_all();
            Follow::Nothing
        }
        // The AI rows have no tick boxes or arrows.
        PanelEvent::Check(_) | PanelEvent::Expand(_) => Follow::Nothing,
    }
}

/// The pack as the app makes it: the user's data folder and catalogue (if there is one).
///
/// The download transports: `file://` and HTTPS ([`https::transports`]).
fn make_pack(wake: Wake) -> Pack {
    let data = specular_settings::Locations::detect().data_dir;
    let catalogue = specular_library::default_path(&data);
    let catalogue = catalogue.is_file().then_some(catalogue);
    let pack = Pack::new(
        Manifest::load(),
        &data,
        catalogue,
        https::transports(),
        wake,
    )
    .with_model_folders(model_folders());
    if std::env::var_os("SPECULAR_AI_PLACEHOLDERS").is_some_and(|v| v == "1") {
        pack.with_placeholders()
    } else {
        pack
    }
}

/// Folders an administrator placed model files in: the policy's `model_folder`, then the
/// system one (`/etc/specular/models` on Linux).
fn model_folders() -> Vec<std::path::PathBuf> {
    super::privacy::policy()
        .model_folder
        .clone()
        .into_iter()
        .chain(specular_settings::policy::system_model_folder())
        .collect()
}

/// Opens `url` (a licence page) in the browser, on a thread.
pub(crate) fn open_url(url: String) {
    let r = specular_thread::builder("int-open-url").spawn(move || {
        #[cfg(target_os = "macos")]
        let mut cmd = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut cmd = {
            let mut c = std::process::Command::new("rundll32");
            c.arg("url.dll,FileProtocolHandler");
            c
        };
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut cmd = std::process::Command::new("xdg-open");
        specular_platform::launch::scrub_env(&mut cmd);
        if let Err(e) = cmd
            .arg(&url)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
        {
            specular_log::warn!(target: specular_log::UI, "open {url}: {e}");
        }
    });
    if let Err(e) = r {
        specular_log::warn!(target: specular_log::UI, "open-url: {e}");
    }
}

struct State {
    pages: Option<Pages>,
    opened: bool,
    at: Option<Instant>,
    i18n_gen: u64,
    /// Work for the next build.
    work: Vec<Work>,
    /// Questions waiting for an answer, and each feature's size setting before a size
    /// change asked about (Keep puts it back).
    questions: BTreeMap<Feature, Ask>,
    revert: BTreeMap<Feature, ModelSize>,
    /// The startup look has been made.
    started: bool,
    indexer: Option<index::Indexer>,
    faces: Option<faces::FaceIndexer>,
}

static STATE: Mutex<State> = Mutex::new(State {
    pages: None,
    opened: false,
    at: None,
    i18n_gen: u64::MAX,
    work: Vec::new(),
    questions: BTreeMap::new(),
    revert: BTreeMap::new(),
    started: false,
    indexer: None,
    faces: None,
});
static STALE: AtomicBool = AtomicBool::new(true);
static RECHECK: AtomicBool = AtomicBool::new(false);

/// The pack, once made (None before the first look at Local AI).
pub(crate) fn pack() -> Option<Arc<Pack>> {
    STATE.lock().pages.as_ref().and_then(|p| p.pack.clone())
}

/// The machine may suit another size now (the path selector's first calibration ended).
pub(crate) fn recheck() {
    RECHECK.store(true, Ordering::Relaxed);
}

fn push(st: &mut State, w: Work) {
    if !st.work.contains(&w) {
        st.work.push(w);
    }
}

impl App {
    /// Once per event-loop turn: the Local AI page while Settings is open, and the pack's
    /// work (setting changes, the machine looked at again). Returns when to be woken for the
    /// next rebuild (a build on the worker wakes the loop itself).
    pub(crate) fn drain_ai_page(&mut self) -> Option<Instant> {
        let screenshot = self.args.screenshot.is_some();
        index::frame_drawn(self.last_frame);
        self.ai_sideload_tick();
        let open = self.chrome.modal() == Some(Modal::Settings);
        let mut st = STATE.lock();
        if !screenshot {
            let s = &self.settings.ai;
            let any_on = s.search || s.text || s.culling || s.people;
            // An old relative size setting is migrated by a build, on or off.
            let legacy = Feature::ALL
                .iter()
                .any(|f| size_setting(&self.settings, *f).is_legacy());
            if !st.started && self.t0.elapsed() >= STARTUP && (any_on || legacy) {
                st.started = true;
                for f in Feature::ALL {
                    push(&mut st, Work::Look(f, Why::Check));
                    push(&mut st, Work::Look(f, Why::On));
                }
            }
            if RECHECK.swap(false, Ordering::Relaxed) {
                for f in Feature::ALL {
                    push(&mut st, Work::Look(f, Why::Check));
                }
            }
        }
        let pages = st.pages.get_or_insert_with(Pages::default);
        let mut out = None;
        if let Some((page, o)) = pages.take_all() {
            if self.chrome.set_ai_page(&page) {
                self.redraw = true;
            }
            out = Some(o);
        }
        let changed = pages.pack_changed();
        if changed {
            STALE.store(true, Ordering::Relaxed);
        }
        if std::mem::take(&mut pages.internal_failure) {
            // The user asked for the removal on the AI page: say it did not finish.
            let now = self.now();
            self.chrome.warning(
                t!("background-failed", reason = t!("error-reason-internal")),
                now,
            );
            self.redraw = true;
        }
        let pack = pages.pack.clone();
        if let Some(pack) = &pack {
            if st.indexer.is_none() {
                let cfg = index::Config {
                    on: self.settings.ai.search,
                    cache: self.thumb_cache(),
                    flags: self.thumb_flags(),
                    gpu: self
                        .gfx
                        .as_ref()
                        .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone())),
                };
                let w = self.loader.waker();
                st.indexer = Some(index::Indexer::start(
                    pack.clone(),
                    cfg,
                    Arc::new(move || w()),
                ));
                let w = self.loader.waker();
                text_index::start(
                    pack.clone(),
                    self.gfx
                        .as_ref()
                        .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone())),
                    Arc::new(move || w()),
                );
            }
            text_index::set_on(self.settings.ai.text);
            if let Some(ix) = &st.indexer {
                if ix.on() != self.settings.ai.search {
                    ix.set_on(self.settings.ai.search);
                }
                if changed {
                    ix.poke();
                }
            }
            let (people, culling) = (self.settings.ai.people, self.settings.ai.culling);
            if st.faces.is_none() && (people || culling) {
                let cfg = faces::Config {
                    people,
                    culling,
                    cache: self.thumb_cache(),
                    flags: self.thumb_flags(),
                    gpu: self
                        .gfx
                        .as_ref()
                        .map(|g| (g.gpu.device.clone(), g.gpu.queue.clone())),
                    registry: Some(self.loader.registry()),
                };
                let w = self.loader.waker();
                st.faces = Some(faces::FaceIndexer::start(
                    pack.clone(),
                    cfg,
                    Arc::new(move || w()),
                ));
            }
            if let Some(fx) = &st.faces {
                if fx.on() != (people, culling) {
                    fx.set_on(people, culling);
                }
                if changed {
                    fx.poke();
                }
            }
        }
        if let Some(o) = out {
            drop(st);
            self.ai_outcome(o);
            st = STATE.lock();
        }
        if !open && st.work.is_empty() {
            st.opened = false;
            return None;
        }
        let opened = open && !st.opened;
        st.opened = open;
        let now = Instant::now();
        let gen_ = I18n::global().generation();
        let due = !st.work.is_empty()
            || open
                && (opened
                    || STALE.load(Ordering::Relaxed)
                    || st.i18n_gen != gen_
                    || st.at.is_none_or(|t| now >= t));
        if due {
            if screenshot {
                // No pack in screenshots: the page without models.
                st.work.clear();
                if self.chrome.set_ai_page(&AiPage::default()) {
                    self.redraw = true;
                }
            } else {
                let mut ctx = Ctx::from_settings(&self.settings);
                ctx.adapter = self.gfx.as_ref().map(|g| g.gpu.adapter.get_info());
                ctx.work = st.work.clone();
                ctx.questions = st.questions.clone();
                let wake = self.loader.waker();
                let w = wake.clone();
                if !st
                    .pages
                    .as_mut()
                    .is_some_and(|p| p.request_with(move || make_pack(w), wake, ctx))
                {
                    // A build is running (perhaps waiting out another writer): it wakes the
                    // loop when done, and this one is due again then. No timer meanwhile,
                    // or a past one would spin the loop.
                    return None;
                }
                st.work.clear();
            }
            STALE.store(false, Ordering::Relaxed);
            st.at = Some(now + POLL);
            st.i18n_gen = gen_;
        }
        if open { st.at } else { None }
    }

    /// What a build did: questions (a suggestion gets a notice), upgraded features.
    fn ai_outcome(&mut self, o: Outcome) {
        let now = self.now();
        let mut st = STATE.lock();
        for (f, a) in o.asks {
            match a {
                Some(a) => {
                    st.questions.insert(f, a);
                    if a.suggested && self.chrome.modal() != Some(Modal::Settings) {
                        self.chrome.toast_action(
                            t!("ai-suggest-toast"),
                            t!("toolbar-settings"),
                            NOTICE_KEY,
                            now,
                        );
                    }
                }
                None => {
                    st.questions.remove(&f);
                    st.revert.remove(&f);
                }
            }
        }
        drop(st);
        if !o.migrated.is_empty() {
            self.store.update(Origin::Program, |s| {
                for (f, c) in &o.migrated {
                    // Only if still the old value (not changed meanwhile).
                    if size_setting(s, *f).is_legacy() {
                        set_size(s, *f, plan::setting(*c));
                    }
                }
            });
        }
        if !o.upgraded.is_empty() {
            self.store.update(Origin::Program, |s| {
                for f in &o.upgraded {
                    set_on(s, *f);
                }
            });
        }
    }

    /// The suggestion notice's button: Settings at the size. False when `key` is not ours.
    pub(crate) fn ai_notice(&mut self, key: u64) -> bool {
        if key != NOTICE_KEY {
            return false;
        }
        self.chrome.dismiss_undo(key);
        // The size setting of the feature the suggestion is for.
        let at = STATE
            .lock()
            .questions
            .iter()
            .find(|(_, a)| a.suggested)
            .and_then(|(f, _)| ai_keys::size_key(f.id()))
            .unwrap_or(ai_keys::SIZES[0]);
        self.chrome.open_settings_at(at);
        self.redraw = true;
        true
    }

    /// The Local AI settings changed from `old` to `new` (before they apply).
    pub(crate) fn ai_settings_changed(&mut self, old: &Settings, new: &Settings) {
        if self.args.screenshot.is_some() {
            return;
        }
        let mut st = STATE.lock();
        let (a, b) = (&old.ai, &new.ai);
        for (f, was, is) in [
            (Feature::Search, a.search, b.search),
            (Feature::Text, a.text, b.text),
            (Feature::Culling, a.culling, b.culling),
            (Feature::People, a.people, b.people),
            (Feature::Speech, a.speech, b.speech),
            (Feature::Masks, a.masks, b.masks),
            (Feature::Denoise, a.denoise, b.denoise),
            (Feature::Remove, a.remove, b.remove),
            (Feature::Depth, a.depth, b.depth),
        ] {
            if is && !was {
                push(&mut st, Work::Look(f, Why::On));
            }
        }
        for f in Feature::ALL {
            let (was, is) = (size_setting(old, f), size_setting(new, f));
            // The migration of an old value changes nothing in what runs.
            if was == is || was.is_legacy() {
                continue;
            }
            if !st.questions.contains_key(&f) {
                st.revert.insert(f, was);
            }
            push(&mut st, Work::Look(f, Why::Size));
        }
    }

    /// "Remove <add-on>?" was accepted.
    pub(crate) fn ai_addon_remove_confirmed(&mut self) {
        let Some((f, id)) = ADDON_REMOVE.lock().take() else {
            return;
        };
        if let Some(pack) = STATE.lock().pages.as_ref().and_then(|p| p.pack.clone()) {
            pack.remove_addon(f, &id);
        }
        STALE.store(true, Ordering::Relaxed);
        self.redraw = true;
    }

    /// Whether `key` is a Local AI panel's (the pack's, people's and the size panels).
    pub(crate) fn is_ai_panel(key: &str) -> bool {
        key == ai_keys::PACK || key == ai_keys::PEOPLE || ai_keys::SIZES.contains(&key)
    }

    /// The Local AI page's buttons (the pack's threads do the work).
    pub(crate) fn ai_panel_event(&mut self, key: &str, e: &PanelEvent) {
        let Some(pack) = STATE.lock().pages.as_ref().and_then(|p| p.pack.clone()) else {
            return;
        };
        match act(&pack, key, e) {
            Follow::Nothing => {}
            Follow::TurnOn(f) => {
                self.store.update(Origin::SettingsWindow, |s| set_on(s, f));
                // Also when it was on already (a failed install is tried again).
                push(&mut STATE.lock(), Work::Look(f, Why::On));
            }
            Follow::Work(w) => push(&mut STATE.lock(), w),
            Follow::Revert(f) => {
                let mut st = STATE.lock();
                let revert = st.revert.remove(&f);
                let suggested = st.questions.get(&f).is_none_or(|a| a.suggested);
                st.questions.remove(&f);
                match revert {
                    Some(size) if !suggested => {
                        drop(st);
                        self.store
                            .update(Origin::SettingsWindow, |s| set_size(s, f, size));
                    }
                    _ => push(&mut st, Work::Keep(f)),
                }
            }
            Follow::Use(f, c) => {
                let m = plan::setting(c);
                if size_setting(&self.settings, f) == m {
                    // Already the setting (a question dismissed earlier): look again.
                    push(&mut STATE.lock(), Work::Look(f, Why::Size));
                } else {
                    self.store
                        .update(Origin::SettingsWindow, |s| set_size(s, f, m));
                }
            }
            Follow::Open(url) => {
                if self.args.screenshot.is_none() {
                    open_url(url);
                }
            }
            Follow::ConfirmRemoveAddon(name) => {
                self.chrome.confirm_with_detail(
                    t!("ai-addon-remove-title", name = name),
                    t!("ai-addon-remove-detail"),
                    t!("ai-button", action = "remove"),
                    crate::app::items::AI_ADDON_REMOVE,
                );
            }
        }
        STALE.store(true, Ordering::Relaxed);
        self.redraw = true;
    }
}

fn set_on(s: &mut Settings, f: Feature) {
    match f {
        Feature::Search => s.ai.search = true,
        Feature::Text => s.ai.text = true,
        Feature::Culling => s.ai.culling = true,
        Feature::People => s.ai.people = true,
        Feature::Speech => s.ai.speech = true,
        Feature::Masks => s.ai.masks = true,
        Feature::Denoise => s.ai.denoise = true,
        Feature::Remove => s.ai.remove = true,
        Feature::Depth => s.ai.depth = true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_settings::SettingEnum;

    /// The page names the card the model runs on, and every reason it doesn't is a variant
    /// of `ai-runs-on`.
    #[test]
    fn engines_map_to_the_page() {
        assert_eq!(
            engine(RunsOn::Gpu, Some("AMD Radeon 860M Graphics")),
            AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into()))
        );
        assert_eq!(engine(RunsOn::Gpu, None), AiEngine::Gpu(None));
        let en = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/specular-i18n/i18n/en/ai.ftl"
        ))
        .unwrap();
        for why in [
            CpuWhy::NoGpu,
            CpuWhy::Differed,
            CpuWhy::Failed,
            CpuWhy::Setting,
            CpuWhy::Checking,
            CpuWhy::Chosen,
        ] {
            let AiEngine::Cpu(key) = engine(RunsOn::Cpu(why), Some("x")) else {
                panic!("{why:?}");
            };
            assert!(
                en.contains(&format!("[{key}] Runs on: CPU")),
                "{why:?}: {key}"
            );
        }
        // Nothing loaded (as in tests): no line.
        assert_eq!(index::search_runs_on(), None);
    }

    /// The size settings persist (they are ordinary settings) and map to the choices.
    #[test]
    fn size_settings_round_trip_and_map_to_choices() {
        let dir = std::env::temp_dir().join(format!("specular-aisettings-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.toml");
        let store = specular_settings::SettingsStore::open(&path).unwrap();
        for f in Feature::ALL {
            assert_eq!(
                size_setting(&store.snapshot(), f),
                ModelSize::Recommended,
                "{f}"
            );
        }
        let named = [
            ModelSize::Large,
            ModelSize::Small,
            ModelSize::Standard,
            ModelSize::Recommended,
            ModelSize::Small,
            ModelSize::Standard,
            ModelSize::Standard,
            ModelSize::Small,
            ModelSize::Small,
        ];
        store.update(Origin::SettingsWindow, |s| {
            s.ai.search = true;
            for (f, m) in Feature::ALL.into_iter().zip(named) {
                set_size(s, f, m);
            }
        });
        store.save().unwrap();
        drop(store);
        let back = specular_settings::SettingsStore::open(&path)
            .unwrap()
            .snapshot();
        assert!(back.ai.search);
        assert_eq!(back.ai.search_size, ModelSize::Large);
        assert_eq!(back.ai.text_size, ModelSize::Small);
        assert_eq!(back.ai.culling_size, ModelSize::Standard);
        assert_eq!(back.ai.people_size, ModelSize::Recommended);
        let ctx = Ctx::from_settings(&back);
        assert!(ctx.on(Feature::Search));
        assert!(!ctx.on(Feature::People));
        assert_eq!(ctx.sizes, named);
        // Every setting value names a choice by its id; the old relative values none.
        for c in SizeChoice::ALL {
            assert_eq!(plan::setting(c).as_str(), c.id());
        }
        // An old value, as an old settings file has it, loads and is left to the build.
        std::fs::write(&path, "[ai]\nsearch_size = \"larger\"\n").unwrap();
        let old = specular_settings::SettingsStore::open(&path)
            .unwrap()
            .snapshot();
        assert_eq!(old.ai.search_size, ModelSize::Larger);
        assert_eq!(plan::choice(old.ai.search_size), None);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn buttons_map_to_work() {
        let root = std::env::temp_dir().join(format!("specular-aiact-{}", std::process::id()));
        let pack = Pack::new(
            Manifest::default(),
            &root,
            None,
            Default::default(),
            Arc::new(|| {}),
        );
        let row = |id: &str| act(&pack, ai_keys::PACK, &PanelEvent::Row(id.into()));
        assert_eq!(row("download:search"), Follow::TurnOn(Feature::Search));
        assert_eq!(
            row("switch:search:large"),
            Follow::Work(Work::Switch(Feature::Search, Tier::Large))
        );
        assert_eq!(row("keep:search"), Follow::Revert(Feature::Search));
        assert_eq!(
            row("use:search:large"),
            Follow::Use(Feature::Search, SizeChoice::Tier(Tier::Large))
        );
        assert_eq!(
            row("use:people:recommended"),
            Follow::Use(Feature::People, SizeChoice::Recommended)
        );
        assert_eq!(row("use:search:larger"), Follow::Nothing);
        // The size panels are Local AI panels (their buttons reach the pack).
        for k in ai_keys::SIZES {
            assert!(App::is_ai_panel(k), "{k}");
        }
        assert_eq!(
            row("licence:https://example.invalid/MIT"),
            Follow::Open("https://example.invalid/MIT".into())
        );
        assert_eq!(row("licence:file:///etc/passwd"), Follow::Nothing);
        let _ = std::fs::remove_dir_all(&root);
    }
}
