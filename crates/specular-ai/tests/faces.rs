//! People and Culling around the models: photos read through a decoder rather than the
//! thumbnail cache, and grouping faces into people (named people and the user's placements
//! kept). The networks themselves are checked in specular-infer (`tests/face.rs`).

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use common::{Dir, catalogue};
use specular_ai::Feature;
use specular_ai::faces::regroup;
use specular_ai::job::{Env, Image, Job, LibrarySource};
use specular_ai::runtime::{Dispatch, StubRuntime};
use specular_core::paths::PathSelector;
use specular_library::ai::{self, Face};
use specular_platform::MemoryPressure;
use specular_thumbs::DiskCache;

static SEL: PathSelector = PathSelector::new();

#[test]
fn photos_are_read_through_the_decoder_when_given() {
    let dir = Dir::new("faces-decode");
    let db = dir.path().join("library.sqlite3");
    // No thumbnails cached at all: only the decoder can give pixels.
    catalogue(&db, 6, |_| false);
    let mut conn = ai::open(&db).unwrap();
    ai::install(&mut conn, "people").unwrap();
    let cache = Arc::new(DiskCache::new(dir.path().join("thumbs"), 1 << 30));
    let asked = Arc::new(AtomicUsize::new(0));
    let a = asked.clone();
    let decode = Arc::new(move |p: &std::path::Path, side: u32| {
        assert!(p.is_absolute() || p.components().count() > 1, "{p:?}");
        assert_eq!(side, 128, "the model's side");
        a.fetch_add(1, Ordering::Relaxed);
        Some(Image {
            width: 16,
            height: 12,
            rgba: vec![200; 16 * 12 * 4],
        })
    });
    let source = LibrarySource::new(ai::open(&db).unwrap(), cache, 0, Feature::People, "faces-1")
        .with_decoder(decode);
    let stub = StubRuntime::new(Feature::People, "faces-1");
    let env = Env {
        browsing: Arc::new(|| false),
        machine: Arc::new(|| (Default::default(), MemoryPressure::Normal)),
        wake: Arc::new(|| {}),
        pause_poll: Duration::from_millis(5),
        throttle_ratio: 1,
        on_thread_start: None,
        switching: false,
    };
    let job = Job::start(Box::new(source), Dispatch::new(Arc::new(stub), &SEL), env);
    assert!(job.wait(Duration::from_secs(20)));
    let p = job.progress();
    assert_eq!((p.done, p.total), (6, 6), "{p:?}");
    assert_eq!(asked.load(Ordering::Relaxed), 6);
}

fn unit(v: &[f32]) -> Vec<u16> {
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    v.iter()
        .map(|x| half::f16::from_f32(x / n).to_bits())
        .collect()
}

#[test]
fn regrouping_keeps_names_and_placements() {
    let dir = Dir::new("faces-regroup");
    let db = dir.path().join("library.sqlite3");
    catalogue(&db, 8, |_| true);
    let mut c = ai::open(&db).unwrap();
    ai::install(&mut c, "people").unwrap();
    // Two people, A along x and B along y, three faces each over six files, plus a stranger.
    let face = |v: &[f32]| Face {
        x: 0.1,
        y: 0.1,
        w: 0.2,
        h: 0.2,
        score: 0.9,
        vec: unit(v),
        roll: 0.0,
    };
    let a = |k: f32| face(&[1.0, 0.05 * k, 0.0, 0.0]);
    let b = |k: f32| face(&[0.0, 1.0, 0.05 * k, 0.0]);
    for (file, f) in [
        (1, a(1.0)),
        (2, a(2.0)),
        (3, a(3.0)),
        (4, b(1.0)),
        (5, b(2.0)),
        (6, b(3.0)),
        (7, face(&[0.0, 0.0, 0.0, 1.0])),
    ] {
        ai::put_faces(&mut c, file, "m", &[f]).unwrap();
    }
    assert_eq!(regroup(&mut c, "m", 0.5).unwrap(), 7);
    let people = ai::people(&c).unwrap();
    assert_eq!(people.len(), 2, "{people:?}");
    assert!(people.iter().all(|p| p.faces == 3 && p.name.is_none()));
    let pa = ai::file_faces(&c, 1).unwrap()[0].person.unwrap();
    assert_eq!(ai::person_files(&c, pa).unwrap(), [1, 2, 3]);
    assert_eq!(
        ai::file_faces(&c, 7).unwrap()[0].person,
        None,
        "a stranger alone"
    );

    // Named, and one face taken from them: grouping again keeps both.
    ai::name_person(&c, pa, Some("Ana")).unwrap();
    let f3 = ai::file_faces(&c, 3).unwrap()[0].id;
    ai::place_face(&c, f3, None).unwrap();
    // A new photo of Ana joins her by her mean.
    ai::put_faces(&mut c, 8, "m", &[a(1.5)]).unwrap();
    regroup(&mut c, "m", 0.5).unwrap();
    assert_eq!(ai::person_files(&c, pa).unwrap(), [1, 2, 8]);
    assert_eq!(ai::file_faces(&c, 3).unwrap()[0].person, None);
    // Ids of unnamed groups stay put.
    let pb = ai::file_faces(&c, 4).unwrap()[0].person.unwrap();
    regroup(&mut c, "m", 0.5).unwrap();
    assert_eq!(ai::file_faces(&c, 5).unwrap()[0].person, Some(pb));
    // Faces of another model are not touched.
    assert_eq!(regroup(&mut c, "other", 0.5).unwrap(), 0);
}

