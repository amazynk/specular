//! The print thread: owns a [`PageRenderer`] on a clone of the app's GPU and answers
//! preview and PDF requests, newest first.

use std::io::{self, BufWriter};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::Instant;

use specular_gfx::{Gpu, Text};

use crate::layout::Document;
use crate::pdf::Meta;
use crate::render::{ImageSource, PageRenderer, RenderError};

/// Applied to every preview's RGBA before it is sent (the soft proof of the page as the
/// printer will print it).
pub type PreviewFilter = Arc<dyn Fn(&mut [u8]) + Send + Sync>;

/// What the thread is asked to do.
enum Request {
    Source(Arc<dyn ImageSource>),
    Filter(Option<PreviewFilter>),
    Preview {
        job: u64,
        doc: Arc<Document>,
        page: usize,
        long_px: u32,
    },
    Pdf {
        job: u64,
        doc: Arc<Document>,
        meta: Meta,
        path: PathBuf,
    },
}

/// A finished PDF.
#[derive(Clone, Debug, PartialEq)]
pub struct PdfDone {
    pub path: PathBuf,
    pub pages: usize,
    pub bytes: u64,
    /// Images that could not be decoded.
    pub missing: usize,
    pub seconds: f64,
}

/// What the thread answers.
#[derive(Debug)]
pub enum Reply {
    Preview {
        job: u64,
        page: usize,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    },
    PreviewFailed {
        job: u64,
        error: String,
    },
    /// `done` of `total` pages written.
    Progress {
        job: u64,
        done: usize,
        total: usize,
    },
    Pdf {
        job: u64,
        result: Result<PdfDone, String>,
    },
    /// The job was cancelled (its file removed).
    Cancelled {
        job: u64,
    },
}

/// The handle the app keeps.
pub struct Worker {
    tx: Sender<Request>,
    rx: Receiver<Reply>,
    /// Requests older than this are abandoned.
    live: Arc<AtomicU64>,
    job: u64,
}

impl std::fmt::Debug for Worker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Worker").field("job", &self.job).finish()
    }
}

