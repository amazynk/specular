use super::*;

/// A synthetic CT header as the app would list it: file meta, patient attributes (hidden),
/// a sequence of two items, pixel data. No real patient's data.
pub(crate) fn dicom_tags() -> DicomTags {
    let row = |depth, tag: &str, keyword: &str, vr: &str, length: &str, value: &str| TagLine {
        depth,
        tag: tag.into(),
        keyword: keyword.into(),
        vr: vr.into(),
        length: length.into(),
        value: value.into(),
        descendants: 0,
        identifying: false,
        jump: false,
    };
    let hidden = |mut r: TagLine| {
        r.identifying = true;
        r
    };
    let mut rows = vec![
        row(
            0,
            "(0002,0001)",
            "FileMetaInformationVersion",
            "OB",
            "2",
            "2 bytes: 00 01",
        ),
        row(
            0,
            "(0002,0010)",
            "TransferSyntaxUID",
            "UI",
            "20",
            "1.2.840.10008.1.2.1",
        ),
        row(0, "(0008,0060)", "Modality", "CS", "2", "CT"),
        row(
            0,
            "(0008,103E)",
            "SeriesDescription",
            "LO",
            "18",
            "Thorax 1.0 B70f",
        ),
        hidden(row(
            0,
            "(0010,0010)",
            "PatientName",
            "PN",
            "10",
            "TEST^ANON",
        )),
        hidden(row(0, "(0010,0020)", "PatientID", "LO", "6", "000000")),
        hidden(row(
            0,
            "(0010,0030)",
            "PatientBirthDate",
            "DA",
            "8",
            "19000101",
        )),
        row(0, "(0018,0050)", "SliceThickness", "DS", "4", "1.00"),
        row(
            0,
            "(0020,0032)",
            "ImagePositionPatient",
            "DS",
            "26",
            "-175.0\\-180.0\\-312.5",
        ),
        row(0, "(0020,1041)", "SliceLocation", "DS", "6", "-312.5"),
        row(0, "(0028,0010)", "Rows", "US", "2", "512"),
        row(0, "(0028,0011)", "Columns", "US", "2", "512"),
        row(0, "(0028,1050)", "WindowCenter", "DS", "8", "-600\\40"),
        row(0, "(0028,1051)", "WindowWidth", "DS", "8", "1200\\400"),
    ];
    let mut seq = row(
        0,
        "(0040,0275)",
        "RequestAttributesSequence",
        "SQ",
        "",
        "2 items",
    );
    seq.descendants = 6;
    rows.push(seq);
    for k in 1..=2 {
        let mut item = row(1, "", &format!("Item {k}"), "", "", "");
        item.descendants = 2;
        rows.push(item);
        rows.push(row(
            2,
            "(0040,0007)",
            "ScheduledProcedureStepDescription",
            "LO",
            "12",
            "CT THORAX",
        ));
        rows.push(row(
            2,
            "(0040,0009)",
            "ScheduledProcedureStepID",
            "SH",
            "4",
            &format!("SP{k}"),
        ));
    }
    rows.push(row(
        0,
        "(7FE0,0010)",
        "PixelData",
        "OW",
        "524,288",
        "524,288 bytes",
    ));
    DicomTags {
        title: "CT000042.dcm".into(),
        rows,
        status: None,
        note: None,
        heads: None,
    }
}

