//! The frame on screen saved as a PNG or copied, at the movie's full size: decoded again on
//! a worker from the movie (the frames on screen may be at a reduced scale), through the
//! same RGBA conversion and clipboard the viewer's Copy uses.

use std::path::{Path, PathBuf};
use std::sync::{Arc, mpsc};

use specular_core::CancelToken;
use specular_decode::Session;
use specular_i18n::t;

use super::super::App;

/// What a save or copy came to.
pub(crate) enum Done {
    Saved(PathBuf),
    Copied,
    Failed(String),
}

/// The default name of a saved frame: `<video> <time>.png` (`Film 0-12-34.png`).
pub(crate) fn default_name(video: &Path, secs: f64) -> String {
    let stem = video
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let s = secs.max(0.0) as u64;
    let (h, m, sec) = (s / 3600, s / 60 % 60, s % 60);
    // Colons are not allowed in Windows file names.
    let time = if h > 0 {
        format!("{h}-{m:02}-{sec:02}")
    } else {
        format!("{m}-{sec:02}")
    };
    format!("{stem} {time}.png")
}

/// Frame `index` of `session` at full size as straight RGBA.
pub(crate) fn decode(
    session: &Arc<dyn Session>,
    index: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let mut frames = session
        .frames(1)
        .map_err(|e| crate::error_text::reason(&e))?;
    if index > 0
        && !frames
            .seek(index)
            .map_err(|e| crate::error_text::reason(&e))?
    {
        frames.rewind().map_err(|e| crate::error_text::reason(&e))?;
        for _ in 0..index {
            match frames.next_frame(&CancelToken::new()) {
                Some(Ok(_)) => {}
                Some(Err(e)) => return Err(crate::error_text::reason(&e)),
                None => break,
            }
        }
    }
    let f = frames
        .next_frame(&CancelToken::new())
        .ok_or_else(|| t!("error-reason-internal"))?
        .map_err(|e| crate::error_text::reason(&e))?;
    let rgba = crate::clipboard::to_rgba(&f.raster);
    Ok((f.raster.width, f.raster.height, rgba))
}

impl App {
    /// What to decode: the movie's session, the frame on screen and its time.
    fn frame_target(&self) -> Option<(Arc<dyn Session>, u32, f64, PathBuf)> {
        let a = self.anim.as_ref().filter(|a| a.movie.is_some())?;
        let img = self.images.get(&a.id)?;
        let s = img.session.clone()?;
        let path = s.source().path().to_path_buf();
        let t = a
            .time(std::time::Instant::now())
            .map_or(0.0, |d| d.as_secs_f64());
        Some((s, a.frame()?, t, path))
    }

    /// Ctrl+s on a movie: the frame on screen as a PNG, where the user says.
    pub(in crate::app) fn frame_save(&mut self) {
        let Some((session, index, t, path)) = self.frame_target() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.player.frame_rx = Some(rx);
        let name = default_name(&path, t);
        let dir = path.parent().map(Path::to_path_buf);
        let title = t!("player-save-frame-title");
        self.loader.background("dlg-save-frame", move |l| {
            let mut d = rfd::FileDialog::new()
                .set_title(&title)
                .add_filter("PNG", &["png"])
                .set_file_name(&name);
            if let Some(dir) = dir {
                d = d.set_directory(dir);
            }
            let Some(out) = d.save_file() else { return };
            let r = decode(&session, index).and_then(|(w, h, rgba)| {
                let png = specular_png::encode(w, h, specular_png::ColorType::Rgba, &rgba)
                    .map_err(|e| e.to_string())?;
                // Beside and renamed: a failed write never leaves half a picture.
                let tmp = out.with_extension("png.part");
                std::fs::write(&tmp, &png)
                    .and_then(|()| std::fs::rename(&tmp, &out))
                    .map_err(|e| {
                        let _ = std::fs::remove_file(&tmp);
                        crate::error_text::io_reason(&e)
                    })
            });
            let _ = tx.send(match r {
                Ok(()) => Done::Saved(out),
                Err(e) => Done::Failed(e),
            });
            l.waker()();
        });
    }

    /// Ctrl+c on a movie: the frame on screen, at full size, on the clipboard.
    pub(in crate::app) fn frame_copy(&mut self) {
        let Some((session, index, _, _)) = self.frame_target() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        self.player.frame_rx = Some(rx);
        let clip = self.clipboard.clone();
        self.loader.background("clip-frame", move |l| {
            match decode(&session, index) {
                Ok((w, h, rgba)) => {
                    clip.put_rgba(
                        l,
                        w as usize,
                        h as usize,
                        rgba,
                        crate::loader::Copied::Image,
                    );
                    let _ = tx.send(Done::Copied);
                }
                Err(e) => {
                    let _ = tx.send(Done::Failed(e));
                }
            }
            l.waker()();
        });
    }

    /// Says how a save or copy went (a copy says itself through the clipboard's toast).
    pub(in crate::app) fn frame_poll(&mut self) {
        let Some(rx) = &self.player.frame_rx else {
            return;
        };
        let Ok(done) = rx.try_recv() else { return };
        self.player.frame_rx = None;
        let now = self.now();
        match done {
            Done::Saved(p) => {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                self.chrome
                    .toast(t!("toast-frame-saved", file = name), None, now);
            }
            Done::Copied => {}
            Done::Failed(why) => {
                self.chrome
                    .toast(t!("toast-frame-failed", reason = why), None, now);
            }
        }
        self.redraw = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_names() {
        assert_eq!(
            default_name(Path::new("/v/Film.mkv"), 754.9),
            "Film 12-34.png"
        );
        assert_eq!(
            default_name(Path::new("Trip.mp4"), 3723.0),
            "Trip 1-02-03.png"
        );
    }
}
