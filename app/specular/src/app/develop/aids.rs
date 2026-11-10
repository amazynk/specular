//! Darkroom aids: scopes, clipping overlays and soft-proofing.
//!
//! - **Scopes** dock at the top of the panel ([`ScopeDock`]): histogram, waveform (luma or
//!   RGB), RGB parade, vectorscope. The engine bins the picture on the GPU once per render
//!   (`specular_develop::scopes`); the bins come back without stalling and are drawn here at
//!   the dock's size. Shift+G steps through them (and hides them).
//! - **Clipping** (J display, Shift+J scene, Alt+J gamut; the Clipping card; the picture's
//!   right-click menu): a coloured overlay over the picture, never in exports.
//! - **Soft proof** (Shift+P; the Soft proof card): a printer profile from the system's ICC
//!   folders or a file, the intent, black point compensation, paper white and ink black.
//!   The chain is baked in the background and applied on the GPU; the gamut warning then
//!   uses the proof profile. None of it is part of the edit or its history.

use std::path::PathBuf;
use std::sync::Arc;

use crossbeam_channel::Receiver;
use specular_develop::clip::ClipFlags;
use specular_develop::proof::{FoundProfile, ProofIntent, ProofLut, ProofSettings, Proofer};
use specular_develop::scopes::{self, Bins, Targets};
use specular_gfx::{Gpu, Icon, Rect, TexId, UiRenderer, wgpu};
use specular_i18n::t;
use specular_input::Action;
use specular_widgets::PaletteItem;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control, DevelopPanel, Event};
use specular_widgets::scopes::{Marks, Press, Scope, ScopeDock};

use super::super::App;
use super::super::items;

/// The Soft proof card (a switch: proofing on) and the Clipping card (no switch). Their low
/// 16 bits are no module's.
pub(super) const PROOF_CARD: u32 = 0x5_FFF0;
pub(super) const CLIP_CARD: u32 = 0x6_FFF0;

/// The picture menu's row of the ghost overlay (after the aids' five, before Print's 8).
const GHOSTS_ROW: u64 = 6;
/// The scope picker's menu: the scopes from this row (in [`Scope::ALL`]'s order), then "Hide
/// scopes" (the 16 ids end at 15).
const SCOPE_ROWS: u64 = 10;
const HIDE_SCOPES_ROW: u64 = SCOPE_ROWS + Scope::ALL.len() as u64;

/// Control ids of the two cards.
pub(super) mod id {
    pub const PROOF_PROFILE: u32 = 190;
    pub const PROOF_FILE: u32 = 191;
    pub const PROOF_INTENT: u32 = 192;
    pub const PROOF_BPC: u32 = 193;
    pub const PROOF_PAPER: u32 = 194;
    pub const PROOF_INK: u32 = 195;
    pub const CLIP_SCENE: u32 = 196;
    pub const CLIP_DISPLAY: u32 = 197;
    pub const CLIP_GAMUT: u32 = 198;
    pub const CLIP_GHOSTS: u32 = 199;
}

/// Whether `e` is one of this module's.
pub(super) fn is_ours(e: &Event) -> bool {
    match *e {
        Event::Value { control, .. } => (id::PROOF_PROFILE..=id::CLIP_GHOSTS).contains(&control),
        Event::Enabled { card, .. } => card == PROOF_CARD,
        _ => false,
    }
}

/// The chosen proof profile.
#[derive(Clone, Debug)]
pub(in crate::app) struct Chosen {
    pub path: PathBuf,
    pub name: String,
    pub icc: Arc<Vec<u8>>,
}

/// What a bake sends back: the settings' key, the table and the profile's marks, or why not.
type Baked = (u64, Result<(Arc<ProofLut>, Option<Targets>), String>);

/// Soft-proof state.
#[derive(Default)]
pub(in crate::app) struct Proof {
    pub on: bool,
    pub settings: ProofSettings,
    pub chosen: Option<Chosen>,
    /// Profiles in the system's ICC folders (`None` until scanned).
    pub found: Option<Vec<FoundProfile>>,
    scan: Option<Receiver<Vec<FoundProfile>>>,
    pub lut: Option<Arc<ProofLut>>,
    pub targets: Option<Targets>,
    baking: Option<(u64, Receiver<Baked>)>,
    /// The key the table was asked for.
    want: Option<u64>,
    file: Option<Receiver<PathBuf>>,
    /// The remembered profile that was missing when Develop opened: none is picked
    /// automatically, and it is not reported again.
    missing: Option<PathBuf>,
}

/// The scope picture: texture, id, size, and what it shows (scope, RGB, bins).
/// The texture, its id and size, and what it shows: the scope, the waveform's RGB, the
/// histogram's log scale and the bins' generation.
type ScopeTex = (wgpu::Texture, TexId, (u32, u32), (Scope, bool, bool, u64));

