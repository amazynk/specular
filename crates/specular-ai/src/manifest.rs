//! The model manifest: what the pack can download, for which feature, from where, how big,
//! under which licence, and what it needs to run.
//!
//! A feature is offered in up to three sizes ([`Tier`]: small, standard, large). The files of
//! one size are installed together (an image encoder with its tokenizer, say); a file several
//! sizes need lists them all. Which size suits the machine is [`crate::choose`]'s call; the
//! user only turns a feature on, and may ask for a smaller or larger size than recommended.
//!
//! The manifest is in-tree (`manifest/models.toml`, compiled in) so a build offers exactly the
//! models it was reviewed with; `SPECULAR_AI_MANIFEST` points at another file for development.
//! Entries marked `placeholder = true` show the format and are never offered to users (their
//! URLs must be on the reserved `.invalid` domain, or `file://` for tests).
//!
//! Schema 3 adds [`Choice`]s (`[[choice]]`): what the model manager shows for each model the
//! user can pick per feature (nothing downloads by default; per feature the user picks which
//! model to download and use). A choice names its
//! model, says which is the feature's default, its languages, where its training data comes
//! from and whether its terms are stated, and the host it downloads from. A choice marked
//! `coming` is shown (size, licence, data, host) but cannot be installed yet: its runtime is
//! a later lane's. A choice fills one size slot ([`Tier`]) of its feature, or is an add-on
//! (`addon`, such as a script's text recogniser) installed beside the feature's model.

use std::fmt;

/// A feature of the pack, installed and switched on its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Feature {
    /// Search by description and "more like this" (image embeddings).
    Search,
    /// Text in images (OCR).
    Text,
    /// Culling assistance: sharpness, eyes open, eye focus, expression.
    Culling,
    /// Face detection and grouping into people.
    People,
    /// Speech to text: generated subtitles for movies and sound files.
    Speech,
    /// Develop's AI masks: an object at a click (the feature's own model), the subject
    /// and the sky (add-ons).
    Masks,
    /// Develop's AI denoise: a denoising network blended with the classic result.
    Denoise,
    /// Develop's "Remove (AI)" spots: an inpainting network fills a brushed area.
    Remove,
    /// Develop's depth tools (lens blur, relight, depth masks): a monocular depth network.
    Depth,
}

impl Feature {
    pub const ALL: [Feature; 9] = [
        Feature::Search,
        Feature::Text,
        Feature::Culling,
        Feature::People,
        Feature::Speech,
        Feature::Masks,
        Feature::Denoise,
        Feature::Remove,
        Feature::Depth,
    ];

    /// Stable id: the manifest's `feature`, the library's table set, the settings key.
    pub const fn id(self) -> &'static str {
        match self {
            Feature::Search => "search",
            Feature::Text => "text",
            Feature::Culling => "culling",
            Feature::People => "people",
            Feature::Speech => "speech",
            Feature::Masks => "masks",
            Feature::Denoise => "denoise",
            Feature::Remove => "remove",
            Feature::Depth => "depth",
        }
    }

    pub fn from_id(s: &str) -> Option<Feature> {
        Feature::ALL.into_iter().find(|f| f.id() == s)
    }

    /// On once installed: the user opted in by downloading. One switch each turns it off.
    pub const fn default_on(self) -> bool {
        true
    }
}

impl fmt::Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// One size of a feature's models (DESIGN.md §10 "Local AI pack": several models per
/// feature, Specular picks one by the GPU).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Tier {
    Small,
    Standard,
    Large,
}

impl Tier {
    /// Smallest first.
    pub const ALL: [Tier; 3] = [Tier::Small, Tier::Standard, Tier::Large];

    /// The manifest's `tier`, and the settings value.
    pub const fn id(self) -> &'static str {
        match self {
            Tier::Small => "small",
            Tier::Standard => "standard",
            Tier::Large => "large",
        }
    }

    pub fn from_id(s: &str) -> Option<Tier> {
        Tier::ALL.into_iter().find(|t| t.id() == s)
    }
}

impl fmt::Display for Tier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Whether a model needs a GPU.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum GpuNeed {
    Required,
    #[default]
    Optional,
    None,
}

/// How an image model normalises its input pixels (mean and standard deviation per channel,
/// from the model's training recipe; the weights do not say).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Norm {
    /// OpenAI CLIP's mean (0.481, 0.458, 0.408) and std (0.269, 0.261, 0.276).
    #[default]
    Openai,
    /// Inception-style 0.5 and 0.5 (maps [0, 1] to [-1, 1]).
    Inception,
}

impl Norm {
    /// Mean and standard deviation per channel (R, G, B).
    pub const fn mean_std(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Norm::Openai => (
                [0.481_454_66, 0.457_827_5, 0.408_210_73],
                [0.268_629_54, 0.261_302_6, 0.275_777_1],
            ),
            Norm::Inception => ([0.5; 3], [0.5; 3]),
        }
    }
}

/// What a model needs to run.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Requirements {
    /// File format downloaded ("safetensors"; "pt", a PyTorch checkpoint, converted to
    /// safetensors; "txt" for a tokenizer's merges; "spm" for a SentencePiece model).
    pub format: String,
    /// The network family, for weights that do not describe themselves (upstream files):
    /// "clip" (an OpenCLIP or Hugging Face CLIP checkpoint with image and text towers),
    /// "siglip" (SigLIP 2), "pecore" (Meta's Perception Encoder).
    pub arch: Option<String>,
    /// Memory the loaded model needs (weights as the runtime holds them plus working
    /// buffers), on the GPU or, without one, in RAM.
    pub min_memory_mb: u32,
    pub gpu: GpuNeed,
    /// Side of the square thumbnail the model takes, in pixels (image models).
    pub input: Option<u32>,
    /// Embedding size, for models that give one.
    pub dims: Option<u32>,
    /// Input normalisation of an image model.
    pub norm: Norm,
    /// Work per photo in millions of multiply-adds (the image encoder's; a text encoder
    /// runs per query, not per photo). Against the calibration's measured time for
    /// ViT-Ti/16 at 224² ([`VIT_TI_MMACS`]) it predicts the time per photo.
    pub mmacs: Option<u32>,
    /// Converted after download (verified first), the original deleted: SigLIP 2's f32
    /// checkpoint becomes the f16 file the runtime keeps anyway.
    pub convert: Option<Convert>,
    /// The converted file's SHA-256, checked after the conversion when the manifest pins it.
    pub converted_sha256: Option<[u8; 32]>,
}

/// A conversion applied to a downloaded file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Convert {
    /// Matrices to f16 (`specular_infer::safetensors::convert_f16`).
    F16,
}

