//! When an action can run (DESIGN.md §11, "Enabled state"). This is the one rule the toolbar,
//! edge arrows, keyboard dispatch, command palette and accessibility tree all read, so a
//! control never looks usable while its key does nothing, or the reverse.

use crate::{Action, Profile, Topic};

/// How far the current image has got.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ImageState {
    /// Empty window or empty folder.
    #[default]
    None,
    /// A file is chosen but its header has not been read yet. Disabled like `None` (with
    /// "still loading" as the reason), but panels stay open: this lasts a moment while flipping.
    Opening,
    /// Header read (dimensions known); pixels still decoding.
    Decoding,
    /// Pixels on screen.
    Ready,
    /// The decoder gave up; the error card is shown.
    Failed,
}

/// The viewer facts the rule depends on, filled in by the application each frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ActionState {
    pub image: ImageState,
    /// Images in the folder (0 with no image).
    pub count: usize,
    /// Files the gallery shows, while it is shown (0 otherwise): what Print can print with
    /// no image open.
    pub gallery_files: usize,
    /// The current image is an animation with a player.
    pub animated: bool,
    /// The current file is a movie that plays (MP4, MOV, 3GP...).
    pub video: bool,
    /// The movie has sound that plays (a sound track this build decodes).
    pub sound: bool,
    /// The current file is a sound file (its page of visuals shows).
    pub sound_file: bool,
    /// The current image is a page of an archive (no file of its own to trash).
    pub in_archive: bool,
    /// The current image has an alpha channel (known once the header is read).
    pub has_alpha: bool,
    /// Set while the gallery shows images the channel views apply to (quick look, compare,
    /// the similar-shots review): whether one of them may have alpha. The channel actions
    /// then follow those images rather than the viewer's.
    pub gallery_views: Option<bool>,
    /// The current image is shown in medical mode (a grey DICOM image with its values on the
    /// GPU): its number of frames.
    pub medical: Option<u32>,
    /// The current file is a DICOM file (its tags can be browsed).
    pub dicom: bool,
    /// The series of the folder in series mode (0 outside it).
    pub series: usize,
    /// The current image (or its series) is a 3D volume that MPR opens on: a DICOM series or
    /// multi-frame file, a research or industrial CT volume, a microscopy z-stack, a seismic
    /// or science cube. Whatever the profile.
    pub volume: bool,
    /// MPR is open.
    pub mpr: bool,
    /// Series are compared in linked viewports.
    pub comparing: bool,
    /// The gallery's compare is open: with how many images (0: closed).
    pub compare_images: usize,
    /// Presentation states (GSPS) that reference the medical image shown.
    pub presentation_states: usize,
    /// Pages of the current file (ICNS sizes, texture layers, TIFF pages; 0 or 1 for a
    /// single image). Medical mode steps its frames instead (`medical`).
    pub pages: u32,
    /// The current image is shown in science mode (FITS, XISF, DM, MRC, Gwyddion, Igor).
    pub science: Option<ScienceState>,
    /// The current image is a one-shot colour camera's raw frame (FITS science mode, SER
    /// video) that can be shown debayered or raw.
    pub debayer: bool,
    /// The current image has annotations from a sidecar file (a whole-slide image's).
    pub annotations: bool,
    /// The current image is a colour image whose stains can be separated: whether
    /// separation is on (`None`: not a colour image).
    pub stain: Option<bool>,
    /// A region (a drawn area) to estimate stains from.
    pub stain_region: bool,
    /// Annotations or drawn regions and points to export.
    pub slide_annotations: bool,
    /// The current image is a thermal image with a visible-light photo, or that photo.
    pub thermal_visible: bool,
    /// The current image is a drawing (DXF) drawn in drawing mode.
    pub drawing: bool,
    /// The current file is a 3D mesh or a point cloud shown in the 3D view.
    pub mesh: bool,
    /// What is shown in the 3D view is a point cloud.
    pub points: bool,
    /// What is shown in the 3D view is a tractogram (TrackVis, MRtrix): the point size and
    /// colouring keys make its streamlines thicker or thinner and step their colouring.
    pub tracts: bool,
    /// What is shown in the 3D view is a molecular structure.
    pub molecule: bool,
    /// The current image is a reflectance transformation image (PTM, RTI, Relight) relit
    /// on the GPU.
    pub relight: bool,
    /// A memory card, camera or phone with photos is connected (Import… has a source).
    pub import_sources: bool,
    /// The current file's format can be rotated in the file (JPEG, PNG, WebP, TIFF, JPEG XL,
    /// HEIF, AVIF, RAW).
    pub rotatable: bool,
    /// "Set as wallpaper" can work on this desktop.
    pub wallpaper: bool,
    /// The system's share sheet, or the portal's application chooser, is there.
    pub share: bool,
    /// A mail program can be started with an attachment.
    pub email: bool,
    /// The current image is a map shown in the map view (Maps on): what it has.
    pub map: Option<MapState>,
    /// The capabilities in effect: which actions show at all.
    /// The default hides nothing.
    pub profile: Profile,
}

/// What map mode's actions depend on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MapState {
    /// Positions convert to longitude and latitude (the CRS is known).
    pub lonlat: bool,
    /// A layer has features (an attribute table to show).
    pub features: bool,
    /// The map has heights (an elevation model) to draw a profile over.
    pub heights: bool,
    /// Steps in time to slide through: a grid's time axis, or a track with times.
    pub time: bool,
    /// A grid with eastward and northward wind (u and v) to draw arrows from.
    pub wind: bool,
    /// A GPS track with times (to place photos on).
    pub track: bool,
    /// The map is a science grid (its colour map can be chosen).
    pub grid: bool,
}

