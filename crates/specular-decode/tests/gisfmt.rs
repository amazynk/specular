//! GIS rasters and vectors of the fmt500d corpus through the registry: BSB/KAP charts,
//! ER Mapper, SAGA grids, Arc/Info grids (a folder), GDAL VRTs over other rasters, S-57
//! cells, TopoJSON, IGC and NMEA logs. Each opens as its format, georeferenced, and draws;
//! the readers' own values come through the science view. Small files are written here;
//! the corpus (`tools/corpus/fetch-gisfmt.sh` and its oracle, read from
//! SPECULAR_GISFMT_CORPUS or $CARGO_TARGET_DIR/corpus/gisfmt) adds GDAL's samples and the
//! VRTs' values as rasterio reads them (skipped without it).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use specular_core::{CancelToken, PixelLayout, Source};
use specular_decode::geo::GeoFacts;
use specular_decode::{DecodeError, DecodeRequest, Format, Registry, Session};

fn open_path(p: &Path) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::default().open(Arc::new(Source::open(p).map_err(DecodeError::Io)?))
}

fn open_bytes(name: &str, data: &[u8]) -> specular_decode::Result<Arc<dyn Session>> {
    Registry::default().open(Arc::new(Source::from_vec(name, data.to_vec())))
}

fn rgba(s: &dyn Session) -> Vec<u8> {
    let r = s
        .decode(&DecodeRequest::full(1), &CancelToken::new())
        .expect("decodes");
    assert_eq!((r.width, r.height), (s.info().width, s.info().height));
    assert_eq!(r.layout, PixelLayout::Rgba);
    let p = &r.planes[0];
    (0..r.height as usize)
        .flat_map(|y| p.data[y * p.stride..y * p.stride + r.width as usize * 4].to_vec())
        .collect()
}

fn values(s: &dyn Session, index: &[u32]) -> Vec<f32> {
    s.nd_plane(index, &CancelToken::new())
        .expect("values")
        .expect("plane")
        .values
        .clone()
}

