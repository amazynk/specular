//! MPR for any volume: which sources open it in which profile, the science z-stacks'
//! axis, voxel size and notes, a stack loaded through a session's planes within the memory
//! budget, the neutral plane names, the maximised layout and the planes' export.

use std::sync::Arc;
use std::time::Duration;

use specular_core::CancelToken;
use specular_decode::science::{
    AxisKind, Linear, NdAxis, NdPlane, PixelCalibration, ScienceSummary,
};
use specular_input::{
    Action, ActionState, Capabilities, Capability, Disabled, ImageState, Profile, ScienceState,
    Visibility,
};

use super::draw::{empty, layout_maximised};
use super::load::{Length, Loading, Source, lock};
use super::science_vol::{self, Stack, z_axis};
use super::tests::wait;
use super::{VolumeFacts, VolumeKind, view_name, volume_kind};

fn axis(name: &str, len: u32, kind: AxisKind) -> NdAxis {
    NdAxis {
        name: name.into(),
        len,
        unit: String::new(),
        labels: Vec::new(),
        wcs_axis: None,
        linear: None,
        kind,
    }
}

/// A light-microscopy hyperstack: 3 channels, 20 z planes 0.5 µm apart, 12 time points,
/// 0.2 µm pixels.
fn hyperstack() -> ScienceSummary {
    let mut c = axis("Channel", 3, AxisKind::Channel);
    c.labels = vec!["DAPI".into(), "GFP".into(), "mCherry".into()];
    let mut z = axis("Z", 20, AxisKind::Cube);
    z.unit = "\u{b5}m".into();
    z.linear = Some(Linear {
        offset: 0.0,
        step: 0.5,
    });
    ScienceSummary {
        width: 64,
        height: 48,
        axes: vec![c, z, axis("T", 12, AxisKind::Cube)],
        channel_axis: Some(0),
        pixel: Some(PixelCalibration {
            x: Linear {
                offset: 0.0,
                step: 0.2,
            },
            y: Linear {
                offset: 0.0,
                step: 0.2,
            },
            unit_x: "\u{b5}m".into(),
            unit_y: "\u{b5}m".into(),
        }),
        ..ScienceSummary::default()
    }
}

/// Each source the action's state reports as a volume, and what is not one.
#[test]
fn volume_sources() {
    let none = VolumeFacts::default();
    assert_eq!(volume_kind(&none), None);
    let research = |d| VolumeFacts {
        research: Some(d),
        ..VolumeFacts::default()
    };
    // NIfTI, NRRD, MetaImage, industrial CT, a density map: a research volume.
    assert_eq!(
        volume_kind(&research([256, 256, 120])),
        Some(VolumeKind::Research)
    );
    // A single 2D NIfTI slice is not.
    assert_eq!(volume_kind(&research([256, 256, 1])), None);
    // Its medical frames do not make it one either.
    let flat = VolumeFacts {
        medical_frames: Some(1),
        ..research([256, 256, 1])
    };
    assert_eq!(volume_kind(&flat), None);
    let seismic = VolumeFacts {
        seismic: true,
        ..VolumeFacts::default()
    };
    assert_eq!(volume_kind(&seismic), Some(VolumeKind::Seismic));
    let science = VolumeFacts {
        science: true,
        ..VolumeFacts::default()
    };
    assert_eq!(volume_kind(&science), Some(VolumeKind::Science));
    let series = VolumeFacts {
        medical_frames: Some(1),
        in_series: true,
        ..VolumeFacts::default()
    };
    assert_eq!(volume_kind(&series), Some(VolumeKind::Series));
    let frames = |n| VolumeFacts {
        medical_frames: Some(n),
        ..VolumeFacts::default()
    };
    assert_eq!(volume_kind(&frames(40)), Some(VolumeKind::MultiFrame));
    assert_eq!(volume_kind(&frames(2)), None);
}

