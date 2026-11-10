//! Soft-proofing: the picture as a printer or other output
//! profile would reproduce it, and which colours it cannot.
//!
//! - **Chain.** The display-linear picture (linear Rec.709) → the proof profile's device
//!   values with the chosen intent (perceptual, or relative colorimetric with or without
//!   black point compensation) → back to the display: relative colorimetric with black
//!   point compensation by default, without it to show the ink's black, or absolute
//!   colorimetric to show the paper's white (which also shows its black). All of it is
//!   `specular-color`'s transforms: [`Proofer::proof`] is the CPU reference.
//! - **Gamut.** A colour is out of the proof gamut when it does not survive a colorimetric
//!   round trip through the profile: [`Proofer::gamut_de`] is the CIE76 ΔE of that trip,
//!   flagged above [`GAMUT_DE`].
//! - **GPU.** [`Proofer::bake`] samples the chain on a 33³ grid over the sRGB-encoded cube
//!   (even steps to the eye); [`ProofPass`] interpolates it tetrahedrally in place on the
//!   rendered picture and writes the round-trip ΔE into alpha for the gamut overlay. One
//!   pipeline; a new profile or setting uploads a new table, nothing is compiled.
//! - **Profiles.** [`system_profiles`] lists the output profiles in the system's ICC folders.

pub mod synthetic;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use specular_color::icc::{ColorSpace, Profile};
use specular_color::math::xyz_to_lab;
use specular_color::profiles;
use specular_color::transform::{Intent, Transform};

use crate::colour;
use crate::graph::Precision;
use crate::scopes::{self, Targets};

/// Grid points per side of the baked table.
pub const GRID: usize = 33;
/// Round-trip ΔE (CIE76) above which a colour counts as out of the proof gamut.
pub const GAMUT_DE: f32 = 3.0;

/// The rendering intent into the proof profile.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ProofIntent {
    #[default]
    Perceptual,
    Relative,
}

/// How to proof.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ProofSettings {
    pub intent: ProofIntent,
    /// Black point compensation into the proof profile.
    pub bpc: bool,
    /// Show the paper's white (absolute colorimetric back to the display).
    pub paper: bool,
    /// Show the ink's black (no black point compensation back to the display).
    pub ink: bool,
}

/// The proof chain of one profile and settings; see the module docs.
pub struct Proofer {
    to_proof: Transform,
    back: Transform,
    check_in: Transform,
    check_back: Transform,
    outputs: usize,
    space: ColorSpace,
    /// The profile's content hash with the settings (cache key of the baked table).
    pub key: u64,
}

impl std::fmt::Debug for Proofer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Proofer").field("key", &self.key).finish()
    }
}

/// Why a profile cannot proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProofError {
    /// Not an ICC profile, or damaged.
    Unreadable(String),
    /// A kind of profile that does not describe an output (L*a*b*, named colours…).
    NotOutput,
}

impl std::fmt::Display for ProofError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProofError::Unreadable(e) => write!(f, "the profile cannot be read: {e}"),
            ProofError::NotOutput => f.write_str("the profile does not describe an output"),
        }
    }
}

impl Proofer {
    /// The chain for the profile in `icc` with `s`.
    pub fn new(icc: &[u8], s: ProofSettings) -> Result<Proofer, ProofError> {
        let p = Profile::parse(icc).map_err(|e| ProofError::Unreadable(format!("{e:?}")))?;
        Self::for_profile(&p, s)
    }

