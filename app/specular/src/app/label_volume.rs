//! "Overlay label volume…": a NIfTI (or MGH, NRRD, MetaImage, Analyze) file of integer
//! labels laid over the volume in MPR, for any volume in every profile (MPR's tier 1), or
//! over a DICOM image in medical mode (Medical, tier 2). Placed by both files' patient
//! coordinates ([`specular_labelmap`]: the RAS/LPS flip, sform before qform, oblique and
//! sheared grids), drawn by the segmentation overlay MPR and medical mode share: fill
//! opacity down to outlines only (`Alt+O`), shown or hidden (`Shift+O`), each label in the
//! segment list.
//!
//! Colours and names come from a colour table beside the file when there is one (an
//! ITK-SNAP label description or a FreeSurfer LUT: `<name>.txt`, `<name>.label`,
//! `<name>.lut`, `labels.txt`, `FreeSurferColorLUT.txt`), else from a generated palette.
//! The file is read on a thread; its memory is reserved from the budget first.

use std::path::{Path, PathBuf};

use crossbeam_channel::{Receiver, bounded};
use specular_dicom::labels::Labels;
use specular_i18n::t;
use specular_labelmap::{Lut, Refusal};

use super::App;

/// Most bytes a colour table may have.
const MAX_LUT: u64 = 8 << 20;
/// Extensions of label volumes (the file chooser's filter).
const EXTENSIONS: [&str; 9] = [
    "nii", "gz", "mgh", "mgz", "nrrd", "nhdr", "mha", "mhd", "hdr",
];

/// A finished read: the file's name and its labels, or why not.
type Loaded = (String, Result<Labels, String>);

#[derive(Default)]
pub(super) struct LabelVolumeState {
    rx: Option<Receiver<Loaded>>,
}

impl std::fmt::Debug for LabelVolumeState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LabelVolumeState")
            .field("loading", &self.rx.is_some())
            .finish()
    }
}

/// The name without `.nii.gz`, `.nii` or the last extension.
fn stem(p: &Path) -> String {
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let lower = name.to_ascii_lowercase();
    for ext in [
        ".nii.gz", ".nii", ".mgz", ".mgh", ".nrrd", ".nhdr", ".mha", ".mhd", ".hdr", ".img",
    ] {
        if lower.ends_with(ext) {
            return name[..name.len() - ext.len()].to_owned();
        }
    }
    name
}

/// A colour table beside `p`.
pub(super) fn find_lut(p: &Path) -> Option<Lut> {
    let dir = p.parent()?;
    let s = stem(p);
    let names = [
        format!("{s}.txt"),
        format!("{s}.label"),
        format!("{s}.lut"),
        format!("{s}_labels.txt"),
        format!("{s}-labels.txt"),
        "labels.txt".to_owned(),
        "label_description.txt".to_owned(),
        "FreeSurferColorLUT.txt".to_owned(),
    ];
    names.iter().map(|n| dir.join(n)).find_map(|f| {
        let m = std::fs::metadata(&f).ok()?;
        if !m.is_file() || m.len() > MAX_LUT {
            return None;
        }
        Lut::parse(&std::fs::read_to_string(&f).ok()?).ok()
    })
}

fn refusal_reason(r: &Refusal) -> String {
    match r {
        Refusal::Colour => t!("label-volume-colour"),
        Refusal::NotWhole => t!("label-volume-not-whole"),
        Refusal::TooManyLabels(_) => t!("label-volume-too-many"),
        Refusal::Empty => t!("label-volume-empty"),
        Refusal::TooLarge(_) => t!("label-volume-too-large"),
        Refusal::Degenerate => t!("label-volume-degenerate"),
    }
}

/// Reads `p` as labels, named by its colour table (or "Label n").
pub(super) fn load(p: &Path) -> Result<Labels, String> {
    let size = std::fs::metadata(p)
        .map_err(|e| crate::error_text::io_reason(&e))?
        .len();
    // The file, then about as much again for the decoded voxels and the labels' layer.
    let _hold = specular_core::budget::reserve(size.saturating_mul(3))
        .map_err(crate::error_text::needs_memory_reason)?;
    let vol =
        specular_decode::medvol::open_path(p).map_err(|e| crate::error_text::medvol_reason(&e))?;
    let lut = find_lut(p);
    let mut l =
        specular_labelmap::labels(&vol, lut.as_ref(), &stem(p)).map_err(|r| refusal_reason(&r))?;
    for s in &mut l.segments {
        if s.label.is_empty() {
            s.label = t!("label-volume-unnamed", value = s.number);
        }
    }
    Ok(l)
}

