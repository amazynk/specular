//! Slides through the registry: synthetic SVS, NDPI, OME-TIFF (with companion files), an
//! OME-Zarr folder and DICOM WSI series (specular-wsi's fixtures); pages, science facts, and
//! damaged files.

use std::path::PathBuf;

use super::*;
use crate::Registry;
use wsi::fixtures as fx;

fn open_bytes(name: &str, data: Vec<u8>) -> Arc<dyn Session> {
    let src = Arc::new(Source::from_vec(PathBuf::from(name), data));
    Registry::default()
        .open(src)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn req(scale: u32, region: Option<Rect>) -> DecodeRequest {
    DecodeRequest {
        region,
        ..DecodeRequest::full(scale)
    }
}

/// A fresh folder under the test temp dir.
fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-wsi-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn svs_opens_as_a_pyramid_with_pages() {
    let (file, _) = fx::svs();
    // By content, whatever the name.
    for name in ["slide.svs", "slide.tif", "slide"] {
        let s = open_bytes(name, file.clone());
        assert_eq!(s.info().format, Format::Svs, "{name}");
    }
    let s = open_bytes("slide.svs", file);
    let i = s.info();
    assert_eq!((i.width, i.height), (512, 384));
    assert!(i.fast_regions);
    assert!(i.scales.len() >= 6 && i.scales[5] == 32, "{:?}", i.scales);
    let c = CancelToken::new();
    let r = s
        .decode(
            &req(
                1,
                Some(Rect {
                    x: 100,
                    y: 50,
                    w: 64,
                    h: 32,
                }),
            ),
            &c,
        )
        .unwrap();
    assert_eq!(
        (r.width, r.height, r.origin, r.scale),
        (64, 32, (100, 50), 1)
    );
    let r = s.decode(&req(16, None), &c).unwrap();
    assert_eq!((r.width, r.height), (32, 24));
    // Thumbnail, label and macro are pages; the thumbnail is the preview.
    assert_eq!(s.page_count(), 4);
    assert_eq!(s.page_label(), Some(PageLabel::Slide(SlideImage::Main)));
    let p = s.decode_preview().unwrap().unwrap();
    assert_eq!((p.width, p.height), (64, 48));
    let label = s.open_page(2).unwrap();
    assert_eq!(
        label.page_label(),
        Some(PageLabel::Slide(SlideImage::Label))
    );
    assert_eq!(label.page_index(), 2);
    let r = label.decode(&req(1, None), &c).unwrap();
    assert_eq!((r.width, r.height), (40, 30));
    let r = label
        .decode(
            &req(
                2,
                Some(Rect {
                    x: 2,
                    y: 2,
                    w: 8,
                    h: 4,
                }),
            ),
            &c,
        )
        .unwrap();
    assert_eq!((r.width, r.height), (8, 4));
    // Science facts: microns per pixel, objective, scanner; shown as a raster.
    let sc = s.science().unwrap();
    assert_eq!(sc.kind, ScienceKind::Slide);
    assert!(sc.raster_only);
    assert!(s.nd_plane(&[], &c).is_none());
    let px = sc.pixel.as_ref().unwrap();
    assert!((px.x.step - 0.499).abs() < 1e-9);
    assert_eq!(px.unit_x, "µm");
    assert_eq!(sc.magnification, Some(20.0));
    assert_eq!(sc.instrument.as_deref(), Some("SS1234"));
    assert_eq!(sc.vendor.as_deref(), Some("Aperio"));
    assert_eq!(sc.levels.len(), 3);
    assert!(s.memory_bytes() > 0);
    s.trim_memory();
}

#[test]
fn vms_opens_from_its_folder() {
    let dir = scratch("vms");
    let (set, _, (w, h)) = fx::vms_set(true);
    let names = wsi::vms::files(&set[0]).unwrap();
    std::fs::write(dir.join("s.vms"), &set[0]).unwrap();
    for (n, d) in names.iter().zip(&set[1..]) {
        std::fs::write(dir.join(n), d).unwrap();
    }
    let src = Arc::new(Source::open(dir.join("s.vms")).unwrap());
    let s = Registry::default().open(src).unwrap();
    assert_eq!(s.info().format, Format::Vms);
    assert_eq!((s.info().width, s.info().height), (w, h));
    let r = s
        .decode(
            &req(
                1,
                Some(Rect {
                    x: 100,
                    y: 40,
                    w: 80,
                    h: 50,
                }),
            ),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!((r.width, r.height), (80, 50));
    assert!(s.science().unwrap().pixel.is_some());
    assert_eq!(s.page_count(), 2);
    // A missing image file reads as background, marked truncated, not an error.
    std::fs::remove_file(dir.join(&names[3])).unwrap();
    let src = Arc::new(Source::open(dir.join("s.vms")).unwrap());
    let s = Registry::default().open(src).unwrap();
    let r = s.decode(&req(1, None), &CancelToken::new()).unwrap();
    assert!(r.truncated);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A VMS with three focal layers is a stack of pages labelled "Z n of 3" (as OMERO's
/// planes are), each showing its own layer, then the macro.
#[test]
fn vms_focal_layers_are_pages() {
    let dir = scratch("vms-layers");
    let (set, _, (w, h)) = fx::vms_layers_set();
    let names = wsi::vms::files(&set[0]).unwrap();
    std::fs::write(dir.join("s.vms"), &set[0]).unwrap();
    for (n, d) in names.iter().zip(&set[1..]) {
        std::fs::write(dir.join(n), d).unwrap();
    }
    let src = Arc::new(Source::open(dir.join("s.vms")).unwrap());
    let s = Registry::default().open(src).unwrap();
    assert_eq!((s.info().width, s.info().height), (w, h));
    assert_eq!(s.page_count(), 4);
    assert_eq!(
        s.page_label(),
        Some(PageLabel::Plane {
            z: Some((0, 3)),
            t: None
        })
    );
    // Not science planes: a slide of RGB pages.
    assert!(s.science().unwrap().axes.is_empty());
    let mut shown = Vec::new();
    for p in 0..3 {
        let l = s.open_page(p).unwrap();
        assert_eq!(l.page_index(), p);
        assert_eq!(
            l.page_label(),
            Some(PageLabel::Plane {
                z: Some((p, 3)),
                t: None
            })
        );
        assert_eq!(l.page_count(), 4);
        shown.push(l.decode(&req(1, None), &CancelToken::new()).unwrap());
    }
    assert_ne!(shown[0].planes[0].data, shown[1].planes[0].data);
    assert_ne!(shown[1].planes[0].data, shown[2].planes[0].data);
    let m = s.open_page(3).unwrap();
    assert_eq!(m.page_label(), Some(PageLabel::Slide(SlideImage::Macro)));
    assert_eq!(m.page_index(), 3);
    assert!(s.open_page(4).is_err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn ndpi_opens() {
    let (file, _) = fx::ndpi(true, false);
    let s = open_bytes("x.ndpi", file);
    assert_eq!(s.info().format, Format::Ndpi);
    let r = s
        .decode(
            &req(
                1,
                Some(Rect {
                    x: 70,
                    y: 20,
                    w: 100,
                    h: 40,
                }),
            ),
            &CancelToken::new(),
        )
        .unwrap();
    assert_eq!((r.width, r.height), (100, 40));
    assert_eq!(s.page_count(), 2);
    assert_eq!(
        s.open_page(1).unwrap().page_label(),
        Some(PageLabel::Slide(SlideImage::Macro))
    );
}

#[test]
fn slide_annotations_come_from_sidecars() {
    let d = scratch("annot");
    let (file, _) = fx::ndpi(true, false);
    let slide = d.join("s.ndpi");
    std::fs::write(&slide, file).unwrap();
    let open = || {
        let src = Arc::new(Source::open(&slide).unwrap());
        Registry::default().open(src).unwrap()
    };
    assert!(open().science().unwrap().annotations.is_none());
    // A file that is not annotations is passed over.
    std::fs::write(d.join("s.xml"), "<OME/>").unwrap();
    assert!(open().science().unwrap().annotations.is_none());
    // NDPA: 0.5 µm pixels, the image centre (128, 48) at (-3000, 1500) nm.
    std::fs::write(
        d.join("s.ndpi.ndpa"),
        r##"<annotations><ndpviewstate id="1"><title>Pin</title>
<annotation type="pin" color="#00ff00"><x>-2000</x><y>1000</y></annotation></ndpviewstate></annotations>"##,
    )
    .unwrap();
    let s = open();
    let a = s.science().unwrap().annotations.clone().unwrap();
    assert_eq!(a.file, "s.ndpi.ndpa");
    assert_eq!(a.set.items[0].points, vec![(130.0, 47.0)]);
    assert_eq!(a.set.items[0].colour, Some([0, 255, 0]));
    std::fs::remove_file(d.join("s.ndpi.ndpa")).unwrap();
    // Aperio XML by the stem.
    std::fs::write(
        d.join("s.xml"),
        r#"<Annotations><Annotation Id="1" LineColor="255"><Regions><Region Type="1" Text="box"><Vertices><Vertex X="1" Y="2"/><Vertex X="5" Y="9"/></Vertices></Region></Regions></Annotation></Annotations>"#,
    )
    .unwrap();
    let a = open().science().unwrap().annotations.clone().unwrap();
    assert_eq!(a.set.format, wsi::annot::Format::Aperio);
    assert_eq!(a.set.items[0].name, "box");
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ome_tiff_is_a_science_image() {
    let s = open_bytes("a.ome.tif", fx::ome_tiff());
    assert_eq!(s.info().format, Format::OmeTiff);
    assert_eq!(s.colour_model(), Some("Multichannel"));
    let sc = s.science().unwrap();
    assert!(!sc.raster_only);
    let names: Vec<_> = sc.axes.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["C", "Z"]);
    assert_eq!(sc.channel_axis, Some(0));
    assert_eq!(sc.axes[0].labels, ["DAPI", "GFP"]);
    assert_eq!(sc.channel_colours[0], [0.0, 0.0, 1.0]);
    assert_eq!(sc.axes[1].linear.unwrap().step, 1.5);
    let p = s.nd_plane(&[1, 1], &CancelToken::new()).unwrap().unwrap();
    assert_eq!((p.width, p.height), (64, 48));
    assert_eq!(p.value(3, 2), Some(11000.0 + 128.0 + 3.0));
    // The OME label image is a page.
    assert_eq!(s.page_count(), 2);
}

#[test]
fn ome_tiff_companions_come_from_the_folder_only() {
    let xml = r#"<?xml version="1.0"?><OME xmlns="http://www.openmicroscopy.org/Schemas/OME/2016-06" UUID="urn:uuid:a">
<Image ID="Image:0"><Pixels ID="Pixels:0" DimensionOrder="XYZCT" Type="uint16" SizeX="8" SizeY="4" SizeZ="3" SizeC="1" SizeT="1">
<Channel ID="Channel:0:0" SamplesPerPixel="1"/>
<TiffData IFD="0" FirstZ="0" PlaneCount="1"><UUID FileName="a.ome.tif">urn:uuid:a</UUID></TiffData>
<TiffData IFD="0" FirstZ="1" PlaneCount="1"><UUID FileName="b.ome.tif">urn:uuid:b</UUID></TiffData>
<TiffData IFD="0" FirstZ="2" PlaneCount="1"><UUID FileName="../c.ome.tif">urn:uuid:c</UUID></TiffData>
</Pixels></Image></OME>"#;
    let d = scratch("companions");
    let a = fx::tiff(&[fx::gray16(
        8,
        4,
        &|x, _| x as u16,
        Some(xml.to_owned()),
        false,
    )]);
    let b = fx::tiff(&[fx::gray16(8, 4, &|x, _| 100 + x as u16, None, false)]);
    let outside = d.parent().unwrap().join("c.ome.tif");
    std::fs::write(d.join("a.ome.tif"), a).unwrap();
    std::fs::write(d.join("b.ome.tif"), &b).unwrap();
    std::fs::write(&outside, &b).unwrap();
    let s = Registry::default().open_path(&d.join("a.ome.tif")).unwrap();
    let c = CancelToken::new();
    let p = s.nd_plane(&[1], &c).unwrap().unwrap();
    assert_eq!(p.value(2, 0), Some(102.0));
    // A name with a path is never followed.
    assert!(s.nd_plane(&[2], &c).unwrap().is_err());
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_file(outside);
}

#[test]
fn ome_zarr_folder_opens() {
    let root = scratch("zarr");
    let d = root.join("img.ome.zarr");
    let put = |k: &str, v: &[u8]| {
        let p = d.join(k);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, v).unwrap();
    };
    put(
        ".zattrs",
        br#"{"multiscales":[{"version":"0.4","axes":[{"name":"y","type":"space","unit":"micrometer"},{"name":"x","type":"space","unit":"micrometer"}],
        "datasets":[{"path":"0","coordinateTransformations":[{"type":"scale","scale":[0.25,0.25]}]}]}]}"#,
    );
    put(".zgroup", br#"{"zarr_format":2}"#);
    put(
        "0/.zarray",
        br#"{"zarr_format":2,"shape":[4,6],"chunks":[4,6],"dtype":"|u1","compressor":null,"fill_value":0,"order":"C","filters":null}"#,
    );
    put("0/0.0", &(0..24u8).map(|v| v * 10).collect::<Vec<_>>());
    let s = Registry::default().open_path(&d).unwrap();
    assert_eq!(s.info().format, Format::OmeZarr);
    assert_eq!((s.info().width, s.info().height), (6, 4));
    let sc = s.science().unwrap();
    assert_eq!(sc.pixel.as_ref().unwrap().x.step, 0.25);
    let r = s.decode(&req(1, None), &CancelToken::new()).unwrap();
    assert_eq!((r.width, r.height), (6, 4));
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn damaged_slides_fail_cleanly() {
    let (file, _) = fx::svs();
    let c = CancelToken::new();
    for cut in [16, 200, file.len() / 3, file.len() / 2, file.len() - 10] {
        let src = Arc::new(Source::from_vec("x.svs", file[..cut].to_vec()));
        if let Ok(s) = Registry::default().open(src) {
            let _ = s.decode(&req(1, None), &c);
            let _ = s.decode(&req(8, None), &c);
            for p in 1..s.page_count() {
                if let Ok(p) = s.open_page(p) {
                    let _ = p.decode(&req(1, None), &c);
                }
            }
        }
    }
}

fn dicom_fixture(set: &str, name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../formats/specular-wsi/tests/data/dicomwsi")
        .join(set)
        .join(name)
}

#[test]
fn dicom_wsi_series_opens_as_one_pyramid() {
    let c = CancelToken::new();
    // Any instance of the series opens the whole pyramid, with its label, overview and
    // thumbnail as pages.
    for name in ["level0.dcm", "level1.dcm", "overview.dcm"] {
        let s = Registry::default()
            .open_path(&dicom_fixture("full", name))
            .unwrap();
        let i = s.info();
        assert_eq!(i.format, Format::DicomWsi, "{name}");
        assert_eq!((i.width, i.height), (160, 96));
        assert!(i.scales.starts_with(&[1, 2, 4]), "{:?}", i.scales);
        assert_eq!(s.page_count(), 4);
        let r = s.decode(&req(4, None), &c).unwrap();
        assert_eq!((r.width, r.height), (40, 24));
        let sc = s.science().unwrap();
        assert_eq!(sc.kind, ScienceKind::Slide);
        assert_eq!(sc.pixel.as_ref().unwrap().x.step, 0.5);
        assert_eq!(sc.magnification, Some(20.0));
        assert_eq!(sc.levels.len(), 3);
        let p = s.decode_preview().unwrap().unwrap();
        assert_eq!((p.width, p.height), (20, 12));
        let label = (1..4)
            .map(|p| s.open_page(p).unwrap())
            .find(|p| p.page_label() == Some(PageLabel::Slide(SlideImage::Label)))
            .unwrap();
        let r = label.decode(&req(1, None), &c).unwrap();
        assert_eq!((r.width, r.height), (40, 32));
    }
    // Sparse tiling; the label of another series of the slide is found beside it.
    let s = Registry::default()
        .open_path(&dicom_fixture("sparse", "level0.dcm"))
        .unwrap();
    assert_eq!((s.info().width, s.info().height), (128, 96));
    assert_eq!(s.page_count(), 2);
}

#[test]
fn lone_dicom_wsi_instance_opens() {
    // Without its folder: the one level it holds; damaged copies fail cleanly or show the
    // first frame.
    let data = std::fs::read(dicom_fixture("full", "level1.dcm")).unwrap();
    let s = open_bytes("lone.dcm", data.clone());
    assert_eq!(s.info().format, Format::DicomWsi);
    assert_eq!((s.info().width, s.info().height), (80, 48));
    let c = CancelToken::new();
    for cut in [200, data.len() / 3, data.len() - 10] {
        let src = Arc::new(Source::from_vec("cut.dcm", data[..cut].to_vec()));
        if let Ok(s) = Registry::default().open(src) {
            let _ = s.decode(&req(1, None), &c);
        }
    }
}
