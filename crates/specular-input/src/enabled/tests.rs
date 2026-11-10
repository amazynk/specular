use super::*;
use crate::Mode;

#[test]
fn page_actions() {
    let st = |pages, medical| ActionState {
        image: ImageState::Ready,
        count: 1,
        pages,
        medical,
        ..ActionState::default()
    };
    for a in [
        Action::NextPage,
        Action::PrevPage,
        Action::FirstPage,
        Action::LastPage,
    ] {
        assert_eq!(a.disabled(&st(0, None)), Some(Disabled::OnePage));
        assert_eq!(a.disabled(&st(1, None)), Some(Disabled::OnePage));
        assert_eq!(a.disabled(&st(4, None)), None);
        // Medical mode steps the frames of a multi-frame file with the same keys.
        assert_eq!(a.disabled(&st(1, Some(30))), None);
        assert_eq!(a.disabled(&st(1, Some(1))), Some(Disabled::OnePage));
        assert_eq!(a.disabled(&ActionState::default()), Some(Disabled::NoImage));
        assert_eq!(a.mode(), crate::Mode::Viewer);
        assert_eq!(a.group(), crate::Group::Navigation);
    }
}

/// The DESIGN.md table, action by action: `E` enabled, `-` disabled, for the states
/// none, opening, decoding, ready and failed (in a folder of several images).
#[test]
fn enabled_table() {
    use Action as A;
    let table: &[(Action, &str)] = &[
        (A::Open, "EEEEE"),
        (A::Settings, "EEEEE"),
        (A::CustomiseToolbar, "EEEEE"),
        (A::Palette, "EEEEE"),
        (A::CheatSheet, "EEEEE"),
        (A::About, "EEEEE"),
        (A::ShowWelcome, "EEEEE"),
        (A::Help, "EEEEE"),
        (A::JoinDiscord, "EEEEE"),
        (A::SaveDiagnostics, "EEEEE"),
        (A::Language, "EEEEE"),
        (A::ToggleTheme, "EEEEE"),
        (A::ToggleFullscreen, "EEEEE"),
        (A::Back, "EEEEE"),
        (A::Close, "EEEEE"),
        (A::Quit, "EEEEE"),
        (A::ToggleInfo, "--EEE"),
        (A::CopyPath, "--EEE"),
        (A::OpenWith, "--EEE"),
        (A::OpenAs, "--EEE"),
        (A::Paste, "EEEEE"),
        (A::MoveToTrash, "--EEE"),
        (A::Rename, "--EEE"),
        (A::CopyToFolder, "--EEE"),
        (A::MoveToFolder, "--EEE"),
        (A::Redo, "EEEEE"),
        (A::ExportSettings, "EEEEE"),
        (A::ImportSettings, "EEEEE"),
        (A::RotateFileLeft, "--EEE"),
        (A::RotateFileRight, "--EEE"),
        (A::SetWallpaper, "--EEE"),
        (A::Share, "--EEE"),
        (A::SendByEmail, "--EEE"),
        (A::ShareWithZynk, "--EEE"),
        (A::SendWithZynk, "--EEE"),
        (A::BatchRename, "EEEEE"),
        (A::CopyImage, "---E-"),
        (A::ZoomIn, "--EE-"),
        (A::ZoomOut, "--EE-"),
        (A::ZoomFit, "--EE-"),
        (A::ZoomTo(100.0), "--EE-"),
        (A::RotateLeft, "--EE-"),
        (A::RotateRight, "--EE-"),
        (A::PanLeft, "--EE-"),
        (A::Loupe, "--EE-"),
        (A::ToggleNavigator, "--EE-"),
        (A::ZoomRectangle, "--EE-"),
        (A::TogglePixelGrid, "--EE-"),
        (A::ToggleClipping, "--EE-"),
        (A::ToggleFocusPeaking, "--EE-"),
        (A::ToggleHistogram, "--EE-"),
        (A::TogglePixelInspector, "--EE-"),
        (A::ToggleCrosshair, "--EE-"),
        (A::AnnounceCursor, "--EE-"),
        (A::ChannelRed, "--EE-"),
        (A::ChannelGreen, "--EE-"),
        (A::ChannelBlue, "--EE-"),
        (A::ChannelAlpha, "--EE-"),
        (A::ChannelLuma, "--EE-"),
        (A::ToggleFalseColour, "--EE-"),
        (A::NextColourMap, "--EE-"),
        (A::NextChannelColours, "--EE-"),
        (A::ToggleHiLo, "--EE-"),
        (A::NextColourVision, "EEEEE"),
        (A::SimulateProtanopia, "EEEEE"),
        (A::SimulateDeuteranopia, "EEEEE"),
        (A::SimulateTritanopia, "EEEEE"),
        (A::SimulateAchromatopsia, "EEEEE"),
        (A::NextImage, "EEEEE"),
        (A::PrevImage, "EEEEE"),
        (A::Slideshow, "-EEEE"),
        (A::ExportSlideshow, "-EEEE"),
        (A::ToggleCull, "EEEEE"),
        (A::Print, "EEEEE"),
        (A::Convert, "EEEEE"),
        // No card in this table's state.
        (A::Import, "-----"),
    ];
    let states = [
        ImageState::None,
        ImageState::Opening,
        ImageState::Decoding,
        ImageState::Ready,
        ImageState::Failed,
    ];
    for (a, row) in table {
        for (s, want) in states.iter().zip(row.chars()) {
            let st = ActionState {
                comparing: false,
                compare_images: 0,
                volume: false,
                mpr: false,
                image: *s,
                count: 5,
                gallery_files: 0,
                animated: false,
                video: false,
                sound: false,
                sound_file: false,
                in_archive: false,
                has_alpha: true,
                gallery_views: None,
                medical: None,
                dicom: false,
                series: 0,
                presentation_states: 0,
                science: None,
                debayer: false,
                annotations: false,
                stain: None,
                stain_region: false,
                slide_annotations: false,
                thermal_visible: false,
                drawing: false,
                mesh: false,
                points: false,
                molecule: false,
                tracts: false,
                relight: false,
                import_sources: false,
                pages: 0,
                rotatable: true,
                wallpaper: true,
                share: true,
                email: true,
                map: None,
                profile: Profile::UNRESTRICTED,
            };
            assert_eq!(a.enabled(&st), want == 'E', "{a:?} in {s:?}");
        }
    }
    // Every listed action has a rule that covers the empty window.
    let empty = ActionState::default();
    for a in Action::LISTED {
        let _ = a.disabled(&empty);
    }
}

