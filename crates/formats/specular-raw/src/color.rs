//! Camera colour → linear Rec.709: the DNG colour model (ColorMatrix / ForwardMatrix with
//! illuminant interpolation, CameraCalibration, AnalogBalance, AsShotNeutral/WhiteXY), embedded
//! vendor matrices and a generic fallback.

use crate::MatrixSource;
use crate::container::{ColorSource, DngColor};
use crate::profile::{HsvTable, ProfileTransform};

type M3 = [[f64; 3]; 3];

const IDENTITY: M3 = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// Linear sRGB (D65) → XYZ.
const SRGB_TO_XYZ: M3 = [
    [0.4124564, 0.3575761, 0.1804375],
    [0.2126729, 0.7151522, 0.0721750],
    [0.0193339, 0.1191920, 0.9503041],
];

/// XYZ (D50) → linear sRGB (D65), Bradford adaptation.
pub(crate) const XYZD50_TO_SRGB: M3 = [
    [3.1338561, -1.6168667, -0.4906146],
    [-0.9787684, 1.9161415, 0.0334540],
    [0.0719453, -0.2289914, 1.4052427],
];

/// Linear ProPhoto RGB (ROMM, D50) → XYZ (D50).
pub(crate) const PROPHOTO_TO_XYZD50: M3 = [
    [0.7976749, 0.1351917, 0.0313534],
    [0.2880402, 0.7118741, 0.0000857],
    [0.0, 0.0, 0.8252100],
];

/// XYZ -> linear sRGB (D65).
const XYZ_TO_SRGB_D65: M3 = [
    [3.2404542, -1.5371385, -0.4985314],
    [-0.9692660, 1.8760108, 0.0415560],
    [0.0556434, -0.2040259, 1.0572252],
];

/// RGB -> XYZ for primaries and white given as CIE xy.
fn rgb_to_xyz(primaries: [(f64, f64); 3], white: (f64, f64)) -> Option<M3> {
    let cols: [[f64; 3]; 3] = primaries.map(|(x, y)| xy_to_xyz(x, y));
    let p: M3 = std::array::from_fn(|i| std::array::from_fn(|j| cols[j][i]));
    let s = mulv(&inv(&p)?, xy_to_xyz(white.0, white.1));
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| p[i][j] * s[j])
    }))
}

/// Bradford chromatic adaptation between two whites (CIE xy).
fn bradford(from: (f64, f64), to: (f64, f64)) -> Option<M3> {
    const B: M3 = [
        [0.8951, 0.2664, -0.1614],
        [-0.7502, 1.7135, 0.0367],
        [0.0389, -0.0685, 1.0296],
    ];
    let (a, b) = (
        mulv(&B, xy_to_xyz(from.0, from.1)),
        mulv(&B, xy_to_xyz(to.0, to.1)),
    );
    let d = diag(std::array::from_fn(|i| b[i] / a[i]));
    Some(mul(&inv(&B)?, &mul(&d, &B)))
}

/// ACES AP0 (SMPTE ST 2065-1 primaries, ACES white) -> linear sRGB (D65, Bradford).
fn ap0_to_srgb() -> Option<M3> {
    const ACES_WHITE: (f64, f64) = (0.32168, 0.33767);
    let ap0 = rgb_to_xyz(
        [(0.7347, 0.2653), (0.0, 1.0), (0.0001, -0.0770)],
        ACES_WHITE,
    )?;
    let adapt = bradford(ACES_WHITE, (0.3127, 0.3290))?;
    Some(mul(&XYZ_TO_SRGB_D65, &mul(&adapt, &ap0)))
}

/// An average-camera matrix for files with no colour data (white-balanced camera RGB → sRGB).
const GENERIC: [[f32; 3]; 3] = [
    [1.65, -0.55, -0.10],
    [-0.20, 1.55, -0.35],
    [0.02, -0.50, 1.48],
];

fn mul(a: &M3, b: &M3) -> M3 {
    let mut o = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            o[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    o
}

fn mulv(a: &M3, v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| (0..3).map(|k| a[i][k] * v[k]).sum())
}

fn inv(m: &M3) -> Option<M3> {
    let c =
        |r0: usize, r1: usize, c0: usize, c1: usize| m[r0][c0] * m[r1][c1] - m[r0][c1] * m[r1][c0];
    let cof = [
        [c(1, 2, 1, 2), -c(1, 2, 0, 2), c(1, 2, 0, 1)],
        [-c(0, 2, 1, 2), c(0, 2, 0, 2), -c(0, 2, 0, 1)],
        [c(0, 1, 1, 2), -c(0, 1, 0, 2), c(0, 1, 0, 1)],
    ];
    let det: f64 = (0..3).map(|j| m[0][j] * cof[0][j]).sum();
    if !det.is_finite() || det.abs() < 1e-12 {
        return None;
    }
    // inverse = adjugate / det = cofactor^T / det
    Some(std::array::from_fn(|i| {
        std::array::from_fn(|j| cof[j][i] / det)
    }))
}

fn diag(v: [f64; 3]) -> M3 {
    [[v[0], 0.0, 0.0], [0.0, v[1], 0.0], [0.0, 0.0, v[2]]]
}

fn m3(v: &[f64; 9]) -> M3 {
    [[v[0], v[1], v[2]], [v[3], v[4], v[5]], [v[6], v[7], v[8]]]
}

fn lerp(a: &M3, b: &M3, w: f64) -> M3 {
    std::array::from_fn(|i| std::array::from_fn(|j| a[i][j] * w + b[i][j] * (1.0 - w)))
}

/// Normalise rows to sum to 1 (white-preserving).
fn normalize_rows(m: &mut M3) {
    for r in m.iter_mut() {
        let s: f64 = r.iter().sum();
        if s.abs() > 1e-9 {
            for v in r.iter_mut() {
                *v /= s;
            }
        }
    }
}

fn to_f32(m: &M3) -> [[f32; 3]; 3] {
    std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] as f32))
}

/// Correlated colour temperature of an EXIF LightSource value. Fluorescent classes use the
/// middle of their EXIF temperature range.
fn illuminant_temp(code: u32) -> Option<f64> {
    Some(match code {
        17 | 3 => 2856.0,
        24 => 3200.0,
        23 => 5000.0,
        1 | 4 | 9 | 20 | 18 => 5500.0,
        10 | 21 | 19 => 6500.0,
        11 | 22 => 7500.0,
        12 => 6400.0,
        13 => 5050.0,
        2 | 14 => 4150.0,
        15 => 3525.0,
        16 => 2925.0,
        _ => return None,
    })
}

