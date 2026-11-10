use super::*;

#[test]
fn dicom_actions() {
    let st = |image, dicom, series| ActionState {
        image,
        count: 3,
        dicom,
        series,
        ..ActionState::default()
    };
    let tags = Action::DicomTags;
    assert_eq!(
        tags.disabled(&st(ImageState::None, false, 0)),
        Some(Disabled::NoImage)
    );
    assert_eq!(
        tags.disabled(&st(ImageState::Ready, false, 0)),
        Some(Disabled::NotDicom)
    );
    assert_eq!(tags.disabled(&st(ImageState::Failed, true, 0)), None);
    let export = Action::ExportAnonymised;
    assert_eq!(
        export.disabled(&st(ImageState::Ready, false, 0)),
        Some(Disabled::NotDicom)
    );
    assert_eq!(export.disabled(&st(ImageState::Decoding, true, 0)), None);
    assert_eq!(
        Action::ExportPatientDisc.disabled(&st(ImageState::Opening, true, 0)),
        Some(Disabled::StillLoading)
    );
    let mask = Action::MaskBurnedInText;
    assert_eq!(
        mask.disabled(&st(ImageState::Decoding, true, 0)),
        Some(Disabled::StillLoading)
    );
    assert_eq!(
        mask.disabled(&st(ImageState::Ready, false, 0)),
        Some(Disabled::NotDicom)
    );
    assert_eq!(mask.disabled(&st(ImageState::Ready, true, 0)), None);
    assert_eq!(
        Action::NextSeries.disabled(&st(ImageState::Ready, true, 1)),
        Some(Disabled::NoOtherSeries)
    );
    assert_eq!(
        Action::PrevSeries.disabled(&st(ImageState::Ready, true, 2)),
        None
    );
}

#[test]
fn medical_actions_need_a_medical_image() {
    let st = |medical| ActionState {
        image: ImageState::Ready,
        count: 2,
        medical,
        ..ActionState::default()
    };
    for a in [
        Action::WindowLevelTool,
        Action::WindowPreset(3),
        Action::InvertGrey,
        Action::NextSlice,
        Action::Cine,
        Action::MeasureLength,
        Action::MeasureAngle,
        Action::MeasureEllipse,
        Action::MeasureRectangle,
        Action::ClearMeasurements,
        Action::ToggleOverlays,
        Action::ToggleOrientationLabels,
    ] {
        assert_eq!(a.disabled(&st(None)), Some(Disabled::NotMedical), "{a:?}");
        assert_eq!(a.disabled(&st(Some(4))), None, "{a:?}");
    }
    let gsps = Action::NextPresentationState;
    assert_eq!(gsps.disabled(&st(None)), Some(Disabled::NotMedical));
    assert_eq!(
        gsps.disabled(&st(Some(1))),
        Some(Disabled::NoPresentationState)
    );
    let with = ActionState {
        presentation_states: 2,
        science: None,
        debayer: false,
        annotations: false,
        thermal_visible: false,
        drawing: false,
        mesh: false,
        points: false,
        molecule: false,
        tracts: false,
        relight: false,
        import_sources: false,
        ..st(Some(1))
    };
    assert_eq!(gsps.disabled(&with), None);
    // The app says whether the image is a volume (a series, a multi-frame file, a research
    // volume, a z-stack); MPR follows that, medical mode or not.
    assert_eq!(Action::Mpr.disabled(&st(None)), Some(Disabled::NoVolume));
    assert_eq!(Action::Mpr.disabled(&st(Some(1))), Some(Disabled::NoVolume));
    let volume = |medical| ActionState {
        volume: true,
        ..st(medical)
    };
    assert_eq!(Action::Mpr.disabled(&volume(Some(40))), None);
    assert_eq!(Action::Mpr.disabled(&volume(None)), None);
    // MPR's own tools wait for it to open.
    assert_eq!(
        Action::MprSlab.disabled(&volume(None)),
        Some(Disabled::MprClosed)
    );
    assert_eq!(
        Action::MprMaximise.disabled(&st(None)),
        Some(Disabled::NoVolume)
    );
    let open = ActionState {
        mpr: true,
        ..st(None)
    };
    for a in [
        Action::Mpr,
        Action::MprSlab,
        Action::MprMaximise,
        Action::MprExportPlanes,
    ] {
        assert_eq!(a.disabled(&open), None, "{a:?}");
    }
    assert_eq!(Action::InvertGrey.disabled(&st(Some(1))), None);
    assert_eq!(
        Action::PrevSlice.disabled(&st(Some(1))),
        Some(Disabled::OneFrame)
    );
    assert_eq!(
        Action::Cine.disabled(&st(Some(1))),
        Some(Disabled::OneFrame)
    );
    assert_eq!(Action::MeasureLength.disabled(&st(Some(1))), None);
    // In series mode the slice keys and cine step the series' single-frame files.
    let series = ActionState {
        series: 1,
        ..st(Some(1))
    };
    for a in [Action::NextSlice, Action::PrevSlice, Action::Cine] {
        assert_eq!(a.disabled(&series), None, "{a:?}");
    }
}
