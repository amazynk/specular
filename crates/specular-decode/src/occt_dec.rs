//! STEP (AP203, AP214, AP242) and IGES CAD models through OpenCASCADE (OCCT).
//!
//! OCCT never runs in the viewer: a decode host started with `--occt` (a pool of its own,
//! [`pool`]) loads Specular's OCCT bridge, reads the file from memory with XCAF (assembly
//! tree, names, colours), tessellates each distinct part once (BRepMesh, a deflection
//! relative to the model's size) and sends the triangles back
//! ([`specular_sandbox::CadModel`]). They become one [`specular_mesh::Mesh`] (a material per
//! colour) shown by the mesh viewer; the file's header and the assembly tree go to the info
//! panel.
//!
//! OCCT is the user's own install (LGPL, dynamically loaded). Without it, or with the setting
//! off ([`set_occt`]), the file is refused with what is needed. Reading runs in the host
//! while the viewer's loader thread waits, so a large model never blocks the window; the
//! wait is bounded by the CAD pool's timeout (`SPECULAR_CAD_TIMEOUT_MS`, default 10 minutes)
//! and its memory by the decode memory budget.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use specular_core::Source;
use specular_mesh::{Bounds, Material, Mesh, Part, Unit, Up};
use specular_sandbox::{
    CadFormat, CadLimits, CadModel, Config, Error as HostError, ErrorKind, Host,
};

use crate::{DecodeError, Format, HeaderFacts, Result, Session};

/// How STEP and IGES files may be opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OcctConfig {
    /// Open them with OpenCASCADE when it is installed (on by default).
    pub enabled: bool,
    /// A folder holding OpenCASCADE's libraries, when they are not where the system looks.
    pub library_dir: Option<PathBuf>,
}

impl Default for OcctConfig {
    fn default() -> Self {
        OcctConfig {
            enabled: std::env::var_os("SPECULAR_OCCT").is_none_or(|v| v != "0"),
            library_dir: None,
        }
    }
}

static CONFIG: RwLock<Option<OcctConfig>> = RwLock::new(None);

/// Sets how STEP and IGES files may be opened (the app calls this from its settings).
pub fn set_occt(c: Option<OcctConfig>) {
    *CONFIG.write() = c;
}

fn config() -> OcctConfig {
    CONFIG.read().clone().unwrap_or_default()
}

/// OpenCASCADE as an optional decoder ([`crate::optional`]): settings go to [`set_occt`].
/// Finding it looks for its kernel library on disk without loading it (only the decode host
/// loads it, when a file needs it, and says then if it can't).
pub fn occt_bridge() -> crate::optional::OcctBridge {
    crate::optional::OcctBridge {
        apply: |enabled, library_dir| {
            set_occt(Some(OcctConfig {
                enabled,
                library_dir,
            }))
        },
        probe: find_occt,
    }
}

/// The folder holding OpenCASCADE's kernel library and its version from the file name
/// (`libTKernel.so.7.9.3`, `libTKernel.7.9.3.dylib`; "installed" when the name has none).
fn find_occt(dir: Option<&Path>) -> crate::optional::OcctFound {
    let mut dirs: Vec<PathBuf> = Vec::new();
    match dir {
        Some(d) => dirs.push(d.to_path_buf()),
        None => {
            let var = if cfg!(windows) {
                "PATH"
            } else if cfg!(target_os = "macos") {
                "DYLD_FALLBACK_LIBRARY_PATH"
            } else {
                "LD_LIBRARY_PATH"
            };
            if let Some(v) = std::env::var_os(var) {
                dirs.extend(std::env::split_paths(&v));
            }
            let system: &[&str] = if cfg!(target_os = "macos") {
                &["/opt/homebrew/lib", "/usr/local/lib"]
            } else {
                &[
                    "/usr/lib",
                    "/usr/lib64",
                    "/usr/local/lib",
                    "/usr/lib/x86_64-linux-gnu",
                    "/usr/lib/aarch64-linux-gnu",
                ]
            };
            dirs.extend(system.iter().map(PathBuf::from));
        }
    }
    for d in &dirs {
        let Ok(rd) = std::fs::read_dir(d) else {
            continue;
        };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let rest = name
                .strip_prefix("libTKernel")
                .or_else(|| name.strip_prefix("TKernel"));
            let Some(rest) = rest else { continue };
            let version = if let Some(v) = rest.strip_prefix(".so.") {
                v.to_owned()
            } else if let Some(v) = rest.strip_suffix(".dylib") {
                v.trim_start_matches('.').to_owned()
            } else if rest == ".so" || rest == ".dll" {
                String::new()
            } else {
                continue;
            };
            let version = if version.is_empty() {
                "installed".to_owned()
            } else {
                version
            };
            return Ok((d.clone(), version));
        }
    }
    Err(match dir {
        Some(d) => format!("no OpenCASCADE kernel library in {}", d.display()),
        None => "OpenCASCADE's libraries were not found where the system keeps them".to_owned(),
    })
}

