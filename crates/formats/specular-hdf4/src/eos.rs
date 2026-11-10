//! HDF-EOS2: grids and swaths described by the `StructMetadata.N` file attributes, and the
//! inventory (`CoreMetadata.N`) and archive (`ArchiveMetadata.N`) metadata, all ODL text.
//!
//! Grid and swath fields are ordinary scientific datasets; they are linked to [`File::sds`] by
//! name (HDF-EOS appends `:GridName` or `:SwathName` to the dimension names of its datasets,
//! which settles a name shared by several structures).

use crate::odl::{self, Group};
use crate::{File, limits};
use std::collections::{HashMap, HashSet};

/// GCTP projection codes (`ProjCode` or the `GCTP_*` names).
pub mod gctp {
    pub const GEO: i32 = 0;
    pub const UTM: i32 = 1;
    pub const SPCS: i32 = 2;
    pub const ALBERS: i32 = 3;
    pub const LAMCC: i32 = 4;
    pub const MERCAT: i32 = 5;
    pub const PS: i32 = 6;
    pub const POLYC: i32 = 7;
    pub const EQUIDC: i32 = 8;
    pub const TM: i32 = 9;
    pub const STEREO: i32 = 10;
    pub const LAMAZ: i32 = 11;
    pub const AZMEQD: i32 = 12;
    pub const GNOMON: i32 = 13;
    pub const ORTHO: i32 = 14;
    pub const GVNSP: i32 = 15;
    pub const SNSOID: i32 = 16;
    pub const EQRECT: i32 = 17;
    pub const MILLER: i32 = 18;
    pub const VGRINT: i32 = 19;
    pub const HOM: i32 = 20;
    pub const ROBIN: i32 = 21;
    pub const SOM: i32 = 22;
    pub const ALASKA: i32 = 23;
    pub const GOOD: i32 = 24;
    pub const MOLL: i32 = 25;
    pub const IMOLL: i32 = 26;
    pub const HAMMER: i32 = 27;
    pub const WAGIV: i32 = 28;
    pub const WAGVII: i32 = 29;
    pub const OBLEQA: i32 = 30;
    pub const ISINUS1: i32 = 31;
    pub const CEA: i32 = 97;
    pub const BCEA: i32 = 98;
    pub const ISINUS: i32 = 99;

    const NAMES: &[(&str, i32)] = &[
        ("GEO", GEO),
        ("UTM", UTM),
        ("SPCS", SPCS),
        ("ALBERS", ALBERS),
        ("LAMCC", LAMCC),
        ("MERCAT", MERCAT),
        ("PS", PS),
        ("POLYC", POLYC),
        ("EQUIDC", EQUIDC),
        ("TM", TM),
        ("STEREO", STEREO),
        ("LAMAZ", LAMAZ),
        ("AZMEQD", AZMEQD),
        ("GNOMON", GNOMON),
        ("ORTHO", ORTHO),
        ("GVNSP", GVNSP),
        ("SNSOID", SNSOID),
        ("EQRECT", EQRECT),
        ("MILLER", MILLER),
        ("VGRINT", VGRINT),
        ("HOM", HOM),
        ("ROBIN", ROBIN),
        ("SOM", SOM),
        ("ALASKA", ALASKA),
        ("GOOD", GOOD),
        ("MOLL", MOLL),
        ("IMOLL", IMOLL),
        ("HAMMER", HAMMER),
        ("WAGIV", WAGIV),
        ("WAGVII", WAGVII),
        ("OBLEQA", OBLEQA),
        ("ISINUS1", ISINUS1),
        ("CEA", CEA),
        ("BCEA", BCEA),
        ("ISINUS", ISINUS),
    ];

    /// The code of a `GCTP_NAME` (or bare `NAME`).
    pub fn code(name: &str) -> Option<i32> {
        let n = name.trim();
        let n = n.strip_prefix("GCTP_").unwrap_or(n);
        NAMES
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(n))
            .map(|&(_, c)| c)
    }

    /// The name of a code.
    pub fn name(code: i32) -> Option<&'static str> {
        NAMES.iter().find(|&&(_, c)| c == code).map(|&(k, _)| k)
    }
}

