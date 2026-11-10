use super::*;

const UUID: [u8; 16] = [
    0x55, 0x4a, 0x27, 0xfa, 0x79, 0xd2, 0x47, 0x66, 0x9a, 0x2c, 0x86, 0x2e, 0x19, 0x1c, 0xb9, 0xb1,
];

fn put(ibd: &mut Vec<u8>, dtype: DataType, v: &[f64]) -> u64 {
    let off = ibd.len() as u64;
    for &x in v {
        match dtype {
            DataType::F32 => ibd.extend((x as f32).to_le_bytes()),
            DataType::F64 => ibd.extend(x.to_le_bytes()),
            DataType::I32 => ibd.extend((x as i32).to_le_bytes()),
            DataType::I64 => ibd.extend((x as i64).to_le_bytes()),
        }
    }
    off
}

fn acc(t: DataType) -> &'static str {
    match t {
        DataType::F32 => r#"<cvParam cvRef="MS" accession="MS:1000521" name="32-bit float"/>"#,
        DataType::F64 => r#"<cvParam cvRef="MS" accession="MS:1000523" name="64-bit float"/>"#,
        DataType::I32 => r#"<cvParam cvRef="IMS" accession="IMS:1000141" name="32-bit integer"/>"#,
        DataType::I64 => r#"<cvParam cvRef="MS" accession="MS:1000522" name="64-bit integer"/>"#,
    }
}

/// A data set: `spectra` are (x, y, m/z, intensities).
struct Fixture {
    processed: bool,
    mz_type: DataType,
    int_type: DataType,
    compression: &'static str,
    spectra: Vec<(u32, u32, Vec<f64>, Vec<f64>)>,
}

