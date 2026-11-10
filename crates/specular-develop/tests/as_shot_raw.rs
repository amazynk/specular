//! As-shot white of non-DNG raws from the real samples in `target/raw-samples/` (read
//! only; skipped when absent): a body with a table matrix and recorded multipliers gives a
//! plausible daylight-to-tungsten illuminant, others keep the placeholder (`None`).

use std::path::PathBuf;

use specular_develop::preview::{self, Input};

#[test]
fn raw_samples_give_their_as_shot_white() {
    // The checkout's target folder (a lane worktree sits four levels below it).
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let dir = [
        std::env::var_os("SPECULAR_RAW_SAMPLES").map(PathBuf::from),
        Some(root.join("../../target/raw-samples")),
        Some(root.join("../../../../../target/raw-samples")),
    ]
    .into_iter()
    .flatten()
    .find(|d| d.is_dir());
    let Some(Ok(rd)) = dir.map(std::fs::read_dir) else {
        eprintln!("no raw samples: skipped");
        return;
    };
    for e in rd.flatten() {
        let p = e.path();
        let ext = p
            .extension()
            .map(|x| x.to_string_lossy().to_ascii_lowercase())
            .unwrap_or_default();
        if matches!(ext.as_str(), "xmp" | "md" | "dng") {
            continue;
        }
        let data = std::fs::read(&p).unwrap();
        let Ok(Input::Raw(s)) = preview::decode_raw(&data) else {
            eprintln!("{}: not developed", p.display());
            continue;
        };
        let info = specular_raw::parse(&data).unwrap().info().clone();
        let xy = preview::as_shot_white_of(&s, &data);
        eprintln!(
            "{} ({} {}): wb {:?} ({:?}) → {xy:?}",
            p.display(),
            info.make,
            info.model,
            s.meta.wb,
            s.meta.wb_source
        );
        match (
            xy,
            specular_raw::as_shot_xy(&info.make, &info.model, s.meta.wb),
        ) {
            (Some(xy), Some(direct)) => {
                assert_eq!(xy, direct);
                // Between tungsten and shade.
                assert!((0.28..0.47).contains(&xy[0]), "{xy:?}");
            }
            (None, direct) => assert!(
                direct.is_none() || s.meta.wb_source != specular_raw::WbSource::AsShot,
                "{direct:?}"
            ),
            (Some(_), None) => panic!("as-shot white without a table matrix"),
        }
    }
}
