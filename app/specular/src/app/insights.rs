//! Performance insights (DESIGN.md §12.7): the "Why is this slow?" indicator, in the viewer
//! for slow opens and in the gallery for slow thumbnails, the panel it and the command palette
//! open, and the panel's buttons.

use specular_i18n::t;
use specular_settings::Origin;
use specular_widgets::{Insights, PaletteItem, PanelEvent};

use super::App;
use crate::open_report::{self, insights as ins, thumbs};

/// The palette item "Performance insights".
pub(super) const ITEM: u64 = 1 << 42;

/// What the indicator stands for.
#[derive(Debug, Default)]
pub(crate) struct InsightState {
    /// The pattern key of the opens' indicator, if it is up (shown in the viewer).
    shown: Option<String>,
    /// The pattern key of the thumbnails' indicator, if it is up (shown in the gallery).
    thumbs_shown: Option<String>,
    /// What the chrome shows now: whether it is about thumbnails, and the pattern key.
    up: Option<(bool, String)>,
    /// The newest generated thumbnail could not be kept in the thumbnail cache.
    no_thumb_cache: bool,
}

/// Settings → Performance → "Show hidden insights again".
pub(super) const RESTORE: &str = "performance.insights_restore";

impl App {
    /// Greys out "Show hidden insights again" in Settings while no insight is hidden (it
    /// would do nothing); cheap, so it runs every turn Settings is open and after a hide.
    pub(super) fn sync_restore_setting(&mut self) {
        if self
            .chrome
            .set_setting_disabled(RESTORE, !open_report::any_hidden())
        {
            self.redraw = true;
        }
    }

    pub(super) fn insight_items(&self) -> Vec<PaletteItem> {
        vec![PaletteItem {
            id: ITEM,
            label: t!("open-report-insights-title"),
            key: None,
            unavailable: None,
            icon: specular_gfx::Icon::Info,
        }]
    }

    /// The panel's content now: in the gallery about thumbnails, when there is something to
    /// say about them, else about opens.
    fn insights_now(&self) -> Insights {
        let p = ins::Prefs::of(&self.settings);
        let mut cx = ins::Context {
            hidden: !open_report::hidden(false).is_empty(),
            saving_power: specular_platform::power_state().saving(),
            ..ins::Context::default()
        };
        if self.gallery.active {
            let (advice, _, last_slow) = thumbs::advice();
            let a = ins::pick(&advice, last_slow.as_ref(), p);
            if a.is_some() || last_slow.is_some() || self.insights.no_thumb_cache {
                cx.thumbnails = true;
                cx.hidden = !open_report::hidden(true).is_empty();
                cx.no_thumb_cache = self.insights.no_thumb_cache;
                return ins::build(a.as_ref(), last_slow.as_ref(), &self.settings, cx);
            }
        }
        let (advice, _, last_slow) = open_report::advice();
        let a = ins::pick(&advice, last_slow.as_ref(), p);
        ins::build(a.as_ref(), last_slow.as_ref(), &self.settings, cx)
    }

    /// The panel's content as plain text, title first (for the hardware report).
    pub(super) fn insights_text(&self) -> String {
        ins::text(&self.insights_now())
    }

    /// Opens the panel (the palette, the indicator, `--show insights`).
    pub(super) fn open_insights(&mut self) {
        let i = self.insights_now();
        self.chrome.show_insights(i);
        self.redraw = true;
    }

    /// Thumbnails the gallery's engine generated: into the thumbnails' advisor, and the
    /// indicator follows.
    pub(super) fn thumb_timings(&mut self, timings: Vec<specular_thumbs::Timing>) {
        let Some(newest) = timings.last() else {
            return;
        };
        self.insights.no_thumb_cache = !newest.cached;
        let probed = specular_hwdec::probed().is_some();
        for t in &timings {
            // A page of an archive (`<archive>/<entry>`), read through the engine.
            let in_archive = t.path.ancestors().any(crate::book::is_archive_path);
            thumbs::remember(&thumbs::timeline(t, in_archive, probed));
        }
        self.refresh_insights();
    }

    /// After a report, a settings change, a button or entering or leaving the gallery: the
    /// indicator comes or goes, and the open panel follows.
    pub(super) fn refresh_insights(&mut self) {
        let p = ins::Prefs::of(&self.settings);
        self.sync_restore_setting();
        let st = &mut self.insights;
        let (advice, last, _) = open_report::advice();
        st.shown = ins::indicator(
            st.shown.as_deref(),
            &advice,
            last.as_ref(),
            p,
            &open_report::hidden(false),
            open_report::SLOW_MS,
        );
        let (advice, last, _) = thumbs::advice();
        st.thumbs_shown = ins::indicator(
            st.thumbs_shown.as_deref(),
            &advice,
            last.as_ref(),
            p,
            &open_report::hidden(true),
            thumbs::SLOW_MS,
        );
        let up = if self.gallery.active {
            st.thumbs_shown.clone().map(|k| (true, k))
        } else {
            st.shown.clone().map(|k| (false, k))
        };
        if up != st.up {
            // Another kind of indicator is announced anew.
            if up.as_ref().map(|u| u.0) != st.up.as_ref().map(|u| u.0) {
                self.chrome.set_insight_indicator(None);
            }
            let ind = up.as_ref().map(|(thumbnails, _)| {
                (
                    t!("open-report-insights-indicator"),
                    if *thumbnails {
                        t!("open-report-insights-announce-thumbnails")
                    } else {
                        t!("open-report-insights-announce")
                    },
                )
            });
            st.up = up;
            self.chrome.set_insight_indicator(ind);
            self.redraw = true;
        }
        if self.chrome.insights_open() {
            let i = self.insights_now();
            self.chrome.set_insights(i);
            self.redraw = true;
        }
    }

    /// A button of the indicator or the panel.
    pub(super) fn insight_command(&mut self, key: &str) {
        match key {
            "open" => self.open_insights(),
            "hide" => {
                // Hidden for good (kept with the advisor's history) until shown again, in
                // the context it was hidden in: the gallery's thumbnails or the viewer's opens.
                if let Some((thumbnails, k)) = self.insights.up.take() {
                    open_report::hide(&k, thumbnails);
                    if thumbnails {
                        self.insights.thumbs_shown = None;
                    } else {
                        self.insights.shown = None;
                    }
                }
                self.chrome.set_insight_indicator(None);
                self.redraw = true;
            }
            // The panel's: its own context (the gallery's thumbnails or the viewer's opens).
            "unhide:thumbnails" => open_report::unhide(Some(true)),
            "unhide:opens" => open_report::unhide(Some(false)),
            // Settings': everything.
            "unhide" => open_report::unhide(None),
            k if k.starts_with("retry:") => {
                self.paths_panel_event(&PanelEvent::Row(k.to_owned()));
            }
            k => {
                let mut ok = false;
                self.store
                    .update(Origin::Program, |s| ok = ins::apply_setting(k, s));
                if !ok {
                    specular_log::debug!(target: specular_log::UI, "insights: unknown button {k}");
                }
                // Now, so the panel and the indicator follow at once.
                self.apply_settings(false);
            }
        }
        self.refresh_insights();
    }
}