impl Fixture {
    fn build(&self) -> (String, Vec<u8>) {
        let mut ibd = UUID.to_vec();
        let shared = (!self.processed).then(|| put(&mut ibd, self.mz_type, &self.spectra[0].2));
        let mut body = String::new();
        for (i, (x, y, mz, int)) in self.spectra.iter().enumerate() {
            let mo = shared.unwrap_or_else(|| put(&mut ibd, self.mz_type, mz));
            let io = put(&mut ibd, self.int_type, int);
            let n = mz.len();
            body += &format!(
                r#"<spectrum id="Scan={i}" defaultArrayLength="0" index="{i}">
<referenceableParamGroupRef ref="spectrum1"/>
<scanList count="1"><scan><cvParam cvRef="IMS" accession="IMS:1000050" name="position x" value="{x}"/>
<cvParam cvRef="IMS" accession="IMS:1000051" name="position y" value="{y}"/></scan></scanList>
<binaryDataArrayList count="2">
<binaryDataArray encodedLength="0"><referenceableParamGroupRef ref="mzArray"/>
<cvParam accession="IMS:1000103" name="external array length" value="{n}"/>
<cvParam accession="IMS:1000104" name="external encoded length" value="{}"/>
<cvParam accession="IMS:1000102" name="external offset" value="{mo}"/><binary/></binaryDataArray>
<binaryDataArray encodedLength="0"><referenceableParamGroupRef ref="intensities"/>
<cvParam accession="IMS:1000103" name="external array length" value="{n}"/>
<cvParam accession="IMS:1000104" name="external encoded length" value="{}"/>
<cvParam accession="IMS:1000102" name="external offset" value="{io}"/><binary/></binaryDataArray>
</binaryDataArrayList></spectrum>
"#,
                n * self.mz_type.size(),
                n * self.int_type.size(),
            );
        }
        let mode = if self.processed {
            r#"<cvParam cvRef="IMS" accession="IMS:1000031" name="processed"/>"#
        } else {
            r#"<cvParam cvRef="IMS" accession="IMS:1000030" name="continuous"/>"#
        };
        let xml = format!(
            r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<mzML xmlns="http://psi.hupo.org/ms/mzml" version="1.1">
<fileDescription><fileContent>
<cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum"/>
<cvParam cvRef="IMS" accession="IMS:1000080" name="universally unique identifier" value="{{554A27FA-79D2-4766-9A2C-862E191CB9B1}}"/>
{mode}
</fileContent></fileDescription>
<referenceableParamGroupList count="3">
<referenceableParamGroup id="mzArray">
<cvParam cvRef="MS" accession="MS:1000514" name="m/z array" unitAccession="MS:1000040" unitName="m/z"/>
{}
{}
<cvParam cvRef="IMS" accession="IMS:1000101" name="external data" value="true"/>
</referenceableParamGroup>
<referenceableParamGroup id="intensities">
<cvParam cvRef="MS" accession="MS:1000515" name="intensity array"/>
{}
<cvParam cvRef="MS" accession="MS:1000576" name="no compression"/>
<cvParam cvRef="IMS" accession="IMS:1000101" name="external data" value="true"/>
</referenceableParamGroup>
<referenceableParamGroup id="spectrum1">
<cvParam cvRef="MS" accession="MS:1000579" name="MS1 spectrum"/>
<cvParam cvRef="MS" accession="MS:1000130" name="positive scan"/>
<cvParam cvRef="MS" accession="MS:1000128" name="profile spectrum"/>
</referenceableParamGroup>
</referenceableParamGroupList>
<scanSettingsList count="1"><scanSettings id="s1">
<cvParam cvRef="IMS" accession="IMS:1000046" name="pixel size (x)" value="50.0" unitName="micrometer"/>
<cvParam cvRef="IMS" accession="IMS:1000047" name="pixel size y" value="25.0"/>
</scanSettings></scanSettingsList>
<instrumentConfigurationList count="1"><instrumentConfiguration id="IC1">
<cvParam cvRef="MS" accession="MS:1000557" name="LTQ FT Ultra"/>
<cvParam cvRef="MS" accession="MS:1000529" name="instrument serial number" value="none"/>
<componentList count="1"><source order="1"><cvParam cvRef="MS" accession="MS:1000075" name="matrix-assisted laser desorption ionization"/></source></componentList>
</instrumentConfiguration></instrumentConfigurationList>
<run defaultInstrumentConfigurationRef="IC1" id="r"><spectrumList count="{}" defaultDataProcessingRef="dp">
{body}</spectrumList></run></mzML>
"#,
            acc(self.mz_type),
            self.compression,
            acc(self.int_type),
            self.spectra.len()
        );
        (xml, ibd)
    }
}

fn sample(processed: bool, mz_type: DataType, int_type: DataType) -> Fixture {
    let mut spectra = Vec::new();
    for y in 1..=3u32 {
        for x in 1..=4u32 {
            if (x, y) == (4, 3) {
                continue; // A hole.
            }
            let k = f64::from(x + 10 * y);
            let mz: Vec<f64> = if processed {
                (0..(3 + x as usize))
                    .map(|i| 100.0 + 50.0 * i as f64 + k * 0.01)
                    .collect()
            } else {
                vec![100.0, 150.0, 200.0, 250.0, 300.0]
            };
            let int: Vec<f64> = (0..mz.len()).map(|i| k + i as f64).collect();
            spectra.push((x, y, mz, int));
        }
    }
    Fixture {
        processed,
        mz_type,
        int_type,
        compression: r#"<cvParam cvRef="MS" accession="MS:1000576" name="no compression"/>"#,
        spectra,
    }
}

fn never() -> bool {
    false
}