/// A synthetic Comprehensive SR as the app lists it in report mode: a measurement report
/// whose last finding points at an image. No real patient's data.
pub(crate) fn sr_tree() -> DicomTags {
    let row = |depth, item: &str, concept: &str, vt: &str, rel: &str, value: &str| TagLine {
        depth,
        tag: item.into(),
        keyword: concept.into(),
        vr: vt.into(),
        length: rel.into(),
        value: value.into(),
        descendants: 0,
        identifying: false,
        jump: false,
    };
    let mut rows = vec![row(
        0,
        "1",
        "Imaging Measurement Report",
        "CONTAINER",
        "",
        "SEPARATE",
    )];
    let children = [
        (
            "1.1",
            "Language of Content Item and Descendants",
            "CODE",
            "HAS CONCEPT MOD",
            "English",
        ),
        ("1.2", "Observer Type", "CODE", "HAS OBS CONTEXT", "Person"),
        (
            "1.3",
            "Person Observer Name",
            "PNAME",
            "HAS OBS CONTEXT",
            "Observer^Synthetic",
        ),
        (
            "1.4",
            "Procedure Reported",
            "CODE",
            "HAS CONCEPT MOD",
            "CT of chest",
        ),
        (
            "1.5",
            "Imaging Measurements",
            "CONTAINER",
            "CONTAINS",
            "SEPARATE",
        ),
        (
            "1.5.1",
            "Tracking Identifier",
            "TEXT",
            "HAS OBS CONTEXT",
            "Lesion 1",
        ),
        ("1.5.2", "Long Axis", "NUM", "CONTAINS", "23.4 mm"),
        ("1.5.3", "Short Axis", "NUM", "CONTAINS", "15.1 mm"),
        (
            "1.5.4",
            "Finding Site",
            "CODE",
            "HAS CONCEPT MOD",
            "Upper lobe of right lung",
        ),
        (
            "1.5.5",
            "Image Region",
            "SCOORD",
            "INFERRED FROM",
            "POLYLINE (4 points) on image 1.2.3.4",
        ),
    ];
    for (item, concept, vt, rel, value) in children {
        let depth = item.matches('.').count() as u16;
        rows.push(row(depth, item, concept, vt, rel, value));
    }
    rows[0].descendants = children.len() as u32;
    rows[3].identifying = true;
    rows[5].descendants = 5;
    rows[10].jump = true;
    DicomTags {
        title: "SR000001.dcm".into(),
        rows,
        status: None,
        note: None,
        heads: Some((
            [
                "Item".into(),
                "Concept".into(),
                "Type".into(),
                "Relationship".into(),
                "Value".into(),
            ],
            "Imaging Measurement Report (Comprehensive SR)".into(),
        )),
    }
}

/// Synthetic cumulative DVHs (a target and two organs) with their table.
pub(crate) fn dvh_sheet() -> PlotSheet {
    // Page: 0–70 Gy across 140 units, 0–100 % up 100 units.
    let (x0, y0, w, h) = (14.0, 4.0, 140.0, 100.0);
    let curve = |d50: f64, slope: f64| -> Vec<[f32; 2]> {
        (0..=140)
            .map(|i| {
                let dose = i as f64 * 0.5;
                let v = 100.0 / (1.0 + ((dose - d50) / slope).exp());
                [(x0 + dose * 2.0) as f32, (y0 + h - v) as f32]
            })
            .collect()
    };
    let rois = [
        ("PTV", [230u8, 70, 70], curve(60.0, 0.8)),
        ("Lung R", [80, 160, 240], curve(12.0, 6.0)),
        ("Heart", [240, 190, 60], curve(6.0, 3.0)),
    ];
    let x_ticks = nice_ticks(0.0, 70.0, 7)
        .into_iter()
        .map(|g| (x0 + g * 2.0, format!("{g:.0}")))
        .collect();
    let y_ticks = nice_ticks(0.0, 100.0, 5)
        .into_iter()
        .map(|p| (y0 + h - p, format!("{p:.0}")))
        .collect();
    PlotSheet {
        title: "Dose volume histograms".into(),
        page: [x0 + w + 4.0, y0 + h + 12.0],
        grid: PlotGrid::Ticks,
        cells: vec![PlotCell {
            rect: [x0, y0, w, h],
            traces: rois
                .iter()
                .map(|(n, c, p)| PlotTrace {
                    points: p.clone(),
                    srgb: *c,
                    name: (*n).into(),
                })
                .collect(),
            x_ticks,
            y_ticks,
            x_title: "Dose (Gy)".into(),
            y_title: "Volume (%)".into(),
            x_value: (0.0, 0.5),
            y_value: (0.0, 1.0),
            x_unit: "Gy".into(),
            y_unit: "%".into(),
            ..PlotCell::default()
        }],
        caliper: CaliperKind::Value,
        subtitle: "Prescription 60 Gy".into(),
        legend: rois.iter().map(|(n, c, _)| ((*n).into(), *c)).collect(),
        table: [
            ["ROI", "Volume", "D95", "Dmean", "Dmax", "V20"],
            [
                "PTV",
                "182.4 cm³",
                "58.1 Gy",
                "60.0 Gy",
                "63.2 Gy",
                "100.0 %",
            ],
            [
                "Lung R",
                "1610.2 cm³",
                "0.4 Gy",
                "13.1 Gy",
                "62.0 Gy",
                "21.7 %",
            ],
            ["Heart", "590.7 cm³", "0.2 Gy", "6.4 Gy", "41.5 Gy", "6.2 %"],
        ]
        .iter()
        .map(|r| r.iter().map(|s| (*s).to_owned()).collect())
        .collect(),
        ..PlotSheet::default()
    }
}

