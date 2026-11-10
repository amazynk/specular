//! Reading the text in the Library's photos for `text:` searches ([`super::text_search`]):
//! one low-priority `ai-text` worker, running only while the Text feature is installed and
//! switched on and the user turned "Read text in this library's photos" on for the library
//! (off by default, like grouping faces). Each photo is decoded (at most 24 MP), read by the
//! installed models (`specular_ai::ocr`) and its lines stored in the catalogue's `ocr_text`
//! under the models' key, with what the reading found (scripts, the add-ons that read lines,
//! lines it couldn't read). A new size is a new key: every photo is read again; a script
//! add-on installed or removed re-reads only the photos it matters to
//! ([`specular_ai::ocr::reading_still_good`]): those with lines it read, or with its script
//! or unreadable lines when added; the rest are re-keyed. The orientation classifier, once
//! installed, and PP-OCRv5 running instead of v6 (the key names the detector that runs)
//! re-read only the photos with unreadable or unsure lines. It waits while the user is busy
//! (a frame drawn in the last moments) and between photos.
//! Nothing here runs on the UI thread.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use specular_ai::ocr::{DetPick, KeyPart, Ocr, OcrFiles, TextLine};
use specular_ai::{Feature, Pack};
use specular_core::CancelToken;
use specular_decode::DecodeRequest;
use specular_gfx::wgpu;

use super::device::Session;

/// How long the worker sleeps when it has nothing to do.
const IDLE: Duration = Duration::from_secs(5);
/// Between photos (it never takes the machine over).
const BETWEEN: Duration = Duration::from_millis(150);
/// Photos are read at most this big.
const MAX_PIXELS: u64 = 24_000_000;

static STARTED: AtomicBool = AtomicBool::new(false);
static ON: AtomicBool = AtomicBool::new(false);

/// Text's switch, as the app has it.
pub(crate) fn set_on(on: bool) {
    ON.store(on, Ordering::Relaxed);
}

/// Starts the worker once (it lives as long as the app).
pub(crate) fn start(
    pack: Arc<Pack>,
    gpu: Option<(wgpu::Device, wgpu::Queue)>,
    wake: Arc<dyn Fn() + Send + Sync>,
) {
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let _ = specular_thread::builder("ai-text").spawn(move || run(&pack, gpu, &*wake));
}

/// The key the catalogue stores a reading under: the models that read it. The detector
/// that runs (`det`: PP-OCRv5 where it stands in for v6), the size's recogniser, the
/// orientation classifier when it runs (`orient`), then the add-ons.
fn model_key(files: &OcrFiles, det: DetPick, orient: bool) -> String {
    let det = match (det, &files.detector_fallback) {
        (DetPick::V5 { .. }, Some((w, _))) => dir_name(w),
        _ => dir_name(&files.detector),
    };
    let mut parts = vec![det, rec_part(&files.main)];
    if let Some((w, _)) = files.orientation.as_ref().filter(|_| orient) {
        parts.push(dir_name(w));
    }
    parts.extend(files.addons.iter().map(|(_, r)| rec_part(r)));
    parts.join("+")
}