/// Every volume source opens MPR in every profile, with the same action; a photo does not.
#[test]
fn availability_per_source_and_profile() {
    let sources: [(&str, VolumeFacts, Option<u32>, bool); 6] = [
        (
            "research",
            VolumeFacts {
                research: Some([64, 64, 40]),
                medical_frames: Some(40),
                ..VolumeFacts::default()
            },
            Some(40),
            false,
        ),
        (
            "industrial ct",
            VolumeFacts {
                research: Some([300, 300, 280]),
                medical_frames: Some(280),
                ..VolumeFacts::default()
            },
            Some(280),
            false,
        ),
        (
            "series",
            VolumeFacts {
                medical_frames: Some(1),
                in_series: true,
                ..VolumeFacts::default()
            },
            Some(1),
            false,
        ),
        (
            "microscopy",
            VolumeFacts {
                science: true,
                ..VolumeFacts::default()
            },
            None,
            true,
        ),
        (
            "seismic",
            VolumeFacts {
                seismic: true,
                ..VolumeFacts::default()
            },
            None,
            true,
        ),
        ("photo", VolumeFacts::default(), None, false),
    ];
    let profiles = [
        Capabilities::GENERAL,
        Capabilities::GENERAL.with(Capability::Science),
        Capabilities::GENERAL.with(Capability::Engineering),
        Capabilities::GENERAL.with(Capability::Maps),
        Capabilities::GENERAL.with(Capability::Medical),
        Capabilities::ALL,
    ];
    for (name, facts, medical, science) in sources {
        for enabled in profiles {
            let st = ActionState {
                image: ImageState::Ready,
                count: 1,
                medical,
                science: science.then(|| ScienceState {
                    planes: 20,
                    axes: 1,
                    ..ScienceState::default()
                }),
                volume: volume_kind(&facts).is_some(),
                profile: Profile::of(enabled),
                ..ActionState::default()
            };
            let want = if name == "photo" {
                Visibility::Disabled(Disabled::NoVolume)
            } else {
                Visibility::Visible
            };
            assert_eq!(Action::Mpr.visibility(&st), want, "{name} {enabled:?}");
        }
    }
}

#[test]
fn z_axes_of_science_images() {
    let s = hyperstack();
    assert_eq!(z_axis(&s), Some(1));
    // A FITS spectral cube: its first data cube axis.
    let mut cube = ScienceSummary {
        width: 100,
        height: 100,
        axes: vec![axis("FREQ", 30, AxisKind::Cube)],
        ..ScienceSummary::default()
    };
    assert_eq!(z_axis(&cube), Some(0));
    // A time series, a filter set, an extension list, too few planes: not volumes.
    cube.axes = vec![axis("T", 30, AxisKind::Cube)];
    assert_eq!(z_axis(&cube), None);
    cube.axes = vec![axis("", 4, AxisKind::Channel)];
    assert_eq!(z_axis(&cube), None);
    cube.axes = vec![axis("", 5, AxisKind::Extension)];
    assert_eq!(z_axis(&cube), None);
    cube.axes = vec![axis("Z", 2, AxisKind::Cube)];
    assert_eq!(z_axis(&cube), None);
    // A slide shown from its raster has no planes to read.
    let mut slide = hyperstack();
    slide.raster_only = true;
    assert_eq!(z_axis(&slide), None);
}

#[test]
fn voxel_sizes_from_the_file_or_cubes_with_a_note() {
    let s = hyperstack();
    let (sp, l, assumed) = science_vol::spacing(&s, 1);
    assert_eq!(l, Length::Micron);
    assert!(!assumed);
    for (a, b) in sp.iter().zip([0.2, 0.2, 0.5]) {
        assert!((a - b).abs() < 1e-9, "{sp:?}");
    }
    // No z step: the slices one pixel apart, said in the panel.
    let mut no_z = hyperstack();
    no_z.axes[1].linear = None;
    let (sp, l, assumed) = science_vol::spacing(&no_z, 1);
    assert!(assumed && l == Length::Micron && (sp[2] - 0.2).abs() < 1e-9);
    let st = science_vol::stack(&no_z, &[0, 0, 0], None, false).unwrap();
    assert!(st.notes.contains(&specular_i18n::t!("mpr-isotropic-z")));
    // Nothing at all: voxel units.
    let mut bare = hyperstack();
    bare.pixel = None;
    bare.axes[1].linear = None;
    let (sp, l, assumed) = science_vol::spacing(&bare, 1);
    assert_eq!((sp, l, assumed), ([1.0; 3], Length::Voxel, true));
    let st = science_vol::stack(&bare, &[0, 0, 0], None, false).unwrap();
    assert!(st.notes.contains(&specular_i18n::t!("mpr-isotropic")));
    // A CT-like stack in millimetres; an EM tomogram in nanometres.
    let mut mm = hyperstack();
    mm.pixel.as_mut().unwrap().unit_x = "mm".into();
    mm.pixel.as_mut().unwrap().unit_y = "mm".into();
    mm.axes[1].unit = "mm".into();
    assert_eq!(science_vol::spacing(&mm, 1).1, Length::Mm);
    let mut nm = hyperstack();
    nm.pixel.as_mut().unwrap().unit_x = "nm".into();
    nm.pixel.as_mut().unwrap().unit_y = "nm".into();
    nm.axes[1].unit = "nm".into();
    nm.pixel.as_mut().unwrap().x.step = 2.0;
    nm.pixel.as_mut().unwrap().y.step = 2.0;
    assert_eq!(science_vol::spacing(&nm, 1).1, Length::Nanometre);
    assert_eq!(science_vol::metres("deg"), None);
}

