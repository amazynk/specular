//! Profiles: which capabilities (General, Engineering, Medical, Science, Maps) are on, and so which
//! actions are shown at all. A hidden action is not the same as a disabled one: it is left out
//! of the toolbar, menus, the cheat sheet and the accessibility tree, and its keys fall through
//! to whatever else binds them; the palette still finds it (greyed, "Medical tools are off")
//! and can turn its capability on.
//!
//! Every action has a [`Capability`] and a [`Tier`]. General actions always show. A
//! capability's tier-1 (viewing) actions also show while an image of its domain is on screen,
//! whatever the profile (window/level for a CT); its tier-2 (workflow) actions show only while
//! the capability is on, in the settings or for this session. Profiles never limit decoding.
//!
//! Lanes adding actions only tag them in [`Action::capability`] and [`Action::tier`].

use crate::{Action, ActionState, Disabled, Mode};

/// A set of tools a profile turns on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Capability {
    /// Viewing, browsing, editing basics: always on.
    General,
    Engineering,
    Medical,
    Science,
    /// Maps (GIS): the map view of georeferenced and vector map files.
    Maps,
}

impl Capability {
    pub const ALL: [Capability; 5] = [
        Capability::General,
        Capability::Engineering,
        Capability::Medical,
        Capability::Science,
        Capability::Maps,
    ];

    /// The name in `settings.toml` (`profile.capabilities`) and in Fluent selectors.
    pub fn id(self) -> &'static str {
        match self {
            Capability::General => "general",
            Capability::Engineering => "engineering",
            Capability::Medical => "medical",
            Capability::Science => "science",
            Capability::Maps => "maps",
        }
    }

    pub fn from_id(id: &str) -> Option<Capability> {
        Capability::ALL.into_iter().find(|c| c.id() == id)
    }

    const fn bit(self) -> u8 {
        1 << self as u8
    }
}

/// A set of capabilities. General is always a member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Capabilities(u8);

impl Capabilities {
    /// General only (the default profile).
    pub const GENERAL: Capabilities = Capabilities(Capability::General.bit());
    /// Every capability: an unrestricted profile.
    pub const ALL: Capabilities = Capabilities(0b1_1111);

    pub const fn contains(self, c: Capability) -> bool {
        self.0 & c.bit() != 0
    }

    pub const fn with(self, c: Capability) -> Capabilities {
        Capabilities(self.0 | c.bit())
    }

    /// Without `c` (General stays).
    pub const fn without(self, c: Capability) -> Capabilities {
        Capabilities((self.0 & !c.bit()) | Capability::General.bit())
    }

    pub const fn union(self, o: Capabilities) -> Capabilities {
        Capabilities(self.0 | o.0)
    }

    /// The members, in [`Capability::ALL`] order.
    pub fn iter(self) -> impl Iterator<Item = Capability> {
        Capability::ALL
            .into_iter()
            .filter(move |c| self.contains(*c))
    }

    /// From settings names (unknown names are ignored).
    pub fn from_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Capabilities {
        ids.into_iter()
            .filter_map(Capability::from_id)
            .fold(Capabilities::GENERAL, Capabilities::with)
    }
}

impl Default for Capabilities {
    fn default() -> Self {
        Capabilities::GENERAL
    }
}

impl FromIterator<Capability> for Capabilities {
    fn from_iter<I: IntoIterator<Item = Capability>>(it: I) -> Self {
        it.into_iter()
            .fold(Capabilities::GENERAL, Capabilities::with)
    }
}

/// How deep into a domain an action goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Tier {
    /// Needed to look at the domain's images properly (window/level, slices, the tag
    /// browser): shown automatically while such an image is on screen.
    Viewing,
    /// A workflow (MPR, anonymised export, measurements beyond the ruler): shown only while
    /// the capability is on; a toolbar chip offers it while such an image is on screen.
    Workflow,
}

/// The capabilities in effect, filled in by the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Profile {
    /// Chosen in the settings (`profile.capabilities`).
    pub enabled: Capabilities,
    /// Turned on for this session (the toolbar chip, the palette).
    pub session: Capabilities,
    /// The domains of the image on screen, or of the gallery's focused item.
    pub content: Capabilities,
    /// MPR is open: the medical actions it uses on any volume show
    /// ([`Action::is_mpr_basic`]).
    pub mpr: bool,
}

