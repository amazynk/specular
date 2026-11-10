use super::scale::{self, Axis, Logicle, Scale, Stats};
use super::*;

fn kv(pairs: &[(&'static str, &str)]) -> Vec<(&'static str, String)> {
    pairs.iter().map(|(k, v)| (*k, (*v).to_owned())).collect()
}

/// Two 16-bit parameters, big-endian, three events; P2 log amplified (4 decades).
fn int_file() -> Vec<u8> {
    let text = kv(&[
        ("$BYTEORD", "2,1"),
        ("$DATATYPE", "I"),
        ("$MODE", "L"),
        ("$PAR", "2"),
        ("$TOT", "3"),
        ("$P1N", "FSC-A"),
        ("$P1B", "16"),
        ("$P1R", "1024"),
        ("$P1E", "0,0"),
        ("$P1G", "2"),
        ("$P2N", "FL1-H"),
        ("$P2S", "CD3 / FITC"),
        ("$P2B", "16"),
        ("$P2R", "1024"),
        ("$P2E", "4,0"),
    ]);
    // P1 has a high bit set past its range: masked.
    let vals: [u16; 6] = [100 | 0x8000, 0, 512, 256, 1023, 1024 - 1];
    let data: Vec<u8> = vals.iter().flat_map(|v| v.to_be_bytes()).collect();
    write_dataset(&text, &data)
}

#[test]
fn header_text_and_integer_data() {
    let f = int_file();
    assert!(sniff(&f));
    let fcs = parse(&f).unwrap();
    let d = &fcs.datasets[0];
    assert_eq!(d.version, "FCS3.1");
    assert_eq!(d.events, 3);
    assert_eq!(d.params[1].label, "CD3 / FITC");
    assert!(!d.truncated);
    let ch = d.read_channels(&f).unwrap();
    assert_eq!(ch.values, vec![100.0, 0.0, 512.0, 256.0, 1023.0, 1023.0]);
    let sc = d.read_events(&f, true).unwrap();
    // Gain 2 on P1; P2: 10^(4·x/1024).
    assert_eq!(sc.get(0, 0), 50.0);
    assert_eq!(sc.get(0, 1), 1.0);
    assert!((sc.get(1, 1) - 10.0).abs() < 1e-4);
    assert_eq!(d.params[1].scale_top(), 10_000.0);
}

#[test]
fn floats_spillover_and_compensation() {
    let text = kv(&[
        ("$BYTEORD", "1,2,3,4"),
        ("$DATATYPE", "F"),
        ("$PAR", "3"),
        ("$TOT", "2"),
        ("$P1N", "FSC-A"),
        ("$P1B", "32"),
        ("$P1R", "262144"),
        ("$P2N", "FITC-A"),
        ("$P2B", "32"),
        ("$P2R", "262144"),
        ("$P3N", "PE-A"),
        ("$P3B", "32"),
        ("$P3R", "262144"),
        ("$SPILLOVER", "2,FITC-A,PE-A,1,0.2,0.1,1"),
    ]);
    // True signals (1000, 0) and (0, 500) observed through the spill.
    let obs = [10.0f32, 1000.0, 200.0, 20.0, 50.0, 500.0];
    let data: Vec<u8> = obs.iter().flat_map(|v| v.to_le_bytes()).collect();
    let f = write_dataset(&text, &data);
    let d = &parse(&f).unwrap().datasets[0];
    let s = d.spillover.as_ref().unwrap();
    assert_eq!(s.params, vec![1, 2]);
    let ev = d.read_events(&f, true).unwrap();
    assert!(ev.compensated);
    assert!((ev.get(0, 1) - 1000.0).abs() < 1e-3 && ev.get(0, 2).abs() < 1e-3);
    assert!(ev.get(1, 1).abs() < 1e-3 && (ev.get(1, 2) - 500.0).abs() < 1e-3);
    assert_eq!(ev.get(1, 0), 20.0);
    let raw = d.read_events(&f, false).unwrap();
    assert_eq!(raw.get(0, 2), 200.0);
}

#[test]
fn several_datasets_ascii_and_refusals() {
    // Dataset 1 (ASCII, delimited) chained to dataset 2 by $NEXTDATA.
    let a = write_dataset(
        &kv(&[
            ("$DATATYPE", "A"),
            ("$PAR", "2"),
            ("$TOT", "2"),
            ("$P1N", "A"),
            ("$P1B", "*"),
            ("$P1R", "100"),
            ("$P2N", "B"),
            ("$P2B", "*"),
            ("$P2R", "100"),
            ("$NEXTDATA", "000000000000"),
        ]),
        b"1 2,3\n4",
    );
    let b = int_file();
    let next = format!("{:0>12}", a.len());
    let a = String::from_utf8(a)
        .unwrap()
        .replacen("$NEXTDATA/000000000000", &format!("$NEXTDATA/{next}"), 1)
        .into_bytes();
    let mut file = a.clone();
    file.extend_from_slice(&b);
    let fcs = parse(&file).unwrap();
    assert_eq!(fcs.datasets.len(), 2);
    let ev = fcs.datasets[0].read_channels(&file).unwrap();
    assert_eq!(ev.values, vec![1.0, 2.0, 3.0, 4.0]);
    let ev2 = fcs.datasets[1].read_channels(&file).unwrap();
    assert_eq!(ev2.get(1, 0), 512.0);
    // Histogram mode and packed integers are refused by name.
    let h = write_dataset(
        &kv(&[("$MODE", "U"), ("$PAR", "1"), ("$DATATYPE", "I")]),
        &[],
    );
    assert!(matches!(parse(&h), Err(Error::Unsupported(_))));
    let p = write_dataset(
        &kv(&[
            ("$PAR", "1"),
            ("$DATATYPE", "I"),
            ("$P1B", "10"),
            ("$TOT", "1"),
        ]),
        &[0, 0],
    );
    assert!(matches!(parse(&p), Err(Error::Unsupported(_))));
    // Escaped delimiters; truncated data reads as zero, marked.
    let t = parse_text(b"/$P1S/CD3//FITC/$PAR/1/");
    assert_eq!(t[0].1, "CD3/FITC");
    let mut short = int_file();
    short.truncate(short.len() - 3);
    let d = &parse(&short).unwrap().datasets[0];
    assert!(d.truncated);
    assert_eq!(d.read_channels(&short).unwrap().values.len(), 6);
    assert!(parse(b"FCS3.1    garbage").is_err());
}

#[test]
fn hostile_sizes() {
    let big = write_dataset(
        &kv(&[
            ("$PAR", "4000"),
            ("$DATATYPE", "F"),
            ("$TOT", "999999999999"),
        ]),
        &[],
    );
    // No $PnB for float params: malformed, never an allocation.
    assert!(parse(&big).is_err());
    let mut t = vec![("$DATATYPE", "F".to_owned()), ("$PAR", "1".into())];
    t.push(("$TOT", "99999999999".into()));
    t.push(("$P1B", "32".into()));
    let f = write_dataset(&t, &[0; 8]);
    let d = &parse(&f).unwrap().datasets[0];
    assert_eq!(d.read_channels(&f), Err(Error::TooLarge));
}

#[test]
fn logicle_round_trips_and_anchors() {
    let l = Logicle::new(262_144.0, 0.5, 4.5, 0.0).unwrap();
    assert!((l.scale(262_144.0) - 1.0).abs() < 1e-9);
    assert!(l.inverse(l.scale(0.0)).abs() < 1e-6);
    for x in [-300.0, -10.0, 0.5, 42.0, 1e3, 5e4, 3e5] {
        let y = l.scale(x);
        assert!((l.inverse(y) - x).abs() < 1e-6 * x.abs().max(1.0), "{x}");
    }
    // Monotone.
    let ys: Vec<f64> = (-100..100).map(|i| l.scale(f64::from(i) * 50.0)).collect();
    assert!(ys.windows(2).all(|w| w[1] > w[0]));
    assert!(Logicle::new(-1.0, 0.5, 4.5, 0.0).is_none());
}

#[test]
fn axes_density_and_ticks() {
    let ev = Events {
        params: 2,
        count: 4,
        values: vec![0.0, 0.0, 1000.0, 1000.0, 500.0, 10.0, -5.0, 2000.0],
        compensated: false,
    };
    let st = Stats::of(1000.0, (0.0, 0.0), ev.column(0));
    let lin = Axis::new(Scale::Linear, &st);
    assert_eq!(lin.unit(1000.0), 1.0);
    assert!((lin.value(lin.unit(250.0)) - 250.0).abs() < 1e-9);
    let d = scale::density(&ev, 0, &lin, 1, &lin, 4);
    assert_eq!(d.iter().sum::<f32>(), 4.0);
    // (1000, 1000) and (−5, 2000): the top row (clamped); (0, 0): bottom left.
    assert_eq!(d[12], 1.0);
    let ticks = lin.ticks();
    assert!(ticks.iter().any(|(_, l)| l == "1K"));
    for s in Scale::ALL {
        let a = Axis::new(s, &st);
        let t = a.ticks();
        assert!(!t.is_empty(), "{s:?}");
        assert!(t.windows(2).all(|w| w[1].0 >= w[0].0));
    }
    let log = Axis::new(Scale::Log, &Stats::of(1e4, (4.0, 0.0), ev.column(0)));
    assert_eq!(log.unit(1.0), 0.0);
    assert_eq!(log.unit(1e4), 1.0);
    let h = scale::histogram(&ev, 0, &lin, 4);
    assert_eq!(h.iter().sum::<f64>(), 4.0);
    let img = scale::histogram_image(&h, 4);
    assert_eq!(img.len(), 16);
    assert_eq!(scale::short(50_000.0), "50K");
    assert_eq!(scale::short(-2500.0), "−2.5K");
}

/// Events using 5 % of `$PnR` (Fortessa, Miltenyi files): the linear, arcsinh and logicle
/// axes fit the data, so the density plot is not an empty corner (the visual review).
#[test]
fn axes_fit_data_far_below_the_range() {
    let values: Vec<f32> = (0..1000).map(|i| (i % 100) as f32 * 32.77).collect();
    let st = Stats::of(262_144.0, (0.0, 0.0), values.iter().copied());
    for s in [Scale::Linear, Scale::Arcsinh, Scale::Logicle] {
        let a = Axis::new(s, &st);
        let top = a.unit(3244.0);
        assert!(top > 0.9 && top <= 1.0, "{s:?}: the largest event at {top}");
    }
    // Data that fills its range keeps `$PnR`.
    let full: Vec<f32> = (0..1000).map(|i| i as f32 * 262.0).collect();
    let st = Stats::of(262_144.0, (0.0, 0.0), full.iter().copied());
    assert_eq!(Axis::new(Scale::Linear, &st).unit(262_144.0), 1.0);
}