/// The gallery's quick look, compare and similar-shots review: the channel views follow
/// the images they show, whatever the viewer holds (nothing, or an image still opening).
#[test]
fn gallery_views_enable_the_channels() {
    let channels = [
        Action::ChannelRed,
        Action::ChannelGreen,
        Action::ChannelBlue,
        Action::ChannelLuma,
    ];
    for image in [ImageState::None, ImageState::Opening, ImageState::Failed] {
        let st = ActionState {
            image,
            gallery_views: Some(true),
            dicom: false,
            series: 0,
            presentation_states: 0,
            science: None,
            debayer: false,
            annotations: false,
            stain: None,
            stain_region: false,
            slide_annotations: false,
            thermal_visible: false,
            drawing: false,
            mesh: false,
            points: false,
            molecule: false,
            tracts: false,
            relight: false,
            import_sources: false,
            ..ActionState::default()
        };
        for a in channels.iter().chain([&Action::ChannelAlpha]) {
            assert_eq!(a.disabled(&st), None, "{a:?} {image:?}");
        }
        // None of the images shown has alpha: the alpha view only is off.
        let opaque = ActionState {
            gallery_views: Some(false),
            dicom: false,
            series: 0,
            presentation_states: 0,
            science: None,
            debayer: false,
            annotations: false,
            stain: None,
            stain_region: false,
            slide_annotations: false,
            thermal_visible: false,
            drawing: false,
            mesh: false,
            points: false,
            molecule: false,
            tracts: false,
            relight: false,
            import_sources: false,
            ..st
        };
        for a in channels {
            assert_eq!(a.disabled(&opaque), None, "{a:?}");
        }
        assert_eq!(
            Action::ChannelAlpha.disabled(&opaque),
            Some(Disabled::NoAlpha)
        );
        // Other actions still follow the viewer's image.
        assert_eq!(
            Action::ToggleHistogram.disabled(&st),
            Action::ToggleHistogram.disabled(&ActionState {
                image,
                ..ActionState::default()
            })
        );
    }
}

