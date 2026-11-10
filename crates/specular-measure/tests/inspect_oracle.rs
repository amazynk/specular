//! The inspection probes on generated radiographs, for the numpy oracle
//! (`tools/oracle/inspect_oracle.py` writes the images and `cases.json`, runs this, and
//! compares). Ignored without `INSPECT_ORACLE_DIR`.

use std::path::PathBuf;

use serde_json::{Value, json};
use specular_measure::calib::{Calibration, Source, Unit};
use specular_measure::geom::P;
use specular_measure::inspect::{self, Density, Grey};

fn pt(v: &Value) -> P {
    P::new(v[0].as_f64().unwrap(), v[1].as_f64().unwrap())
}

#[test]
#[ignore = "run by tools/oracle/inspect_oracle.py"]
fn inspect_oracle() {
    let Some(dir) = std::env::var_os("INSPECT_ORACLE_DIR").map(PathBuf::from) else {
        return;
    };
    let cases: Value =
        serde_json::from_slice(&std::fs::read(dir.join("cases.json")).unwrap()).unwrap();
    let mut out = Vec::new();
    for case in cases.as_array().unwrap() {
        let (w, h) = (
            case["w"].as_u64().unwrap() as u32,
            case["h"].as_u64().unwrap() as u32,
        );
        let raw = std::fs::read(dir.join(case["file"].as_str().unwrap())).unwrap();
        let data: Vec<f64> = raw
            .chunks_exact(2)
            .map(|b| f64::from(u16::from_le_bytes([b[0], b[1]])))
            .collect();
        let g = Grey::new(0, 0, w, h, data).unwrap();
        let mm = case["mm_per_px"].as_f64().unwrap();
        let cal = Calibration::spacing(mm, mm, Unit::Mm, Source::User).unwrap();
        let mut results = Vec::new();
        for p in case["probes"].as_array().unwrap() {
            let r = match p["kind"].as_str().unwrap() {
                "profile" => {
                    let (v, step) = inspect::profile(
                        &g,
                        pt(&p["a"]),
                        pt(&p["b"]),
                        p["width"].as_u64().unwrap() as u32,
                    );
                    json!({"values": v, "step": step})
                }
                "snr" => {
                    let r = &p["rect"];
                    let rect = (
                        r[0].as_f64().unwrap(),
                        r[1].as_f64().unwrap(),
                        r[2].as_f64().unwrap(),
                        r[3].as_f64().unwrap(),
                    );
                    let region = inspect::snr_region(rect, w, h).unwrap();
                    let s = inspect::snr(&g, region, p["srb_um"].as_f64()).unwrap();
                    json!({
                        "region": [region.0, region.1, region.2, region.3],
                        "mean": s.mean, "sd": s.sd, "snr": s.snr,
                        "snrn": s.normalised.map(|n| n.1),
                    })
                }
                "duplex" => {
                    let (a, b) = (pt(&p["a"]), pt(&p["b"]));
                    let (v, _) = inspect::profile(&g, a, b, inspect::IQI_WIDTH);
                    let step_mm = inspect::mm_per_px(&cal, a, b).unwrap() * (b - a).hypot()
                        / (v.len() - 1) as f64;
                    let d = inspect::duplex(&v, step_mm).unwrap();
                    json!({
                        "dips": d.pairs.iter().map(|p| p.dip).collect::<Vec<_>>(),
                        "numbers": d.pairs.iter().map(|p| p.number.map(|n| n.0)).collect::<Vec<_>>(),
                        "first_unresolved": d.srb().map(|s| s.0),
                        "srb_mm": d.srb().map(|s| s.1),
                        "isrb_mm": d.isrb,
                        "step_mm": step_mm,
                    })
                }
                "wire" => {
                    let (a, b) = (pt(&p["a"]), pt(&p["b"]));
                    let (v, _) = inspect::profile(&g, a, b, inspect::IQI_WIDTH);
                    let step_mm = inspect::mm_per_px(&cal, a, b).unwrap() * (b - a).hypot()
                        / (v.len() - 1) as f64;
                    let wr = inspect::wire(&v, step_mm).unwrap();
                    json!({"fwhm": wr.fwhm, "number": wr.number, "nominal": wr.nominal})
                }
                "crack" => {
                    let line: Vec<P> = p["line"].as_array().unwrap().iter().map(pt).collect();
                    let c = inspect::crack(
                        &g,
                        &line,
                        p["n"].as_u64().unwrap() as usize,
                        p["half"].as_f64().unwrap(),
                        &cal,
                    )
                    .unwrap();
                    json!({
                        "px": c.cuts.iter().map(|c| c.px).collect::<Vec<_>>(),
                        "min": c.widths.0, "mean": c.widths.1, "max": c.widths.2,
                    })
                }
                "density" => {
                    let gr = &p["grey"];
                    let de = &p["density"];
                    let d = Density::new(
                        [gr[0].as_f64().unwrap(), gr[1].as_f64().unwrap()],
                        [de[0].as_f64().unwrap(), de[1].as_f64().unwrap()],
                    )
                    .unwrap();
                    let at: Vec<f64> = p["at"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|q| d.at(g.sample(pt(q))))
                        .collect();
                    json!({"density": at})
                }
                k => panic!("unknown probe {k}"),
            };
            results.push(r);
        }
        out.push(json!({"name": case["name"], "results": results}));
    }
    std::fs::write(
        dir.join("rust.json"),
        serde_json::to_vec_pretty(&out).unwrap(),
    )
    .unwrap();
}
