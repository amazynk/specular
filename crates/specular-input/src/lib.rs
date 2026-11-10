//! Every behaviour is an [`Action`]. Keys, mouse, the toolbar, the command palette and the CLI
//! all dispatch actions. This crate defines the actions, key combinations, the default viewer
//! keymap (DESIGN §11), its per-mode layers, presets and `keymap.toml` (load, merge, export),
//! the key recorder of Settings → Keys and its conflict checks.

mod conflict;
pub mod device;
mod enabled;
pub mod fixed;
mod keymap;
mod names;
mod presets;
mod profile;
mod record;
mod registry;
mod typed;

pub use conflict::{Conflict, Reserved, conflicts, reserved};
pub use device::{DEVICE_KEYS, PadButton, device_default, puck_combo};
pub use enabled::{ActionState, Disabled, ImageState, MapState, ScienceState, develop_off};
pub use keymap::{
    Displace, Key, KeyCombo, KeySeq, Keymap, KeymapEdit, KeymapError, KeymapProblem, Mods,
    NamedKey, Step,
};
pub use presets::Preset;
pub use profile::{Capabilities, Capability, Profile, Tier, Visibility, visibility};
pub use record::{RecordEvent, Recorder};
pub use typed::{Os, is_altgr_text, typed_text};

/// A place in the manual: a page and a section of it (empty: its top). The manual
/// (`specular-help`) checks that every topic named here exists.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Topic {
    pub page: &'static str,
    pub anchor: &'static str,
}

/// A keymap layer (DESIGN §11: keymaps are per mode). A mode's bindings come before those of
/// its parent, which still apply where the mode binds nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mode {
    /// The image viewer (and everything that has no mode of its own).
    Viewer,
    /// Cull mode (DESIGN §6.3), over the viewer's bindings.
    Cull,
    /// A grey medical (DICOM) image is shown (DESIGN §11): window/level, presets on the
    /// digits and slice stepping, over the viewer's bindings.
    Medical,
    /// A scientific image (FITS, XISF, DM, MRC, Gwyddion, Igor) is shown: stretch, limits, the planes of its further
    /// axes, channel compositing and the coordinate grid, over the viewer's bindings.
    Science,
    /// A drawing (DXF) is shown: the layers panel, the paper colour and lineweights, over
    /// the viewer's bindings.
    Drawing,
    /// A 3D mesh (STL, OBJ, PLY, glTF, 3MF) is shown in the 3D view: orbit, standard views,
    /// shading, projection and the ground grid, over the viewer's bindings.
    Mesh,
    /// A reflectance transformation image (PTM, RTI, Relight) is shown: the light's
    /// direction, the shading modes and the light widget, over the viewer's bindings.
    Relight,
    /// A movie is shown: Space plays and pauses, seeking, the sound's volume and mute,
    /// over the viewer's bindings.
    Video,
    /// A map is shown with the Maps tools on (a georeferenced image or a vector map): the
    /// layer panel, the attribute table, coordinates and measuring, over the drawing or
    /// science layer the map has and the viewer's bindings.
    Map,
    /// Develop (DESIGN §6.10): before/after, the panel and the speed-edit keys, over the
    /// viewer's bindings.
    Develop,
}

impl Mode {
    pub const ALL: [Mode; 10] = [
        Mode::Viewer,
        Mode::Cull,
        Mode::Medical,
        Mode::Science,
        Mode::Drawing,
        Mode::Mesh,
        Mode::Relight,
        Mode::Video,
        Mode::Map,
        Mode::Develop,
    ];

    /// The table name in `keymap.toml`.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Viewer => "viewer",
            Mode::Cull => "cull",
            Mode::Medical => "medical",
            Mode::Science => "science",
            Mode::Drawing => "drawing",
            Mode::Mesh => "mesh",
            Mode::Relight => "relight",
            Mode::Video => "video",
            Mode::Map => "maps",
            Mode::Develop => "develop",
        }
    }

    pub fn from_id(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.id() == s.trim())
    }

    /// Fluent message ID of the mode's name.
    pub fn message_id(self) -> &'static str {
        match self {
            Mode::Viewer => "keys-mode-viewer",
            Mode::Cull => "keys-mode-cull",
            Mode::Medical => "keys-mode-medical",
            Mode::Science => "keys-mode-science",
            Mode::Drawing => "keys-mode-drawing",
            Mode::Mesh => "keys-mode-mesh",
            Mode::Relight => "keys-mode-relight",
            Mode::Video => "keys-mode-video",
            Mode::Map => "keys-mode-map",
            Mode::Develop => "keys-mode-develop",
        }
    }

    /// The mode whose bindings apply under this one's.
    pub fn parent(self) -> Option<Mode> {
        match self {
            Mode::Viewer => None,
            Mode::Cull
            | Mode::Medical
            | Mode::Science
            | Mode::Drawing
            | Mode::Mesh
            | Mode::Relight
            | Mode::Video
            | Mode::Map
            | Mode::Develop => Some(Mode::Viewer),
        }
    }

    /// This mode, then its parents: the order key presses are looked up in.
    pub fn chain(self) -> Vec<Mode> {
        let mut v = vec![self];
        while let Some(p) = v.last().and_then(|m| m.parent()) {
            v.push(p);
        }
        v
    }

    pub(crate) fn index(self) -> usize {
        Mode::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }

    /// The actions that belong to this mode, in registry order.
    pub fn actions(self) -> impl Iterator<Item = Action> {
        Action::BINDABLE
            .iter()
            .copied()
            .filter(move |a| a.mode() == self)
    }
}