/// Multiply-adds (millions) of the inference calibration's ViT-Ti/16 at 224²: the unit
/// [`Requirements::mmacs`] is compared with.
pub const VIT_TI_MMACS: u32 = 1_255;

/// One file to download.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Model {
    pub feature: Feature,
    /// The sizes this file is part of (one, or several for a shared file), smallest first;
    /// empty for an add-on's file.
    pub tiers: Vec<Tier>,
    /// The add-on this file belongs to (`addon = "<id>"` instead of `tier`).
    pub addon: Option<String>,
    pub id: String,
    pub version: String,
    /// What the feature uses it for ("image-encoder", "detector", …).
    pub role: String,
    /// Mirrors, tried in order.
    pub urls: Vec<String>,
    pub sha256: [u8; 32],
    /// Bytes.
    pub size: u64,
    /// SPDX licence expression.
    pub licence: String,
    pub licence_url: String,
    pub placeholder: bool,
    pub runtime: Requirements,
}

impl Model {
    /// `<id>-<version>`: the install directory's name.
    pub fn key(&self) -> String {
        format!("{}-{}", self.id, self.version)
    }

    pub fn sha256_hex(&self) -> String {
        hex(&self.sha256)
    }
}

/// The models of the manifest.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub models: Vec<Model>,
    /// What the model manager offers per feature (schema 3).
    pub choices: Vec<Choice>,
}

/// Where a [`Choice`] sits in its feature.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// One of the feature's exclusive models.
    Tier(Tier),
    /// An add-on beside the feature's model (an OCR script pack): its id.
    Addon(String),
}

/// Whether the publisher states the terms of a model's training data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DataTerms {
    /// Not stated (web-scale scrapes and most published models): shown as such.
    #[default]
    Unstated,
    /// Stated by the publisher (licensed, consented or synthetic data).
    Stated,
    /// Named datasets licensed for research only (face recognisers): shown as such.
    Research,
}

impl DataTerms {
    pub const fn id(self) -> &'static str {
        match self {
            DataTerms::Unstated => "unstated",
            DataTerms::Stated => "stated",
            DataTerms::Research => "research",
        }
    }
}

/// A model the user can pick for a feature, as the model manager shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub feature: Feature,
    pub slot: Slot,
    /// The model's own name ("SigLIP 2 B/16"), shown as it is (not translated).
    pub name: String,
    /// The feature's default: what switching the feature on picks unless the user picked
    /// another (one per feature at most).
    pub default: bool,
    /// What it reads: `multilingual` or `english` (search), or a script id (OCR).
    pub languages: Option<String>,
    /// A plain hint id shown with it (`faster`, `small-faces`, …; the page words it).
    pub hint: Option<String>,
    /// The training data as the publisher names it ("WebLI"); empty when not named.
    pub data: String,
    pub data_terms: DataTerms,
    /// Shown but not installable yet.
    pub coming: bool,
    /// A coming choice's approximate download, licence and host (an available one's come
    /// from its models).
    pub size: Option<u64>,
    pub licence: Option<(String, String)>,
    pub host: Option<String>,
}

impl Choice {
    pub fn tier(&self) -> Option<Tier> {
        match self.slot {
            Slot::Tier(t) => Some(t),
            Slot::Addon(_) => None,
        }
    }
}

/// One size of a feature: the files installed together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TierOffer {
    pub tier: Tier,
    pub models: Vec<Model>,
}

impl TierOffer {
    /// Bytes to download.
    pub fn size(&self) -> u64 {
        self.models.iter().map(|m| m.size).sum()
    }

    /// Distinct licences (SPDX), in manifest order, with their URLs.
    pub fn licences(&self) -> Vec<(&str, &str)> {
        licences(&self.models)
    }

    /// Memory the size's models need when loaded, in MB.
    pub fn memory_mb(&self) -> u32 {
        self.models
            .iter()
            .map(|m| m.runtime.min_memory_mb)
            .fold(0, u32::saturating_add)
    }

    /// Work per photo, millions of multiply-adds (None: the manifest does not say).
    pub fn mmacs(&self) -> Option<u32> {
        self.models
            .iter()
            .filter_map(|m| m.runtime.mmacs)
            .reduce(u32::saturating_add)
    }

    /// Whether a model of the size must have a GPU.
    pub fn needs_gpu(&self) -> bool {
        self.models
            .iter()
            .any(|m| m.runtime.gpu == GpuNeed::Required)
    }

    /// The embedding size, for a feature that gives one.
    pub fn dims(&self) -> Option<u32> {
        self.models.iter().find_map(|m| m.runtime.dims)
    }
}

/// What Settings offers for one feature: its sizes, and the models of one of them (standard
/// unless [`Offer::with_tier`] picked another), which is what installing the offer downloads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Offer {
    pub feature: Feature,
    /// Every size the manifest has for the feature, smallest first.
    pub tiers: Vec<TierOffer>,
    /// The size `models` are.
    pub tier: Tier,
    /// The models to install: those of `tier`.
    pub models: Vec<Model>,
    /// The size the manifest names as the feature's default (a [`Choice`]), if it is offered.
    pub default: Option<Tier>,
    /// The feature's choices (sizes, add-ons and those still coming).
    pub choices: Vec<Choice>,
}

impl Offer {
    fn new(feature: Feature, tiers: Vec<TierOffer>, choices: Vec<Choice>) -> Option<Offer> {
        let default = choices
            .iter()
            .filter(|c| c.default && !c.coming)
            .find_map(Choice::tier)
            .filter(|d| tiers.iter().any(|t| t.tier == *d));
        // The default when there is one; else standard when there is one, else the nearest
        // smaller, else the smallest.
        let pick = tiers
            .iter()
            .find(|t| Some(t.tier) == default)
            .or_else(|| tiers.iter().rev().find(|t| t.tier <= Tier::Standard))
            .or(tiers.first())?;
        let (tier, models) = (pick.tier, pick.models.clone());
        Some(Offer {
            feature,
            tiers,
            tier,
            models,
            default,
            choices,
        })
    }

    /// The choice of size `tier`, if the manifest describes one.
    pub fn choice(&self, tier: Tier) -> Option<&Choice> {
        self.choices.iter().find(|c| c.slot == Slot::Tier(tier))
    }

    /// The host a size downloads from (its largest file's first mirror).
    pub fn host(&self, tier: Tier) -> Option<&str> {
        let t = self.tier_offer(tier)?;
        let m = t.models.iter().max_by_key(|m| m.size)?;
        m.urls.first().and_then(|u| host(u))
    }

    /// Bytes to download for the chosen size.
    pub fn size(&self) -> u64 {
        self.models.iter().map(|m| m.size).sum()
    }