#[test]
fn both_modes_all_types() {
    for processed in [false, true] {
        for (mt, it) in [
            (DataType::F32, DataType::F32),
            (DataType::F64, DataType::I32),
            (DataType::F64, DataType::I64),
            (DataType::F32, DataType::F64),
        ] {
            let f = sample(processed, mt, it);
            let (xml, ibd) = f.build();
            let m = ImzMl::parse(xml.as_bytes()).unwrap();
            m.check_ibd(&ibd).unwrap();
            assert_eq!(
                m.mode,
                if processed {
                    Mode::Processed
                } else {
                    Mode::Continuous
                }
            );
            assert_eq!(m.shared_mz, !processed);
            assert_eq!((m.width, m.height, m.depth()), (4, 3, 1));
            assert_eq!(m.origin, (1, 1));
            assert_eq!(m.pixel_size, Some((50.0, 25.0)));
            assert_eq!(m.instrument.as_deref(), Some("LTQ FT Ultra"));
            assert_eq!(m.polarity, Some(Polarity::Positive));
            assert_eq!(m.centroid, Some(false));
            assert_eq!(m.uuid, Some(UUID));
            assert_eq!(m.entries.len(), 11);
            assert_eq!(m.index_at(3, 2, 0), None);
            let i = m.index_at(2, 1, 0).unwrap();
            let s = m.spectrum(&ibd, i);
            assert_eq!(s.mz.len(), f.spectra[i].2.len());
            assert_eq!(s.intensity[1], 24.0);
            assert!((s.mz[0] - f.spectra[i].2[0]).abs() < 1e-4);

            let tic = m.tic(&ibd, &never).unwrap();
            let img = m.image(&tic, 0);
            let want = |x: u32, y: u32, n: usize| {
                let k = f64::from(x + 10 * y);
                (0..n).map(|i| k + i as f64).sum::<f64>() as f32
            };
            let n = |x: u32| if processed { 3 + x as usize } else { 5 };
            assert_eq!(img[0], want(1, 1, n(1)));
            assert_eq!(img[4 + 2], want(3, 2, n(3)));
            assert!(img[11].is_nan());

            // 200 ± 0.5 Da: the third peak (index 2) of every spectrum.
            let w = Window {
                mz: 200.2,
                tolerance: Tolerance::Da(0.5),
            };
            let ion = m.image(&m.ion(&ibd, w, &never).unwrap(), 0);
            assert_eq!(ion[0], 11.0 + 2.0);
            assert_eq!(ion[5], 22.0 + 2.0);
            // Nothing there.
            let w = Window {
                mz: 175.0,
                tolerance: Tolerance::Ppm(10.0),
            };
            let ion = m.image(&m.ion(&ibd, w, &never).unwrap(), 0);
            assert_eq!(ion[0], 0.0);
            assert!(ion[11].is_nan());
            let (lo, hi) = m.mz_range(&ibd).unwrap();
            assert!((100.0..100.5).contains(&lo) && hi >= 250.0, "{lo} {hi}");
        }
    }
}

#[test]
fn tolerance_windows() {
    let w = Window {
        mz: 500.0,
        tolerance: Tolerance::Ppm(10.0),
    };
    assert!((w.half_width() - 0.005).abs() < 1e-12);
    let (lo, hi) = w.bounds();
    assert!((lo - 499.995).abs() < 1e-9 && (hi - 500.005).abs() < 1e-9);
    let w = Window {
        mz: 500.0,
        tolerance: Tolerance::Da(-0.25),
    };
    assert_eq!(w.bounds(), (499.75, 500.25));
}