/// Everything the viewer can do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Action {
    NextImage,
    PrevImage,
    FirstImage,
    LastImage,
    /// Type the number of the image to show (the position readout turns into a field).
    GoToImage,
    ZoomIn,
    ZoomOut,
    ZoomFit,
    ZoomFill,
    /// Fit the image's width (or height) to the window; kept page to page in a book.
    ZoomFitWidth,
    ZoomFitHeight,
    /// Zoom to a percentage (100 = one image pixel per device pixel).
    ZoomTo(f32),
    /// Start drawing a zoom rectangle (also `z` + drag).
    ZoomRectangle,
    RotateLeft,
    RotateRight,
    FlipHorizontal,
    FlipVertical,
    PanLeft,
    PanRight,
    PanUp,
    PanDown,
    PanPageLeft,
    PanPageRight,
    PanPageUp,
    PanPageDown,
    ToggleFullscreen,
    /// Start or stop the slideshow (DESIGN.md §6.5).
    Slideshow,
    /// Show the current image full screen on another monitor, following the main window's
    /// selection; again to close it (DESIGN.md §6.5, multi-monitor).
    ViewerOnOtherMonitor,
    /// Renders the slideshow into an MP4 video (the export dialog, DESIGN.md §6.5).
    ExportSlideshow,
    /// Enter or leave cull mode (DESIGN.md §6.3).
    ToggleCull,
    /// Esc, one layer per press: close the topmost overlay, else reset the zoom to fit, else
    /// leave full screen.
    Back,
    ToggleInfo,
    ToggleNavigator,
    /// Hold to show the loupe.
    Loupe,
    TogglePixelGrid,
    /// Inspection overlays and panels (DESIGN.md §6.2).
    ToggleClipping,
    ToggleFocusPeaking,
    ToggleHistogram,
    TogglePixelInspector,
    /// The keyboard crosshair: arrow keys move a crosshair over the image that drives the
    /// readouts, the pixel inspector and measuring as the pointer does.
    ToggleCrosshair,
    /// Reads the value under the cursor or crosshair out to a screen reader.
    AnnounceCursor,
    /// Channel isolation (DESIGN.md §6.2): one channel in grey; the same key again goes back
    /// to the image. The alpha key steps grey → over a checkerboard → over a matte colour.
    ChannelRed,
    ChannelGreen,
    ChannelBlue,
    ChannelAlpha,
    ChannelLuma,
    /// Colour-vision-safe display (DESIGN.md §6.2): false colour through the chosen
    /// colour map (with its colour bar), the next map, the next channel-colour preset
    /// and HiLo (pixels at the bit depth's maximum and minimum).
    ToggleFalseColour,
    NextColourMap,
    NextChannelColours,
    ToggleHiLo,
    /// Colour-vision simulation of the whole window (never of exports): the next one,
    /// or one in particular (pressing it again turns the simulation off).
    NextColourVision,
    SimulateProtanopia,
    SimulateDeuteranopia,
    SimulateTritanopia,
    SimulateAchromatopsia,
    CopyImage,
    CopyPath,
    /// Reads the QR codes in the picture: their text to copy, links to open.
    ReadCodes,
    /// Reads the text in the picture (the Local AI pack's Text models): shown over it, to copy.
    FindText,
    /// Shows or hides the faces Local AI found on the picture (People's names, Culling's
    /// closed eyes).
    FaceBoxes,
    /// Opens the image in another application, or runs a user command, from a menu.
    OpenWith,
    /// Asks which format to read the file as, for an extension several formats share.
    OpenAs,
    /// Prints the image or a contact sheet, or saves them as a PDF (the print dialog).
    Print,
    /// Writes the photos as a static web page folder for a client, with picks the client
    /// sends back (the client gallery dialog).
    ClientGallery,
    /// Marks the photos a client picked in their gallery (the client picks dialog).
    ImportClientPicks,
    /// The view as a figure (SVG, PDF, 16-bit PNG or TIFF) with a scale bar, colour bar,
    /// inset, split channels, labels, measurements and provenance (the figure dialog).
    ExportFigure,
    /// Marks the image as a key image, or unmarks it (key images last for the session).
    ToggleKeyImage,
    /// Copies the key images to the clipboard as one PNG.
    CopyKeyImages,
    /// Exports the key images as slides (PPTX) or a folder of PNGs.
    ExportKeyImages,
    /// Writes copies of the image or the selection in another format, size or name
    /// (the convert dialog; the sources are never changed).
    Convert,
    /// Exports the image or the selection with one or more saved export recipes.
    ExportRecipes,
    /// Copies new photos and videos from a memory card, camera or phone into the pictures
    /// folder (the import sheet).
    Import,
    /// Shows the clipboard's image, or opens the files or paths on the clipboard.
    Paste,
    MoveToTrash,
    /// Undoes the last change: a move to the trash, a rename, a copy or move to a folder, a
    /// rotation written to a file.
    Undo,
    /// Does again what the last undo took back.
    Redo,
    /// Renames the file in place (the field selects the name without its extension); its
    /// sidecars follow.
    Rename,
    /// Renames the gallery's selection (or the folder) from a pattern, with a live preview.
    BatchRename,
    /// Copies / moves the file or the selection to a folder chosen in a dialog or from the
    /// recent folders.
    CopyToFolder,
    MoveToFolder,
    /// Turns the file itself a quarter left / right: losslessly where the format allows,
    /// otherwise by its orientation tag (a RAW by its XMP sidecar).
    RotateFileLeft,
    RotateFileRight,
    /// Makes the image the desktop background.
    SetWallpaper,
    /// The system's share sheet for the file (Linux: the portal's application chooser).
    Share,
    /// A new email with the file attached, in the user's mail program.
    SendByEmail,
    /// Hands the file to the Zynk desktop app, to choose who gets it.
    ShareWithZynk,
    /// Sends the file with the Zynk command-line tool, to a contact or one of the user's devices.
    SendWithZynk,
    /// Saves every setting and the keymap's changes to a file the user chooses.
    ExportSettings,
    /// Reads settings from such a file, after showing what would change.
    ImportSettings,
    Open,
    /// Opens a web address: a Zarr store, a Cloud-Optimized GeoTIFF or COPC point cloud on
    /// a server, or a DICOMweb server.
    OpenAddress,
    /// Opens a network stream: a file on a web server, HLS, DASH, internet radio (a page
    /// through yt-dlp when the user turned it on).
    OpenStream,
    Settings,
    /// Settings → Viewing → Toolbar contents (the right-click menu of the toolbar has it too).
    CustomiseToolbar,
    Palette,
    CheatSheet,
    /// The About box (version, licence, data attributions).
    About,
    /// Back to the start screen (closing the image), with its "Things to try" cards on again.
    ShowWelcome,
    /// The manual, open at the page for what is on screen (F1).
    Help,
    /// Opens the invite to the Specular Discord server in the browser
    /// (`specular_core::links::DISCORD_INVITE`).
    JoinDiscord,
    /// Save a diagnostics bundle for a bug report (shows what goes in it first).
    SaveDiagnostics,
    Hud,
    Language,
    ToggleTheme,
    /// The interface (bars, panels, dialogs, text) larger, smaller or back to 100 %: the
    /// interface scale in Settings › Accessibility. The image's zoom is separate.
    InterfaceLarger,
    InterfaceSmaller,
    InterfaceReset,
    Reload,
    /// Close the image, or the gallery or library: back to the start screen, like the window's
    /// close button; on the start screen, quit.
    Close,
    Quit,
    /// Animations: play or pause.
    PlayPause,
    /// Animations: show the next / previous frame (pauses).
    NextFrame,
    PrevFrame,
    /// Animations: playback speed up / down one step, and back to 1×.
    FasterPlayback,
    SlowerPlayback,
    NormalPlayback,
    /// Animations: loop forever instead of the file's loop count (and back).
    ToggleLoopForever,
    /// Movies: play or pause (Space in video mode, where Space would otherwise go to the next
    /// image).
    PlayPauseVideo,
    /// Movies: jump 5 seconds forward / back.
    SeekForward,
    SeekBack,
    /// Movies: the sound one step louder / quieter, and muted or not.
    VolumeUp,
    VolumeDown,
    ToggleMute,
    /// Movies: fullscreen play mode (`f` while a movie is shown): the window fullscreen, the
    /// picture fitted on black, the bars and panels hidden; again (or Esc) goes back to the
    /// windowed layout as it was.
    VideoFullscreen,
    /// Movies: jump 10 seconds forward / back (J and L, as web players do).
    SeekForwardLong,
    SeekBackLong,
    /// Movies: jump to a tenth of the movie (`SeekPercent(30)`: 30 %; 0 the start).
    SeekPercent(u8),
    /// Movies: the next subtitle track (or off), and a subtitle file to add.
    CycleSubtitles,
    OpenSubtitle,
    /// Movies and sound: subtitles from the speech (Local AI), and saving them as a file.
    GenerateSubtitles,
    SaveSubtitles,
    /// Movies: subtitles 50 ms earlier / later, and 10 ms (fine).
    SubtitleEarlier,
    SubtitleLater,
    SubtitleEarlierFine,
    SubtitleLaterFine,
    /// Movies: shift the subtitle delay so the previous / next subtitle starts now.
    SubtitleStepPrev,
    SubtitleStepNext,
    /// Movies: subtitles higher / lower on the picture, larger / smaller, their place and
    /// size back to the defaults, and Settings at the subtitles' look.
    SubtitleUp,
    SubtitleDown,
    SubtitleBigger,
    SubtitleSmaller,
    SubtitleResetPlace,
    SubtitleStyle,
    /// Movies: the next sound track.
    CycleSound,
    /// A network stream: the next quality (automatic, then each variant).
    StreamQuality,
    /// Movies and sound files: dialogue boost (night mode) on or off.
    DialogueBoost,
    /// Movies and sound files: the equaliser's panel.
    Equaliser,
    /// Movies: the sound 50 ms earlier / later, and 10 ms (fine).
    SoundEarlier,
    SoundLater,
    SoundEarlierFine,
    SoundLaterFine,
    /// Movies: both delays back to zero.
    ResetDelays,
    /// Movies: the previous / next chapter.
    PrevChapter,
    NextChapter,
    /// Movies: A-B repeat: set A, then B (the part between loops), then clear.
    AbLoop,
    /// Movies: the frame on screen saved as a PNG, or copied.
    SaveFrame,
    CopyFrame,
    /// Movies: a small always-on-top window without its bars (again: back).
    MiniPlayer,
    /// Movies: the picture's shape (as stored, 4:3, 16:9, 1.85, 2.39) and a crop to it.
    CycleAspect,
    CycleCrop,
    /// Sound files, videos and the queue: the next / previous item (the queue's order,
    /// shuffled or not; else the folder's files of the same kind).
    NextTrack,
    PrevTrack,
    /// Repeat: off, all, one (sound files, videos and the queue).
    CycleRepeat,
    /// Shuffle: play in a random order, each item once a round.
    ToggleShuffle,
    /// The play queue panel.
    ToggleQueue,
    /// The current file (the gallery: the selection) plays after the current item / at the
    /// end of the queue.
    QueuePlayNext,
    QueueAdd,
    /// The queue saved as an M3U8 or XSPF playlist.
    SaveQueue,
    /// Sound files: the next set of visuals on the sound page (overview, meters, all).
    CycleSoundVisuals,
    /// Sound files: the lyrics column on the sound page, shown or hidden.
    ToggleLyrics,
    /// Movies (and sound files): a field over the time to type where to go (1:23:45, 50%,
    /// +30).
    GoToTime,
    /// Movies: the picture adjustments (brightness, contrast, saturation, gamma, hue).
    VideoPicture,
    /// Movies and sound files: "Play on…", the TVs and speakers on the network to play on.
    CastMenu,
    /// Reading mode: pages run right to left (manga), flipping the arrow keys and spreads.
    ToggleRightToLeft,
    /// Reading mode: two pages side by side, the first (cover) alone.
    ToggleSpreads,
    /// A book's table of contents (EPUB) in a side panel.
    ToggleContents,
    /// Switch between the image and the thumbnail gallery of its folder (DESIGN §10).
    ToggleGallery,
    /// Settings → Keys (the key binding configurator).
    CustomiseKeys,
    /// Cull mode: pick, reject, clear the flag.
    CullPick,
    CullReject,
    CullUnflag,
    /// Cull mode: rate 0 (none) to 5 stars.
    CullRate(u8),
    /// Cull mode: colour label 1–4 (red, yellow, green, blue); the same label again removes it.
    CullLabel(u8),
    /// Cull mode: copy / move the current item (with its sidecars) to folder slot 1–3
    /// (`cull.slot_1`‥`slot_3`).
    CullCopyTo(u8),
    CullMoveTo(u8),
    /// Cull mode: open the burst on screen, or close it on the frame shown.
    CullBurst,
    /// Cull mode: pick the frame shown and reject the rest of its burst.
    CullKeepBest,
    /// Cull mode: follow new files as they arrive (tethered shooting), or stop.
    CullFollow,
    /// Cull mode on astronomy subframes: show or hide the subframe table and chart.
    CullSubframes,
    /// Cull mode on subframes: sort the frames by the next column of the table, or reverse
    /// the order.
    CullSortNext,
    CullSortReverse,
    /// Cull mode on subframes: chart the next metric (the one auto-reject uses).
    CullMetricNext,
    /// Cull mode on subframes: lower or raise auto-reject's k (σ from the median).
    CullSigmaDown,
    CullSigmaUp,
    /// Cull mode on subframes: reject the frames auto-reject suggests (asks first).
    CullAutoReject,
    /// Cull mode on subframes: move the rejected frames to a subfolder (asks first).
    CullMoveRejects,
    /// Cull mode on subframes: every frame with the first one's stretch, or its own.
    CullSameStretch,
    /// Switch to the Library (every watched location, DESIGN §10) and back.
    ToggleLibrary,
    /// The keyword panel (gallery, Library and cull mode): show or hide it.
    Keywords,
    /// The metadata editor (gallery, Library, cull mode and the viewer): show or hide it.
    EditMetadata,
    /// Medical mode (DESIGN.md §11): left-drag adjusts window/level instead of panning, until
    /// pressed again (right-drag always does).
    WindowLevelTool,
    /// Medical mode: a window/level preset, 0 the file's own, 9 the full value range, 1‥8
    /// the CT presets.
    WindowPreset(u8),
    /// Medical mode: invert the grey scale (after the window).
    InvertGrey,
    /// Medical mode: the next / previous frame (slice) of a multi-frame file.
    NextSlice,
    PrevSlice,
    /// Medical mode: play the frames of a multi-frame grey file at their stated rate, or stop.
    Cine,
    /// Medical mode: measuring tools (left drag or clicks; the label says how the size is
    /// calibrated).
    MeasureLength,
    MeasureAngle,
    MeasureEllipse,
    MeasureRectangle,
    /// Medical mode: remove the measurements of the image.
    ClearMeasurements,
    /// Medical mode: show or hide the overlay planes (shutters stay).
    ToggleOverlays,
    /// Medical mode: show or hide the orientation labels (L R A P S I) at the image's edges.
    ToggleOrientationLabels,
    /// Medical mode: apply the next presentation state (GSPS) that references the image,
    /// newest first, then none.
    NextPresentationState,
    /// Multiplanar reformatting of the volume shown (a DICOM series or multi-frame file, a
    /// research or industrial CT volume, a microscopy z-stack, a seismic or science cube):
    /// three orthogonal views with linked crosshairs, or back to the image. Any profile.
    Mpr,
    /// Medical mode, in MPR: the next slab (thin, MIP, MinIP, average).
    MprSlab,
    /// Medical mode, in MPR of a brain volume with streamlines beside it (TrackVis,
    /// MRtrix): the next thickness of the slab whose streamlines are drawn over each
    /// view, or none.
    MprTractSlab,
    /// Medical mode, in MPR: the 3D view's next transfer function preset.
    MprPreset,
    /// Medical mode, in MPR: the 3D view's next rendering (shaded, cinematic, MIP, MinIP).
    MprRender,
    /// Medical mode, in MPR: the 3D view's next clipping (none, the 2D views' box, the
    /// crosshair's corner cut away).
    MprClip,
    /// Medical mode, in MPR: saves the surface at the 3D view's threshold as an STL file.
    MprExportStl,
    /// In MPR: the active view fills the area, or the four views come back.
    MprMaximise,
    /// In MPR: saves the three orthogonal planes through the crosshair as PNG images.
    MprExportPlanes,
    /// In MPR: curved MPR along a curve clicked on a plane (straightened or stretched, with
    /// cross-sections along it), or back to the three planes.
    MprCurve,
    /// In curved MPR: straightened or stretched.
    MprCurveLayout,
    /// In curved MPR: saves the curve as a JSON sidecar, or loads one.
    MprCurveSave,
    MprCurveLoad,
    /// In MPR: saves each segment of the segmentations shown (or of a label volume) as its
    /// own STL file, with one 3MF file holding them all.
    MprExportSegments,
    /// Medical mode, in MPR: the dental layout (panoramic and cross-sections along the
    /// arch), or back.
    MprDental,
    /// Medical mode, in the dental layout: places points of the nerve canal.
    MprNerve,
    /// Medical mode, in series mode: several series side by side in linked viewports
    /// (synced scrolling, reference lines, 3D cursor), or back to the single image.
    CompareSeries,
    /// Medical mode, comparing series: the next viewport layout (1×2, 2×2, 3×3, 1×1).
    CompareLayout,
    /// Medical mode, comparing series: the next or previous viewport becomes the active one.
    NextViewport,
    PrevViewport,
    /// Medical mode, comparing series: shows or hides the active slice's reference lines on
    /// the other viewports.
    ReferenceLines,
    /// Medical mode, comparing series: the next window/level sync (off, same modality, all).
    SyncWindow,
    /// Medical mode, comparing series: zoom and pan by physical size in every viewport, or
    /// each on its own.
    SyncZoom,
    /// Medical mode, comparing series: how the active viewport's series links to the others
    /// (its Frame of Reference, its patient position, aligned at the slices shown, or not
    /// linked).
    LinkSeries,
    /// Medical mode, comparing series: aligns the other viewports' series (another Frame of
    /// Reference, such as a prior study) with the active one at the slices shown; kept for
    /// the session.
    AlignSeries,
    /// Science mode (FITS, XISF, DM, MRC, Gwyddion, Igor): the next stretch (linear, square root, logarithmic, asinh,
    /// histogram equalisation).
    ScienceStretch,
    /// Science mode: the next limits (zscale, min/max, percentiles).
    ScienceLimits,
    /// Science mode: the next / previous position on the axis being stepped (a cube's
    /// planes, a file's images).
    ScienceNextPlane,
    SciencePrevPlane,
    /// Science mode: step another axis with the plane keys.
    ScienceNextAxis,
    /// Science mode: composite the channel axis in colour, or show one plane in grey.
    ScienceComposite,
    /// Science mode: show or hide channel 1‥4 of the composite.
    ScienceChannel(u8),
    /// Science mode: the coordinate grid (sky coordinates from the WCS; coastlines and a
    /// longitude/latitude graticule on a georeferenced image).
    ScienceGrid,
    /// Science mode: plane levelling of height maps (scanning probe), for display.
    ScienceLevel,
    /// Science mode: relief shading (hillshade) of a georeferenced elevation model.
    ScienceHillshade,
    /// Science mode: seismic traces drawn as wiggles with the positive lobes filled
    /// (variable area) over the section, once they are a few pixels apart.
    ScienceWiggle,
    /// Science mode: the edges of an unstructured mesh (UGRID, ICON, MPAS) drawn over its
    /// faces, once they are a few pixels apart.
    ScienceWireframe,
    /// Science mode: the automatic look again (zscale, linear, the default channels).
    ScienceReset,
    /// Science mode: show single planes through an ImageJ colour table file (`.lut`) chosen
    /// in a dialog, instead of the file's own.
    ScienceImportLut,
    /// Science mode: the centred 2-D power spectrum (log magnitude) of the plane shown, in
    /// spatial frequency, or the plane again.
    ScienceFft,
    /// Science mode: overlay a source catalogue (VOTable, FITS table, CSV with RA/Dec) chosen
    /// in a dialog on the sky image.
    ScienceCatalogue,
    /// Science mode: remove the overlaid catalogues.
    ScienceClearCatalogues,
    /// Science mode: overlay DS9 regions (`.reg`) chosen in a dialog: sky regions through the
    /// image's WCS, image regions as they are.
    ScienceLoadRegions,
    /// Science mode: save the image's measurements and the overlaid regions as a DS9 region
    /// file.
    ScienceExportRegions,
    /// Science mode: turn the overlaid regions into measurements of the image, to edit them.
    ScienceEditRegions,
    /// Science mode: remove the overlaid regions.
    ScienceClearRegions,
    /// Science mode: find the image's sky coordinates with a plate solver the user installed
    /// (ASTAP or astrometry.net); again while it runs, stop it.
    PlateSolve,
    /// Science mode: save the plate solution beside the image (`IMAGE.wcs`).
    PlateSolveSave,
    /// Science mode, sky images: show the next sky survey (HiPS) around the image, or none.
    SkySurvey,
    /// Science mode: send the image to the applications on the SAMP hub.
    SampSendImage,
    /// Science mode, sky images: send the position under the pointer (or the view's centre)
    /// to the applications on the SAMP hub.
    SampSendPosition,
    /// Science mode, mass spectrometry images: type an m/z (and tolerance) to show its ion
    /// image; opens on the base peak of the pixel under the pointer.
    ScienceIonImage,
    /// Science mode, thermal images: the next temperature unit (°C, °F, K).
    ScienceTemperatureUnit,
    /// Science mode, thermal images: an isotherm at the temperature under the pointer,
    /// marking what is hotter, then what is colder, then none.
    ScienceIsotherm,
    /// Science mode, radiometric thermal images: type the emissivity and the reflected
    /// temperature, and the temperatures are worked out again.
    ScienceThermalParameters,
    /// Science mode, thermal images: save the temperatures as a 32-bit float TIFF or CSV.
    ScienceExportTemperatures,
    /// Science mode, thermal sequences (FLIR SEQ/CSQ): play or pause the frames.
    ScienceThermalPlay,
    /// Science mode, thermal sequences: save each spot's, line's and area's temperatures over
    /// time as CSV.
    ScienceThermalSeriesCsv,
    /// Science mode, thermal images: a PDF report of the image (and its folder's other
    /// thermal images) with the measurements, facts and notes.
    ThermalReport,
    /// A thermal image with a visible-light photo: show the photo or the thermal image.
    ToggleThermalVisible,
    /// A one-shot colour camera's raw frames (FITS `BAYERPAT`, SER video): show them
    /// debayered or raw.
    ToggleDebayer,
    /// Show or hide the annotations saved beside a whole-slide image (NDPA, Aperio, ASAP).
    ToggleAnnotations,
    /// The next Bayer pattern (RGGB, GRBG, GBRG, BGGR) for this file, when its header's is
    /// wrong or missing.
    NextBayerPattern,
    /// Brightfield stain separation (colour deconvolution) on a colour image: off, H&E,
    /// H-DAB, H&E-DAB, in turn.
    StainSeparation,
    /// Show or hide the first, second or third stain.
    StainShow1,
    StainShow2,
    StainShow3,
    /// Stains as transmitted light or as optical density.
    StainMode,
    /// Estimate the two stains' vectors from the last drawn region (Macenko).
    StainEstimate,
    /// Normalise the stains to the reference (Macenko), for display.
    StainNormalise,
    /// Use the last drawn region as the normalisation reference.
    StainReference,
    /// Back to the published stain vectors.
    StainReset,
    /// Export a slide's annotations as COCO, GeoJSON or label masks.
    ExportAnnotations,
    /// Import annotations from QuPath GeoJSON or COCO.
    ImportAnnotations,
    /// Lay a label volume (NIfTI, MGH…) over the volume or image, placed by its world
    /// coordinates.
    LabelVolume,
    /// Medical mode: show or hide the segmentations (SEG, RTSTRUCT) found for the volume or image.
    Segments,
    /// Medical mode: the next fill opacity of the segmentations.
    SegmentOpacity,
    /// Medical mode, in MPR: fuse a second series of the same frame of reference (PET or SPECT over CT), or stop.
    Fusion,
    /// Medical mode, in MPR: the fused series' next colour map (hot iron, rainbow, viridis).
    FusionColormap,
    /// Medical mode, in MPR: the fused series' next opacity.
    FusionBlend,
    /// Medical mode, in MPR: the arrow keys move (and Shift+arrows turn) the fused series, or stop.
    FusionNudge,
    /// Medical mode, in MPR: the TMTV tool (drag a sphere, then a box, then off) on a PET volume.
    Tmtv,
    /// Medical mode, in MPR: TMTV's threshold, SUV 2.5 or 41 % of SUVmax.
    TmtvThreshold,
    /// Medical mode: the time-intensity curve of the last ellipse or rectangle over the frames (or time points), or hide it.
    TimeCurve,
    /// Medical mode: save the time-intensity curve as CSV.
    ExportCurve,
    /// Drawing mode (DXF): show or hide the layers panel.
    DrawingLayers,
    /// Drawing mode: show every layer again (the file's own on/off and frozen states are
    /// ignored).
    DrawingAllLayers,
    /// Drawing mode: swap the paper between dark and light (colour 7 swaps with it).
    DrawingPaper,
    /// Drawing mode: draw lines at their lineweights, or all thin.
    DrawingLineweights,
    /// Mesh mode (STL, OBJ, PLY, glTF, 3MF): turn the model left / right / up / down about
    /// its centre by 15°.
    MeshOrbitLeft,
    MeshOrbitRight,
    MeshOrbitUp,
    MeshOrbitDown,
    /// Mesh mode: look at the model from the front, the top, the right side, or the
    /// isometric view, fitted.
    MeshViewFront,
    MeshViewTop,
    MeshViewSide,
    MeshViewIso,
    /// Mesh mode: the next shading (shaded, shaded with edges, wireframe, x-ray).
    MeshShading,
    /// Mesh mode: perspective or orthographic projection.
    MeshProjection,
    /// Mesh mode: the ground grid with its scale.
    MeshGrid,
    /// Mesh mode, point clouds (LAS, LAZ, PCD, XYZ, PTS, PLY, E57): larger or smaller
    /// points.
    MeshPointsBigger,
    MeshPointsSmaller,
    /// Mesh mode, point clouds: the next colouring (RGB, intensity, elevation, class) the
    /// cloud has.
    MeshPointColour,
    /// Mesh mode, point clouds: eye-dome lighting (shading by depth) on or off.
    MeshEdl,
    /// Mesh mode, molecular structures (PDB, mmCIF, CIF, SDF, XYZ): the next style
    /// (cartoon, ball-and-stick, space-filling).
    MeshMolStyle,
    /// Mesh mode, molecular structures: the next colouring (element, chain, secondary
    /// structure, B-factor, rainbow).
    MeshMolColour,
    /// Mesh mode, tractograms: the next streamline opacity (opaque down to faint).
    MeshTractOpacity,
    /// Mesh mode: walk through the model (W A S D, the floor under the feet, walls stop
    /// you), or back to orbit.
    MeshWalk,
    /// Mesh mode: fly through the model in every direction, or back to orbit.
    MeshFly,
    /// Mesh mode: the saved views of the model (go to, rename, remove, save as PNG).
    MeshViewpoints,
    /// Mesh mode: save the current view under a name.
    MeshSaveViewpoint,
    /// Mesh mode: add a section plane facing the view (up to six).
    MeshSectionPlane,
    /// Mesh mode: the section box on or off.
    MeshSectionBox,
    /// Mesh mode: flip the chosen section plane.
    MeshSectionFlip,
    /// Mesh mode: remove every section plane and the box.
    MeshSectionClear,
    /// Mesh mode: measure on the cut (two clicks).
    MeshCutMeasure,
    /// Mesh mode: the BCF topics panel (open, review, add topics and comments, export).
    MeshBcf,
    /// Mesh mode: the model checks panel (IDS, clashes, changes, quantities).
    MeshChecks,
    /// Mesh mode: the models in the scene (several IFC files: show, hide, colour, remove).
    MeshModels,
    /// Mesh mode: add another IFC file to the scene.
    MeshAddModel,
    /// Mesh mode: add every IFC file of a folder to the scene.
    MeshAddModelFolder,
    /// Mesh mode: a CAD model's PMI (dimensions, tolerances, datums) and saved views.
    MeshPmi,
    /// Relight mode (PTM, RTI, Relight): move the light left / right / up / down on its
    /// hemisphere.
    RelightLeft,
    RelightRight,
    RelightUp,
    RelightDown,
    /// Relight mode: the next shading (as lit, specular enhancement, normals, diffuse gain).
    RelightMode,
    /// Relight mode: the light straight on again, and the plain shading.
    RelightReset,
    /// Relight mode: show or hide the light widget.
    RelightWidget,
    /// The DICOM tag browser: the file's data set as a tree (DESIGN.md §6.2).
    DicomTags,
    /// The studies browser of the current DICOM file's medium (a patient's CD, DVD or USB
    /// stick): its patients, studies and series.
    Studies,
    /// Medical mode: export the image, its series or its study as de-identified DICOM files
    /// (PS3.15 Basic Profile and options) to a chosen folder.
    ExportAnonymised,
    /// Medical mode: export the study as an IHE PDI folder (DICOMDIR, DICOM/, README.TXT,
    /// INDEX.HTM), optionally anonymised.
    ExportPatientDisc,
    /// Medical mode: mark rectangles of burned-in text that exports black out.
    MaskBurnedInText,
    /// Medical mode: the key images as an anonymised teaching file (PPTX or a folder) with
    /// burned-in text masked and case notes.
    ExportTeachingFile,
    /// Teaching quiz: the key images (or a teaching case folder) one case at a time, the
    /// findings hidden until revealed.
    TeachingQuiz,
    /// Medical mode: send the series or study (or the Studies browser's selection) to a
    /// server of the DICOM servers list, with C-STORE or STOW-RS.
    SendToPacs,
    /// DICOMweb: the servers the user added, study search and series streaming (DESIGN.md
    /// §12.8).
    DicomServers,
    /// OMERO: browse the servers the user added (and the IDR) and open images from them.
    OmeroServers,
    /// Tethered capture: show or hide the panel (camera, exposure settings, folder).
    Tether,
    /// Tethered capture: take a picture with the connected camera.
    TetherCapture,
    /// Series mode: show the next / previous series of the folder.
    NextSeries,
    PrevSeries,
    /// Multi-image files (ICNS sizes, texture layers/faces/slices, TIFF pages): show the
    /// next / previous / first / last page of the current file.
    NextPage,
    PrevPage,
    FirstPage,
    LastPage,
    /// The measurement tool over the current image: lengths, angles and ROI statistics,
    /// calibrated when the file says how big a pixel is.
    Measure,
    /// Copy the current image's measurements as CSV.
    MeasureCopyCsv,
    /// Save the current image's measurements as CSV, GeoJSON or ImageJ ROI.
    MeasureExport,
    /// The measurement tool with its vet, orthopaedic and dental geometry templates.
    MeasureTemplates,
    /// Markup on the current sheet or image: clouds, text, arrows, stamps, measurements and
    /// photo pins, kept in a sidecar next to the file.
    Markup,
    /// Compare the current sheet with another revision: aligned, overlaid in two colours or
    /// as a difference, with the changed regions.
    CompareSheets,
    /// Compare the current image with another, chosen in a file dialog (the gallery's
    /// compare, with these two).
    CompareWith,
    /// Compare (the gallery's, DESIGN §6.3): side by side.
    CompareSide,
    /// Compare: the second image over the first, cut at a divider that can be dragged.
    CompareSplit,
    /// Compare: the split's divider across instead of down, or back.
    CompareSplitDirection,
    /// Compare: the second image over the first, blended.
    CompareOverlay,
    /// Compare: the difference view, then its next kind.
    CompareDifference,
    /// Compare: show the other image of the overlay.
    CompareFlicker,
    /// Compare: switch between the two images by itself, or stop.
    CompareAutoFlicker,
    /// Compare: the next way of aligning images of different sizes (top-left, centred, the
    /// second scaled to the first, found automatically).
    CompareAlign,
    /// Compare: measure in the other metric space (linear light or encoded sRGB).
    CompareMetricSpace,
    /// Compare: measure the whole image instead of what is in view, or back.
    CompareWholeImage,
    /// Compare: the pixel probe of both images under the pointer, on or off.
    CompareProbe,
    /// Compare side by side: zoom and pan each image on its own, or together again.
    CompareLink,
    /// Close compare.
    CompareClose,
    /// Copy a shell command that opens the current file with this view (zoom, position,
    /// window/level, colour map, channel) through the remote-control socket.
    CopyViewCommand,
    /// The same as a Python snippet for the `specular` client package.
    CopyViewPython,
    /// Map mode: show or hide the layer panel (order, opacity, visibility, style).
    MapLayers,
    /// Map mode: add a layer from another file (a GPX track over a shapefile).
    MapAddLayer,
    /// Map mode: zoom to the extent of the layer chosen in the layer panel.
    MapZoomToLayer,
    /// Map mode: the attribute table of the chosen vector layer.
    MapAttributeTable,
    /// Map mode: the next coordinate format (decimal degrees, DMS, UTM, MGRS).
    MapCoordinateFormat,
    /// Map mode: metric or imperial lengths and areas.
    MapUnits,
    /// Map mode: copy the coordinate under the pointer (or of the map's centre).
    MapCopyCoordinate,
    /// Map mode: type a coordinate in any format and centre the map there.
    MapGoToCoordinate,
    /// Map mode: measure a distance or a path on the ellipsoid, with its bearing.
    MapMeasureDistance,
    /// Map mode: measure an area and its perimeter on the ellipsoid.
    MapMeasureArea,
    /// Map mode: draw a line over an elevation model and chart its heights.
    MapElevationProfile,
    /// Map mode: show or hide the overview map.
    MapOverview,
    /// Map mode: colour an elevation model by its slope.
    MapSlope,
    /// Map mode: colour an elevation model by the direction its slopes face.
    MapAspect,
    /// Map mode: contour lines over an elevation model, with labels.
    MapContours,
    /// Map mode: the elevation model in 3-D, with its colours draped over it.
    MapTerrain3d,
    /// Map mode: the time slider (a grid's time steps, a track's times).
    MapTime,
    /// Map mode: play or pause the time slider.
    MapPlay,
    /// Map mode: wind arrows from a grid's u and v components.
    MapWind,
    /// Map mode: the next colour map suited to the weather grid shown.
    MapWeatherColours,
    /// Map mode: online map services (WMS, WMTS, WFS, OGC API, STAC).
    MapServices,
    /// Map mode: the folder's photos placed along a GPS track by their times.
    MapPhotosOnTrack,
    /// Map mode: write positions from a GPS track into the photos' XMP sidecars.
    MapGeotag,
    /// Map mode: save the map as PNG or PDF at a scale, with its furniture.
    MapExport,
    /// Map mode: print the map to scale.
    MapPrint,
    /// Map mode: copy the area on screen as a bounding box or WKT.
    MapCopyExtent,
    /// Open Develop on the image shown, or leave it (DESIGN §6.10).
    ToggleDevelop,
    /// Develop: show the original, or the edit again.
    DevelopBeforeAfter,
    /// Develop: the original and the edit side by side, split down the middle.
    DevelopSplit,
    /// Develop: the keyboard to the panel.
    DevelopFocusPanel,
    /// Develop: the keyboard to the next region (picture, panel, filmstrip), and back.
    DevelopNextRegion,
    DevelopPrevRegion,
    /// Develop: write the edited photo (or several) as new files.
    DevelopExport,
    /// Develop: export the edited photo (or several) with saved export recipes.
    DevelopExportRecipes,
    /// Develop: the crop and straighten tool on the image (Enter keeps, Esc cancels).
    DevelopCrop,
    /// Develop: show or hide the retouch spots' outlines.
    DevelopSpotOutlines,
    /// Develop: a crosshair the arrow keys move over the picture; Enter places a retouch spot
    /// there, Esc puts it away.
    DevelopSpotCrosshair,
    /// Develop: the next scope (histogram, waveform, parade, vectorscope), or none.
    DevelopScopes,
    /// Develop: show where the picture clips on the display (top and bottom codes).
    DevelopClipDisplay,
    /// Develop: show where the scene clips before the look (over its white, under its black).
    DevelopClipScene,
    /// Develop: show colours outside the output gamut (or the proof profile's).
    DevelopClipGamut,
    /// Develop: soft-proof the picture with the chosen printer profile.
    DevelopSoftProof,
    /// Develop's speed edit: hold and scroll (or press arrows) to change a control without
    /// the panel.
    DevelopSpeedExposure,
    DevelopSpeedContrast,
    DevelopSpeedHighlights,
    DevelopSpeedShadows,
    DevelopSpeedWhites,
    DevelopSpeedBlacks,
    DevelopSpeedTemperature,
    DevelopSpeedVibrance,
    /// Develop: copy the photo's edit (all but crop and lens), to paste on others.
    DevelopCopyEdit,
    /// Develop: copy chosen modules of the photo's edit.
    DevelopCopySelective,
    /// Develop: paste the copied edit on the photo, or on the gallery's selection.
    DevelopPasteEdit,
    /// Develop: paste chosen modules of the copied edit, appending or replacing.
    DevelopPasteSelective,
    /// Develop: paste the copied edit on every picked photo of the folder.
    DevelopPastePicks,
    /// Develop: the photo becomes the reference for match to reference.
    DevelopSetReference,
    /// Develop: match the exposure and white balance of the photo (or the selection, or the
    /// picks in cull mode) to the reference.
    DevelopMatchReference,
    /// Cull mode: apply the current photo's edit to the picked photos.
    DevelopSync,
    /// Cull mode: apply chosen modules of the current photo's edit to the picked photos.
    DevelopSyncSelective,
    /// Cull mode with new files arriving: the style each new photo gets.
    DevelopNextCapture,
    /// Develop: the rules that apply a style when a photo is first opened.
    DevelopAutoRules,
    /// Develop: import a style file.
    DevelopImportStyle,
    /// Merge to HDR on the gallery's selection or the photo shown (or the bracket around it).
    DevelopMergeHdr,
    /// Merge to panorama on the gallery's selection or the photo shown (or the panorama
    /// around it).
    DevelopMergePano,
    /// Focus stack on the gallery's selection or the photo shown (or the stack around it).
    DevelopFocusStack,
    /// Develop: name the edit as it is now, to compare against or go back to.
    DevelopSnapshot,
    /// Develop: drop superseded history steps.
    DevelopCompressHistory,
    /// Develop: a new version (virtual copy) of the photo.
    DevelopNewVersion,
    /// Develop: a style from the edit's chosen modules.
    DevelopNewStyle,
    /// Develop: the brush smaller, larger, softer, harder.
    DevelopBrushSmaller,
    DevelopBrushLarger,
    DevelopBrushSofter,
    DevelopBrushHarder,
    /// Develop: the brush paints or erases.
    DevelopBrushErase,
    /// Develop: show or hide the focused module's mask.
    DevelopMaskOverlay,
}