/// Robertson's isotemperature lines (Wyszecki & Stiles): reciprocal megakelvin, CIE 1960 u, v,
/// and the slope of the line.
const ROBERTSON: [[f64; 4]; 31] = [
    [0.0, 0.18006, 0.26352, -0.24341],
    [10.0, 0.18066, 0.26589, -0.25479],
    [20.0, 0.18133, 0.26846, -0.26876],
    [30.0, 0.18208, 0.27119, -0.28539],
    [40.0, 0.18293, 0.27407, -0.30470],
    [50.0, 0.18388, 0.27709, -0.32675],
    [60.0, 0.18494, 0.28021, -0.35156],
    [70.0, 0.18611, 0.28342, -0.37915],
    [80.0, 0.18740, 0.28668, -0.40955],
    [90.0, 0.18880, 0.28997, -0.44278],
    [100.0, 0.19032, 0.29326, -0.47888],
    [125.0, 0.19462, 0.30141, -0.58204],
    [150.0, 0.19962, 0.30921, -0.70471],
    [175.0, 0.20525, 0.31647, -0.84901],
    [200.0, 0.21142, 0.32312, -1.0182],
    [225.0, 0.21807, 0.32909, -1.2168],
    [250.0, 0.22511, 0.33439, -1.4512],
    [275.0, 0.23247, 0.33904, -1.7298],
    [300.0, 0.24010, 0.34308, -2.0637],
    [325.0, 0.24792, 0.34655, -2.4681],
    [350.0, 0.25591, 0.34951, -2.9641],
    [375.0, 0.26400, 0.35200, -3.5814],
    [400.0, 0.27218, 0.35407, -4.3633],
    [425.0, 0.28039, 0.35577, -5.3762],
    [450.0, 0.28863, 0.35714, -6.7262],
    [475.0, 0.29685, 0.35823, -8.5955],
    [500.0, 0.30505, 0.35907, -11.324],
    [525.0, 0.31320, 0.35968, -15.628],
    [550.0, 0.32129, 0.36011, -23.325],
    [575.0, 0.32931, 0.36038, -40.770],
    [600.0, 0.33724, 0.36051, -116.45],
];