impl Profile {
    /// Every capability on: nothing hidden. The default, so code that does not care about
    /// profiles sees every action; the application sets the real one.
    pub const UNRESTRICTED: Profile = Profile {
        enabled: Capabilities::ALL,
        session: Capabilities::GENERAL,
        content: Capabilities::GENERAL,
        mpr: false,
    };

    /// A profile of `enabled` with no session enables and no domain content.
    pub const fn of(enabled: Capabilities) -> Profile {
        Profile {
            enabled,
            session: Capabilities::GENERAL,
            content: Capabilities::GENERAL,
            mpr: false,
        }
    }

    /// The capability is on (settings or session).
    pub const fn is_on(&self, c: Capability) -> bool {
        self.enabled.union(self.session).contains(c)
    }

    /// Whether actions of capability `c` and tier `tier` show.
    pub const fn shows(&self, c: Capability, tier: Tier) -> bool {
        matches!(c, Capability::General)
            || self.is_on(c)
            || (matches!(tier, Tier::Viewing) && self.content.contains(c))
    }

    /// Whether `a` shows: its capability and tier, or (MPR open) one of MPR's tier-1 tools.
    pub fn shows_action(&self, a: Action) -> bool {
        if a.is_develop() && self.develop_hidden() {
            return false;
        }
        (self.mpr && a.is_mpr_basic()) || self.shows(a.capability(), a.tier())
    }

    /// Develop is hidden: the Medical or Science profile is on (DESIGN §6.10). A profile
    /// with every capability on hides nothing, Develop included.
    pub fn develop_hidden(&self) -> bool {
        (self.is_on(Capability::Medical) || self.is_on(Capability::Science))
            && self.enabled != Capabilities::ALL
    }

    /// The capability whose toolbar chip ("Medical tools") shows: the content's first domain
    /// whose workflows are off; Maps before Engineering (a vector map is a drawing too).
    pub fn chip(&self) -> Option<Capability> {
        let off = |c: &Capability| *c != Capability::General && !self.is_on(*c);
        if self.content.contains(Capability::Maps) && off(&Capability::Maps) {
            return Some(Capability::Maps);
        }
        self.content.iter().find(off)
    }
}

impl Default for Profile {
    fn default() -> Self {
        Profile::UNRESTRICTED
    }
}

/// Whether an action is shown, and whether it can run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Visible,
    /// Left out: the capability is off (the palette lists it greyed, offering to turn it on).
    Hidden(Capability),
    /// Shown, greyed, with the reason.
    Disabled(Disabled),
}

/// The pure rule: whether `action` shows under `profile` with `content` on screen and the
/// `session` enables, and if so whether `st` lets it run.
pub fn visibility(
    action: Action,
    profile: Capabilities,
    content: Capabilities,
    session: Capabilities,
    st: &ActionState,
) -> Visibility {
    let p = Profile {
        enabled: profile,
        session,
        content,
        mpr: st.mpr,
    };
    if !p.shows_action(action) {
        return Visibility::Hidden(action.capability());
    }
    match action.disabled(st) {
        Some(d) => Visibility::Disabled(d),
        None => Visibility::Visible,
    }
}

impl Action {
    /// The capability the action belongs to. Medical and science modes' actions are Medical and
    /// Science; viewer actions are General unless tagged here.
    pub fn capability(self) -> Capability {
        if self.is_mpr_view() {
            return Capability::General;
        }
        match self.mode() {
            Mode::Medical => Capability::Medical,
            Mode::Science => Capability::Science,
            Mode::Drawing => Capability::Engineering,
            Mode::Mesh => Capability::Engineering,
            // Relighting is viewing: the light and shading of an image.
            Mode::Relight => Capability::General,
            Mode::Video => Capability::General,
            Mode::Map => Capability::Maps,
            // Photo editing: shown in General; the Medical and Science profiles hide it
            // (`Profile::shows_action`).
            Mode::Develop => Capability::General,
            Mode::Viewer | Mode::Cull => match self {
                Action::DicomTags
                | Action::Studies
                | Action::NextSeries
                | Action::PrevSeries
                | Action::ExportAnonymised
                | Action::ExportPatientDisc
                | Action::MaskBurnedInText
                | Action::MeasureTemplates => Capability::Medical,
                _ => Capability::General,
            },
        }
    }