/// A fresh folder under the test temp dir.
fn folder(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("specular-gisfmt-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn kap() -> Vec<u8> {
    let mut v = b"! test\r\nVER/3.0\r\nBSB/NA=Harbour,NU=1,RA=4,2,DU=254\r\nKNP/SC=25000,GD=WGS84,PR=MERCATOR,PP=0.0\r\nRGB/1,255,0,0\r\nRGB/2,0,0,255\r\nREF/1,0,0,50.0,1.0\r\nREF/2,4,0,50.0,1.4\r\nREF/3,0,2,49.9,1.0\r\n".to_vec();
    v.extend([0x1A, 0x00, 2]);
    // Depth 2: row 1 = colour 1 ×3, colour 2 ×1; row 2 = colour 2 ×4.
    v.extend([1, 0b0010_0010, 0b0100_0000, 0, 2, 0b0100_0011, 0]);
    v
}

#[test]
fn extensions_and_types() {
    let reg = Registry::default();
    let claims = reg.extension_claims();
    for (ext, f) in [
        ("kap", Format::BsbChart),
        ("bsb", Format::BsbChart),
        ("ers", Format::ErMapper),
        ("sgrd", Format::SagaGrid),
        ("sg-grd-z", Format::SagaGrid),
        ("vrt", Format::GdalVrt),
        ("000", Format::S57),
        ("topojson", Format::TopoJson),
        ("igc", Format::Igc),
        ("nmea", Format::Nmea),
    ] {
        assert_eq!(claims.get(ext), Some(&vec![f]), ".{ext}");
    }
    let groups = reg.mime_groups();
    let has = |m: &str, e: &str| groups.iter().any(|(t, x)| *t == m && x.contains(&e));
    assert!(has("application/x-bsb-chart", "kap"));
    assert!(has("application/x-gdal-vrt", "vrt"));
    assert!(has("application/x-igc", "igc"));
    // `.000` and `.sdat` are not registered.
    assert!(
        !groups
            .iter()
            .any(|(_, x)| x.contains(&"000") || x.contains(&"sdat"))
    );
}

#[test]
fn chart_in_its_colours_on_the_map() {
    let s = open_bytes("harbour.kap", &kap()).unwrap();
    assert_eq!(s.info().format, Format::BsbChart);
    assert_eq!((s.info().width, s.info().height), (4, 2));
    let px = rgba(s.as_ref());
    assert_eq!(&px[..4], [255, 0, 0, 255]);
    assert_eq!(&px[12..16], [0, 0, 255, 255]);
    assert_eq!(&px[16..20], [0, 0, 255, 255]);
    let g = s.geo().expect("placed");
    assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(3395));
    let (x, y) = g.map_at(4.0, 0.0);
    let (lon, lat) = g.crs.as_ref().unwrap().to_lonlat(x, y).unwrap();
    assert!((lon - 1.4).abs() < 1e-9 && (lat - 50.0).abs() < 1e-9);
    let GeoFacts::Raster(r) = &g.facts else {
        panic!()
    };
    assert!(
        r.keys
            .iter()
            .any(|(k, v)| k == "BSB" && v.contains("Harbour"))
    );
    // A `.bsb` description opens its first panel beside it.
    let d = folder("bsb");
    std::fs::write(d.join("HARBOUR_1.KAP"), kap()).unwrap();
    std::fs::write(
        d.join("harbour.bsb"),
        b"CHT/NA=Harbour,NU=1\r\nK01/NA=Main\r\n    FN=harbour_1.kap\r\n",
    )
    .unwrap();
    let s = open_path(&d.join("harbour.bsb")).unwrap();
    assert_eq!(s.info().format, Format::BsbChart);
    std::fs::remove_file(d.join("HARBOUR_1.KAP")).unwrap();
    assert!(matches!(
        open_path(&d.join("harbour.bsb")),
        Err(DecodeError::Unsupported(_))
    ));
    let _ = std::fs::remove_dir_all(&d);
}