#[test]
fn binary_search() {
    let mut b = Vec::new();
    let v = [1.0, 2.0, 2.0, 2.0, 3.0, 5.0];
    put(&mut b, DataType::F64, &v);
    let n = v.len();
    assert_eq!(lower_bound(&b, DataType::F64, n, 2.0), 1);
    assert_eq!(upper_bound(&b, DataType::F64, n, 2.0), 4);
    assert_eq!(lower_bound(&b, DataType::F64, n, 0.0), 0);
    assert_eq!(upper_bound(&b, DataType::F64, n, 9.0), n);
    assert_eq!(lower_bound(&b, DataType::F64, n, 4.0), 5);
    assert_eq!(lower_bound(&b, DataType::F64, 0, 4.0), 0);
    let a = Array {
        offset: 0,
        len: n as u64,
        dtype: DataType::F64,
    };
    // Both ends included.
    assert_eq!(window_range(&b, &a, (2.0, 3.0)), 1..5);
    assert_eq!(window_range(&b, &a, (3.5, 4.5)), 5..5);
    assert_eq!(window_range(&b, &a, (6.0, 7.0)), 6..6);
    // Against a linear scan, on integers too.
    let mut b = Vec::new();
    let v: Vec<f64> = (0..200).map(|i| f64::from(i / 3)).collect();
    put(&mut b, DataType::I32, &v);
    let a = Array {
        offset: 0,
        len: v.len() as u64,
        dtype: DataType::I32,
    };
    for lo in -2..70 {
        for w in 0..4 {
            let (l, h) = (f64::from(lo) - 0.5, f64::from(lo + w));
            let want: Vec<usize> = (0..v.len()).filter(|&i| v[i] >= l && v[i] <= h).collect();
            let r = window_range(&b, &a, (l, h));
            assert_eq!(r.clone().collect::<Vec<_>>(), want, "{l} {h}");
        }
    }
}

#[test]
fn refuses_compressed() {
    for c in [
        r#"<cvParam cvRef="MS" accession="MS:1000574" name="zlib compression"/>"#,
        r#"<cvParam cvRef="MS" accession="MS:1002312" name="MS-Numpress linear prediction compression"/>"#,
    ] {
        let mut f = sample(true, DataType::F32, DataType::F32);
        f.compression = c;
        let (xml, _) = f.build();
        let e = ImzMl::parse(xml.as_bytes()).unwrap_err();
        assert!(matches!(e, Error::Compressed { .. }), "{e}");
        assert!(e.to_string().contains("compress"), "{e}");
    }
}

#[test]
fn refuses_wrong_or_truncated_ibd() {
    let (xml, mut ibd) = sample(true, DataType::F64, DataType::F32).build();
    let m = ImzMl::parse(xml.as_bytes()).unwrap();
    m.check_ibd(&ibd).unwrap();
    let mut other = ibd.clone();
    other[3] ^= 1;
    assert!(matches!(
        m.check_ibd(&other),
        Err(Error::UuidMismatch { .. })
    ));
    assert!(matches!(m.check_ibd(&[]), Err(Error::UuidMismatch { .. })));
    ibd.truncate(ibd.len() - 1);
    let e = m.check_ibd(&ibd).unwrap_err();
    assert!(matches!(e, Error::Truncated { spectrum: 10, .. }), "{e}");
}

