//! A compact table of the common parameters and level types: WMO GRIB2 (disciplines 0, 2
//! and 10), WMO GRIB1 table 2 and ECMWF's GRIB1 table 128. Short names follow the usual
//! ecCodes ones ("t", "2t", "msl", "tp"); fields outside the table keep a numeric code.

use crate::{Level, Param};

/// (discipline, category, number, short name, name, units).
const GRIB2: &[(u8, u8, u8, &str, &str, &str)] = &[
    (0, 0, 0, "t", "Temperature", "K"),
    (0, 0, 1, "vtmp", "Virtual temperature", "K"),
    (0, 0, 2, "pt", "Potential temperature", "K"),
    (
        0,
        0,
        3,
        "papt",
        "Pseudo-adiabatic potential temperature",
        "K",
    ),
    (0, 0, 4, "tmax", "Maximum temperature", "K"),
    (0, 0, 5, "tmin", "Minimum temperature", "K"),
    (0, 0, 6, "dpt", "Dew point temperature", "K"),
    (0, 0, 7, "depr", "Dew point depression", "K"),
    (0, 0, 10, "lhtfl", "Latent heat net flux", "W m⁻²"),
    (0, 0, 11, "shtfl", "Sensible heat net flux", "W m⁻²"),
    (0, 0, 17, "skt", "Skin temperature", "K"),
    (0, 1, 0, "q", "Specific humidity", "kg kg⁻¹"),
    (0, 1, 1, "r", "Relative humidity", "%"),
    (0, 1, 2, "mixr", "Humidity mixing ratio", "kg kg⁻¹"),
    (0, 1, 3, "pwat", "Precipitable water", "kg m⁻²"),
    (0, 1, 7, "prate", "Precipitation rate", "kg m⁻² s⁻¹"),
    (0, 1, 8, "tp", "Total precipitation", "kg m⁻²"),
    (0, 1, 11, "sde", "Snow depth", "m"),
    (
        0,
        1,
        13,
        "sdwe",
        "Water equivalent of accumulated snow depth",
        "kg m⁻²",
    ),
    (0, 1, 22, "clwmr", "Cloud mixing ratio", "kg kg⁻¹"),
    (0, 1, 52, "tprate", "Total precipitation rate", "kg m⁻² s⁻¹"),
    (
        0,
        1,
        64,
        "tciwv",
        "Total column integrated water vapour",
        "kg m⁻²",
    ),
    (0, 2, 0, "wdir", "Wind direction", "°"),
    (0, 2, 1, "ws", "Wind speed", "m s⁻¹"),
    (0, 2, 2, "u", "U component of wind", "m s⁻¹"),
    (0, 2, 3, "v", "V component of wind", "m s⁻¹"),
    (0, 2, 8, "w", "Vertical velocity", "Pa s⁻¹"),
    (0, 2, 9, "wz", "Geometric vertical velocity", "m s⁻¹"),
    (0, 2, 10, "absv", "Absolute vorticity", "s⁻¹"),
    (0, 2, 12, "vo", "Vorticity (relative)", "s⁻¹"),
    (0, 2, 13, "d", "Divergence", "s⁻¹"),
    (0, 2, 22, "gust", "Wind speed (gust)", "m s⁻¹"),
    (0, 3, 0, "pres", "Pressure", "Pa"),
    (0, 3, 1, "prmsl", "Pressure reduced to MSL", "Pa"),
    (0, 3, 4, "z", "Geopotential", "m² s⁻²"),
    (0, 3, 5, "gh", "Geopotential height", "gpm"),
    (0, 3, 6, "h", "Geometrical height", "m"),
    (0, 3, 18, "blh", "Boundary layer height", "m"),
    (
        0,
        4,
        7,
        "dswrf",
        "Downward short-wave radiation flux",
        "W m⁻²",
    ),
    (
        0,
        5,
        3,
        "dlwrf",
        "Downward long-wave radiation flux",
        "W m⁻²",
    ),
    (0, 5, 5, "olr", "Outgoing long-wave radiation", "W m⁻²"),
    (0, 6, 1, "tcc", "Total cloud cover", "%"),
    (0, 6, 3, "lcc", "Low cloud cover", "%"),
    (0, 6, 4, "mcc", "Medium cloud cover", "%"),
    (0, 6, 5, "hcc", "High cloud cover", "%"),
    (
        0,
        7,
        6,
        "cape",
        "Convective available potential energy",
        "J kg⁻¹",
    ),
    (0, 7, 7, "cin", "Convective inhibition", "J kg⁻¹"),
    (0, 14, 0, "tozne", "Total ozone", "DU"),
    (0, 16, 4, "refc", "Composite reflectivity", "dB"),
    (0, 19, 0, "vis", "Visibility", "m"),
    (2, 0, 0, "lsm", "Land-sea mask", "(0–1)"),
    (2, 0, 7, "mterh", "Model terrain height", "m"),
    (2, 0, 22, "soilw", "Soil moisture", "kg m⁻³"),
    (2, 3, 18, "tsoil", "Soil temperature", "K"),
    (
        10,
        0,
        3,
        "swh",
        "Significant height of combined wind waves and swell",
        "m",
    ),
    (10, 0, 4, "mdww", "Direction of wind waves", "°"),
    (10, 0, 5, "shww", "Significant height of wind waves", "m"),
    (10, 0, 11, "mpww", "Mean period of wind waves", "s"),
    (10, 2, 0, "ci", "Sea ice area fraction", "(0–1)"),
    (10, 3, 0, "wtmp", "Water temperature", "K"),
    (10, 3, 1, "zos", "Sea surface height", "m"),
    (10, 4, 21, "so", "Salinity", "kg kg⁻¹"),
];