    pub fn for_profile(p: &Profile, s: ProofSettings) -> Result<Proofer, ProofError> {
        if !matches!(
            p.color_space,
            ColorSpace::Rgb | ColorSpace::Cmyk | ColorSpace::Cmy | ColorSpace::Gray
        ) || matches!(&p.class, b"abst" | b"nmcl" | b"link")
        {
            return Err(ProofError::NotOutput);
        }
        let disp = profiles::linear_bt709();
        let err = |e: specular_color::icc::Error| ProofError::Unreadable(format!("{e:?}"));
        let intent = match s.intent {
            ProofIntent::Perceptual => Intent::Perceptual,
            ProofIntent::Relative => Intent::Relative,
        };
        let to_proof = Transform::new(&disp, p, intent, s.bpc).map_err(err)?;
        let back = if s.paper {
            Transform::new(p, &disp, Intent::Absolute, false)
        } else {
            Transform::new(p, &disp, Intent::Relative, !s.ink)
        }
        .map_err(err)?;
        let check_in = Transform::new(&disp, p, Intent::Relative, false).map_err(err)?;
        let check_back = Transform::new(p, &disp, Intent::Relative, false).map_err(err)?;
        let mut key = p.hash ^ 0x9e37_79b9_7f4a_7c15;
        for b in [s.intent == ProofIntent::Relative, s.bpc, s.paper, s.ink] {
            key = key.rotate_left(7) ^ u64::from(b);
        }
        Ok(Proofer {
            outputs: to_proof.outputs(),
            to_proof,
            back,
            check_in,
            check_back,
            space: p.color_space,
            key,
        })
    }

    /// The proofed display colour of display-linear Rec.709 `rgb` (clipped to 0..1): the
    /// CPU reference.
    pub fn proof(&self, rgb: [f32; 3]) -> [f32; 3] {
        let x = rgb.map(|v| v.clamp(0.0, 1.0));
        let mut dev = [0f32; 16];
        self.to_proof.eval(&x, &mut dev[..self.outputs]);
        let mut out = [0f32; 3];
        self.back.eval(&dev[..self.outputs], &mut out);
        out.map(|v| v.clamp(0.0, 1.0))
    }

    /// The CIE76 ΔE of a colorimetric round trip through the profile (0 inside its gamut).
    pub fn gamut_de(&self, rgb: [f32; 3]) -> f32 {
        let x = rgb.map(|v| v.clamp(0.0, 1.0));
        let a = xyz_to_lab(self.check_in.to_pcs(&x));
        let mut dev = [0f32; 16];
        self.check_in.eval(&x, &mut dev[..self.outputs]);
        let b = xyz_to_lab(self.check_back.to_pcs(&dev[..self.outputs]));
        ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
    }

    /// The chain sampled on a [`GRID`]³ table over the sRGB-encoded cube: per node the
    /// proofed linear Rec.709 colour and the gamut ΔE (red slowest).
    pub fn bake(&self) -> ProofLut {
        let n = GRID;
        let mut data = vec![0f32; n * n * n * 4];
        let dec = |i: usize| decode(i as f32 / (n - 1) as f32);
        for r in 0..n {
            for g in 0..n {
                for b in 0..n {
                    let rgb = [dec(r), dec(g), dec(b)];
                    let p = self.proof(rgb);
                    let at = ((r * n + g) * n + b) * 4;
                    data[at..at + 3].copy_from_slice(&p);
                    data[at + 3] = self.gamut_de(rgb);
                }
            }
        }
        ProofLut {
            n: n as u32,
            data,
            key: self.key,
        }
    }