    /// The action's tier within its capability (General actions are all `Viewing`).
    pub fn tier(self) -> Tier {
        match self {
            Action::MeasureAngle
            | Action::MeasureTemplates
            | Action::MeasureEllipse
            | Action::MeasureRectangle
            | Action::Fusion
            | Action::FusionColormap
            | Action::FusionBlend
            | Action::FusionNudge
            | Action::Tmtv
            | Action::TmtvThreshold
            | Action::TimeCurve
            | Action::ExportCurve
            | Action::ExportAnonymised
            | Action::ExportPatientDisc
            | Action::MaskBurnedInText
            | Action::CompareSeries
            | Action::CompareLayout
            | Action::NextViewport
            | Action::PrevViewport
            | Action::ReferenceLines
            | Action::SyncWindow
            | Action::SyncZoom
            | Action::LinkSeries
            | Action::AlignSeries
            | Action::ExportTeachingFile
            | Action::TeachingQuiz
            | Action::SendToPacs
            | Action::MprDental
            | Action::MprNerve
            | Action::LabelVolume
            // The 3D view's CT and MR transfer presets: medical work, not any volume's.
            | Action::MprPreset
            | Action::DrawingLineweights => Tier::Workflow,
            Action::MeshShading
            | Action::MeshProjection
            | Action::MeshGrid
            | Action::MeshPointsBigger
            | Action::MeshPointsSmaller
            | Action::MeshPointColour
            | Action::MeshEdl
            | Action::MeshMolStyle
            | Action::MeshMolColour
            | Action::MeshTractOpacity
            | Action::MeshWalk
            | Action::MeshFly
            | Action::MeshViewpoints
            | Action::MeshSaveViewpoint
            | Action::MeshSectionPlane
            | Action::MeshSectionBox
            | Action::MeshSectionFlip
            | Action::MeshSectionClear
            | Action::MeshCutMeasure
            | Action::MeshBcf
            | Action::MeshChecks
            | Action::MeshModels
            | Action::MeshAddModel
            | Action::MeshAddModelFolder
            | Action::MeshPmi => Tier::Workflow,
            // A map shows as an image or a drawing while Maps is off: the map view is a
            // workflow the chip offers.
            a if a.mode() == Mode::Map => Tier::Workflow,
            _ => Tier::Viewing,
        }
    }

    /// MPR and the tools of its views (tier 1 for any volume): the slab, the 3D view, the
    /// maximised view and the planes' export. General in every profile; the 3D view's CT
    /// and MR presets are Medical's tier 2.
    pub fn is_mpr_view(self) -> bool {
        matches!(
            self,
            Action::Mpr
                | Action::MprSlab
                | Action::MprTractSlab
                | Action::MprRender
                | Action::MprClip
                | Action::MprExportStl
                | Action::MprMaximise
                | Action::MprExportPlanes
                | Action::MprCurve
                | Action::MprCurveLayout
                | Action::MprCurveSave
                | Action::MprCurveLoad
                | Action::MprExportSegments
        )
    }

    /// Medical actions MPR uses on any volume while it is open (its tier 1): stepping the
    /// planes, the basic window (the file's own, the full range, inverted) and the plane
    /// letters. CT and SUV presets, measuring and the analyses keep their own tiers.
    pub fn is_mpr_basic(self) -> bool {
        matches!(
            self,
            Action::NextSlice
                | Action::PrevSlice
                | Action::InvertGrey
                | Action::WindowPreset(0)
                | Action::WindowPreset(9)
                | Action::ToggleOrientationLabels
                // A label volume lies over any volume in MPR (tier 1 there), with its
                // display: the labels shown, their fill.
                | Action::LabelVolume
                | Action::Segments
                | Action::SegmentOpacity
        )
    }

    /// The capability that hides the action under `st`'s profile, or `None` when it shows.
    pub fn hidden(self, st: &ActionState) -> Option<Capability> {
        // Develop hidden by the Medical or Science profile: no tools to turn on, so the
        // palette finds it greyed with the reason ([`Disabled::DevelopOff`]).
        if self.is_develop() && crate::enabled::develop_off(st) {
            return None;
        }
        let c = self.capability();
        (!st.content_profile().shows_action(self)).then_some(c)
    }