/// A packed GCTP angle (`DDDMMMSSS.SS`) in decimal degrees.
pub fn dms_to_deg(v: f64) -> f64 {
    let s = v.signum();
    let a = v.abs();
    let d = (a / 1e6).trunc();
    let m = ((a - d * 1e6) / 1e3).trunc();
    let sec = a - d * 1e6 - m * 1e3;
    s * (d + m / 60.0 + sec / 3600.0)
}

/// A field of a grid or swath.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub name: String,
    pub dims: Vec<String>,
    /// The field's dataset in [`File::sds`].
    pub sds: Option<usize>,
}

/// A grid: a regular raster in a GCTP projection.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub name: String,
    pub xdim: u64,
    pub ydim: u64,
    /// Outer corner, projection units (metres; packed DMS for geographic grids).
    pub upper_left: Option<(f64, f64)>,
    pub lower_right: Option<(f64, f64)>,
    /// The `Projection` as written (`GCTP_SNSOID`, …).
    pub projection: String,
    pub proj_code: Option<i32>,
    /// The 13 (or 15) GCTP projection parameters.
    pub params: Vec<f64>,
    pub zone: Option<i32>,
    pub sphere: Option<i32>,
    /// `HDFE_GD_UL` and friends.
    pub origin: Option<String>,
    /// `HDFE_CENTER` or `HDFE_CORNER`.
    pub registration: Option<String>,
    pub dims: Vec<(String, u64)>,
    pub fields: Vec<Field>,
}

impl Grid {
    /// Whether the corners are geographic (packed DMS).
    pub fn geographic(&self) -> bool {
        self.proj_code == Some(gctp::GEO)
    }

    /// The corners in projection units (degrees for geographic grids), `(ul, lr)`.
    pub fn corners(&self) -> Option<((f64, f64), (f64, f64))> {
        let (ul, lr) = (self.upper_left?, self.lower_right?);
        if self.geographic() {
            let c = |p: (f64, f64)| (dms_to_deg(p.0), dms_to_deg(p.1));
            Some((c(ul), c(lr)))
        } else {
            Some((ul, lr))
        }
    }

    /// Pixel size `(dx, dy)` in projection units; `dy` is negative when north is up.
    pub fn pixel_size(&self) -> Option<(f64, f64)> {
        let (ul, lr) = self.corners()?;
        if self.xdim == 0 || self.ydim == 0 {
            return None;
        }
        Some((
            (lr.0 - ul.0) / self.xdim as f64,
            (lr.1 - ul.1) / self.ydim as f64,
        ))
    }

    /// The field whose dataset is `sds`.
    pub fn field_of(&self, sds: usize) -> Option<&Field> {
        self.fields.iter().find(|f| f.sds == Some(sds))
    }
}

/// How a data dimension follows a geolocation dimension:
/// `data = offset + increment · geo` (a negative increment means `geo = −increment · data`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DimMap {
    pub geo: String,
    pub data: String,
    pub offset: i64,
    pub increment: i64,
}

impl DimMap {
    /// The (fractional) geolocation index of data index `i`.
    pub fn geo_index(&self, i: f64) -> f64 {
        if self.increment > 0 {
            (i - self.offset as f64) / self.increment as f64
        } else if self.increment < 0 {
            i * -(self.increment as f64) - self.offset as f64
        } else {
            i
        }
    }
}

/// A swath: along- and across-track data with geolocation fields.
#[derive(Clone, Debug, PartialEq)]
pub struct Swath {
    pub name: String,
    pub dims: Vec<(String, u64)>,
    pub maps: Vec<DimMap>,
    pub geo_fields: Vec<Field>,
    pub data_fields: Vec<Field>,
}

