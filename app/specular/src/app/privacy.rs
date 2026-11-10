//! The no-network switch on the app's side (Settings › Privacy, `--no-network`,
//! `SPECULAR_NO_NETWORK`, the system policy file).
//!
//! The switch itself is `specular_core::net::off`; `specular_remote::Client` refuses every
//! request while it is on. Here: setting it from the four sources, stopping what was running
//! when it goes off, and the panel under the setting that says what is off and what holds it.

use std::path::Path;
use std::sync::OnceLock;

use specular_core::net::off::{self, By};
use specular_i18n::{isolate_ltr, t};
use specular_settings::policy::Policy;
use specular_widgets::{PanelRow, SettingPanel};

use crate::app::App;

/// The setting the panel sits under.
pub(super) const PANEL: &str = "privacy.no_network";

static POLICY: OnceLock<Policy> = OnceLock::new();

/// The system policy read at start-up (none before [`startup`]).
pub(crate) fn policy() -> &'static Policy {
    POLICY.get_or_init(Policy::default)
}

/// Start-up, before anything could connect: the environment, the flag and the policy file.
pub(crate) fn startup(flag: bool) {
    let p = POLICY.get_or_init(Policy::load);
    off::apply_env();
    off::set(By::Flag, flag);
    off::set(By::Policy, p.no_network);
    if off::is_off() {
        specular_log::info!(target: specular_log::SETTINGS, "network off ({:?})", off::held_by());
    }
}

/// "Network is off in Settings › Privacy." (where a network feature would be).
pub(crate) fn off_text() -> String {
    t!("network-off")
}

/// The panel under the switch: nothing while the network is on; else what is off and, when
/// something other than the switch holds it, what (the switch cannot undo that).
pub(crate) fn panel(held: Option<By>, policy_file: Option<&Path>) -> Option<SettingPanel> {
    let held = held?;
    let mut rows = vec![PanelRow {
        id: "what".into(),
        label: t!("privacy-off-what"),
        ..PanelRow::default()
    }];
    let why = match held {
        By::Setting => None,
        By::Policy => Some(t!(
            "privacy-held-policy",
            file = isolate_ltr(
                &policy_file
                    .map(|f| f.display().to_string())
                    .unwrap_or_default()
            )
        )),
        By::Env => Some(t!("privacy-held-run", what = isolate_ltr(off::ENV))),
        By::Flag => Some(t!("privacy-held-run", what = isolate_ltr(off::FLAG))),
    };
    if let Some(why) = why {
        rows.insert(
            0,
            PanelRow {
                id: "held".into(),
                label: why,
                attention: true,
                ..PanelRow::default()
            },
        );
    }
    Some(SettingPanel {
        title: t!("privacy-off-title"),
        rows,
        keywords: "offline internet network air-gapped".into(),
        ..SettingPanel::default()
    })
}

impl App {
    /// Applies the setting (from `apply_settings`); going off stops whatever was downloading.
    pub(crate) fn privacy_apply(&mut self) {
        let was = off::is_off();
        off::set(By::Setting, self.settings.privacy.no_network);
        if off::is_off() && !was {
            let reg = specular_core::net::global();
            for a in reg.snapshot() {
                if a.state == specular_core::net::State::Running {
                    reg.cancel(a.id);
                }
            }
            specular_log::info!(target: specular_log::SETTINGS, "network off");
        }
        self.privacy_panel();
        self.stream_settings_apply();
    }