/// Develop's speed-edit actions, in registry order.
pub const DEVELOP_SPEED: [Action; 8] = [
    Action::DevelopSpeedExposure,
    Action::DevelopSpeedContrast,
    Action::DevelopSpeedHighlights,
    Action::DevelopSpeedShadows,
    Action::DevelopSpeedWhites,
    Action::DevelopSpeedBlacks,
    Action::DevelopSpeedTemperature,
    Action::DevelopSpeedVibrance,
];

/// The Maps actions, in registry order.
pub const MAP_ACTIONS: [Action; 26] = [
    Action::MapLayers,
    Action::MapAddLayer,
    Action::MapZoomToLayer,
    Action::MapAttributeTable,
    Action::MapCoordinateFormat,
    Action::MapUnits,
    Action::MapCopyCoordinate,
    Action::MapGoToCoordinate,
    Action::MapMeasureDistance,
    Action::MapMeasureArea,
    Action::MapElevationProfile,
    Action::MapOverview,
    Action::MapSlope,
    Action::MapAspect,
    Action::MapContours,
    Action::MapTerrain3d,
    Action::MapTime,
    Action::MapPlay,
    Action::MapWind,
    Action::MapWeatherColours,
    Action::MapServices,
    Action::MapPhotosOnTrack,
    Action::MapGeotag,
    Action::MapExport,
    Action::MapPrint,
    Action::MapCopyExtent,
];