impl App {
    /// Asks for a label volume and reads it on a thread.
    pub(super) fn label_volume_open(&mut self) {
        let dir = self
            .current
            .and_then(|i| self.entries.get(i))
            .and_then(|p| p.parent())
            .map(Path::to_path_buf);
        let (tx, rx) = bounded(1);
        self.label_volume.rx = Some(rx);
        self.loader.background("label-volume", move |l| {
            let mut dlg = rfd::FileDialog::new().add_filter(t!("label-volume-file"), &EXTENSIONS);
            if let Some(d) = dir {
                dlg = dlg.set_directory(d);
            }
            let Some(p): Option<PathBuf> = dlg.pick_file() else {
                return;
            };
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let r = load(&p);
            let _ = tx.send((name, r));
            l.waker()();
        });
    }

    /// Lays a finished read over MPR's volume or the medical image.
    pub(super) fn label_volume_tick(&mut self) {
        let Some(rx) = &self.label_volume.rx else {
            return;
        };
        let Ok((file, r)) = rx.try_recv() else { return };
        self.label_volume.rx = None;
        let msg = match r {
            Err(e) => t!("label-volume-failed", file = file.clone(), reason = e),
            Ok(labels) => {
                let n = labels.segments.len() as i64;
                match self.label_volume_place(labels) {
                    Ok(true) => t!("label-volume-loaded", file = file.clone(), count = n),
                    Ok(false) => t!("label-volume-outside", file = file.clone()),
                    Err(e) => t!("label-volume-failed", file = file.clone(), reason = e),
                }
            }
        };
        let now = self.now();
        self.chrome.toast(msg, None, now);
        self.scene = None;
        self.redraw = true;
    }

    /// Adds `labels` to MPR (when open) or the medical image; false when they do not
    /// overlap it.
    fn label_volume_place(&mut self, labels: Labels) -> Result<bool, String> {
        if self.mpr_active() {
            return self.mpr_add_labels(labels);
        }
        Ok(self.medical.an.add_user_labels(labels))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_and_colour_tables_beside_the_file() {
        assert_eq!(stem(Path::new("/x/seg.nii.gz")), "seg");
        assert_eq!(stem(Path::new("/x/aseg.mgz")), "aseg");
        assert_eq!(stem(Path::new("/x/a.b.nrrd")), "a.b");
        let dir = std::env::temp_dir().join(format!("specular-lut-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let nii = dir.join("liver.nii.gz");
        assert!(find_lut(&nii).is_none());
        std::fs::write(dir.join("liver.txt"), "1 255 0 0 1 1 1 \"Liver\"\n").unwrap();
        assert_eq!(find_lut(&nii).unwrap().get(1).unwrap().name, "Liver");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn loads_an_oblique_label_file() {
        use specular_labelmap::testing::{Form, nifti, spheres};
        let m = [
            [0.9, -0.2, 0.0, -10.0],
            [0.2, 0.9, 0.1, -12.0],
            [0.0, -0.1, 1.2, -8.0],
        ];
        let data = spheres([24, 24, 16], &m, &[([0.0, 0.0, 0.0], 5.0, 3)]);
        let dir = std::env::temp_dir().join(format!("specular-labels-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("tumour.nii");
        std::fs::write(
            &p,
            nifti([24, 24, 16], [0.92, 0.93, 1.2], &Form::Sform(m), &data),
        )
        .unwrap();
        let l = load(&p).unwrap();
        assert_eq!(l.segments.len(), 1);
        assert_eq!(l.segments[0].number, 3);
        assert_eq!(l.segments[0].label, t!("label-volume-unnamed", value = 3));
        assert_eq!(l.title, "tumour");
        // The sphere's centre (RAS 0, 0, 0) is LPS 0, 0, 0.
        assert_eq!(specular_labelmap::value_at(&l, [0.0, 0.0, 0.0]), 3);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