    /// The vectorscope's marks for this profile: its primaries and secondaries (inks and
    /// their overprints for CMYK) as OkLab a/b; `None` for a grey profile.
    pub fn targets(&self) -> Option<Targets> {
        let dev: [[f32; 4]; 6] = match self.space {
            ColorSpace::Rgb => [
                [1.0, 0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 1.0, 1.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [1.0, 0.0, 1.0, 0.0],
            ],
            ColorSpace::Cmyk | ColorSpace::Cmy => [
                [0.0, 1.0, 1.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [1.0, 0.0, 1.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [1.0, 1.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
            ],
            _ => return None,
        };
        let to709 = colour::mul(
            &colour::inv(&colour::rgb_to_xyz(
                &colour::REC709_PRIMARIES,
                colour::D65_XY,
            ))
            .expect("invertible"),
            &colour::adaptation_xyz(
                colour::Cat::Bradford,
                colour::xy_to_xyz(colour::D50_XY),
                colour::xy_to_xyz(colour::D65_XY),
            ),
        );
        let (lms, m2) = (scopes::rec709_to_lms(), scopes::m2_ab());
        let ab = dev.map(|d| {
            let xyz = self.check_back.to_pcs(&d[..self.outputs]).map(f64::from);
            let rgb = colour::mulv(&to709, xyz).map(|v| v as f32);
            scopes::ab(rgb, &lms, &m2)
        });
        Some(Targets { ab })
    }
}

fn decode(e: f32) -> f32 {
    if e <= 0.040_45 {
        e / 12.92
    } else {
        ((e + 0.055) / 1.055).powf(2.4)
    }
}

/// The baked chain ([`Proofer::bake`]).
#[derive(Clone, Debug, PartialEq)]
pub struct ProofLut {
    pub n: u32,
    pub data: Vec<f32>,
    pub key: u64,
}

impl ProofLut {
    /// The table at display-linear `rgb`, interpolated exactly as the GPU does: proofed
    /// colour and gamut ΔE.
    pub fn eval(&self, rgb: [f32; 3]) -> [f32; 4] {
        let n = self.n as usize;
        let s = (n - 1) as f32;
        let p = rgb.map(|v| scopes::encode(v) * s);
        let i = p.map(|v| (v.floor() as usize).min(n - 2));
        let f = [p[0] - i[0] as f32, p[1] - i[1] as f32, p[2] - i[2] as f32];
        let at = |dr: usize, dg: usize, db: usize| {
            let k = (((i[0] + dr) * n + i[1] + dg) * n + i[2] + db) * 4;
            [
                self.data[k],
                self.data[k + 1],
                self.data[k + 2],
                self.data[k + 3],
            ]
        };
        let c000 = at(0, 0, 0);
        let c111 = at(1, 1, 1);
        let (fr, fg, fb) = (f[0], f[1], f[2]);
        // The tetrahedron holding the point, by the order of its fractions.
        let (c1, c2, w1, w2, w3) = if fr >= fg {
            if fg >= fb {
                (at(1, 0, 0), at(1, 1, 0), fr, fg, fb)
            } else if fr >= fb {
                (at(1, 0, 0), at(1, 0, 1), fr, fb, fg)
            } else {
                (at(0, 0, 1), at(1, 0, 1), fb, fr, fg)
            }
        } else if fb >= fg {
            (at(0, 0, 1), at(0, 1, 1), fb, fg, fr)
        } else if fb >= fr {
            (at(0, 1, 0), at(0, 1, 1), fg, fb, fr)
        } else {
            (at(0, 1, 0), at(1, 1, 0), fg, fr, fb)
        };
        std::array::from_fn(|k| {
            c000[k] + (c1[k] - c000[k]) * w1 + (c2[k] - c1[k]) * w2 + (c111[k] - c2[k]) * w3
        })
    }
}

const PROOF_WGSL: &str = "
struct U { w: u32, h: u32, prec: u32, n: u32 };
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var<storage, read_write> img: array<u32>;
@group(0) @binding(2) var<storage, read> lut: array<vec4<f32>>;
fn load(i: u32) -> vec4<f32> {
    if (u.prec == 1u) {
        return vec4<f32>(unpack2x16float(img[2u * i]), unpack2x16float(img[2u * i + 1u]));
    }
    return vec4<f32>(bitcast<f32>(img[4u * i]), bitcast<f32>(img[4u * i + 1u]),
        bitcast<f32>(img[4u * i + 2u]), bitcast<f32>(img[4u * i + 3u]));
}
fn store(i: u32, v: vec4<f32>) {
    if (u.prec == 1u) {
        img[2u * i] = pack2x16float(v.xy);
        img[2u * i + 1u] = pack2x16float(v.zw);
        return;
    }
    img[4u * i] = bitcast<u32>(v.x);
    img[4u * i + 1u] = bitcast<u32>(v.y);
    img[4u * i + 2u] = bitcast<u32>(v.z);
    img[4u * i + 3u] = bitcast<u32>(v.w);
}
fn enc(v: f32) -> f32 {
    let l = clamp(v, 0.0, 1.0);
    if (l <= 0.0031308) { return l * 12.92; }
    return 1.055 * pow(l, 1.0 / 2.4) - 0.055;
}
fn at(i: vec3<u32>) -> vec4<f32> {
    return lut[(i.x * u.n + i.y) * u.n + i.z];
}
@compute @workgroup_size(16, 16, 1)
fn proof(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x >= u.w || gid.y >= u.h) { return; }
    let k = gid.y * u.w + gid.x;
    let c = load(k);
    let s = f32(u.n - 1u);
    let p = vec3<f32>(enc(c.x), enc(c.y), enc(c.z)) * s;
    let i = min(vec3<u32>(floor(p)), vec3<u32>(u.n - 2u));
    let f = p - vec3<f32>(i);
    let c000 = at(i);
    let c111 = at(i + vec3<u32>(1u, 1u, 1u));
    var c1: vec4<f32>;
    var c2: vec4<f32>;
    var w: vec3<f32>;
    if (f.x >= f.y) {
        if (f.y >= f.z) {
            c1 = at(i + vec3<u32>(1u, 0u, 0u)); c2 = at(i + vec3<u32>(1u, 1u, 0u)); w = f.xyz;
        } else if (f.x >= f.z) {
            c1 = at(i + vec3<u32>(1u, 0u, 0u)); c2 = at(i + vec3<u32>(1u, 0u, 1u)); w = f.xzy;
        } else {
            c1 = at(i + vec3<u32>(0u, 0u, 1u)); c2 = at(i + vec3<u32>(1u, 0u, 1u)); w = f.zxy;
        }
    } else if (f.z >= f.y) {
        c1 = at(i + vec3<u32>(0u, 0u, 1u)); c2 = at(i + vec3<u32>(0u, 1u, 1u)); w = f.zyx;
    } else if (f.z >= f.x) {
        c1 = at(i + vec3<u32>(0u, 1u, 0u)); c2 = at(i + vec3<u32>(0u, 1u, 1u)); w = f.yzx;
    } else {
        c1 = at(i + vec3<u32>(0u, 1u, 0u)); c2 = at(i + vec3<u32>(1u, 1u, 0u)); w = f.yxz;
    }
    let v = c000 + (c1 - c000) * w.x + (c2 - c1) * w.y + (c111 - c2) * w.z;
    store(k, vec4<f32>(clamp(v.xyz, vec3<f32>(0.0), vec3<f32>(1.0)), max(v.w, 0.0)));
}
";

/// The proof pass on one device; see the module docs.
pub struct ProofPass {
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    pipeline: wgpu::ComputePipeline,
    uniform: wgpu::Buffer,
    /// The uploaded table and its key.
    lut: Option<(u64, wgpu::Buffer)>,
}

impl ProofPass {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> ProofPass {
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("develop proof"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: true }),
            ],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("develop proof"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("develop proof"),
            source: wgpu::ShaderSource::Wgsl(PROOF_WGSL.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("develop proof"),
            layout: Some(&pl),
            module: &module,
            entry_point: Some("proof"),
            compilation_options: Default::default(),
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("develop proof params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        ProofPass {
            device: device.clone(),
            queue: queue.clone(),
            layout,
            pipeline,
            uniform,
            lut: None,
        }
    }

    /// Proofs the `w`×`h` picture in `img` (packed RGBA at `prec`) in place through `lut`;
    /// alpha becomes the gamut ΔE.
    pub fn run(&mut self, lut: &ProofLut, img: &wgpu::Buffer, w: u32, h: u32, prec: Precision) {
        if self.lut.as_ref().map(|l| l.0) != Some(lut.key) {
            let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("develop proof table"),
                size: (lut.data.len() * 4) as u64,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.queue
                .write_buffer(&b, 0, bytemuck::cast_slice(&lut.data));
            self.lut = Some((lut.key, b));
        }
        let Some((_, table)) = &self.lut else { return };
        let u = [w, h, u32::from(prec == Precision::F16), lut.n];
        self.queue
            .write_buffer(&self.uniform, 0, bytemuck::cast_slice(&u));
        let bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("develop proof"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: img.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: table.as_entire_binding(),
                },
            ],
        });
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("develop proof"),
            });
        {
            let mut cp = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("develop proof"),
                timestamp_writes: None,
            });
            cp.set_pipeline(&self.pipeline);
            cp.set_bind_group(0, &bind, &[]);
            cp.dispatch_workgroups(w.div_ceil(16), h.div_ceil(16), 1);
        }
        self.queue.submit([enc.finish()]);
    }
}