/// What science mode's actions depend on.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ScienceState {
    /// Planes past the image (the product of the further axes' lengths; 1 for a plain image).
    pub planes: u64,
    /// Further axes (each can be stepped).
    pub axes: u32,
    /// Positions of the channel axis that can be composited (0 without one).
    pub channels: u32,
    /// Channel compositing is on.
    pub composite: bool,
    /// The coordinate grid applies: sky coordinates (a celestial WCS), a georeference, or a
    /// diffraction frame with complete geometry (resolution rings).
    pub celestial: bool,
    /// The planes are height maps that plane levelling suits (scanning probe).
    pub levelling: bool,
    /// One georeferenced band of elevations that relief shading suits (a DEM).
    pub relief: bool,
    /// The planes are seismic sections (traces across).
    pub seismic: bool,
    /// The image is a mass spectrometry image (spectra per pixel, ion images by m/z).
    pub mass_spec: bool,
    /// The image is a thermal image (temperatures per pixel).
    pub thermal: bool,
    /// The thermal image's temperatures can be worked out again for another emissivity and
    /// reflected temperature (a radiometric FLIR image).
    pub radiometric: bool,
    /// The thermal image is a sequence of frames (FLIR SEQ/CSQ).
    pub sequence: bool,
    /// The image is an unstructured mesh (UGRID, ICON, MPAS) painted onto a grid.
    pub ugrid: bool,
    /// Connected to a SAMP hub (Settings › Integration).
    pub samp: bool,
}

