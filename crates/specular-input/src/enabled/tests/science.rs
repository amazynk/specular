use super::*;

#[test]
fn science_actions_follow_the_image() {
    let st = |science| ActionState {
        image: ImageState::Ready,
        count: 1,
        science,
        ..Default::default()
    };
    for a in Mode::Science.actions() {
        assert_eq!(a.disabled(&st(None)), Some(Disabled::NotScience), "{a:?}");
    }
    let plain = ScienceState {
        planes: 1,
        ..Default::default()
    };
    assert_eq!(Action::ScienceStretch.disabled(&st(Some(plain))), None);
    assert_eq!(
        Action::ScienceNextPlane.disabled(&st(Some(plain))),
        Some(Disabled::OnePlane)
    );
    assert_eq!(
        Action::ScienceComposite.disabled(&st(Some(plain))),
        Some(Disabled::NoChannels)
    );
    assert_eq!(
        Action::ScienceGrid.disabled(&st(Some(plain))),
        Some(Disabled::NoCoordinates)
    );
    let rgb = ScienceState {
        planes: 3,
        axes: 1,
        channels: 3,
        composite: true,
        celestial: true,
        levelling: false,
        relief: false,
        seismic: false,
        mass_spec: false,
        thermal: false,
        radiometric: false,
        sequence: false,
        ugrid: false,
        samp: false,
    };
    assert_eq!(Action::ScienceChannel(3).disabled(&st(Some(rgb))), None);
    assert_eq!(
        Action::ScienceChannel(4).disabled(&st(Some(rgb))),
        Some(Disabled::NoChannels)
    );
    assert_eq!(
        Action::ScienceNextAxis.disabled(&st(Some(rgb))),
        Some(Disabled::OneAxis)
    );
    assert_eq!(Action::ScienceGrid.disabled(&st(Some(rgb))), None);
    assert_eq!(
        Action::ScienceLevel.disabled(&st(Some(rgb))),
        Some(Disabled::NotHeightMap)
    );
    let height = ScienceState {
        levelling: true,
        ..plain
    };
    assert_eq!(Action::ScienceLevel.disabled(&st(Some(height))), None);
    assert_eq!(
        Action::ScienceHillshade.disabled(&st(Some(height))),
        Some(Disabled::NotElevation)
    );
    let dem = ScienceState {
        relief: true,
        ..plain
    };
    assert_eq!(Action::ScienceHillshade.disabled(&st(Some(dem))), None);
    assert_eq!(
        Action::ScienceWiggle.disabled(&st(Some(height))),
        Some(Disabled::NotSeismic)
    );
    let seismic = ScienceState {
        seismic: true,
        ..plain
    };
    assert_eq!(Action::ScienceWiggle.disabled(&st(Some(seismic))), None);
    assert_eq!(
        Action::ScienceWireframe.disabled(&st(Some(seismic))),
        Some(Disabled::NotUgrid)
    );
    let ugrid = ScienceState {
        ugrid: true,
        ..plain
    };
    assert_eq!(Action::ScienceWireframe.disabled(&st(Some(ugrid))), None);
    assert_eq!(
        Action::ScienceIonImage.disabled(&st(Some(seismic))),
        Some(Disabled::NotMassSpec)
    );
    let ms = ScienceState {
        mass_spec: true,
        ..plain
    };
    assert_eq!(Action::ScienceIonImage.disabled(&st(Some(ms))), None);
    assert_eq!(
        Action::ScienceIsotherm.disabled(&st(Some(ms))),
        Some(Disabled::NotThermal)
    );
    let irg = ScienceState {
        thermal: true,
        ..plain
    };
    assert_eq!(Action::ScienceIsotherm.disabled(&st(Some(irg))), None);
    assert_eq!(
        Action::ScienceTemperatureUnit.disabled(&st(Some(irg))),
        None
    );
    assert_eq!(
        Action::ScienceThermalParameters.disabled(&st(Some(irg))),
        Some(Disabled::NotRadiometric)
    );
    let flir = ScienceState {
        radiometric: true,
        ..irg
    };
    assert_eq!(
        Action::ScienceThermalParameters.disabled(&st(Some(flir))),
        None
    );
    for a in [Action::ScienceThermalPlay, Action::ScienceThermalSeriesCsv] {
        assert_eq!(
            a.disabled(&st(Some(flir))),
            Some(Disabled::NotThermalSequence)
        );
        assert_eq!(a.disabled(&st(Some(ms))), Some(Disabled::NotThermal));
        let seq = ScienceState {
            sequence: true,
            planes: 6,
            axes: 1,
            ..flir
        };
        assert_eq!(a.disabled(&st(Some(seq))), None);
    }
    let photo = ActionState {
        thermal_visible: true,
        ..st(None)
    };
    assert_eq!(Action::ToggleThermalVisible.disabled(&photo), None);
    assert_eq!(
        Action::ToggleThermalVisible.disabled(&st(Some(flir))),
        Some(Disabled::NoVisiblePhoto)
    );
}