/// Everything here; see the module docs.
#[derive(Default)]
pub(in crate::app) struct Aids {
    pub dock: ScopeDock,
    pub clip: ClipFlags,
    pub proof: Proof,
    /// The newest bins and their generation.
    bins: Option<Bins>,
    bins_gen: u64,
    /// The scope picture.
    tex: Option<ScopeTex>,
    retired: Vec<TexId>,
    started: bool,
    /// A bracket merge's kept ghost mask, when the open photo has one (`ghosts.rs`).
    pub ghosts: Option<super::ghosts::Ghosts>,
    ghost_retired: Vec<TexId>,
}

impl Aids {
    /// The engine's part for the next render: scopes on, clipping, the proof table.
    pub fn request(&self) -> super::preview::AidsReq {
        super::preview::AidsReq {
            scopes: self.dock.shown,
            clip: self.clip,
            proof: if self.proof.on {
                self.proof.lut.clone()
            } else {
                None
            },
        }
    }

    /// New bins from the engine.
    pub fn set_bins(&mut self, b: Bins) {
        self.bins = Some(b);
        self.bins_gen += 1;
    }

    /// The vectorscope's marks.
    pub fn marks(&self) -> Marks {
        Marks {
            gamut: scopes::targets().ab,
            proof: self
                .proof
                .targets
                .filter(|_| self.proof.on && self.proof.lut.is_some())
                .map(|t| t.ab),
            skin: scopes::skin_hue(),
            range: scopes::VEC_RANGE,
        }
    }

    /// The scope picture at `size` physical px, made again when the bins, the scope or the
    /// size changed.
    pub fn scope_texture(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        size: (u32, u32),
    ) -> Option<TexId> {
        for id in self.retired.drain(..) {
            ui.unregister_texture(id);
        }
        let b = self.bins.as_ref()?;
        let scope = self.dock.scope;
        let what = (scope, self.dock.rgb, self.dock.log, self.bins_gen);
        let size = if scope == Scope::Vectorscope {
            (size.1, size.1)
        } else {
            size
        };
        let size = (size.0.clamp(16, 2048), size.1.clamp(16, 2048));
        if let Some((_, id, s, w)) = &self.tex
            && *s == size
            && *w == what
        {
            return Some(*id);
        }
        let rgba = match scope {
            Scope::Histogram => scopes::raster::histogram(b, size.0, size.1, self.dock.log),
            Scope::Waveform => scopes::raster::waveform(b, size.0, size.1, self.dock.rgb),
            Scope::Parade => scopes::raster::parade(b, size.0, size.1),
            Scope::Vectorscope => scopes::raster::vectorscope(b, size.0),
        };
        let fits = self
            .tex
            .as_ref()
            .is_some_and(|t| t.0.width() == size.0 && t.0.height() == size.1);
        if !fits {
            let (tex, id) = super::preview::upload(gpu, ui, size.0, size.1, &rgba);
            if let Some(old) = self.tex.replace((tex, id, size, what)) {
                self.retired.push(old.1);
            }
        } else if let Some(t) = self.tex.as_mut() {
            gpu.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &t.0,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * size.0),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: size.0,
                    height: size.1,
                    depth_or_array_layers: 1,
                },
            );
            t.3 = what;
        }
        self.tex.as_ref().map(|t| t.1)
    }

    /// The open photo's kept ghost mask (or none); its texture goes.
    pub fn set_ghosts(&mut self, g: Option<super::ghosts::Ghosts>) {
        if let Some(id) = self.ghosts.as_mut().and_then(|g| g.take_texture()) {
            self.ghost_retired.push(id);
        }
        self.ghosts = g;
    }

    /// The ghost overlay for the picture drawn through `plan` at `size` physical px, while
    /// it is on; frees replaced textures.
    pub fn ghost_texture(
        &mut self,
        gpu: &Gpu,
        ui: &mut UiRenderer,
        plan: Option<&specular_develop::ops::geometry::Plan>,
        size: (u32, u32),
    ) -> Option<TexId> {
        for id in self.ghost_retired.drain(..) {
            ui.unregister_texture(id);
        }
        let g = self.ghosts.as_mut().filter(|g| g.on)?;
        g.texture(gpu, ui, plan?, size, &mut self.ghost_retired)
    }

    /// The proof settings' key with the chosen profile.
    fn key(&self) -> Option<u64> {
        let c = self.proof.chosen.as_ref()?;
        let s = self.proof.settings;
        let mut k = specular_color::icc::hash_bytes(&c.icc);
        for b in [s.intent == ProofIntent::Relative, s.bpc, s.paper, s.ink] {
            k = k.rotate_left(9) ^ u64::from(b);
        }
        Some(k)
    }
}

