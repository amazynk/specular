//! The actions, declared once: each one's keymap id, the message naming it, its cheat-sheet
//! group, its keymap layer and the manual's page and section about it. Adding an action is a
//! variant (`lib.rs`), a row here, its default keys (`keymap.rs`), when it can run
//! (`enabled.rs`) and its name and description in `en`. The names are read through
//! [`Action::name`] only (`names.rs`).
//!
//! An action that takes a value has one row for all of its values; [`Action::message_id`]
//! names the values that have a message of their own.

use crate::{Action, Group, Mode, Topic};

use Action as A;

macro_rules! actions {
    ($($pat:pat => $id:literal, $name:literal, $group:ident, $mode:ident, $page:literal, $anchor:literal;)*) => {
        impl Action {
            /// The keymap id without its value (`zoom-to` for `zoom-to:200`).
            pub(crate) fn simple_id(self) -> &'static str {
                match self {
                    $($pat => $id,)*
                }
            }

            /// The message naming the action (for an action with a value, the name of any
            /// value without one of its own).
            pub(crate) fn row_message_id(self) -> &'static str {
                match self {
                    $($pat => $name,)*
                }
            }

            /// Where the action is listed in the cheat sheet.
            pub fn group(self) -> Group {
                match self {
                    $($pat => Group::$group,)*
                }
            }

            /// The keymap layer the action belongs to.
            pub fn mode(self) -> Mode {
                match self {
                    $($pat => Mode::$mode,)*
                }
            }

            /// The page and section of the manual about the action.
            pub fn topic(self) -> Topic {
                match self {
                    $($pat => Topic { page: $page, anchor: $anchor },)*
                }
            }
        }
    };
}

