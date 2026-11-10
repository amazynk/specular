//! Test commands of the remote control, compiled into debug builds only (the real-use runs,
//! `tools/realuse/`):
//!
//! * `idle`: answered once the window has settled (the app gate's rule, `frame_gate.rs`):
//!   nothing drawn for `quiet_ms`, no frame asked for, no timer set, no tiles or decodes on
//!   their way, the current image sharp or failed, the gallery's visible tiles in, and the
//!   idle-time calibration done. After `timeout_ms` it answers `"settled": false` with what
//!   still held it.
//! * `ui-state`: what is open, read from the app rather than the screen: the mode, Develop,
//!   cull, MPR, the gallery and the Library (with the indexer's counts), background jobs, and
//!   from the last accessibility tree sent the focused control and every dialog, alert and
//!   status line (toasts) and every slider with its role, text and bounds, and Develop's
//!   preview while dragging (`develop_drag`). The tree exists only while a screen reader
//!   (or the runs' AT-SPI bus) listens.

use std::time::{Duration, Instant};

use serde_json::{Value, json};
use specular_widgets::accesskit::Role;

use super::App;
use crate::remote::protocol::Reply;
use crate::remote::server::Call;

/// How often a waiting `idle` looks again.
const POLL: Duration = Duration::from_millis(50);
/// The most labels `ui-state` lists.
const TEXTS: usize = 600;

/// An `idle` call waiting for the window to settle.
pub(super) struct IdleWait {
    call: Call,
    asked: Instant,
    quiet: Duration,
    until: Instant,
}

impl App {
    /// Takes an `idle` call: answered now when settled, else by [`App::test_idle_tick`].
    pub(super) fn test_idle_wait(&mut self, call: Call) {
        let crate::remote::protocol::Request::Idle {
            quiet_ms,
            timeout_ms,
        } = call.req
        else {
            return;
        };
        let now = Instant::now();
        self.remote.idle.push(IdleWait {
            call,
            asked: now,
            quiet: Duration::from_millis(quiet_ms),
            until: now + Duration::from_millis(timeout_ms),
        });
        self.test_idle_tick();
    }

    /// Answers the `idle` calls whose wait is over (settled, or out of time).
    pub(super) fn test_idle_tick(&mut self) {
        if self.remote.idle.is_empty() {
            return;
        }
        let now = Instant::now();
        let waits = std::mem::take(&mut self.remote.idle);
        for w in waits {
            let held = self.test_holding(w.quiet);
            if held.is_empty() || now >= w.until {
                let r: Reply = Ok(json!({
                    "settled": held.is_empty(),
                    "waited_ms": w.asked.elapsed().as_millis() as u64,
                    "held_by": held,
                }));
                w.call.answer(r);
            } else {
                self.remote.idle.push(w);
            }
        }
    }

    /// When a waiting `idle` looks again.
    pub(super) fn test_idle_wake(&self) -> Option<Instant> {
        (!self.remote.idle.is_empty()).then(|| Instant::now() + POLL)
    }

    /// What keeps the window from being settled (empty: settled).
    fn test_holding(&self, quiet: Duration) -> Vec<&'static str> {
        let mut h = Vec::new();
        if self.last_frame.elapsed() < quiet {
            h.push("drawing");
        }
        if self.redraw {
            h.push("frame-asked");
        }
        if self.wake_at.is_some() {
            h.push("timer");
        }
        if !self.gate_settled() {
            h.push("loading");
        }
        if self.gallery_active() && !self.gallery_settled() {
            h.push("gallery");
        }
        if super::performance::calibrating() {
            h.push("calibrating");
        }
        h
    }

    /// The `ui-state` answer.
    pub(super) fn test_ui_state(&self) -> Value {
        let jobs: Vec<Value> = specular_core::work::snapshot()
            .iter()
            .map(|j| {
                json!({
                    "kind": format!("{:?}", j.kind).to_lowercase(),
                    "step": format!("{:?}", j.step).to_lowercase(),
                    "done": j.done,
                    "total": j.total,
                    "paused": j.paused.is_some(),
                })
            })
            .collect();
        let (focus, dialogs, alerts, texts, sliders) = self.test_a11y();
        json!({
            "gallery": self.gallery_active(),
            "gallery_dir": self.gallery_dir().map(|p| p.to_string_lossy()),
            "gallery_images": if self.gallery_active() { self.gallery_images().len() } else { 0 },
            "develop": self.develop_active(),
            "develop_file": self.develop_open_edit().map(|(p, _)| p.to_string_lossy().into_owned()),
            "develop_drag": self.develop_drag_state(),
            "cull": self.cull_dir().map(|p| p.to_string_lossy()),
            "mpr": self.mpr_active(),
            "library": self.library_test_state(),
            "jobs": jobs,
            "accessibility": self.a11y.as_ref().is_some_and(|a| a.is_active()),
            "focus": focus,
            "dialogs": dialogs,
            "alerts": alerts,
            "texts": texts,
            "sliders": sliders,
        })
    }

    /// From the last accessibility tree sent: the focused node, the dialogs, the alerts and
    /// status lines, and every label and description line (at most [`TEXTS`]), for a run to
    /// look for a message.
    fn test_a11y(&self) -> (Value, Vec<Value>, Vec<Value>, Vec<String>, Vec<Value>) {
        let Some((nodes, focus)) = self.a11y.as_ref().and_then(|a| a.last()) else {
            return (Value::Null, Vec::new(), Vec::new(), Vec::new(), Vec::new());
        };
        let texts: Vec<String> = nodes
            .iter()
            .flat_map(|(_, n)| {
                n.label()
                    .into_iter()
                    .chain(n.description().into_iter().flat_map(str::lines))
            })
            .filter(|l| !l.is_empty())
            .take(TEXTS)
            .map(str::to_owned)
            .collect();
        let node = |id, n: &specular_widgets::accesskit::Node| {
            json!({
                "id": id,
                "role": format!("{:?}", n.role()),
                "label": n.label(),
                "value": n.value(),
                "bounds": n.bounds().map(|b| [b.x0, b.y0, b.x1, b.y1]),
            })
        };
        let mut f = Value::Null;
        let mut dialogs = Vec::new();
        let mut alerts = Vec::new();
        let mut sliders = Vec::new();
        for (id, n) in nodes {
            if *id == focus {
                f = node(id.0, n);
            }
            match n.role() {
                Role::Dialog | Role::AlertDialog => dialogs.push(node(id.0, n)),
                Role::Alert | Role::Status | Role::Log => alerts.push(node(id.0, n)),
                Role::Slider => sliders.push(node(id.0, n)),
                _ => {}
            }
        }
        (f, dialogs, alerts, texts, sliders)
    }
}