/// The two cards.
fn cards(a: &Aids, expanded: &[(u32, bool)]) -> Vec<Card> {
    let open = |id: u32, d: bool| expanded.iter().find(|e| e.0 == id).map_or(d, |e| e.1);
    let choice = |id: u32, label: String, options: Vec<String>, at: usize| {
        let mut c = Choice::new(label, options);
        c.set(at);
        Control {
            id,
            body: Body::Choice(c),
        }
    };
    let off_on = || vec![t!("develop-off"), t!("develop-on")];
    let p = &a.proof;
    let names = profile_names(p);
    let at = p
        .chosen
        .as_ref()
        .and_then(|c| profile_list(p).iter().position(|f| f.path == c.path))
        .unwrap_or(0);
    let clip = Card {
        id: CLIP_CARD,
        title: t!("develop-clipping"),
        enabled: None,
        expanded: open(CLIP_CARD, false),
        controls: vec![
            choice(
                id::CLIP_DISPLAY,
                t!("develop-clip-display"),
                off_on(),
                usize::from(a.clip.display),
            ),
            choice(
                id::CLIP_SCENE,
                t!("develop-clip-scene"),
                off_on(),
                usize::from(a.clip.scene),
            ),
            choice(
                id::CLIP_GAMUT,
                t!("develop-clip-gamut"),
                off_on(),
                usize::from(a.clip.gamut),
            ),
        ]
        .into_iter()
        .chain(a.ghosts.as_ref().map(|g| {
            choice(
                id::CLIP_GHOSTS,
                t!("develop-merge-show-ghosts"),
                off_on(),
                usize::from(g.on),
            )
        }))
        .collect(),
    };
    let s = p.settings;
    let proof = Card {
        id: PROOF_CARD,
        title: t!("develop-proof"),
        enabled: Some(p.on),
        expanded: open(PROOF_CARD, false),
        controls: vec![
            choice(id::PROOF_PROFILE, t!("develop-proof-profile"), names, at),
            Control {
                id: id::PROOF_FILE,
                body: Body::Choice(Choice::button(t!("develop-proof-file"))),
            },
            choice(
                id::PROOF_INTENT,
                t!("develop-proof-intent"),
                vec![t!("develop-proof-perceptual"), t!("develop-proof-relative")],
                usize::from(s.intent == ProofIntent::Relative),
            ),
            choice(
                id::PROOF_BPC,
                t!("develop-proof-bpc"),
                off_on(),
                usize::from(s.bpc),
            ),
            choice(
                id::PROOF_PAPER,
                t!("develop-proof-paper"),
                off_on(),
                usize::from(s.paper),
            ),
            choice(
                id::PROOF_INK,
                t!("develop-proof-ink"),
                off_on(),
                usize::from(s.ink),
            ),
        ],
    };
    vec![clip, proof]
}

/// The proof settings kept in the user's settings.
fn remembered_settings(c: &specular_settings::Colour) -> ProofSettings {
    ProofSettings {
        intent: match c.proof_intent {
            specular_settings::ProofIntent::Perceptual => ProofIntent::Perceptual,
            specular_settings::ProofIntent::Relative => ProofIntent::Relative,
        },
        bpc: c.proof_bpc,
        paper: c.proof_paper,
        ink: c.proof_ink,
    }
}

/// Writes `s` into the user's settings.
fn store_settings(c: &mut specular_settings::Colour, s: ProofSettings) {
    c.proof_intent = match s.intent {
        ProofIntent::Perceptual => specular_settings::ProofIntent::Perceptual,
        ProofIntent::Relative => specular_settings::ProofIntent::Relative,
    };
    c.proof_bpc = s.bpc;
    c.proof_paper = s.paper;
    c.proof_ink = s.ink;
}

/// A remembered profile file, read and checked: `Err` when it is gone, unreadable or no
/// printer profile.
fn read_profile(path: &std::path::Path) -> Result<Chosen, ()> {
    let icc = std::fs::read(path).map_err(|_| ())?;
    Proofer::new(&icc, ProofSettings::default()).map_err(|_| ())?;
    let name = specular_color::icc::description(&icc).unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Ok(Chosen {
        path: path.to_owned(),
        name,
        icc: Arc::new(icc),
    })
}

/// Brings `p` in line with the remembered proof settings and profile in `c` (at every
/// Develop open). A profile that changed is read with `read`; one that is gone or unusable
/// leaves nothing chosen and proofing off, and is returned the first time so Develop can
/// say so. No remembered profile keeps what was picked this run.
fn sync_proof(
    p: &mut Proof,
    c: &specular_settings::Colour,
    read: impl FnOnce(&std::path::Path) -> Result<Chosen, ()>,
) -> Option<PathBuf> {
    p.settings = remembered_settings(c);
    let path = c.proof_profile.as_ref()?;
    if p.chosen.as_ref().is_some_and(|ch| &ch.path == path) {
        return None;
    }
    match read(path) {
        Ok(ch) => {
            p.chosen = Some(ch);
            p.missing = None;
            None
        }
        Err(()) => {
            // Gone or no longer usable: proofing stays off, nothing picked for it.
            p.chosen = None;
            p.on = false;
            p.lut = None;
            if p.missing.as_ref() == Some(path) {
                return None;
            }
            p.missing = Some(path.clone());
            Some(path.clone())
        }
    }
}