impl Worker {
    /// Starts the thread. `bundled_fonts` limits captions to the bundled fonts (tests);
    /// `wake` is called after every reply.
    pub fn spawn(
        gpu: Gpu,
        bundled_fonts: bool,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> io::Result<Worker> {
        let (tx, req_rx) = channel::<Request>();
        let (reply_tx, rx) = channel::<Reply>();
        let live = Arc::new(AtomicU64::new(0));
        let live2 = live.clone();
        specular_thread::builder("print-render").spawn(move || {
            let text = if bundled_fonts {
                Text::bundled_only()
            } else {
                Text::new()
            };
            run(gpu, text, &req_rx, &reply_tx, &live2, &*wake);
        })?;
        Ok(Worker {
            tx,
            rx,
            live,
            job: 0,
        })
    }

    fn next(&mut self) -> u64 {
        self.job += 1;
        self.live.store(self.job, Ordering::Relaxed);
        self.job
    }

    /// The images of the following requests.
    pub fn set_source(&mut self, source: Arc<dyn ImageSource>) {
        let _ = self.tx.send(Request::Source(source));
    }

    /// The filter the following previews go through (`None`: as rendered).
    pub fn set_preview_filter(&mut self, filter: Option<PreviewFilter>) {
        let _ = self.tx.send(Request::Filter(filter));
    }

    /// Asks for page `page` with a long side of `long_px`; older requests are abandoned.
    pub fn preview(&mut self, doc: Arc<Document>, page: usize, long_px: u32) -> u64 {
        let job = self.next();
        let _ = self.tx.send(Request::Preview {
            job,
            doc,
            page,
            long_px,
        });
        job
    }

    /// Asks for the whole document as a PDF at `path`; older requests are abandoned.
    pub fn pdf(&mut self, doc: Arc<Document>, meta: Meta, path: PathBuf) -> u64 {
        let job = self.next();
        let _ = self.tx.send(Request::Pdf {
            job,
            doc,
            meta,
            path,
        });
        job
    }

    /// Abandons whatever is running or queued.
    pub fn cancel(&mut self) {
        self.next();
    }

    /// The next reply, if any.
    pub fn try_recv(&self) -> Option<Reply> {
        self.rx.try_recv().ok()
    }

    /// The next reply, waiting for it (tests and benchmarks).
    pub fn recv(&self) -> Option<Reply> {
        self.rx.recv().ok()
    }
}

fn run(
    gpu: Gpu,
    text: Text,
    rx: &Receiver<Request>,
    tx: &Sender<Reply>,
    live: &AtomicU64,
    wake: &dyn Fn(),
) {
    let mut renderer: Option<PageRenderer> = None;
    let mut source: Option<Arc<dyn ImageSource>> = None;
    let mut filter: Option<PreviewFilter> = None;
    let mut text = Some(text);
    let send = |r: Reply| {
        let _ = tx.send(r);
        wake();
    };
    while let Ok(req) = rx.recv() {
        let job = match &req {
            Request::Source(_) | Request::Filter(_) => 0,
            Request::Preview { job, .. } | Request::Pdf { job, .. } => *job,
        };
        let stale = |g: u64| g < live.load(Ordering::Relaxed);
        if job != 0 && stale(job) {
            continue;
        }
        let cancelled = || stale(job);
        match req {
            Request::Source(s) => {
                if let Some(r) = renderer.as_mut() {
                    r.clear_cache();
                }
                source = Some(s);
            }
            Request::Filter(f) => filter = f,
            Request::Preview {
                job,
                doc,
                page,
                long_px,
            } => {
                let Some(src) = source.clone() else { continue };
                let r = renderer.get_or_insert_with(|| {
                    PageRenderer::new(gpu.clone(), text.take().unwrap_or_default())
                });
                if page >= doc.pages.len() {
                    continue;
                }
                match r.render_rgba(
                    &doc,
                    page,
                    crate::render::preview_dpi(&doc.setup, long_px),
                    &*src,
                    &cancelled,
                ) {
                    Ok((width, height, mut rgba)) => {
                        if let Some(f) = &filter {
                            f(&mut rgba);
                        }
                        send(Reply::Preview {
                            job,
                            page,
                            width,
                            height,
                            rgba,
                        })
                    }
                    Err(RenderError::Cancelled) => {}
                    Err(e) => send(Reply::PreviewFailed {
                        job,
                        error: e.to_string(),
                    }),
                }
            }
            Request::Pdf {
                job,
                doc,
                meta,
                path,
            } => {
                let Some(src) = source.clone() else { continue };
                let r = renderer.get_or_insert_with(|| {
                    PageRenderer::new(gpu.clone(), text.take().unwrap_or_default())
                });
                let t0 = Instant::now();
                let total = doc.pages.len();
                let result = std::fs::File::create(&path)
                    .map_err(RenderError::Io)
                    .and_then(|f| {
                        r.write_pdf(
                            &doc,
                            &meta,
                            BufWriter::with_capacity(1 << 20, f),
                            &*src,
                            &mut |done| send(Reply::Progress { job, done, total }),
                            &cancelled,
                        )
                    });
                match result {
                    Ok((out, stats, missing)) => {
                        let flushed = out.into_inner().map_err(|e| e.into_error());
                        send(Reply::Pdf {
                            job,
                            result: flushed
                                .map(|_| PdfDone {
                                    path,
                                    pages: stats.pages,
                                    bytes: stats.bytes,
                                    missing,
                                    seconds: t0.elapsed().as_secs_f64(),
                                })
                                .map_err(|e| e.to_string()),
                        });
                    }
                    Err(RenderError::Cancelled) => {
                        let _ = std::fs::remove_file(&path);
                        send(Reply::Cancelled { job });
                    }
                    Err(RenderError::Io(e)) => {
                        let _ = std::fs::remove_file(&path);
                        send(Reply::Pdf {
                            job,
                            result: Err(e.to_string()),
                        });
                    }
                }
                // Free the GPU memory of a large document's images.
                r.clear_cache();
            }
        }
    }
}