    /// Distinct licences (SPDX) of the chosen size, in manifest order, with their URLs.
    pub fn licences(&self) -> Vec<(&str, &str)> {
        licences(&self.models)
    }

    /// Distinct licences of every size: what Settings shows before a size is chosen.
    pub fn all_licences(&self) -> Vec<(&str, &str)> {
        let mut out: Vec<(&str, &str)> = Vec::new();
        for t in &self.tiers {
            for l in t.licences() {
                if !out.iter().any(|(o, _)| *o == l.0) {
                    out.push(l);
                }
            }
        }
        out
    }

    /// The sizes on offer, smallest first.
    pub fn tier_ids(&self) -> Vec<Tier> {
        self.tiers.iter().map(|t| t.tier).collect()
    }

    pub fn tier_offer(&self, tier: Tier) -> Option<&TierOffer> {
        self.tiers.iter().find(|t| t.tier == tier)
    }

    /// The offer with `tier`'s models to install (None: the feature has no such size).
    pub fn with_tier(&self, tier: Tier) -> Option<Offer> {
        let t = self.tier_offer(tier)?;
        Some(Offer {
            feature: self.feature,
            tiers: self.tiers.clone(),
            tier,
            models: t.models.clone(),
            default: self.default,
            choices: self.choices.clone(),
        })
    }
}

fn licences(models: &[Model]) -> Vec<(&str, &str)> {
    let mut out: Vec<(&str, &str)> = Vec::new();
    for m in models {
        if !out.iter().any(|(l, _)| *l == m.licence) {
            out.push((&m.licence, &m.licence_url));
        }
    }
    out
}

/// Why a manifest was refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestError {
    /// The model entry (0-based), if the problem is in one.
    pub entry: Option<usize>,
    pub what: String,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.entry {
            Some(i) => write!(f, "model manifest, entry {}: {}", i + 1, self.what),
            None => write!(f, "model manifest: {}", self.what),
        }
    }
}

impl std::error::Error for ManifestError {}

/// The manifest compiled into this build.
pub const BUILTIN: &str = include_str!("../manifest/models.toml");

/// The schema this build writes. Schema 1 (no sizes) is still read: its models are each
/// feature's one standard size; schema 2 has no choices.
pub const SCHEMA: i64 = 3;

impl Manifest {
    /// The built-in manifest, or the file `SPECULAR_AI_MANIFEST` names. A manifest that does
    /// not parse offers nothing (and says so in the log).
    pub fn load() -> Manifest {
        let (text, from) = match std::env::var_os("SPECULAR_AI_MANIFEST") {
            Some(p) => match std::fs::read_to_string(&p) {
                Ok(t) => (t, p.to_string_lossy().into_owned()),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: {}: {e}", p.to_string_lossy());
                    return Manifest::default();
                }
            },
            None => (BUILTIN.to_owned(), "built-in".to_owned()),
        };
        match Manifest::parse(&text) {
            Ok(m) => m,
            Err(e) => {
                specular_log::warn!(target: specular_log::JOBS, "ai: {from}: {e}");
                Manifest::default()
            }
        }
    }

    pub fn parse(text: &str) -> Result<Manifest, ManifestError> {
        let top = |what: String| ManifestError { entry: None, what };
        let doc: toml_edit::DocumentMut = text.parse().map_err(|e| top(format!("{e}")))?;
        let schema = match doc.get("schema").and_then(|s| s.as_integer()) {
            Some(n @ (1 | 2 | SCHEMA)) => n,
            Some(n) => return Err(top(format!("schema {n} is not 1 to {SCHEMA}"))),
            None => return Err(top("no `schema`".into())),
        };
        for (k, _) in doc.iter() {
            if k != "schema" && k != "model" && !(k == "choice" && schema >= 3) {
                return Err(top(format!("unknown key `{k}`")));
            }
        }
        let mut models = Vec::new();
        if let Some(item) = doc.get("model") {
            let tables = item
                .as_array_of_tables()
                .ok_or_else(|| top("`model` must be [[model]] tables".into()))?;
            for (i, t) in tables.iter().enumerate() {
                let m = model(t, schema).map_err(|what| ManifestError {
                    entry: Some(i),
                    what,
                })?;
                if models.iter().any(|o: &Model| o.id == m.id) {
                    return Err(ManifestError {
                        entry: Some(i),
                        what: format!("duplicate id {:?}", m.id),
                    });
                }
                models.push(m);
            }
        }
        let mut choices: Vec<Choice> = Vec::new();
        if let Some(item) = doc.get("choice") {
            let tables = item
                .as_array_of_tables()
                .ok_or_else(|| top("`choice` must be [[choice]] tables".into()))?;
            for (i, t) in tables.iter().enumerate() {
                let err = |what: String| ManifestError {
                    entry: Some(i),
                    what: format!("choice: {what}"),
                };
                let c = choice(t).map_err(err)?;
                if choices
                    .iter()
                    .any(|o| o.feature == c.feature && o.slot == c.slot)
                {
                    return Err(err(format!("{} has two choices for one slot", c.feature)));
                }
                if c.default && choices.iter().any(|o| o.feature == c.feature && o.default) {
                    return Err(err(format!("{} has two defaults", c.feature)));
                }
                let has_models = |t: Tier| {
                    models
                        .iter()
                        .any(|m| m.feature == c.feature && m.tiers.contains(&t))
                };
                if let (false, Some(t)) = (c.coming, c.tier())
                    && !has_models(t)
                {
                    return Err(err(format!(
                        "{} {t}: no models (mark it `coming`)",
                        c.feature
                    )));
                }
                if let (false, Slot::Addon(a)) = (c.coming, &c.slot)
                    && !models
                        .iter()
                        .any(|m| m.feature == c.feature && m.addon.as_deref() == Some(a))
                {
                    return Err(err(format!(
                        "{} add-on {a}: no models (mark it `coming`)",
                        c.feature
                    )));
                }
                choices.push(c);
            }
        }
        Ok(Manifest { models, choices })
    }

    /// The choices of `feature`, in manifest order.
    pub fn choices(&self, feature: Feature) -> Vec<Choice> {
        self.choices
            .iter()
            .filter(|c| c.feature == feature)
            .cloned()
            .collect()
    }

    /// Features with nothing to install yet but choices still coming, in feature order.
    pub fn coming_only(&self, placeholders: bool) -> Vec<Feature> {
        let offered: Vec<Feature> = self
            .offers(placeholders)
            .iter()
            .map(|o| o.feature)
            .collect();
        Feature::ALL
            .into_iter()
            .filter(|f| !offered.contains(f) && self.choices.iter().any(|c| c.feature == *f))
            .collect()
    }

    /// What can be offered, per feature with models, in feature order. Placeholders only
    /// with `placeholders` (tests and development).
    pub fn offers(&self, placeholders: bool) -> Vec<Offer> {
        Feature::ALL
            .into_iter()
            .filter_map(|feature| {
                let tiers: Vec<TierOffer> = Tier::ALL
                    .into_iter()
                    .filter_map(|tier| {
                        let models: Vec<Model> = self
                            .models
                            .iter()
                            .filter(|m| {
                                m.feature == feature
                                    && m.tiers.contains(&tier)
                                    && (placeholders || !m.placeholder)
                            })
                            .cloned()
                            .collect();
                        (!models.is_empty()).then_some(TierOffer { tier, models })
                    })
                    .collect();
                Offer::new(feature, tiers, self.choices(feature))
            })
            .collect()
    }

    pub fn offer(&self, feature: Feature, placeholders: bool) -> Option<Offer> {
        self.offers(placeholders)
            .into_iter()
            .find(|o| o.feature == feature)
    }

    /// The files of `feature`'s add-on `addon` (placeholders only with `placeholders`).
    pub fn addon_models(&self, feature: Feature, addon: &str, placeholders: bool) -> Vec<Model> {
        self.models
            .iter()
            .filter(|m| {
                m.feature == feature
                    && m.addon.as_deref() == Some(addon)
                    && (placeholders || !m.placeholder)
            })
            .cloned()
            .collect()
    }

    /// The entry with this id.
    pub fn model(&self, id: &str) -> Option<&Model> {
        self.models.iter().find(|m| m.id == id)
    }
}