/// The profiles the Profile choice offers: those found, then a file chosen elsewhere.
fn profile_list(p: &Proof) -> Vec<FoundProfile> {
    let mut v = p.found.clone().unwrap_or_default();
    if let Some(c) = &p.chosen
        && !v.iter().any(|f| f.path == c.path)
    {
        v.push(FoundProfile {
            path: c.path.clone(),
            name: c.name.clone(),
            printer: true,
        });
    }
    v
}

fn profile_names(p: &Proof) -> Vec<String> {
    let v = profile_list(p);
    if v.is_empty() {
        return vec![if p.found.is_none() {
            t!("develop-proof-looking")
        } else {
            t!("develop-proof-none")
        }];
    }
    v.into_iter().map(|f| f.name).collect()
}

/// Puts the two cards in the panel before the Styles, History and Versions cards (or at the
/// end), keeping whether each is open.
pub(super) fn place_cards(panel: &mut DevelopPanel, a: &Aids) {
    let ours = [CLIP_CARD, PROOF_CARD];
    let expanded: Vec<(u32, bool)> = panel
        .cards
        .iter()
        .filter(|c| ours.contains(&c.id))
        .map(|c| (c.id, c.expanded))
        .collect();
    let focus = panel.focus();
    panel.cards.retain(|c| !ours.contains(&c.id));
    let at = panel
        .cards
        .iter()
        .position(|c| (super::batch::STYLES_CARD..=super::batch::VERSIONS_CARD).contains(&c.id))
        .unwrap_or(panel.cards.len());
    for (k, c) in cards(a, &expanded).into_iter().enumerate() {
        panel.cards.insert(at + k, c);
    }
    panel.set_focus(focus);
}

/// The picture's right-click menu rows: (label, on, key action).
fn menu_rows(a: &Aids) -> [(String, bool, Action); 5] {
    [
        (
            t!("develop-clip-display"),
            a.clip.display,
            Action::DevelopClipDisplay,
        ),
        (
            t!("develop-clip-scene"),
            a.clip.scene,
            Action::DevelopClipScene,
        ),
        (
            t!("develop-clip-gamut"),
            a.clip.gamut,
            Action::DevelopClipGamut,
        ),
        (t!("develop-proof"), a.proof.on, Action::DevelopSoftProof),
        (t!("develop-scopes"), a.dock.shown, Action::DevelopScopes),
    ]
}

impl App {
    /// Applies the remembered proof settings and profile at every Develop open (they may
    /// have changed in the Settings window or the settings file since); starts the profile
    /// scan the first time.
    pub(super) fn develop_aids_start(&mut self) {
        // The proof settings and profile last used (per user, `colour.proof_*`).
        if let Some(path) = sync_proof(
            &mut self.develop.aids.proof,
            &self.settings.colour,
            read_profile,
        ) {
            let name = path.file_name().map_or_else(
                || path.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            );
            let now = self.now();
            self.chrome
                .warning(t!("develop-proof-missing", name = name), now);
        }
        let a = &mut self.develop.aids;
        if a.started {
            return;
        }
        a.started = true;
        a.dock = ScopeDock::new();
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("develop-icc", move |l| {
            let _ = tx.send(specular_develop::proof::system_profiles());
            l.waker()();
        });
        self.develop.aids.proof.scan = Some(rx);
    }

    /// Keeps the proof settings and profile in the user's settings (`colour.proof_*`) when
    /// they changed: they come back next time, never as part of an edit.
    fn develop_proof_remember(&mut self) {
        let p = &self.develop.aids.proof;
        let path = p.chosen.as_ref().map(|c| c.path.clone());
        let s = p.settings;
        let c = &self.settings.colour;
        if remembered_settings(c) == s && (path.is_none() || c.proof_profile == path) {
            return;
        }
        self.store.update(specular_settings::Origin::Program, |st| {
            store_settings(&mut st.colour, s);
            if path.is_some() {
                st.colour.proof_profile = path;
            }
        });
        self.apply_settings(false);
    }