#[test]
fn not_imzml_and_malformed() {
    assert!(matches!(
        ImzMl::parse(b"<svg xmlns='http://www.w3.org/2000/svg'/>"),
        Err(Error::NotImzml)
    ));
    assert!(matches!(
        ImzMl::parse(b"<mzML><run><spectrumList count='0'/></run></mzML>"),
        Err(Error::NoSpectra)
    ));
    let (xml, _) = sample(false, DataType::F32, DataType::F32).build();
    let bad = xml.replacen("IMS:1000050", "IMS:1999999", 1);
    assert!(matches!(
        ImzMl::parse(bad.as_bytes()),
        Err(Error::Malformed(_))
    ));
    let bad = xml.replacen(r#"ref="mzArray""#, r#"ref="nope""#, 1);
    assert!(matches!(
        ImzMl::parse(bad.as_bytes()),
        Err(Error::Malformed(_))
    ));
}

#[test]
fn cancel_stops() {
    let mut f = sample(false, DataType::F32, DataType::F32);
    let s = f.spectra[0].clone();
    f.spectra = (0..3000)
        .map(|i| (i % 100 + 1, i / 100 + 1, s.2.clone(), s.3.clone()))
        .collect();
    let (xml, ibd) = f.build();
    let m = ImzMl::parse(xml.as_bytes()).unwrap();
    assert!(m.tic(&ibd, &|| true).is_none());
    let t = m.tic(&ibd, &never).unwrap();
    assert_eq!(t.len(), 3000);
    assert!(t.iter().all(|&v| v == t[0]));
}

#[test]
fn helpers() {
    assert_eq!(
        parse::unescape(b"a &amp; b &lt;&#65;&#x42;&bogus"),
        "a & b <AB&bogus"
    );
    assert_eq!(
        parse::parse_uuid("{554a27fa-79d2-4766-9a2c-862e191cb9b1}"),
        Some(UUID)
    );
    assert_eq!(
        parse::parse_uuid("554A27FA79D247669A2C862E191CB9B1"),
        Some(UUID)
    );
    assert_eq!(parse::parse_uuid("554A"), None);
}

/// Offsets some writers store as signed 32-bit numbers (negative past 2 GiB) unwrap to
/// their true positions, past 4 GiB too.
#[test]
fn signed_offsets_unwrap() {
    let g = 1u64 << 30;
    let e = |mz: i64, int: i64| Entry {
        x: 1,
        y: 1,
        z: 1,
        mz: Array {
            offset: mz as u64,
            len: 1,
            dtype: DataType::F32,
        },
        intensity: Array {
            offset: int as u64,
            len: 1,
            dtype: DataType::F32,
        },
    };
    let s = |v: u64| i64::from(v as u32 as i32);
    let truth = [
        16,
        g,
        2 * g + 16,
        3 * g,
        4 * g + 16,
        5 * g,
        6 * g + 16,
        7 * g,
    ];
    let mut entries: Vec<Entry> = truth.iter().map(|&t| e(s(t), s(t + 8))).collect();
    unwrap_offsets(&mut entries);
    for (got, &t) in entries.iter().zip(&truth) {
        assert_eq!((got.mz.offset, got.intensity.offset), (t, t + 8));
    }
    // Offsets that are all non-negative are left alone.
    let mut plain = vec![e(16, 24), e(32, 40)];
    unwrap_offsets(&mut plain);
    assert_eq!(plain[1].mz.offset, 32);
}

mod hostile {
    use super::super::*;

    /// A negative offset (switching on the 2³² unwrapping) and then one near 2⁶³.
    #[test]
    fn offsets_unwrapped_past_the_integer_limit() {
        let entry = |offset: u64| {
            let a = Array {
                offset,
                len: 1,
                dtype: DataType::F32,
            };
            Entry {
                x: 1,
                y: 1,
                z: 0,
                mz: a,
                intensity: a,
            }
        };
        let mut e = [entry(16), entry(-8i64 as u64), entry(i64::MAX as u64 - 2)];
        unwrap_offsets(&mut e);
        assert_eq!(e[1].mz.offset, (1 << 32) - 8);
    }
}

/// Each group refers twice to the previous one: group 30 used to hold 2^30 parameters.
#[test]
fn doubling_parameter_groups_are_refused() {
    let mut xml = String::from(
        "<mzML><referenceableParamGroupList><referenceableParamGroup id='g0'>\
         <cvParam accession='MS:1000514' name='m/z array'/></referenceableParamGroup>",
    );
    for i in 1..=30 {
        xml.push_str(&format!(
            "<referenceableParamGroup id='g{i}'><referenceableParamGroupRef ref='g{p}'/>\
             <referenceableParamGroupRef ref='g{p}'/></referenceableParamGroup>",
            p = i - 1
        ));
    }
    xml.push_str("</referenceableParamGroupList><run><spectrumList count='1'/></run></mzML>");
    assert!(matches!(
        ImzMl::parse(xml.as_bytes()),
        Err(Error::Malformed(_))
    ));
}

/// Two far-apart spectra used to allocate a 40000 × 40000 index grid (6 GB).
#[test]
fn sparse_grids_are_refused() {
    let mut f = sample(false, DataType::F32, DataType::F32);
    f.spectra.truncate(1);
    let mz = f.spectra[0].2.clone();
    f.spectra.push((40_000, 40_000, mz.clone(), mz));
    let (xml, _) = f.build();
    assert!(matches!(
        ImzMl::parse(xml.as_bytes()),
        Err(Error::TooLarge(_))
    ));
}