#[test]
fn ermapper_and_saga_beside_their_data() {
    let d = folder("raw");
    std::fs::write(
        d.join("dem.ers"),
        "DatasetHeader Begin\n\tByteOrder = MSBFirst\n\tCoordinateSpace Begin\n\t\tDatum = \"WGS84\"\n\t\tProjection = \"NUTM33\"\n\tCoordinateSpace End\n\tRasterInfo Begin\n\t\tCellType = Signed16BitInteger\n\t\tNullCellValue = -9999\n\t\tCellInfo Begin\n\t\t\tXdimension = 30\n\t\t\tYdimension = 30\n\t\tCellInfo End\n\t\tNrOfLines = 2\n\t\tNrOfCellsPerLine = 3\n\t\tRegistrationCoord Begin\n\t\t\tEastings = 500000\n\t\t\tNorthings = 4600000\n\t\tRegistrationCoord End\n\t\tNrOfBands = 1\n\tRasterInfo End\nDatasetHeader End\n",
    )
    .unwrap();
    let raw: Vec<u8> = [1i16, 2, 3, -9999, 5, 6]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    std::fs::write(d.join("dem"), &raw).unwrap();
    for p in ["dem.ers", "dem"] {
        let s = open_path(&d.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert_eq!(s.info().format, Format::ErMapper, "{p}");
        let v = values(s.as_ref(), &[]);
        assert_eq!(v[..3], [1.0, 2.0, 3.0]);
        assert!(v[3].is_nan());
        let g = s.geo().unwrap();
        assert_eq!(g.crs.as_ref().and_then(|c| c.epsg), Some(32633));
        assert_eq!(g.map_at(0.0, 0.0), (500000.0, 4600000.0));
    }
    std::fs::write(
        d.join("grid.sgrd"),
        "NAME\t= grid\nDATAFORMAT\t= FLOAT\nBYTEORDER_BIG\t= FALSE\nPOSITION_XMIN\t= 5\nPOSITION_YMIN\t= 5\nCELLCOUNT_X\t= 2\nCELLCOUNT_Y\t= 2\nCELLSIZE\t= 10\nNODATA_VALUE\t= -99999\nTOPTOBOTTOM\t= FALSE\n",
    )
    .unwrap();
    let f: Vec<u8> = [1f32, 2.0, 3.0, -99999.0]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    std::fs::write(d.join("grid.sdat"), &f).unwrap();
    for p in ["grid.sgrd", "grid.sdat"] {
        let s = open_path(&d.join(p)).unwrap_or_else(|e| panic!("{p}: {e}"));
        assert_eq!(s.info().format, Format::SagaGrid, "{p}");
        // Stored bottom row first.
        let v = values(s.as_ref(), &[]);
        assert_eq!(v[0], 3.0);
        assert!(v[1].is_nan());
        assert_eq!(v[2..], [1.0, 2.0]);
        assert_eq!(s.geo().unwrap().map_at(0.0, 0.0), (0.0, 20.0));
        assert_eq!(s.science().unwrap().title.as_deref(), Some("grid"));
    }
    // The header without its data says what is missing.
    std::fs::remove_file(d.join("grid.sdat")).unwrap();
    assert!(matches!(
        open_path(&d.join("grid.sgrd")),
        Err(DecodeError::Unsupported(e)) if e.contains(".sdat")
    ));
    let _ = std::fs::remove_dir_all(&d);
}

/// A 3 × 2 Arc/Info integer grid (one 4 × 2 block, literal runs) with a colour file.
fn write_grid(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    let mut hdr = b"GRID1.2\0".to_vec();
    hdr.resize(308, 0);
    hdr[16..20].copy_from_slice(&1i32.to_be_bytes());
    hdr[256..264].copy_from_slice(&10f64.to_be_bytes());
    hdr[264..272].copy_from_slice(&10f64.to_be_bytes());
    for (at, v) in [(288, 1i32), (292, 1), (296, 4), (300, 1), (304, 2)] {
        hdr[at..at + 4].copy_from_slice(&v.to_be_bytes());
    }
    std::fs::write(dir.join("hdr.adf"), hdr).unwrap();
    let bnd: Vec<u8> = [100f64, 200.0, 130.0, 220.0]
        .iter()
        .flat_map(|v| v.to_be_bytes())
        .collect();
    std::fs::write(dir.join("dblbnd.adf"), bnd).unwrap();
    // 0xD7: literal 3 (1, 2, 1), no data 1, literal 3 (2, 2, 1), no data 1.
    let block = [0xD7, 0, 3, 1, 2, 1, 0xFF, 3, 2, 2, 1, 0xFF];
    let mut data = vec![0u8; 100];
    data.extend((block.len() as u16 / 2).to_be_bytes());
    data.extend(block);
    std::fs::write(dir.join("w001001.adf"), data).unwrap();
    let mut index = vec![0u8; 100];
    index.extend(50i32.to_be_bytes());
    index.extend((block.len() as i32 / 2).to_be_bytes());
    std::fs::write(dir.join("w001001x.adf"), index).unwrap();
    std::fs::write(
        dir.join("prj.adf"),
        "Projection    UTM\nZone          55\nDatum         GDA94\nUnits         METERS\nYshift        10000000.0\n",
    )
    .unwrap();
}

#[test]
fn arcinfo_grid_folder() {
    let d = folder("aig");
    let g = d.join("landuse");
    write_grid(&g);
    std::fs::write(
        d.join("landuse.clr"),
        "# value red green blue\n1 255 0 0\n2 0 255 0\n",
    )
    .unwrap();
    assert!(specular_core::is_image_folder(&g));
    assert!(Registry::default().supports_file(&g));
    for p in [g.clone(), g.join("hdr.adf")] {
        let s = open_path(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
        assert_eq!(s.info().format, Format::ArcInfoGrid);
        assert_eq!((s.info().width, s.info().height), (3, 2));
        let v = values(s.as_ref(), &[]);
        assert_eq!(v[..3], [1.0, 2.0, 1.0]);
        assert_eq!(v[3..], [2.0, 2.0, 1.0]);
        let geo = s.geo().unwrap();
        assert_eq!(geo.crs.as_ref().and_then(|c| c.epsg), Some(28355));
        // The colour file colours the values.
        let px = rgba(s.as_ref());
        assert_eq!(&px[..8], [255, 0, 0, 255, 0, 255, 0, 255]);
    }
    let _ = std::fs::remove_dir_all(&d);
}

fn asc(cols: usize, rows: usize, x0: f64, vals: &[i32]) -> String {
    let mut t = format!(
        "ncols {cols}\nnrows {rows}\nxllcorner {x0}\nyllcorner 0\ncellsize 1\nNODATA_value -9999\n"
    );
    for r in vals.chunks(cols) {
        t += &r
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        t.push('\n');
    }
    t
}

#[test]
fn vrt_mosaic_scaled_and_refused() {
    let d = folder("vrt");
    std::fs::write(d.join("a.asc"), asc(2, 2, 0.0, &[1, 2, 3, -9999])).unwrap();
    std::fs::write(d.join("b.asc"), asc(2, 2, 2.0, &[5, 6, 7, 8])).unwrap();
    let src = |f: &str, dx: u32, extra: &str, kind: &str| {
        format!(
            "<{kind}><SourceFilename relativeToVRT=\"1\">{f}</SourceFilename><SourceBand>1</SourceBand>\
             <SrcRect xOff=\"0\" yOff=\"0\" xSize=\"2\" ySize=\"2\"/><DstRect xOff=\"{dx}\" yOff=\"0\" xSize=\"2\" ySize=\"2\"/>{extra}</{kind}>"
        )
    };
    let vrt = format!(
        "<VRTDataset rasterXSize=\"4\" rasterYSize=\"2\"><SRS>EPSG:4326</SRS><GeoTransform>0, 1, 0, 2, 0, -1</GeoTransform>\
         <VRTRasterBand dataType=\"Float32\" band=\"1\"><NoDataValue>-1</NoDataValue>{}{}</VRTRasterBand></VRTDataset>",
        src("a.asc", 0, "<NODATA>-9999</NODATA>", "ComplexSource"),
        src(
            "b.asc",
            2,
            "<ScaleOffset>0.5</ScaleOffset><ScaleRatio>2</ScaleRatio>",
            "ComplexSource"
        )
    );
    std::fs::write(d.join("m.vrt"), &vrt).unwrap();
    let s = open_path(&d.join("m.vrt")).unwrap();
    assert_eq!(s.info().format, Format::GdalVrt);
    let v = values(s.as_ref(), &[]);
    // Each source in its window, the second scaled; no data stays empty.
    assert_eq!(v[..4], [1.0, 2.0, 10.5, 12.5]);
    assert_eq!(v[4], 3.0);
    assert!(v[5].is_nan());
    assert_eq!(v[6..], [14.5, 16.5]);
    assert_eq!(
        s.geo().unwrap().crs.as_ref().and_then(|c| c.epsg),
        Some(4326)
    );
    // A missing source leaves its window empty and says the picture is incomplete.
    std::fs::remove_file(d.join("b.asc")).unwrap();
    let s = open_path(&d.join("m.vrt")).unwrap();
    let p = s.nd_plane(&[], &CancelToken::new()).unwrap().unwrap();
    assert!(p.truncated);
    assert!(p.values[2].is_nan());
    // Sources on a server, in GDAL's virtual file systems, warped rasters: refused.
    for (name, text) in [
        (
            "http.vrt",
            vrt.replace("b.asc", "https://example.org/b.tif"),
        ),
        ("vsi.vrt", vrt.replace("b.asc", "/vsizip/x.zip/b.tif")),
        (
            "warp.vrt",
            vrt.replace("<VRTDataset", "<VRTDataset subClass=\"VRTWarpedDataset\""),
        ),
    ] {
        std::fs::write(d.join(name), text).unwrap();
        assert!(
            matches!(open_path(&d.join(name)), Err(DecodeError::Unsupported(_))),
            "{name}"
        );
    }
    // A VRT naming itself ends at the nesting limit instead of recursing.
    std::fs::write(
        d.join("self.vrt"),
        vrt.replace("a.asc", "self.vrt")
            .replace("b.asc", "self.vrt"),
    )
    .unwrap();
    let s = open_path(&d.join("self.vrt")).unwrap();
    let p = s.nd_plane(&[], &CancelToken::new()).unwrap();
    assert!(p.is_err() || p.unwrap().truncated);
    // From memory (an archive), sources cannot be found.
    assert!(matches!(
        open_bytes("m.vrt", vrt.as_bytes()),
        Err(DecodeError::Unsupported(_))
    ));
    let _ = std::fs::remove_dir_all(&d);
}

fn check_map(s: &dyn Session, want: Format, features: usize) {
    assert_eq!(s.info().format, want);
    let g = s.geo().expect("georeferenced");
    let GeoFacts::Vector(v) = &g.facts else {
        panic!("not a vector map")
    };
    assert_eq!(v.features, features);
    let px = rgba(s);
    assert!(px.chunks(4).any(|p| p != &px[..4]), "nothing drawn");
}

#[test]
fn tracks_and_topologies_as_maps() {
    let topo =
        br#"{"type":"Topology","objects":{"roads":{"type":"GeometryCollection","geometries":[
        {"type":"LineString","arcs":[0],"properties":{"name":"A"}}]},
        "spot":{"type":"Point","coordinates":[3,3]}},
        "arcs":[[[0,0],[5,5],[10,0]]]}"#;
    let s = open_bytes("roads.json", topo).unwrap();
    check_map(s.as_ref(), Format::TopoJson, 2);
    let igc = "AXXX001 logger\r\nHFDTE010725\r\nHFPLTPILOTINCHARGE:Alex\r\nB1000004630000N00800000EA0100001100\r\nB1000044630500N00800500EA0101001110\r\nB1000084631000N00801000EA0102001120\r\n";
    let s = open_bytes("flight.igc", igc.as_bytes()).unwrap();
    check_map(s.as_ref(), Format::Igc, 1);
    let sum = |b: &str| format!("${b}*{:02X}\r\n", b.bytes().fold(0u8, |a, c| a ^ c));
    let nmea = [
        "GPRMC,120000.00,A,4630.000,N,00800.000,E,5.0,90.0,010725,,",
        "GPRMC,120001.00,A,4630.010,N,00800.020,E,5.0,90.0,010725,,",
        "GPRMC,120002.00,A,4630.020,N,00800.040,E,5.0,90.0,010725,,",
    ]
    .map(sum)
    .concat();
    let s = open_bytes("walk.log", nmea.as_bytes()).unwrap();
    check_map(s.as_ref(), Format::Nmea, 1);
    // A log with no fix is refused as unsupported, not as damage.
    let empty = [sum("GPGSV,1,1,00"), sum("GPGSV,1,1,00")].concat();
    assert!(matches!(
        open_bytes("x.nmea", empty.as_bytes()),
        Err(DecodeError::Unsupported(_))
    ));
}