/// Longest a CAD read may take in the host.
const TIMEOUT: Duration = Duration::from_secs(600);

/// The CAD host pool for this library folder (rebuilt when the folder changes).
fn pool(dir: &Option<PathBuf>) -> Arc<Host> {
    static POOL: Mutex<Option<(Option<PathBuf>, Arc<Host>)>> = Mutex::new(None);
    let mut g = POOL.lock();
    if let Some((d, h)) = &*g
        && d == dir
    {
        return h.clone();
    }
    let mut cfg = Config::from_env();
    cfg.occt = true;
    // Two: a file the user opens need not wait behind a thumbnail's model.
    cfg.pool_size = 2;
    cfg.timeout = std::env::var("SPECULAR_CAD_TIMEOUT_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map_or(TIMEOUT, |ms| Duration::from_millis(ms.max(1)));
    // OCCT's B-rep and meshes of a large assembly take far more than an image: up to the
    // budget's share of the memory available now (the viewer reserves its part per file).
    if let Some(avail) = specular_core::available_memory() {
        cfg.memory_limit = cfg.memory_limit.max(avail / 5 * 4).min(1 << 46);
    }
    if let Some(d) = dir {
        let var = if cfg!(windows) {
            "PATH"
        } else if cfg!(target_os = "macos") {
            "DYLD_FALLBACK_LIBRARY_PATH"
        } else {
            "LD_LIBRARY_PATH"
        };
        let mut paths = vec![d.clone()];
        if let Some(old) = std::env::var_os(var) {
            paths.extend(std::env::split_paths(&old));
        }
        if let Ok(joined) = std::env::join_paths(paths) {
            cfg.env.push((var.into(), joined));
        }
    }
    let h = Arc::new(Host::new(cfg));
    *g = Some((dir.clone(), h.clone()));
    h
}

/// What the format is called in messages.
fn what(format: CadFormat) -> &'static str {
    match format {
        CadFormat::Step => "STEP",
        CadFormat::Iges => "IGES",
    }
}

fn map_err(format: CadFormat, e: HostError) -> DecodeError {
    let what = what(format);
    match e {
        HostError::NotInstalled(m) => DecodeError::Unsupported(format!(
            "{what} files need OpenCASCADE, which cannot run: {m}"
        )),
        HostError::Unavailable(m) => DecodeError::Unsupported(format!(
            "{what} files need the sandboxed decode host, which cannot run: {m}"
        )),
        HostError::Remote { kind, message } => match kind {
            ErrorKind::NotInstalled => {
                DecodeError::Unsupported(format!("{what} files need OpenCASCADE: {message}"))
            }
            ErrorKind::Unsupported | ErrorKind::Encrypted => DecodeError::Unsupported(message),
            ErrorKind::TooLarge => DecodeError::Malformed(format!("too large: {message}")),
            ErrorKind::Io => DecodeError::Io(std::io::Error::other(message)),
            _ => DecodeError::Malformed(message),
        },
        HostError::Timeout(t) => DecodeError::Malformed(format!(
            "OpenCASCADE took longer than {} s on this model",
            t.as_secs()
        )),
        e @ (HostError::Crashed(_) | HostError::Protocol(_)) => {
            DecodeError::Malformed(format!("OpenCASCADE failed on this model ({e})"))
        }
        HostError::Io(m) => DecodeError::Io(std::io::Error::other(m)),
    }
}

/// Tessellation for a model the user opened, and the coarser one for thumbnails.
fn limits() -> CadLimits {
    if specular_core::budget::current_share() == specular_core::budget::Share::Thumbnails {
        CadLimits {
            relative_deflection: 0.004,
            angular_deflection: 0.6,
            max_triangles: 2_000_000,
            max_bytes: 256 << 20,
        }
    } else {
        CadLimits::default()
    }
}