#[test]
fn navigation_needs_other_images() {
    let st = |count| ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Ready,
        count,
        gallery_files: 0,
        animated: false,
        video: false,
        sound: false,
        sound_file: false,
        in_archive: false,
        has_alpha: false,
        gallery_views: None,
        medical: None,
        dicom: false,
        series: 0,
        presentation_states: 0,
        science: None,
        debayer: false,
        annotations: false,
        stain: None,
        stain_region: false,
        slide_annotations: false,
        thermal_visible: false,
        drawing: false,
        mesh: false,
        points: false,
        molecule: false,
        tracts: false,
        relight: false,
        import_sources: false,
        pages: 0,
        rotatable: false,
        wallpaper: false,
        share: false,
        email: false,
        map: None,
        profile: Profile::UNRESTRICTED,
    };
    assert_eq!(Action::NextImage.disabled(&st(0)), Some(Disabled::NoImage));
    assert_eq!(
        Action::NextImage.disabled(&st(1)),
        Some(Disabled::NoOtherImages)
    );
    assert_eq!(Action::PrevImage.disabled(&st(2)), None);
}

#[test]
fn playback_needs_an_animation() {
    let st = |animated| ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Ready,
        count: 3,
        gallery_files: 0,
        animated,
        video: false,
        sound: false,
        sound_file: false,
        in_archive: false,
        has_alpha: false,
        gallery_views: None,
        medical: None,
        dicom: false,
        series: 0,
        presentation_states: 0,
        science: None,
        debayer: false,
        annotations: false,
        stain: None,
        stain_region: false,
        slide_annotations: false,
        thermal_visible: false,
        drawing: false,
        mesh: false,
        points: false,
        molecule: false,
        tracts: false,
        relight: false,
        import_sources: false,
        pages: 0,
        rotatable: false,
        wallpaper: false,
        share: false,
        email: false,
        map: None,
        profile: Profile::UNRESTRICTED,
    };
    assert_eq!(
        Action::PlayPause.disabled(&st(false)),
        Some(Disabled::NotAnimated)
    );
    assert_eq!(Action::NextFrame.disabled(&st(true)), None);
    assert_eq!(Action::ToggleSpreads.disabled(&st(false)), None);
    let page = ActionState {
        in_archive: true,
        ..st(false)
    };
    assert_eq!(
        Action::MoveToTrash.disabled(&page),
        Some(Disabled::InArchive)
    );
    assert_eq!(Action::CopyPath.disabled(&page), None);
    for a in [
        Action::PlayPause,
        Action::ToggleSpreads,
        Action::FasterPlayback,
    ] {
        assert_eq!(Action::from_id(&a.id()), Some(a));
    }
}

#[test]
fn file_actions_say_why() {
    let st = ActionState {
        image: ImageState::Ready,
        count: 3,
        ..ActionState::default()
    };
    assert_eq!(
        Action::RotateFileLeft.disabled(&st),
        Some(Disabled::NotRotatable)
    );
    for a in [Action::SetWallpaper, Action::Share, Action::SendByEmail] {
        assert_eq!(a.disabled(&st), Some(Disabled::NotOnThisDesktop), "{a:?}");
    }
    // Without Zynk they explain it, so they are never greyed for its want.
    for a in [Action::ShareWithZynk, Action::SendWithZynk] {
        assert_eq!(a.disabled(&st), None, "{a:?}");
    }
    // In the gallery they hand over its selection, with no image open.
    let gallery = ActionState {
        gallery_files: 12,
        ..ActionState::default()
    };
    for a in [Action::ShareWithZynk, Action::SendWithZynk] {
        assert_eq!(
            a.disabled(&ActionState::default()),
            Some(Disabled::NoImage),
            "{a:?}"
        );
        assert_eq!(a.disabled(&gallery), None, "{a:?}");
    }
    let able = ActionState {
        rotatable: true,
        wallpaper: true,
        share: true,
        email: true,
        ..st
    };
    assert_eq!(Action::RotateFileRight.disabled(&able), None);
    assert_eq!(Action::SetWallpaper.disabled(&able), None);
    let page = ActionState {
        in_archive: true,
        ..able
    };
    for a in [
        Action::Rename,
        Action::CopyToFolder,
        Action::MoveToFolder,
        Action::RotateFileLeft,
        Action::Share,
    ] {
        assert_eq!(a.disabled(&page), Some(Disabled::InArchiveFile), "{a:?}");
    }
    assert_eq!(
        Action::BatchRename.disabled(&ActionState::default()),
        Some(Disabled::NoImage)
    );
    assert_eq!(Action::Redo.disabled(&ActionState::default()), None);
}