/// ECMWF's local GRIB2 parameters (numbers 192 and up are each centre's own).
const GRIB2_ECMWF: &[(u8, u8, u8, &str, &str, &str)] = &[
    (0, 1, 193, "tp", "Total precipitation", "m"),
    (0, 6, 192, "tcc", "Total cloud cover", "(0–1)"),
    (0, 3, 192, "msl", "Mean sea level pressure", "Pa"),
];

/// Short names that depend on the level, as ecCodes gives them: (discipline, category,
/// number, level type, level value, short name, name).
type AtLevel = (u8, u8, u8, u8, Option<f64>, &'static str, &'static str);

const GRIB2_AT_LEVEL: &[AtLevel] = &[
    (0, 0, 0, 103, Some(2.0), "2t", "2 metre temperature"),
    (
        0,
        0,
        6,
        103,
        Some(2.0),
        "2d",
        "2 metre dewpoint temperature",
    ),
    (0, 1, 0, 103, Some(2.0), "2sh", "2 metre specific humidity"),
    (0, 1, 1, 103, Some(2.0), "2r", "2 metre relative humidity"),
    (0, 2, 2, 103, Some(10.0), "10u", "10 metre U wind component"),
    (0, 2, 3, 103, Some(10.0), "10v", "10 metre V wind component"),
    (0, 2, 1, 103, Some(10.0), "10si", "10 metre wind speed"),
    (0, 3, 0, 1, None, "sp", "Surface pressure"),
    (0, 3, 0, 101, None, "msl", "Mean sea level pressure"),
    (0, 0, 0, 1, None, "t", "Temperature"),
    (0, 3, 4, 1, None, "z", "Geopotential"),
    (10, 3, 0, 1, None, "sst", "Sea surface temperature"),
];

