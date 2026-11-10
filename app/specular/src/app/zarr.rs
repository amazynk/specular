//! Opening a Zarr store by its address (`crate::zarr_remote`): the metadata is read and the
//! links written on a worker, while the download indicator shows "Zarr metadata from
//! <host>"; then the first image opens like a file, and the arrow keys step through the
//! store's other images (a plate's fields, a layout's series).

use std::path::PathBuf;

use specular_i18n::t;

use super::App;
use crate::loader::Event;
use crate::zarr_remote;

impl App {
    /// Opens the Zarr store at `url` (`https://…`, `s3://…`).
    pub(super) fn zarr_open(&mut self, url: String) {
        let cache = specular_settings::Locations::detect().cache_dir;
        let dir = zarr_remote::links_dir(&cache, &url);
        let address = zarr_remote::root_of(&url);
        self.loader.background("zarr-open", move |l| {
            let result = zarr_remote::open_links(&url, &dir);
            l.post(Event::ZarrOpened { address, result });
        });
    }

    /// Fetches a COPC point cloud or a Cloud-Optimized GeoTIFF given by its address on a
    /// worker (the download indicator shows it), then opens the cached file like a Zarr
    /// store's first image.
    pub(super) fn remote_fetch_open(&mut self, url: String) {
        // Without query or user (a signed URL's signature stays out of the message).
        let address = specular_remote::keystore::display_url(&url);
        self.loader.background("remote-fetch", move |l| {
            let result = if crate::copc_remote::is_remote_copc(&url) {
                crate::copc_remote::fetch_arg(&url)
            } else if crate::stac_remote::is_remote_stac(&url) {
                crate::stac_remote::fetch_arg(&url)
            } else {
                crate::cog_remote::fetch_arg(&url)
            };
            l.post(Event::ZarrOpened { address, result });
        });
    }

    /// The store's links are written, or a fetched file is in the cache (or it failed).
    pub(super) fn zarr_opened(&mut self, address: String, result: Result<PathBuf, String>) {
        match result {
            Ok(path) => {
                self.gallery_dismiss();
                self.open_target(path);
            }
            Err(reason) => {
                let now = self.now();
                let address = address.trim_end_matches('/').to_owned();
                self.chrome.warning(
                    t!("zarr-open-failed", address = address, reason = reason),
                    now,
                );
            }
        }
        self.redraw = true;
    }
}

#[cfg(test)]
mod shots;

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use specular_remote::testing::{ServerOptions, TestServer};
    use specular_remote::zarr_fixture::{self, Spec};

    use crate::zarr_remote;

    /// An OME-Zarr z-stack, on a server and as a local folder, is an MPR volume through the
    /// science volume source: Z is the stacked axis, its voxels 0.5 × 0.5 × 2 µm from the
    /// multiscales' scale, and a plane read for MPR holds the file's samples.
    #[test]
    fn an_ome_zarr_z_stack_is_an_mpr_volume() {
        let spec = Spec {
            size: 64,
            chunk: 32,
            levels: 2,
            planes: 5,
            channels: 2,
            ..Spec::default()
        };
        let files = Arc::new(zarr_fixture::ome_zarr(&spec));
        let server = TestServer::start(
            ServerOptions::default(),
            zarr_fixture::handler(files.clone(), "/z.zarr/"),
        )
        .unwrap();
        let dir = std::env::temp_dir().join(format!("specular-zarr-z-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        zarr_remote::install(dir.join("cache"));
        let link = zarr_remote::open_links(&server.url("/z.zarr"), &dir.join("links")).unwrap();
        // The same store as a local folder.
        let local = dir.join("z.ome.zarr");
        for (k, v) in files.iter() {
            let p = local.join(k);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, v).unwrap();
        }
        for path in [link, local] {
            let s = specular_decode::Registry::default()
                .open_path(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let sum = s.science().expect("a science image");
            let st = super::super::mpr::science_stack(sum, &[1, 0], None, true).expect("a volume");
            assert_eq!((st.width, st.height, st.depth), (64, 64, 5));
            assert_eq!(sum.axes[st.z].name, "Z");
            assert_eq!(
                st.length,
                super::super::mpr::Length::Micron,
                "{}",
                path.display()
            );
            assert!((st.spacing[0] - 0.5).abs() < 1e-9, "{:?}", st.spacing);
            assert!((st.spacing[2] - 2.0).abs() < 1e-9, "{:?}", st.spacing);
            let cancel = specular_core::CancelToken::new();
            let p = s.nd_plane(&st.index_at(4), &cancel).unwrap().unwrap();
            let want = f32::from(zarr_fixture::sample(1, 4, 10, 20, 64));
            assert_eq!(p.values[10 * 64 + 20], want, "{}", path.display());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