    /// Background results: the profile scan, a chosen file, a baked table. Asks for a new
    /// table when the settings changed.
    pub(super) fn develop_aids_tick(&mut self) {
        let mut cards = false;
        let p = &mut self.develop.aids.proof;
        if let Some(rx) = &p.scan
            && let Ok(found) = rx.try_recv()
        {
            p.scan = None;
            // Nothing chosen yet: the first printer profile.
            if p.chosen.is_none()
                && p.missing.is_none()
                && let Some(f) = found.iter().find(|f| f.printer)
                && let Ok(icc) = std::fs::read(&f.path)
            {
                p.chosen = Some(Chosen {
                    path: f.path.clone(),
                    name: f.name.clone(),
                    icc: Arc::new(icc),
                });
            }
            p.found = Some(found);
            cards = true;
        }
        if let Some(rx) = &p.file
            && let Ok(path) = rx.try_recv()
        {
            p.file = None;
            self.develop_proof_file(path);
            cards = true;
        }
        let a = &mut self.develop.aids;
        if let Some((k, rx)) = &a.proof.baking
            && let Ok((key, r)) = rx.try_recv()
        {
            let k = *k;
            a.proof.baking = None;
            if key == k && a.proof.want == Some(k) {
                match r {
                    Ok((lut, t)) => {
                        a.proof.lut = Some(lut);
                        a.proof.targets = t;
                    }
                    Err(e) => {
                        a.proof.lut = None;
                        a.proof.on = false;
                        let now = self.now();
                        self.chrome
                            .warning(t!("develop-proof-failed", reason = e), now);
                        cards = true;
                    }
                }
                self.redraw = true;
            }
        }
        self.develop_proof_bake();
        if cards {
            self.develop_aids_cards();
        }
    }

    /// Bakes the proof table for the current settings when it is not there or on its way.
    fn develop_proof_bake(&mut self) {
        let a = &mut self.develop.aids;
        if !a.proof.on {
            return;
        }
        let Some(key) = a.key() else { return };
        if a.proof.want == Some(key) && (a.proof.lut.is_some() || a.proof.baking.is_some()) {
            return;
        }
        if a.proof.baking.as_ref().is_some_and(|b| b.0 == key) {
            return;
        }
        a.proof.want = Some(key);
        a.proof.lut = None;
        let Some(c) = a.proof.chosen.clone() else {
            return;
        };
        let s = a.proof.settings;
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("develop-proof", move |l| {
            let r = Proofer::new(&c.icc, s)
                .map(|p| (Arc::new(p.bake()), p.targets()))
                .map_err(|e| match e {
                    specular_develop::proof::ProofError::NotOutput => {
                        t!("develop-proof-not-output")
                    }
                    specular_develop::proof::ProofError::Unreadable(_) => {
                        t!("develop-proof-unreadable")
                    }
                });
            let _ = tx.send((key, r));
            l.waker()();
        });
        a.proof.baking = Some((key, rx));
    }

    /// A profile file chosen: read and checked, then used.
    fn develop_proof_file(&mut self, path: PathBuf) {
        let now = self.now();
        let icc = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => {
                self.chrome.warning(t!("develop-proof-unreadable"), now);
                return;
            }
        };
        match Proofer::new(&icc, ProofSettings::default()) {
            Ok(_) => {}
            Err(specular_develop::proof::ProofError::NotOutput) => {
                self.chrome.warning(t!("develop-proof-not-output"), now);
                return;
            }
            Err(_) => {
                self.chrome.warning(t!("develop-proof-unreadable"), now);
                return;
            }
        }
        let name = specular_color::icc::description(&icc).unwrap_or_else(|| {
            path.file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
        let p = &mut self.develop.aids.proof;
        p.chosen = Some(Chosen {
            path,
            name: name.clone(),
            icc: Arc::new(icc),
        });
        p.on = true;
        self.chrome
            .announce_status(t!("develop-proof-using", name = name));
        self.develop_proof_remember();
        self.redraw = true;
    }

    /// The cards follow the state.
    pub(super) fn develop_aids_cards(&mut self) {
        if let Some(s) = self.develop.session.as_mut() {
            place_cards(&mut s.panel, &self.develop.aids);
        }
        self.redraw = true;
    }

    /// The cards' events.
    pub(super) fn develop_aids_events(&mut self, ev: Vec<Event>) {
        for e in ev {
            match e {
                Event::Enabled {
                    card: PROOF_CARD,
                    on,
                } => self.develop_proof_toggle(Some(on)),
                Event::Value { control, .. } => self.develop_aids_control(control),
                _ => {}
            }
        }
    }

    fn develop_aids_control(&mut self, control: u32) {
        let chosen = |s: &super::controls::Session, c: u32| match s.panel.control(c) {
            Some(Control {
                body: Body::Choice(ch),
                ..
            }) => ch.selected,
            _ => 0,
        };
        let Some(s) = self.develop.session.as_ref() else {
            return;
        };
        let v = chosen(s, control);
        let a = &mut self.develop.aids;
        match control {
            id::CLIP_DISPLAY => a.clip.display = v == 1,
            id::CLIP_SCENE => a.clip.scene = v == 1,
            id::CLIP_GAMUT => a.clip.gamut = v == 1,
            id::CLIP_GHOSTS => {
                self.develop_ghosts_toggle(Some(v == 1));
                return;
            }
            id::PROOF_INTENT => {
                a.proof.settings.intent = if v == 1 {
                    ProofIntent::Relative
                } else {
                    ProofIntent::Perceptual
                };
            }
            id::PROOF_BPC => a.proof.settings.bpc = v == 1,
            id::PROOF_PAPER => {
                a.proof.settings.paper = v == 1;
                // The paper's white shows the ink's black too.
                if a.proof.settings.paper {
                    a.proof.settings.ink = true;
                }
            }
            id::PROOF_INK => a.proof.settings.ink = v == 1,
            id::PROOF_PROFILE => {
                let list = profile_list(&a.proof);
                if let Some(f) = list.get(v) {
                    let same = a.proof.chosen.as_ref().is_some_and(|c| c.path == f.path);
                    if !same {
                        match std::fs::read(&f.path) {
                            Ok(icc) => {
                                a.proof.chosen = Some(Chosen {
                                    path: f.path.clone(),
                                    name: f.name.clone(),
                                    icc: Arc::new(icc),
                                });
                            }
                            Err(_) => {
                                let now = self.now();
                                self.chrome.warning(t!("develop-proof-unreadable"), now);
                            }
                        }
                    }
                }
            }
            id::PROOF_FILE => {
                self.develop_proof_choose();
                return;
            }
            _ => return,
        }
        self.develop_proof_remember();
        self.develop_aids_cards();
    }

    /// The file dialog for a profile.
    fn develop_proof_choose(&mut self) {
        let dir = specular_develop::proof::icc_folders()
            .into_iter()
            .find(|d| d.is_dir());
        let (tx, rx) = crossbeam_channel::bounded(1);
        self.loader.background("dlg-icc", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter("ICC", &["icc", "icm", "ICC", "ICM"]);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            if let Some(p) = dlg.pick_file() {
                let _ = tx.send(p);
                l.waker()();
            }
        });
        self.develop.aids.proof.file = Some(rx);
    }