/// Where an action is listed in the cheat sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Group {
    Navigation,
    ZoomPan,
    View,
    File,
    Window,
    /// Cull mode (the cheat sheet adds its mark keys under this heading).
    Cull,
    /// Medical images (window/level, presets, slices); listed while a medical image is shown.
    Medical,
    /// Scientific images (stretch, limits, planes, channels); listed while one is shown.
    Science,
    /// Drawings (layers, paper, lineweights); listed while one is shown.
    Drawing,
    /// 3D meshes (orbit, views, shading); listed while one is shown.
    Mesh,
    /// Reflectance transformation images (the light, shading modes); listed while one is
    /// shown.
    Relight,
    /// Maps (layers, coordinates, measuring); listed while one is shown.
    Map,
    /// Develop (before/after, the panel, speed edit); listed while Develop is open.
    Develop,
    Help,
}

impl Group {
    /// Fluent message ID of the group heading.
    pub fn message_id(self) -> &'static str {
        match self {
            Group::Navigation => "cheatsheet-navigation",
            Group::ZoomPan => "cheatsheet-zoom-pan",
            Group::View => "cheatsheet-view",
            Group::File => "cheatsheet-file",
            Group::Window => "cheatsheet-window",
            Group::Cull => "cheatsheet-cull",
            Group::Medical => "cheatsheet-medical",
            Group::Science => "cheatsheet-science",
            Group::Drawing => "cheatsheet-drawing",
            Group::Mesh => "keys-mode-mesh",
            Group::Relight => "keys-mode-relight",
            Group::Map => "keys-mode-map",
            Group::Develop => "keys-mode-develop",
            Group::Help => "cheatsheet-help",
        }
    }
    pub const ALL: [Group; 14] = [
        Group::Navigation,
        Group::ZoomPan,
        Group::View,
        Group::File,
        Group::Window,
        Group::Cull,
        Group::Medical,
        Group::Science,
        Group::Drawing,
        Group::Mesh,
        Group::Relight,
        Group::Map,
        Group::Develop,
        Group::Help,
    ];
}