/// Opens a STEP or IGES file.
pub(crate) fn open(source: Arc<Source>, format: CadFormat) -> Result<Arc<dyn Session>> {
    let cfg = config();
    let what = what(format);
    if !cfg.enabled {
        return Err(DecodeError::Unsupported(format!(
            "{what} files open with OpenCASCADE, which is turned off: turn on \"Open STEP and \
             IGES with OpenCASCADE\" in Settings"
        )));
    }
    let bytes = source.bytes();
    // OCCT's own model of the file, then the meshes; the viewer keeps the meshes. A rough
    // share of the file's size for each, reserved while the host works.
    let len = bytes.len() as u64;
    let _hold = specular_core::budget::reserve(len.saturating_mul(4).max(64 << 20))?;
    let limits = limits();
    let t0 = std::time::Instant::now();
    let host = pool(&cfg.library_dir);
    let raw = host
        .cad_read(bytes, format, limits)
        .map_err(|e| map_err(format, e))?;
    let mut model = CadModel::decode(&raw, limits.max_triangles, limits.max_triangles * 3)
        .map_err(|e| DecodeError::Malformed(format!("OpenCASCADE's model: {e}")))?;
    drop(raw);
    let ms = t0.elapsed().as_secs_f64() * 1e3;
    specular_log::debug!(
        target: specular_log::DECODE,
        "{what}: {} triangles, {} parts read by OpenCASCADE in the host in {ms:.0} ms",
        model.triangle_count(),
        model.instances
    );
    if model.meshes.is_empty() {
        return Err(DecodeError::Unsupported(format!(
            "this {what} file has no surfaces to show (only curves or points)"
        )));
    }
    let header = match format {
        CadFormat::Step => step_header(bytes),
        CadFormat::Iges => iges_header(bytes),
    };
    let mut facts = header_facts(&header, &model, format);
    // A part used several times has its PMI once: copies per instance, found now while the
    // model has its meshes (they go into the viewer's mesh below).
    let split = (!model.pmi.is_empty()).then(|| crate::pmi::instance_split(&model));
    let (pmi, views) = (
        std::mem::take(&mut model.pmi),
        std::mem::take(&mut model.views),
    );
    let starts = (!pmi.is_empty()).then(|| crate::pmi::record_starts(&model));
    let scale = unit_of(model.unit_metres).1;
    let mesh = to_mesh(model, &header)?;
    let pmi = starts.map(|starts| {
        let cad = CadModel {
            pmi,
            views,
            ..CadModel::default()
        };
        crate::pmi::from_model(&cad, &mesh, scale, &starts, split)
    });
    if let Some(p) = &pmi {
        facts.fields.extend(crate::pmi::facts(p));
    }
    let fmt = match format {
        CadFormat::Step => Format::Step,
        CadFormat::Iges => Format::Iges,
    };
    Ok(crate::mesh_dec::session_with_pmi(
        source,
        fmt,
        mesh,
        Some(facts),
        None,
        pmi.filter(|p| !p.is_empty()).map(Arc::new),
    ))
}

/// Whether OpenCASCADE is switched on and installed (found on disk; not loaded).
pub(crate) fn available() -> bool {
    let cfg = config();
    cfg.enabled && find_occt(cfg.library_dir.as_deref()).is_ok()
}

/// Tessellates the solids of a STEP file in the host (IFC's advanced B-reps): the model, or
/// why not.
pub(crate) fn tessellate_step(step: &[u8]) -> std::result::Result<CadModel, String> {
    let cfg = config();
    if !cfg.enabled {
        return Err("OpenCASCADE is turned off in Settings".into());
    }
    let limits = limits();
    let host = pool(&cfg.library_dir);
    let raw = host
        .cad_read(step, CadFormat::Step, limits)
        .map_err(|e| map_err(CadFormat::Step, e).to_string())?;
    CadModel::decode(&raw, limits.max_triangles, limits.max_triangles * 3)
        .map_err(|e| format!("OpenCASCADE's model: {e}"))
}

/// The unit of the model's coordinates, and the factor taking them to it.
fn unit_of(metres: f64) -> (Unit, f32) {
    let known = [
        (Unit::Millimetre, 1e-3),
        (Unit::Metre, 1.0),
        (Unit::Centimetre, 1e-2),
        (Unit::Inch, 0.0254),
        (Unit::Foot, 0.3048),
        (Unit::Micron, 1e-6),
    ];
    for (u, m) in known {
        if ((metres - m) / m).abs() < 1e-6 {
            return (u, 1.0);
        }
    }
    (Unit::Millimetre, (metres / 1e-3) as f32)
}

