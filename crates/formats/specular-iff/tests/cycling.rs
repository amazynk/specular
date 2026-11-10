//! Colour cycling over real files (`tools/corpus/fetch-image-variants.sh iff`: TwelveMonkeys'
//! IFF test files, Deluxe Paint pictures with `CRNG` and `DRNG` ranges):
//!
//! ```text
//! SPECULAR_IFF_CORPUS=<dir> cargo test -p specular-iff --test cycling -- --ignored --nocapture
//! ```
//!
//! Per file: frame 0 draws exactly the still; every frame moves only the colours of entries
//! inside a cycling range; each frame shows for a positive time.

use std::path::PathBuf;

use specular_iff::{cycling, decode, decode_with_cmap, info};

#[test]
#[ignore = "needs the IFF corpus"]
fn real_files_cycle() {
    let dir = std::env::var_os("SPECULAR_IFF_CORPUS")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("CARGO_TARGET_DIR").map(|t| PathBuf::from(t).join("corpus/iff"))
        });
    let Some(dir) = dir.filter(|d| d.is_dir()) else {
        eprintln!("skipped: no IFF corpus");
        return;
    };
    let mut names: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    names.sort();
    let mut animated = 0;
    for p in names {
        let data = std::fs::read(&p).unwrap();
        let Ok(i) = info(&data) else { continue };
        if !i.colour_cycling {
            continue;
        }
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let Some(frames) = cycling(&data).unwrap() else {
            println!("{name}: ranges, none active");
            continue;
        };
        animated += 1;
        let still = decode(&data).unwrap();
        assert_eq!(
            decode_with_cmap(&data, &frames[0].cmap).unwrap(),
            still,
            "{name}"
        );
        assert!(frames.iter().all(|f| f.delay_ms > 0), "{name}");
        let last = decode_with_cmap(&data, &frames[frames.len() - 1].cmap).unwrap();
        assert_eq!((last.width, last.height), (still.width, still.height));
        let changed = frames[0]
            .cmap
            .chunks(3)
            .zip(frames[1].cmap.chunks(3))
            .filter(|(a, b)| a != b)
            .count();
        println!(
            "{name}: {} frames, first {} ms, {changed} entries move in the first step",
            frames.len(),
            frames[0].delay_ms
        );
    }
    println!("{animated} animated");
}