    /// Shows (or clears) the panel under the switch.
    pub(crate) fn privacy_panel(&mut self) {
        let p = panel(off::held_by(), policy().file.as_deref());
        if self.chrome.set_setting_panel(PANEL, p) {
            self.redraw = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_says_what_holds_the_network_off() {
        assert!(panel(None, None).is_none());
        let p = panel(Some(By::Setting), None).unwrap();
        assert_eq!(p.title, t!("privacy-off-title"));
        assert!(p.rows.iter().all(|r| r.id != "held"));
        let f = Path::new("/etc/specular/policy.toml");
        let p = panel(Some(By::Policy), Some(f)).unwrap();
        assert_eq!(p.rows[0].id, "held");
        assert!(p.rows[0].attention);
        assert!(p.rows[0].label.contains("/etc/specular/policy.toml"));
        for by in [By::Env, By::Flag] {
            let p = panel(Some(by), None).unwrap();
            assert_eq!(p.rows[0].id, "held", "{by:?}");
        }
        assert!(off_text().contains("Privacy"));
    }

    /// Every network feature, with the switch on (from `SPECULAR_NO_NETWORK`), against a
    /// loopback server that counts connections: none is accepted, nothing reaches the
    /// Downloads registry, and each says the network is off. The switch is process-wide, so
    /// this runs itself again in a child process (`--exact`) where it is on.
    #[test]
    fn no_network_feature_opens_a_socket() {
        const CHILD: &str = "SPECULAR_NO_NETWORK_HARNESS";
        const NAME: &str = "app::privacy::tests::no_network_feature_opens_a_socket";
        if std::env::var_os(CHILD).is_none() {
            let out = std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", NAME, "--test-threads=1"])
                .env(CHILD, "1")
                .env(off::ENV, "1")
                .output()
                .unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            assert!(out.status.success() && text.contains("1 passed"), "{text}");
            return;
        }
        use std::sync::atomic::Ordering;

        use specular_core::CancelToken;
        use specular_remote::testing::{ServerOptions, TestResponse, TestServer};
        use specular_remote::{Client, ClientConfig};

        off::apply_env();
        assert!(off::is_off() && off::held_by() == Some(By::Env));
        let server =
            TestServer::start(ServerOptions::default(), |_| TestResponse::ok("{}")).unwrap();
        let client = Client::new(ClientConfig::for_tests()).unwrap();
        let cancel = CancelToken::new();
        let url = server.url("/x");
        let dir = std::path::PathBuf::from(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../target/test-tmp/no-network-harness"
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let words = t!("error-reason-network-off");

        // Update check.
        let e = specular_diag::update::check(
            &client.tracked(specular_core::net::What::UpdateCheck),
            &url,
            &specular_diag::update::Version::parse("1.0.0").unwrap(),
            specular_diag::update::Channel::Stable,
            &cancel,
        )
        .unwrap_err();
        assert!(e.contains(off::MESSAGE), "{e}");
        // Local AI downloads, through the installer.
        let t = specular_ai::transport::Transports::default().with(Box::new(
            crate::app::ai::https::HttpsTransport::new(client.clone(), None).allowing_http(),
        ));
        let e = specular_ai::transport::Transport::open(&t, &url, 0)
            .err()
            .unwrap();
        assert!(e.to_string().contains(off::MESSAGE), "{e}");
        // DICOMweb.
        let dw =
            specular_dicomweb::DicomWeb::new(client.clone(), &url, specular_remote::Auth::None);
        let e = dw
            .search_studies(&specular_dicomweb::StudyQuery::default(), &cancel)
            .unwrap_err();
        assert!(e.is_network_off(), "{e:?}");
        // OMERO.
        let r = specular_omero::api::Omero::new(client.clone(), &url)
            .and_then(|o| o.connect(None, &cancel));
        assert!(
            matches!(r, Err(specular_omero::api::OmeroError::NetworkOff)),
            "{r:?}"
        );
        // A remote Zarr store.
        let store = crate::zarr_remote::store(&server.url("/img.zarr")).unwrap();
        assert_eq!(
            specular_core::keys::KeyStore::get(store.as_ref(), ".zattrs", None),
            Err(specular_core::keys::KeyError::NetworkOff)
        );
        // Cloud GeoTIFF and COPC point clouds.
        let e = crate::cog_remote::fetch_to(&client, &url, &dir, &cancel, &mut |_| {})
            .err()
            .unwrap();
        assert!(e.contains(&words) || e.contains(off::MESSAGE), "{e}");
        let e = crate::copc_remote::fetch_to(&client, &url, &dir, &cancel, &mut |_, _| {})
            .err()
            .unwrap();
        assert!(e.contains(&words) || e.contains(off::MESSAGE), "{e}");
        // The same given as addresses (the window's "remote-fetch" worker), tracked.
        for fetch in [
            crate::cog_remote::fetch_arg_with,
            crate::copc_remote::fetch_arg_with,
        ] {
            let e = fetch(&client, &url, &dir).unwrap_err();
            assert!(e.contains(&words) || e.contains(off::MESSAGE), "{e}");
        }
        // Base-map tiles and map services.
        let cache = specular_remote::DiskCache::open(dir.join("tiles"), 1 << 20).unwrap();
        let e = crate::app::gallery::basemap::fetch_online(
            &client,
            &cache,
            &specular_map::ogc::Tiles::Xyz(server.url("/{z}/{x}/{y}.png")),
            "loopback",
            (1, 0, 0),
            &cancel,
        )
        .unwrap_err();
        assert_eq!(e, words);
        let e = crate::app::map::services::describe(&client, &url, &cancel).unwrap_err();
        assert!(!e.is_empty());
        // "Play on…": the search for TVs and speakers, a renderer's description.
        let fake = specular_cast::testing::FakeRenderer::start(Default::default()).unwrap();
        let finder = specular_cast::find::find(
            fake.ssdp,
            std::time::Duration::from_millis(200),
            std::sync::Arc::new(|| {}),
        );
        let t0 = std::time::Instant::now();
        let done = loop {
            if let Some(specular_cast::Found::Done(e)) = finder.try_recv() {
                break e;
            }
            assert!(t0.elapsed() < std::time::Duration::from_secs(10));
            std::thread::sleep(std::time::Duration::from_millis(10));
        };
        assert_eq!(done, Some(specular_cast::Error::NetworkOff));
        assert_eq!(
            specular_cast::device::fetch(&fake.location(), std::time::Duration::from_secs(1)).err(),
            Some(specular_cast::Error::NetworkOff)
        );
        assert_eq!(fake.state().searches, 0);

        assert_eq!(server.stats().connections.load(Ordering::Relaxed), 0);
        assert_eq!(server.requests(), 0);
        assert!(
            specular_core::net::global().snapshot().is_empty(),
            "the Downloads indicator would show"
        );
        // The harness does see connections: with the switch off, one gets through.
        off::set(By::Env, false);
        assert!(
            client
                .get(&specular_remote::Request::get(&url), &cancel)
                .is_ok()
        );
        assert!(server.stats().connections.load(Ordering::Relaxed) >= 1);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