/// A synthetic 12-lead ECG at 25 mm/s and 10 mm/mV: four columns of three leads, 2.5 s
/// each, and a 10 s rhythm strip of II.
pub(crate) fn ecg_sheet() -> PlotSheet {
    // A beat (mV) at t seconds into the R–R interval, scaled per lead.
    let beat = |t: f64, amp: f64, tw: f64| {
        let g = |c: f64, w: f64, a: f64| a * (-((t - c) / w).powi(2) * 0.5).exp();
        g(0.10, 0.025, 0.15)
            + g(0.24, 0.012, amp)
            + g(0.27, 0.01, -0.25 * amp.abs())
            + g(0.22, 0.01, -0.1)
            + g(0.52, 0.05, tw)
    };
    let leads: [(&str, f64, f64); 12] = [
        ("I", 0.9, 0.25),
        ("II", 1.3, 0.35),
        ("III", 0.5, 0.1),
        ("aVR", -1.0, -0.25),
        ("aVL", 0.3, 0.1),
        ("aVF", 0.9, 0.2),
        ("V1", -0.8, -0.1),
        ("V2", 0.6, 0.4),
        ("V3", 1.0, 0.45),
        ("V4", 1.6, 0.45),
        ("V5", 1.4, 0.35),
        ("V6", 1.0, 0.25),
    ];
    let rr = 60.0 / 72.0;
    let trace = |x: f64, y: f64, t0: f64, secs: f64, amp: f64, tw: f64| {
        (0..=(secs * 250.0) as usize)
            .map(|i| {
                let t = t0 + i as f64 / 250.0;
                let mv = beat(t % rr, amp, tw);
                [(x + (t - t0) * 25.0) as f32, (y - mv * 10.0) as f32]
            })
            .collect::<Vec<_>>()
    };
    let (left, top, row_h) = (5.0, 5.0, 25.0);
    let mut cells = Vec::new();
    for (k, (name, amp, tw)) in leads.iter().enumerate() {
        let (col, row) = (k / 3, k % 3);
        let x = left + col as f64 * 62.5;
        let y = top + row as f64 * row_h;
        cells.push(PlotCell {
            rect: [x, y, 62.5, row_h],
            label: (*name).into(),
            traces: vec![PlotTrace {
                points: trace(x, y + row_h * 0.6, col as f64 * 2.5, 2.5, *amp, *tw),
                srgb: [0, 0, 0],
                name: (*name).into(),
            }],
            x_value: (col as f64 * 2.5, 1.0 / 25.0),
            y_value: (-row_h * 0.4 / 10.0, 0.1),
            x_unit: "s".into(),
            y_unit: "mV".into(),
            ..PlotCell::default()
        });
    }
    let y = top + 3.0 * row_h;
    cells.push(PlotCell {
        rect: [left, y, 250.0, row_h],
        label: "II".into(),
        traces: vec![PlotTrace {
            points: trace(left, y + row_h * 0.6, 0.0, 10.0, 1.3, 0.35),
            srgb: [0, 0, 0],
            name: "II".into(),
        }],
        x_value: (0.0, 1.0 / 25.0),
        y_value: (-row_h * 0.4 / 10.0, 0.1),
        x_unit: "s".into(),
        y_unit: "mV".into(),
        ..PlotCell::default()
    });
    PlotSheet {
        title: "12-lead ECG".into(),
        page: [260.0, top * 2.0 + 4.0 * row_h],
        grid: PlotGrid::Paper {
            minor: 1.0,
            major: 5.0,
        },
        cells,
        caliper: CaliperKind::Time,
        subtitle: "25 mm/s · 10 mm/mV · 500 Hz".into(),
        paper: true,
        adjustable: true,
        ..PlotSheet::default()
    }
}