/// GRIB1: (table version range, centre (0 for any), number, short name, name, units).
const GRIB1: &[(u8, u8, u16, u8, &str, &str, &str)] = &[
    // WMO table 2 (versions 1 to 3, shared by most centres' tables up to 127).
    (1, 3, 0, 1, "pres", "Pressure", "Pa"),
    (1, 3, 0, 2, "prmsl", "Pressure reduced to MSL", "Pa"),
    (1, 3, 0, 6, "z", "Geopotential", "m² s⁻²"),
    (1, 3, 0, 7, "gh", "Geopotential height", "gpm"),
    (1, 3, 0, 11, "t", "Temperature", "K"),
    (1, 3, 0, 15, "tmax", "Maximum temperature", "K"),
    (1, 3, 0, 16, "tmin", "Minimum temperature", "K"),
    (1, 3, 0, 17, "dpt", "Dew point temperature", "K"),
    (1, 3, 0, 31, "wdir", "Wind direction", "°"),
    (1, 3, 0, 32, "ws", "Wind speed", "m s⁻¹"),
    (1, 3, 0, 33, "u", "U component of wind", "m s⁻¹"),
    (1, 3, 0, 34, "v", "V component of wind", "m s⁻¹"),
    (1, 3, 0, 39, "w", "Vertical velocity", "Pa s⁻¹"),
    (1, 3, 0, 41, "absv", "Absolute vorticity", "s⁻¹"),
    (1, 3, 0, 43, "vo", "Relative vorticity", "s⁻¹"),
    (1, 3, 0, 44, "d", "Relative divergence", "s⁻¹"),
    (1, 3, 0, 51, "q", "Specific humidity", "kg kg⁻¹"),
    (1, 3, 0, 52, "r", "Relative humidity", "%"),
    (1, 3, 0, 54, "pwat", "Precipitable water", "kg m⁻²"),
    (1, 3, 0, 61, "tp", "Total precipitation", "kg m⁻²"),
    (
        1,
        3,
        0,
        65,
        "sdwe",
        "Water equivalent of accumulated snow depth",
        "kg m⁻²",
    ),
    (1, 3, 0, 66, "sd", "Snow depth", "m"),
    (1, 3, 0, 71, "tcc", "Total cloud cover", "%"),
    (1, 3, 0, 80, "wtmp", "Water temperature", "K"),
    (1, 3, 0, 81, "lsm", "Land-sea mask", "(0–1)"),
    (1, 3, 0, 91, "icec", "Ice cover", "(0–1)"),
    // ECMWF table 128.
    (128, 128, 98, 31, "ci", "Sea ice area fraction", "(0–1)"),
    (128, 128, 98, 34, "sst", "Sea surface temperature", "K"),
    (
        128,
        128,
        98,
        59,
        "cape",
        "Convective available potential energy",
        "J kg⁻¹",
    ),
    (128, 128, 98, 129, "z", "Geopotential", "m² s⁻²"),
    (128, 128, 98, 130, "t", "Temperature", "K"),
    (128, 128, 98, 131, "u", "U component of wind", "m s⁻¹"),
    (128, 128, 98, 132, "v", "V component of wind", "m s⁻¹"),
    (128, 128, 98, 133, "q", "Specific humidity", "kg kg⁻¹"),
    (128, 128, 98, 134, "sp", "Surface pressure", "Pa"),
    (128, 128, 98, 135, "w", "Vertical velocity", "Pa s⁻¹"),
    (
        128,
        128,
        98,
        137,
        "tcwv",
        "Total column vertically-integrated water vapour",
        "kg m⁻²",
    ),
    (128, 128, 98, 138, "vo", "Vorticity (relative)", "s⁻¹"),
    (128, 128, 98, 139, "stl1", "Soil temperature level 1", "K"),
    (
        128,
        128,
        98,
        141,
        "sd",
        "Snow depth",
        "m of water equivalent",
    ),
    (128, 128, 98, 151, "msl", "Mean sea level pressure", "Pa"),
    (
        128,
        128,
        98,
        152,
        "lnsp",
        "Logarithm of surface pressure",
        "",
    ),
    (128, 128, 98, 155, "d", "Divergence", "s⁻¹"),
    (128, 128, 98, 156, "gh", "Geopotential height", "gpm"),
    (128, 128, 98, 157, "r", "Relative humidity", "%"),
    (128, 128, 98, 164, "tcc", "Total cloud cover", "(0–1)"),
    (
        128,
        128,
        98,
        165,
        "10u",
        "10 metre U wind component",
        "m s⁻¹",
    ),
    (
        128,
        128,
        98,
        166,
        "10v",
        "10 metre V wind component",
        "m s⁻¹",
    ),
    (128, 128, 98, 167, "2t", "2 metre temperature", "K"),
    (128, 128, 98, 168, "2d", "2 metre dewpoint temperature", "K"),
    (128, 128, 98, 172, "lsm", "Land-sea mask", "(0–1)"),
    (128, 128, 98, 228, "tp", "Total precipitation", "m"),
    (128, 128, 98, 235, "skt", "Skin temperature", "K"),
];