    /// Hidden, disabled (with why) or visible.
    pub fn visibility(self, st: &ActionState) -> Visibility {
        let p = st.content_profile();
        if !p.shows_action(self) {
            return Visibility::Hidden(self.capability());
        }
        match self.disabled(st) {
            Some(d) => Visibility::Disabled(d),
            None => Visibility::Visible,
        }
    }
}

impl ActionState {
    /// The profile with the content the state itself shows (a DICOM file is medical, a FITS
    /// file science).
    pub fn content_profile(&self) -> Profile {
        let mut p = self.profile;
        p.mpr |= self.mpr;
        if self.dicom || self.medical.is_some() {
            p.content = p.content.with(Capability::Medical);
        }
        if self.science.is_some() {
            p.content = p.content.with(Capability::Science);
        }
        if self.drawing || self.mesh {
            p.content = p.content.with(Capability::Engineering);
        }
        if self.map.is_some() {
            p.content = p.content.with(Capability::Maps);
        }
        p
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ImageState;

    const GENERAL: Capabilities = Capabilities::GENERAL;
    const MEDICAL: Capabilities = Capabilities::GENERAL.with(Capability::Medical);

    fn ct() -> ActionState {
        ActionState {
            image: ImageState::Ready,
            count: 300,
            dicom: true,
            medical: Some(300),
            ..ActionState::default()
        }
    }

    fn photo() -> ActionState {
        ActionState {
            image: ImageState::Ready,
            count: 3,
            ..ActionState::default()
        }
    }

    #[test]
    fn general_actions_always_show() {
        for a in Action::BINDABLE {
            if a.capability() == Capability::General {
                assert_eq!(a.tier(), Tier::Viewing, "{a:?}");
                assert_ne!(
                    visibility(*a, GENERAL, GENERAL, GENERAL, &photo()),
                    Visibility::Hidden(Capability::General)
                );
            }
        }
        assert_eq!(
            visibility(Action::ZoomIn, GENERAL, GENERAL, GENERAL, &photo()),
            Visibility::Visible
        );
    }

    #[test]
    fn medical_tiers() {
        let st = ct();
        // A CT in a General profile: viewing tools show, workflows do not.
        for (a, tier) in [
            (Action::WindowLevelTool, Tier::Viewing),
            (Action::WindowPreset(1), Tier::Viewing),
            (Action::NextSlice, Tier::Viewing),
            (Action::DicomTags, Tier::Viewing),
            (Action::MeasureLength, Tier::Viewing),
            (Action::Segments, Tier::Viewing),
            (Action::SegmentOpacity, Tier::Viewing),
            (Action::Fusion, Tier::Workflow),
            (Action::Tmtv, Tier::Workflow),
            (Action::TimeCurve, Tier::Workflow),
            (Action::ExportCurve, Tier::Workflow),
            (Action::MeasureAngle, Tier::Workflow),
            (Action::ExportAnonymised, Tier::Workflow),
        ] {
            assert_eq!(a.capability(), Capability::Medical, "{a:?}");
            assert_eq!(a.tier(), tier, "{a:?}");
            let v = visibility(a, GENERAL, MEDICAL, GENERAL, &st);
            match tier {
                Tier::Viewing => assert_eq!(v, Visibility::Visible, "{a:?}"),
                Tier::Workflow => {
                    assert_eq!(v, Visibility::Hidden(Capability::Medical), "{a:?}")
                }
            }
            // On in the settings, or for the session: everything shows.
            assert_eq!(
                visibility(a, MEDICAL, MEDICAL, GENERAL, &st),
                Visibility::Visible
            );
            assert_eq!(
                visibility(a, GENERAL, MEDICAL, MEDICAL, &st),
                Visibility::Visible
            );
        }
        // A photo: the viewing tools go away again.
        assert_eq!(
            visibility(Action::WindowLevelTool, GENERAL, GENERAL, GENERAL, &photo()),
            Visibility::Hidden(Capability::Medical)
        );
        // With Medical on, a photo shows them disabled with the reason.
        assert_eq!(
            visibility(Action::WindowLevelTool, MEDICAL, GENERAL, GENERAL, &photo()),
            Visibility::Disabled(Disabled::NotMedical)
        );
    }

    /// MPR is tier 1 for any volume in every profile; inside it, its basic window, plane
    /// stepping and labels show too, while CT presets, measuring and the analyses keep
    /// their tiers.
    #[test]
    fn mpr_in_every_profile() {
        let profiles = [
            Capabilities::GENERAL,
            Capabilities::GENERAL.with(Capability::Science),
            Capabilities::GENERAL.with(Capability::Engineering),
            MEDICAL,
            Capabilities::ALL,
        ];
        // A microscopy z-stack (science), a research volume (medical), a CT series.
        let stack = ActionState {
            image: ImageState::Ready,
            count: 1,
            science: Some(crate::ScienceState {
                planes: 40,
                axes: 1,
                ..Default::default()
            }),
            volume: true,
            ..ActionState::default()
        };
        let research = ActionState {
            medical: Some(64),
            science: None,
            ..stack
        };
        let ct = ActionState {
            volume: true,
            ..ct()
        };
        for enabled in profiles {
            for base in [stack, research, ct] {
                let mut st = base;
                st.profile = Profile::of(enabled);
                assert_eq!(
                    Action::Mpr.visibility(&st),
                    Visibility::Visible,
                    "{enabled:?}"
                );
                assert_eq!(
                    Action::MprSlab.visibility(&st),
                    Visibility::Disabled(Disabled::MprClosed)
                );
                st.mpr = true;
                for a in [
                    Action::Mpr,
                    Action::MprSlab,
                    Action::MprMaximise,
                    Action::MprExportPlanes,
                    Action::MprRender,
                    Action::NextSlice,
                    Action::WindowPreset(0),
                    Action::InvertGrey,
                ] {
                    assert_eq!(a.hidden(&st), None, "{a:?} {enabled:?}");
                }
                let medical_on = enabled.contains(Capability::Medical);
                for a in [Action::Fusion, Action::Tmtv, Action::MeasureAngle] {
                    assert_eq!(a.hidden(&st).is_none(), medical_on, "{a:?} {enabled:?}");
                }
            }
        }
        // A z-stack in General: CT presets and the ruler are the medical domain's.
        let mut st = stack;
        st.profile = Profile::of(Capabilities::GENERAL);
        st.mpr = true;
        assert_eq!(
            Action::WindowPreset(1).hidden(&st),
            Some(Capability::Medical)
        );
        assert_eq!(Action::MeasureLength.hidden(&st), Some(Capability::Medical));
        // The chip offers science tools for a microscopy stack, not medical ones.
        assert_eq!(st.content_profile().chip(), Some(Capability::Science));
        // A photo: MPR shows, greyed with the reason.
        let mut ph = photo();
        ph.profile = Profile::of(Capabilities::GENERAL);
        assert_eq!(
            Action::Mpr.visibility(&ph),
            Visibility::Disabled(Disabled::NoVolume)
        );
    }

    #[test]
    fn state_supplies_the_content() {
        let mut st = ct();
        st.profile = Profile::of(GENERAL);
        assert_eq!(Action::Cine.hidden(&st), None);
        assert_eq!(Action::Fusion.hidden(&st), Some(Capability::Medical));
        assert_eq!(st.content_profile().chip(), Some(Capability::Medical));
        st.profile.session = MEDICAL;
        assert_eq!(Action::Fusion.hidden(&st), None);
        assert_eq!(st.content_profile().chip(), None);
        let mut ph = photo();
        ph.profile = Profile::of(GENERAL);
        assert_eq!(ph.content_profile().chip(), None);
        assert_eq!(
            Action::DicomTags.visibility(&ph),
            Visibility::Hidden(Capability::Medical)
        );
        // The default profile hides nothing.
        assert!(
            Action::BINDABLE
                .iter()
                .all(|a| a.hidden(&ActionState::default()).is_none())
        );
    }

    #[test]
    fn capability_sets() {
        assert!(Capabilities::default().contains(Capability::General));
        let c = Capabilities::from_ids(["medical", "nonsense", "science"]);
        assert_eq!(
            c.iter().collect::<Vec<_>>(),
            [
                Capability::General,
                Capability::Medical,
                Capability::Science
            ]
        );
        assert!(c.without(Capability::General).contains(Capability::General));
        for cap in Capability::ALL {
            assert_eq!(Capability::from_id(cap.id()), Some(cap));
        }
        // Every capability is in the unrestricted set.
        assert!(
            Capability::ALL
                .iter()
                .all(|c| Capabilities::ALL.contains(*c))
        );
        assert_eq!(Capabilities::ALL.iter().count(), 5);
    }

    #[test]
    fn maps_tools_follow_the_profile() {
        use crate::MapState;
        const MAPS: Capabilities = Capabilities::GENERAL.with(Capability::Maps);
        let map = ActionState {
            image: ImageState::Ready,
            count: 1,
            map: Some(MapState {
                lonlat: true,
                features: true,
                heights: false,
                time: false,
                wind: false,
                track: false,
                grid: false,
            }),
            ..ActionState::default()
        };
        for a in crate::MAP_ACTIONS {
            assert_eq!(a.capability(), Capability::Maps, "{a:?}");
            assert_eq!(a.tier(), Tier::Workflow, "{a:?}");
            // Maps off: hidden even over a map, and the chip offers the tools.
            assert_eq!(
                visibility(a, GENERAL, MAPS, GENERAL, &map),
                Visibility::Hidden(Capability::Maps),
                "{a:?}"
            );
        }
        let mut st = map;
        st.profile = Profile::of(GENERAL);
        assert_eq!(st.content_profile().chip(), Some(Capability::Maps));
        // A vector map is a drawing too: the chip offers Maps first.
        st.drawing = true;
        assert_eq!(st.content_profile().chip(), Some(Capability::Maps));
        st.profile = Profile::of(MAPS);
        assert_eq!(st.content_profile().chip(), Some(Capability::Engineering));
        st.drawing = false;
        assert_eq!(st.content_profile().chip(), None);
        assert_eq!(Action::MapLayers.visibility(&st), Visibility::Visible);
        // A map without heights: the elevation profile says why.
        assert_eq!(
            Action::MapElevationProfile.visibility(&st),
            Visibility::Disabled(Disabled::NoHeights)
        );
        assert_eq!(
            Action::MapContours.visibility(&st),
            Visibility::Disabled(Disabled::NoHeights)
        );
        // Without times, wind or a timed track: each says why.
        assert_eq!(
            Action::MapPlay.visibility(&st),
            Visibility::Disabled(Disabled::NoTime)
        );
        assert_eq!(
            Action::MapWind.visibility(&st),
            Visibility::Disabled(Disabled::NoWind)
        );
        assert_eq!(
            Action::MapGeotag.visibility(&st),
            Visibility::Disabled(Disabled::NoTrack)
        );
        assert_eq!(
            Action::MapWeatherColours.visibility(&st),
            Visibility::Disabled(Disabled::NotGrid)
        );
        assert_eq!(Action::MapExport.visibility(&st), Visibility::Visible);
        assert_eq!(Action::MapServices.visibility(&st), Visibility::Visible);
        // A photo with Maps on: shown, disabled with the reason.
        let mut ph = photo();
        ph.profile = Profile::of(MAPS);
        assert_eq!(
            Action::MapLayers.visibility(&ph),
            Visibility::Disabled(Disabled::NotMap)
        );
    }
}

#[cfg(test)]
mod develop_tests {
    use super::*;

    /// Develop shows in General, is hidden (its keys fall through) in the Medical and
    /// Science profiles, and the palette finds it greyed with the reason.
    #[test]
    fn develop_is_hidden_in_medical_and_science() {
        let general = Profile::of(Capabilities::GENERAL);
        assert!(general.shows_action(Action::ToggleDevelop));
        assert!(Profile::UNRESTRICTED.shows_action(Action::DevelopBeforeAfter));
        for c in [Capability::Medical, Capability::Science] {
            let p = Profile::of(Capabilities::GENERAL.with(c));
            assert!(!p.shows_action(Action::ToggleDevelop), "{c:?}");
            assert!(!p.shows_action(Action::DevelopSpeedExposure), "{c:?}");
            let st = ActionState {
                profile: p,
                ..Default::default()
            };
            assert_eq!(Action::ToggleDevelop.hidden(&st), None);
            assert_eq!(
                Action::ToggleDevelop.disabled(&st),
                Some(Disabled::DevelopOff)
            );
        }
        let p = Profile::of(Capabilities::GENERAL.with(Capability::Engineering));
        assert!(p.shows_action(Action::ToggleDevelop));
    }
}