#[test]
fn copy_image_reasons() {
    let st = |image| ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image,
        count: 1,
        gallery_files: 0,
        animated: false,
        video: false,
        sound: false,
        sound_file: false,
        in_archive: false,
        has_alpha: false,
        gallery_views: None,
        medical: None,
        dicom: false,
        series: 0,
        presentation_states: 0,
        science: None,
        debayer: false,
        annotations: false,
        stain: None,
        stain_region: false,
        slide_annotations: false,
        thermal_visible: false,
        drawing: false,
        mesh: false,
        points: false,
        molecule: false,
        tracts: false,
        relight: false,
        import_sources: false,
        pages: 0,
        rotatable: false,
        wallpaper: false,
        share: false,
        email: false,
        map: None,
        profile: Profile::UNRESTRICTED,
    };
    assert_eq!(
        Action::CopyImage.disabled(&st(ImageState::None)),
        Some(Disabled::NoImage)
    );
    assert_eq!(
        Action::CopyImage.disabled(&st(ImageState::Decoding)),
        Some(Disabled::StillLoading)
    );
    assert_eq!(
        Action::CopyImage.disabled(&st(ImageState::Failed)),
        Some(Disabled::DecodeFailed)
    );
}

#[test]
fn alpha_view_needs_an_alpha_channel() {
    let st = |has_alpha| ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Ready,
        count: 1,
        gallery_files: 0,
        animated: false,
        video: false,
        sound: false,
        sound_file: false,
        in_archive: false,
        has_alpha,
        gallery_views: None,
        medical: None,
        dicom: false,
        series: 0,
        presentation_states: 0,
        science: None,
        debayer: false,
        annotations: false,
        stain: None,
        stain_region: false,
        slide_annotations: false,
        thermal_visible: false,
        drawing: false,
        mesh: false,
        points: false,
        molecule: false,
        tracts: false,
        relight: false,
        import_sources: false,
        pages: 0,
        rotatable: false,
        wallpaper: false,
        share: false,
        email: false,
        map: None,
        profile: Profile::UNRESTRICTED,
    };
    assert_eq!(
        Action::ChannelAlpha.disabled(&st(false)),
        Some(Disabled::NoAlpha)
    );
    assert_eq!(Action::ChannelAlpha.disabled(&st(true)), None);
    // The colour channels and luminance work on any image.
    for a in [
        Action::ChannelRed,
        Action::ChannelGreen,
        Action::ChannelBlue,
        Action::ChannelLuma,
    ] {
        assert_eq!(a.disabled(&st(false)), None, "{a:?}");
        assert_eq!(Action::from_id(&a.id()), Some(a));
    }
    // No image: the usual reason, not the alpha one.
    let none = ActionState::default();
    assert_eq!(
        Action::ChannelAlpha.disabled(&none),
        Some(Disabled::NoImage)
    );
}

mod medical;
mod relight;
mod science;

/// Print works from the gallery with no image open (a contact sheet of what it shows).
#[test]
fn print_from_the_gallery_without_an_image() {
    let st = ActionState::default();
    assert_eq!(Action::Print.disabled(&st), Some(Disabled::NoImage));
    let st = ActionState {
        gallery_files: 3,
        ..ActionState::default()
    };
    assert_eq!(Action::Print.disabled(&st), None);
}

/// Play mode (`f`) fills the screen with a movie's picture: off on a sound file's page.
#[test]
fn play_mode_is_off_for_sound_files() {
    let st = |sound_file| ActionState {
        image: ImageState::Ready,
        count: 1,
        video: true,
        sound: true,
        sound_file,
        ..ActionState::default()
    };
    assert_eq!(Action::VideoFullscreen.disabled(&st(false)), None);
    assert_eq!(
        Action::VideoFullscreen.disabled(&st(true)),
        Some(Disabled::NotVideo)
    );
    // The sound's own controls still work there.
    assert_eq!(Action::PlayPauseVideo.disabled(&st(true)), None);
    assert_eq!(Action::ToggleMute.disabled(&st(true)), None);
}