/// The parameter of a GRIB2 field.
pub fn param2(centre: u16, discipline: u8, category: u8, number: u8, level: &Level) -> Param {
    let mut p = Param {
        discipline,
        category,
        number,
        centre: 0,
        short: format!("{discipline}.{category}.{number}"),
        name: String::new(),
        units: String::new(),
    };
    let local = if centre == 98 && number >= 192 {
        GRIB2_ECMWF
    } else {
        GRIB2
    };
    if let Some(e) = local
        .iter()
        .find(|e| (e.0, e.1, e.2) == (discipline, category, number))
    {
        p.short = e.3.to_owned();
        p.name = e.4.to_owned();
        p.units = e.5.to_owned();
    }
    if let Some(e) = GRIB2_AT_LEVEL.iter().find(|e| {
        (e.0, e.1, e.2, e.3) == (discipline, category, number, level.kind)
            && (e.4.is_none() || e.4 == level.value)
    }) {
        p.short = e.5.to_owned();
        p.name = e.6.to_owned();
    }
    p
}

/// The parameter of a GRIB1 field (`table` its parameter table version).
pub fn param1(table: u8, centre: u16, number: u8, level: &Level) -> Param {
    let mut p = Param {
        discipline: 255,
        category: table,
        number,
        centre,
        short: format!("{table}.{number}"),
        name: String::new(),
        units: String::new(),
    };
    let hit = GRIB1
        .iter()
        .find(|e| (e.0..=e.1).contains(&table) && (e.2 == 0 || e.2 == centre) && e.3 == number);
    if let Some(e) = hit {
        p.short = e.4.to_owned();
        p.name = e.5.to_owned();
        p.units = e.6.to_owned();
        // WMO table 2's surface fields with ecCodes' level-specific names.
        if table < 128 {
            let at = |k: u8, v: f64| level.kind == k && level.value == Some(v);
            let special = match number {
                11 if at(105, 2.0) => Some(("2t", "2 metre temperature")),
                17 if at(105, 2.0) => Some(("2d", "2 metre dewpoint temperature")),
                33 if at(105, 10.0) => Some(("10u", "10 metre U wind component")),
                34 if at(105, 10.0) => Some(("10v", "10 metre V wind component")),
                1 if level.kind == 1 => Some(("sp", "Surface pressure")),
                2 if level.kind == 102 => Some(("msl", "Mean sea level pressure")),
                _ => None,
            };
            if let Some((s, n)) = special {
                p.short = s.to_owned();
                p.name = n.to_owned();
            }
        }
    }
    p
}