    /// Soft proof on or off (`None`: the other way).
    fn develop_proof_toggle(&mut self, on: Option<bool>) {
        let p = &mut self.develop.aids.proof;
        let on = on.unwrap_or(!p.on);
        if on && p.chosen.is_none() {
            p.on = false;
            self.chrome.announce_status(t!("develop-proof-choose"));
            self.develop_aids_cards();
            self.develop_proof_choose();
            return;
        }
        p.on = on;
        let msg = match (&p.chosen, on) {
            (Some(c), true) => t!("develop-proof-on", name = c.name.clone()),
            _ => t!("develop-proof-off"),
        };
        self.chrome.announce_status(msg);
        self.develop_aids_cards();
    }

    /// The ghost overlay on or off (`None`: the other way); says so.
    pub(in crate::app) fn develop_ghosts_toggle(&mut self, on: Option<bool>) {
        let Some(g) = self.develop.aids.ghosts.as_mut() else {
            return;
        };
        g.on = on.unwrap_or(!g.on);
        let what = t!("develop-merge-show-ghosts");
        let msg = if g.on {
            t!("develop-aid-on", what = what)
        } else {
            t!("develop-aid-off", what = what)
        };
        self.chrome.announce_status(msg);
        self.develop_aids_cards();
    }

    /// Develop's aid actions; whether `a` was one.
    pub(super) fn develop_aids_action(&mut self, a: Action) -> bool {
        let say = |on: bool, what: String| {
            if on {
                t!("develop-aid-on", what = what)
            } else {
                t!("develop-aid-off", what = what)
            }
        };
        let c = &mut self.develop.aids.clip;
        let msg = match a {
            Action::DevelopScopes => self.develop.aids.dock.cycle(),
            Action::DevelopClipDisplay => {
                c.display = !c.display;
                say(c.display, t!("develop-clip-display"))
            }
            Action::DevelopClipScene => {
                c.scene = !c.scene;
                say(c.scene, t!("develop-clip-scene"))
            }
            Action::DevelopClipGamut => {
                c.gamut = !c.gamut;
                say(c.gamut, t!("develop-clip-gamut"))
            }
            Action::DevelopSoftProof => {
                self.develop_proof_toggle(None);
                return true;
            }
            _ => return false,
        };
        self.chrome.announce_status(msg);
        self.develop_aids_cards();
        true
    }