fn model(t: &toml_edit::Table, schema: i64) -> Result<Model, String> {
    const KEYS: &[&str] = &[
        "feature",
        "tier",
        "addon",
        "id",
        "version",
        "role",
        "urls",
        "sha256",
        "size",
        "licence",
        "licence_url",
        "placeholder",
        "runtime",
    ];
    for (k, _) in t.iter() {
        if !KEYS.contains(&k) || (k == "tier" && schema < 2) || (k == "addon" && schema < 3) {
            return Err(format!("unknown key `{k}`"));
        }
    }
    let s = |k: &str| -> Result<String, String> {
        t.get(k)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| format!("`{k}` must be a string"))
    };
    let feature = s("feature")?;
    let feature =
        Feature::from_id(&feature).ok_or_else(|| format!("unknown feature {feature:?}"))?;
    let addon = match t.get("addon") {
        None => None,
        Some(v) => match v.as_str() {
            Some(a) if is_name(a) && t.get("tier").is_none() => Some(a.to_owned()),
            _ => return Err("`addon` must be [a-z0-9._-], without `tier`".into()),
        },
    };
    let tiers = if schema < 2 {
        vec![Tier::Standard]
    } else if addon.is_some() {
        Vec::new()
    } else {
        tiers(t.get("tier"))?
    };
    let id = s("id")?;
    if !is_name(&id) {
        return Err(format!("id {id:?} must be [a-z0-9._-]"));
    }
    let version = s("version")?;
    if !is_name(&version) {
        return Err(format!("version {version:?} must be [a-z0-9._-]"));
    }
    let role = s("role")?;
    let placeholder = match t.get("placeholder") {
        None => false,
        Some(v) => v.as_bool().ok_or("`placeholder` must be true or false")?,
    };
    let urls: Vec<String> = t
        .get("urls")
        .and_then(|v| v.as_array())
        .ok_or("`urls` must be an array of strings")?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect::<Option<_>>()
        .ok_or("`urls` must be an array of strings")?;
    if urls.is_empty() {
        return Err("`urls` is empty".into());
    }
    for u in &urls {
        check_url(u, placeholder)?;
    }
    let sha = s("sha256")?;
    let sha256 = parse_hex(&sha).ok_or("`sha256` must be 64 hex digits")?;
    let size = t
        .get("size")
        .and_then(|v| v.as_integer())
        .filter(|n| *n > 0)
        .ok_or("`size` must be a positive number of bytes")? as u64;
    let licence = s("licence")?;
    if !is_spdx(&licence) {
        return Err(format!("licence {licence:?} is not an SPDX expression"));
    }
    let licence_url = s("licence_url")?;
    if !(licence_url.starts_with("https://") || placeholder && is_test_url(&licence_url)) {
        return Err(format!("licence_url {licence_url:?} must be https://"));
    }
    let r = t
        .get("runtime")
        .and_then(|v| v.as_table())
        .ok_or("no [model.runtime]")?;
    const RUNTIME: &[&str] = &[
        "format",
        "arch",
        "min_memory_mb",
        "gpu",
        "input",
        "dims",
        "norm",
        "mmacs",
        "convert",
        "converted_sha256",
    ];
    for (k, _) in r.iter() {
        if !RUNTIME.contains(&k)
            || (schema < 2 && matches!(k, "arch" | "norm" | "mmacs"))
            || (schema < 3 && matches!(k, "convert" | "converted_sha256"))
        {
            return Err(format!("unknown runtime key `{k}`"));
        }
    }
    let int = |k: &str| r.get(k).and_then(|v| v.as_integer());
    let name = |k: &str| -> Result<Option<String>, String> {
        match r.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .filter(|f| is_name(f))
                .map(|f| Some(f.to_owned()))
                .ok_or_else(|| format!("runtime `{k}` must be [a-z0-9._-]")),
        }
    };
    let format = name("format")?.ok_or("runtime `format` must be [a-z0-9._-]")?;
    let arch = name("arch")?;
    let gpu = match r.get("gpu").and_then(|v| v.as_str()).unwrap_or("optional") {
        "required" => GpuNeed::Required,
        "optional" => GpuNeed::Optional,
        "none" => GpuNeed::None,
        g => {
            return Err(format!(
                "runtime gpu {g:?} must be required, optional or none"
            ));
        }
    };
    // Schema 1 had image models only, so `input` was required.
    let input = match r.get("input") {
        None if schema >= 2 => None,
        v => Some(
            v.and_then(|v| v.as_integer())
                .filter(|n| (16..=4096).contains(n))
                .ok_or("runtime `input` must be 16 to 4096 pixels")? as u32,
        ),
    };
    let dims = match int("dims") {
        None => None,
        Some(n) if (1..=4096).contains(&n) && n % 2 == 0 => Some(n as u32),
        Some(n) => return Err(format!("runtime dims {n} must be even, 2 to 4096")),
    };
    let norm = match r.get("norm").map(|v| v.as_str()) {
        None => Norm::default(),
        Some(Some("openai")) => Norm::Openai,
        Some(Some("inception")) => Norm::Inception,
        Some(_) => return Err("runtime `norm` must be openai or inception".into()),
    };
    let mmacs = match r.get("mmacs") {
        None => None,
        Some(v) => Some(
            v.as_integer()
                .filter(|n| (1..=10_000_000).contains(n))
                .ok_or("runtime `mmacs` must be 1 to 10000000")? as u32,
        ),
    };
    let min_memory_mb = int("min_memory_mb").unwrap_or(0).clamp(0, 1 << 20) as u32;
    let convert = match r.get("convert").map(|v| v.as_str()) {
        None => None,
        Some(Some("f16")) if format == "safetensors" || format == "pt" => Some(Convert::F16),
        Some(_) => {
            return Err("runtime `convert` must be f16 (of a safetensors or pt file)".into());
        }
    };
    let converted_sha256 = match r.get("converted_sha256") {
        None => None,
        Some(v) => Some(
            v.as_str()
                .and_then(parse_hex)
                .filter(|_| convert.is_some())
                .ok_or("runtime `converted_sha256` must be 64 hex digits, with `convert`")?,
        ),
    };
    Ok(Model {
        feature,
        tiers,
        addon,
        id,
        version,
        role,
        urls,
        sha256,
        size,
        licence,
        licence_url,
        placeholder,
        runtime: Requirements {
            format,
            arch,
            min_memory_mb,
            gpu,
            input,
            dims,
            norm,
            mmacs,
            convert,
            converted_sha256,
        },
    })
}

