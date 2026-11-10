use super::*;

pub(crate) fn report(hebrew: bool) -> MetadataReport {
    let item = |key, label: &str, value: &str| SummaryItem {
        key,
        label: label.into(),
        value: value.into(),
        detail: None,
    };
    let group = |key, title: &str, items| SummaryGroup {
        key,
        title: title.into(),
        items,
    };
    let summary = if hebrew {
        vec![
            group(
                "camera",
                "מצלמה",
                vec![
                    item("model", "דגם", "Synthetic S-1"),
                    // Parts in both scripts, each keeping its order (the first on the right).
                    item(
                        "lens",
                        "עדשה",
                        &specular_i18n::join_parts(
                            &["35 mm f/1.8", "עדשה ראשית 2", "Synthetic"],
                            " · ",
                            specular_i18n::Direction::Rtl,
                        ),
                    ),
                ],
            ),
            group(
                "exposure",
                "חשיפה",
                vec![item("exposure", "חשיפה", "1/250 s · f/5.6 · ISO 200")],
            ),
            group(
                "image",
                "תמונה",
                vec![item("size", "גודל", "4000 × 3000 (12 MP)")],
            ),
        ]
    } else {
        vec![
            group(
                "camera",
                "Camera",
                vec![
                    item("model", "Model", "Synthetic S-1"),
                    item("lens", "Lens", "35 mm f/1.8"),
                ],
            ),
            group(
                "exposure",
                "Exposure",
                vec![item("exposure", "Exposure", "1/250 s · f/5.6 · ISO 200")],
            ),
            group(
                "image",
                "Image",
                vec![item("size", "Size", "4000 × 3000 (12 MP)")],
            ),
        ]
    };
    MetadataReport {
        summary,
        language: if hebrew { "he" } else { "en" }.into(),
        ..Default::default()
    }
}