/// One mesh: every instance's triangles, a material per colour (uncoloured faces draw with
/// the default look).
fn to_mesh(model: CadModel, header: &[(String, String)]) -> Result<Mesh> {
    let (unit, scale) = unit_of(model.unit_metres);
    let tris = model.triangle_count();
    let verts = model.vertex_count();
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    positions
        .try_reserve_exact(verts)
        .and_then(|_| normals.try_reserve_exact(verts))
        .and_then(|_| indices.try_reserve_exact(tris * 3))
        .map_err(|_| DecodeError::TooLarge)?;
    // Triangles grouped by colour, in first-seen order.
    let mut materials: Vec<Material> = Vec::new();
    let mut by_colour: Vec<(Option<[u32; 4]>, Vec<usize>)> = Vec::new();
    for (i, m) in model.meshes.iter().enumerate() {
        let key = m.colour.map(|c| c.map(f32::to_bits));
        match by_colour.iter_mut().find(|(k, _)| *k == key) {
            Some((_, v)) => v.push(i),
            None => by_colour.push((key, vec![i])),
        }
    }
    let mut parts = Vec::new();
    let mut bounds = Bounds::EMPTY;
    for (key, members) in &by_colour {
        let first = (indices.len() / 3) as u32;
        let material = key.map(|c| {
            let rgba = c.map(f32::from_bits);
            // Named after the first part drawn in it.
            let node = model.meshes[members[0]].node as usize;
            let name = model
                .nodes
                .get(node)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            materials.push(Material {
                name,
                base_color: rgba,
                texture: None,
            });
            (materials.len() - 1) as u32
        });
        for &i in members {
            let m = &model.meshes[i];
            let base = positions.len() as u32;
            for p in &m.positions {
                let q = p.map(|v| v * scale);
                bounds.add(q);
                positions.push(q);
            }
            normals.extend_from_slice(&m.normals);
            indices.extend(m.indices.iter().map(|&k| base + k));
        }
        let count = (indices.len() / 3) as u32 - first;
        if count > 0 {
            parts.push(Part {
                first,
                count,
                material,
            });
        }
    }
    let field = |k: &str| {
        header
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.trim().is_empty())
    };
    let title = model
        .nodes
        .iter()
        .find(|n| n.parent.is_none() && !n.name.trim().is_empty())
        .map(|n| n.name.clone())
        .or_else(|| field("Name"));
    let generator = field("Originating system")
        .or_else(|| field("Preprocessor"))
        .or_else(|| field("System"));
    Ok(Mesh {
        format: None,
        positions,
        normals,
        colors: None,
        uvs: None,
        indices,
        parts,
        materials,
        images: Vec::new(),
        unit,
        up: Up::Z,
        bounds,
        flat: false,
        file_normals: true,
        notes: model.notes.clone(),
        title,
        generator,
    })
}

/// Most assembly rows in the info panel.
const MAX_TREE_ROWS: usize = 500;

/// The info panel's header facts: the file's header, the counts, and the assembly tree.
fn header_facts(header: &[(String, String)], m: &CadModel, format: CadFormat) -> HeaderFacts {
    let get = |k: &str| {
        header
            .iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.trim().is_empty())
    };
    let mut summary = Vec::new();
    let creator = [get("Author"), get("Organization")]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    if !creator.is_empty() {
        summary.push(("creator", creator));
    }
    if let Some(d) = get("Date") {
        summary.push(("created", d));
    }
    if let Some(d) = get("Description") {
        summary.push(("comment", d));
    }
    let mut fields = header.to_vec();
    let mut add = |k: &str, v: String| fields.push((k.to_owned(), v));
    if !m.file_unit.is_empty() {
        add("Length unit", m.file_unit.to_lowercase());
    }
    add("Solids", m.solids.to_string());
    add("Faces", m.faces.to_string());
    if m.faces_unmeshed > 0 {
        add("Faces not meshed", m.faces_unmeshed.to_string());
    }
    add("Part instances", m.instances.to_string());
    add(
        "Mesh deflection",
        format!(
            "{:.3} {}",
            m.deflection,
            unit_of(m.unit_metres).0.symbol().unwrap_or("")
        ),
    );
    add(
        "Read by",
        match format {
            CadFormat::Step | CadFormat::Iges => "OpenCASCADE (sandboxed)".to_owned(),
        },
    );
    // The tree, indented by depth.
    let mut depth = vec![0usize; m.nodes.len()];
    for (i, n) in m.nodes.iter().enumerate() {
        depth[i] = n.parent.map_or(0, |p| depth[p as usize] + 1);
    }
    for (i, n) in m.nodes.iter().enumerate().take(MAX_TREE_ROWS) {
        let name = if n.name.trim().is_empty() {
            "(unnamed)"
        } else {
            n.name.trim()
        };
        let kind = if n.assembly { "assembly" } else { "part" };
        add(
            &format!("Assembly {}", i + 1),
            format!("{}{name} ({kind})", "  ".repeat(depth[i].min(16))),
        );
    }
    if m.nodes.len() > MAX_TREE_ROWS {
        add(
            "Assembly",
            format!("… {} more", m.nodes.len() - MAX_TREE_ROWS),
        );
    }
    HeaderFacts { summary, fields }
}