actions! {
    A::NextImage => "next", "palette-next", Navigation, Viewer, "viewer", "navigate";
    A::PrevImage => "prev", "palette-prev", Navigation, Viewer, "viewer", "navigate";
    A::FirstImage => "first", "palette-first", Navigation, Viewer, "viewer", "navigate";
    A::LastImage => "last", "palette-last", Navigation, Viewer, "viewer", "navigate";
    A::GoToImage => "go-to", "palette-go-to", Navigation, Viewer, "viewer", "navigate";
    A::ZoomIn => "zoom-in", "palette-zoom-in", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomOut => "zoom-out", "palette-zoom-out", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomFit => "zoom-fit", "palette-zoom-fit", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomFill => "zoom-fill", "palette-zoom-fill", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomFitWidth => "zoom-fit-width", "palette-zoom-fit-width", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomFitHeight => "zoom-fit-height", "palette-zoom-fit-height", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomTo(_) => "zoom-to", "palette-zoom-100", ZoomPan, Viewer, "viewer", "zoom";
    A::ZoomRectangle => "zoom-rectangle", "palette-zoom-rectangle", ZoomPan, Viewer, "viewer", "zoom";
    A::RotateLeft => "rotate-left", "palette-rotate-left", View, Viewer, "inspect", "";
    A::RotateRight => "rotate-right", "palette-rotate-right", View, Viewer, "inspect", "";
    A::FlipHorizontal => "flip-horizontal", "palette-flip-horizontal", View, Viewer, "inspect", "";
    A::FlipVertical => "flip-vertical", "palette-flip-vertical", View, Viewer, "inspect", "";
    A::PanLeft => "pan-left", "action-pan-left", ZoomPan, Viewer, "viewer", "zoom";
    A::PanRight => "pan-right", "action-pan-right", ZoomPan, Viewer, "viewer", "zoom";
    A::PanUp => "pan-up", "action-pan-up", ZoomPan, Viewer, "viewer", "zoom";
    A::PanDown => "pan-down", "action-pan-down", ZoomPan, Viewer, "viewer", "zoom";
    A::PanPageLeft => "pan-page-left", "action-pan-page-left", ZoomPan, Viewer, "viewer", "zoom";
    A::PanPageRight => "pan-page-right", "action-pan-page-right", ZoomPan, Viewer, "viewer", "zoom";
    A::PanPageUp => "pan-page-up", "action-pan-page-up", ZoomPan, Viewer, "viewer", "zoom";
    A::PanPageDown => "pan-page-down", "action-pan-page-down", ZoomPan, Viewer, "viewer", "zoom";
    A::ToggleFullscreen => "fullscreen", "palette-toggle-fullscreen", Window, Viewer, "interface", "";
    A::Slideshow => "slideshow", "palette-slideshow", Window, Viewer, "interface", "";
    A::ViewerOnOtherMonitor => "viewer-on-other-monitor", "palette-viewer-other-monitor", Window, Viewer, "interface", "";
    A::ExportSlideshow => "export-slideshow", "palette-export-slideshow", Window, Viewer, "interface", "";
    A::ToggleCull => "cull", "palette-cull", Cull, Viewer, "viewer", "";
    A::Back => "back", "action-back", Window, Viewer, "interface", "";
    A::ToggleInfo => "info", "palette-info-panel", View, Viewer, "inspect", "";
    A::ToggleNavigator => "navigator", "palette-navigator", ZoomPan, Viewer, "viewer", "zoom";
    A::Loupe => "loupe", "palette-loupe", ZoomPan, Viewer, "viewer", "zoom";
    A::TogglePixelGrid => "pixel-grid", "palette-pixel-grid", View, Viewer, "inspect", "";
    A::ToggleClipping => "clipping", "palette-clipping", View, Viewer, "inspect", "";
    A::ToggleFocusPeaking => "focus-peaking", "palette-focus-peaking", View, Viewer, "inspect", "";
    A::ToggleHistogram => "histogram", "palette-histogram", View, Viewer, "inspect", "";
    A::TogglePixelInspector => "pixel-inspector", "palette-pixel-inspector", View, Viewer, "inspect", "";
    A::ToggleCrosshair => "crosshair", "palette-crosshair", View, Viewer, "accessibility", "crosshair";
    A::AnnounceCursor => "announce-cursor", "palette-announce-cursor", View, Viewer, "accessibility", "crosshair";
    A::ChannelRed => "channel-red", "palette-channel-red", View, Viewer, "inspect", "";
    A::ChannelGreen => "channel-green", "palette-channel-green", View, Viewer, "inspect", "";
    A::ChannelBlue => "channel-blue", "palette-channel-blue", View, Viewer, "inspect", "";
    A::ChannelAlpha => "channel-alpha", "palette-channel-alpha", View, Viewer, "inspect", "";
    A::ChannelLuma => "channel-luma", "palette-channel-luma", View, Viewer, "inspect", "";
    A::ToggleFalseColour => "false-colour", "palette-false-colour", View, Viewer, "inspect", "";
    A::NextColourMap => "next-colour-map", "palette-next-colour-map", View, Viewer, "inspect", "";
    A::NextChannelColours => "channel-colours", "palette-channel-colours", View, Viewer, "inspect", "";
    A::ToggleHiLo => "hilo", "palette-hilo", View, Viewer, "inspect", "";
    A::NextColourVision => "colour-vision", "palette-colour-vision", View, Viewer, "inspect", "";
    A::SimulateProtanopia => "colour-vision-protanopia", "palette-colour-vision-protanopia", View, Viewer, "inspect", "";
    A::SimulateDeuteranopia => "colour-vision-deuteranopia", "palette-colour-vision-deuteranopia", View, Viewer, "inspect", "";
    A::SimulateTritanopia => "colour-vision-tritanopia", "palette-colour-vision-tritanopia", View, Viewer, "inspect", "";
    A::SimulateAchromatopsia => "colour-vision-achromatopsia", "palette-colour-vision-achromatopsia", View, Viewer, "inspect", "";
    A::CopyImage => "copy-image", "palette-copy-image", File, Viewer, "files", "";
    A::ReadCodes => "read-codes", "palette-read-codes", File, Viewer, "files", "codes";
    A::FindText => "find-text", "palette-find-text", File, Viewer, "ai", "find-text";
    A::FaceBoxes => "face-boxes", "palette-face-boxes", View, Viewer, "ai", "faces";
    A::CopyPath => "copy-path", "palette-copy-path", File, Viewer, "files", "";
    A::OpenWith => "open-with", "palette-open-with", File, Viewer, "files", "";
    A::OpenAs => "open-as", "palette-open-as", File, Viewer, "files", "";
    A::Print => "print", "palette-print", File, Viewer, "files", "";
    A::ClientGallery => "client-gallery", "palette-client-gallery", File, Viewer, "client-gallery", "";
    A::ImportClientPicks => "import-client-picks", "palette-import-client-picks", File, Viewer, "client-gallery", "client-picks";
    A::ExportFigure => "export-figure", "palette-export-figure", File, Viewer, "figures", "";
    A::ToggleKeyImage => "toggle-key-image", "palette-toggle-key-image", File, Viewer, "figures", "key-images";
    A::CopyKeyImages => "copy-key-images", "palette-copy-key-images", File, Viewer, "figures", "key-images";
    A::ExportKeyImages => "export-key-images", "palette-export-key-images", File, Viewer, "figures", "key-images";
    A::Convert => "convert", "palette-convert", File, Viewer, "files", "";
    A::ExportRecipes => "export-recipes", "palette-export-recipes", File, Viewer, "export-recipes", "";
    A::Import => "import", "palette-import", File, Viewer, "files", "";
    A::Paste => "paste", "palette-paste", File, Viewer, "files", "";
    A::MoveToTrash => "trash", "palette-move-to-trash", File, Viewer, "files", "";
    A::Undo => "undo", "palette-undo", File, Viewer, "files", "";
    A::Redo => "redo", "palette-redo", File, Viewer, "files", "";
    A::Rename => "rename", "palette-rename", File, Viewer, "files", "";
    A::BatchRename => "batch-rename", "palette-batch-rename", File, Viewer, "files", "";
    A::CopyToFolder => "copy-to-folder", "palette-copy-to-folder", File, Viewer, "files", "";
    A::MoveToFolder => "move-to-folder", "palette-move-to-folder", File, Viewer, "files", "";
    A::RotateFileLeft => "rotate-file-left", "palette-rotate-file-left", File, Viewer, "files", "";
    A::RotateFileRight => "rotate-file-right", "palette-rotate-file-right", File, Viewer, "files", "";
    A::SetWallpaper => "set-wallpaper", "palette-set-wallpaper", File, Viewer, "files", "";
    A::Share => "share", "palette-share", File, Viewer, "files", "";
    A::SendByEmail => "send-by-email", "palette-send-by-email", File, Viewer, "files", "";
    A::ShareWithZynk => "share-with-zynk", "palette-share-with-zynk", File, Viewer, "files", "";
    A::SendWithZynk => "send-with-zynk", "palette-send-with-zynk", File, Viewer, "files", "";
    A::ExportSettings => "export-settings", "palette-export-settings", Window, Viewer, "interface", "";
    A::ImportSettings => "import-settings", "palette-import-settings", Window, Viewer, "interface", "";
    A::Open => "open", "palette-open", File, Viewer, "files", "";
    A::OpenAddress => "open-address", "palette-open-address", File, Viewer, "opening", "open-address";
    A::OpenStream => "open-stream", "palette-open-stream", File, Viewer, "opening", "network-streams";
    A::Settings => "settings", "palette-settings", Window, Viewer, "interface", "";
    A::CustomiseToolbar => "customise-toolbar", "palette-customise-toolbar", Window, Viewer, "interface", "";
    A::Palette => "palette", "toolbar-palette", Help, Viewer, "interface", "";
    A::CheatSheet => "cheat-sheet", "palette-cheat-sheet", Help, Viewer, "interface", "";
    A::About => "about", "about-title", Help, Viewer, "interface", "";
    A::ShowWelcome => "show-welcome", "palette-show-welcome", Help, Viewer, "interface", "";
    A::Help => "help", "palette-help-manual", Help, Viewer, "interface", "";
    A::JoinDiscord => "join-discord", "toolbar-discord", Help, Viewer, "interface", "toolbar";
    A::SaveDiagnostics => "save-diagnostics", "palette-save-diagnostics", Help, Viewer, "interface", "";
    A::Hud => "hud", "palette-hud", Help, Viewer, "interface", "";
    A::Language => "language", "palette-language", Window, Viewer, "interface", "";
    A::ToggleTheme => "theme", "palette-theme", Window, Viewer, "interface", "";
    A::InterfaceLarger => "interface-larger", "palette-interface-larger", Window, Viewer, "interface", "";
    A::InterfaceSmaller => "interface-smaller", "palette-interface-smaller", Window, Viewer, "interface", "";
    A::InterfaceReset => "interface-reset", "palette-interface-reset", Window, Viewer, "interface", "";
    A::Reload => "reload", "action-reload", File, Viewer, "files", "";
    A::Close => "close", "palette-close", Window, Viewer, "interface", "";
    A::Quit => "quit", "palette-quit", Window, Viewer, "interface", "";
    A::PlayPause => "play-pause", "palette-play-pause", View, Viewer, "inspect", "";
    A::NextFrame => "next-frame", "palette-next-frame", View, Viewer, "inspect", "";
    A::PrevFrame => "prev-frame", "palette-prev-frame", View, Viewer, "inspect", "";
    A::FasterPlayback => "faster", "palette-faster", View, Viewer, "inspect", "";
    A::SlowerPlayback => "slower", "palette-slower", View, Viewer, "inspect", "";
    A::NormalPlayback => "normal-speed", "palette-normal-speed", View, Viewer, "inspect", "";
    A::ToggleLoopForever => "loop-forever", "palette-loop-forever", View, Viewer, "inspect", "";
    A::PlayPauseVideo => "play-pause-video", "palette-play-pause-video", View, Video, "video", "";
    A::SeekForward => "seek-forward", "palette-seek-forward", View, Video, "video", "";
    A::SeekBack => "seek-back", "palette-seek-back", View, Video, "video", "";
    A::VolumeUp => "volume-up", "palette-volume-up", View, Video, "video", "";
    A::VolumeDown => "volume-down", "palette-volume-down", View, Video, "video", "";
    A::ToggleMute => "mute", "palette-mute", View, Video, "video", "";
    A::VideoFullscreen => "video-fullscreen", "palette-video-fullscreen", View, Video, "video", "fullscreen";
    A::SeekForwardLong => "seek-forward-long", "palette-seek-forward-long", View, Video, "video", "";
    A::SeekBackLong => "seek-back-long", "palette-seek-back-long", View, Video, "video", "";
    A::SeekPercent(_) => "seek-percent", "palette-seek-percent", View, Video, "video", "";
    A::CycleSubtitles => "cycle-subtitles", "palette-cycle-subtitles", View, Video, "video", "subtitles";
    A::OpenSubtitle => "open-subtitle", "palette-open-subtitle", View, Video, "video", "subtitles";
    A::GenerateSubtitles => "generate-subtitles", "palette-generate-subtitles", View, Video, "video", "generated-subtitles";
    A::SaveSubtitles => "save-subtitles", "palette-save-subtitles", View, Video, "video", "generated-subtitles";
    A::SubtitleEarlier => "subtitle-earlier", "palette-subtitle-earlier", View, Video, "video", "align";
    A::SubtitleLater => "subtitle-later", "palette-subtitle-later", View, Video, "video", "align";
    A::SubtitleEarlierFine => "subtitle-earlier-fine", "palette-subtitle-earlier-fine", View, Video, "video", "align";
    A::SubtitleLaterFine => "subtitle-later-fine", "palette-subtitle-later-fine", View, Video, "video", "align";
    A::SubtitleStepPrev => "subtitle-step-prev", "palette-subtitle-step-prev", View, Video, "video", "align";
    A::SubtitleStepNext => "subtitle-step-next", "palette-subtitle-step-next", View, Video, "video", "align";
    A::SubtitleUp => "subtitle-up", "palette-subtitle-up", View, Video, "video", "subtitle-look";
    A::SubtitleDown => "subtitle-down", "palette-subtitle-down", View, Video, "video", "subtitle-look";
    A::SubtitleBigger => "subtitle-bigger", "palette-subtitle-bigger", View, Video, "video", "subtitle-look";
    A::SubtitleSmaller => "subtitle-smaller", "palette-subtitle-smaller", View, Video, "video", "subtitle-look";
    A::SubtitleResetPlace => "subtitle-reset-place", "palette-subtitle-reset-place", View, Video, "video", "subtitle-look";
    A::SubtitleStyle => "subtitle-style", "palette-subtitle-style", View, Video, "video", "subtitle-look";
    A::CycleSound => "cycle-sound", "palette-cycle-sound", View, Video, "video", "sound-tracks";
    A::StreamQuality => "stream-quality", "palette-stream-quality", View, Video, "video", "stream-quality";
    A::DialogueBoost => "dialogue-boost", "palette-dialogue-boost", View, Video, "video", "sound-effects";
    A::Equaliser => "equaliser", "palette-equaliser", View, Video, "video", "sound-effects";
    A::SoundEarlier => "sound-earlier", "palette-sound-earlier", View, Video, "video", "align";
    A::SoundLater => "sound-later", "palette-sound-later", View, Video, "video", "align";
    A::SoundEarlierFine => "sound-earlier-fine", "palette-sound-earlier-fine", View, Video, "video", "align";
    A::SoundLaterFine => "sound-later-fine", "palette-sound-later-fine", View, Video, "video", "align";
    A::ResetDelays => "reset-delays", "palette-reset-delays", View, Video, "video", "align";
    A::PrevChapter => "prev-chapter", "palette-prev-chapter", View, Video, "video", "chapters";
    A::NextChapter => "next-chapter", "palette-next-chapter", View, Video, "video", "chapters";
    A::AbLoop => "ab-loop", "palette-ab-loop", View, Video, "video", "ab";
    A::SaveFrame => "save-frame", "palette-save-frame", View, Video, "video", "frames";
    A::CopyFrame => "copy-frame", "palette-copy-frame", View, Video, "video", "frames";
    A::MiniPlayer => "mini-player", "palette-mini-player", View, Video, "video", "mini";
    A::CycleAspect => "cycle-aspect", "palette-cycle-aspect", View, Video, "video", "picture";
    A::CycleCrop => "cycle-crop", "palette-cycle-crop", View, Video, "video", "picture";
    A::NextTrack => "next-track", "palette-next-track", View, Video, "video", "queue";
    A::PrevTrack => "prev-track", "palette-prev-track", View, Video, "video", "queue";
    A::CycleRepeat => "cycle-repeat", "palette-cycle-repeat", View, Video, "video", "queue";
    A::ToggleShuffle => "shuffle", "palette-shuffle", View, Video, "video", "queue";
    A::ToggleQueue => "play-queue", "palette-play-queue", Window, Viewer, "video", "queue";
    A::QueuePlayNext => "queue-play-next", "palette-queue-play-next", Window, Viewer, "video", "queue";
    A::QueueAdd => "queue-add", "palette-queue-add", Window, Viewer, "video", "queue";
    A::SaveQueue => "save-queue", "palette-save-queue", Window, Viewer, "video", "queue";
    A::CycleSoundVisuals => "cycle-sound-visuals", "palette-cycle-sound-visuals", View, Video, "video", "sound-visuals";
    A::ToggleLyrics => "lyrics", "palette-toggle-lyrics", View, Video, "video", "lyrics";
    A::GoToTime => "go-to-time", "palette-go-to-time", View, Video, "video", "go-to-time";
    A::VideoPicture => "video-picture", "palette-video-picture", View, Video, "video", "picture";
    A::CastMenu => "cast-menu", "palette-cast-menu", Window, Video, "video", "cast";
    A::ToggleRightToLeft => "right-to-left", "palette-right-to-left", Navigation, Viewer, "viewer", "navigate";
    A::ToggleSpreads => "spreads", "palette-spreads", Navigation, Viewer, "viewer", "navigate";
    A::ToggleContents => "contents", "palette-contents", Navigation, Viewer, "viewer", "navigate";
    A::ToggleGallery => "gallery", "palette-gallery", Navigation, Viewer, "viewer", "navigate";
    A::CustomiseKeys => "customise-keys", "palette-customise-keys", Window, Viewer, "interface", "";
    A::CullPick => "cull-pick", "gallery-pick", Cull, Cull, "cull", "";
    A::CullReject => "cull-reject", "gallery-reject", Cull, Cull, "cull", "";
    A::CullUnflag => "cull-unflag", "gallery-unflag", Cull, Cull, "cull", "";
    A::CullRate(_) => "cull-rate", "gallery-stars", Cull, Cull, "cull", "";
    A::CullLabel(_) => "cull-label", "gallery-label-red", Cull, Cull, "cull", "";
    A::CullCopyTo(_) => "cull-copy-to", "cull-key-copy", Cull, Cull, "cull", "";
    A::CullMoveTo(_) => "cull-move-to", "cull-key-move", Cull, Cull, "cull", "";
    A::CullBurst => "cull-burst", "cull-key-burst", Cull, Cull, "cull", "";
    A::CullKeepBest => "cull-keep-best", "cull-key-best", Cull, Cull, "cull", "";
    A::CullFollow => "cull-follow", "cull-key-follow", Cull, Cull, "cull", "";
    A::CullSubframes => "cull-subframes", "cull-key-subframes", Cull, Cull, "cull", "subframes";
    A::CullSortNext => "cull-sort-next", "cull-key-sort", Cull, Cull, "cull", "subframes";
    A::CullSortReverse => "cull-sort-reverse", "cull-key-sort-reverse", Cull, Cull, "cull", "subframes";
    A::CullMetricNext => "cull-metric-next", "cull-key-metric", Cull, Cull, "cull", "subframes";
    A::CullSigmaDown => "cull-sigma-down", "cull-key-sigma-down", Cull, Cull, "cull", "subframes";
    A::CullSigmaUp => "cull-sigma-up", "cull-key-sigma-up", Cull, Cull, "cull", "subframes";
    A::CullAutoReject => "cull-auto-reject", "cull-key-auto-reject", Cull, Cull, "cull", "subframes";
    A::CullMoveRejects => "cull-move-rejects", "cull-key-move-rejects", Cull, Cull, "cull", "subframes";
    A::CullSameStretch => "cull-same-stretch", "cull-key-same-stretch", Cull, Cull, "cull", "subframes";
    A::ToggleLibrary => "library", "palette-library", Navigation, Viewer, "viewer", "navigate";
    A::Keywords => "keywords", "palette-keywords", Navigation, Viewer, "gallery", "keywords";
    A::EditMetadata => "edit-metadata", "palette-edit-metadata", Navigation, Viewer, "gallery", "metadata";
    A::WindowLevelTool => "window-level", "palette-window-level", Medical, Medical, "medical", "";
    A::WindowPreset(_) => "window-preset", "palette-window-default", Medical, Medical, "medical", "";
    A::InvertGrey => "invert-grey", "palette-invert-grey", Medical, Medical, "medical", "";
    A::NextSlice => "next-slice", "palette-next-slice", Medical, Medical, "medical", "";
    A::PrevSlice => "prev-slice", "palette-prev-slice", Medical, Medical, "medical", "";
    A::Cine => "cine", "palette-cine", Medical, Medical, "medical", "";
    A::MeasureLength => "measure-length", "palette-measure-length", Medical, Medical, "medical", "";
    A::MeasureAngle => "measure-angle", "palette-measure-angle", Medical, Medical, "medical", "";
    A::MeasureEllipse => "measure-ellipse", "palette-measure-ellipse", Medical, Medical, "medical", "";
    A::MeasureRectangle => "measure-rectangle", "palette-measure-rectangle", Medical, Medical, "medical", "";
    A::ClearMeasurements => "clear-measurements", "palette-clear-measurements", Medical, Medical, "medical", "";
    A::MeasureTemplates => "measure-templates", "palette-measure-templates", Medical, Viewer, "measure", "geometry";
    A::ToggleOverlays => "overlays", "palette-overlays", Medical, Medical, "medical", "";
    A::ToggleOrientationLabels => "orientation-labels", "palette-orientation-labels", Medical, Medical, "medical", "";
    A::NextPresentationState => "presentation-state", "palette-presentation-state", Medical, Medical, "medical", "";
    A::Mpr => "mpr", "palette-mpr", View, Viewer, "medical-3d", "mpr";
    A::MprSlab => "mpr-slab", "palette-mpr-slab", Medical, Medical, "medical", "";
    A::MprTractSlab => "mpr-tract-slab", "palette-mpr-tract-slab", Medical, Medical, "medical", "";
    A::MprPreset => "mpr-preset", "palette-mpr-preset", Medical, Medical, "medical", "";
    A::MprRender => "mpr-render", "palette-mpr-render", Medical, Medical, "medical", "";
    A::CompareSeries => "compare-series", "palette-compare-series", Medical, Medical, "medical-compare", "";
    A::CompareLayout => "compare-layout", "palette-compare-layout", Medical, Medical, "medical-compare", "layout";
    A::NextViewport => "next-viewport", "palette-next-viewport", Medical, Medical, "medical-compare", "layout";
    A::PrevViewport => "prev-viewport", "palette-prev-viewport", Medical, Medical, "medical-compare", "layout";
    A::ReferenceLines => "reference-lines", "palette-reference-lines", Medical, Medical, "medical-compare", "lines";
    A::SyncWindow => "sync-window", "palette-sync-window", Medical, Medical, "medical-compare", "sync";
    A::SyncZoom => "sync-zoom", "palette-sync-zoom", Medical, Medical, "medical-compare", "sync";
    A::LinkSeries => "link-series", "palette-link-series", Medical, Medical, "medical-compare", "priors";
    A::AlignSeries => "align-series", "palette-align-series", Medical, Medical, "medical-compare", "priors";
    A::MprClip => "mpr-clip", "palette-mpr-clip", Medical, Medical, "medical", "";
    A::MprExportStl => "mpr-export-stl", "palette-mpr-export-stl", Medical, Medical, "medical", "";
    A::MprMaximise => "mpr-maximise", "palette-mpr-maximise", Medical, Medical, "medical-3d", "mpr";
    A::MprExportPlanes => "mpr-export-planes", "palette-mpr-export-planes", Medical, Medical, "medical-3d", "mpr";
    A::MprCurve => "mpr-curve", "palette-mpr-curve", Medical, Medical, "medical-3d", "curved";
    A::MprCurveLayout => "mpr-curve-layout", "palette-mpr-curve-layout", Medical, Medical, "medical-3d", "curved";
    A::MprCurveSave => "mpr-curve-save", "palette-mpr-curve-save", Medical, Medical, "medical-3d", "curved";
    A::MprCurveLoad => "mpr-curve-load", "palette-mpr-curve-load", Medical, Medical, "medical-3d", "curved";
    A::MprExportSegments => "mpr-export-segments", "palette-mpr-export-segments", Medical, Medical, "medical-3d", "segments";
    A::MprDental => "mpr-dental", "palette-mpr-dental", Medical, Medical, "medical-3d", "dental";
    A::MprNerve => "mpr-nerve", "palette-mpr-nerve", Medical, Medical, "medical-3d", "dental";
    A::ScienceStretch => "stretch", "palette-science-stretch", Science, Science, "science", "";
    A::ScienceLimits => "limits", "palette-science-limits", Science, Science, "science", "";
    A::ScienceNextPlane => "next-plane", "palette-science-next-plane", Science, Science, "science", "";
    A::SciencePrevPlane => "prev-plane", "palette-science-prev-plane", Science, Science, "science", "";
    A::ScienceNextAxis => "next-axis", "palette-science-next-axis", Science, Science, "science", "";
    A::ScienceComposite => "composite", "palette-science-composite", Science, Science, "science", "";
    A::ScienceChannel(_) => "science-channel", "palette-science-channel-1", Science, Science, "science", "";
    A::ScienceGrid => "coordinate-grid", "palette-science-grid", Science, Science, "science", "";
    A::ScienceLevel => "level-plane", "palette-science-level", Science, Science, "science", "";
    A::ScienceHillshade => "hillshade", "palette-science-hillshade", Science, Science, "science", "";
    A::ScienceWiggle => "seismic-wiggle", "palette-science-wiggle", Science, Science, "science", "";
    A::ScienceWireframe => "mesh-wireframe", "palette-science-wireframe", Science, Science, "science", "";
    A::ScienceReset => "reset-look", "palette-science-reset", Science, Science, "science", "";
    A::ScienceImportLut => "import-lut", "palette-science-import-lut", Science, Science, "science", "";
    A::ScienceFft => "power-spectrum", "palette-science-fft", Science, Science, "science", "";
    A::ScienceCatalogue => "open-catalogue", "palette-science-catalogue", Science, Science, "science", "";
    A::ScienceClearCatalogues => "clear-catalogues", "palette-science-clear-catalogues", Science, Science, "science", "";
    A::ScienceLoadRegions => "load-regions", "palette-science-load-regions", Science, Science, "science", "astronomy";
    A::ScienceExportRegions => "export-regions", "palette-science-export-regions", Science, Science, "science", "astronomy";
    A::ScienceEditRegions => "edit-regions", "palette-science-edit-regions", Science, Science, "science", "astronomy";
    A::ScienceClearRegions => "clear-regions", "palette-science-clear-regions", Science, Science, "science", "astronomy";
    A::PlateSolve => "plate-solve", "palette-science-plate-solve", Science, Science, "science", "astronomy";
    A::PlateSolveSave => "save-plate-solution", "palette-science-save-plate-solution", Science, Science, "science", "astronomy";
    A::SkySurvey => "sky-survey", "palette-science-sky-survey", Science, Science, "science", "astronomy";
    A::SampSendImage => "samp-send-image", "palette-science-samp-send-image", Science, Science, "science", "astronomy";
    A::SampSendPosition => "samp-send-position", "palette-science-samp-send-position", Science, Science, "science", "astronomy";
    A::ScienceIonImage => "ion-image", "palette-science-ion-image", Science, Science, "science", "";
    A::ScienceTemperatureUnit => "temperature-unit", "palette-science-temperature-unit", Science, Science, "science", "";
    A::ScienceIsotherm => "isotherm", "palette-science-isotherm", Science, Science, "science", "";
    A::ScienceThermalParameters => "thermal-parameters", "palette-science-thermal-parameters", Science, Science, "science", "";
    A::ScienceExportTemperatures => "export-temperatures", "palette-science-export-temperatures", Science, Science, "science", "";
    A::ScienceThermalPlay => "thermal-play", "palette-science-thermal-play", Science, Science, "science", "";
    A::ScienceThermalSeriesCsv => "thermal-series-csv", "palette-science-thermal-series-csv", Science, Science, "science", "";
    A::ThermalReport => "thermal-report", "palette-thermal-report", File, Viewer, "measure", "report";
    A::ToggleThermalVisible => "thermal-visible", "palette-thermal-visible", Science, Viewer, "viewer", "";
    A::ToggleDebayer => "debayer", "palette-debayer", Science, Viewer, "viewer", "";
    A::ToggleAnnotations => "annotations", "palette-annotations", Science, Viewer, "viewer", "";
    A::NextBayerPattern => "bayer-pattern", "palette-bayer-pattern", Science, Viewer, "viewer", "";
    A::StainSeparation => "stain-separation", "palette-stain-separation", Science, Viewer, "pathology", "stains";
    A::StainShow1 => "stain-1", "palette-stain-1", Science, Viewer, "pathology", "stains";
    A::StainShow2 => "stain-2", "palette-stain-2", Science, Viewer, "pathology", "stains";
    A::StainShow3 => "stain-3", "palette-stain-3", Science, Viewer, "pathology", "stains";
    A::StainMode => "stain-mode", "palette-stain-mode", Science, Viewer, "pathology", "stains";
    A::StainEstimate => "stain-estimate", "palette-stain-estimate", Science, Viewer, "pathology", "estimate";
    A::StainNormalise => "stain-normalise", "palette-stain-normalise", Science, Viewer, "pathology", "estimate";
    A::StainReference => "stain-reference", "palette-stain-reference", Science, Viewer, "pathology", "estimate";
    A::StainReset => "stain-reset", "palette-stain-reset", Science, Viewer, "pathology", "estimate";
    A::ExportAnnotations => "export-annotations", "palette-export-annotations", Science, Viewer, "pathology", "export";
    A::ImportAnnotations => "import-annotations", "palette-import-annotations", Science, Viewer, "pathology", "export";
    A::LabelVolume => "label-volume", "palette-label-volume", Medical, Medical, "medical", "label-volumes";
    A::Segments => "segments", "palette-segments", Medical, Medical, "medical", "";
    A::SegmentOpacity => "segment-opacity", "palette-segment-opacity", Medical, Medical, "medical", "";
    A::Fusion => "fusion", "palette-fusion", Medical, Medical, "medical", "";
    A::FusionColormap => "fusion-colormap", "palette-fusion-colormap", Medical, Medical, "medical", "";
    A::FusionBlend => "fusion-blend", "palette-fusion-blend", Medical, Medical, "medical", "";
    A::FusionNudge => "fusion-nudge", "palette-fusion-nudge", Medical, Medical, "medical", "";
    A::Tmtv => "tmtv", "palette-tmtv", Medical, Medical, "medical", "";
    A::TmtvThreshold => "tmtv-threshold", "palette-tmtv-threshold", Medical, Medical, "medical", "";
    A::TimeCurve => "time-curve", "palette-time-curve", Medical, Medical, "medical", "";
    A::ExportCurve => "export-curve", "palette-export-curve", Medical, Medical, "medical", "";
    A::DrawingLayers => "drawing-layers", "palette-drawing-layers", Drawing, Drawing, "drawing", "";
    A::DrawingAllLayers => "drawing-all-layers", "palette-drawing-all-layers", Drawing, Drawing, "drawing", "";
    A::DrawingPaper => "drawing-paper", "palette-drawing-paper", Drawing, Drawing, "drawing", "";
    A::DrawingLineweights => "drawing-lineweights", "palette-drawing-lineweights", Drawing, Drawing, "drawing", "";
    A::MeshOrbitLeft => "orbit-left", "palette-mesh-orbit-left", Mesh, Mesh, "mesh", "";
    A::MeshOrbitRight => "orbit-right", "palette-mesh-orbit-right", Mesh, Mesh, "mesh", "";
    A::MeshOrbitUp => "orbit-up", "palette-mesh-orbit-up", Mesh, Mesh, "mesh", "";
    A::MeshOrbitDown => "orbit-down", "palette-mesh-orbit-down", Mesh, Mesh, "mesh", "";
    A::MeshViewFront => "view-front", "palette-mesh-view-front", Mesh, Mesh, "mesh", "";
    A::MeshViewTop => "view-top", "palette-mesh-view-top", Mesh, Mesh, "mesh", "";
    A::MeshViewSide => "view-side", "palette-mesh-view-side", Mesh, Mesh, "mesh", "";
    A::MeshViewIso => "view-iso", "palette-mesh-view-iso", Mesh, Mesh, "mesh", "";
    A::MeshShading => "shading", "palette-mesh-shading", Mesh, Mesh, "mesh", "";
    A::MeshProjection => "projection", "palette-mesh-projection", Mesh, Mesh, "mesh", "";
    A::MeshGrid => "ground-grid", "palette-mesh-ground-grid", Mesh, Mesh, "mesh", "";
    A::MeshPointsBigger => "points-bigger", "palette-mesh-points-bigger", Mesh, Mesh, "mesh", "";
    A::MeshPointsSmaller => "points-smaller", "palette-mesh-points-smaller", Mesh, Mesh, "mesh", "";
    A::MeshPointColour => "point-colour", "palette-mesh-point-colour", Mesh, Mesh, "mesh", "";
    A::MeshEdl => "eye-dome-lighting", "palette-mesh-eye-dome-lighting", Mesh, Mesh, "mesh", "";
    A::MeshMolStyle => "molecule-style", "palette-mesh-molecule-style", Mesh, Mesh, "mesh", "";
    A::MeshMolColour => "molecule-colour", "palette-mesh-molecule-colour", Mesh, Mesh, "mesh", "";
    A::MeshTractOpacity => "tract-opacity", "palette-mesh-tract-opacity", Mesh, Mesh, "mesh", "";
    A::MeshWalk => "walk", "palette-mesh-walk", Mesh, Mesh, "mesh", "";
    A::MeshFly => "fly", "palette-mesh-fly", Mesh, Mesh, "mesh", "";
    A::MeshViewpoints => "views", "palette-mesh-views", Mesh, Mesh, "mesh", "";
    A::MeshSaveViewpoint => "save-view", "palette-mesh-save-view", Mesh, Mesh, "mesh", "";
    A::MeshSectionPlane => "section-plane", "palette-mesh-section-plane", Mesh, Mesh, "mesh", "";
    A::MeshSectionBox => "section-box", "palette-mesh-section-box", Mesh, Mesh, "mesh", "";
    A::MeshSectionFlip => "section-flip", "palette-mesh-section-flip", Mesh, Mesh, "mesh", "";
    A::MeshSectionClear => "section-clear", "palette-mesh-section-clear", Mesh, Mesh, "mesh", "";
    A::MeshCutMeasure => "measure-cut", "palette-mesh-measure-cut", Mesh, Mesh, "mesh", "";
    A::MeshBcf => "bcf-topics", "palette-mesh-bcf-topics", Mesh, Mesh, "mesh", "";
    A::MeshChecks => "model-checks", "palette-mesh-model-checks", Mesh, Mesh, "mesh", "";
    A::MeshModels => "models", "palette-mesh-models", Mesh, Mesh, "mesh", "";
    A::MeshAddModel => "add-model", "palette-mesh-add-model", Mesh, Mesh, "mesh", "";
    A::MeshAddModelFolder => "add-model-folder", "palette-mesh-add-model-folder", Mesh, Mesh, "mesh", "";
    A::MeshPmi => "pmi", "palette-mesh-pmi", Mesh, Mesh, "mesh", "";
    A::RelightLeft => "light-left", "palette-relight-left", Relight, Relight, "relight", "";
    A::RelightRight => "light-right", "palette-relight-right", Relight, Relight, "relight", "";
    A::RelightUp => "light-up", "palette-relight-up", Relight, Relight, "relight", "";
    A::RelightDown => "light-down", "palette-relight-down", Relight, Relight, "relight", "";
    A::RelightMode => "relight-shading", "palette-relight-shading", Relight, Relight, "relight", "";
    A::RelightReset => "light-reset", "palette-relight-reset", Relight, Relight, "relight", "";
    A::RelightWidget => "light-widget", "palette-relight-widget", Relight, Relight, "relight", "";
    A::DicomTags => "dicom-tags", "palette-dicom-tags", View, Viewer, "inspect", "";
    A::Studies => "studies", "palette-studies", View, Viewer, "inspect", "";
    A::ExportAnonymised => "export-anonymised", "palette-export-anonymised", Medical, Medical, "medical", "";
    A::ExportPatientDisc => "export-patient-disc", "palette-export-patient-disc", Medical, Medical, "medical", "";
    A::MaskBurnedInText => "mask-burned-in-text", "palette-mask-burned-in-text", Medical, Medical, "medical", "";
    A::ExportTeachingFile => "export-teaching-file", "palette-export-teaching-file", Medical, Medical, "figures", "teaching";
    A::TeachingQuiz => "teaching-quiz", "palette-teaching-quiz", Medical, Medical, "figures", "quiz";
    A::SendToPacs => "send-to-pacs", "palette-send-to-pacs", Medical, Medical, "medical", "send-to-pacs";
    A::DicomServers => "dicom-servers", "palette-dicom-servers", File, Viewer, "files", "";
    A::OmeroServers => "omero-servers", "palette-omero-servers", File, Viewer, "files", "";
    A::Tether => "tether", "palette-tether", File, Viewer, "tether", "";
    A::TetherCapture => "tether-capture", "palette-tether-capture", File, Viewer, "tether", "capture";
    A::NextSeries => "next-series", "palette-next-series", Navigation, Viewer, "viewer", "navigate";
    A::PrevSeries => "prev-series", "palette-prev-series", Navigation, Viewer, "viewer", "navigate";
    A::NextPage => "next-page", "palette-next-page", Navigation, Viewer, "viewer", "navigate";
    A::PrevPage => "prev-page", "palette-prev-page", Navigation, Viewer, "viewer", "navigate";
    A::FirstPage => "first-page", "palette-first-page", Navigation, Viewer, "viewer", "navigate";
    A::LastPage => "last-page", "palette-last-page", Navigation, Viewer, "viewer", "navigate";
    A::Measure => "measure", "palette-measure", View, Viewer, "inspect", "";
    A::MeasureCopyCsv => "measure-copy-csv", "palette-measure-copy-csv", View, Viewer, "inspect", "";
    A::MeasureExport => "measure-export", "palette-measure-export", View, Viewer, "inspect", "";
    A::Markup => "markup", "palette-markup", View, Viewer, "drawing", "markup";
    A::CompareSheets => "compare-sheets", "palette-compare-sheets", View, Viewer, "drawing", "compare";
    A::CompareWith => "compare-with", "palette-compare-with", View, Viewer, "compare", "";
    A::CompareSide => "compare-side", "palette-compare-side", View, Viewer, "compare", "";
    A::CompareSplit => "compare-split", "palette-compare-split", View, Viewer, "compare", "split";
    A::CompareSplitDirection => "compare-split-direction", "palette-compare-split-direction", View, Viewer, "compare", "split";
    A::CompareOverlay => "compare-overlay", "palette-compare-overlay", View, Viewer, "compare", "overlay";
    A::CompareDifference => "compare-difference", "palette-compare-difference", View, Viewer, "compare", "difference";
    A::CompareFlicker => "compare-flicker", "palette-compare-flicker", View, Viewer, "compare", "overlay";
    A::CompareAutoFlicker => "compare-auto-flicker", "palette-compare-auto-flicker", View, Viewer, "compare", "overlay";
    A::CompareAlign => "compare-align", "palette-compare-align", View, Viewer, "compare", "sizes";
    A::CompareMetricSpace => "compare-metric-space", "palette-compare-metric-space", View, Viewer, "compare", "metrics";
    A::CompareWholeImage => "compare-whole-image", "palette-compare-whole-image", View, Viewer, "compare", "metrics";
    A::CompareProbe => "compare-probe", "palette-compare-probe", View, Viewer, "compare", "probe";
    A::CompareLink => "compare-link", "palette-compare-link", View, Viewer, "compare", "";
    A::CompareClose => "compare-close", "palette-compare-close", View, Viewer, "compare", "";
    A::CopyViewCommand => "copy-view-command", "palette-copy-view-command", File, Viewer, "scripting", "copy-as-command";
    A::CopyViewPython => "copy-view-python", "palette-copy-view-python", File, Viewer, "scripting", "copy-as-command";
    A::MapLayers => "map-layers", "palette-map-layers", Map, Map, "maps", "layers";
    A::MapAddLayer => "map-add-layer", "palette-map-add-layer", Map, Map, "maps", "layers";
    A::MapZoomToLayer => "map-zoom-to-layer", "palette-map-zoom-to-layer", Map, Map, "maps", "layers";
    A::MapAttributeTable => "map-attribute-table", "palette-map-attribute-table", Map, Map, "maps", "attributes";
    A::MapCoordinateFormat => "map-coordinate-format", "palette-map-coordinate-format", Map, Map, "maps", "coordinates";
    A::MapUnits => "map-units", "palette-map-units", Map, Map, "maps", "coordinates";
    A::MapCopyCoordinate => "map-copy-coordinate", "palette-map-copy-coordinate", Map, Map, "maps", "coordinates";
    A::MapGoToCoordinate => "map-go-to-coordinate", "palette-map-go-to-coordinate", Map, Map, "maps", "coordinates";
    A::MapMeasureDistance => "map-measure-distance", "palette-map-measure-distance", Map, Map, "maps", "measure";
    A::MapMeasureArea => "map-measure-area", "palette-map-measure-area", Map, Map, "maps", "measure";
    A::MapElevationProfile => "map-elevation-profile", "palette-map-elevation-profile", Map, Map, "maps", "profile";
    A::MapOverview => "map-overview", "palette-map-overview", Map, Map, "maps", "layers";
    A::MapSlope => "map-slope", "palette-map-slope", Map, Map, "maps", "terrain";
    A::MapAspect => "map-aspect", "palette-map-aspect", Map, Map, "maps", "terrain";
    A::MapContours => "map-contours", "palette-map-contours", Map, Map, "maps", "terrain";
    A::MapTerrain3d => "map-terrain-3d", "palette-map-terrain-3d", Map, Map, "maps", "terrain";
    A::MapTime => "map-time", "palette-map-time", Map, Map, "maps", "time";
    A::MapPlay => "map-play", "palette-map-play", Map, Map, "maps", "time";
    A::MapWind => "map-wind", "palette-map-wind", Map, Map, "maps", "time";
    A::MapWeatherColours => "map-weather-colours", "palette-map-weather-colours", Map, Map, "maps", "time";
    A::MapServices => "map-services", "palette-map-services", Map, Map, "maps", "online";
    A::MapPhotosOnTrack => "map-photos-on-track", "palette-map-photos-on-track", Map, Map, "maps", "photos";
    A::MapGeotag => "map-geotag", "palette-map-geotag", Map, Map, "maps", "photos";
    A::MapExport => "map-export", "palette-map-export", Map, Map, "maps", "export";
    A::MapPrint => "map-print", "palette-map-print", Map, Map, "maps", "export";
    A::MapCopyExtent => "map-copy-extent", "palette-map-copy-extent", Map, Map, "maps", "export";
    A::ToggleDevelop => "develop", "palette-develop", View, Viewer, "develop", "";
    A::DevelopBeforeAfter => "develop-before-after", "palette-develop-before-after", Develop, Develop, "develop", "before-after";
    A::DevelopSplit => "develop-split", "palette-develop-split", Develop, Develop, "develop", "before-after";
    A::DevelopFocusPanel => "develop-panel", "palette-develop-panel", Develop, Develop, "develop", "panel";
    A::DevelopNextRegion => "develop-next-region", "palette-develop-next-region", Develop, Develop, "develop", "panel";
    A::DevelopPrevRegion => "develop-prev-region", "palette-develop-prev-region", Develop, Develop, "develop", "panel";
    A::DevelopExport => "develop-export", "palette-develop-export", Develop, Develop, "develop", "export";
    A::DevelopExportRecipes => "develop-export-recipes", "palette-develop-export-recipes", Develop, Develop, "export-recipes", "";
    A::DevelopCrop => "develop-crop", "palette-develop-crop", Develop, Develop, "develop", "crop";
    A::DevelopSpotOutlines => "develop-spot-outlines", "palette-develop-spot-outlines", Develop, Develop, "develop", "retouch";
    A::DevelopSpotCrosshair => "develop-spot-crosshair", "palette-develop-spot-crosshair", Develop, Develop, "develop", "retouch";
    A::DevelopScopes => "develop-scopes", "palette-develop-scopes", Develop, Develop, "develop", "scopes";
    A::DevelopClipDisplay => "develop-clip-display", "palette-develop-clip-display", Develop, Develop, "develop", "clipping";
    A::DevelopClipScene => "develop-clip-scene", "palette-develop-clip-scene", Develop, Develop, "develop", "clipping";
    A::DevelopClipGamut => "develop-clip-gamut", "palette-develop-clip-gamut", Develop, Develop, "develop", "clipping";
    A::DevelopSoftProof => "develop-soft-proof", "palette-develop-soft-proof", Develop, Develop, "develop", "proof";
    A::DevelopSpeedExposure => "develop-speed-exposure", "palette-develop-speed-exposure", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedContrast => "develop-speed-contrast", "palette-develop-speed-contrast", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedHighlights => "develop-speed-highlights", "palette-develop-speed-highlights", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedShadows => "develop-speed-shadows", "palette-develop-speed-shadows", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedWhites => "develop-speed-whites", "palette-develop-speed-whites", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedBlacks => "develop-speed-blacks", "palette-develop-speed-blacks", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedTemperature => "develop-speed-temperature", "palette-develop-speed-temperature", Develop, Develop, "develop", "speed-edit";
    A::DevelopSpeedVibrance => "develop-speed-vibrance", "palette-develop-speed-vibrance", Develop, Develop, "develop", "speed-edit";
    A::DevelopCopyEdit => "develop-copy", "palette-develop-copy", Develop, Viewer, "develop", "copy-paste";
    A::DevelopCopySelective => "develop-copy-some", "palette-develop-copy-some", Develop, Viewer, "develop", "copy-paste";
    A::DevelopPasteEdit => "develop-paste", "palette-develop-paste", Develop, Viewer, "develop", "copy-paste";
    A::DevelopPasteSelective => "develop-paste-some", "palette-develop-paste-some", Develop, Viewer, "develop", "copy-paste";
    A::DevelopPastePicks => "develop-paste-picks", "palette-develop-paste-picks", Develop, Viewer, "develop", "copy-paste";
    A::DevelopSetReference => "develop-reference", "palette-develop-reference", Develop, Viewer, "develop", "match";
    A::DevelopMatchReference => "develop-match", "palette-develop-match", Develop, Viewer, "develop", "match";
    A::DevelopSync => "develop-sync", "palette-develop-sync", Develop, Cull, "develop", "copy-paste";
    A::DevelopSyncSelective => "develop-sync-some", "palette-develop-sync-some", Develop, Cull, "develop", "copy-paste";
    A::DevelopNextCapture => "develop-next-capture", "palette-develop-next-capture", Develop, Cull, "develop", "next-capture";
    A::DevelopAutoRules => "develop-auto-rules", "palette-develop-auto-rules", Develop, Viewer, "develop", "styles";
    A::DevelopImportStyle => "develop-import-style", "palette-develop-import-style", Develop, Viewer, "develop", "styles";
    A::DevelopMergeHdr => "develop-merge-hdr", "palette-develop-merge-hdr", Develop, Viewer, "develop", "merge-hdr";
    A::DevelopMergePano => "develop-merge-pano", "palette-develop-merge-pano", Develop, Viewer, "develop", "merge-pano";
    A::DevelopFocusStack => "develop-focus-stack", "palette-develop-focus-stack", Develop, Viewer, "develop", "focus-stack";
    A::DevelopSnapshot => "develop-snapshot", "palette-develop-snapshot", Develop, Develop, "develop", "history";
    A::DevelopCompressHistory => "develop-compress", "palette-develop-compress", Develop, Develop, "develop", "history";
    A::DevelopNewVersion => "develop-new-version", "palette-develop-new-version", Develop, Develop, "develop", "versions";
    A::DevelopNewStyle => "develop-new-style", "palette-develop-new-style", Develop, Develop, "develop", "styles";
    A::DevelopBrushSmaller => "develop-brush-smaller", "palette-develop-brush-smaller", Develop, Develop, "develop", "masks";
    A::DevelopBrushLarger => "develop-brush-larger", "palette-develop-brush-larger", Develop, Develop, "develop", "masks";
    A::DevelopBrushSofter => "develop-brush-softer", "palette-develop-brush-softer", Develop, Develop, "develop", "masks";
    A::DevelopBrushHarder => "develop-brush-harder", "palette-develop-brush-harder", Develop, Develop, "develop", "masks";
    A::DevelopBrushErase => "develop-brush-erase", "palette-develop-brush-erase", Develop, Develop, "develop", "masks";
    A::DevelopMaskOverlay => "develop-mask-overlay", "palette-develop-mask-overlay", Develop, Develop, "develop", "masks";
}

