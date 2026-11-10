//! DICONDE (ASTM E2339) files against tests/data/diconde_oracle.tsv, which pydicom read back
//! from the fixtures gen/diconde_make.py wrote: the NDE summary and the dictionary's names for
//! group 0014.

use specular_dicom::Dicom;

fn file(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data")
            .join(name),
    )
    .unwrap()
}

fn oracle() -> Vec<Vec<&'static str>> {
    include_str!("data/diconde_oracle.tsv")
        .lines()
        .map(|l| l.split('\t').collect::<Vec<_>>())
        .collect()
}

fn opt(s: &str) -> Option<String> {
    (!s.is_empty()).then(|| s.to_owned())
}

#[test]
fn nde_summary_matches_pydicom() {
    let mut checked = 0;
    for name in ["diconde-ct.dcm", "diconde-ec.dcm"] {
        let data = file(name);
        let d = Dicom::parse(&data).unwrap();
        let s = d.summary(&data);
        let nde = s
            .nde
            .clone()
            .unwrap_or_else(|| panic!("{name}: not told as DICONDE"));
        for f in oracle().iter().filter(|f| f[0] == "nde" && f[1] == name) {
            let (field, want) = (f[2], f[3]);
            let num = |x: Option<f64>| x.map(|x| format!("{x:?}")).unwrap_or_default();
            let got = match field {
                "modality" => s.modality.clone().unwrap_or_default(),
                "component_shape" => nde.component_shape.clone().unwrap_or_default(),
                "component_manufacturer" => nde.component_manufacturer.clone().unwrap_or_default(),
                "material_grade" => nde.material_grade.clone().unwrap_or_default(),
                "material_thickness" => num(nde.material_thickness),
                "environment" => nde.environment.clone().unwrap_or_default(),
                "iqi" => nde.iqi.clone().unwrap_or_default(),
                "indications" => nde.indications.to_string(),
                "kvp" => num(nde.source.filter(|s| s.1 == "kV").map(|s| s.0)),
                f => panic!("unknown field {f}"),
            };
            assert_eq!(opt(&got), opt(want), "{name} {field}");
            checked += 1;
        }
        // The component's name and ID (kept where a patient's would be) and the evaluator's
        // name stay out of the summary.
        let text = format!("{s:?}");
        for hidden in ["Blade 7", "TB-0007", "Smith"] {
            assert!(!text.contains(hidden), "{name}: {hidden} in the summary");
        }
        d.check_supported().unwrap();
    }
    assert_eq!(checked, 18);
}

#[test]
fn group_0014_is_named() {
    let mut n = 0;
    for f in oracle().iter().filter(|f| f[0] == "kw") {
        let tag = u32::from_str_radix(f[1], 16).unwrap();
        assert_eq!(specular_dicom::dict::keyword(tag), Some(f[2]), "{}", f[1]);
        n += 1;
    }
    assert!(n >= 10);
}

/// A medical image (no group 0014, a medical class) is not taken for an inspection.
#[test]
fn medical_images_are_not_nde() {
    let data = file("rwvm.dcm");
    let d = Dicom::parse(&data).unwrap();
    assert!(d.summary(&data).nde.is_none());
}