/// The channel and time point shown, and the composite's single channel, in the notes.
#[test]
fn stack_notes_name_the_channel_and_time_point() {
    let s = hyperstack();
    let st = science_vol::stack(&s, &[1, 7, 4], Some((10.0, 900.0)), true).unwrap();
    assert_eq!((st.z, st.depth), (1, 20));
    assert_eq!(st.index_at(13), vec![1, 13, 4]);
    assert_eq!(st.limits, Some((10.0, 900.0)));
    let strip = |s: &str| s.replace(['\u{2068}', '\u{2069}'], "");
    let notes: Vec<String> = st.notes.iter().map(|n| strip(n)).collect();
    assert_eq!(
        notes,
        [
            "Channel: GFP",
            "T: 5 of 12",
            "MPR shows one channel at a time"
        ]
    );
    // Out-of-range positions are clamped.
    let st = science_vol::stack(&s, &[9, 0, 99], None, false).unwrap();
    assert_eq!(st.index, vec![2, 0, 11]);
}

/// A session whose plane at (c, z, t) holds `x + 10 y + 100 z + 1000 c` (NaN at the origin
/// of z = 3), as a z-stack's reader gives its planes.
struct Planes(ScienceSummary);

impl specular_decode::Session for Planes {
    fn info(&self) -> &specular_decode::ImageInfo {
        unreachable!()
    }
    fn source(&self) -> &Arc<specular_core::Source> {
        unreachable!()
    }
    fn decode_preview(&self) -> Option<specular_decode::Result<specular_core::Raster>> {
        None
    }
    fn decode(
        &self,
        _: &specular_decode::DecodeRequest,
        _: &CancelToken,
    ) -> specular_decode::Result<specular_core::Raster> {
        unreachable!()
    }
    fn science(&self) -> Option<&ScienceSummary> {
        Some(&self.0)
    }
    fn nd_plane(
        &self,
        index: &[u32],
        _: &CancelToken,
    ) -> Option<specular_decode::Result<Arc<NdPlane>>> {
        let (w, h) = (self.0.width, self.0.height);
        let (c, z) = (index[0] as f32, index[1] as f32);
        let values = (0..h)
            .flat_map(|y| (0..w).map(move |x| (x, y)))
            .map(|(x, y)| {
                if index[1] == 3 && x == 0 && y == 0 {
                    f32::NAN
                } else {
                    x as f32 + 10.0 * y as f32 + 100.0 * z + 1000.0 * c
                }
            })
            .collect();
        Some(Ok(Arc::new(NdPlane {
            width: w,
            height: h,
            values,
            truncated: false,
        })))
    }
}

/// A z-stack loads plane by plane at its channel, in micrometres, unoriented, its voxels'
/// bytes held in the memory budget until the volume goes.
#[test]
fn a_science_stack_loads_through_its_planes() {
    let s = hyperstack();
    let stack: Stack = science_vol::stack(&s, &[2, 0, 0], None, false).unwrap();
    let session: Arc<dyn specular_decode::Session> = Arc::new(Planes(s));
    let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test"));
    let mut l = Loading::start(
        Source::Science(session, Arc::new(stack)),
        pool,
        Arc::new(|| {}),
        2048,
        1 << 30,
    );
    assert!(wait(&mut l, Duration::from_secs(60)));
    assert_eq!(l.failed, 0);
    let info = l.info.clone().unwrap();
    assert_eq!(info.full, [64, 48, 20]);
    assert_eq!(info.length, Length::Micron);
    assert!(!info.oriented);
    assert_eq!(l.reserved(), info.plan.bytes);
    let v = lock(&l.volume);
    assert!(v.complete());
    let mut mismatches = 0;
    for z in 0..20u32 {
        for y in (0..48u32).step_by(7) {
            for x in (0..64u32).step_by(5) {
                let p = [f64::from(x) * 0.2, f64::from(y) * 0.2, f64::from(z) * 0.5];
                let want = if z == 3 && x == 0 && y == 0 {
                    0.0
                } else {
                    (x + 10 * y + 100 * z + 2000) as f32
                };
                let got = v.sample_patient(p).unwrap();
                if (got - want).abs() > 1e-3 {
                    mismatches += 1;
                }
            }
        }
    }
    assert_eq!(mismatches, 0);
}