impl Swath {
    /// The geolocation field named like `name` (case-insensitive).
    pub fn geo_field(&self, name: &str) -> Option<&Field> {
        self.geo_fields
            .iter()
            .find(|f| f.name.eq_ignore_ascii_case(name))
    }

    /// The field (data or geolocation) whose dataset is `sds`.
    pub fn field_of(&self, sds: usize) -> Option<&Field> {
        self.data_fields
            .iter()
            .chain(&self.geo_fields)
            .find(|f| f.sds == Some(sds))
    }

    /// How data dimension `data` maps onto the geolocation dimension `geo`: identity when
    /// they are the same dimension, the dimension map otherwise.
    pub fn map(&self, geo: &str, data: &str) -> Option<DimMap> {
        if geo == data {
            return Some(DimMap {
                geo: geo.into(),
                data: data.into(),
                offset: 0,
                increment: 1,
            });
        }
        self.maps
            .iter()
            .find(|m| m.geo == geo && m.data == data)
            .cloned()
    }
}

/// HDF-EOS2 structures and metadata of a file.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Eos {
    /// `HDFEOSVersion`.
    pub version: Option<String>,
    pub grids: Vec<Grid>,
    pub swaths: Vec<Swath>,
    /// Point structures (counted, not read).
    pub points: usize,
    /// The structure metadata text.
    pub structure: String,
    /// The inventory metadata text and tree.
    pub core_text: Option<String>,
    pub core: Option<Group>,
    /// The archive metadata text and tree.
    pub archive_text: Option<String>,
    pub archive: Option<Group>,
    pub warnings: Vec<String>,
}

/// The text of the file attributes `base.0`, `base.1`, … (any case) joined.
fn joined(f: &File, base: &str) -> Option<String> {
    let mut out = String::new();
    let mut found = false;
    for k in 0..100 {
        let name = format!("{base}.{k}");
        let Some(a) = f.attrs.iter().find(|a| a.name.eq_ignore_ascii_case(&name)) else {
            // Some products name the only part without a suffix.
            if k == 0
                && let Some(a) = f.attrs.iter().find(|a| a.name.eq_ignore_ascii_case(base))
                && let Some(s) = a.value.text()
            {
                return Some(s);
            }
            break;
        };
        if let Some(s) = a.value.text() {
            out.push_str(s.trim_end_matches('\0'));
            found = true;
        }
        if out.len() > limits::MAX_ODL {
            break;
        }
    }
    found.then_some(out)
}

fn fields(idx: &SdsIndex, g: Option<&Group>, key: &str, owner: &str) -> Vec<Field> {
    let Some(g) = g else {
        return Vec::new();
    };
    g.groups
        .iter()
        .filter_map(|o| {
            let name = o.str(key)?.to_owned();
            let dims = o.get("DimList").map(|v| v.strings()).unwrap_or_default();
            let sds = idx.find(&name, owner);
            Some(Field { name, dims, sds })
        })
        .collect()
}

/// The datasets of a file by name, and by name and the `:owner` suffix of a dimension name,
/// built once: a scan of every dataset per field was quadratic in a hostile file holding
/// thousands of both.
struct SdsIndex<'a> {
    first: HashMap<&'a str, usize>,
    owned: HashMap<(&'a str, &'a str), usize>,
}

impl<'a> SdsIndex<'a> {
    /// Indexes the dimension-name suffixes that are one of `owners` (the grid and swath
    /// names).
    fn new(f: &'a File, owners: &HashSet<&str>) -> Self {
        let longest = owners.iter().map(|o| o.len()).max().unwrap_or(0);
        let mut first = HashMap::new();
        let mut owned = HashMap::new();
        for (k, s) in f.sds.iter().enumerate() {
            first.entry(s.name.as_str()).or_insert(k);
            for d in &s.dims {
                let n = d.name.as_str();
                for (p, _) in n.rmatch_indices(':') {
                    let suffix = &n[p + 1..];
                    if suffix.len() > longest {
                        break;
                    }
                    if owners.contains(suffix) {
                        owned.entry((s.name.as_str(), suffix)).or_insert(k);
                    }
                }
            }
        }
        SdsIndex { first, owned }
    }