/// Correlated colour temperature of a chromaticity by Robertson's method.
fn xy_to_cct(x: f64, y: f64) -> f64 {
    let d = -2.0 * x + 12.0 * y + 3.0;
    if !(d.is_finite() && d.abs() > 1e-9) {
        return 5000.0;
    }
    let (u, v) = (4.0 * x / d, 6.0 * y / d);
    // Signed distance to each isotemperature line; the temperature lies where it changes sign.
    let dist = |r: &[f64; 4]| ((v - r[2]) - r[3] * (u - r[1])) / (1.0 + r[3] * r[3]).sqrt();
    let mut prev = dist(&ROBERTSON[0]);
    let mut mired = ROBERTSON[30][0];
    for i in 1..ROBERTSON.len() {
        let di = dist(&ROBERTSON[i]);
        if di <= 0.0 || i == ROBERTSON.len() - 1 {
            let f = if prev - di != 0.0 {
                (prev / (prev - di)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let (a, b) = (ROBERTSON[i - 1][0], ROBERTSON[i][0]);
            mired = a + (b - a) * f;
            break;
        }
        prev = di;
    }
    if mired.is_finite() && mired > 0.0 {
        (1e6 / mired).clamp(1500.0, 50000.0)
    } else {
        50000.0
    }
}

/// Weight of calibration 1 for temperature `t` (interpolation in inverse temperature).
fn weight(c: &DngColor, t: f64) -> f64 {
    let (Some(t1), Some(t2)) = (
        illuminant_temp(c.illuminant[0]),
        illuminant_temp(c.illuminant[1]),
    ) else {
        return if c.cm[0].is_some() { 1.0 } else { 0.0 };
    };
    if c.cm[0].is_none() {
        return 0.0;
    }
    if c.cm[1].is_none() || (t1 - t2).abs() < 1.0 {
        return 1.0;
    }
    let (t1, t2, swap) = if t1 < t2 {
        (t1, t2, false)
    } else {
        (t2, t1, true)
    };
    let w = if t <= t1 {
        1.0
    } else if t >= t2 {
        0.0
    } else {
        (1.0 / t - 1.0 / t2) / (1.0 / t1 - 1.0 / t2)
    };
    if swap { 1.0 - w } else { w }
}

fn pick(pair: &[Option<[f64; 9]>; 2], w: f64) -> Option<M3> {
    match (pair[0].as_ref().map(m3), pair[1].as_ref().map(m3)) {
        (Some(a), Some(b)) => Some(lerp(&a, &b, w)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// AnalogBalance x CameraCalibration at weight `w`.
fn abcc(c: &DngColor, w: f64) -> M3 {
    let cc = pick(&c.cc, w).unwrap_or(IDENTITY);
    let ab = c.ab.map(diag).unwrap_or(IDENTITY);
    mul(&ab, &cc)
}

/// XYZ → camera at weight `w`.
fn xyz_to_cam(c: &DngColor, w: f64) -> Option<M3> {
    // Each ColorMatrix is first scaled so that D50 white peaks at 1 in camera space, which
    // matters when the two calibrations differ in scale and are interpolated.
    let cm = c.cm.map(|m| {
        m.map(|m| {
            let peak = mulv(&m3(&m), D50_XYZ).into_iter().fold(f64::MIN, f64::max);
            if peak > 0.0 && !(0.99..=1.01).contains(&peak) {
                m.map(|v| v / peak)
            } else {
                m
            }
        })
    });
    Some(mul(&abcc(c, w), &pick(&cm, w)?))
}

/// D50 white (XYZ, Y = 1).
const D50_XYZ: [f64; 3] = [0.9642, 1.0, 0.8249];

fn xy_to_xyz(x: f64, y: f64) -> [f64; 3] {
    [x / y, 1.0, (1.0 - x - y) / y]
}

/// Colour transform of a DNG profile: (matrix, its source, white balance, profile with look
/// tables). Profiles that need their tables (Adobe's) but whose tables are missing or malformed
/// fall back to the ColorMatrix: their ForwardMatrix alone renders flat colour.
#[allow(clippy::type_complexity)]
pub(crate) fn dng(
    c: &DngColor,
) -> Option<(
    [[f32; 3]; 3],
    MatrixSource,
    [f32; 3],
    Option<ProfileTransform>,
)> {
    // Camera neutral.
    let neutral = match (c.neutral, c.white_xy) {
        (Some(n), _) => n,
        (None, xy) => {
            let (x, y) = xy.unwrap_or((0.3127, 0.3290));
            let m = xyz_to_cam(c, weight(c, xy_to_cct(x, y)))?;
            mulv(&m, xy_to_xyz(x, y))
        }
    };
    if neutral.iter().any(|&v| v.is_nan() || v <= 0.0) {
        return None;
    }
    // Iterate to the white's temperature.
    let mut w = weight(c, xy_to_cct(0.3457, 0.3585));
    for _ in 0..16 {
        let m = inv(&xyz_to_cam(c, w)?)?;
        let xyz = mulv(&m, neutral);
        let s: f64 = xyz.iter().sum();
        if s.is_nan() || s <= 0.0 {
            break;
        }
        let (nx, ny) = (xyz[0] / s, xyz[1] / s);
        if ny.is_nan() || ny <= 0.0 {
            break;
        }
        let nw = weight(c, xy_to_cct(nx, ny));
        let done = (nw - w).abs() < 1e-6;
        w = nw;
        if done {
            break;
        }
    }
    let n = neutral;
    let wb = [(n[1] / n[0]) as f32, 1.0, (n[1] / n[2]) as f32];
    let unwb = diag([n[0] / n[1], 1.0, n[2] / n[1]]);

    let tables = profile_tables(c, w);
    let use_fm = tables.is_some() || !(c.needs_tables || c.tables_malformed);
    // Camera (white-balanced) → XYZ D50 from the ForwardMatrix.
    let fm_xyz = pick(&c.fm, w).filter(|_| use_fm).and_then(|fm| {
        let aci = inv(&abcc(c, w))?;
        let refn = mulv(&aci, n);
        refn.iter().all(|&v| v > 0.0).then(|| {
            let d = diag([1.0 / refn[0], 1.0 / refn[1], 1.0 / refn[2]]);
            mul(&mul(&mul(&fm, &d), &aci), &unwb)
        })
    });
    let (rgb_cam, src) = match &fm_xyz {
        Some(xyz_cam) => {
            let mut rgb_cam = mul(&XYZD50_TO_SRGB, xyz_cam);
            normalize_rows(&mut rgb_cam);
            (rgb_cam, MatrixSource::DngForwardMatrix)
        }
        None => {
            let mut cam_rgb = mul(&xyz_to_cam(c, w)?, &SRGB_TO_XYZ);
            normalize_rows(&mut cam_rgb);
            (inv(&cam_rgb)?, MatrixSource::DngColorMatrix)
        }
    };
    // The tone curve alone also makes a profile (without tables to rely on the ForwardMatrix).
    let tone = c.tone_curve.clone().filter(|_| !c.hdr);
    let tables = tables.or_else(|| tone.is_some().then_some((None, None)));
    let profile = tables.and_then(|(hsm, look)| {
        let xyz_pro = inv(&PROPHOTO_TO_XYZD50)?;
        // Without a ForwardMatrix: the ColorMatrix result (D65 Rec.709) adapted to D50.
        let xyz_cam = match &fm_xyz {
            Some(m) => *m,
            None => mul(&inv(&XYZD50_TO_SRGB)?, &rgb_cam),
        };
        let mut pro_cam = mul(&xyz_pro, &xyz_cam);
        normalize_rows(&mut pro_cam);
        let mut rgb_pro = mul(&XYZD50_TO_SRGB, &PROPHOTO_TO_XYZD50);
        normalize_rows(&mut rgb_pro);
        Some(ProfileTransform {
            pro_cam: to_f32(&pro_cam),
            rgb_pro: to_f32(&rgb_pro),
            hue_sat_map: hsm,
            look_table: look,
            tone_curve: tone,
            // The viewer's gain: 2^exposure, exposure clamped to +-4 EV.
            gain: 2f32.powf(c.exposure.clamp(-4.0, 4.0) as f32),
            hdr: c.hdr,
        })
    });
    let src = if profile.is_some() {
        MatrixSource::DngProfile
    } else {
        src
    };
    Some((to_f32(&rgb_cam), src, wb, profile))
}

/// The profile's HueSatMap (interpolated at weight `w`) and LookTable, or `None` when there are
/// none or one is malformed.
fn profile_tables(c: &DngColor, w: f64) -> Option<(Option<HsvTable>, Option<HsvTable>)> {
    if c.tables_malformed {
        return None;
    }
    let hsm = match &c.hue_sat_map {
        [Some(a), Some(b)] => Some(HsvTable::lerp(a, b, w)?),
        [Some(a), None] | [None, Some(a)] => Some(a.clone()),
        [None, None] => None,
    };
    (hsm.is_some() || c.look_table.is_some()).then(|| (hsm, c.look_table.clone()))
}

/// Normalised "Make Model" as used by the Adobe coefficient table (dcraw conventions): the make
/// is reduced to the vendor name and not repeated in the model.
pub fn camera_key(make: &str, model: &str) -> String {
    const CORP: [(&str, &str); 28] = [
        ("agfaphoto", "AgfaPhoto"),
        ("canon", "Canon"),
        ("casio", "Casio"),
        ("epson", "Epson"),
        ("fujifilm", "Fujifilm"),
        ("mamiya", "Mamiya"),
        ("minolta", "Minolta"),
        ("motorola", "Motorola"),
        ("kodak", "Kodak"),
        ("konica", "Konica"),
        ("leica", "Leica"),
        ("nikon", "Nikon"),
        ("nokia", "Nokia"),
        ("olympus", "Olympus"),
        ("om digital", "Olympus"),
        ("ricoh", "Ricoh"),
        ("pentax", "Pentax"),
        ("asahi", "Pentax"),
        ("phase one", "Phase One"),
        ("samsung", "Samsung"),
        ("sigma", "Sigma"),
        ("sinar", "Sinar"),
        ("sony", "Sony"),
        ("panasonic", "Panasonic"),
        ("hasselblad", "Hasselblad"),
        ("gopro", "GoPro"),
        ("apple", "Apple"),
        ("yi technology", "YI"),
    ];
    let lm = make.trim().to_ascii_lowercase();
    let mut mk = CORP
        .iter()
        .find(|(k, _)| lm.contains(k))
        .map_or_else(|| make.trim().to_string(), |(_, v)| v.to_string());
    let mut md = model.trim();
    // Ricoh-made Pentax bodies: "RICOH IMAGING COMPANY, LTD." + "PENTAX K-3 II".
    if mk == "Ricoh" && md.to_ascii_lowercase().starts_with("pentax") {
        mk = "Pentax".into();
    }
    for p in [mk.as_str(), make.split_whitespace().next().unwrap_or("")] {
        if !p.is_empty()
            && md.len() > p.len()
            && md.as_bytes()[..p.len()].eq_ignore_ascii_case(p.as_bytes())
            && md.as_bytes()[p.len()] == b' '
        {
            md = md[p.len()..].trim_start();
        }
    }
    // "FinePix E900" is "E900" in the table.
    if mk == "Fujifilm" {
        md = md.strip_prefix("FinePix ").unwrap_or(md);
    }
    // US names of Canon bodies (the table uses the international ones).
    const REBEL: [(&str, &str); 22] = [
        ("EOS DIGITAL REBEL XTi", "EOS 400D"),
        ("EOS DIGITAL REBEL XT", "EOS 350D"),
        ("EOS DIGITAL REBEL", "EOS 300D"),
        ("EOS REBEL XSi", "EOS 450D"),
        ("EOS REBEL XS", "EOS 1000D"),
        ("EOS REBEL T1i", "EOS 500D"),
        ("EOS REBEL T2i", "EOS 550D"),
        ("EOS REBEL T3i", "EOS 600D"),
        ("EOS REBEL T3", "EOS 1100D"),
        ("EOS REBEL T4i", "EOS 650D"),
        ("EOS REBEL T5i", "EOS 700D"),
        ("EOS REBEL T5", "EOS 1200D"),
        ("EOS REBEL T6i", "EOS 750D"),
        ("EOS REBEL T6s", "EOS 760D"),
        ("EOS REBEL T6", "EOS 1300D"),
        ("EOS REBEL T7i", "EOS 800D"),
        ("EOS REBEL T7", "EOS 2000D"),
        ("EOS REBEL T100", "EOS 4000D"),
        ("EOS REBEL SL1", "EOS 100D"),
        ("EOS REBEL SL2", "EOS 200D"),
        ("EOS REBEL SL3", "EOS 250D"),
        ("EOS REBEL T8i", "EOS 850D"),
    ];
    if mk == "Canon"
        && let Some((_, intl)) = REBEL.iter().find(|(us, _)| md.eq_ignore_ascii_case(us))
    {
        md = intl;
    }
    format!("{mk} {md}")
}

/// Whether table prefix `prefix` names the camera `key`: the whole model name, or a prefix ending
/// at a word boundary. Unlike dcraw, a prefix does not cover later bodies that merely extend the
/// name ("ILCE-7" vs "ILCE-7RM5", "X100" vs "X100V", "K-3" vs "K-3 Mark III") or add a
/// generation anywhere after the prefix ("EOS-1D" vs "EOS-1D X Mark III", "PowerShot G1" vs
/// "PowerShot G1 X Mark II"): those have different sensors.
fn prefix_matches(key: &str, prefix: &str) -> bool {
    let Some(rest) = key.strip_prefix(prefix) else {
        return false;
    };
    if rest.is_empty() {
        return true;
    }
    rest.starts_with(' ')
        && !rest
            .split_whitespace()
            .any(|w| matches!(w, "Mark" | "II" | "III" | "IV" | "V" | "VI" | "VII"))
}

/// The Adobe DNG Converter entry for a camera: the first entry (in table order, as in dcraw)
/// whose prefix matches (see [`prefix_matches`]).
pub(crate) fn adobe_entry(make: &str, model: &str) -> Option<&'static crate::adobe_coeff::Entry> {
    let key = camera_key(make, model);
    crate::adobe_coeff::TABLE
        .iter()
        .find(|e| prefix_matches(&key, e.prefix))
}

/// The entry of the published third-party table (`extra_coeff.rs`) for a body the Adobe table
/// lacks.
pub(crate) fn extra_entry(make: &str, model: &str) -> Option<&'static crate::adobe_coeff::Entry> {
    let key = camera_key(make, model);
    crate::extra_coeff::TABLE
        .iter()
        .find(|e| prefix_matches(&key, e.prefix))
}

/// The four-colour matrix of a CMYG or RGBE body.
pub(crate) fn four_color_entry(
    make: &str,
    model: &str,
) -> Option<&'static crate::extra_coeff::Four> {
    let key = camera_key(make, model);
    crate::extra_coeff::FOUR
        .iter()
        .find(|e| prefix_matches(&key, e.prefix))
}

/// White-balanced camera RGB -> linear sRGB from a camera -> ROMM (linear ProPhoto RGB, D50)
/// matrix such as Phase One's (rows normalised to keep white).
pub(crate) fn romm_rgb_cam(romm_cam: &[f32; 9]) -> Option<[[f32; 3]; 3]> {
    let m = m3(&std::array::from_fn(|i| romm_cam[i] as f64));
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let mut rgb = mul(&mul(&XYZD50_TO_SRGB, &PROPHOTO_TO_XYZD50), &m);
    normalize_rows(&mut rgb);
    rgb.iter()
        .all(|r| (r.iter().sum::<f64>() - 1.0).abs() < 1e-6)
        .then(|| to_f32(&rgb))
}

/// White-balanced ARRI ALEXA sensor RGB -> linear sRGB at colour temperature `cct` (K), from
/// ARRI's ACES transforms (interpolated in reciprocal temperature between the nearest two).
pub(crate) fn alexa_rgb_cam(cct: f64) -> Option<[[f32; 3]; 3]> {
    let t = crate::extra_coeff::ALEXA_TO_AP0;
    let (lo, hi) = (t.first()?, t.last()?);
    let cct = cct.clamp(lo.0 as f64, hi.0 as f64);
    let k = t.iter().rposition(|e| e.0 as f64 <= cct)?;
    let (a, b) = (t[k], t[(k + 1).min(t.len() - 1)]);
    let w = if b.0 == a.0 {
        0.0
    } else {
        (1.0 / cct - 1.0 / a.0 as f64) / (1.0 / b.0 as f64 - 1.0 / a.0 as f64)
    };
    let ap0 = lerp(&m3(&a.1), &m3(&b.1), w);
    let mut rgb = mul(&ap0_to_srgb()?, &ap0);
    normalize_rows(&mut rgb);
    Some(to_f32(&rgb))
}

/// The built-in DNG colour profile for a camera, if a reference DNG of that body provided one.
pub(crate) fn camera_profile(
    make: &str,
    model: &str,
) -> Option<&'static crate::adobe_coeff::Profile> {
    let key = camera_key(make, model);
    crate::adobe_coeff::PROFILES
        .iter()
        .find(|p| prefix_matches(&key, p.prefix))
}

/// White-balanced camera RGB -> linear sRGB from a built-in profile, interpolated for the
/// shot's white balance (`wb`: camera multipliers, green = 1) as a DNG reader would with
/// AsShotNeutral. Without a white balance the profile is evaluated for D65.
pub(crate) fn profile_rgb_cam(
    p: &crate::adobe_coeff::Profile,
    wb: Option<[f32; 3]>,
) -> Option<[[f32; 3]; 3]> {
    let neutral = wb
        .filter(|w| w.iter().all(|&v| v.is_finite() && v > 0.0))
        .map(|w| [1.0 / w[0] as f64, 1.0, 1.0 / w[2] as f64]);
    let c = DngColor {
        cm: p.cm,
        fm: p.fm,
        illuminant: p.illuminant,
        neutral,
        ..Default::default()
    };
    dng(&c).map(|(m, ..)| m)
}

/// The as-shot illuminant (CIE 1931 xy) of a non-DNG raw from its as-shot multipliers `wb`
/// (R, G, B; G = 1): the camera's neutral `1/wb` through the body's Adobe (or published)
/// ColorMatrix (XYZ → camera, D65 reference) inverted. `None` for bodies without a table
/// matrix or multipliers that give no valid chromaticity.
pub fn as_shot_xy(make: &str, model: &str, wb: [f32; 3]) -> Option<[f64; 2]> {
    let e = adobe_entry(make, model).or_else(|| extra_entry(make, model))?;
    let m = e.matrix.as_ref()?;
    if wb.iter().any(|v| !(v.is_finite() && *v > 0.0)) {
        return None;
    }
    let cam_xyz = m3(&std::array::from_fn(|i| f64::from(m[i]) / 10000.0));
    let neutral = wb.map(|v| 1.0 / f64::from(v));
    let xyz = mulv(&inv(&cam_xyz)?, neutral);
    let sum: f64 = xyz.iter().sum();
    if !(sum.is_finite() && sum > 0.0) || xyz.iter().any(|v| *v < 0.0) {
        return None;
    }
    let xy = [xyz[0] / sum, xyz[1] / sum];
    (xy[0] > 0.2 && xy[0] < 0.6 && xy[1] > 0.2 && xy[1] < 0.5).then_some(xy)
}

/// White-balanced camera RGB -> linear sRGB from an Adobe ColorMatrix (XYZ -> camera, x10000):
/// camera <- sRGB with rows normalised to keep white, inverted.
/// The matrix from white-balanced camera RGB to linear sRGB of a camera → XYZ (D65) matrix
/// that takes white-balanced camera white to D65 (ProRes RAW frames), white kept.
pub(crate) fn xyz_rgb_cam(cam_to_xyz: &[[f32; 3]; 3]) -> Option<[[f32; 3]; 3]> {
    let m: M3 = std::array::from_fn(|r| std::array::from_fn(|c| f64::from(cam_to_xyz[r][c])));
    if m.iter().flatten().any(|v| !v.is_finite()) {
        return None;
    }
    let mut rgb = mul(&XYZ_TO_SRGB_D65, &m);
    for row in &rgb {
        let s: f64 = row.iter().sum();
        if !s.is_finite() || s.abs() < 1e-6 {
            return None;
        }
    }
    normalize_rows(&mut rgb);
    Some(to_f32(&rgb))
}

pub(crate) fn adobe_rgb_cam(m: &[i16; 9]) -> Option<[[f32; 3]; 3]> {
    let cam_xyz = m3(&std::array::from_fn(|i| m[i] as f64 / 10000.0));
    let mut cam_rgb = mul(&cam_xyz, &SRGB_TO_XYZ);
    normalize_rows(&mut cam_rgb);
    inv(&cam_rgb).map(|m| to_f32(&m))
}

/// Colour data for files without DNG colour tags: a built-in DNG profile of the body first, then
/// the Adobe table (the body's own matrix, or a same-sensor sibling's), then whatever the
/// container found (embedded matrix or the generic one). Also fills black / white levels the
/// table carries when the container has none.
pub(crate) fn apply_adobe(make: &str, model: &str, s: &mut crate::container::Sensor) {
    if matches!(s.color, ColorSource::Dng(_) | ColorSource::Identity) {
        return;
    }
    let mut profiled = false;
    if matches!(s.cpp, 1 | 3)
        && let Some(p) = camera_profile(make, model)
        && let Some(rgb_cam) = profile_rgb_cam(p, s.wb)
    {
        let src = if p.adobe {
            MatrixSource::Adobe
        } else {
            MatrixSource::CameraProfile
        };
        s.color = ColorSource::Matrix(rgb_cam, src);
        profiled = true;
    }
    let (e, published) = match adobe_entry(make, model) {
        Some(e) => (e, false),
        None => match extra_entry(make, model) {
            Some(e) => (e, true),
            None => return,
        },
    };
    if let Some(m) = &e.matrix
        && matches!(s.cpp, 1 | 3)
        && !profiled
        && let Some(rgb_cam) = adobe_rgb_cam(m)
    {
        let src = if published {
            MatrixSource::Published
        } else if e.sibling_of.is_empty() {
            MatrixSource::Adobe
        } else {
            MatrixSource::AdobeSibling
        };
        s.color = ColorSource::Matrix(rgb_cam, src);
    }
    if e.black > 0
        && let crate::container::Black::Levels(b) = &mut s.black
    {
        if b.iter().all(|&v| v == 0.0) {
            *b = [e.black as f32; 4];
        } else if matches!(s.storage, crate::container::Storage::Panasonic { .. }) {
            // RW2 formats up to 4 keep the +15 offset in the samples (the format 5/6 decoder
            // removes it itself), and the per-channel black tags do not include it: the
            // table's level is that offset, on top of the tags.
            for v in b.iter_mut() {
                *v += e.black as f32;
            }
        }
    }
    if e.maximum > 0
        && s.white.is_none()
        && (e.maximum as u64) < (1u64 << s.bits.min(16))
        && !TABLE_WHITE_TOO_LOW.contains(&e.prefix)
    {
        s.white = Some(e.maximum as f32);
    }
}

/// Bodies whose dcraw table maximum sits below where their data clips: the white stays the
/// sensor's full scale (and the develop lowers it to a clip the data shows).
///
/// - EOS 1300D (Rebel T6), table 13584 (LibRaw 14274): raw.pixls.us `IMG_2422.CR2` (ISO 200)
///   runs evenly past both (7 075 green samples above 14274, about 280 per 200 codes all the
///   way up) and piles up only at 16383 (33 119 samples).
const TABLE_WHITE_TOO_LOW: &[&str] = &["Canon EOS 1300D"];

/// Matrix applied to white-balanced camera RGB, its source, the white balance to use, and a DNG
/// profile with look tables (applied instead of the matrix when present).
#[allow(clippy::type_complexity)]
pub(crate) fn camera_matrix(
    src: &ColorSource,
    cpp: u32,
    wb: Option<[f32; 3]>,
) -> (
    [[f32; 3]; 3],
    MatrixSource,
    Option<[f32; 3]>,
    Option<ProfileTransform>,
) {
    let identity = to_f32(&IDENTITY);
    match src {
        ColorSource::Dng(c) => match dng(c) {
            Some((m, s, w, p)) => (m, s, Some(w), p),
            None if cpp == 3 => (
                identity,
                MatrixSource::Identity,
                wb.or(Some([1.0; 3])),
                None,
            ),
            None => (GENERIC, MatrixSource::Generic, wb, None),
        },
        ColorSource::Matrix(m, s) => (*m, *s, wb, None),
        ColorSource::Generic => (GENERIC, MatrixSource::Generic, wb, None),
        ColorSource::Identity => (
            identity,
            MatrixSource::Identity,
            wb.or(Some([1.0; 3])),
            None,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_table_fills_gaps() {
        let e = extra_entry("Leica", "D-Lux 7").unwrap();
        assert_eq!((e.sibling_of, e.black), ("Panasonic DC-LX100M2", 15));
        assert!(extra_entry("Phase One", "IQ3 100MP").is_some());
        assert!(adobe_entry("Phase One", "IQ3 100MP").is_none());
        assert!(four_color_entry("Canon", "Canon PowerShot Pro90 IS").is_some());
        assert!(four_color_entry("Canon", "Canon EOS D30").is_none());
    }

    #[test]
    fn robertson_temperature_of_standard_whites() {
        for (x, y, t) in [
            (0.31271, 0.32902, 6504.0),
            (0.44757, 0.40745, 2856.0),
            (0.34567, 0.35850, 5003.0),
        ] {
            let got = xy_to_cct(x, y);
            assert!((got - t).abs() < 0.005 * t, "{x},{y}: {got}");
        }
        assert!(xy_to_cct(0.2, 0.2).is_finite() && xy_to_cct(0.7, 0.3).is_finite());
        assert!(xy_to_cct(f64::NAN, 0.3).is_finite());
    }

    #[test]
    fn primaries_give_the_standard_matrices() {
        let srgb =
            rgb_to_xyz([(0.64, 0.33), (0.30, 0.60), (0.15, 0.06)], (0.3127, 0.3290)).unwrap();
        for i in 0..3 {
            for j in 0..3 {
                assert!((srgb[i][j] - SRGB_TO_XYZ[i][j]).abs() < 5e-4, "{srgb:?}");
            }
        }
        // AP0 white maps to sRGB white; AP0 is wider than sRGB (its red needs negative G, B).
        let m = ap0_to_srgb().unwrap();
        let w = mulv(&m, [1.0, 1.0, 1.0]);
        assert!(w.iter().all(|v| (v - 1.0).abs() < 1e-3), "{w:?}");
        assert!(m[0][0] > 2.0 && m[1][0] < 0.0 && m[2][0] < 0.0, "{m:?}");
        // ALEXA at 5600 K: a white-preserving matrix with the usual dominant diagonal.
        let a = alexa_rgb_cam(5600.0).unwrap();
        for (r, row) in a.iter().enumerate() {
            assert!((row.iter().sum::<f32>() - 1.0).abs() < 1e-4);
            assert!(row[r] > 1.0, "{a:?}");
        }
        assert_eq!(alexa_rgb_cam(1000.0), alexa_rgb_cam(2000.0));
    }

    #[test]
    fn romm_identity_is_prophoto() {
        let m = romm_rgb_cam(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]).unwrap();
        // ProPhoto green is far outside sRGB: strong negative red and blue in its column.
        assert!(m[1][1] > 1.0 && m[0][1] < -0.3, "{m:?}");
        assert!(romm_rgb_cam(&[f32::NAN; 9]).is_none());
    }

    #[test]
    fn inverse_roundtrip() {
        let m = [[2.0, 1.0, 0.5], [0.1, 3.0, 0.2], [0.3, 0.4, 1.5]];
        let p = mul(&m, &inv(&m).unwrap());
        for i in 0..3 {
            for j in 0..3 {
                assert!((p[i][j] - IDENTITY[i][j]).abs() < 1e-9);
            }
        }
        assert!(inv(&[[1.0, 2.0, 3.0], [2.0, 4.0, 6.0], [0.0, 0.0, 1.0]]).is_none());
    }

    #[test]
    fn bradford_maps_d50_white_to_rgb_white() {
        let w = mulv(&XYZD50_TO_SRGB, [0.9642, 1.0, 0.8249]);
        for v in w {
            assert!((v - 1.0).abs() < 2e-3, "{w:?}");
        }
    }

    #[test]
    fn xyz_colour_matrix_gives_srgb() {
        // A "camera" whose space is XYZ: ColorMatrix = identity. Neutral = D65 white.
        let id = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0];
        let c = DngColor {
            cm: [Some(id), None],
            illuminant: [21, 0],
            neutral: Some(xy_to_xyz(0.3127, 0.3290)),
            ..Default::default()
        };
        let (m, src, wb, _) = dng(&c).unwrap();
        assert_eq!(src, MatrixSource::DngColorMatrix);
        // Feeding the WB'd XYZ of pure sRGB red must give roughly (1, 0, 0).
        let red_xyz = [0.4124564, 0.2126729, 0.0193339];
        let cam = [
            red_xyz[0] * wb[0] as f64,
            red_xyz[1],
            red_xyz[2] * wb[2] as f64,
        ];
        let m64: M3 = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] as f64));
        let rgb = mulv(&m64, cam);
        let s = rgb[0];
        assert!(
            s > 0.5 && rgb[1].abs() / s < 0.02 && rgb[2].abs() / s < 0.02,
            "{rgb:?}"
        );
    }

    /// ColorMatrix1/2 of the reference DNGs as printed by ExifTool 12 (`exiftool -n`), an
    /// extractor independent of the generator.
    #[test]
    fn profiles_match_exiftool_readout() {
        let cases: [(&str, [[f64; 9]; 2]); 2] = [
            (
                "Pentax K-3 Mark III",
                [
                    [
                        0.7637023926,
                        -0.3138122559,
                        -0.03483581543,
                        -0.3809967041,
                        0.9970397949,
                        0.4467773438,
                        -0.01083374023,
                        0.03407287598,
                        0.6953735352,
                    ],
                    [
                        0.7021942139,
                        -0.1622161865,
                        -0.08895874023,
                        -0.4613952637,
                        1.272781372,
                        0.2065429688,
                        -0.06399536133,
                        0.1428833008,
                        0.5685577393,
                    ],
                ],
            ),
            (
                "Pentax K-1 Mark II",
                [
                    [
                        1.129867554,
                        -0.5832824707,
                        -0.1215820312,
                        -0.3411560059,
                        1.01663208,
                        0.3785552979,
                        -0.01783752441,
                        0.04113769531,
                        0.7324676514,
                    ],
                    [
                        0.9062957764,
                        -0.2904815674,
                        -0.1271209717,
                        -0.3611755371,
                        1.220397949,
                        0.1549835205,
                        -0.08999633789,
                        0.1692962646,
                        0.6311187744,
                    ],
                ],
            ),
        ];
        for (name, cm) in cases {
            let p = crate::adobe_coeff::PROFILES
                .iter()
                .find(|p| p.prefix == name)
                .unwrap();
            // StdA and D65.
            assert_eq!(p.illuminant, [17, 21], "{name}");
            assert_eq!(p.fm, [None, None], "{name}");
            for (k, want) in cm.iter().enumerate() {
                let got = p.cm[k].unwrap();
                for (i, (g, w)) in got.iter().zip(want).enumerate() {
                    assert!((g - w).abs() < 1e-9, "{name} cm{k}[{i}]");
                }
            }
        }
    }

    #[test]
    fn profile_lookup_and_white() {
        let make = "RICOH IMAGING COMPANY, LTD.";
        let p = camera_profile(make, "PENTAX K-3 Mark III").unwrap();
        assert_eq!(p.prefix, "Pentax K-3 Mark III");
        assert!(camera_profile(make, "PENTAX K-3 Mark II").is_none());
        assert!(camera_profile("PENTAX Corporation", "PENTAX K-1").is_none());
        for wb in [None, Some([2.46, 1.0, 1.69]), Some([1.4, 1.0, 2.6])] {
            let m = profile_rgb_cam(p, wb).unwrap();
            for row in m {
                let sum: f32 = row.iter().sum();
                assert!((sum - 1.0).abs() < 1e-4, "{wb:?}: {m:?}");
            }
        }
        // Tungsten white balance moves the interpolation towards the StdA matrix.
        assert_ne!(
            profile_rgb_cam(p, Some([1.4, 1.0, 2.6])),
            profile_rgb_cam(p, Some([2.46, 1.0, 1.69]))
        );
    }

    #[test]
    fn table_whites_overridden_by_data_are_table_bodies() {
        for p in TABLE_WHITE_TOO_LOW {
            let e = crate::adobe_coeff::TABLE.iter().find(|e| e.prefix == *p);
            assert!(e.is_some_and(|e| e.maximum > 0), "{p}");
        }
        // The Rebel T6 is the 1300D.
        let e = adobe_entry("Canon", "Canon EOS Rebel T6").unwrap();
        assert!(TABLE_WHITE_TOO_LOW.contains(&e.prefix));
    }

    #[test]
    fn siblings_reuse_the_named_matrix() {
        for e in crate::adobe_coeff::TABLE
            .iter()
            .filter(|e| !e.sibling_of.is_empty())
        {
            let src = crate::adobe_coeff::TABLE
                .iter()
                .find(|s| s.prefix == e.sibling_of && s.sibling_of.is_empty())
                .unwrap();
            assert_eq!(e.matrix, src.matrix, "{}", e.prefix);
            assert_eq!((e.black, e.maximum), (0, 0), "{}", e.prefix);
        }
        // Siblings that now have Adobe's own profile for the body: the substitute is dropped.
        for (make, model, key) in [
            ("NIKON CORPORATION", "NIKON Z 7", "Nikon Z 7"),
            ("NIKON CORPORATION", "NIKON Z 7_2", "Nikon Z 7_2"),
            ("NIKON CORPORATION", "NIKON Z 50", "Nikon Z 50"),
            ("NIKON CORPORATION", "NIKON Z fc", "Nikon Z fc"),
            ("Canon", "Canon EOS R", "Canon EOS R"),
            ("Canon", "Canon EOS RP", "Canon EOS RP"),
            ("Canon", "Canon EOS 250D", "Canon EOS 250D"),
            ("Canon", "Canon EOS REBEL SL3", "Canon EOS 250D"),
            ("Canon", "Canon EOS 850D", "Canon EOS 850D"),
        ] {
            assert!(adobe_entry(make, model).is_none(), "{model}");
            let p = camera_profile(make, model).unwrap();
            assert_eq!(p.prefix, key);
            assert!(p.adobe, "{model}");
        }
        assert!(adobe_entry("Canon", "Canon EOS R5").is_none());
        assert!(adobe_entry("Canon", "Canon EOS R6m2").is_none());
        // Bodies with their own Adobe entry keep it.
        assert!(
            adobe_entry("NIKON CORPORATION", "NIKON D850")
                .unwrap()
                .sibling_of
                .is_empty()
        );
    }

    #[test]
    fn later_generations_do_not_match() {
        assert_eq!(
            camera_key("Canon", "Canon EOS-1D X Mark III"),
            "Canon EOS-1D X Mark III"
        );
        assert!(prefix_matches("Canon EOS-1D X", "Canon EOS-1D X"));
        assert!(!prefix_matches("Canon EOS-1D X Mark III", "Canon EOS-1D"));
        assert!(!prefix_matches("Canon EOS-1D X Mark III", "Canon EOS-1D X"));
        assert!(!prefix_matches(
            "Canon PowerShot G1 X Mark II",
            "Canon PowerShot G1"
        ));
        assert!(!prefix_matches("Fujifilm X-T30 II", "Fujifilm X-T30"));
        assert!(!prefix_matches("Canon EOS R6m2", "Canon EOS R6"));
        assert!(prefix_matches("Nikon 1 V1", "Nikon 1"));
        let p = camera_profile("Canon", "Canon EOS-1D X Mark III").unwrap();
        assert_eq!(p.prefix, "Canon EOS-1D X Mark III");
    }

    fn corpus(rel: &str) -> Option<Vec<u8>> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../tools/corpus/large/raw")
            .join(rel);
        std::fs::read(p).ok()
    }

    /// The table holds exactly what our own DNG reader finds in the source files (bit-exact),
    /// and raw files of the same body pick the profile up. Skipped without the corpus
    /// (tools/corpus/fetch-conformance.sh large_raw).
    #[test]
    fn profiles_match_source_dngs() {
        let mut checked = 0;
        for (prefix, dng, raw) in [
            (
                "Pentax K-3 Mark III",
                "Pentax/K-3 Mark III/_IMG0114.DNG",
                "Pentax/K-3 Mark III/IMGP9854.PEF",
            ),
            (
                "Pentax K-1 Mark II",
                "Pentax/K-1 Mark II/_IMG2387.DNG",
                "Pentax/K-1 Mark II/_IMG2388.PEF",
            ),
        ] {
            let Some(d) = corpus(dng) else {
                eprintln!("skipping {dng}: not in the corpus");
                continue;
            };
            let p = crate::adobe_coeff::PROFILES
                .iter()
                .find(|p| p.prefix == prefix)
                .unwrap();
            let (info, sensor) = crate::container::parse(&d).unwrap();
            assert_eq!(camera_key(&info.make, &info.model), prefix);
            let ColorSource::Dng(c) = sensor.unwrap().color else {
                panic!("{dng}: no DNG colour");
            };
            assert_eq!(
                (c.cm, c.fm, c.illuminant),
                (p.cm, p.fm, p.illuminant),
                "{dng}"
            );
            checked += 1;
            if let Some(r) = corpus(raw) {
                let f = crate::parse(&r).unwrap();
                let s = f.sensor.as_ref().unwrap();
                assert!(
                    matches!(s.color, ColorSource::Matrix(_, MatrixSource::CameraProfile)),
                    "{raw}: {:?}",
                    s.color
                );
            }
        }
        eprintln!("{checked} profile sources checked");
    }

    /// Adobe DNG Converter rows of tools/dng-colour-sources.tsv: the table holds the DNG's
    /// ColorMatrix pair bit-exact (ForwardMatrix left out, see adobe_coeff.rs), and the source
    /// raw picks it up as `MatrixSource::Adobe`. Skipped without the corpus
    /// (fetch-conformance.sh large_raw large_raw_adobe, then tools/corpus/adobe-dng.sh).
    #[test]
    fn converter_profiles_match_their_dngs() {
        let tsv = include_str!("../../../../tools/dng-colour-sources.tsv");
        let (mut rows, mut dngs, mut raws) = (0, 0, 0);
        for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let cols: Vec<&str> = line.split('\t').collect();
            let [prefix, dng, _, _, raw] = cols[..] else {
                continue;
            };
            rows += 1;
            let p = crate::adobe_coeff::PROFILES
                .iter()
                .find(|p| p.prefix == prefix)
                .unwrap_or_else(|| panic!("{prefix}: not in PROFILES"));
            assert!(p.adobe, "{prefix}");
            assert_eq!(p.fm, [None, None], "{prefix}");
            // Dual-pixel DNGs (2 samples per pixel, e.g. EOS R5 "dual") are not read as a
            // sensor yet; the generator reads their IFD0 tags directly.
            if let Some(d) = corpus(dng)
                && let Some(s) = crate::container::parse(&d).unwrap().1
                && let ColorSource::Dng(c) = s.color
            {
                assert_eq!((c.cm, c.illuminant), (p.cm, p.illuminant), "{dng}");
                dngs += 1;
            }
            // Some sources are not decodable yet (newer RW2, Hasselblad): skip those.
            if let Some(r) = corpus(raw)
                && let Ok(f) = crate::parse(&r)
                && let Some(s) = f.sensor.as_ref()
            {
                assert_eq!(camera_key(&f.info.make, &f.info.model), prefix, "{raw}");
                // Hasselblad 3FR/FFF carry DNG-style colour tags of their own, used as such.
                assert!(
                    matches!(
                        s.color,
                        ColorSource::Matrix(_, MatrixSource::Adobe) | ColorSource::Dng(_)
                    ),
                    "{raw}: {:?}",
                    s.color
                );
                raws += 1;
            }
        }
        assert!(rows >= 40, "{rows} converter rows");
        eprintln!("{rows} converter profiles, {dngs} DNGs and {raws} source raws checked");
    }

    /// ColorChecker 24 (X-Rite, CIE L*a*b* D50).
    const CHECKER_LAB: [[f64; 3]; 24] = [
        [37.54, 14.37, 14.92],
        [64.66, 19.27, 17.5],
        [49.32, -3.82, -22.54],
        [43.46, -12.74, 22.72],
        [54.94, 9.61, -24.79],
        [70.48, -32.26, -0.37],
        [62.73, 35.83, 56.5],
        [39.43, 10.75, -45.17],
        [50.57, 48.64, 16.67],
        [30.1, 22.54, -20.87],
        [71.77, -24.13, 58.19],
        [71.51, 18.24, 67.37],
        [28.37, 15.42, -49.8],
        [54.38, -39.72, 32.27],
        [42.43, 51.05, 28.62],
        [81.8, 2.67, 80.41],
        [50.63, 51.28, -14.12],
        [49.57, -29.71, -28.32],
        [95.19, -1.03, 2.93],
        [81.29, -0.57, 0.44],
        [66.89, -0.75, -0.06],
        [50.76, -0.13, 0.14],
        [35.63, -0.46, -0.48],
        [20.64, 0.07, -0.46],
    ];
    const D50: [f64; 3] = [0.9642, 1.0, 0.8249];

    fn lab_to_xyz(l: [f64; 3]) -> [f64; 3] {
        let fy = (l[0] + 16.0) / 116.0;
        let f = [fy + l[1] / 500.0, fy, fy - l[2] / 200.0];
        std::array::from_fn(|i| {
            let t = f[i];
            let v = if t > 6.0 / 29.0 {
                t * t * t
            } else {
                3.0 * (6.0f64 / 29.0).powi(2) * (t - 4.0 / 29.0)
            };
            v * D50[i]
        })
    }

    fn xyz_to_lab(x: [f64; 3]) -> [f64; 3] {
        let f = |t: f64| {
            if t > (6.0f64 / 29.0).powi(3) {
                t.cbrt()
            } else {
                t / (3.0 * (6.0f64 / 29.0).powi(2)) + 4.0 / 29.0
            }
        };
        let [fx, fy, fz] = std::array::from_fn(|i| f(x[i] / D50[i]));
        [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
    }

    /// Mean CIE76 ΔE over the ColorChecker of a DNG colour description against a camera model
    /// (`cam_xyz`: XYZ D65 → camera, LibRaw's matrix for the body), rendered as the viewer does
    /// (exposure 0, D65 white balance).
    fn checker_delta_e(c: &DngColor, cam_xyz: &M3) -> Option<f64> {
        let (m, _, wb, profile) = dng(c)?;
        let m64: M3 = std::array::from_fn(|i| std::array::from_fn(|j| m[i][j] as f64));
        let d50_to_d65 = mul(&SRGB_TO_XYZ, &XYZD50_TO_SRGB);
        let srgb_to_d50 = inv(&XYZD50_TO_SRGB)?;
        let mut sum = 0.0;
        for lab in CHECKER_LAB {
            let cam = mulv(cam_xyz, mulv(&d50_to_d65, lab_to_xyz(lab)));
            let cam = [cam[0] * wb[0] as f64, cam[1], cam[2] * wb[2] as f64];
            let rgb = match &profile {
                Some(p) => p.apply(cam.map(|v| v as f32)).map(f64::from),
                None => mulv(&m64, cam),
            };
            let out = xyz_to_lab(mulv(&srgb_to_d50, rgb));
            sum += (0..3)
                .map(|i| (out[i] - lab[i]).powi(2))
                .sum::<f64>()
                .sqrt();
        }
        Some(sum / 24.0)
    }

    /// ColorChecker ΔE of the converter DNGs for: ColorMatrix only, ForwardMatrix without the
    /// profile tables (the earlier behaviour), and the full profile. Needs the corpus and
    /// LibRaw's matrices of the source raws (`SPECULAR_CAMXYZ_TSV`: path, 9 values, name).
    #[test]
    #[ignore = "needs the corpus and SPECULAR_CAMXYZ_TSV"]
    fn converter_profile_delta_e() {
        let tsv = include_str!("../../../../tools/dng-colour-sources.tsv");
        let cams = std::fs::read_to_string(std::env::var("SPECULAR_CAMXYZ_TSV").unwrap()).unwrap();
        let mut tot = [0.0f64; 3];
        let mut n = 0;
        for line in tsv.lines().filter(|l| !l.starts_with('#') && !l.is_empty()) {
            let cols: Vec<&str> = line.split('\t').collect();
            let [prefix, dng_path, _, _, raw] = cols[..] else {
                continue;
            };
            let Some(cam_line) = cams
                .lines()
                .find(|l| l.split('\t').next().is_some_and(|p| p.ends_with(raw)))
            else {
                continue;
            };
            let v: Vec<f64> = cam_line
                .split('\t')
                .skip(1)
                .take(9)
                .flat_map(str::parse)
                .collect();
            let Ok(v) = <[f64; 9]>::try_from(v) else {
                continue;
            };
            let cam_xyz = m3(&v);
            let Some(d) = corpus(dng_path) else { continue };
            let Some(s) = crate::container::parse(&d).unwrap().1 else {
                continue;
            };
            let ColorSource::Dng(mut c) = s.color else {
                continue;
            };
            let neutral = mulv(&cam_xyz, xy_to_xyz(0.3127, 0.3290));
            c.neutral = Some(neutral.map(|x| x / neutral[1]));
            c.white_xy = None;
            c.exposure = 0.0;
            // Colour accuracy of the profile, not its rendering look.
            let full = DngColor {
                tone_curve: None,
                ..(*c).clone()
            };
            let fm_only = DngColor {
                hue_sat_map: [None, None],
                look_table: None,
                needs_tables: false,
                tables_malformed: false,
                ..full.clone()
            };
            let cm_only = DngColor {
                fm: [None, None],
                ..fm_only.clone()
            };
            let de = [&cm_only, &fm_only, &full].map(|c| checker_delta_e(c, &cam_xyz));
            let Ok(de) = <[f64; 3]>::try_from(de.iter().flatten().copied().collect::<Vec<_>>())
            else {
                eprintln!("{prefix:32} no colour transform: {de:?}");
                continue;
            };
            let src = dng(&full).unwrap().1;
            eprintln!(
                "{prefix:32} CM {:5.2}  FM {:5.2}  FM+tables {:5.2}  {src:?}",
                de[0], de[1], de[2]
            );
            for i in 0..3 {
                tot[i] += de[i];
            }
            n += 1;
        }
        assert!(n > 0);
        let n = n as f64;
        eprintln!(
            "{n} bodies: mean CM {:.2}  FM {:.2}  FM+tables {:.2}",
            tot[0] / n,
            tot[1] / n,
            tot[2] / n
        );
    }
}