impl Action {
    /// Fluent message ID naming the action. Private: the UI names an action with
    /// [`Action::name`] (`names.rs`), which passes the arguments some names take.
    pub(crate) fn message_id(self) -> &'static str {
        match self {
            A::ZoomTo(p) => match p as u32 {
                200 => "palette-zoom-200",
                300 => "palette-zoom-300",
                400 => "palette-zoom-400",
                _ => "palette-zoom-100",
            },
            // Takes `$stars` (the naming code passes it).
            A::CullRate(0) => "gallery-rate-none",
            A::CullLabel(n) => match n {
                1 => "gallery-label-red",
                2 => "gallery-label-yellow",
                3 => "gallery-label-green",
                4 => "gallery-label-blue",
                _ => "gallery-label-none",
            },
            A::WindowPreset(n) => match n {
                0 => "palette-window-default",
                1 => "palette-window-lung",
                2 => "palette-window-mediastinum",
                3 => "palette-window-bone",
                4 => "palette-window-brain",
                5 => "palette-window-stroke",
                6 => "palette-window-soft-tissue",
                7 => "palette-window-liver",
                8 => "palette-window-subdural",
                _ => "palette-window-full-range",
            },
            A::ScienceChannel(n @ 1..=16) => SCIENCE_CHANNELS[usize::from(n) - 1],
            A::ScienceChannel(_) => "palette-science-channel-16",
            a => a.row_message_id(),
        }
    }
}

const SCIENCE_CHANNELS: [&str; 16] = [
    "palette-science-channel-1",
    "palette-science-channel-2",
    "palette-science-channel-3",
    "palette-science-channel-4",
    "palette-science-channel-5",
    "palette-science-channel-6",
    "palette-science-channel-7",
    "palette-science-channel-8",
    "palette-science-channel-9",
    "palette-science-channel-10",
    "palette-science-channel-11",
    "palette-science-channel-12",
    "palette-science-channel-13",
    "palette-science-channel-14",
    "palette-science-channel-15",
    "palette-science-channel-16",
];