/// A model file's folder name (its id and version): its part of the key.
fn dir_name(p: &std::path::Path) -> String {
    p.parent()
        .and_then(|d| d.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn rec_part(r: &specular_ai::ocr::RecFiles) -> String {
    match r {
        specular_ai::ocr::RecFiles::Paddle { weights, .. } => dir_name(weights),
        specular_ai::ocr::RecFiles::Tesseract(p) => dir_name(p),
    }
}

/// An installed add-on's part of the key, by add-on id.
fn addon_part(files: &OcrFiles, id: &str) -> Option<String> {
    files
        .addons
        .iter()
        .find(|(a, _)| a == id)
        .map(|(_, r)| rec_part(r))
}

/// What a part of a key is: an installed detector, the orientation classifier, or an add-on
/// with the scripts it reads (none when it isn't installed).
fn key_part(files: &OcrFiles, part: &str) -> KeyPart {
    let fallback = files.detector_fallback.as_ref().map(|(w, _)| w);
    if std::iter::once(&files.detector)
        .chain(fallback)
        .any(|w| dir_name(w) == part)
    {
        return KeyPart::Detector;
    }
    if files
        .orientation
        .as_ref()
        .is_some_and(|(w, _)| dir_name(w) == part)
    {
        return KeyPart::Orientation;
    }
    KeyPart::Addon(
        files
            .addons
            .iter()
            .find(|(_, r)| rec_part(r) == part)
            .map_or(&[], |(a, _)| specular_ai::ocr::addon_scripts(a)),
    )
}

/// The lines as the catalogue keeps them: the text (one line each), and the boxes as JSON.
fn stored(lines: &[TextLine]) -> (String, String) {
    let text = lines
        .iter()
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let boxes: Vec<String> = lines
        .iter()
        .map(|l| {
            let q: Vec<String> = l
                .quad
                .iter()
                .flat_map(|p| [format!("{:.1}", p[0]), format!("{:.1}", p[1])])
                .collect();
            format!("[{}]", q.join(","))
        })
        .collect();
    (text, format!("[{}]", boxes.join(",")))
}

fn run(pack: &Pack, gpu: Option<(wgpu::Device, wgpu::Queue)>, wake: &(dyn Fn() + Send + Sync)) {
    let mut loaded: Option<(OcrFiles, Ocr)> = None;
    let mut session = Session::new(gpu);
    let mut after = 0i64;
    // The key readings were last re-keyed to.
    let mut rekeyed = String::new();
    loop {
        let files = pack.ocr_files();
        super::text_search::set_ready(files.is_some());
        let wanted = ON.load(Ordering::Relaxed)
            && files.is_some()
            && pack.text_indexing() == Some(true)
            && pack.catalogue().is_some();
        if !wanted || super::index::busy() {
            if !wanted {
                loaded = None;
            }
            std::thread::sleep(IDLE);
            continue;
        }
        let files = files.expect("checked above");
        // After a GPU reset the models are loaded again on the new device.
        if session.stale() {
            loaded = None;
        }
        if loaded.as_ref().is_none_or(|(f, _)| *f != files) {
            let ctx = session.context();
            match Ocr::load(&files, ctx, specular_core::paths::selector(), 4) {
                Ok(o) => loaded = Some((files.clone(), o)),
                Err(e) => {
                    specular_log::warn!(target: specular_log::JOBS, "ai: text models: {e}");
                    std::thread::sleep(IDLE * 12);
                    continue;
                }
            }
        }
        let Some((files, ocr)) = loaded.as_mut() else {
            continue;
        };
        let key = model_key(files, ocr.detector(), ocr.orientation());
        let Some(path) = pack.catalogue() else {
            continue;
        };
        let Ok(mut conn) = specular_library::ai::open(path) else {
            std::thread::sleep(IDLE);
            continue;
        };
        if !specular_library::ai::has(&conn, Feature::Text.id()).unwrap_or(false) {
            std::thread::sleep(IDLE);
            continue;
        }
        if rekeyed != key {
            // The tables up to date (the `scripts` column), then readings an add-on change
            // doesn't touch kept under the new key.
            let kept =
                specular_library::ai::install(&mut conn, Feature::Text.id()).and_then(|_| {
                    specular_library::ai::rekey_text(&mut conn, &key, &|old, tokens| {
                        specular_ai::ocr::reading_still_good(old, &key, tokens, &|p| {
                            key_part(files, p)
                        })
                    })
                });
            match kept {
                Ok(n) => {
                    if n > 0 {
                        specular_log::info!(target: specular_log::JOBS,
                            "ai: text: {n} photos' readings kept for the new add-ons");
                    }
                    rekeyed = key.clone();
                }
                Err(e) => specular_log::warn!(target: specular_log::JOBS, "ai: text re-key: {e}"),
            }
        }
        let pending = specular_library::ai::pending(&conn, Feature::Text.id(), &key, after, 8)
            .unwrap_or_default();
        if pending.is_empty() {
            after = 0;
            drop(conn);
            std::thread::sleep(IDLE * 6);
            continue;
        }
        for p in pending {
            let before = after;
            after = p.file_id;
            if !ON.load(Ordering::Relaxed) || super::index::busy() {
                break;
            }
            let lines = specular_library::ai::file_path(&conn, p.file_id)
                .ok()
                .flatten()
                .and_then(|f| read_file(ocr, &f))
                .unwrap_or_default();
            if session.stale() {
                // The device went during the reading: not stored, read again.
                after = before;
                break;
            }
            let (text, boxes) = stored(&lines);
            let tokens = specular_ai::ocr::reading_tokens(&lines, ocr.unreadable(), &|id| {
                addon_part(files, id)
            });
            if let Err(e) =
                specular_library::ai::put_text(&conn, p.file_id, &key, &text, &boxes, &tokens)
            {
                specular_log::warn!(target: specular_log::JOBS, "ai: storing text: {e}");
                break;
            }
            wake();
            std::thread::sleep(BETWEEN);
        }
    }
}

/// Reads one file's text (None: it could not be decoded; stored as no text).
fn read_file(ocr: &mut Ocr, path: &std::path::Path) -> Option<Vec<TextLine>> {
    let session = specular_decode::Registry::new().open_path(path).ok()?;
    let info = session.info();
    let px = u64::from(info.width) * u64::from(info.height);
    let mut scale = 1u32;
    while px / u64::from(scale * scale) > MAX_PIXELS && scale < 16 {
        scale *= 2;
    }
    let r = session
        .decode(&DecodeRequest::full(scale), &CancelToken::new())
        .ok()?;
    let rgba = crate::clipboard::to_rgba(&r);
    ocr.read(r.width as usize, r.height as usize, &rgba, &|| false)
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stored_text_and_boxes() {
        let l = |t: &str, x: f32| TextLine {
            quad: [[x, 0.0], [x + 10.0, 0.0], [x + 10.0, 5.0], [x, 5.0]],
            text: t.into(),
            confidence: 0.9,
            addon: None,
        };
        let (t, b) = stored(&[l("Exit", 0.0), l("יציאה", 20.0)]);
        assert_eq!(t, "Exit\nיציאה");
        assert_eq!(
            b,
            "[[0.0,0.0,10.0,0.0,10.0,5.0,0.0,5.0],[20.0,0.0,30.0,0.0,30.0,5.0,20.0,5.0]]"
        );
    }

    fn files(v5: bool, orient: bool) -> OcrFiles {
        let p = |d: &str, f: &str| std::path::PathBuf::from(format!("/m/{d}/{f}"));
        OcrFiles {
            detector: p("det-v6-1", "w.onnx"),
            detector_config: p("det-v6-1", "c.yml"),
            detector_fallback: v5.then(|| (p("det-v5-1", "w.onnx"), p("det-v5-1", "c.yml"))),
            orientation: orient.then(|| (p("orient-1", "w.onnx"), p("orient-1", "c.yml"))),
            main: specular_ai::ocr::RecFiles::Paddle {
                weights: p("rec-1", "w.onnx"),
                config: p("rec-1", "c.yml"),
                chars: p("rec-1", "k.txt"),
            },
            addons: vec![(
                "arabic".into(),
                specular_ai::ocr::RecFiles::Paddle {
                    weights: p("ar-arabic-1", "w.onnx"),
                    config: p("ar-arabic-1", "c.yml"),
                    chars: p("ar-arabic-1", "k.txt"),
                },
            )],
        }
    }

    #[test]
    fn key_names_the_detector_and_classifier_that_run() {
        let v6 = DetPick::V6 { gpu: true };
        let v5 = DetPick::V5 { gpu: false };
        assert_eq!(
            model_key(&files(false, false), v6, false),
            "det-v6-1+rec-1+ar-arabic-1"
        );
        // v5 installed but v6 still runs: the same key, nothing read again.
        assert_eq!(
            model_key(&files(true, false), v6, false),
            "det-v6-1+rec-1+ar-arabic-1"
        );
        assert_eq!(
            model_key(&files(true, false), v5, false),
            "det-v5-1+rec-1+ar-arabic-1"
        );
        // The classifier counts once it runs.
        assert_eq!(
            model_key(&files(false, true), v6, false),
            "det-v6-1+rec-1+ar-arabic-1"
        );
        assert_eq!(
            model_key(&files(false, true), v6, true),
            "det-v6-1+rec-1+orient-1+ar-arabic-1"
        );
        let f = files(false, true);
        assert_eq!(key_part(&f, "orient-1"), KeyPart::Orientation);
        assert_eq!(key_part(&f, "ar-arabic-1"), KeyPart::Addon(&["arab"]));
        assert_eq!(key_part(&f, "gone-1"), KeyPart::Addon(&[]));
        assert_eq!(key_part(&f, "det-v6-1"), KeyPart::Detector);
        assert_eq!(key_part(&f, "det-v5-1"), KeyPart::Addon(&[]));
        assert_eq!(key_part(&files(true, true), "det-v5-1"), KeyPart::Detector);
    }

    /// Readings stored before the classifier or PP-OCRv5 came: only photos with unreadable
    /// or unsure lines are read again, the rest keep their reading under the new key.
    #[test]
    fn classifier_or_v5_installed_re_reads_only_unsure_photos() {
        use specular_library::ai;
        let dir = std::env::temp_dir().join(format!("specular-text-rekey-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut conn = ai::open(&dir.join("lib.db")).unwrap();
        conn.execute_batch(
            "INSERT INTO folders(id, path, added) VALUES (1, '/p', 0);
             INSERT INTO dirs(id, folder_id, path) VALUES (1, 1, '/p');
             INSERT INTO files(id, dir_id, name, ext, size, mtime_ns, state) VALUES
                (1, 1, 'a.jpg', 'jpg', 10, 100, 1), (2, 1, 'b.jpg', 'jpg', 20, 200, 1),
                (3, 1, 'c.jpg', 'jpg', 30, 300, 1), (4, 1, 'd.jpg', 'jpg', 40, 400, 1),
                (5, 1, 'e.jpg', 'jpg', 50, 500, 1);
             INSERT INTO thumbs(file_id, content, classes) VALUES
                (1, 11, 1), (2, 22, 1), (3, 33, 1), (4, 44, 1), (5, 55, 1);",
        )
        .unwrap();
        ai::install(&mut conn, Feature::Text.id()).unwrap();
        let v6 = DetPick::V6 { gpu: true };
        let old = model_key(&files(false, false), v6, false);
        for (id, tokens) in [
            (1, ""),
            (2, "latn"),
            (3, "?"),
            (4, "arab addon:ar-arabic-1"),
            (5, "latn low"),
        ] {
            ai::put_text(&conn, id, &old, "", "[]", tokens).unwrap();
        }
        let rekey = |conn: &mut rusqlite::Connection, f: &OcrFiles, key: &str| {
            ai::rekey_text(conn, key, &|o, t| {
                specular_ai::ocr::reading_still_good(o, key, t, &|p| key_part(f, p))
            })
            .unwrap()
        };
        let pending = |conn: &rusqlite::Connection, key: &str| -> Vec<i64> {
            ai::pending(conn, Feature::Text.id(), key, 0, 10)
                .unwrap()
                .iter()
                .map(|p| p.file_id)
                .collect()
        };
        // v5 installed, v6 still runs: the key stays, nothing pends.
        assert_eq!(model_key(&files(true, false), v6, false), old);
        let f = files(true, true);
        let key = model_key(&f, v6, true);
        assert_eq!(rekey(&mut conn, &f, &key), 3);
        assert_eq!(pending(&conn, &key), [3, 5]);
        // Read again (now surely, say); then v5 runs instead (the GPU check failed): only
        // the photos still unsure.
        ai::put_text(&conn, 3, &key, "", "[]", "latn").unwrap();
        ai::put_text(&conn, 5, &key, "", "[]", "latn ?").unwrap();
        let key5 = model_key(&f, DetPick::V5 { gpu: false }, true);
        assert_eq!(rekey(&mut conn, &f, &key5), 4);
        assert_eq!(pending(&conn, &key5), [5]);
        // v5 removed while it ran: what it read (or re-keyed) is read again; photo 5, still
        // under the v6 key, stays.
        let f6 = files(false, true);
        let key6 = model_key(&f6, v6, true);
        assert_eq!(rekey(&mut conn, &f6, &key6), 0);
        assert_eq!(pending(&conn, &key6), [1, 2, 3, 4]);
        drop(conn);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