#[cfg(test)]
mod as_shot_tests {
    use super::*;

    /// Multipliers a body would record under an illuminant of chromaticity `xy`.
    fn wb_under(m: &[i16; 9], xy: [f64; 2]) -> [f32; 3] {
        let cam_xyz = m3(&std::array::from_fn(|i| f64::from(m[i]) / 10000.0));
        let xyz = [xy[0] / xy[1], 1.0, (1.0 - xy[0] - xy[1]) / xy[1]];
        let n = mulv(&cam_xyz, xyz);
        [n[1] / n[0], 1.0, n[1] / n[2]].map(|v| v as f32)
    }

    /// Per brand (the first body of each in the table): daylight and tungsten multipliers
    /// give back their illuminant.
    #[test]
    fn as_shot_multipliers_give_back_the_illuminant() {
        let mut brands = 0;
        for brand in [
            "Canon",
            "Nikon",
            "Sony",
            "Fujifilm",
            "Olympus",
            "Panasonic",
            "Pentax",
            "Leica",
            "Hasselblad",
            "Samsung",
        ] {
            let Some(e) = crate::adobe_coeff::TABLE
                .iter()
                .find(|e| e.prefix.starts_with(brand) && e.matrix.is_some())
            else {
                continue;
            };
            let m = e.matrix.unwrap();
            let model = e.prefix[brand.len()..].trim();
            for xy in [[0.3127, 0.3290], [0.4476, 0.4074]] {
                let got = as_shot_xy(brand, model, wb_under(&m, xy))
                    .unwrap_or_else(|| panic!("{}", e.prefix));
                assert!(
                    (got[0] - xy[0]).abs() < 1e-4 && (got[1] - xy[1]).abs() < 1e-4,
                    "{}: {got:?} vs {xy:?}",
                    e.prefix
                );
            }
            brands += 1;
        }
        assert!(brands >= 8, "{brands} brands");
        // No table matrix, or nonsense multipliers: nothing.
        assert_eq!(as_shot_xy("Nobody", "X1", [2.0, 1.0, 1.5]), None);
        assert_eq!(
            as_shot_xy("Canon", "EOS 5D Mark III", [0.0, 1.0, 1.5]),
            None
        );
    }
}