/// The disabled reasons: each one's variant, the message saying it (tooltips, the palette,
/// screen readers) and the manual's page and section about it, declared once. A new reason
/// is one line here (and its message in `en`).
macro_rules! disabled_reasons {
    ($($(#[$doc:meta])* $name:ident => $id:literal, $page:literal, $anchor:literal;)*) => {
        /// Why an action is unavailable; shown in tooltips, the palette and to screen readers.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Disabled {
            $($(#[$doc])* $name,)*
        }

        impl Disabled {
            /// Every reason (for checks that each has its message and help topic).
            pub const ALL: &'static [Disabled] = &[$(Disabled::$name),*];

            /// Fluent message ID of the reason.
            pub fn message_id(self) -> &'static str {
                match self {
                    $(Disabled::$name => $id,)*
                }
            }

            /// The page and section of the manual explaining the reason.
            pub fn topic(self) -> Topic {
                match self {
                    $(Disabled::$name => Topic { page: $page, anchor: $anchor },)*
                }
            }
        }
    };
}

disabled_reasons! {
    NoImage => "disabled-no-image", "opening", "open";
    /// Copy image waits for pixels.
    StillLoading => "disabled-still-loading", "troubleshooting", "slow";
    DecodeFailed => "disabled-decode-failed", "troubleshooting", "wont-open";
    /// Next/previous in a folder of one.
    NoOtherImages => "disabled-no-other-images", "viewer", "navigate";
    /// Playback controls on a still image.
    NotAnimated => "disabled-not-animated", "animation", "";
    /// File operations on a page inside an archive.
    InArchive => "disabled-in-archive", "reading", "archives";
    /// The alpha channel view on an image without transparency.
    NoAlpha => "disabled-no-alpha", "inspect", "channels";
    /// Window/level and the other medical actions on an image that is not a grey medical
    /// image.
    NotMedical => "disabled-not-medical", "medical", "";
    /// Slice stepping in a single-frame medical file.
    OneFrame => "disabled-one-frame", "medical", "slices";
    /// The DICOM tag browser on a file that is not DICOM.
    NotDicom => "disabled-not-dicom", "medical", "tags";
    /// Next / previous series outside series mode, or with one series.
    NoOtherSeries => "disabled-no-other-series", "medical", "series";
    /// Stepping presentation states when none references the image.
    NoPresentationState => "disabled-no-presentation-state", "medical", "presentation-states";
    /// Page stepping in a file with a single image.
    OnePage => "disabled-one-page", "reading", "pages";
    /// MPR of a single image (neither a series nor a multi-frame volume).
    NoVolume => "disabled-no-volume", "medical-3d", "";
    /// An MPR tool while MPR is closed.
    MprClosed => "disabled-mpr-closed", "medical-3d", "";
    /// A linked-view setting while series are not being compared.
    NotComparing => "disabled-not-comparing", "medical-compare", "";
    /// A compare control while compare is closed.
    CompareClosed => "disabled-compare-closed", "compare", "";
    /// A control of two images' compare (the link) with three or four compared.
    CompareNotTwo => "disabled-compare-not-two", "compare", "";
    /// Stretch, limits and the other science actions on an image that is not a FITS or XISF
    /// image.
    NotScience => "disabled-not-science", "science", "";
    /// Plane stepping in a science image with a single plane.
    OnePlane => "disabled-one-plane", "science", "planes";
    /// Axis switching with fewer than two further axes.
    OneAxis => "disabled-one-axis", "science", "planes";
    /// Channel compositing on an image without a channel axis, or a channel past its last.
    NoChannels => "disabled-no-channels", "science", "channels";
    /// The coordinate grid on an image without celestial coordinates.
    NoCoordinates => "disabled-no-coordinates", "science", "coordinates";
    /// A SAMP action while not connected to a hub.
    SampOff => "disabled-samp-off", "science", "astronomy";
    /// Plane levelling on an image that is not a height map.
    NotHeightMap => "disabled-not-height-map", "maps", "terrain";
    /// Hillshade on an image that is not a georeferenced elevation model.
    NotElevation => "disabled-not-elevation", "maps", "terrain";
    /// Wiggle traces on an image that is not a seismic section.
    NotSeismic => "disabled-not-seismic", "science", "special-data";
    /// An ion image on an image that is not a mass spectrometry image.
    NotMassSpec => "disabled-not-mass-spec", "science", "mass-spec";
    /// A thermal action on an image that is not a thermal image.
    NotThermal => "disabled-not-thermal", "science", "thermal";
    NotThermalSequence => "disabled-not-thermal-sequence", "science", "thermal";
    /// New emissivity on a thermal image that stores temperatures, not calibrated counts.
    NotRadiometric => "disabled-not-radiometric", "science", "thermal";
    /// The visible photo of an image without one.
    NoVisiblePhoto => "disabled-no-visible-photo", "science", "thermal";
    /// A mesh wireframe on an image that is not an unstructured mesh.
    NotUgrid => "disabled-not-ugrid", "science", "special-data";
    /// Debayering an image that is not a colour camera's raw frame.
    NotBayer => "disabled-not-bayer", "science", "camera-frames";
    /// Annotations shown or hidden on an image without any.
    NoAnnotations => "disabled-no-annotations", "science", "annotations";
    /// Stain separation on an image that is not a colour image.
    NotColourImage => "disabled-not-colour-image", "pathology", "stains";
    /// A stain action while stain separation is off.
    StainsOff => "disabled-stains-off", "pathology", "stains";
    /// Estimating stains without a drawn region.
    NoRegion => "disabled-no-region", "pathology", "estimate";
    /// Exporting annotations from an image without any.
    NoSlideAnnotations => "disabled-no-slide-annotations", "pathology", "export";
    /// A label volume with neither a volume in MPR nor a medical image to lay it over.
    NoLabelTarget => "disabled-no-label-target", "medical", "label-volumes";
    /// Layers, paper and lineweights on an image that is not a drawing.
    NotDrawing => "disabled-not-drawing", "drawing", "";
    /// Orbit, views and the other mesh actions on a file that is not a 3D mesh.
    NotMesh => "disabled-not-mesh", "mesh", "";
    /// Point size, colouring and eye-dome lighting on a file that is not a point cloud.
    NotPoints => "disabled-not-points", "mesh", "point-clouds";
    /// Point size or streamline thickness, and the colouring, on a file that is neither a
    /// point cloud nor a tractogram.
    NotPointsOrTracts => "disabled-not-points-or-tracts", "mesh", "point-clouds";
    /// Streamline opacity on a file that is not a tractogram.
    NotTracts => "disabled-not-tracts", "mesh", "point-clouds";
    /// Style and colouring of a molecule on a file that is not a molecular structure.
    NotMolecule => "disabled-not-molecule", "mesh", "molecules";
    /// Moving the light and the shading modes on an image that is not a reflectance
    /// transformation image.
    NotRelightable => "disabled-not-relightable", "relight", "";
    /// Import… with no memory card, camera or phone connected.
    NoImportSource => "disabled-no-import-source", "files", "import";
    /// Renaming, copying, rotating or sharing a page inside an archive.
    InArchiveFile => "disabled-in-archive-file", "reading", "archives";
    /// Rotating the file itself in a format whose files cannot record a rotation.
    NotRotatable => "disabled-not-rotatable", "files", "rotate";
    /// Wallpaper, sharing or email where the desktop offers no way to do it.
    NotOnThisDesktop => "disabled-not-on-this-desktop", "files", "share";
    /// Seeking and the sound's controls on a file that is not a movie.
    NotVideo => "disabled-not-video", "video", "";
    /// The volume and mute of a movie without sound (or whose sound can't play).
    NoSound => "disabled-no-sound", "video", "sound";
    /// The sound page's visuals on a file that is not a sound file.
    NotSoundFile => "disabled-not-sound-file", "video", "sound-visuals";
    /// Layers, coordinates and measuring on an image that is not a map (or with Maps off).
    NotMap => "disabled-not-map", "maps", "";
    /// Coordinates and measuring on a map whose positions can't be turned into longitude
    /// and latitude (no coordinate system).
    NoLonLat => "disabled-no-lonlat", "maps", "coordinates";
    /// The attribute table of a map without features (a raster).
    NoFeatures => "disabled-no-features", "maps", "attributes";
    /// An elevation profile over a map without heights.
    NoHeights => "disabled-no-heights", "maps", "profile";
    /// A time slider on a map without time steps or a timed track.
    NoTime => "disabled-no-time", "maps", "time";
    /// Wind arrows on a map without u and v wind components.
    NoWind => "disabled-no-wind", "maps", "time";
    /// Photos on a track, or geotagging, without a GPS track that has times.
    NoTrack => "disabled-no-track", "maps", "photos";
    /// Weather colours on a map that is not a grid of values.
    NotGrid => "disabled-not-grid", "maps", "time";
    /// Develop on a file that is not a still photo.
    NotPhoto => "disabled-not-photo", "develop", "";
    /// Develop while the Medical or Science profile is on.
    DevelopOff => "disabled-develop-off", "develop", "profiles";
}

/// Develop is off: the Medical or Science profile is on (DESIGN §6.10).
pub fn develop_off(st: &ActionState) -> bool {
    st.profile.develop_hidden()
}

/// The image shown is a still photo Develop can edit: not a medical, scientific, drawing,
/// 3D, map, relightable, animated or movie file.
fn is_photo(st: &ActionState) -> bool {
    !(st.dicom
        || st.medical.is_some()
        || st.volume
        || st.science.is_some()
        || st.drawing
        || st.mesh
        || st.relight
        || st.map.is_some()
        || st.animated
        || st.video)
}

impl Action {
    /// The message id of this action's name for a tractogram, for the actions a tractogram
    /// gives another meaning (the point size and colouring keys).
    pub(crate) fn tract_message_id(self) -> Option<&'static str> {
        match self {
            Action::MeshPointsBigger => Some("palette-tracts-thicker"),
            Action::MeshPointsSmaller => Some("palette-tracts-thinner"),
            Action::MeshPointColour => Some("palette-tracts-colour"),
            _ => None,
        }
    }

    /// Why this action cannot run now, or `None` when it can.
    pub fn disabled(self, st: &ActionState) -> Option<Disabled> {
        use ImageState as I;
        match self {
            // Compare's controls while it is open (the gallery's, DESIGN §6.3).
            Action::CompareSide
            | Action::CompareSplit
            | Action::CompareSplitDirection
            | Action::CompareOverlay
            | Action::CompareDifference
            | Action::CompareFlicker
            | Action::CompareAutoFlicker
            | Action::CompareAlign
            | Action::CompareMetricSpace
            | Action::CompareWholeImage
            | Action::CompareProbe
            | Action::CompareClose => (st.compare_images == 0).then_some(Disabled::CompareClosed),
            Action::CompareLink => match st.compare_images {
                0 => Some(Disabled::CompareClosed),
                2 => None,
                _ => Some(Disabled::CompareNotTwo),
            },
            // Another image to compare with: the current one first.
            Action::CompareWith => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Decoding | I::Ready | I::Failed => None,
            },
            // The gallery's quick look, compare and similar-shots review show channel views
            // of their own images.
            Action::ChannelRed
            | Action::ChannelGreen
            | Action::ChannelBlue
            | Action::ChannelAlpha
            | Action::ChannelLuma
                if st.gallery_views.is_some() =>
            {
                (self == Action::ChannelAlpha && st.gallery_views == Some(false))
                    .then_some(Disabled::NoAlpha)
            }
            // Window-level actions never depend on the image.
            Action::Open
            | Action::OpenAddress
            | Action::OpenStream
            | Action::Paste
            | Action::Settings
            | Action::CustomiseToolbar
            | Action::Palette
            | Action::CheatSheet
            | Action::About
            | Action::ShowWelcome
            | Action::Help
            | Action::JoinDiscord
            | Action::SaveDiagnostics
            | Action::Language
            | Action::ToggleTheme
            | Action::InterfaceLarger
            | Action::InterfaceSmaller
            | Action::InterfaceReset
            | Action::ToggleFullscreen
            // The colour-vision simulation covers the whole window.
            | Action::NextColourVision
            | Action::SimulateProtanopia
            | Action::SimulateDeuteranopia
            | Action::SimulateTritanopia
            | Action::SimulateAchromatopsia
            // With one monitor it says so (a toast) rather than doing nothing.
            | Action::ViewerOnOtherMonitor
            | Action::Hud
            | Action::Back
            | Action::Undo
            | Action::Redo
            // Settings files are chosen in a dialog.
            | Action::ExportSettings
            | Action::ImportSettings
            // DICOMweb servers are browsed with nothing open (a study is what it opens).
            | Action::DicomServers
            | Action::OmeroServers
            // Tethered capture says what is missing (no camera) itself.
            | Action::Tether
            | Action::TetherCapture
            | Action::Close
            | Action::Quit => None,
            // Cull mode works from the gallery as well as the viewer: entering it with
            // nothing to cull says so instead.
            Action::ToggleCull => None,
            // Printing needs something to print: the current image, or the folder, selection
            // or marked files for a contact sheet.
            Action::Print => (st.count == 0 && st.gallery_files == 0).then_some(Disabled::NoImage),
            // A client gallery, or the picks, need photos.
            Action::ClientGallery | Action::ImportClientPicks => {
                (st.count == 0 && st.gallery_files == 0).then_some(Disabled::NoImage)
            }
            // The key images are kept with the session's images; with none open there are
            // none (the actions say so when the set is empty).
            Action::CopyKeyImages | Action::ExportKeyImages => {
                (st.count == 0).then_some(Disabled::NoImage)
            }
            // Converting needs a file: the current image or the gallery's selection.
            Action::Convert | Action::ExportRecipes => (st.count == 0).then_some(Disabled::NoImage),
            // Importing needs a memory card, camera or phone with a DCIM folder.
            Action::Import => (!st.import_sources).then_some(Disabled::NoImportSource),
            // A slideshow needs something to show (stopping is handled before this check).
            Action::Slideshow | Action::ExportSlideshow => {
                (st.image == I::None).then_some(Disabled::NoImage)
            }
            Action::NextImage
            | Action::PrevImage
            | Action::FirstImage
            | Action::LastImage
            | Action::GoToImage => match st.count {
                0 => Some(Disabled::NoImage),
                1 => Some(Disabled::NoOtherImages),
                _ => None,
            },
            // Renaming many files works on the gallery's selection or the folder.
            Action::BatchRename => (st.count == 0).then_some(Disabled::NoImage),
            // A page inside an archive has no file of its own.
            Action::MoveToTrash | Action::OpenWith | Action::OpenAs if st.in_archive && st.image != I::None => {
                Some(Disabled::InArchive)
            }
            Action::Rename
            | Action::CopyToFolder
            | Action::MoveToFolder
            | Action::RotateFileLeft
            | Action::RotateFileRight
            | Action::SetWallpaper
            | Action::Share
            | Action::SendByEmail
            | Action::ShareWithZynk
            | Action::SendWithZynk
                if st.in_archive && st.image != I::None =>
            {
                Some(Disabled::InArchiveFile)
            }
            Action::RotateFileLeft | Action::RotateFileRight => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                _ if !st.rotatable => Some(Disabled::NotRotatable),
                _ => None,
            },
            // Never for want of Zynk: without it, they say what Zynk is and where to get it.
            // The gallery hands over its selection, with or without an image open.
            Action::ShareWithZynk | Action::SendWithZynk => match st.image {
                _ if st.gallery_files > 0 => None,
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                _ => None,
            },
            Action::SetWallpaper | Action::Share | Action::SendByEmail => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                _ if !match self {
                    Action::SetWallpaper => st.wallpaper,
                    Action::Share => st.share,
                    _ => st.email,
                } =>
                {
                    Some(Disabled::NotOnThisDesktop)
                }
                _ => None,
            },
            // The file itself: available once its header is read, even if decoding failed.
            Action::ToggleInfo
            | Action::CopyPath
            | Action::OpenWith
            | Action::OpenAs
            | Action::MoveToTrash
            | Action::Rename
            | Action::CopyToFolder
            | Action::MoveToFolder
            | Action::Reload => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Decoding | I::Ready | I::Failed => None,
            },
            // Measurements read the pixels.
            Action::CopyImage
            | Action::ReadCodes
            | Action::FindText
            | Action::FaceBoxes
            | Action::ExportFigure
            | Action::ToggleKeyImage
            | Action::Measure
            | Action::Markup
            | Action::CompareSheets
            | Action::MeasureCopyCsv
            | Action::CopyViewCommand
            | Action::CopyViewPython
            | Action::MeasureExport
            | Action::MeasureTemplates
            // The report: thermal for a thermal image, an inspection report for any other.
            | Action::ThermalReport => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening | I::Decoding => Some(Disabled::StillLoading),
                I::Failed => Some(Disabled::DecodeFailed),
                I::Ready => None,
            },
            // Pixel controls work on the known dimensions while decoding.
            Action::ZoomIn
            | Action::ZoomOut
            | Action::ZoomFit
            | Action::ZoomFill
            | Action::ZoomFitWidth
            | Action::ZoomFitHeight
            | Action::ZoomTo(_)
            | Action::ZoomRectangle
            | Action::RotateLeft
            | Action::RotateRight
            | Action::FlipHorizontal
            | Action::FlipVertical
            | Action::PanLeft
            | Action::PanRight
            | Action::PanUp
            | Action::PanDown
            | Action::PanPageLeft
            | Action::PanPageRight
            | Action::PanPageUp
            | Action::PanPageDown
            | Action::ToggleNavigator
            | Action::Loupe
            | Action::TogglePixelGrid
            | Action::ToggleClipping
            | Action::ToggleFocusPeaking
            | Action::ToggleHistogram
            | Action::TogglePixelInspector
            | Action::ToggleCrosshair
            | Action::AnnounceCursor
            | Action::ChannelRed
            | Action::ChannelGreen
            | Action::ChannelBlue
            | Action::ChannelAlpha
            | Action::ToggleFalseColour
            | Action::NextColourMap
            | Action::NextChannelColours
            | Action::ToggleHiLo
            | Action::ChannelLuma => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Failed => Some(Disabled::DecodeFailed),
                I::Decoding | I::Ready if self == Action::ChannelAlpha && !st.has_alpha => {
                    Some(Disabled::NoAlpha)
                }
                I::Decoding | I::Ready => None,
            },
            Action::PlayPause
            | Action::NextFrame
            | Action::PrevFrame
            | Action::FasterPlayback
            | Action::SlowerPlayback
            | Action::NormalPlayback
            | Action::ToggleLoopForever => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Failed => Some(Disabled::DecodeFailed),
                I::Decoding | I::Ready if st.animated => None,
                I::Decoding | I::Ready => Some(Disabled::NotAnimated),
            },
            Action::PlayPauseVideo
            | Action::SeekForward
            | Action::SeekBack
            | Action::VolumeUp
            | Action::VolumeDown
            | Action::ToggleMute
            | Action::VideoFullscreen
            | Action::SeekForwardLong
            | Action::SeekBackLong
            | Action::SeekPercent(_)
            | Action::CycleSubtitles
            | Action::OpenSubtitle
            | Action::GenerateSubtitles
            | Action::SaveSubtitles
            | Action::SubtitleEarlier
            | Action::SubtitleLater
            | Action::SubtitleEarlierFine
            | Action::SubtitleLaterFine
            | Action::SubtitleStepPrev
            | Action::SubtitleStepNext
            | Action::SubtitleUp
            | Action::SubtitleDown
            | Action::SubtitleBigger
            | Action::SubtitleSmaller
            | Action::SubtitleResetPlace
            | Action::SubtitleStyle
            | Action::CycleSound
            | Action::StreamQuality
            | Action::DialogueBoost
            | Action::Equaliser
            | Action::SoundEarlier
            | Action::SoundLater
            | Action::SoundEarlierFine
            | Action::SoundLaterFine
            | Action::ResetDelays
            | Action::PrevChapter
            | Action::NextChapter
            | Action::AbLoop
            | Action::SaveFrame
            | Action::CopyFrame
            | Action::MiniPlayer
            | Action::CycleAspect
            | Action::CycleCrop
            | Action::GoToTime
            | Action::VideoPicture
            | Action::CastMenu => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Failed => Some(Disabled::DecodeFailed),
                I::Decoding | I::Ready if !st.video => Some(Disabled::NotVideo),
                I::Decoding | I::Ready
                    if !st.sound
                        && matches!(
                            self,
                            Action::VolumeUp
                                | Action::VolumeDown
                                | Action::ToggleMute
                                | Action::CycleSound
                                | Action::DialogueBoost
                                | Action::Equaliser
                                | Action::SoundEarlier
                                | Action::SoundLater
                                | Action::SoundEarlierFine
                                | Action::SoundLaterFine
                        ) =>
                {
                    Some(Disabled::NoSound)
                }
                // Play mode is for pictures: a sound file's page has nothing to fill the screen.
                I::Decoding | I::Ready if st.sound_file && matches!(self, Action::VideoFullscreen) => {
                    Some(Disabled::NotVideo)
                }
                I::Decoding | I::Ready => None,
            },
            // Repeat and shuffle are settings; the queue panel and saving it say when it is
            // empty.
            Action::CycleRepeat
            | Action::ToggleShuffle
            | Action::ToggleQueue
            | Action::SaveQueue => None,
            Action::NextTrack | Action::PrevTrack => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening | I::Failed | I::Decoding | I::Ready if !st.video => {
                    Some(Disabled::NotVideo)
                }
                _ => None,
            },
            // The current file goes into the queue (the gallery adds its selection itself).
            Action::QueuePlayNext | Action::QueueAdd => match st.image {
                I::None => Some(Disabled::NoImage),
                _ => None,
            },
            Action::CycleSoundVisuals | Action::ToggleLyrics => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Failed => Some(Disabled::DecodeFailed),
                I::Decoding | I::Ready if !st.sound_file => Some(Disabled::NotSoundFile),
                I::Decoding | I::Ready => None,
            },
            // Reading order and spreads are view preferences: always available.
            Action::ToggleRightToLeft | Action::ToggleSpreads | Action::ToggleContents => None,
            // The gallery shows the folder (or an empty one with a hint).
            Action::ToggleGallery => None,
            Action::CustomiseKeys => None,
            // Develop edits still photos, and is off where journal-integrity rules forbid
            // local changes (the Medical and Science profiles).
            Action::ToggleDevelop => {
                if develop_off(st) {
                    return Some(Disabled::DevelopOff);
                }
                match st.image {
                    I::None => Some(Disabled::NoImage),
                    I::Opening => Some(Disabled::StillLoading),
                    I::Failed => Some(Disabled::DecodeFailed),
                    I::Decoding | I::Ready if !is_photo(st) => Some(Disabled::NotPhoto),
                    I::Decoding | I::Ready => None,
                }
            }
            // Develop's own keys run only in Develop, which checks the image itself.
            Action::DevelopBeforeAfter
            | Action::DevelopSplit
            | Action::DevelopFocusPanel
            | Action::DevelopNextRegion
            | Action::DevelopPrevRegion
            | Action::DevelopExport
            | Action::DevelopExportRecipes
            | Action::DevelopCrop
            | Action::DevelopSpotOutlines
            | Action::DevelopSpotCrosshair
            | Action::DevelopScopes
            | Action::DevelopClipDisplay
            | Action::DevelopClipScene
            | Action::DevelopClipGamut
            | Action::DevelopSoftProof
            | Action::DevelopSpeedExposure
            | Action::DevelopSpeedContrast
            | Action::DevelopSpeedHighlights
            | Action::DevelopSpeedShadows
            | Action::DevelopSpeedWhites
            | Action::DevelopSpeedBlacks
            | Action::DevelopSpeedTemperature
            | Action::DevelopSpeedVibrance
            | Action::DevelopSnapshot
            | Action::DevelopCompressHistory
            | Action::DevelopNewVersion
            | Action::DevelopNewStyle
            | Action::DevelopBrushSmaller
            | Action::DevelopBrushLarger
            | Action::DevelopBrushSofter
            | Action::DevelopBrushHarder
            | Action::DevelopBrushErase
            | Action::DevelopMaskOverlay => None,
            // Copy, paste, sync and match work on the photo shown, the gallery's selection or
            // the picks; Develop checks them itself and says why not.
            Action::DevelopCopyEdit
            | Action::DevelopCopySelective
            | Action::DevelopPasteEdit
            | Action::DevelopPasteSelective
            | Action::DevelopPastePicks
            | Action::DevelopSetReference
            | Action::DevelopMatchReference
            | Action::DevelopSync
            | Action::DevelopSyncSelective
            | Action::DevelopNextCapture
            | Action::DevelopAutoRules
            | Action::DevelopImportStyle
            | Action::DevelopMergeHdr
            | Action::DevelopMergePano
            | Action::DevelopFocusStack => None,
            // Cull keys only run in cull mode, which checks the current image itself.
            Action::CullPick
            | Action::CullReject
            | Action::CullUnflag
            | Action::CullRate(_)
            | Action::CullLabel(_)
            | Action::CullCopyTo(_)
            | Action::CullMoveTo(_)
            | Action::CullBurst
            | Action::CullKeepBest
            | Action::CullFollow
            | Action::CullSubframes
            | Action::CullSortNext
            | Action::CullSortReverse
            | Action::CullMetricNext
            | Action::CullSigmaDown
            | Action::CullSigmaUp
            | Action::CullAutoReject
            | Action::CullMoveRejects
            | Action::CullSameStretch => None,
            // The Library shows its locations (or how to add one).
            Action::ToggleLibrary => None,
            // The panel says what to select when nothing is.
            Action::Keywords => None,
            // The panel says what to select when nothing is.
            Action::EditMetadata => None,
            Action::WindowLevelTool
            | Action::WindowPreset(_)
            | Action::InvertGrey
            | Action::MeasureLength
            | Action::MeasureAngle
            | Action::MeasureEllipse
            | Action::MeasureRectangle
            | Action::ClearMeasurements
            | Action::ToggleOverlays
            | Action::ToggleOrientationLabels => {
                st.medical.is_none().then_some(Disabled::NotMedical)
            }
            Action::NextPresentationState => match st.medical {
                None => Some(Disabled::NotMedical),
                Some(_) => (st.presentation_states == 0).then_some(Disabled::NoPresentationState),
            },
            // A volume needs a series (series mode), a multi-frame file or a file that is a
            // volume (the app finds which); it says why when the slices cannot stack (too
            // few, not parallel, gaps).
            Action::Mpr => (!st.volume && !st.mpr).then_some(Disabled::NoVolume),
            // MPR's own tools act on the open views.
            Action::MprSlab
            | Action::MprTractSlab
            | Action::MprPreset
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
            | Action::MprDental
            | Action::MprNerve => match st.mpr {
                true => None,
                false if st.volume => Some(Disabled::MprClosed),
                false => Some(Disabled::NoVolume),
            },
            // Linked views need series; their settings need the viewports open.
            Action::CompareSeries => match st.medical {
                _ if st.comparing => None,
                None => Some(Disabled::NotMedical),
                Some(_) if st.series == 0 => Some(Disabled::NoOtherSeries),
                Some(_) => None,
            },
            Action::CompareLayout
            | Action::NextViewport
            | Action::PrevViewport
            | Action::ReferenceLines
            | Action::SyncWindow
            | Action::SyncZoom
            | Action::LinkSeries
            | Action::AlignSeries => (!st.comparing).then_some(Disabled::NotComparing),
            // The volume's analyses: MPR says why when there is nothing to fuse or measure.
            Action::Fusion
            | Action::FusionColormap
            | Action::FusionBlend
            | Action::FusionNudge
            | Action::Tmtv
            | Action::TmtvThreshold => match st.medical {
                None => Some(Disabled::NotMedical),
                Some(n) if n < 3 && st.series == 0 => Some(Disabled::NoVolume),
                Some(_) => None,
            },
            Action::Segments | Action::SegmentOpacity => {
                (st.medical.is_none() && !st.mpr).then_some(Disabled::NotMedical)
            }
            // A curve needs frames or a series of time points.
            Action::TimeCurve | Action::ExportCurve => match st.medical {
                None => Some(Disabled::NotMedical),
                Some(n) if n < 2 && st.series == 0 => Some(Disabled::OneFrame),
                Some(_) => None,
            },
            // One frame per file in series mode: they step and play the series' files.
            Action::NextSlice | Action::PrevSlice | Action::Cine => match st.medical {
                None => Some(Disabled::NotMedical),
                Some(n) if n < 2 && st.series == 0 => Some(Disabled::OneFrame),
                Some(_) => None,
            },
            Action::ScienceStretch
            | Action::ScienceLimits
            | Action::ScienceReset
            | Action::ScienceImportLut
            | Action::ScienceFft
            | Action::ScienceClearCatalogues
            | Action::ScienceLoadRegions
            | Action::ScienceExportRegions
            | Action::ScienceEditRegions
            | Action::ScienceClearRegions
            | Action::PlateSolve
            | Action::PlateSolveSave => st.science.is_none().then_some(Disabled::NotScience),
            Action::SampSendImage => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.samp => Some(Disabled::SampOff),
                Some(_) => None,
            },
            Action::SampSendPosition => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.samp => Some(Disabled::SampOff),
                Some(s) if !s.celestial => Some(Disabled::NoCoordinates),
                Some(_) => None,
            },
            Action::ScienceNextPlane | Action::SciencePrevPlane => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if s.planes < 2 => Some(Disabled::OnePlane),
                Some(_) => None,
            },
            Action::ScienceNextAxis => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if s.axes < 2 => Some(Disabled::OneAxis),
                Some(_) => None,
            },
            Action::ScienceComposite => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if s.channels < 2 => Some(Disabled::NoChannels),
                Some(_) => None,
            },
            Action::ScienceChannel(n) => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.composite || u32::from(n) > s.channels.min(16) => {
                    Some(Disabled::NoChannels)
                }
                Some(_) => None,
            },
            Action::DrawingLayers
            | Action::DrawingAllLayers
            | Action::DrawingPaper
            | Action::DrawingLineweights => (!st.drawing).then_some(Disabled::NotDrawing),
            Action::ScienceGrid | Action::ScienceCatalogue | Action::SkySurvey => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.celestial => Some(Disabled::NoCoordinates),
                Some(_) => None,
            },
            Action::ToggleDebayer | Action::NextBayerPattern => {
                (!st.debayer).then_some(Disabled::NotBayer)
            }
            Action::ToggleAnnotations => (!st.annotations).then_some(Disabled::NoAnnotations),
            Action::StainSeparation => st.stain.is_none().then_some(Disabled::NotColourImage),
            Action::StainShow1
            | Action::StainShow2
            | Action::StainShow3
            | Action::StainMode
            | Action::StainNormalise
            | Action::StainReset => match st.stain {
                None => Some(Disabled::NotColourImage),
                Some(false) => Some(Disabled::StainsOff),
                Some(true) => None,
            },
            Action::StainEstimate | Action::StainReference => match st.stain {
                None => Some(Disabled::NotColourImage),
                Some(false) => Some(Disabled::StainsOff),
                Some(true) if !st.stain_region => Some(Disabled::NoRegion),
                Some(true) => None,
            },
            Action::ExportAnnotations => (!st.slide_annotations).then_some(Disabled::NoSlideAnnotations),
            Action::LabelVolume => (!st.mpr && st.medical.is_none()).then_some(Disabled::NoLabelTarget),
            // Imported shapes are drawn over the pixels.
            Action::ImportAnnotations => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Ready => None,
                I::Failed => Some(Disabled::DecodeFailed),
                I::Opening | I::Decoding => Some(Disabled::StillLoading),
            },
            Action::ScienceLevel => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.levelling => Some(Disabled::NotHeightMap),
                Some(_) => None,
            },
            Action::ScienceHillshade => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.relief => Some(Disabled::NotElevation),
                Some(_) => None,
            },
            Action::ScienceWiggle => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.seismic => Some(Disabled::NotSeismic),
                Some(_) => None,
            },
            Action::ScienceWireframe => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.ugrid => Some(Disabled::NotUgrid),
                Some(_) => None,
            },
            Action::ScienceIonImage => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.mass_spec => Some(Disabled::NotMassSpec),
                Some(_) => None,
            },
            Action::ScienceTemperatureUnit
            | Action::ScienceIsotherm
            | Action::ScienceExportTemperatures => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.thermal => Some(Disabled::NotThermal),
                Some(_) => None,
            },
            Action::ScienceThermalPlay | Action::ScienceThermalSeriesCsv => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.thermal => Some(Disabled::NotThermal),
                Some(s) if !s.sequence => Some(Disabled::NotThermalSequence),
                Some(_) => None,
            },
            Action::ScienceThermalParameters => match st.science {
                None => Some(Disabled::NotScience),
                Some(s) if !s.thermal => Some(Disabled::NotThermal),
                Some(s) if !s.radiometric => Some(Disabled::NotRadiometric),
                Some(_) => None,
            },
            Action::ToggleThermalVisible => {
                (!st.thermal_visible).then_some(Disabled::NoVisiblePhoto)
            }
            Action::MeshOrbitLeft
            | Action::MeshOrbitRight
            | Action::MeshOrbitUp
            | Action::MeshOrbitDown
            | Action::MeshViewFront
            | Action::MeshViewTop
            | Action::MeshViewSide
            | Action::MeshViewIso
            | Action::MeshShading
            | Action::MeshProjection
            | Action::MeshGrid => {
                (!st.mesh).then_some(Disabled::NotMesh)
            }
            Action::MeshPointsBigger | Action::MeshPointsSmaller | Action::MeshPointColour => {
                (!(st.mesh && (st.points || st.tracts))).then_some(Disabled::NotPointsOrTracts)
            }
            Action::MeshEdl => (!(st.mesh && st.points)).then_some(Disabled::NotPoints),
            Action::MeshTractOpacity => (!(st.mesh && st.tracts)).then_some(Disabled::NotTracts),
            Action::MeshWalk
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
            | Action::MeshPmi => (!st.mesh).then_some(Disabled::NotMesh),
            Action::MeshMolStyle | Action::MeshMolColour => {
                (!(st.mesh && st.molecule)).then_some(Disabled::NotMolecule)
            }
            Action::RelightLeft
            | Action::RelightRight
            | Action::RelightUp
            | Action::RelightDown
            | Action::RelightMode
            | Action::RelightReset
            | Action::RelightWidget => (!st.relight).then_some(Disabled::NotRelightable),
            Action::MapLayers
            | Action::MapAddLayer
            | Action::MapZoomToLayer
            | Action::MapUnits
            | Action::MapOverview
            | Action::MapServices => st.map.is_none().then_some(Disabled::NotMap),
            Action::MapSlope
            | Action::MapAspect
            | Action::MapContours
            | Action::MapTerrain3d => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.heights => Some(Disabled::NoHeights),
                Some(_) => None,
            },
            Action::MapTime
            | Action::MapPlay => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.time => Some(Disabled::NoTime),
                Some(_) => None,
            },
            Action::MapWind => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.wind => Some(Disabled::NoWind),
                Some(_) => None,
            },
            Action::MapWeatherColours => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.grid => Some(Disabled::NotGrid),
                Some(_) => None,
            },
            Action::MapPhotosOnTrack
            | Action::MapGeotag => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.track => Some(Disabled::NoTrack),
                Some(_) => None,
            },
            Action::MapExport
            | Action::MapPrint
            | Action::MapCopyExtent => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.lonlat => Some(Disabled::NoLonLat),
                Some(_) => None,
            },
            Action::MapAttributeTable => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.features => Some(Disabled::NoFeatures),
                Some(_) => None,
            },
            Action::MapCoordinateFormat
            | Action::MapCopyCoordinate
            | Action::MapGoToCoordinate
            | Action::MapMeasureDistance
            | Action::MapMeasureArea => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.lonlat => Some(Disabled::NoLonLat),
                Some(_) => None,
            },
            Action::MapElevationProfile => match st.map {
                None => Some(Disabled::NotMap),
                Some(m) if !m.lonlat => Some(Disabled::NoLonLat),
                Some(m) if !m.heights => Some(Disabled::NoHeights),
                Some(_) => None,
            },
            // The tags are read from the file: available once its header is read.
            Action::DicomTags => match st.image {
                I::None => Some(Disabled::NoImage),
                I::Opening => Some(Disabled::StillLoading),
                I::Decoding | I::Ready | I::Failed if st.dicom => None,
                I::Decoding | I::Ready | I::Failed => Some(Disabled::NotDicom),
            },
            // The medium is found from the file's folder.
            Action::Studies => match st.image {
                I::None => Some(Disabled::NoImage),
                _ if st.dicom => None,
                _ => Some(Disabled::NotDicom),
            },
            // Exports read the files: available once the header says DICOM.
            // A quiz runs on the key images or a teaching case folder picked.
            Action::TeachingQuiz => None,
            Action::ExportAnonymised
            | Action::ExportPatientDisc
            | Action::ExportTeachingFile
            | Action::SendToPacs => {
                match st.image {
                    I::None => Some(Disabled::NoImage),
                    I::Opening => Some(Disabled::StillLoading),
                    I::Decoding | I::Ready | I::Failed if st.dicom => None,
                    I::Decoding | I::Ready | I::Failed => Some(Disabled::NotDicom),
                }
            }
            // Masks are drawn over the pixels.
            Action::MaskBurnedInText => match st.image {
                I::None => Some(Disabled::NoImage),
                _ if !st.dicom && st.image != I::Opening => Some(Disabled::NotDicom),
                I::Ready => None,
                I::Failed => Some(Disabled::DecodeFailed),
                I::Opening | I::Decoding => Some(Disabled::StillLoading),
            },
            Action::NextSeries | Action::PrevSeries => {
                (st.series < 2).then_some(Disabled::NoOtherSeries)
            }
            // Pages of the file; in medical mode the same keys step its frames.
            Action::NextPage | Action::PrevPage | Action::FirstPage | Action::LastPage => {
                match st.medical {
                    Some(n) if n >= 2 => None,
                    _ if st.pages >= 2 => None,
                    _ if st.image == I::None => Some(Disabled::NoImage),
                    _ => Some(Disabled::OnePage),
                }
            }
        }
    }

    pub fn enabled(self, st: &ActionState) -> bool {
        self.disabled(st).is_none()
    }
}

#[cfg(test)]
mod tests;