/// The text of a STEP string ('It''s', with `\X2\…\X0\` UTF-16 hex escapes).
fn step_string(s: &str) -> String {
    let s = s.trim();
    let inner = s
        .strip_prefix('\'')
        .and_then(|t| t.strip_suffix('\''))
        .unwrap_or(s)
        .replace("''", "'");
    let mut out = String::new();
    let mut rest = inner.as_str();
    while let Some(at) = rest.find("\\X2\\") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 4..];
        let Some(end) = after.find("\\X0\\") else {
            rest = after;
            break;
        };
        let hex = &after[..end];
        let units: Vec<u16> = (0..hex.len() / 4)
            .filter_map(|i| u16::from_str_radix(hex.get(i * 4..i * 4 + 4)?, 16).ok())
            .collect();
        out.push_str(&String::from_utf16_lossy(&units));
        rest = &after[end + 4..];
    }
    out.push_str(rest);
    out
}

/// The top-level arguments of `ENTITY( … );`, split at commas outside strings and lists.
fn step_args(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let (mut depth, mut quoted, mut cur) = (0i32, false, String::new());
    for c in s.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                cur.push(c);
            }
            '(' if !quoted => {
                depth += 1;
                if depth > 1 {
                    cur.push(c);
                }
            }
            ')' if !quoted => {
                depth -= 1;
                if depth >= 1 {
                    cur.push(c);
                }
                if depth == 0 {
                    out.push(std::mem::take(&mut cur));
                    break;
                }
            }
            ',' if !quoted && depth == 1 => out.push(std::mem::take(&mut cur)),
            _ if depth >= 1 => cur.push(c),
            _ => {}
        }
    }
    out
}

/// A STEP list argument's strings: `('a','b')` → a, b.
fn step_list(s: &str) -> String {
    let s = s.trim();
    if !s.starts_with('(') {
        return step_string(s);
    }
    step_args(s)
        .iter()
        .map(|x| step_string(x))
        .filter(|x| !x.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ")
}

/// The application protocol a STEP schema name stands for.
fn step_protocol(schema: &str) -> Option<&'static str> {
    let s = schema.to_ascii_uppercase();
    Some(
        if s.starts_with("AP242") || s.contains("MANAGED_MODEL_BASED_3D_ENGINEERING") {
            "AP242"
        } else if s.starts_with("AUTOMOTIVE_DESIGN") || s.starts_with("AP214") {
            "AP214"
        } else if s.starts_with("CONFIG_CONTROL_DESIGN") || s.starts_with("AP203") {
            "AP203"
        } else {
            return None;
        },
    )
}