/// A synthetic patient disc as the app would list it (made-up patient, no real data).
pub(crate) fn studies(thumbs: &[(wgpu::Texture, (TexId, u32, u32))]) -> Studies {
    let row = |kind, title: &str, detail: &str, id| StudyLine {
        kind,
        title: title.into(),
        detail: detail.into(),
        identifying: false,
        hidden_title: String::new(),
        thumb: None,
        badge: String::new(),
        id,
        missing: false,
    };
    let series = |title: &str, detail: &str, badge: &str, id, thumb: Option<usize>| StudyLine {
        thumb: thumb.map(|i| thumbs[i].1),
        badge: badge.into(),
        ..row(StudyKind::Series, title, detail, id)
    };
    let mut patient = row(StudyKind::Patient, "DOE, JANE", "Born 1 Jan 1970", 0);
    patient.identifying = true;
    patient.hidden_title = "Patient 1".into();
    let mut gone = series(
        "Series 4: T2 sagittal",
        "MR · 24 images, all missing",
        "MR",
        4,
        None,
    );
    gone.missing = true;
    Studies {
        title: "DISC_0001".into(),
        rows: vec![
            patient,
            row(
                StudyKind::Study,
                "CT Thorax with contrast",
                "3 Feb 2024 · CT",
                0,
            ),
            series("Series 1: Scout", "CT · 2 images", "CT", 1, Some(1)),
            series(
                "Series 2: Thorax 1.0 B70f",
                "CT · 312 images",
                "CT",
                2,
                Some(0),
            ),
            series("Series 3: Dose report", "SR · 1 document", "SR", 3, None),
            row(StudyKind::Study, "MR Knee left", "12 Oct 2023 · MR", 0),
            gone,
        ],
        status: None,
        note: Some("24 files listed on the disc are missing".into()),
        cancellable: false,
        sendable: false,
    }
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn medical_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c("dicom-tags-dark", State::DicomTags, true, langid!("en-US")),
        c("sr-tree-dark", State::SrTree, true, langid!("en-US")),
        c("dvh-plot-dark", State::PlotDvh, true, langid!("en-US")),
        c("ecg-12-lead-light", State::PlotEcg, false, langid!("en-US")),
        c("studies-dark", State::Studies, true, langid!("en-US")),
        c("studies-light", State::Studies, false, langid!("en-US")),
        c("he-studies-dark", State::Studies, true, langid!("he")),
        c(
            "studies-reading-dark",
            State::StudiesReading,
            true,
            langid!("en-US"),
        ),
        c(
            "studies-send-dark",
            State::StudiesSend,
            true,
            langid!("en-US"),
        ),
        c(
            "he-studies-send-dark",
            State::StudiesSend,
            true,
            langid!("he"),
        ),
        c(
            "studies-patient-light",
            State::StudiesPatient,
            false,
            langid!("en-US"),
        ),
        c(
            "dicom-tags-filter-light",
            State::DicomTagsFilter,
            false,
            langid!("en-US"),
        ),
        c(
            "dicom-tags-filter-select-light",
            State::DicomTagsFilterSelect,
            false,
            langid!("en-US"),
        ),
        c(
            "he-dicom-tags-filter-select-dark",
            State::DicomTagsFilterSelect,
            true,
            langid!("he"),
        ),
    ]
}