    /// The dataset of a field: by name, preferring one whose dimension names carry
    /// `:owner` (`owner` must be among those the index was built for).
    fn find(&self, name: &str, owner: &str) -> Option<usize> {
        self.owned
            .get(&(name, owner))
            .or_else(|| self.first.get(name))
            .copied()
    }
}

/// The dataset of a field without an index (the reference for [`SdsIndex::find`]).
#[cfg(test)]
fn find_sds(f: &File, name: &str, owner: &str) -> Option<usize> {
    let suffix = format!(":{owner}");
    let mut first = None;
    for (k, s) in f.sds.iter().enumerate() {
        if s.name != name {
            continue;
        }
        if s.dims.iter().any(|d| d.name.ends_with(&suffix)) {
            return Some(k);
        }
        first.get_or_insert(k);
    }
    first
}

fn dims(g: Option<&Group>) -> Vec<(String, u64)> {
    let Some(g) = g else {
        return Vec::new();
    };
    g.groups
        .iter()
        .filter_map(|o| {
            let n = o.str("DimensionName")?.to_owned();
            let s = o.num("Size").unwrap_or(0.0);
            Some((n, if s > 0.0 { s as u64 } else { 0 }))
        })
        .collect()
}

fn pair(v: Option<&odl::Val>) -> Option<(f64, f64)> {
    let n = v?.numbers();
    (n.len() == 2).then(|| (n[0], n[1]))
}

fn grid_name(g: &Group) -> &str {
    g.str("GridName").unwrap_or(&g.name)
}

fn swath_name(g: &Group) -> &str {
    g.str("SwathName").unwrap_or(&g.name)
}

fn grid(idx: &SdsIndex, g: &Group) -> Grid {
    let name = grid_name(g).to_owned();
    let projection = g.str("Projection").unwrap_or("").to_owned();
    let proj_code = gctp::code(&projection).or_else(|| g.num("ProjCode").map(|v| v as i32));
    Grid {
        xdim: g.num("XDim").filter(|v| *v > 0.0).map_or(0, |v| v as u64),
        ydim: g.num("YDim").filter(|v| *v > 0.0).map_or(0, |v| v as u64),
        upper_left: pair(g.get("UpperLeftPointMtrs")),
        lower_right: pair(g.get("LowerRightMtrs")),
        proj_code,
        projection,
        params: g.get("ProjParams").map(|v| v.numbers()).unwrap_or_default(),
        zone: g.num("ZoneCode").map(|v| v as i32),
        sphere: g.num("SphereCode").map(|v| v as i32),
        origin: g.str("GridOrigin").map(str::to_owned),
        registration: g.str("PixelRegistration").map(str::to_owned),
        dims: dims(g.group("Dimension")),
        fields: fields(idx, g.group("DataField"), "DataFieldName", &name),
        name,
    }
}

