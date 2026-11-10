//! Named camera views kept next to the model: `<model file name>.specular-views.json`.
//!
//! A view holds where the camera is (eye, direction, up, projection, field of view or the
//! height shown), the section planes and box, and the elements hidden (IFC GUIDs), in the
//! viewer's frame of the model (z up, the model's own unit). The file is written atomically
//! (a temporary file renamed over it); the model itself is never touched.

use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// What the file says it is.
pub const FORMAT: &str = "specular-views";
pub const FORMAT_VERSION: u32 = 1;

/// Views a file keeps at most (more are not read).
pub const MAX_VIEWS: usize = 1000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SavedView {
    pub name: String,
    pub eye: [f64; 3],
    pub direction: [f64; 3],
    pub up: [f64; 3],
    /// `perspective` or `orthographic`.
    pub projection: String,
    /// Vertical field of view (degrees), in perspective.
    pub fov_degrees: f64,
    /// The height shown (model units), in orthographic.
    pub height: f64,
    /// The orbit centre's distance ahead of the eye.
    #[serde(default)]
    pub distance: f64,
    /// Section planes: normal x, y, z and offset (what `normal·p > offset` is cut away).
    #[serde(default)]
    pub planes: Vec<[f64; 4]>,
    /// The section box: min and max corners.
    #[serde(default)]
    pub section_box: Option<[[f64; 3]; 2]>,
    /// Elements hidden (IFC GUIDs).
    #[serde(default)]
    pub hidden: Vec<String>,
    /// Shading (`shaded`, `edges`, `wireframe`, `xray`).
    #[serde(default)]
    pub shading: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ViewsFile {
    pub format: String,
    pub version: u32,
    /// The model's file name (for a person reading the file).
    #[serde(default)]
    pub model: String,
    pub views: Vec<SavedView>,
}

impl ViewsFile {
    pub fn new(model: &str) -> ViewsFile {
        ViewsFile {
            format: FORMAT.into(),
            version: FORMAT_VERSION,
            model: model.into(),
            views: Vec::new(),
        }
    }

    /// A name not yet used: `base`, else `base 2`, `base 3`…
    pub fn free_name(&self, base: &str) -> String {
        if !self.views.iter().any(|v| v.name == base) {
            return base.to_owned();
        }
        (2..)
            .map(|i| format!("{base} {i}"))
            .find(|n| !self.views.iter().any(|v| &v.name == n))
            .unwrap_or_else(|| base.to_owned())
    }
}

/// The sidecar of `model`.
pub fn sidecar_path(model: &Path) -> PathBuf {
    let name = model
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    model.with_file_name(format!("{name}.specular-views.json"))
}

/// Reads views from `text`; views past [`MAX_VIEWS`] and ones with non-finite numbers are
/// left out.
pub fn parse(text: &str) -> Result<ViewsFile, String> {
    let mut f: ViewsFile = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if f.format != FORMAT {
        return Err(format!("not a views file ({})", f.format));
    }
    if f.version > FORMAT_VERSION {
        return Err(format!("views file version {} is newer", f.version));
    }
    f.views.truncate(MAX_VIEWS);
    f.views.retain(|v| {
        v.eye
            .iter()
            .chain(&v.direction)
            .chain(&v.up)
            .chain([&v.fov_degrees, &v.height])
            .all(|x| x.is_finite())
    });
    Ok(f)
}

/// Reads the sidecar at `path` (`Ok(None)` when there is none).
pub fn load(path: &Path) -> Result<Option<ViewsFile>, String> {
    match std::fs::read_to_string(path) {
        Ok(t) => parse(&t).map(Some),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

/// Writes `f` to `path` atomically; with no views left, removes the file.
pub fn save(path: &Path, f: &ViewsFile) -> io::Result<()> {
    if f.views.is_empty() {
        return match std::fs::remove_file(path) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        };
    }
    let text = serde_json::to_string_pretty(f).map_err(io::Error::other)?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text.as_bytes())?;
    std::fs::rename(&tmp, path).inspect_err(|_| {
        let _ = std::fs::remove_file(&tmp);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(name: &str) -> SavedView {
        SavedView {
            name: name.into(),
            eye: [1.0, -2.5, 1.6],
            direction: [0.0, 1.0, 0.0],
            up: [0.0, 0.0, 1.0],
            projection: "perspective".into(),
            fov_degrees: 55.0,
            height: 3.0,
            distance: 4.0,
            planes: vec![[0.0, 0.0, 1.0, 2.5]],
            section_box: Some([[0.0; 3], [5.0, 5.0, 3.0]]),
            hidden: vec!["2O2Fr$t4X7Zf8NOew3FLOH".into()],
            shading: Some("edges".into()),
        }
    }

    #[test]
    fn views_round_trip_through_the_sidecar() {
        let dir = std::env::temp_dir().join(format!("specular-views-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let model = dir.join("house.ifc");
        let side = sidecar_path(&model);
        assert_eq!(side.file_name().unwrap(), "house.ifc.specular-views.json");
        assert_eq!(load(&side), Ok(None));
        let mut f = ViewsFile::new("house.ifc");
        f.views.push(view("Entrance"));
        assert_eq!(f.free_name("Entrance"), "Entrance 2");
        f.views.push(view(&f.free_name("Entrance")));
        save(&side, &f).unwrap();
        assert_eq!(load(&side).unwrap(), Some(f.clone()));
        // Renamed, then every view deleted: the file goes.
        f.views[1].name = "Stair".into();
        save(&side, &f).unwrap();
        assert_eq!(load(&side).unwrap().unwrap().views[1].name, "Stair");
        f.views.clear();
        save(&side, &f).unwrap();
        assert!(!side.exists());
        let _ = std::fs::remove_dir_all(&dir);
        // Not a views file, or a newer one.
        assert!(parse("{\"format\":\"x\",\"version\":1,\"views\":[]}").is_err());
        assert!(parse("{\"format\":\"specular-views\",\"version\":9,\"views\":[]}").is_err());
    }
}