/// The HEADER section of a STEP file (the first 64 KiB are enough).
pub(crate) fn step_header(bytes: &[u8]) -> Vec<(String, String)> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64 << 10)]);
    let Some(start) = head.find("HEADER;") else {
        return Vec::new();
    };
    let end = head[start..]
        .find("ENDSEC;")
        .map_or(head.len(), |e| start + e);
    let text: String = head[start..end].replace(['\r', '\n'], "");
    let mut out = Vec::new();
    let mut push = |k: &str, v: String| {
        if !v.trim().is_empty() {
            out.push((k.to_owned(), v.trim().to_owned()));
        }
    };
    let entity = |name: &str| text.find(name).map(|i| step_args(&text[i + name.len()..]));
    if let Some(a) = entity("FILE_DESCRIPTION") {
        push(
            "Description",
            a.first().map(|s| step_list(s)).unwrap_or_default(),
        );
    }
    if let Some(a) = entity("FILE_NAME") {
        let s = |i: usize| a.get(i).map(|x| step_string(x)).unwrap_or_default();
        let l = |i: usize| a.get(i).map(|x| step_list(x)).unwrap_or_default();
        push("Name", s(0));
        push("Date", s(1));
        push("Author", l(2));
        push("Organization", l(3));
        push("Preprocessor", s(4));
        push("Originating system", s(5));
        push("Authorization", s(6));
    }
    if let Some(a) = entity("FILE_SCHEMA") {
        let schema = a.first().map(|s| step_list(s)).unwrap_or_default();
        if let Some(p) = step_protocol(&schema) {
            push("Protocol", p.to_owned());
        }
        push("Schema", schema);
    }
    out
}