/// The report of a two-page HP-GL/2 plot in a PCL job, through the metadata reader.
pub(crate) fn plot_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let input = specular_metadata::MetadataInput {
        drawing: Some(specular_metadata::DrawingFacts {
            format: "HP-GL/2".into(),
            units: 4,
            unit_symbol: Some("mm".into()),
            extents: Some((205.4, 141.25)),
            layout: Some("Page 1".into()),
            layouts: 1,
            layers: 4,
            counts: vec![
                ("Stroke".into(), 1820),
                ("Fill".into(), 64),
                ("Label".into(), 31),
            ],
            plot: Some(specular_metadata::PlotFacts {
                paper_mm: Some((210.0, 148.0)),
                pens: vec![1, 2, 5, 6],
                page: 1,
                pages: 2,
                job: Some("PCL 5".into()),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// The report of a DXF drawing whose SHX fonts were partly found, through the metadata
/// reader.
pub(crate) fn drawing_fonts_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let missing = |file: &str, styles: &[&str], sub: &str| specular_metadata::MissingFontFact {
        file: file.into(),
        styles: styles.iter().map(|s| (*s).into()).collect(),
        substitute: sub.into(),
    };
    let input = specular_metadata::MetadataInput {
        drawing: Some(specular_metadata::DrawingFacts {
            format: "DXF".into(),
            version: "AC1015 (AutoCAD 2000)".into(),
            units: 4,
            unit_symbol: Some("mm".into()),
            extents: Some((841.0, 594.0)),
            layers: 12,
            counts: vec![("TEXT".into(), 214), ("MTEXT".into(), 31)],
            codepage: Some("ANSI_932".into()),
            shx_fonts: vec!["romans.shx".into(), "txt.shx".into()],
            missing_fonts: vec![
                missing("extfont2.shx", &["KANJI"], ""),
                missing(
                    "isocp.shx",
                    &["Dimensions", "Notes"],
                    "Liberation Sans Narrow",
                ),
            ],
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// The report of LandXML survey data, through the metadata reader.
pub(crate) fn survey_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let input = specular_metadata::MetadataInput {
        drawing: Some(specular_metadata::DrawingFacts {
            format: "LandXML".into(),
            version: "1.2".into(),
            units: 6,
            unit_symbol: Some("m".into()),
            extents: Some((612.4, 388.9)),
            layers: 9,
            survey: Some(specular_metadata::SurveyFacts {
                crs: Some("ETRS89 / UTM zone 32N, EPSG:25832, DHHN2016".into()),
                unit: "m".into(),
                application: Some("Civil 3D 2024".into()),
                project: Some("Ringstraße".into()),
                surfaces: vec![specular_metadata::SurveySurface {
                    name: "Existing ground".into(),
                    points: 12_480,
                    triangles: 24_612,
                    area_2d: 182_340.5,
                    area_3d: 184_101.2,
                    volume: 1_203_310.0,
                    z_min: 312.05,
                    z_max: 341.88,
                    contour_interval: 2.0,
                }],
                alignments: vec![specular_metadata::SurveyAlignment {
                    name: "Main road".into(),
                    length: 1334.72,
                    sta_start: "1+000.00".into(),
                    sta_end: "2+334.72".into(),
                }],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// The report of a crystal structure of a protein with ligands, through the metadata
/// reader.
pub(crate) fn structure_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let input = specular_metadata::MetadataInput {
        structure: Some(specular_metadata::StructureFacts {
            id: Some("1ABC".into()),
            title: Some("Crystal structure of a haem protein with bound ligands".into()),
            classification: Some("OXYGEN TRANSPORT".into()),
            method: Some("X-RAY DIFFRACTION".into()),
            resolution: Some(1.8),
            date: Some("2024-03-15".into()),
            chains: 4,
            residues: 574,
            atoms: 4779,
            bonds: 4880,
            models: 1,
            records: 1,
            ligands: vec!["HEM".into(), "SO4".into(), "GOL".into()],
            cell: Some([63.15, 83.59, 53.80, 90.0, 99.34, 90.0]),
            space_group: Some("P 1 21 1".into()),
            secondary: Some(specular_metadata::StructureSecondary::File),
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// The report of a FLIR radiometric JPEG, through the metadata reader: its Thermal group.
pub(crate) fn thermal_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let input = specular_metadata::MetadataInput {
        science: Some(specular_metadata::ScienceFacts {
            format: "FLIR radiometric JPEG".into(),
            domain: specular_metadata::ScienceDomain::Thermal,
            date_obs: Some("2024-03-02 14:21:07".into()),
            dims: vec![640, 480],
            thermal: Some(specular_metadata::ThermalFacts {
                format: "FLIR radiometric JPEG".into(),
                camera: Some("FLIR E8".into()),
                lens: Some("FOL6".into()),
                emissivity: Some(0.95),
                distance: Some(1.0),
                reflected: Some(20.0),
                atmospheric: Some(20.0),
                humidity: Some(0.5),
                raw_range: Some((13512, 16740)),
                temperature_range: Some((18.4, 41.7)),
                unit: "°C".into(),
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// The report of a TH2F from a CERN ROOT file, through the metadata reader.
pub(crate) fn root_report(lang: &specular_metadata::LanguageIdentifier) -> MetadataReport {
    let l = specular_metadata::Localizer::new(lang.clone());
    let axis =
        |title: &str, unit: &str, bins, min, max, mean_rms| specular_metadata::RootAxisFacts {
            title: title.into(),
            unit: unit.into(),
            bins,
            min,
            max,
            variable: false,
            mean_rms: Some(mean_rms),
        };
    let input = specular_metadata::MetadataInput {
        science: Some(specular_metadata::ScienceFacts {
            format: "ROOT".into(),
            // No dimensions: the ROOT group from the top of the panel.
            dims: Vec::new(),
            root: Some(specular_metadata::RootFacts {
                version: "6.28/04".into(),
                compression: "zlib level 4".into(),
                keys: 16,
                class: "TH2F".into(),
                path: "run1/tracker/inner/occ".into(),
                title: "Inner tracker occupancy".into(),
                entries: Some(20000.0),
                sum_weights: Some(19876.0),
                axes: vec![
                    axis("x", "cm", 24, -12.0, 12.0, (0.0123, 3.98)),
                    axis("y", "cm", 16, -8.0, 8.0, (-0.0456, 2.01)),
                ],
                points: None,
                trees: vec![("events".into(), 1234)],
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    specular_metadata::read(&input, &l)
}

/// A few Exif fields for the info panel's All fields tab.
pub(crate) fn all_fields() -> Vec<FieldGroup> {
    let field = |tag: &str, v: &str| Field {
        tag: tag.into(),
        raw: v.into(),
        display: v.into(),
        empty: false,
        full: None,
    };
    vec![FieldGroup {
        title: "Exif".into(),
        fields: vec![
            field("Make", "Synthetic"),
            field("Model", "S-1"),
            field("LensModel", "35 mm f/1.8"),
            field("ExposureTime", "1/250"),
            field("FNumber", "5.6"),
            field("ISOSpeedRatings", "200"),
            field("FocalLength", "35 mm"),
            field("DateTimeOriginal", "2026:06:14 18:42:07"),
            field("Software", "Specular test"),
            field("Artist", "Test"),
            field("Copyright", "CC0"),
            field("ColorSpace", "sRGB"),
            field("PixelXDimension", "4000"),
            field("PixelYDimension", "3000"),
        ],
    }]
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn info_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c("info-dark", State::Info, true, langid!("en-US")),
        c("he-info-light", State::Info, false, langid!("he")),
        c("he-info-plot-dark", State::InfoPlot, true, langid!("he")),
        c(
            "info-drawing-fonts-light",
            State::InfoDrawingFonts,
            false,
            langid!("en-US"),
        ),
        c(
            "he-info-drawing-fonts-dark",
            State::InfoDrawingFonts,
            true,
            langid!("he"),
        ),
        c(
            "info-survey-dark",
            State::InfoSurvey,
            true,
            langid!("en-US"),
        ),
        c(
            "he-info-survey-light",
            State::InfoSurvey,
            false,
            langid!("he"),
        ),
        c(
            "info-structure-dark",
            State::InfoStructure,
            true,
            langid!("en-US"),
        ),
        c(
            "he-info-structure-light",
            State::InfoStructure,
            false,
            langid!("he"),
        ),
        c(
            "info-thermal-dark",
            State::InfoThermal,
            true,
            langid!("en-US"),
        ),
        c("info-root-dark", State::InfoRoot, true, langid!("en-US")),
    ]
}