/// Patient orientation names the views only where the file states it: a NIfTI with an
/// sform, not a NIfTI without one, nor an industrial CT (Nikon, Volume Graphics, Zeiss).
#[test]
fn research_volumes_say_whether_they_are_oriented() {
    let data = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/formats/specular-medvol/tests/data");
    for (name, oriented) in [
        ("sagittal.nii", true),
        ("no_form.nii", false),
        ("xtek_u16.xtekVolume", false),
        ("vgi_u16.vgi", false),
        ("xtek_f32.xtekct", false),
    ] {
        let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test-r"));
        let mut l = Loading::start(
            Source::Research(data.join(name), 0, None),
            pool,
            Arc::new(|| {}),
            2048,
            1 << 30,
        );
        assert!(wait(&mut l, Duration::from_secs(60)), "{name}");
        let info = l.info.as_ref().unwrap();
        assert_eq!(info.oriented, oriented, "{name}");
        assert_eq!(info.length, Length::Mm, "{name}");
        assert!(l.reserved() > 0, "{name}");
    }
}

#[test]
fn plane_names_follow_orientation() {
    let strip = |s: String| s.replace(['\u{2068}', '\u{2069}'], "");
    let names: Vec<String> = (0..3).map(|v| strip(view_name(v, true, false))).collect();
    assert_eq!(names, ["Axial", "Coronal", "Sagittal"]);
    let names: Vec<String> = (0..3).map(|v| strip(view_name(v, false, false))).collect();
    assert_eq!(names, ["XY", "XZ", "YZ"]);
    assert_eq!(strip(view_name(1, false, true)), "Oblique");
}

#[test]
fn a_maximised_view_keeps_the_controls_beside_it() {
    let l = layout_maximised([0.0, 0.0, 1200.0, 800.0], 1.0, false, 1);
    assert!(empty(l.views[0]) && empty(l.views[2]));
    let v = l.views[1];
    assert_eq!(v[0], 0.0);
    assert!(v[2] > 800.0 && v[3] == 800.0, "{v:?}");
    assert!(
        l.slot[0] > v[0] + v[2] && l.slot[2] >= 240.0,
        "{:?}",
        l.slot
    );
    // Mirrored in right-to-left layouts.
    let r = layout_maximised([0.0, 0.0, 1200.0, 800.0], 1.0, true, 1);
    assert_eq!(r.slot[0], 0.0);
    assert!(r.views[1][0] > r.slot[2]);
    // At 1.5× on whole physical pixels.
    let s = layout_maximised([0.0, 0.0, 1000.0, 700.0], 1.5, false, 0);
    for x in [s.views[0][0], s.views[0][2], s.slot[0], s.slot[2]] {
        assert!((x * 1.5 - (x * 1.5).round()).abs() < 1e-9, "{x}");
    }
}

/// The planes export: each plane over the whole volume at its finest voxel edge, the
/// window applied, as PNG images named for the planes.
#[test]
fn planes_export_as_images() {
    use specular_dicom::volume::phantom;
    use specular_render::{VoiShape, Window};
    let vol = phantom::volume([48, 40, 30], 2.0, 3.0, 0.0);
    let views = super::views::Views::new(&vol.geometry);
    let w = Window {
        center: 40.0,
        width: 400.0,
        shape: VoiShape::Linear,
        invert: false,
    };
    let (pw, ph, px) = super::planes::plane_image(&vol, &views, 0, w);
    // 48 × 40 pixels of 2 mm: the axial plane at 2 mm per pixel.
    assert_eq!((pw, ph), (48, 40));
    assert_eq!(px.len(), 48 * 40);
    // The coronal plane: 48 across, 30 slices of 3 mm resampled to 45 rows.
    let (cw, ch, _) = super::planes::plane_image(&vol, &views, 1, w);
    assert_eq!((cw, ch), (48, 45));
    // The centre pixel is the window of the value there.
    let c = views.c;
    let want = vol
        .sample_patient(c)
        .map(|x| (w.grey(f64::from(x)) * 255.0).round() as u8)
        .unwrap();
    let got = px[20 * 48 + 24];
    assert!(got.abs_diff(want) <= 40, "{got} vs {want}");
    let dir = std::env::temp_dir().join(format!("specular-mpr-planes-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (n, _) = super::planes::export(&vol, &views, w, false, &dir, "stack").unwrap();
    assert_eq!(n, 3);
    for name in ["stack-xy.png", "stack-xz.png", "stack-yz.png"] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        let d = png::Decoder::new(std::io::Cursor::new(bytes));
        let r = d.read_info().unwrap();
        assert_eq!(r.info().color_type, png::ColorType::Grayscale, "{name}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