/// (code, name, unit, factor to `unit`, has a value).
const LEVELS2: &[(u8, &str, &str, f64, bool)] = &[
    (1, "surface", "", 1.0, false),
    (2, "cloudBase", "", 1.0, false),
    (3, "cloudTop", "", 1.0, false),
    (4, "isothermZero", "", 1.0, false),
    (6, "maxWind", "", 1.0, false),
    (7, "tropopause", "", 1.0, false),
    (8, "nominalTop", "", 1.0, false),
    (9, "seaBottom", "", 1.0, false),
    (10, "entireAtmosphere", "", 1.0, false),
    (100, "isobaricInhPa", "hPa", 0.01, true),
    (101, "meanSea", "", 1.0, false),
    (102, "heightAboveSea", "m", 1.0, true),
    (103, "heightAboveGround", "m", 1.0, true),
    (104, "sigma", "", 1.0, true),
    (105, "hybrid", "", 1.0, true),
    (106, "depthBelowLand", "m", 1.0, true),
    (107, "theta", "K", 1.0, true),
    (108, "pressureFromGround", "hPa", 0.01, true),
    (109, "potentialVorticity", "PVU", 1e6, true),
    (111, "eta", "", 1.0, true),
    (114, "snow", "", 1.0, true),
    (150, "generalVertical", "", 1.0, true),
    (151, "soil", "", 1.0, true),
    (160, "depthBelowSea", "m", 1.0, true),
    (200, "entireAtmosphere", "", 1.0, false),
    (201, "entireOcean", "", 1.0, false),
    (220, "planetaryBoundaryLayer", "", 1.0, false),
];

/// A GRIB2 level from the first and second fixed surfaces' values.
pub fn level2(kind: u8, v1: Option<f64>, v2: Option<f64>) -> Level {
    match LEVELS2.iter().find(|e| e.0 == kind) {
        Some(&(_, name, unit, factor, valued)) => Level {
            kind,
            name,
            value: v1.filter(|_| valued).map(|v| clean(v * factor)),
            value2: v2.filter(|_| valued).map(|v| clean(v * factor)),
            unit,
        },
        None => Level {
            kind,
            name: if kind == 255 { "unknown" } else { "level" },
            value: v1.filter(|_| kind != 255),
            value2: None,
            unit: "",
        },
    }
}

/// Removes the decimal noise of scaled values (850.0000001 → 850).
fn clean(v: f64) -> f64 {
    let r = (v * 1e6).round() / 1e6;
    if (r - v).abs() < 1e-9 * v.abs().max(1.0) {
        r
    } else {
        v
    }
}

/// (code, name, unit, factor to `unit`, has a value, is a layer of two octets).
const LEVELS1: &[(u8, &str, &str, f64, bool, bool)] = &[
    (1, "surface", "", 1.0, false, false),
    (2, "cloudBase", "", 1.0, false, false),
    (3, "cloudTop", "", 1.0, false, false),
    (4, "isothermZero", "", 1.0, false, false),
    (6, "maxWind", "", 1.0, false, false),
    (7, "tropopause", "", 1.0, false, false),
    (8, "nominalTop", "", 1.0, false, false),
    (9, "seaBottom", "", 1.0, false, false),
    (100, "isobaricInhPa", "hPa", 1.0, true, false),
    (101, "isobaricLayer", "hPa", 10.0, true, true),
    (102, "meanSea", "", 1.0, false, false),
    (103, "heightAboveSea", "m", 1.0, true, false),
    (104, "heightAboveSeaLayer", "m", 100.0, true, true),
    (105, "heightAboveGround", "m", 1.0, true, false),
    (106, "heightAboveGroundLayer", "m", 100.0, true, true),
    (107, "sigma", "", 1e-4, true, false),
    (108, "sigmaLayer", "", 0.01, true, true),
    (109, "hybrid", "", 1.0, true, false),
    (110, "hybridLayer", "", 1.0, true, true),
    (111, "depthBelowLand", "cm", 1.0, true, false),
    (112, "depthBelowLandLayer", "cm", 1.0, true, true),
    (113, "theta", "K", 1.0, true, false),
    (114, "thetaLayer", "K", 1.0, true, true),
    (116, "pressureFromGroundLayer", "hPa", 1.0, true, true),
    (117, "potentialVorticity", "PVU", 1e-3, true, false),
    (120, "etaLayer", "", 0.01, true, true),
    (121, "isobaricLayer", "hPa", 1.0, true, true),
    (128, "sigmaLayer", "", 0.001, true, true),
    (141, "isobaricLayer", "hPa", 1.0, true, true),
    (160, "depthBelowSea", "m", 1.0, true, false),
    (200, "entireAtmosphere", "", 1.0, false, false),
    (201, "entireOcean", "", 1.0, false, false),
];

