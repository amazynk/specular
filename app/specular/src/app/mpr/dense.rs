//! Dense connectivity over voxels (CIFTI `.dconn.nii` whose seeds are voxels) in MPR: the
//! volume is one seed's row, its connectivity to every voxel. Moving the crosshair (a click
//! or a drag in a view) and letting go picks the seed whose voxel holds it; its row is read
//! from the file (only that row) and loaded in place of the shown one, the views kept.

use std::path::PathBuf;
use std::sync::Arc;

use specular_decode::neuro_dec::Connectivity;
use specular_i18n::t;

use super::load::{Prepared, Source};

type V3 = [f64; 3];

/// The connectivity file shown and the seed whose row is the volume.
pub(super) struct Dense {
    conn: Arc<Connectivity>,
    path: PathBuf,
    seed: u32,
}

impl Dense {
    pub(super) fn new(conn: Arc<Connectivity>, path: PathBuf, seed: u32) -> Dense {
        Dense { conn, path, seed }
    }

    /// The seed at patient point `c` (LPS) when it is another than the one shown.
    pub(super) fn pick(&self, c: V3) -> Option<u32> {
        let seed = self.conn.seed_at_ras([-c[0], -c[1], c[2]])?;
        (seed != self.seed).then_some(seed)
    }

    /// Shows `seed` from now on: the source its row loads from.
    pub(super) fn show(&mut self, seed: u32) -> Source {
        self.seed = seed;
        Source::Research(
            self.path.clone(),
            0,
            Some(Prepared::Seed(self.conn.clone(), seed)),
        )
    }

    /// The panel line: the seed shown.
    pub(super) fn text(&self) -> String {
        t!(
            "mpr-dconn-seed",
            seed = specular_i18n::isolate_ltr(&self.conn.seed_name(self.seed))
        )
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::super::load::{Loading, lock};
    use super::*;

    /// On the decoder's voxel fixture (24 thalamus seeds; rows 1 at the seed, falling off
    /// with distance): the crosshair on a thalamus voxel picks its seed, and the row loads as
    /// a volume that is 1 there, in the patient frame the file's affine gives.
    #[test]
    fn the_crosshair_picks_a_seed_and_its_row_loads() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../crates/formats/specular-neuro/tests/data/alone/vox.dconn.nii");
        let s = specular_decode::Registry::default()
            .open_path(&path)
            .unwrap();
        let conn = s.neuro().unwrap().connectivity.clone().unwrap().0;
        let mut d = Dense::new(conn.clone(), path.clone(), 0);
        // RAS (5, -6, -4) is seed 0's voxel: already shown. RAS (1, 0, -2) is seed 23's.
        assert_eq!(d.pick([-5.0, 6.0, -4.0]), None);
        assert_eq!(d.pick([-1.0, 0.0, -2.0]), Some(23));
        assert_eq!(d.pick([3.0, 2.0, 0.0]), None, "a putamen voxel is no seed");
        let pool = Arc::new(specular_core::JobPool::new(2, "mpr-test"));
        // The first row, as the file opened.
        let first = s.prepared_volume().unwrap();
        let mut l = Loading::start(
            Source::Research(path, 0, Some(Prepared::Volume(Arc::new(first)))),
            pool.clone(),
            Arc::new(|| {}),
            2048,
            1 << 30,
        );
        assert!(super::super::tests::wait(&mut l, Duration::from_secs(60)));
        assert_eq!(lock(&l.volume).sample_patient([-5.0, 6.0, -4.0]), Some(1.0));
        let mut l = Loading::start(d.show(23), pool, Arc::new(|| {}), 2048, 1 << 30);
        assert!(super::super::tests::wait(&mut l, Duration::from_secs(60)));
        assert_eq!(l.failed, 0);
        let v = lock(&l.volume);
        assert_eq!(v.sample_patient([-1.0, 0.0, -2.0]), Some(1.0));
        assert!(v.sample_patient([-5.0, 6.0, -4.0]).unwrap() < 1.0);
        assert_eq!(d.pick([-1.0, 0.0, -2.0]), None);
        assert!(d.text().contains("THALAMUS_LEFT (4, 6, 3)"), "{}", d.text());
    }
}