/// Photos with an EXIF orientation, and photos turned on their side or upside down
/// (`SPECULAR_FACE_ROTATED`: tools/corpus/face-rotate.py's folder; `SPECULAR_FACE_MODELS`:
/// the BlazeFace download): the faces found are those of the upright photo, in the upright
/// photo's place (oriented pictures are decoded as the app's People worker decodes them,
/// through the thumbnail generator). With the face-free photos of
/// `tools/corpus/fetch-face-free.py` (walls, leaves, clouds…; `target/face-free`, or
/// `SPECULAR_FACE_FREE`), the faces found in them and in their three turns are counted:
/// false faces.
#[test]
fn real_faces_found_in_turned_and_exif_photos() {
    use specular_infer::face::blazeface::{Detector, Face, Range, iou};
    use specular_infer::face::{Image as FImage, detect_any_roll, unturn_point};
    let (Some(dir), Some(models)) = (
        std::env::var_os("SPECULAR_FACE_ROTATED").map(std::path::PathBuf::from),
        std::env::var_os("SPECULAR_FACE_MODELS").map(std::path::PathBuf::from),
    ) else {
        eprintln!("SPECULAR_FACE_ROTATED / SPECULAR_FACE_MODELS not set: skipped");
        return;
    };
    let blaze = std::fs::read(models.join("blaze_face_full_range.tflite")).unwrap();
    let mut det = Detector::new(&blaze, Range::Full, None, 8).unwrap();
    let reg = specular_decode::Registry::new();
    let decode = |p: &std::path::Path| {
        let src = specular_core::Source::open(p).unwrap();
        let s = reg.open_for_thumbnail(Arc::new(src)).unwrap();
        let opts = specular_thumbs::generate::GenOptions {
            prefer_embedded: false,
            ..Default::default()
        };
        specular_thumbs::generate::generate(
            &*s,
            1024,
            opts,
            &specular_core::CancelToken::new(),
            &mut |_| {},
        )
        .unwrap()
    };
    // Faces matched one to one at IoU ≥ 0.5: (matched, in a, in b).
    let matched = |a: &[Face], b: &[Face]| {
        let mut used = vec![false; b.len()];
        let mut m = 0;
        for f in a {
            if let Some(j) = (0..b.len())
                .filter(|&j| !used[j])
                .max_by(|&x, &y| iou(&f.rect, &b[x].rect).total_cmp(&iou(&f.rect, &b[y].rect)))
                .filter(|&j| iou(&f.rect, &b[j].rect) >= 0.5)
            {
                used[j] = true;
                m += 1;
            }
        }
        (m, a.len(), b.len())
    };
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".jpg") && !n.contains('-'))
        .collect();
    names.sort();
    let (mut exif, mut roll, mut upright_kept) = ([0usize; 3], [0usize; 3], 0usize);
    let (mut exif_all, mut roll_all, mut rolled_found) = (0usize, 0usize, [0usize; 3]);
    for n in &names {
        let base = n.trim_end_matches(".jpg");
        let up = decode(&dir.join(n));
        let upi = FImage::new(up.width as usize, up.height as usize, &up.rgba).unwrap();
        let want = det.detect(upi).unwrap();
        let any = detect_any_roll(&mut det, upi).unwrap();
        upright_kept += matched(&want, &any).0;
        for (k, tag) in [6, 3, 8].into_iter().enumerate() {
            let p = decode(&dir.join(format!("{base}-exif{tag}.jpg")));
            assert_eq!(
                (p.width, p.height),
                (up.width, up.height),
                "{base} exif {tag}"
            );
            let got = det
                .detect(FImage::new(p.width as usize, p.height as usize, &p.rgba).unwrap())
                .unwrap();
            let (m, _, _) = matched(&want, &got);
            exif[k] += m;
            exif_all += want.len();
        }
        for q in 1u8..=3 {
            let p = decode(&dir.join(format!("{base}-roll{q}.jpg")));
            let img = FImage::new(p.width as usize, p.height as usize, &p.rgba).unwrap();
            let upright_pass = det.detect(img).unwrap();
            let got = detect_any_roll(&mut det, img).unwrap();
            // Back in the upright photo's frame.
            let (w, h) = (up.width as f32, up.height as f32);
            let back: Vec<Face> = got
                .iter()
                .map(|f| {
                    let [x, y, fw, fh] = f.rect;
                    let c: Vec<[f32; 2]> = [[x, y], [x + fw, y + fh]]
                        .iter()
                        .map(|&pt| unturn_point(pt, q, w, h))
                        .collect();
                    let (x0, x1) = (c[0][0].min(c[1][0]), c[0][0].max(c[1][0]));
                    let (y0, y1) = (c[0][1].min(c[1][1]), c[0][1].max(c[1][1]));
                    Face {
                        rect: [x0, y0, x1 - x0, y1 - y0],
                        ..*f
                    }
                })
                .collect();
            let (m, _, _) = matched(&want, &back);
            roll[q as usize - 1] += m;
            roll_all += want.len();
            if upright_pass.is_empty() && !got.is_empty() {
                rolled_found[q as usize - 1] += 1;
            }
        }
    }
    let per = exif_all / 3;
    eprintln!(
        "{} photos, {per} faces upright; EXIF 6/3/8 same faces {}/{}/{}",
        names.len(),
        exif[0],
        exif[1],
        exif[2]
    );
    eprintln!(
        "turned ¼ / ½ / ¾: same faces {}/{}/{} of {per} (photos found only by the turned \
         passes {:?}); upright photos through the roll rule: {upright_kept}/{per} kept",
        roll[0], roll[1], roll[2], rolled_found
    );
    assert!(upright_kept * 100 >= per * 97, "{upright_kept}");
    assert!(per > 0);
    for e in exif {
        assert!(e * 100 >= per * 97, "EXIF: {e} of {per}");
    }
    for r in roll {
        assert!(r * 100 >= per * 90, "turned: {r} of {per}");
    }
    let _ = roll_all;

    // tools/corpus/fetch-face-free.py's folder.
    let free = std::env::var_os("SPECULAR_FACE_FREE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/face-free")
        });
    if !free.is_dir() {
        eprintln!(
            "{}: no face-free photos (tools/corpus/fetch-face-free.py): skipped",
            free.display()
        );
        return;
    }
    let mut names: Vec<std::path::PathBuf> = std::fs::read_dir(&free)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jpg"))
        .collect();
    names.sort();
    let (mut upright_false, mut any_false, mut pictures) = (0usize, 0usize, 0usize);
    for p in &names {
        let t = decode(p);
        let img = FImage::new(t.width as usize, t.height as usize, &t.rgba).unwrap();
        for q in 0u8..4 {
            let tt = specular_infer::face::turn(img, q);
            upright_false += det.detect(tt.view()).unwrap().len();
            any_false += detect_any_roll(&mut det, tt.view()).unwrap().len();
            pictures += 1;
        }
    }
    eprintln!(
        "face-free: {pictures} pictures ({} photos and their turns): false faces {upright_false} \
         upright only, {any_false} whatever the roll",
        names.len()
    );
    // The turned passes add no false faces to those the upright pass finds.
    assert!(any_false <= upright_false, "{any_false} vs {upright_false}");
}