/// The Global section of an IGES file: its fixed-column lines marked `G` in column 73.
pub(crate) fn iges_header(bytes: &[u8]) -> Vec<(String, String)> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(64 << 10)]);
    let mut g = String::new();
    for line in head.lines() {
        let b = line.as_bytes();
        if b.len() >= 73 && b[72] == b'G' {
            g.push_str(line.get(..72).unwrap_or(""));
        }
    }
    if g.is_empty() {
        return Vec::new();
    }
    // Parameter and record delimiters: 1H, and 1H; unless the section starts with commas.
    let gb = g.as_bytes();
    let (delim, rec) = if gb.len() >= 3 && gb[0] == b'1' && gb[1] == b'H' {
        let d = gb[2] as char;
        let r = if gb.len() >= 7 && gb[4] == b'1' && gb[5] == b'H' {
            gb[6] as char
        } else {
            ';'
        };
        (d, r)
    } else {
        (',', ';')
    };
    // Fields up to the record delimiter, Hollerith strings (nH...) read by their count.
    let mut fields = Vec::new();
    let chars: Vec<char> = g.chars().collect();
    let stop = |c: char| c == delim || c == rec;
    let mut i = 0;
    while i < chars.len() && fields.len() <= 30 {
        let mut cur = String::new();
        let mut j = i;
        while j < chars.len() && chars[j] == ' ' {
            j += 1;
        }
        let num_start = j;
        while j < chars.len() && chars[j].is_ascii_digit() {
            j += 1;
        }
        if j > num_start && j < chars.len() && chars[j] == 'H' {
            let n: usize = chars[num_start..j]
                .iter()
                .collect::<String>()
                .parse()
                .unwrap_or(0);
            let s = j + 1;
            let e = s.saturating_add(n).min(chars.len());
            cur = chars[s..e].iter().collect();
            j = e;
            while j < chars.len() && !stop(chars[j]) {
                j += 1;
            }
        } else {
            j = i;
            while j < chars.len() && !stop(chars[j]) {
                cur.push(chars[j]);
                j += 1;
            }
        }
        fields.push(cur.trim().to_owned());
        if j >= chars.len() || chars[j] == rec {
            break;
        }
        i = j + 1;
    }
    let names = [
        (2, "Product"),
        (3, "File name"),
        (4, "System"),
        (5, "Preprocessor"),
        (11, "Product (receiver)"),
        (14, "Units"),
        (17, "Date"),
        (20, "Author"),
        (21, "Organization"),
    ];
    let mut out = Vec::new();
    for (i, k) in names {
        if let Some(v) = fields.get(i).filter(|v| !v.is_empty()) {
            out.push((k.to_owned(), v.clone()));
        }
    }
    if let Some(v) = fields.get(22).and_then(|v| v.parse::<u32>().ok()) {
        let ver = match v {
            11 => "5.3",
            10 => "5.2",
            9 => "5.1",
            8 => "5.0",
            7 => "4.0",
            6 => "3.0",
            _ => "",
        };
        if !ver.is_empty() {
            out.push(("IGES version".to_owned(), ver.to_owned()));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_occt_by_its_kernel_library() {
        let d = std::env::temp_dir().join(format!("specular-occt-find-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        assert!(find_occt(Some(&d)).is_err());
        std::fs::write(d.join("libTKernel.so.7.9.3"), b"").unwrap();
        assert_eq!(find_occt(Some(&d)), Ok((d.clone(), "7.9.3".to_owned())));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn step_header_fields() {
        let f = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('A gear', 'second'),'2;1');\n\
FILE_NAME('gear.stp','2024-05-01T10:00:00',('J. O''Brien'),('ACME'),'ST-DEVELOPER',\
'FreeCAD \\X2\\00FC\\X0\\ber','');\nFILE_SCHEMA(('AUTOMOTIVE_DESIGN { 1 0 10303 214 1 1 1 1 }'));\n\
ENDSEC;\nDATA;\n";
        let h = step_header(f);
        let get = |k: &str| h.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("Description"), Some("A gear, second"));
        assert_eq!(get("Name"), Some("gear.stp"));
        assert_eq!(get("Author"), Some("J. O'Brien"));
        assert_eq!(get("Organization"), Some("ACME"));
        assert_eq!(get("Originating system"), Some("FreeCAD über"));
        assert_eq!(get("Protocol"), Some("AP214"));
        assert!(get("Authorization").is_none());
        assert!(step_header(b"not step").is_empty());
    }

    #[test]
    fn iges_header_fields() {
        let mut f = String::new();
        let line = |s: &str, sec: char, n: u32| format!("{s:<72}{sec}{n:>7}\n");
        f += &line("A test part", 'S', 1);
        f += &line(
            "1H,,1H;,4Hpart,8Hpart.igs,7HOCCT7.9,4H1.00,32,38,6,308,15,4Hpart,",
            'G',
            1,
        );
        f += &line(
            "1.,2,2HMM,1,1.,15H20240501.100000,1.E-07,100.,6HAuthor,4HACME,11,0,",
            'G',
            2,
        );
        f += &line("15H20240501.100000;", 'G', 3);
        let h = iges_header(f.as_bytes());
        let get = |k: &str| h.iter().find(|(n, _)| n == k).map(|(_, v)| v.as_str());
        assert_eq!(get("Product"), Some("part"));
        assert_eq!(get("File name"), Some("part.igs"));
        assert_eq!(get("System"), Some("OCCT7.9"));
        assert_eq!(get("Units"), Some("MM"));
        assert_eq!(get("Author"), Some("Author"));
        assert_eq!(get("Organization"), Some("ACME"));
        assert_eq!(get("IGES version"), Some("5.3"));
    }

    #[test]
    fn units() {
        assert_eq!(unit_of(0.001), (Unit::Millimetre, 1.0));
        assert_eq!(unit_of(0.0254), (Unit::Inch, 1.0));
        let (u, s) = unit_of(0.5);
        assert_eq!(u, Unit::Millimetre);
        assert!((s - 500.0).abs() < 1e-3);
    }

    #[test]
    fn model_to_mesh_groups_by_colour() {
        use specular_sandbox::{CadMesh, CadNode};
        let tri = |node: u32, colour: Option<[f32; 4]>, x: f32| CadMesh {
            node,
            colour,
            positions: vec![[x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0]; 3],
            indices: vec![0, 1, 2],
        };
        let red = Some([1.0, 0.0, 0.0, 1.0]);
        let m = CadModel {
            nodes: vec![
                CadNode {
                    parent: None,
                    assembly: true,
                    colour: None,
                    name: "Top".into(),
                },
                CadNode {
                    parent: Some(0),
                    assembly: false,
                    colour: red,
                    name: "Bolt".into(),
                },
            ],
            meshes: vec![tri(1, red, 0.0), tri(1, None, 5.0), tri(1, red, 10.0)],
            instances: 1,
            ..CadModel::default()
        };
        let facts = header_facts(&[], &m, CadFormat::Step);
        assert!(facts.fields.iter().any(|(_, v)| v == "  Bolt (part)"));
        let mesh = to_mesh(m, &[]).unwrap();
        assert_eq!(mesh.triangle_count(), 3);
        assert_eq!(mesh.parts.len(), 2);
        assert_eq!(mesh.materials.len(), 1);
        assert_eq!(mesh.materials[0].name, "Bolt");
        assert_eq!((mesh.parts[0].count, mesh.parts[0].material), (2, Some(0)));
        assert_eq!(mesh.parts[1].material, None);
        assert_eq!(mesh.unit, Unit::Millimetre);
        assert_eq!(mesh.title.as_deref(), Some("Top"));
        assert_eq!(mesh.bounds.max[0], 11.0);
        // Indices of later meshes point past the earlier ones' vertices.
        assert_eq!(&mesh.indices[3..9], &[3, 4, 5, 6, 7, 8]);
    }
}