fn swath(idx: &SdsIndex, g: &Group) -> Swath {
    let name = swath_name(g).to_owned();
    let maps = g
        .group("DimensionMap")
        .map(|m| {
            m.groups
                .iter()
                .filter_map(|o| {
                    Some(DimMap {
                        geo: o.str("GeoDimension")?.to_owned(),
                        data: o.str("DataDimension")?.to_owned(),
                        offset: o.num("Offset").unwrap_or(0.0) as i64,
                        increment: o.num("Increment").unwrap_or(1.0) as i64,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Swath {
        dims: dims(g.group("Dimension")),
        maps,
        geo_fields: fields(idx, g.group("GeoField"), "GeoFieldName", &name),
        data_fields: fields(idx, g.group("DataField"), "DataFieldName", &name),
        name,
    }
}

impl Eos {
    /// The HDF-EOS structures of a file, or `None` when it has no structure metadata.
    pub fn from_file(f: &File) -> Option<Eos> {
        let structure = joined(f, "StructMetadata")?;
        let mut eos = Eos {
            version: f
                .attrs
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("HDFEOSVersion"))
                .and_then(|a| a.value.text().map(|s| s.trim_end_matches('\0').to_owned())),
            ..Eos::default()
        };
        match odl::parse(&structure) {
            Ok(root) => {
                let gs = root.group("GridStructure");
                let ss = root.group("SwathStructure");
                let owners: HashSet<&str> = gs
                    .iter()
                    .flat_map(|gs| gs.groups.iter().map(grid_name))
                    .chain(ss.iter().flat_map(|ss| ss.groups.iter().map(swath_name)))
                    .collect();
                let idx = SdsIndex::new(f, &owners);
                if let Some(gs) = gs {
                    eos.grids = gs.groups.iter().map(|g| grid(&idx, g)).collect();
                }
                if let Some(ss) = ss {
                    eos.swaths = ss.groups.iter().map(|g| swath(&idx, g)).collect();
                }
                if let Some(ps) = root.group("PointStructure") {
                    eos.points = ps.groups.len();
                }
            }
            Err(e) => eos.warnings.push(format!("StructMetadata: {e}")),
        }
        eos.structure = structure;
        for (base, text, tree) in [
            ("CoreMetadata", &mut eos.core_text, &mut eos.core),
            ("ArchiveMetadata", &mut eos.archive_text, &mut eos.archive),
        ] {
            if let Some(t) = joined(f, base) {
                match odl::parse(&t) {
                    Ok(g) => *tree = Some(g),
                    Err(e) => eos.warnings.push(format!("{base}: {e}")),
                }
                *text = Some(t);
            }
        }
        Some(eos)
    }

    /// The `VALUE` of an inventory metadata object (`SHORTNAME`, `RANGEBEGINNINGDATE`, …),
    /// searched in the core then the archive metadata.
    pub fn meta_value(&self, object: &str) -> Option<String> {
        [&self.core, &self.archive]
            .into_iter()
            .flatten()
            .find_map(|root| {
                root.walk()
                    .into_iter()
                    .find(|g| g.object && g.name.eq_ignore_ascii_case(object))
                    .and_then(|g| g.get("VALUE"))
                    .map(|v| match v {
                        odl::Val::List(l) if l.len() == 1 => l[0].display(),
                        v => v.display(),
                    })
            })
    }

    /// The product short name.
    pub fn short_name(&self) -> Option<String> {
        self.meta_value("SHORTNAME")
    }

    /// The start of the data, `date time` (as written).
    pub fn begin(&self) -> Option<String> {
        let d = self.meta_value("RANGEBEGINNINGDATE")?;
        Some(match self.meta_value("RANGEBEGINNINGTIME") {
            Some(t) => format!("{d} {t}"),
            None => d,
        })
    }

    /// The end of the data, `date time` (as written).
    pub fn end(&self) -> Option<String> {
        let d = self.meta_value("RANGEENDINGDATE")?;
        Some(match self.meta_value("RANGEENDINGTIME") {
            Some(t) => format!("{d} {t}"),
            None => d,
        })
    }

    /// The grid holding dataset `sds`.
    pub fn grid_of(&self, sds: usize) -> Option<&Grid> {
        self.grids.iter().find(|g| g.field_of(sds).is_some())
    }

    /// The swath holding dataset `sds`.
    pub fn swath_of(&self, sds: usize) -> Option<&Swath> {
        self.swaths.iter().find(|s| s.field_of(sds).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Sds;

    #[test]
    fn packed_dms() {
        assert!((dms_to_deg(-180_000_000.0) + 180.0).abs() < 1e-12);
        assert!((dms_to_deg(45_030_000.0) - 45.5).abs() < 1e-12);
        assert!((dms_to_deg(29_030_000.0) - 29.5).abs() < 1e-12);
        assert!((dms_to_deg(10_000_030.0) - (10.0 + 30.0 / 3600.0)).abs() < 1e-12);
    }

    #[test]
    fn gctp_names() {
        assert_eq!(gctp::code("GCTP_SNSOID"), Some(gctp::SNSOID));
        assert_eq!(gctp::code("GCTP_ISINUS"), Some(gctp::ISINUS));
        assert_eq!(gctp::code("GCTP_NOPE"), None);
        assert_eq!(gctp::name(11), Some("LAMAZ"));
    }

    #[test]
    fn dimension_maps() {
        let m = DimMap {
            geo: "g".into(),
            data: "d".into(),
            offset: 1,
            increment: 2,
        };
        assert_eq!(m.geo_index(1.0), 0.0);
        assert_eq!(m.geo_index(5.0), 2.0);
        let m = DimMap {
            increment: -4,
            offset: 0,
            ..m
        };
        assert_eq!(m.geo_index(2.0), 8.0);
    }

    /// 2,000 swaths of 50 fields over 100,000 datasets: each field scanned every dataset
    /// (10^10 name comparisons); the datasets are now indexed once. The index picks the
    /// same dataset as the scan, including the `:owner` preference.
    #[test]
    fn hostile_many_fields_and_datasets() {
        let (swaths, per) = (2_000usize, 50usize);
        let d = crate::hostile::tiny();
        let mut f = File::open(&d).unwrap();
        f.sds.clear();
        for k in 0..swaths * per {
            // Every name twice, with the owner suffixes of different swaths.
            f.sds.push(Sds::named(
                &format!("f{}", k % (swaths * per / 2)),
                &format!("d:x:sw{}", k % swaths),
            ));
        }
        let mut odl = String::from("GROUP=SwathStructure\n");
        for s in 0..swaths {
            odl += &format!("GROUP=SWATH_{s}\nSwathName=\"sw{s}\"\nGROUP=DataField\n");
            for j in 0..per {
                let n = (s * per + j) % (swaths * per / 2);
                odl += &format!(
                    "OBJECT=DataField_{j}\nDataFieldName=\"f{n}\"\nEND_OBJECT=DataField_{j}\n"
                );
            }
            odl += &format!("END_GROUP=DataField\nEND_GROUP=SWATH_{s}\n");
        }
        odl += "END_GROUP=SwathStructure\nEND\n";
        f.attrs = vec![crate::Attr {
            name: "StructMetadata.0".into(),
            value: crate::Value::Str(vec![odl]),
        }];
        let t = std::time::Instant::now();
        let eos = Eos::from_file(&f).unwrap();
        assert!(
            t.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            t.elapsed()
        );
        assert_eq!(eos.swaths.len(), swaths);
        for sw in eos.swaths.iter().step_by(97) {
            for fl in &sw.data_fields {
                assert_eq!(fl.sds, find_sds(&f, &fl.name, &sw.name), "{}", fl.name);
            }
        }
        // Owner preference: f0 is datasets 0 (dim "…:sw0") and 50,000 (dim "…:sw0").
        assert_eq!(eos.swaths[0].data_fields[0].sds, Some(0));
    }

    #[test]
    fn index_matches_scan_on_odd_suffixes() {
        let d = crate::hostile::tiny();
        let mut f = File::open(&d).unwrap();
        f.sds.clear();
        for (name, dim) in [
            ("a", "x:other"),
            ("a", "x:"),
            ("a", "y:a:b"),
            ("a", "y:b"),
            ("b", "b"),
            ("b", "q:a:b"),
        ] {
            f.sds.push(Sds::named(name, dim));
        }
        let owners: HashSet<&str> = ["", "b", "a:b", "other", "zz"].into_iter().collect();
        let idx = SdsIndex::new(&f, &owners);
        for name in ["a", "b", "c"] {
            for owner in &owners {
                assert_eq!(
                    idx.find(name, owner),
                    find_sds(&f, name, owner),
                    "{name} {owner}"
                );
            }
        }
    }
}