fn choice(t: &toml_edit::Table) -> Result<Choice, String> {
    const KEYS: &[&str] = &[
        "feature",
        "tier",
        "addon",
        "name",
        "default",
        "languages",
        "hint",
        "data",
        "data_terms",
        "coming",
        "size",
        "licence",
        "licence_url",
        "host",
    ];
    for (k, _) in t.iter() {
        if !KEYS.contains(&k) {
            return Err(format!("unknown key `{k}`"));
        }
    }
    let s = |k: &str| -> Result<Option<String>, String> {
        match t.get(k) {
            None => Ok(None),
            Some(v) => v
                .as_str()
                .map(|x| Some(x.to_owned()))
                .ok_or_else(|| format!("`{k}` must be a string")),
        }
    };
    let b = |k: &str| -> Result<bool, String> {
        match t.get(k) {
            None => Ok(false),
            Some(v) => v
                .as_bool()
                .ok_or_else(|| format!("`{k}` must be true or false")),
        }
    };
    let feature = s("feature")?.ok_or("no `feature`")?;
    let feature =
        Feature::from_id(&feature).ok_or_else(|| format!("unknown feature {feature:?}"))?;
    let slot = match (s("tier")?, s("addon")?) {
        (Some(t), None) => Slot::Tier(
            Tier::from_id(&t).ok_or_else(|| format!("tier {t:?}: small, standard or large"))?,
        ),
        (None, Some(a)) if is_name(&a) => Slot::Addon(a),
        (None, Some(a)) => return Err(format!("addon {a:?} must be [a-z0-9._-]")),
        _ => return Err("exactly one of `tier` and `addon`".into()),
    };
    let name = s("name")?.unwrap_or_default();
    if name.trim().is_empty() || name.chars().count() > 64 {
        return Err("`name` must be 1 to 64 characters".into());
    }
    let id = |k: &str| -> Result<Option<String>, String> {
        match s(k)? {
            Some(v) if is_name(&v) => Ok(Some(v)),
            Some(v) => Err(format!("`{k}` {v:?} must be [a-z0-9._-]")),
            None => Ok(None),
        }
    };
    let data_terms = match s("data_terms")?.as_deref() {
        None | Some("unstated") => DataTerms::Unstated,
        Some("stated") => DataTerms::Stated,
        Some("research") => DataTerms::Research,
        Some(x) => return Err(format!("data_terms {x:?}: unstated, stated or research")),
    };
    let coming = b("coming")?;
    let size = match t.get("size") {
        None => None,
        Some(v) => Some(
            v.as_integer()
                .filter(|n| *n > 0)
                .ok_or("`size` must be a positive number of bytes")? as u64,
        ),
    };
    let licence = match (s("licence")?, s("licence_url")?) {
        (None, None) => None,
        (Some(l), Some(u)) if is_spdx(&l) && u.starts_with("https://") => Some((l, u)),
        _ => return Err("`licence` (SPDX) and `licence_url` (https://) go together".into()),
    };
    let host = id("host")?;
    if coming && (size.is_none() || licence.is_none() || host.is_none()) {
        return Err("a coming choice needs `size`, `licence`, `licence_url` and `host`".into());
    }
    if !coming && (size.is_some() || licence.is_some() || host.is_some()) {
        return Err(
            "`size`, `licence` and `host` come from the models of an available choice".into(),
        );
    }
    Ok(Choice {
        feature,
        slot,
        name,
        default: b("default")?,
        languages: id("languages")?,
        hint: id("hint")?,
        data: s("data")?.unwrap_or_default(),
        data_terms,
        coming,
        size,
        licence,
        host,
    })
}

/// `tier = "small"`, or `tier = ["small", "standard"]` for a file several sizes share.
fn tiers(v: Option<&toml_edit::Item>) -> Result<Vec<Tier>, String> {
    const MSG: &str = "`tier` must be small, standard or large, or a list of them";
    let one = |v: &toml_edit::Value| {
        v.as_str()
            .and_then(Tier::from_id)
            .ok_or_else(|| MSG.to_owned())
    };
    let v = v.and_then(|v| v.as_value()).ok_or(MSG)?;
    let mut out = match v.as_array() {
        Some(a) => a.iter().map(one).collect::<Result<Vec<Tier>, String>>()?,
        None => vec![one(v)?],
    };
    let n = out.len();
    out.sort();
    out.dedup();
    if out.is_empty() || out.len() != n {
        return Err(MSG.into());
    }
    Ok(out)
}

pub(crate) fn is_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

/// SPDX license expression, loosely: ids joined by AND / OR / WITH, optional parentheses.
fn is_spdx(s: &str) -> bool {
    let words: Vec<&str> = s
        .split(|c: char| c.is_whitespace() || c == '(' || c == ')')
        .filter(|w| !w.is_empty())
        .collect();
    !words.is_empty()
        && words.iter().enumerate().all(|(i, w)| {
            if i % 2 == 1 {
                matches!(*w, "AND" | "OR" | "WITH")
            } else {
                w.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-.+:".contains(&b))
            }
        })
        && words.len() % 2 == 1
}

/// `file://` (tests) or an `https://` URL on the reserved `.invalid` domain.
fn is_test_url(u: &str) -> bool {
    u.starts_with("file://") || host(u).is_some_and(|h| h == "invalid" || h.ends_with(".invalid"))
}