/// Colour label names in `keymap.toml` (`cull-label:red`), label 1 first.
const LABELS: [&str; 4] = ["red", "yellow", "green", "blue"];

impl Action {
    /// Actions shown in the palette and cheat sheet, in display order.
    pub const LISTED: &'static [Action] = &[
        Action::NextImage,
        Action::PrevImage,
        Action::FirstImage,
        Action::LastImage,
        Action::GoToImage,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomFit,
        Action::ZoomFill,
        Action::ZoomFitWidth,
        Action::ZoomFitHeight,
        Action::ZoomTo(100.0),
        Action::ZoomTo(200.0),
        Action::ZoomTo(300.0),
        Action::ZoomTo(400.0),
        Action::ZoomRectangle,
        Action::RotateLeft,
        Action::RotateRight,
        Action::FlipHorizontal,
        Action::FlipVertical,
        Action::ToggleNavigator,
        Action::Loupe,
        Action::TogglePixelGrid,
        Action::ToggleClipping,
        Action::ToggleFocusPeaking,
        Action::ToggleHistogram,
        Action::TogglePixelInspector,
        Action::ToggleCrosshair,
        Action::AnnounceCursor,
        Action::ChannelRed,
        Action::ChannelGreen,
        Action::ChannelBlue,
        Action::ChannelAlpha,
        Action::ChannelLuma,
        Action::ToggleFalseColour,
        Action::NextColourMap,
        Action::NextChannelColours,
        Action::ToggleHiLo,
        Action::NextColourVision,
        Action::SimulateProtanopia,
        Action::SimulateDeuteranopia,
        Action::SimulateTritanopia,
        Action::SimulateAchromatopsia,
        Action::ToggleInfo,
        Action::ToggleFullscreen,
        Action::Slideshow,
        Action::ViewerOnOtherMonitor,
        Action::ExportSlideshow,
        Action::ToggleCull,
        Action::CopyImage,
        Action::CopyPath,
        Action::ReadCodes,
        Action::FindText,
        Action::FaceBoxes,
        Action::OpenWith,
        Action::OpenAs,
        Action::Print,
        Action::ClientGallery,
        Action::ImportClientPicks,
        Action::ExportFigure,
        Action::ToggleKeyImage,
        Action::CopyKeyImages,
        Action::ExportKeyImages,
        Action::Convert,
        Action::ExportRecipes,
        Action::Import,
        Action::Paste,
        Action::MoveToTrash,
        Action::Undo,
        Action::Redo,
        Action::Rename,
        Action::BatchRename,
        Action::CopyToFolder,
        Action::MoveToFolder,
        Action::RotateFileLeft,
        Action::RotateFileRight,
        Action::SetWallpaper,
        Action::Share,
        Action::SendByEmail,
        Action::ShareWithZynk,
        Action::SendWithZynk,
        Action::Open,
        Action::OpenAddress,
        Action::OpenStream,
        Action::Settings,
        Action::CustomiseToolbar,
        Action::CustomiseKeys,
        Action::ExportSettings,
        Action::ImportSettings,
        Action::Palette,
        Action::CheatSheet,
        Action::About,
        Action::ShowWelcome,
        Action::Help,
        Action::JoinDiscord,
        Action::SaveDiagnostics,
        Action::Hud,
        Action::Language,
        Action::ToggleTheme,
        Action::InterfaceLarger,
        Action::InterfaceSmaller,
        Action::InterfaceReset,
        Action::Close,
        Action::Quit,
        Action::PlayPause,
        Action::NextFrame,
        Action::PrevFrame,
        Action::FasterPlayback,
        Action::SlowerPlayback,
        Action::NormalPlayback,
        Action::ToggleLoopForever,
        Action::PlayPauseVideo,
        Action::SeekForward,
        Action::SeekBack,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::ToggleMute,
        Action::VideoFullscreen,
        Action::SeekForwardLong,
        Action::SeekBackLong,
        Action::CycleSubtitles,
        Action::OpenSubtitle,
        Action::GenerateSubtitles,
        Action::SaveSubtitles,
        Action::SubtitleEarlier,
        Action::SubtitleLater,
        Action::SubtitleEarlierFine,
        Action::SubtitleLaterFine,
        Action::SubtitleStepPrev,
        Action::SubtitleStepNext,
        Action::SubtitleUp,
        Action::SubtitleDown,
        Action::SubtitleBigger,
        Action::SubtitleSmaller,
        Action::SubtitleResetPlace,
        Action::SubtitleStyle,
        Action::CycleSound,
        Action::StreamQuality,
        Action::DialogueBoost,
        Action::Equaliser,
        Action::SoundEarlier,
        Action::SoundLater,
        Action::SoundEarlierFine,
        Action::SoundLaterFine,
        Action::ResetDelays,
        Action::PrevChapter,
        Action::NextChapter,
        Action::AbLoop,
        Action::SaveFrame,
        Action::CopyFrame,
        Action::MiniPlayer,
        Action::CycleAspect,
        Action::CycleCrop,
        Action::NextTrack,
        Action::PrevTrack,
        Action::CycleRepeat,
        Action::ToggleShuffle,
        Action::ToggleQueue,
        Action::QueuePlayNext,
        Action::QueueAdd,
        Action::SaveQueue,
        Action::CycleSoundVisuals,
        Action::ToggleLyrics,
        Action::GoToTime,
        Action::VideoPicture,
        Action::CastMenu,
        Action::ToggleRightToLeft,
        Action::ToggleSpreads,
        Action::ToggleContents,
        Action::ToggleGallery,
        Action::ToggleLibrary,
        Action::Keywords,
        Action::EditMetadata,
        Action::WindowLevelTool,
        Action::WindowPreset(0),
        Action::WindowPreset(1),
        Action::WindowPreset(2),
        Action::WindowPreset(3),
        Action::WindowPreset(4),
        Action::WindowPreset(5),
        Action::WindowPreset(6),
        Action::WindowPreset(7),
        Action::WindowPreset(8),
        Action::WindowPreset(9),
        Action::InvertGrey,
        Action::NextSlice,
        Action::PrevSlice,
        Action::Cine,
        Action::MeasureLength,
        Action::MeasureAngle,
        Action::MeasureEllipse,
        Action::MeasureRectangle,
        Action::ClearMeasurements,
        Action::ToggleOverlays,
        Action::ToggleOrientationLabels,
        Action::NextPresentationState,
        Action::ExportAnonymised,
        Action::ExportPatientDisc,
        Action::MaskBurnedInText,
        Action::ExportTeachingFile,
        Action::TeachingQuiz,
        Action::SendToPacs,
        Action::Mpr,
        Action::MprSlab,
        Action::MprTractSlab,
        Action::MprPreset,
        Action::MprRender,
        Action::MprClip,
        Action::MprExportStl,
        Action::MprMaximise,
        Action::MprExportPlanes,
        Action::MprCurve,
        Action::MprCurveLayout,
        Action::MprCurveSave,
        Action::MprCurveLoad,
        Action::MprExportSegments,
        Action::MprDental,
        Action::MprNerve,
        Action::CompareSeries,
        Action::CompareLayout,
        Action::NextViewport,
        Action::PrevViewport,
        Action::ReferenceLines,
        Action::SyncWindow,
        Action::SyncZoom,
        Action::LinkSeries,
        Action::AlignSeries,
        Action::ScienceStretch,
        Action::ScienceLimits,
        Action::ScienceNextPlane,
        Action::SciencePrevPlane,
        Action::ScienceNextAxis,
        Action::ScienceComposite,
        Action::ScienceChannel(1),
        Action::ScienceChannel(2),
        Action::ScienceChannel(3),
        Action::ScienceChannel(4),
        Action::ScienceChannel(5),
        Action::ScienceChannel(6),
        Action::ScienceChannel(7),
        Action::ScienceChannel(8),
        Action::ScienceChannel(9),
        Action::ScienceChannel(10),
        Action::ScienceChannel(11),
        Action::ScienceChannel(12),
        Action::ScienceChannel(13),
        Action::ScienceChannel(14),
        Action::ScienceChannel(15),
        Action::ScienceChannel(16),
        Action::ScienceGrid,
        Action::ScienceLevel,
        Action::ScienceHillshade,
        Action::ScienceWiggle,
        Action::ScienceWireframe,
        Action::ScienceReset,
        Action::ScienceImportLut,
        Action::ScienceFft,
        Action::ScienceCatalogue,
        Action::ScienceClearCatalogues,
        Action::ScienceLoadRegions,
        Action::ScienceExportRegions,
        Action::ScienceEditRegions,
        Action::ScienceClearRegions,
        Action::PlateSolve,
        Action::PlateSolveSave,
        Action::SkySurvey,
        Action::SampSendImage,
        Action::SampSendPosition,
        Action::ScienceIonImage,
        Action::ScienceTemperatureUnit,
        Action::ScienceIsotherm,
        Action::ScienceThermalParameters,
        Action::ScienceExportTemperatures,
        Action::ScienceThermalPlay,
        Action::ScienceThermalSeriesCsv,
        Action::ThermalReport,
        Action::ToggleThermalVisible,
        Action::ToggleDebayer,
        Action::NextBayerPattern,
        Action::ToggleAnnotations,
        Action::StainSeparation,
        Action::StainShow1,
        Action::StainShow2,
        Action::StainShow3,
        Action::StainMode,
        Action::StainEstimate,
        Action::StainNormalise,
        Action::StainReference,
        Action::StainReset,
        Action::ExportAnnotations,
        Action::ImportAnnotations,
        Action::Segments,
        Action::SegmentOpacity,
        Action::LabelVolume,
        Action::Fusion,
        Action::FusionColormap,
        Action::FusionBlend,
        Action::FusionNudge,
        Action::Tmtv,
        Action::TmtvThreshold,
        Action::TimeCurve,
        Action::ExportCurve,
        Action::DrawingLayers,
        Action::DrawingAllLayers,
        Action::DrawingPaper,
        Action::DrawingLineweights,
        Action::MeshOrbitLeft,
        Action::MeshOrbitRight,
        Action::MeshOrbitUp,
        Action::MeshOrbitDown,
        Action::MeshViewFront,
        Action::MeshViewTop,
        Action::MeshViewSide,
        Action::MeshViewIso,
        Action::MeshShading,
        Action::MeshProjection,
        Action::MeshGrid,
        Action::MeshPointsBigger,
        Action::MeshPointsSmaller,
        Action::MeshPointColour,
        Action::MeshEdl,
        Action::MeshMolStyle,
        Action::MeshMolColour,
        Action::MeshTractOpacity,
        Action::MeshWalk,
        Action::MeshFly,
        Action::MeshViewpoints,
        Action::MeshSaveViewpoint,
        Action::MeshSectionPlane,
        Action::MeshSectionBox,
        Action::MeshSectionFlip,
        Action::MeshSectionClear,
        Action::MeshCutMeasure,
        Action::MeshBcf,
        Action::MeshChecks,
        Action::MeshModels,
        Action::MeshAddModel,
        Action::MeshAddModelFolder,
        Action::MeshPmi,
        Action::RelightLeft,
        Action::RelightRight,
        Action::RelightUp,
        Action::RelightDown,
        Action::RelightMode,
        Action::RelightReset,
        Action::RelightWidget,
        Action::NextSeries,
        Action::PrevSeries,
        Action::DicomTags,
        Action::Studies,
        Action::DicomServers,
        Action::OmeroServers,
        Action::Tether,
        Action::TetherCapture,
        Action::NextPage,
        Action::PrevPage,
        Action::FirstPage,
        Action::LastPage,
        Action::Measure,
        Action::MeasureCopyCsv,
        Action::MeasureExport,
        Action::MeasureTemplates,
        Action::Markup,
        Action::CompareSheets,
        Action::CompareWith,
        Action::CompareSide,
        Action::CompareSplit,
        Action::CompareSplitDirection,
        Action::CompareOverlay,
        Action::CompareDifference,
        Action::CompareFlicker,
        Action::CompareAutoFlicker,
        Action::CompareAlign,
        Action::CompareMetricSpace,
        Action::CompareWholeImage,
        Action::CompareProbe,
        Action::CompareLink,
        Action::CompareClose,
        Action::CopyViewCommand,
        Action::CopyViewPython,
        Action::MapLayers,
        Action::MapAddLayer,
        Action::MapZoomToLayer,
        Action::MapAttributeTable,
        Action::MapCoordinateFormat,
        Action::MapUnits,
        Action::MapCopyCoordinate,
        Action::MapGoToCoordinate,
        Action::MapMeasureDistance,
        Action::MapMeasureArea,
        Action::MapElevationProfile,
        Action::MapOverview,
        Action::MapSlope,
        Action::MapAspect,
        Action::MapContours,
        Action::MapTerrain3d,
        Action::MapTime,
        Action::MapPlay,
        Action::MapWind,
        Action::MapWeatherColours,
        Action::MapServices,
        Action::MapPhotosOnTrack,
        Action::MapGeotag,
        Action::MapExport,
        Action::MapPrint,
        Action::MapCopyExtent,
        Action::ToggleDevelop,
        Action::DevelopBeforeAfter,
        Action::DevelopSplit,
        Action::DevelopFocusPanel,
        Action::DevelopNextRegion,
        Action::DevelopPrevRegion,
        Action::DevelopExport,
        Action::DevelopExportRecipes,
        Action::DevelopCrop,
        Action::DevelopSpotOutlines,
        Action::DevelopSpotCrosshair,
        Action::DevelopScopes,
        Action::DevelopClipDisplay,
        Action::DevelopClipScene,
        Action::DevelopClipGamut,
        Action::DevelopSoftProof,
        Action::DevelopSpeedExposure,
        Action::DevelopSpeedContrast,
        Action::DevelopSpeedHighlights,
        Action::DevelopSpeedShadows,
        Action::DevelopSpeedWhites,
        Action::DevelopSpeedBlacks,
        Action::DevelopSpeedTemperature,
        Action::DevelopSpeedVibrance,
        Action::DevelopCopyEdit,
        Action::DevelopCopySelective,
        Action::DevelopPasteEdit,
        Action::DevelopPasteSelective,
        Action::DevelopPastePicks,
        Action::DevelopSetReference,
        Action::DevelopMatchReference,
        Action::DevelopSync,
        Action::DevelopSyncSelective,
        Action::DevelopNextCapture,
        Action::DevelopAutoRules,
        Action::DevelopImportStyle,
        Action::DevelopMergeHdr,
        Action::DevelopMergePano,
        Action::DevelopFocusStack,
        Action::DevelopSnapshot,
        Action::DevelopCompressHistory,
        Action::DevelopNewVersion,
        Action::DevelopNewStyle,
        Action::DevelopBrushSmaller,
        Action::DevelopBrushLarger,
        Action::DevelopBrushSofter,
        Action::DevelopBrushHarder,
        Action::DevelopBrushErase,
        Action::DevelopMaskOverlay,
    ];

    /// Every action a key can be bound to, in the order Settings → Keys lists them: the
    /// action registry. An action added here appears in the configurator, `keymap.toml` and
    /// the presets' checks without further changes.
    pub const BINDABLE: &'static [Action] = &[
        Action::NextImage,
        Action::PrevImage,
        Action::FirstImage,
        Action::LastImage,
        Action::GoToImage,
        Action::ToggleGallery,
        Action::ToggleSpreads,
        Action::ToggleRightToLeft,
        Action::ToggleContents,
        Action::ToggleLibrary,
        Action::Keywords,
        Action::EditMetadata,
        Action::NextSeries,
        Action::PrevSeries,
        Action::NextPage,
        Action::PrevPage,
        Action::FirstPage,
        Action::LastPage,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomFit,
        Action::ZoomFill,
        Action::ZoomFitWidth,
        Action::ZoomFitHeight,
        Action::ZoomTo(100.0),
        Action::ZoomTo(200.0),
        Action::ZoomTo(300.0),
        Action::ZoomTo(400.0),
        Action::ZoomRectangle,
        Action::PanLeft,
        Action::PanRight,
        Action::PanUp,
        Action::PanDown,
        Action::PanPageLeft,
        Action::PanPageRight,
        Action::PanPageUp,
        Action::PanPageDown,
        Action::Loupe,
        Action::ToggleNavigator,
        Action::RotateLeft,
        Action::RotateRight,
        Action::FlipHorizontal,
        Action::FlipVertical,
        Action::TogglePixelGrid,
        Action::ToggleClipping,
        Action::ToggleFocusPeaking,
        Action::ToggleHistogram,
        Action::TogglePixelInspector,
        Action::ToggleCrosshair,
        Action::AnnounceCursor,
        Action::ChannelRed,
        Action::ChannelGreen,
        Action::ChannelBlue,
        Action::ChannelAlpha,
        Action::ChannelLuma,
        Action::ToggleFalseColour,
        Action::NextColourMap,
        Action::NextChannelColours,
        Action::ToggleHiLo,
        Action::NextColourVision,
        Action::SimulateProtanopia,
        Action::SimulateDeuteranopia,
        Action::SimulateTritanopia,
        Action::SimulateAchromatopsia,
        Action::ToggleInfo,
        Action::Measure,
        Action::MeasureCopyCsv,
        Action::MeasureExport,
        Action::MeasureTemplates,
        Action::Markup,
        Action::CompareSheets,
        Action::CompareWith,
        Action::CompareSide,
        Action::CompareSplit,
        Action::CompareSplitDirection,
        Action::CompareOverlay,
        Action::CompareDifference,
        Action::CompareFlicker,
        Action::CompareAutoFlicker,
        Action::CompareAlign,
        Action::CompareMetricSpace,
        Action::CompareWholeImage,
        Action::CompareProbe,
        Action::CompareLink,
        Action::CompareClose,
        Action::CopyViewCommand,
        Action::CopyViewPython,
        Action::DicomTags,
        Action::Studies,
        Action::DicomServers,
        Action::OmeroServers,
        Action::Tether,
        Action::TetherCapture,
        Action::PlayPause,
        Action::NextFrame,
        Action::PrevFrame,
        Action::FasterPlayback,
        Action::SlowerPlayback,
        Action::NormalPlayback,
        Action::ToggleLoopForever,
        Action::PlayPauseVideo,
        Action::SeekForward,
        Action::SeekBack,
        Action::VolumeUp,
        Action::VolumeDown,
        Action::ToggleMute,
        Action::VideoFullscreen,
        Action::SeekForwardLong,
        Action::SeekBackLong,
        Action::CycleSubtitles,
        Action::OpenSubtitle,
        Action::GenerateSubtitles,
        Action::SaveSubtitles,
        Action::SubtitleEarlier,
        Action::SubtitleLater,
        Action::SubtitleEarlierFine,
        Action::SubtitleLaterFine,
        Action::SubtitleStepPrev,
        Action::SubtitleStepNext,
        Action::SubtitleUp,
        Action::SubtitleDown,
        Action::SubtitleBigger,
        Action::SubtitleSmaller,
        Action::SubtitleResetPlace,
        Action::SubtitleStyle,
        Action::CycleSound,
        Action::StreamQuality,
        Action::DialogueBoost,
        Action::Equaliser,
        Action::SoundEarlier,
        Action::SoundLater,
        Action::SoundEarlierFine,
        Action::SoundLaterFine,
        Action::ResetDelays,
        Action::PrevChapter,
        Action::NextChapter,
        Action::AbLoop,
        Action::SaveFrame,
        Action::CopyFrame,
        Action::MiniPlayer,
        Action::CycleAspect,
        Action::CycleCrop,
        Action::NextTrack,
        Action::PrevTrack,
        Action::CycleRepeat,
        Action::ToggleShuffle,
        Action::ToggleQueue,
        Action::QueuePlayNext,
        Action::QueueAdd,
        Action::SaveQueue,
        Action::CycleSoundVisuals,
        Action::ToggleLyrics,
        Action::GoToTime,
        Action::VideoPicture,
        Action::CastMenu,
        Action::SeekPercent(0),
        Action::SeekPercent(10),
        Action::SeekPercent(20),
        Action::SeekPercent(30),
        Action::SeekPercent(40),
        Action::SeekPercent(50),
        Action::SeekPercent(60),
        Action::SeekPercent(70),
        Action::SeekPercent(80),
        Action::SeekPercent(90),
        Action::CopyImage,
        Action::CopyPath,
        Action::ReadCodes,
        Action::FindText,
        Action::FaceBoxes,
        Action::OpenWith,
        Action::OpenAs,
        Action::Print,
        Action::ClientGallery,
        Action::ImportClientPicks,
        Action::ExportFigure,
        Action::ToggleKeyImage,
        Action::CopyKeyImages,
        Action::ExportKeyImages,
        Action::Convert,
        Action::ExportRecipes,
        Action::Import,
        Action::Paste,
        Action::MoveToTrash,
        Action::Undo,
        Action::Redo,
        Action::Rename,
        Action::BatchRename,
        Action::CopyToFolder,
        Action::MoveToFolder,
        Action::RotateFileLeft,
        Action::RotateFileRight,
        Action::SetWallpaper,
        Action::Share,
        Action::SendByEmail,
        Action::ShareWithZynk,
        Action::SendWithZynk,
        Action::Open,
        Action::OpenAddress,
        Action::OpenStream,
        Action::Reload,
        Action::ToggleFullscreen,
        Action::Slideshow,
        Action::ViewerOnOtherMonitor,
        Action::ExportSlideshow,
        Action::Back,
        Action::Settings,
        Action::CustomiseToolbar,
        Action::CustomiseKeys,
        Action::ExportSettings,
        Action::ImportSettings,
        Action::ToggleTheme,
        Action::InterfaceLarger,
        Action::InterfaceSmaller,
        Action::InterfaceReset,
        Action::Language,
        Action::Close,
        Action::Quit,
        Action::ToggleCull,
        Action::Palette,
        Action::CheatSheet,
        Action::About,
        Action::ShowWelcome,
        Action::Help,
        Action::JoinDiscord,
        Action::SaveDiagnostics,
        Action::Hud,
        Action::CullPick,
        Action::CullReject,
        Action::CullUnflag,
        Action::CullRate(0),
        Action::CullRate(1),
        Action::CullRate(2),
        Action::CullRate(3),
        Action::CullRate(4),
        Action::CullRate(5),
        Action::CullLabel(1),
        Action::CullLabel(2),
        Action::CullLabel(3),
        Action::CullLabel(4),
        Action::CullCopyTo(1),
        Action::CullCopyTo(2),
        Action::CullCopyTo(3),
        Action::CullMoveTo(1),
        Action::CullMoveTo(2),
        Action::CullMoveTo(3),
        Action::CullBurst,
        Action::CullKeepBest,
        Action::CullFollow,
        Action::CullSubframes,
        Action::CullSortNext,
        Action::CullSortReverse,
        Action::CullMetricNext,
        Action::CullSigmaDown,
        Action::CullSigmaUp,
        Action::CullAutoReject,
        Action::CullMoveRejects,
        Action::CullSameStretch,
        Action::WindowLevelTool,
        Action::WindowPreset(0),
        Action::WindowPreset(1),
        Action::WindowPreset(2),
        Action::WindowPreset(3),
        Action::WindowPreset(4),
        Action::WindowPreset(5),
        Action::WindowPreset(6),
        Action::WindowPreset(7),
        Action::WindowPreset(8),
        Action::WindowPreset(9),
        Action::InvertGrey,
        Action::NextSlice,
        Action::PrevSlice,
        Action::Cine,
        Action::MeasureLength,
        Action::MeasureAngle,
        Action::MeasureEllipse,
        Action::MeasureRectangle,
        Action::ClearMeasurements,
        Action::ToggleOverlays,
        Action::ToggleOrientationLabels,
        Action::NextPresentationState,
        Action::ExportAnonymised,
        Action::ExportPatientDisc,
        Action::MaskBurnedInText,
        Action::ExportTeachingFile,
        Action::TeachingQuiz,
        Action::SendToPacs,
        Action::Mpr,
        Action::MprSlab,
        Action::MprTractSlab,
        Action::MprPreset,
        Action::MprRender,
        Action::MprClip,
        Action::MprExportStl,
        Action::MprMaximise,
        Action::MprExportPlanes,
        Action::MprCurve,
        Action::MprCurveLayout,
        Action::MprCurveSave,
        Action::MprCurveLoad,
        Action::MprExportSegments,
        Action::MprDental,
        Action::MprNerve,
        Action::CompareSeries,
        Action::CompareLayout,
        Action::NextViewport,
        Action::PrevViewport,
        Action::ReferenceLines,
        Action::SyncWindow,
        Action::SyncZoom,
        Action::LinkSeries,
        Action::AlignSeries,
        Action::ScienceStretch,
        Action::ScienceLimits,
        Action::ScienceNextPlane,
        Action::SciencePrevPlane,
        Action::ScienceNextAxis,
        Action::ScienceComposite,
        Action::ScienceChannel(1),
        Action::ScienceChannel(2),
        Action::ScienceChannel(3),
        Action::ScienceChannel(4),
        Action::ScienceChannel(5),
        Action::ScienceChannel(6),
        Action::ScienceChannel(7),
        Action::ScienceChannel(8),
        Action::ScienceChannel(9),
        Action::ScienceChannel(10),
        Action::ScienceChannel(11),
        Action::ScienceChannel(12),
        Action::ScienceChannel(13),
        Action::ScienceChannel(14),
        Action::ScienceChannel(15),
        Action::ScienceChannel(16),
        Action::ScienceGrid,
        Action::ScienceLevel,
        Action::ScienceHillshade,
        Action::ScienceWiggle,
        Action::ScienceWireframe,
        Action::ScienceReset,
        Action::ScienceImportLut,
        Action::ScienceFft,
        Action::ScienceCatalogue,
        Action::ScienceClearCatalogues,
        Action::ScienceLoadRegions,
        Action::ScienceExportRegions,
        Action::ScienceEditRegions,
        Action::ScienceClearRegions,
        Action::PlateSolve,
        Action::PlateSolveSave,
        Action::SkySurvey,
        Action::SampSendImage,
        Action::SampSendPosition,
        Action::ScienceIonImage,
        Action::ScienceTemperatureUnit,
        Action::ScienceIsotherm,
        Action::ScienceThermalParameters,
        Action::ScienceExportTemperatures,
        Action::ScienceThermalPlay,
        Action::ScienceThermalSeriesCsv,
        Action::ThermalReport,
        Action::ToggleThermalVisible,
        Action::ToggleDebayer,
        Action::NextBayerPattern,
        Action::ToggleAnnotations,
        Action::StainSeparation,
        Action::StainShow1,
        Action::StainShow2,
        Action::StainShow3,
        Action::StainMode,
        Action::StainEstimate,
        Action::StainNormalise,
        Action::StainReference,
        Action::StainReset,
        Action::ExportAnnotations,
        Action::ImportAnnotations,
        Action::Segments,
        Action::SegmentOpacity,
        Action::LabelVolume,
        Action::Fusion,
        Action::FusionColormap,
        Action::FusionBlend,
        Action::FusionNudge,
        Action::Tmtv,
        Action::TmtvThreshold,
        Action::TimeCurve,
        Action::ExportCurve,
        Action::DrawingLayers,
        Action::DrawingAllLayers,
        Action::DrawingPaper,
        Action::DrawingLineweights,
        Action::MeshOrbitLeft,
        Action::MeshOrbitRight,
        Action::MeshOrbitUp,
        Action::MeshOrbitDown,
        Action::MeshViewFront,
        Action::MeshViewTop,
        Action::MeshViewSide,
        Action::MeshViewIso,
        Action::MeshShading,
        Action::MeshProjection,
        Action::MeshGrid,
        Action::MeshPointsBigger,
        Action::MeshPointsSmaller,
        Action::MeshPointColour,
        Action::MeshEdl,
        Action::MeshMolStyle,
        Action::MeshMolColour,
        Action::MeshTractOpacity,
        Action::MeshWalk,
        Action::MeshFly,
        Action::MeshViewpoints,
        Action::MeshSaveViewpoint,
        Action::MeshSectionPlane,
        Action::MeshSectionBox,
        Action::MeshSectionFlip,
        Action::MeshSectionClear,
        Action::MeshCutMeasure,
        Action::MeshBcf,
        Action::MeshChecks,
        Action::MeshModels,
        Action::MeshAddModel,
        Action::MeshAddModelFolder,
        Action::MeshPmi,
        Action::RelightLeft,
        Action::RelightRight,
        Action::RelightUp,
        Action::RelightDown,
        Action::RelightMode,
        Action::RelightReset,
        Action::RelightWidget,
        Action::MapLayers,
        Action::MapAddLayer,
        Action::MapZoomToLayer,
        Action::MapAttributeTable,
        Action::MapCoordinateFormat,
        Action::MapUnits,
        Action::MapCopyCoordinate,
        Action::MapGoToCoordinate,
        Action::MapMeasureDistance,
        Action::MapMeasureArea,
        Action::MapElevationProfile,
        Action::MapOverview,
        Action::MapSlope,
        Action::MapAspect,
        Action::MapContours,
        Action::MapTerrain3d,
        Action::MapTime,
        Action::MapPlay,
        Action::MapWind,
        Action::MapWeatherColours,
        Action::MapServices,
        Action::MapPhotosOnTrack,
        Action::MapGeotag,
        Action::MapExport,
        Action::MapPrint,
        Action::MapCopyExtent,
        Action::ToggleDevelop,
        Action::DevelopBeforeAfter,
        Action::DevelopSplit,
        Action::DevelopFocusPanel,
        Action::DevelopNextRegion,
        Action::DevelopPrevRegion,
        Action::DevelopExport,
        Action::DevelopExportRecipes,
        Action::DevelopCrop,
        Action::DevelopSpotOutlines,
        Action::DevelopSpotCrosshair,
        Action::DevelopScopes,
        Action::DevelopClipDisplay,
        Action::DevelopClipScene,
        Action::DevelopClipGamut,
        Action::DevelopSoftProof,
        Action::DevelopSpeedExposure,
        Action::DevelopSpeedContrast,
        Action::DevelopSpeedHighlights,
        Action::DevelopSpeedShadows,
        Action::DevelopSpeedWhites,
        Action::DevelopSpeedBlacks,
        Action::DevelopSpeedTemperature,
        Action::DevelopSpeedVibrance,
        Action::DevelopCopyEdit,
        Action::DevelopCopySelective,
        Action::DevelopPasteEdit,
        Action::DevelopPasteSelective,
        Action::DevelopPastePicks,
        Action::DevelopSetReference,
        Action::DevelopMatchReference,
        Action::DevelopSync,
        Action::DevelopSyncSelective,
        Action::DevelopNextCapture,
        Action::DevelopAutoRules,
        Action::DevelopImportStyle,
        Action::DevelopMergeHdr,
        Action::DevelopMergePano,
        Action::DevelopFocusStack,
        Action::DevelopSnapshot,
        Action::DevelopCompressHistory,
        Action::DevelopNewVersion,
        Action::DevelopNewStyle,
        Action::DevelopBrushSmaller,
        Action::DevelopBrushLarger,
        Action::DevelopBrushSofter,
        Action::DevelopBrushHarder,
        Action::DevelopBrushErase,
        Action::DevelopMaskOverlay,
    ];

    /// Stable identifier used in `keymap.toml` and the CLI.
    pub fn id(self) -> String {
        match self {
            Action::ZoomTo(p) => format!("zoom-to:{}", p),
            Action::CullRate(n) => format!("cull-rate:{n}"),
            Action::CullLabel(n) => format!(
                "cull-label:{}",
                LABELS
                    .get(usize::from(n).wrapping_sub(1))
                    .unwrap_or(&"none")
            ),
            Action::CullCopyTo(n) => format!("cull-copy-to:{n}"),
            Action::CullMoveTo(n) => format!("cull-move-to:{n}"),
            Action::WindowPreset(n) => format!("window-preset:{n}"),
            Action::ScienceChannel(n) => format!("science-channel:{n}"),
            Action::SeekPercent(n) => format!("seek-percent:{n}"),
            a => a.simple_id().to_string(),
        }
    }

    /// Parse an identifier produced by [`Action::id`].
    pub fn from_id(s: &str) -> Option<Action> {
        let s = s.trim();
        if let Some(p) = s
            .strip_prefix("zoom-to:")
            .or_else(|| s.strip_prefix("zoom-to "))
        {
            let p: f32 = p.trim().trim_end_matches('%').parse().ok()?;
            return (p > 0.0 && p.is_finite()).then_some(Action::ZoomTo(p));
        }
        if let Some(n) = s.strip_prefix("cull-rate:") {
            let n: u8 = n.trim().parse().ok()?;
            return (n <= 5).then_some(Action::CullRate(n));
        }
        if let Some(l) = s.strip_prefix("cull-label:") {
            let i = LABELS.iter().position(|x| *x == l.trim())?;
            return Some(Action::CullLabel(i as u8 + 1));
        }
        if let Some(n) = s.strip_prefix("cull-copy-to:") {
            let n: u8 = n.trim().parse().ok()?;
            return (1..=3).contains(&n).then_some(Action::CullCopyTo(n));
        }
        if let Some(n) = s.strip_prefix("cull-move-to:") {
            let n: u8 = n.trim().parse().ok()?;
            return (1..=3).contains(&n).then_some(Action::CullMoveTo(n));
        }
        if let Some(n) = s.strip_prefix("window-preset:") {
            let n: u8 = n.trim().parse().ok()?;
            return (n <= 9).then_some(Action::WindowPreset(n));
        }
        if let Some(n) = s.strip_prefix("seek-percent:") {
            let n: u8 = n.trim().trim_end_matches('%').parse().ok()?;
            return (n < 100).then_some(Action::SeekPercent(n));
        }
        if let Some(n) = s.strip_prefix("science-channel:") {
            let n: u8 = n.trim().parse().ok()?;
            return (1..=16).contains(&n).then_some(Action::ScienceChannel(n));
        }
        Action::BINDABLE
            .iter()
            .copied()
            .find(|a| !a.has_parameter() && a.simple_id() == s)
    }

    fn has_parameter(self) -> bool {
        matches!(
            self,
            Action::ZoomTo(_)
                | Action::CullRate(_)
                | Action::CullLabel(_)
                | Action::CullCopyTo(_)
                | Action::CullMoveTo(_)
                | Action::WindowPreset(_)
                | Action::ScienceChannel(_)
                | Action::SeekPercent(_)
        )
    }

    /// Fluent message ID of the action's one-sentence description (under the palette's
    /// list, and its row's accessible description): [`message_id`](Self::message_id) and
    /// "-help". Every [`LISTED`](Self::LISTED) action has one in English.
    pub fn help_id(self) -> String {
        format!("{}-help", self.message_id())
    }

    /// Actions that work while held (the cheat sheet marks them).
    pub fn is_hold(self) -> bool {
        matches!(self, Action::Loupe | Action::ZoomRectangle) || self.is_develop_speed()
    }

    /// Develop's speed-edit actions (held: scroll or arrows change their control).
    pub fn is_develop_speed(self) -> bool {
        DEVELOP_SPEED.contains(&self)
    }

    /// Develop's actions, hidden in the Medical and Science profiles (DESIGN §6.10).
    pub fn is_develop(self) -> bool {
        self == Action::ToggleDevelop
            || self.mode() == Mode::Develop
            || matches!(
                self,
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
                    | Action::DevelopFocusStack
            )
    }

    /// Actions that repeat (with acceleration) while their key is held.
    pub fn repeats(self) -> bool {
        matches!(
            self,
            Action::NextImage
                | Action::PrevImage
                | Action::ZoomIn
                | Action::ZoomOut
                | Action::PanLeft
                | Action::PanRight
                | Action::PanUp
                | Action::PanDown
                | Action::NextFrame
                | Action::PrevFrame
                | Action::NextSlice
                | Action::PrevSlice
                | Action::NextPage
                | Action::PrevPage
                | Action::ScienceNextPlane
                | Action::SciencePrevPlane
                | Action::MeshOrbitLeft
                | Action::MeshOrbitRight
                | Action::MeshOrbitUp
                | Action::MeshOrbitDown
                | Action::RelightLeft
                | Action::RelightRight
                | Action::RelightUp
                | Action::RelightDown
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bindable_action_round_trips_its_id() {
        for a in Action::BINDABLE {
            assert_eq!(Action::from_id(&a.id()), Some(*a), "{}", a.id());
        }
        for a in Action::LISTED {
            assert!(
                Action::BINDABLE.contains(a),
                "{a:?} is listed but not bindable"
            );
        }
        assert_eq!(
            Action::from_id("cull-label:green"),
            Some(Action::CullLabel(3))
        );
        assert_eq!(Action::from_id("cull-rate:6"), None);
        assert_eq!(
            Action::from_id("cull-move-to:2"),
            Some(Action::CullMoveTo(2))
        );
        assert_eq!(Action::from_id("cull-copy-to:4"), None);
        assert_eq!(
            Action::from_id("window-preset:1"),
            Some(Action::WindowPreset(1))
        );
        assert_eq!(Action::from_id("window-preset:12"), None);
        assert_eq!(
            Action::from_id("science-channel:2"),
            Some(Action::ScienceChannel(2))
        );
        assert_eq!(
            Action::from_id("science-channel:9"),
            Some(Action::ScienceChannel(9))
        );
        assert_eq!(
            Action::from_id("science-channel:16"),
            Some(Action::ScienceChannel(16))
        );
        assert_eq!(Action::from_id("science-channel:17"), None);
        assert_eq!(Action::from_id("cull-rate"), None);
        assert_eq!(Action::from_id("zoom-to"), None);
    }

    #[test]
    fn modes_list_their_actions() {
        assert!(Mode::Cull.actions().all(|a| a.mode() == Mode::Cull));
        assert_eq!(Mode::Cull.actions().count(), 34);
        assert_eq!(Mode::Cull.chain(), vec![Mode::Cull, Mode::Viewer]);
        assert!(Mode::Medical.actions().all(|a| a.mode() == Mode::Medical));
        assert_eq!(Mode::Medical.actions().count(), 64);
        assert_eq!(Mode::Medical.chain(), vec![Mode::Medical, Mode::Viewer]);
        assert!(Mode::Science.actions().all(|a| a.mode() == Mode::Science));
        assert_eq!(Mode::Science.actions().count(), 48);
        assert_eq!(Mode::Science.chain(), vec![Mode::Science, Mode::Viewer]);
        assert!(Mode::Mesh.actions().all(|a| a.mode() == Mode::Mesh));
        assert_eq!(Mode::Mesh.actions().count(), 33);
        assert_eq!(Mode::Mesh.chain(), vec![Mode::Mesh, Mode::Viewer]);
        assert!(Mode::Relight.actions().all(|a| a.mode() == Mode::Relight));
        assert_eq!(Mode::Relight.actions().count(), 7);
        assert_eq!(Mode::Relight.chain(), vec![Mode::Relight, Mode::Viewer]);
        assert!(Mode::Video.actions().all(|a| a.mode() == Mode::Video));
        assert_eq!(Mode::Video.actions().count(), 61);
        assert_eq!(Mode::Video.chain(), vec![Mode::Video, Mode::Viewer]);
        assert!(Mode::Map.actions().all(|a| a.mode() == Mode::Map));
        assert_eq!(Mode::Map.actions().count(), 26);
        assert_eq!(Mode::Map.chain(), vec![Mode::Map, Mode::Viewer]);
        // Every listed action belongs to the registry, whichever mode it is in.
        for a in Action::LISTED {
            assert!(a.mode().actions().any(|b| b == *a), "{a:?}");
        }
        for m in Mode::ALL {
            assert_eq!(Mode::from_id(m.id()), Some(m));
        }
    }
}
