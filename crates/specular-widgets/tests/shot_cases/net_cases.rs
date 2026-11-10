use super::*;

use specular_widgets::{DownloadRow, DownloadState, Downloads};

/// What is downloading, as the app words it (`app/net.rs`), in the case's language: a
/// model with its size, speed and time left; map tiles counted by request; a DICOM series
/// of unknown size (its bar moves; still under reduced motion); an update check that failed.
pub(crate) fn downloads() -> Downloads {
    use specular_i18n::t;
    let join = |a: String, b: String| t!("net-join", first = a, second = b);
    let speed = |s: &str| t!("net-speed", size = s);
    let stop = |id: u32| Some((t!("net-stop"), format!("stop:{id}")));
    let rows = vec![
        DownloadRow {
            title: t!("net-what-update"),
            detail: "0 B".into(),
            status: t!("net-failed", reason = t!("net-reason-offline")),
            fraction: None,
            state: DownloadState::Failed,
            stop: None,
        },
        DownloadRow {
            title: t!(
                "net-what-dicom-series",
                series = "CT Abdomen 5 mm",
                host = "pacs.example.org"
            ),
            detail: join("3.4 MB".into(), speed("850 kB")),
            status: t!("net-running"),
            fraction: None,
            state: DownloadState::Running,
            stop: stop(3),
        },
        DownloadRow {
            title: t!("net-what-map-tiles", host = "tile.example.org"),
            detail: join(
                join(
                    t!("net-requests", done = "12", total = "40", count = 40),
                    "1.2 MB".into(),
                ),
                speed("310 kB"),
            ),
            status: t!("net-running"),
            fraction: Some(12.0 / 40.0),
            state: DownloadState::Running,
            stop: stop(2),
        },
        DownloadRow {
            title: t!("net-what-model", name = "clip-vit-b32"),
            detail: join(
                t!("net-bytes-of", done = "45 MB", total = "120 MB"),
                speed("2.3 MB"),
            ),
            status: t!("work-eta-seconds", n = 35),
            fraction: Some(45.0 / 120.0),
            state: DownloadState::Running,
            stop: stop(1),
        },
    ];
    let last = rows.last().unwrap();
    let name = t!(
        "net-chip-name",
        status = join(
            last.title.clone(),
            join(last.detail.clone(), last.status.clone())
        )
    );
    Downloads {
        rows,
        short: "3".into(),
        name,
        fraction: None,
    }
}

/// A DICOM series retrieved by C-GET: the server's counts (images done of all, failed,
/// warnings), bytes and speed, the time left at the pace so far.
pub(crate) fn retrieve_downloads() -> Downloads {
    use specular_i18n::t;
    let join = |a: String, b: String| t!("net-join", first = a, second = b);
    let detail = join(
        join(
            join(
                t!("net-images", done = "37", total = "120", count = 120),
                t!("net-images-failed", n = "2"),
            ),
            t!("net-images-warning", n = "1", count = 1),
        ),
        join("18.4 MB".into(), t!("net-speed", size = "3.1 MB")),
    );
    let rows = vec![DownloadRow {
        title: t!(
            "net-what-dicom-series",
            series = "CT Abdomen 5 mm",
            host = "pacs.example.org"
        ),
        detail,
        status: t!("work-eta-seconds", n = 50),
        fraction: Some(37.0 / 120.0),
        state: DownloadState::Running,
        stop: Some((t!("net-stop"), "stop:4".into())),
    }];
    let r = &rows[0];
    let name = t!(
        "net-chip-name",
        status = join(r.title.clone(), join(r.detail.clone(), r.status.clone()))
    );
    Downloads {
        rows,
        short: "31%".into(),
        name,
        fraction: Some(37.0 / 120.0),
    }
}

/// One model download only (the gallery's bottom-bar line).
pub(crate) fn one_download() -> Downloads {
    let mut d = downloads();
    d.rows.drain(..3);
    d.short = "38%".into();
    d.fraction = Some(45.0 / 120.0);
    d
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn net_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c(
            "downloads-inline-light",
            State::DownloadsInline,
            false,
            langid!("en-US"),
        ),
        c(
            "downloads-inline-he-dark",
            State::DownloadsInline,
            true,
            langid!("he"),
        ),
        c(
            "downloads-narrow-light",
            State::DownloadsNarrow,
            false,
            langid!("en-US"),
        ),
        c(
            "downloads-narrow-he-dark",
            State::DownloadsNarrow,
            true,
            langid!("he"),
        ),
        c(
            "downloads-hidden-bar-dark",
            State::DownloadsHiddenBar,
            true,
            langid!("en-US"),
        ),
        c(
            "downloads-panel-light",
            State::DownloadsPanel,
            false,
            langid!("en-US"),
        ),
        c(
            "downloads-panel-he-dark",
            State::DownloadsPanel,
            true,
            langid!("he"),
        ),
        c(
            "downloads-retrieve-light",
            State::DownloadsRetrieve,
            false,
            langid!("en-US"),
        ),
        c(
            "downloads-retrieve-he-light",
            State::DownloadsRetrieve,
            false,
            langid!("he"),
        ),
        c(
            "gallery-downloads-light",
            State::GalleryDownloads,
            false,
            langid!("en-US"),
        ),
        c(
            "gallery-downloads-he-dark",
            State::GalleryDownloads,
            true,
            langid!("he"),
        ),
    ]
}