fn host(u: &str) -> Option<&str> {
    let rest = u.strip_prefix("https://")?;
    rest.split(['/', '?', '#'])
        .next()
        .and_then(|h| h.rsplit_once('@').map_or(h, |(_, h)| h).split(':').next())
}

fn check_url(u: &str, placeholder: bool) -> Result<(), String> {
    if placeholder {
        return if is_test_url(u) {
            Ok(())
        } else {
            Err(format!(
                "placeholder URL {u:?} must be file:// or on .invalid"
            ))
        };
    }
    match host(u) {
        Some(h) if !h.is_empty() && !(h == "invalid" || h.ends_with(".invalid")) => Ok(()),
        Some(_) => Err(format!(
            "URL {u:?} is a placeholder: mark the entry `placeholder = true`"
        )),
        None => Err(format!("URL {u:?} must be https://")),
    }
}

pub(crate) fn parse_hex(s: &str) -> Option<[u8; 32]> {
    let b = s.as_bytes();
    if b.len() != 64 {
        return None;
    }
    let mut out = [0u8; 32];
    for (i, o) in out.iter_mut().enumerate() {
        let d = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
        *o = d(b[2 * i])? << 4 | d(b[2 * i + 1])?;
    }
    Some(out)
}

pub(crate) fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY: &str = r#"
schema = 1