    /// A right-click on the picture: the aids menu, ticked. Whether it opened.
    pub(super) fn develop_aids_menu(&mut self, lp: (f32, f32)) -> bool {
        let km = self.keymap.clone();
        let rows = menu_rows(&self.develop.aids);
        let mut toggles = Vec::new();
        for (i, (label, on, action)) in rows.iter().enumerate() {
            toggles.push(PaletteItem {
                id: items::DEVELOP_AIDS + i as u64,
                label: label.clone(),
                key: km.keys_for(*action).first().map(|k| k.to_string()),
                unavailable: None,
                icon: if *on { Icon::Check } else { Icon::Minus },
            });
        }
        // A merge's ghost mask (no key of its own).
        if let Some(g) = &self.develop.aids.ghosts {
            toggles.push(PaletteItem {
                id: items::DEVELOP_AIDS + GHOSTS_ROW,
                label: t!("develop-merge-show-ghosts"),
                key: None,
                unavailable: None,
                icon: if g.on { Icon::Check } else { Icon::Minus },
            });
        }
        let print = vec![PaletteItem {
            id: items::DEVELOP_AIDS + 8,
            label: t!("develop-print"),
            key: km.keys_for(Action::Print).first().map(|k| k.to_string()),
            unavailable: None,
            icon: Icon::Image,
        }];
        self.chrome
            .open_menu(lp, Some(t!("develop-aids")), vec![toggles, print]);
        self.redraw = true;
        true
    }

    /// A row of that menu; whether `id` was one.
    /// A press on the scope dock (or a screen reader's click on it) did `p`: say what
    /// changed, or open the picker's menu.
    pub(super) fn develop_dock_pressed(&mut self, p: Press) {
        match p {
            Press::Outside | Press::Took => {}
            Press::Changed => {
                let msg = self.develop.aids.dock.status();
                self.chrome.announce_status(msg);
            }
            Press::Picker(under) => self.develop_scope_menu(under),
        }
        self.redraw = true;
    }

    /// The scope picker's menu under `under` (its rect): the scopes, the one showing
    /// ticked, and "Hide scopes".
    fn develop_scope_menu(&mut self, under: Rect) {
        let d = &self.develop.aids.dock;
        let row = |k: u64, label: String, icon: Icon| PaletteItem {
            id: items::DEVELOP_AIDS + k,
            label,
            key: None,
            unavailable: None,
            icon,
        };
        let scopes = Scope::ALL
            .iter()
            .enumerate()
            .map(|(i, s)| {
                let on = d.shown && d.scope == *s;
                row(
                    SCOPE_ROWS + i as u64,
                    s.name(),
                    if on { Icon::Check } else { Icon::Minus },
                )
            })
            .collect();
        let hide = vec![row(HIDE_SCOPES_ROW, t!("develop-scopes-hide"), Icon::Close)];
        // Down from the picker's start edge.
        let x = if self.rtl() { under.right() } else { under.x };
        self.chrome
            .open_menu((x, under.bottom()), None, vec![scopes, hide]);
        self.develop.menu_closed();
        self.develop.aids.dock.menu_open = true;
        self.redraw = true;
    }