/// Whether a GRIB1 level type stores two one-octet values (top, bottom).
pub fn level1_is_layer(kind: u8) -> bool {
    LEVELS1.iter().any(|e| e.0 == kind && e.5)
}

/// A GRIB1 level from octets 11 and 12 (one value, or top and bottom of a layer).
pub fn level1(kind: u8, v1: Option<f64>, v2: Option<f64>) -> Level {
    match LEVELS1.iter().find(|e| e.0 == kind) {
        Some(&(_, name, unit, factor, valued, _)) => Level {
            kind,
            name,
            value: v1.filter(|_| valued).map(|v| clean(v * factor)),
            value2: v2.filter(|_| valued).map(|v| clean(v * factor)),
            unit,
        },
        None => Level {
            kind,
            name: "level",
            value: v1,
            value2: None,
            unit: "",
        },
    }
}

/// The words of a level type: the suffix after a value with a unit, and the name when
/// there is no unit ("surface", "hybrid level").
pub fn level_words(name: &str) -> (&'static str, &'static str) {
    match name {
        "surface" => ("", "surface"),
        "cloudBase" => ("", "cloud base"),
        "cloudTop" => ("", "cloud top"),
        "isothermZero" => ("", "0 °C isotherm"),
        "maxWind" => ("", "maximum wind level"),
        "tropopause" => ("", "tropopause"),
        "nominalTop" => ("", "top of atmosphere"),
        "seaBottom" => ("", "sea bottom"),
        "entireAtmosphere" => ("", "entire atmosphere"),
        "entireOcean" => ("", "entire ocean"),
        "meanSea" => ("", "mean sea level"),
        "planetaryBoundaryLayer" => ("", "boundary layer"),
        "isobaricInhPa" | "isobaricLayer" => ("", "pressure"),
        "heightAboveSea" | "heightAboveSeaLayer" => (" above sea level", "height above sea"),
        "heightAboveGround" | "heightAboveGroundLayer" => (" above ground", "height above ground"),
        "depthBelowLand" | "depthBelowLandLayer" => (" below ground", "soil depth"),
        "depthBelowSea" => (" below sea level", "depth"),
        "pressureFromGround" | "pressureFromGroundLayer" => {
            (" above ground", "pressure difference")
        }
        "theta" | "thetaLayer" => (" isentrope", "isentrope"),
        "potentialVorticity" => ("", "potential vorticity"),
        "sigma" | "sigmaLayer" => ("", "sigma"),
        "hybrid" | "hybridLayer" => ("", "hybrid level"),
        "eta" | "etaLayer" => ("", "eta level"),
        "soil" => ("", "soil level"),
        "snow" => ("", "snow level"),
        "generalVertical" => ("", "model level"),
        "unknown" => ("", "unknown level"),
        _ => ("", "level"),
    }
}

/// Statistical processes (GRIB2 code table 4.10).
pub fn statistic(code: u8) -> Option<&'static str> {
    Some(match code {
        0 => "average",
        1 => "accumulation",
        2 => "maximum",
        3 => "minimum",
        4 => "difference",
        5 => "root mean square",
        6 => "standard deviation",
        7 => "covariance",
        8 => "difference",
        9 => "ratio",
        10 => "standardised anomaly",
        11 => "sum",
        _ => return None,
    })
}