[[model]]
feature = "search"
id = "example-embed"
version = "1"
role = "image-encoder"
urls = ["https://models.invalid/example-embed-1.safetensors"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 1000
licence = "Apache-2.0"
licence_url = "https://models.invalid/LICENSE"
placeholder = true

[model.runtime]
format = "safetensors"
min_memory_mb = 256
gpu = "optional"
input = 224
dims = 512
"#;

    #[test]
    fn builtin_manifest_offers_siglip_and_the_coming_models() {
        let m = Manifest::parse(BUILTIN).unwrap();
        assert!(BUILTIN.contains(&format!("schema = {SCHEMA}")));
        let offers = m.offers(false);
        // Search, Text, Culling, People, Speech, Masks and Denoise can be installed.
        let ids: Vec<Feature> = offers.iter().map(|o| o.feature).collect();
        assert_eq!(
            ids,
            [
                Feature::Search,
                Feature::Text,
                Feature::Culling,
                Feature::People,
                Feature::Speech,
                Feature::Masks,
                Feature::Denoise,
                Feature::Remove,
                Feature::Depth
            ]
        );
        // Denoise: FFDNet colour.
        let denoise = &offers[6];
        assert_eq!(denoise.tier_ids(), [Tier::Small]);
        assert_eq!(denoise.default, Some(Tier::Small));
        // Masks: EfficientSAM Ti (the default) and S for clicks; subject and sky add-ons.
        let masks = &offers[5];
        assert_eq!(masks.tier_ids(), [Tier::Small, Tier::Standard]);
        assert_eq!(masks.default, Some(Tier::Small));
        for addon in ["subject", "sky"] {
            assert_eq!(
                m.addon_models(Feature::Masks, addon, false).len(),
                1,
                "{addon}"
            );
        }
        // Speech: Whisper base (the default), small and large-v3 turbo, each with its
        // vocabulary.
        let speech = &offers[4];
        assert_eq!(
            speech.tier_ids(),
            [Tier::Small, Tier::Standard, Tier::Large]
        );
        assert_eq!(speech.default, Some(Tier::Small));
        for t in &speech.tiers {
            let roles: Vec<&str> = t.models.iter().map(|m| m.role.as_str()).collect();
            assert!(roles.contains(&"vocabulary") && roles.contains(&"speech-model"));
            assert!(t.models.iter().all(|m| m.licence == "MIT"));
        }
        let people = &offers[3];
        assert_eq!(people.tier_ids(), [Tier::Standard, Tier::Large]);
        assert_eq!(people.default, Some(Tier::Standard));
        // The detector is shared: AuraFace's size downloads its own recogniser only.
        let det = |t| {
            people
                .tier_offer(t)
                .unwrap()
                .models
                .iter()
                .any(|m| m.id == "blazeface-full-range")
        };
        assert!(det(Tier::Standard) && det(Tier::Large));
        let o = &offers[0];
        assert_eq!(o.feature, Feature::Search);
        // B/32 (small), PE Core S16 (standard, English) and B/16 (large, the default).
        assert_eq!(o.tier_ids(), [Tier::Small, Tier::Standard, Tier::Large]);
        assert_eq!((o.default, o.tier), (Some(Tier::Large), Tier::Large));
        assert_eq!(o.choice(Tier::Large).unwrap().name, "SigLIP 2 B/16");
        let pe = o.choice(Tier::Standard).unwrap();
        assert_eq!((pe.name.as_str(), pe.coming), ("PE Core S16", false));
        for tier in [Tier::Small, Tier::Standard, Tier::Large] {
            assert_eq!(o.host(tier), Some("huggingface.co"));
        }
        for t in &o.tiers {
            // A SigLIP 2 safetensors checkpoint with the shared SentencePiece tokenizer, or
            // PE Core's PyTorch checkpoint with CLIP's merges; converted to f16 after the
            // hash check, the converted file pinned.
            let w: Vec<_> = t
                .models
                .iter()
                .filter(|m| m.runtime.arch.is_some())
                .collect();
            assert_eq!(w.len(), 1, "{}", t.tier);
            let (arch, format, side, dims, tok) = if t.tier == Tier::Standard {
                ("pecore", "pt", 384, 512, "txt")
            } else {
                ("siglip", "safetensors", 256, 768, "spm")
            };
            assert_eq!(w[0].runtime.arch.as_deref(), Some(arch));
            assert_eq!(w[0].runtime.format, format);
            assert_eq!(w[0].runtime.convert, Some(Convert::F16));
            assert!(w[0].runtime.converted_sha256.is_some(), "{}", w[0].id);
            assert_eq!(w[0].runtime.norm, Norm::Inception);
            assert_eq!(
                (w[0].runtime.input, w[0].runtime.dims),
                (Some(side), Some(dims))
            );
            assert!(t.models.iter().any(|m| m.runtime.format == tok));
            for u in t.models.iter().flat_map(|m| &m.urls) {
                // Pinned to a commit, not a branch.
                let rev = u
                    .split("/resolve/")
                    .nth(1)
                    .unwrap()
                    .split('/')
                    .next()
                    .unwrap();
                assert!(
                    rev.len() == 40 && rev.bytes().all(|b| b.is_ascii_hexdigit()),
                    "{u}"
                );
            }
            assert_eq!(
                t.licences(),
                [("Apache-2.0", t.models[0].licence_url.as_str())]
            );
        }
        let work = |t| o.tier_offer(t).unwrap().mmacs().unwrap();
        assert!(work(Tier::Small) * 3 < work(Tier::Large));
        assert!(
            work(Tier::Small) < work(Tier::Standard) && work(Tier::Standard) < work(Tier::Large)
        );
        // Every feature has a default and none is still coming.
        assert!(m.coming_only(false).is_empty());
        for f in [Feature::Text, Feature::Culling, Feature::People] {
            assert!(m.offer(f, false).is_some(), "{f}");
        }
        for f in Feature::ALL {
            let cs = m.choices(f);
            assert_eq!(cs.iter().filter(|c| c.default).count(), 1, "{f}");
            for c in cs.iter().filter(|c| c.coming) {
                assert!(
                    c.size.is_some() && c.licence.is_some() && c.host.is_some(),
                    "{}",
                    c.name
                );
            }
        }
        // OCR's script packs are add-ons, Hebrew among them; face recognisers disclose their
        // research-only training data.
        let text = m.choices(Feature::Text);
        assert!(text.iter().any(|c| c.slot == Slot::Addon("hebrew".into())));
        // Text: PP-OCRv6 small (standard, the default) and tiny (small) share the detector;
        // each recogniser is weights, config and character list; every add-on installable.
        let o = &offers[1];
        assert_eq!(o.tier_ids(), [Tier::Small, Tier::Standard]);
        assert_eq!(o.default, Some(Tier::Standard));
        for t in &o.tiers {
            let roles: Vec<&str> = t.models.iter().map(|m| m.role.as_str()).collect();
            for r in [
                "detector",
                "detector-config",
                "recognizer",
                "recognizer-config",
                "recognizer-chars",
            ] {
                assert!(roles.contains(&r), "{} lacks {r}", t.tier);
            }
            assert_eq!(t.licences().len(), 1);
        }
        let det = m.model("ppocrv6-small-det").unwrap();
        assert_eq!(det.tiers, [Tier::Small, Tier::Standard]);
        for c in text.iter().filter(|c| matches!(c.slot, Slot::Addon(_))) {
            let Slot::Addon(id) = &c.slot else {
                unreachable!()
            };
            assert!(!c.coming, "{id}");
            let files = m.addon_models(Feature::Text, id, false);
            let want = if id == "hebrew" { 1 } else { 3 };
            assert_eq!(files.len(), want, "{id}");
            assert!(
                files
                    .iter()
                    .all(|f| f.tiers.is_empty() && f.licence == "Apache-2.0")
            );
        }
        assert_eq!(
            text.iter()
                .filter(|c| matches!(c.slot, Slot::Addon(_)))
                .count(),
            11
        );
        let people = m.choices(Feature::People);
        assert_eq!(
            people.iter().find(|c| c.default).unwrap().data_terms,
            DataTerms::Research
        );
    }

    #[test]
    fn choices_are_checked() {
        let base = TIERED.replace("schema = 2", "schema = 3");
        let ok = format!(
            "{base}\n[[choice]]\nfeature = \"search\"\ntier = \"large\"\nname = \"Big\"\ndefault = true\n"
        );
        let m = Manifest::parse(&ok).unwrap();
        let o = m.offer(Feature::Search, true).unwrap();
        assert_eq!((o.default, o.tier), (Some(Tier::Large), Tier::Large));
        // Schema 2 has no choices.
        assert!(Manifest::parse(&ok.replace("schema = 3", "schema = 2")).is_err());
        for (bad, why) in [
            ("tier = \"standard\"\nname = \"X\"\n", "no models"),
            (
                "tier = \"large\"\nname = \"X\"\n",
                "two choices for one slot",
            ),
            (
                "tier = \"small\"\nname = \"X\"\ndefault = true\n",
                "two defaults",
            ),
            (
                "addon = \"x\"\ntier = \"small\"\nname = \"X\"\n",
                "exactly one",
            ),
            (
                "tier = \"standard\"\nname = \"X\"\ncoming = true\n",
                "coming choice needs",
            ),
            ("tier = \"small\"\nname = \"\"\n", "name"),
            (
                "tier = \"small\"\nname = \"X\"\nsize = 5\n",
                "come from the models",
            ),
            (
                "tier = \"small\"\nname = \"X\"\ndata_terms = \"maybe\"\n",
                "data_terms",
            ),
            ("tier = \"small\"\nname = \"X\"\nwhat = 1\n", "unknown key"),
        ] {
            let text = format!("{ok}\n[[choice]]\nfeature = \"search\"\n{bad}");
            let e = Manifest::parse(&text).unwrap_err().to_string();
            assert!(e.contains(why), "{bad:?}: {e}");
        }
        let coming = format!(
            "{ok}\n[[choice]]\nfeature = \"people\"\ntier = \"standard\"\nname = \"Faces\"\ncoming = true\nsize = 9\nlicence = \"MIT\"\nlicence_url = \"https://x.invalid/l\"\nhost = \"example.org\"\n"
        );
        let m = Manifest::parse(&coming).unwrap();
        assert_eq!(m.coming_only(true), [Feature::People]);
        assert!(m.offer(Feature::People, true).is_none());
    }

    const TIERED: &str = r#"
schema = 2

[[model]]
feature = "search"
tier = "small"
id = "tiny"
version = "1"
role = "image-text-encoder"
urls = ["https://models.invalid/tiny.safetensors"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 300
licence = "MIT"
licence_url = "https://models.invalid/LICENSE"
placeholder = true

[model.runtime]
format = "safetensors"
arch = "clip"
min_memory_mb = 400
input = 224
dims = 512
mmacs = 2000

[[model]]
feature = "search"
tier = ["small", "large"]
id = "merges"
version = "1"
role = "tokenizer"
urls = ["https://models.invalid/merges.txt"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 5
licence = "Apache-2.0"
licence_url = "https://models.invalid/LICENSE2"
placeholder = true

[model.runtime]
format = "txt"
min_memory_mb = 10
gpu = "none"

[[model]]
feature = "search"
tier = "large"
id = "big"
version = "1"
role = "image-text-encoder"
urls = ["https://models.invalid/big.safetensors"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 2000
licence = "MIT"
licence_url = "https://models.invalid/LICENSE"
placeholder = true

[model.runtime]
format = "safetensors"
arch = "clip"
min_memory_mb = 1400
gpu = "required"
input = 224
dims = 768
norm = "inception"
mmacs = 80000
"#;

    #[test]
    fn tiers_group_models() {
        let m = Manifest::parse(TIERED).unwrap();
        assert_eq!(m.models[1].tiers, vec![Tier::Small, Tier::Large]);
        assert_eq!(m.model("big").unwrap().runtime.norm, Norm::Inception);
        assert_eq!(m.model("tiny").unwrap().runtime.norm, Norm::Openai);
        assert_eq!(m.model("merges").unwrap().runtime.input, None);
        let o = m.offer(Feature::Search, true).unwrap();
        assert_eq!(o.tier_ids(), vec![Tier::Small, Tier::Large]);
        // No standard size: the nearest smaller one is the default.
        assert_eq!(o.tier, Tier::Small);
        assert_eq!(o.size(), 305);
        let small = o.tier_offer(Tier::Small).unwrap();
        assert_eq!(small.memory_mb(), 410);
        assert_eq!(small.mmacs(), Some(2000));
        assert_eq!(small.dims(), Some(512));
        assert!(!small.needs_gpu());
        let large = o.with_tier(Tier::Large).unwrap();
        assert_eq!(large.tier, Tier::Large);
        assert_eq!(
            large
                .models
                .iter()
                .map(|m| m.id.as_str())
                .collect::<Vec<_>>(),
            ["merges", "big"]
        );
        assert_eq!(large.size(), 2005);
        assert!(large.tier_offer(Tier::Large).unwrap().needs_gpu());
        assert_eq!(
            large.licences(),
            vec![
                ("Apache-2.0", "https://models.invalid/LICENSE2"),
                ("MIT", "https://models.invalid/LICENSE")
            ]
        );
        assert_eq!(o.all_licences().len(), 2);
        assert!(o.with_tier(Tier::Standard).is_none());
        assert!(m.offers(false).is_empty());
    }

    #[test]
    fn refuses_bad_tiers() {
        let bad = |from: &str, to: &str| {
            let t = TIERED.replacen(from, to, 1);
            Manifest::parse(&t).expect_err(to)
        };
        bad("tier = \"small\"\n", "tier = \"medium\"\n");
        bad("tier = \"small\"\n", "");
        bad("tier = [\"small\", \"large\"]", "tier = []");
        bad(
            "tier = [\"small\", \"large\"]",
            "tier = [\"small\", \"small\"]",
        );
        bad("tier = [\"small\", \"large\"]", "tier = 3");
        bad("norm = \"inception\"", "norm = \"imagenet\"");
        bad("mmacs = 2000", "mmacs = 0");
        bad("arch = \"clip\"", "arch = \"CLIP\"");
        bad("min_memory_mb = 10", "min_memroy_mb = 10");
    }

    #[test]
    fn schema_1_is_one_standard_size() {
        let m = Manifest::parse(ENTRY).unwrap();
        assert_eq!(m.models[0].tiers, vec![Tier::Standard]);
        assert_eq!(m.models[0].runtime.input, Some(224));
        let o = m.offer(Feature::Search, true).unwrap();
        assert_eq!(o.tier, Tier::Standard);
        assert_eq!(o.tiers.len(), 1);
        // Schema 1 knew no sizes nor the newer runtime keys, and needed `input`.
        let t = ENTRY.replace(
            "feature = \"search\"",
            "feature = \"search\"\ntier = \"small\"",
        );
        Manifest::parse(&t).unwrap_err();
        let t = ENTRY.replace("dims = 512", "dims = 512\nmmacs = 5");
        Manifest::parse(&t).unwrap_err();
        Manifest::parse(&ENTRY.replace("input = 224\n", "")).unwrap_err();
    }

    #[test]
    fn placeholder_entry_parses_and_is_not_offered() {
        let m = Manifest::parse(ENTRY).unwrap();
        assert_eq!(m.models.len(), 1);
        let e = &m.models[0];
        assert_eq!(e.feature, Feature::Search);
        assert_eq!(e.key(), "example-embed-1");
        assert_eq!(e.runtime.dims, Some(512));
        assert_eq!(e.runtime.gpu, GpuNeed::Optional);
        assert!(e.placeholder);
        assert!(m.offers(false).is_empty());
        let o = m.offers(true);
        assert_eq!(o.len(), 1);
        assert_eq!(o[0].size(), 1000);
        assert_eq!(
            o[0].licences(),
            vec![("Apache-2.0", "https://models.invalid/LICENSE")]
        );
    }

    #[test]
    fn refuses_bad_entries() {
        let bad = |from: &str, to: &str| {
            let t = ENTRY.replace(from, to);
            Manifest::parse(&t).expect_err(to)
        };
        bad("schema = 1", "schema = 3");
        // Schema 2 needs sizes.
        bad("schema = 1", "schema = 2");
        bad("\"search\"", "\"weather\"");
        bad("example-embed\"", "Example Embed\"");
        bad(
            "000000000000000000000000000000000000000000000000000000000000\"",
            "00\"",
        );
        bad("size = 1000", "size = 0");
        bad("Apache-2.0", "Apache 2.0 or so");
        bad("dims = 512", "dims = 511");
        bad("input = 224", "input = 1");
        bad("gpu = \"optional\"", "gpu = \"maybe\"");
        bad("format = \"safetensors\"", "fromat = \"safetensors\"");
        // A placeholder on a real-looking host.
        bad(
            "https://models.invalid/example",
            "https://example.com/example",
        );
        // A real entry on the placeholder domain.
        bad("placeholder = true", "placeholder = false");
        // Plain HTTP.
        let t = ENTRY
            .replace("placeholder = true", "")
            .replace("https://models.invalid/example", "http://example.com/x");
        Manifest::parse(&t).unwrap_err();
        // Duplicate ids.
        let dup = format!("{ENTRY}{}", &ENTRY[ENTRY.find("[[model]]").unwrap()..]);
        assert!(
            Manifest::parse(&dup)
                .unwrap_err()
                .what
                .contains("duplicate")
        );
    }

    #[test]
    fn real_entry_is_offered() {
        let t = ENTRY
            .replace("placeholder = true", "")
            .replace("models.invalid", "models.example.org");
        let m = Manifest::parse(&t).unwrap();
        assert_eq!(m.offers(false).len(), 1);
    }

    #[test]
    fn spdx_expressions() {
        for ok in [
            "MIT",
            "Apache-2.0",
            "MIT OR Apache-2.0",
            "(MIT AND BSD-3-Clause)",
            "GPL-2.0-only WITH Classpath-exception-2.0",
            "LicenseRef-Custom",
        ] {
            assert!(is_spdx(ok), "{ok}");
        }
        for bad in ["", "MIT OR", "MIT Apache", "some licence"] {
            assert!(!is_spdx(bad), "{bad}");
        }
    }

    #[test]
    fn hex_round_trip() {
        let h = "00ff10aB".repeat(8);
        let b = parse_hex(&h).unwrap();
        assert_eq!(hex(&b), h.to_lowercase());
        assert!(parse_hex("zz").is_none());
    }
}