    pub(in crate::app) fn develop_aids_item(&mut self, id: u64) -> bool {
        let Some(k) = id.checked_sub(items::DEVELOP_AIDS).filter(|k| *k < 16) else {
            return false;
        };
        if let Some(&s) = k
            .checked_sub(SCOPE_ROWS)
            .and_then(|i| Scope::ALL.get(i as usize))
        {
            if self.develop.aids.dock.choose(s) {
                let msg = self.develop.aids.dock.status();
                self.chrome.announce_status(msg);
                self.develop_aids_cards();
            }
            self.redraw = true;
            return true;
        }
        if k == HIDE_SCOPES_ROW {
            self.develop.aids.dock.shown = false;
            self.chrome.announce_status(t!("develop-scopes-hidden"));
            self.develop_aids_cards();
            self.redraw = true;
            return true;
        }
        if k == 8 {
            self.print_open();
            return true;
        }
        if k == GHOSTS_ROW {
            self.develop_ghosts_toggle(None);
            return true;
        }
        if let Some((_, _, a)) = menu_rows(&self.develop.aids).get(k as usize) {
            self.develop_aids_action(*a);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_develop::model::History;

    #[test]
    fn cards_go_before_the_batch_cards_and_keep_their_state() {
        let p = PathBuf::from("/photos/a.cr3");
        let mut s = super::super::controls::Session::new(
            p.clone(),
            super::super::controls::new_edit(&p),
            History::default(),
        );
        let mut a = Aids::default();
        a.clip.gamut = true;
        place_cards(&mut s.panel, &a);
        let ids: Vec<u32> = s.panel.cards.iter().map(|c| c.id).collect();
        let at = |id| ids.iter().position(|&c| c == id).expect("card");
        assert_eq!(at(PROOF_CARD), at(CLIP_CARD) + 1);
        // Opened, placed again: still open, still once, values follow the state.
        s.panel.cards[at(CLIP_CARD)].expanded = true;
        a.clip.display = true;
        place_cards(&mut s.panel, &a);
        let clip: Vec<&Card> = s.panel.cards.iter().filter(|c| c.id == CLIP_CARD).collect();
        assert_eq!(clip.len(), 1);
        assert!(clip[0].expanded);
        let on = |id: u32| match s.panel.control(id).map(|c| &c.body) {
            Some(Body::Choice(c)) => c.selected == 1,
            _ => panic!("choice {id}"),
        };
        assert!(on(id::CLIP_DISPLAY) && on(id::CLIP_GAMUT) && !on(id::CLIP_SCENE));
        assert!(is_ours(&Event::Value {
            control: id::PROOF_INK,
            part: 0,
            done: true
        }));
        assert!(is_ours(&Event::Enabled {
            card: PROOF_CARD,
            on: true
        }));
        assert!(!is_ours(&Event::Value {
            control: super::super::controls::id::EXPOSURE,
            part: 0,
            done: true
        }));
    }

    #[test]
    fn proof_choices_are_remembered_in_the_user_settings() {
        // The defaults are the proof's own.
        let mut c = specular_settings::Settings::default().colour;
        assert_eq!(remembered_settings(&c), ProofSettings::default());
        assert_eq!(c.proof_profile, None);
        for s in [
            ProofSettings {
                intent: ProofIntent::Relative,
                bpc: true,
                paper: true,
                ink: true,
            },
            ProofSettings {
                intent: ProofIntent::Perceptual,
                bpc: false,
                paper: false,
                ink: true,
            },
        ] {
            store_settings(&mut c, s);
            assert_eq!(remembered_settings(&c), s);
        }
        // A remembered profile: read back when it is there, refused when it is gone or not
        // a printer profile (Develop then leaves proofing off and says so).
        let dir = std::env::temp_dir().join(format!("specular-proof-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let icc = dir.join("printer.icc");
        std::fs::write(&icc, specular_develop::proof::synthetic::icc()).unwrap();
        let chosen = read_profile(&icc).expect("a printer profile");
        assert_eq!(chosen.path, icc);
        assert!(!chosen.name.is_empty());
        assert!(read_profile(&dir.join("gone.icc")).is_err());
        let junk = dir.join("junk.icc");
        std::fs::write(&junk, b"not a profile").unwrap();
        assert!(read_profile(&junk).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// Every Develop open applies the remembered proof settings and profile, not only the
    /// first one in a run; a missing profile is reported once.
    #[test]
    fn proof_settings_apply_at_every_develop_open() {
        let chosen = |p: &std::path::Path| Chosen {
            path: p.to_owned(),
            name: "printer".into(),
            icc: Arc::new(Vec::new()),
        };
        let mut p = Proof::default();
        let mut c = specular_settings::Settings::default().colour;
        let a = PathBuf::from("a.icc");
        c.proof_profile = Some(a.clone());
        assert_eq!(sync_proof(&mut p, &c, |q| Ok(chosen(q))), None);
        assert_eq!(p.chosen.as_ref().map(|ch| ch.path.clone()), Some(a.clone()));
        // Changed between opens (Settings window, settings file): applied at the next.
        let s = ProofSettings {
            intent: ProofIntent::Relative,
            bpc: false,
            paper: true,
            ink: true,
        };
        store_settings(&mut c, s);
        let b = PathBuf::from("b.icc");
        c.proof_profile = Some(b.clone());
        assert_eq!(sync_proof(&mut p, &c, |q| Ok(chosen(q))), None);
        assert_eq!(p.settings, s);
        assert_eq!(p.chosen.as_ref().map(|ch| ch.path.clone()), Some(b.clone()));
        // The same profile is not read again.
        assert_eq!(sync_proof(&mut p, &c, |_| panic!("read again")), None);
        // Gone: proofing off, nothing chosen, said once.
        p.on = true;
        let gone = PathBuf::from("gone.icc");
        c.proof_profile = Some(gone.clone());
        assert_eq!(sync_proof(&mut p, &c, |_| Err(())), Some(gone.clone()));
        assert!(!p.on && p.chosen.is_none());
        assert_eq!(sync_proof(&mut p, &c, |_| Err(())), None);
        // Back again: used.
        assert_eq!(sync_proof(&mut p, &c, |q| Ok(chosen(q))), None);
        assert!(p.chosen.is_some() && p.missing.is_none());
    }

    #[test]
    fn scope_dock_requests_scopes_and_marks_the_proof_gamut() {
        let mut a = Aids {
            dock: ScopeDock::new(),
            ..Default::default()
        };
        assert!(a.request().scopes && a.request().proof.is_none());
        a.proof.on = true;
        a.proof.lut = Some(Arc::new(ProofLut {
            n: 2,
            data: vec![0.0; 32],
            key: 7,
        }));
        a.proof.targets = Some(scopes::targets());
        assert_eq!(a.request().proof.map(|l| l.key), Some(7));
        assert!(a.marks().proof.is_some());
        a.proof.on = false;
        assert!(a.marks().proof.is_none() && a.request().proof.is_none());
    }
}