/// An output profile found on the system.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FoundProfile {
    pub path: PathBuf,
    /// Its description, else the file name.
    pub name: String,
    /// A printer (`prtr`) profile, listed first.
    pub printer: bool,
}

/// The system's ICC folders, most specific first.
pub fn icc_folders() -> Vec<PathBuf> {
    let mut v = Vec::new();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(target_os = "macos") {
        if let Some(h) = &home {
            v.push(h.join("Library/ColorSync/Profiles"));
        }
        v.push("/Library/ColorSync/Profiles".into());
        v.push("/System/Library/ColorSync/Profiles".into());
    } else if cfg!(windows) {
        let root = std::env::var_os("SystemRoot")
            .or_else(|| std::env::var_os("WINDIR"))
            .map_or_else(|| PathBuf::from(r"C:\Windows"), PathBuf::from);
        v.push(root.join(r"System32\spool\drivers\color"));
    } else {
        let data = std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| home.as_ref().map(|h| h.join(".local/share")));
        if let Some(d) = data {
            v.push(d.join("icc"));
        }
        if let Some(h) = &home {
            v.push(h.join(".color/icc"));
        }
        v.push("/usr/local/share/color/icc".into());
        v.push("/usr/share/color/icc".into());
        v.push("/var/lib/color/icc".into());
    }
    v
}