fn corpus() -> Option<PathBuf> {
    let dir = std::env::var_os("SPECULAR_GISFMT_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/gisfmt"))
        })?;
    dir.join("expected").is_dir().then_some(dir)
}

/// Every corpus file opens as its format and draws; the VRTs' values match rasterio's.
#[test]
fn corpus_opens_and_vrts_match_gdal() {
    let Some(dir) = corpus() else {
        eprintln!("no gisfmt corpus; skipping");
        return;
    };
    let mut n = 0;
    let mut values_checked = 0usize;
    let mut bad = Vec::new();
    let cases: &[(&str, &str, Format)] = &[
        ("bsb", "kap", Format::BsbChart),
        ("ers", "ers", Format::ErMapper),
        ("gen/ers", "ers", Format::ErMapper),
        ("saga", "sgrd", Format::SagaGrid),
        ("saga", "sg-grd-z", Format::SagaGrid),
        ("gen/saga", "sgrd", Format::SagaGrid),
        ("vrt", "vrt", Format::GdalVrt),
        ("gen/vrt", "vrt", Format::GdalVrt),
        ("s57", "000", Format::S57),
        ("topojson", "topojson", Format::TopoJson),
        ("gen/igc", "igc", Format::Igc),
        ("gen/nmea", "nmea", Format::Nmea),
    ];
    for (sub, ext, f) in cases {
        let Ok(rd) = std::fs::read_dir(dir.join(sub)) else {
            continue;
        };
        let mut paths: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| e.eq_ignore_ascii_case(ext))
            })
            .collect();
        paths.sort();
        for p in paths {
            let name = p.strip_prefix(&dir).unwrap().to_string_lossy().into_owned();
            let s = match open_path(&p) {
                Ok(s) => s,
                // A cell holding only its dataset record has nothing to draw.
                Err(e) if name.contains("fake_s57") => {
                    eprintln!("{name}: {e}");
                    continue;
                }
                Err(e) => {
                    bad.push(format!("{name}: {e}"));
                    continue;
                }
            };
            assert_eq!(s.info().format, *f, "{name}");
            let r = s.decode(&DecodeRequest::full(1), &CancelToken::new());
            if let Err(e) = r {
                bad.push(format!("{name}: decode: {e}"));
                continue;
            }
            n += 1;
            if *f != Format::GdalVrt {
                continue;
            }
            let ej: serde_json::Value = serde_json::from_slice(
                &std::fs::read(dir.join("expected").join(format!("{name}.json"))).unwrap(),
            )
            .unwrap();
            let count = ej["count"].as_u64().unwrap() as u32;
            let nodata = ej["nodata"][0].as_f64();
            for b in 0..count {
                let idx = if count == 1 { vec![] } else { vec![b] };
                let v = values(s.as_ref(), &idx);
                let g: Vec<f64> =
                    std::fs::read(dir.join("expected").join(format!("{name}.b{}.f64", b + 1)))
                        .unwrap()
                        .chunks_exact(8)
                        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
                        .collect();
                values_checked += g.len();
                let wrong = v
                    .iter()
                    .zip(&g)
                    .filter(|&(&a, &b)| {
                        // No data as the band's type holds it (Float32: -1e30 rounded).
                        let nd = nodata
                            .is_some_and(|n| b as f32 == n as f32 || (n.is_nan() && b.is_nan()));
                        !((a.is_nan() && (nd || b.is_nan())) || f64::from(a) == (b as f32 as f64))
                    })
                    .count();
                if wrong > 0 || v.len() != g.len() {
                    let i = v
                        .iter()
                        .zip(&g)
                        .position(|(&a, &b)| f64::from(a) != (b as f32 as f64))
                        .unwrap_or(0);
                    bad.push(format!(
                        "{name} band {}: {wrong} of {} values differ (first at {i}: {} vs {})",
                        b + 1,
                        g.len(),
                        v.get(i).copied().unwrap_or(f32::NAN),
                        g.get(i).copied().unwrap_or(f64::NAN)
                    ));
                }
            }
        }
    }
    // The Arc/Info grids open as folders.
    for g in ["abc3x1", "ABC3X1UC", "teststa"] {
        match open_path(&dir.join("aig").join(g)) {
            Ok(s) => {
                assert_eq!(s.info().format, Format::ArcInfoGrid);
                rgba(s.as_ref());
                n += 1;
            }
            Err(e) => bad.push(format!("aig/{g}: {e}")),
        }
    }
    eprintln!(
        "{n} files opened, {values_checked} VRT values checked, {} problems",
        bad.len()
    );
    assert!(n >= 40, "{n}");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