/// Output profiles (printers, and RGB or CMYK spaces that can be proofed) in `folders` and
/// one level of subfolders: printers first, then by name. Unreadable files are skipped.
pub fn find_profiles(folders: &[PathBuf]) -> Vec<FoundProfile> {
    let mut out: Vec<FoundProfile> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let scan = |dir: &Path, depth: u32, out: &mut Vec<FoundProfile>, f: &mut dyn FnMut(&Path)| {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        let mut entries: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
        entries.sort();
        for p in entries {
            if p.is_dir() {
                if depth > 0 {
                    f(&p);
                }
                continue;
            }
            let ext = p
                .extension()
                .map(|e| e.to_string_lossy().to_ascii_lowercase())
                .unwrap_or_default();
            if ext != "icc" && ext != "icm" {
                continue;
            }
            if let Some(found) = probe(&p) {
                out.push(found);
            }
        }
    };
    let mut subdirs = Vec::new();
    for d in folders {
        if seen.insert(d.clone()) {
            scan(d, 1, &mut out, &mut |p| subdirs.push(p.to_path_buf()));
        }
    }
    for d in subdirs {
        if seen.insert(d.clone()) {
            scan(&d, 0, &mut out, &mut |_| {});
        }
    }
    out.sort_by(|a, b| {
        b.printer
            .cmp(&a.printer)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    out.dedup_by(|a, b| a.name == b.name && a.printer == b.printer);
    out
}

/// The output profiles of the system's ICC folders ([`icc_folders`]).
pub fn system_profiles() -> Vec<FoundProfile> {
    find_profiles(&icc_folders())
}

/// The profile at `path` when it can be proofed: header and description only.
pub fn probe(path: &Path) -> Option<FoundProfile> {
    let meta = std::fs::metadata(path).ok()?;
    if meta.len() < 132 || meta.len() > 64 << 20 {
        return None;
    }
    let data = std::fs::read(path).ok()?;
    let class = data.get(12..16)?;
    let space = data.get(16..20)?;
    if data.get(36..40)? != b"acsp" {
        return None;
    }
    let output = matches!(class, b"prtr" | b"mntr" | b"spac" | b"scnr");
    if !output || !matches!(space, b"RGB " | b"CMYK" | b"GRAY") {
        return None;
    }
    let name = specular_color::icc::description(&data).unwrap_or_else(|| {
        path.file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default()
    });
    Some(FoundProfile {
        path: path.to_path_buf(),
        name,
        printer: class == b"prtr",
    })
}
