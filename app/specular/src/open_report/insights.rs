//! Performance insights (DESIGN.md §12.7): which of the advisor's advice is shown (the
//! switches in Settings → Performance, "Don't show again" per kind and the insights hidden
//! with the indicator's ×), and the panel's localised content: the last slow open's (or
//! thumbnail's) stages, the bottleneck in plain language, what can be done now, then which
//! hardware would help. `--hw-report` prints the same as text.

use specular_core::paths::{Engine, Task};
use specular_i18n::t;
use specular_settings::{PathMode, PathOverride, Settings};
use specular_widgets::{InsightItem, InsightStage, Insights};

use super::advisor::{Advice, Change, Pattern};
use super::{
    Hint, Software, Timeline, Verdict, bears_on, gain_text, hint_text, num, pattern_text,
    software_text, speed, stage_name,
};

/// A kind of slowdown, switched off on its own with "Don't show again".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Reading the files (slow storage, archives).
    Storage,
    /// Decoding on the processor although a hardware decoder could.
    Decoder,
}

impl Kind {
    pub fn of(p: &Pattern) -> Option<Kind> {
        match p {
            Pattern::Software { .. } => Some(Kind::Decoder),
            Pattern::SlowStorage { .. } => Some(Kind::Storage),
            // Nothing the user can change: logged only.
            Pattern::FirstOpen { .. } => None,
        }
    }
}

/// The insights switches (Settings → Performance).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub on: bool,
    pub storage: bool,
    pub decoder: bool,
}

impl Prefs {
    pub fn of(s: &Settings) -> Prefs {
        Prefs {
            on: s.performance.insights,
            storage: s.performance.insights_storage,
            decoder: s.performance.insights_decoder,
        }
    }

    fn allows(self, k: Kind) -> bool {
        match k {
            Kind::Storage => self.storage,
            Kind::Decoder => self.decoder,
        }
    }
}

/// Whether `a` may be shown at all: marked advice of a kind not switched off (the global
/// switch aside: the panel opened from the palette shows it anyway).
fn allowed(a: &Advice, p: Prefs) -> bool {
    a.marked && Kind::of(&a.pattern).is_some_and(|k| p.allows(k))
}

/// The advice the panel is about: of those allowed, the one that bears on the last slow open,
/// else the first.
pub fn pick(
    advice: &[Advice],
    last_slow: Option<&(Timeline, Verdict)>,
    p: Prefs,
) -> Option<Advice> {
    let ok: Vec<&Advice> = advice.iter().filter(|a| allowed(a, p)).collect();
    ok.iter()
        .find(|a| last_slow.is_some_and(|(t, v)| bears_on(a, t, v)))
        .or(ok.first())
        .map(|a| **a)
}

/// The indicator decision after a report: the pattern key it stands for, if it shows.
///
/// It stays up while its pattern is still current advice (and allowed, and not hidden). It
/// comes up only after a slow report (from `slow_ms`: an open's or a thumbnail's threshold)
/// that a marked, allowed pattern bears on (a pattern, not a one-off), when insights are on
/// and the user has not hidden that pattern with the indicator's ×.
pub fn indicator(
    current: Option<&str>,
    advice: &[Advice],
    last: Option<&(Timeline, Verdict)>,
    p: Prefs,
    hidden: &[String],
    slow_ms: f64,
) -> Option<String> {
    if !p.on {
        return None;
    }
    let ok: Vec<&Advice> = advice
        .iter()
        .filter(|a| allowed(a, p) && !hidden.contains(&a.pattern.key()))
        .collect();
    if let Some(k) = current
        && ok.iter().any(|a| a.pattern.key() == k)
    {
        return Some(k.to_owned());
    }
    let (t, v) = last?;
    if !super::advisor::waited(t, v, slow_ms) {
        return None;
    }
    ok.iter()
        .find(|a| bears_on(a, t, v))
        .map(|a| a.pattern.key())
}

/// What the panel is about besides the advice.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Context {
    /// About the gallery's thumbnails rather than opens.
    pub thumbnails: bool,
    /// Thumbnails cannot be kept on disk (no thumbnail cache): each is made again next time.
    pub no_thumb_cache: bool,
    /// Some insights are hidden in this context (thumbnails or opens): offer to show them
    /// again.
    pub hidden: bool,
    /// The computer is saving power (on battery, a power-saving mode, or hot).
    pub saving_power: bool,
}

/// The task whose hardware decoder the pattern is about.
fn task_of(format: &str, last: Option<&(Timeline, Verdict)>) -> Option<Task> {
    last.filter(|(t, _)| t.format == format)
        .and_then(|(t, _)| t.decode.and_then(|d| d.path).map(|p| p.task))
        .or(match format {
            "JPEG" => Some(Task::JpegDecode),
            "HEIC" => Some(Task::HeicDecode),
            _ => None,
        })
}

/// The panel's content: about `a` (none: no recurring slowdown), with the last slow open (or
/// thumbnail, per `cx`).
pub fn build(
    a: Option<&Advice>,
    last_slow: Option<&(Timeline, Verdict)>,
    s: &Settings,
    cx: Context,
) -> Insights {
    let p = Prefs::of(s);
    let thumbs = cx.thumbnails;
    let mut out = Insights {
        switch: Some(if p.on {
            (t!("open-report-insights-off"), "off".to_owned())
        } else {
            (t!("open-report-insights-on"), "on".to_owned())
        }),
        // Only this context's: the gallery's panel brings back hidden thumbnail insights,
        // the viewer's hidden open insights (Settings brings back both).
        restore: cx.hidden.then(|| {
            let key = if thumbs {
                "unhide:thumbnails"
            } else {
                "unhide:opens"
            };
            (t!("open-report-insights-restore"), key.to_owned())
        }),
        ..Insights::default()
    };
    if let Some((t, v)) = last_slow {
        let ms = |x: f64| t!("unit-milliseconds", value = num(x));
        let total = v.total_ms.max(1.0);
        let main = v.bottleneck.map(|b| b.0);
        let stages = t
            .stages()
            .into_iter()
            .filter(|(_, x)| *x >= 0.5 && *x >= total * 0.01)
            .map(|(stage, x)| InsightStage {
                name: stage_name(stage),
                ms: x,
                value: ms(x),
                main: Some(stage) == main,
            })
            .collect();
        out.last = Some((
            if thumbs {
                t!(
                    "open-report-insights-last-thumbnail",
                    total = ms(v.total_ms)
                )
            } else {
                t!("open-report-insights-last", total = ms(v.total_ms))
            },
            stages,
        ));
    }
    let Some(a) = a else {
        out.bottleneck = t!("open-report-insights-none");
        out.detail = t!("open-report-insights-none-detail");
        if thumbs && cx.no_thumb_cache {
            out.now.push(thumb_cache_item());
        }
        return out;
    };
    let gain = a.gain.map(gain_text);
    let pattern = if thumbs {
        thumbs_pattern_text(&a.pattern)
    } else {
        pattern_text(&a.pattern)
    };
    match a.pattern {
        Pattern::Software { format, why, .. } => {
            out.bottleneck = if thumbs {
                t!("open-report-insights-thumbnails-software", format = format)
            } else {
                t!("open-report-insights-software", format = format)
            };
            out.detail = [pattern, Some(why_text(why))]
                .into_iter()
                .flatten()
                .collect::<Vec<_>>()
                .join(": ");
            match a.change {
                Some(Change::Hint(h)) => {
                    let action = match h {
                        Hint::TurnOn => Some((
                            t!("open-report-insights-action-turn-on"),
                            "set:hardware-decode".to_owned(),
                        )),
                        Hint::Automatic => Some((
                            t!("open-report-insights-action-automatic"),
                            "set:processing".to_owned(),
                        )),
                        Hint::TryAgain => task_of(format, last_slow).map(|task| {
                            (
                                t!("open-report-insights-action-try-again"),
                                format!("retry:{}:{}", task.key(), Engine::HwDecoder.key()),
                            )
                        }),
                        _ => None,
                    };
                    out.now.push(InsightItem {
                        text: hint_text(h),
                        gain: gain.clone(),
                        action,
                    });
                }
                Some(Change::HardwareDecoder { codec }) => out.hardware.push(InsightItem {
                    text: t!("open-report-advice-hardware-decoder", codec = codec),
                    gain: gain.clone(),
                    action: None,
                }),
                None => {}
            }
            // Decoding on the processor is slower while it saves power.
            if cx.saving_power {
                out.now.push(InsightItem {
                    text: t!("open-report-insights-power"),
                    gain: None,
                    action: None,
                });
            }
        }
        Pattern::SlowStorage { archive, .. } => {
            out.bottleneck = match (archive, thumbs) {
                (true, false) => t!("open-report-insights-archive"),
                (false, false) => t!("open-report-insights-storage"),
                (true, true) => t!("open-report-insights-thumbnails-archive"),
                (false, true) => t!("open-report-insights-thumbnails-storage"),
            };
            out.detail = pattern.unwrap_or_default();
            let h = if archive {
                Hint::Extract
            } else {
                Hint::CopyLocal
            };
            out.now.push(InsightItem {
                text: hint_text(h),
                gain: gain.clone(),
                action: None,
            });
            if !archive {
                out.hardware.push(InsightItem {
                    text: t!("open-report-insights-hw-ssd"),
                    gain: gain.clone(),
                    action: None,
                });
            }
            // Prefetching is the viewer's: thumbnails are read ahead of the view anyway.
            if !thumbs && s.performance.prefetch_depth < 2 {
                out.now.push(InsightItem {
                    text: t!("open-report-insights-prefetch"),
                    gain: None,
                    action: Some((
                        t!("open-report-insights-action-turn-on"),
                        "set:prefetch".to_owned(),
                    )),
                });
            }
        }
        Pattern::FirstOpen { .. } => {}
    }
    if thumbs {
        // The display cache is the viewer's; thumbnails have a cache of their own.
        if cx.no_thumb_cache {
            out.now.push(thumb_cache_item());
        }
    } else if !s.performance.display_cache {
        out.now.push(InsightItem {
            text: t!("open-report-insights-display-cache"),
            gain: None,
            action: Some((
                t!("open-report-insights-action-turn-on"),
                "set:display-cache".to_owned(),
            )),
        });
    }
    if out.now.is_empty() {
        out.now.push(InsightItem {
            text: t!("open-report-insights-nothing-now"),
            gain: None,
            action: None,
        });
    }
    out.dismiss = Kind::of(&a.pattern).map(|k| match k {
        Kind::Storage => (
            t!("open-report-insights-dismiss-storage"),
            "dismiss:storage".to_owned(),
        ),
        Kind::Decoder => (
            t!("open-report-insights-dismiss-decoder"),
            "dismiss:decoder".to_owned(),
        ),
    });
    out
}

/// "The thumbnail cache is not available": thumbnails are made again each time.
fn thumb_cache_item() -> InsightItem {
    InsightItem {
        text: t!("open-report-insights-thumbnail-cache"),
        gain: None,
        action: None,
    }
}

/// [`pattern_text`] for thumbnails ("5 of the last 6 slow thumbnails…").
fn thumbs_pattern_text(p: &Pattern) -> Option<String> {
    let count = |n: usize| i64::try_from(n).unwrap_or(i64::MAX);
    Some(match *p {
        Pattern::Software {
            format,
            count: n,
            of,
            ..
        } => t!(
            "open-report-advice-thumbnails-software",
            count = count(n),
            total = count(of),
            format = format
        ),
        Pattern::SlowStorage {
            archive: true,
            count: n,
            of,
            ..
        } => t!(
            "open-report-advice-thumbnails-archive",
            count = count(n),
            total = count(of)
        ),
        Pattern::SlowStorage {
            mb_s,
            bytes,
            count: n,
            of,
            ..
        } => match speed(bytes, mb_s) {
            Some(speed) => t!(
                "open-report-advice-thumbnails-slow-storage",
                count = count(n),
                total = count(of),
                speed = speed
            ),
            None => t!(
                "open-report-advice-thumbnails-slow-storage-no-speed",
                count = count(n),
                total = count(of)
            ),
        },
        Pattern::FirstOpen { .. } => return None,
    })
}

/// Why the decode ran in software, without the "Decoded in software:" lead the headline
/// already gives.
fn why_text(why: Software) -> String {
    let s = software_text(why);
    match s.split_once(": ") {
        Some((_, rest)) if !rest.is_empty() => rest.to_owned(),
        _ => s,
    }
}

/// What a button of the panel changes in the settings ("set:…"), if it is one of those.
pub fn apply_setting(key: &str, s: &mut Settings) -> bool {
    match key {
        "set:hardware-decode" => s.performance.hardware_decode = PathOverride::Auto,
        "set:processing" => s.performance.processing = PathMode::Automatic,
        "set:display-cache" => s.performance.display_cache = true,
        "set:prefetch" => s.performance.prefetch_depth = s.performance.prefetch_depth.max(2),
        "dismiss:storage" => s.performance.insights_storage = false,
        "dismiss:decoder" => s.performance.insights_decoder = false,
        "off" => s.performance.insights = false,
        "on" => s.performance.insights = true,
        _ => return false,
    }
    true
}

/// `--hw-report`: the insights from the past week's slow opens kept in `kept` (read only).
pub fn report(kept: &[(Timeline, Verdict)], s: &Settings) -> String {
    let mut h = super::advisor::History::new();
    h.seed(kept.iter().copied());
    let advice = h.advise();
    let last_slow = h.last_slow().copied();
    let a = pick(&advice, last_slow.as_ref(), Prefs::of(s));
    let cx = Context {
        saving_power: specular_platform::power_state().saving(),
        ..Context::default()
    };
    text(&build(a.as_ref(), last_slow.as_ref(), s, cx))
}

/// The panel as plain text (`--hw-report`), without the bidi isolation marks.
pub fn text(i: &Insights) -> String {
    let mut out = String::new();
    let mut line = |s: &str| {
        out.push_str(s);
        out.push('\n');
    };
    line(&t!("open-report-insights-title"));
    line(&format!("  {}", i.bottleneck));
    if !i.detail.is_empty() {
        line(&format!("  {}", i.detail));
    }
    if let Some((caption, stages)) = &i.last {
        line(&format!("  {caption}"));
        for s in stages {
            line(&format!(
                "    {}{}: {}",
                if s.main { "* " } else { "" },
                s.name,
                s.value
            ));
        }
    }
    for (heading, items) in [
        (t!("open-report-insights-now"), &i.now),
        (t!("open-report-insights-hardware"), &i.hardware),
    ] {
        if items.is_empty() {
            continue;
        }
        line(&format!("  {heading}"));
        for it in items {
            line(&format!("    - {}", it.text));
            if let Some(g) = &it.gain {
                line(&format!("      {g}"));
            }
        }
    }
    specular_i18n::strip_isolates(&out)
}

#[cfg(test)]
mod tests {
    use super::super::advisor::{Gain, Pattern};
    use super::*;

    const SLOW: f64 = super::super::SLOW_MS;

    fn software(marked: bool) -> Advice {
        Advice {
            pattern: Pattern::Software {
                format: "HEIC",
                why: Software::NoDevice { api: "VA-API" },
                count: 6,
                of: 6,
            },
            change: Some(Change::HardwareDecoder { codec: "HEVC" }),
            gain: Some(Gain {
                low: 3.0,
                high: 6.0,
            }),
            marked,
        }
    }

    fn storage() -> Advice {
        Advice {
            pattern: Pattern::SlowStorage {
                mb_s: 40.0,
                bytes: 8_000_000,
                archive: false,
                count: 5,
                of: 5,
            },
            change: Some(Change::Hint(Hint::CopyLocal)),
            gain: Some(Gain {
                low: 2.5,
                high: 8.0,
            }),
            marked: true,
        }
    }

    const ALL_ON: Prefs = Prefs {
        on: true,
        storage: true,
        decoder: true,
    };

    /// A slow HEIC open decoded in software (no device), as the pattern says.
    fn slow_heic() -> (Timeline, Verdict) {
        let t = Timeline {
            format: "HEIC",
            read_ms: 30.0,
            decode: Some(super::super::DecodeStage {
                scale: 1,
                ms: 820.0,
                ..Default::default()
            }),
            sharp_ms: 900.0,
            ..Timeline::default()
        };
        let v = Verdict {
            total_ms: 900.0,
            bottleneck: Some((super::super::Stage::Decode, 90)),
            finding: super::super::Finding::Software(Software::NoDevice { api: "VA-API" }),
            hint: None,
            software: Some(Software::NoDevice { api: "VA-API" }),
        };
        (t, v)
    }

    fn fast() -> (Timeline, Verdict) {
        let t = Timeline {
            format: "HEIC",
            sharp_ms: 40.0,
            ..Timeline::default()
        };
        let v = Verdict {
            total_ms: 40.0,
            bottleneck: Some((super::super::Stage::Decode, 90)),
            finding: super::super::Finding::Fast,
            hint: None,
            software: Some(Software::NoDevice { api: "VA-API" }),
        };
        (t, v)
    }

    #[test]
    fn the_indicator_needs_a_marked_pattern_and_a_slow_open_it_bears_on() {
        let key = software(true).pattern.key();
        let slow = slow_heic();
        // Not marked: nothing.
        assert_eq!(
            indicator(None, &[software(false)], Some(&slow), ALL_ON, &[], SLOW),
            None
        );
        // Marked, after a slow open like it: up.
        assert_eq!(
            indicator(None, &[software(true)], Some(&slow), ALL_ON, &[], SLOW),
            Some(key.clone())
        );
        // After a fast open it does not come up, but once up it stays.
        assert_eq!(
            indicator(None, &[software(true)], Some(&fast()), ALL_ON, &[], SLOW),
            None
        );
        assert_eq!(
            indicator(
                Some(&key),
                &[software(true)],
                Some(&fast()),
                ALL_ON,
                &[],
                SLOW
            ),
            Some(key.clone())
        );
        // It goes when the pattern is no longer current.
        assert_eq!(
            indicator(Some(&key), &[storage()], Some(&fast()), ALL_ON, &[], SLOW),
            None
        );
        // A slow open that reading held up does not bring up the decoder pattern.
        let mut read = slow;
        read.1.bottleneck = Some((super::super::Stage::Read, 80));
        read.1.software = None;
        assert_eq!(
            indicator(None, &[software(true)], Some(&read), ALL_ON, &[], SLOW),
            None
        );
    }

    #[test]
    fn hidden_dismissed_and_switched_off_insights_stay_away() {
        let key = software(true).pattern.key();
        let slow = slow_heic();
        let advice = [software(true)];
        // Hidden this session (the indicator's ×).
        assert_eq!(
            indicator(
                None,
                &advice,
                Some(&slow),
                ALL_ON,
                std::slice::from_ref(&key),
                SLOW
            ),
            None
        );
        // "Don't show again" for decoding.
        let no_decoder = Prefs {
            decoder: false,
            ..ALL_ON
        };
        assert_eq!(
            indicator(None, &advice, Some(&slow), no_decoder, &[], SLOW),
            None
        );
        assert_eq!(
            indicator(Some(&key), &advice, Some(&slow), no_decoder, &[], SLOW),
            None
        );
        assert_eq!(pick(&advice, Some(&slow), no_decoder), None);
        // Storage advice is still shown then.
        assert_eq!(
            pick(&[software(true), storage()], Some(&slow), no_decoder).map(|a| a.pattern.key()),
            Some("slow-storage".to_owned())
        );
        // The global switch.
        let off = Prefs {
            on: false,
            ..ALL_ON
        };
        assert_eq!(
            indicator(Some(&key), &advice, Some(&slow), off, &[], SLOW),
            None
        );
        // The panel (from the palette) still explains the pattern.
        assert!(pick(&advice, Some(&slow), off).is_some());
    }

    #[test]
    fn the_panel_prefers_the_pattern_of_the_last_slow_open() {
        let slow = slow_heic();
        let both = [storage(), software(true)];
        assert_eq!(
            pick(&both, Some(&slow), ALL_ON).map(|a| a.pattern.key()),
            Some(software(true).pattern.key())
        );
        assert_eq!(
            pick(&both, None, ALL_ON).map(|a| a.pattern.key()),
            Some("slow-storage".to_owned())
        );
    }

    #[test]
    fn buttons_change_the_settings_they_name() {
        let mut s = Settings::default();
        s.performance.display_cache = false;
        s.performance.hardware_decode = PathOverride::Off;
        for k in [
            "set:display-cache",
            "set:hardware-decode",
            "dismiss:storage",
            "off",
        ] {
            assert!(apply_setting(k, &mut s), "{k}");
        }
        assert!(s.performance.display_cache);
        assert_eq!(s.performance.hardware_decode, PathOverride::Auto);
        assert!(!s.performance.insights_storage);
        assert!(s.performance.insights_decoder);
        assert!(!s.performance.insights);
        assert!(!apply_setting("retry:heic-decode:hw-decoder", &mut s));
    }

    #[test]
    fn the_panel_puts_what_can_be_done_now_before_hardware() {
        let s = Settings::default();
        let slow = slow_heic();
        let i = build(Some(&software(true)), Some(&slow), &s, Context::default());
        assert!(!i.bottleneck.is_empty());
        // Nothing to change in the settings for a missing decoder: said so, then the hardware.
        assert_eq!(i.now.len(), 1);
        assert!(i.now[0].action.is_none());
        assert_eq!(i.hardware.len(), 1);
        assert!(i.hardware[0].gain.is_some());
        assert_eq!(
            i.dismiss.as_ref().map(|d| d.1.as_str()),
            Some("dismiss:decoder")
        );
        let (_, stages) = i.last.as_ref().unwrap();
        assert!(stages.iter().any(|s| s.main));
        let text = text(&i);
        assert!(text.contains(&specular_i18n::strip_isolates(&i.bottleneck)));

        let mut s = Settings::default();
        s.performance.display_cache = false;
        let i = build(Some(&storage()), None, &s, Context::default());
        assert_eq!(i.now.len(), 2);
        assert!(i.now[0].gain.is_some());
        assert_eq!(
            i.now[1].action.as_ref().map(|a| a.1.as_str()),
            Some("set:display-cache")
        );
        assert_eq!(i.hardware.len(), 1);

        let i = build(None, None, &Settings::default(), Context::default());
        assert!(i.now.is_empty() && i.hardware.is_empty() && i.dismiss.is_none());
        assert!(i.switch.is_some());
    }

    #[test]
    fn a_hidden_insight_goes_and_stays_away_until_shown_again() {
        let key = software(true).pattern.key();
        let slow = slow_heic();
        let advice = [software(true)];
        let hidden = [key.clone()];
        // Up, then hidden: it goes although the pattern is still current.
        assert_eq!(
            indicator(Some(&key), &advice, Some(&slow), ALL_ON, &hidden, SLOW),
            None
        );
        assert_eq!(
            indicator(None, &advice, Some(&slow), ALL_ON, &hidden, SLOW),
            None
        );
        // Shown again: it comes back after the next slow open like it.
        assert_eq!(
            indicator(None, &advice, Some(&slow), ALL_ON, &[], SLOW),
            Some(key)
        );
        // The panel offers to show hidden insights again only when some are.
        let s = Settings::default();
        let cx = Context {
            hidden: true,
            ..Context::default()
        };
        let i = build(Some(&software(true)), Some(&slow), &s, cx);
        assert_eq!(i.restore.map(|r| r.1).as_deref(), Some("unhide:opens"));
        // The gallery's panel brings back only hidden thumbnail insights.
        let cx = Context {
            hidden: true,
            thumbnails: true,
            ..Context::default()
        };
        let i = build(None, None, &s, cx);
        assert_eq!(i.restore.map(|r| r.1).as_deref(), Some("unhide:thumbnails"));
        assert!(build(None, None, &s, Context::default()).restore.is_none());
    }

    #[test]
    fn thumbnails_have_their_own_threshold_words_and_remedies() {
        // A 200 ms thumbnail held up reading: slow for a thumbnail, not for an open.
        let t = Timeline {
            format: "JPEG",
            bytes: 8_000_000,
            read_ms: 190.0,
            sharp_ms: 200.0,
            ..Timeline::default()
        };
        let last = (t, super::super::verdict(&t));
        let advice = [storage()];
        assert_eq!(
            indicator(None, &advice, Some(&last), ALL_ON, &[], SLOW),
            None
        );
        assert_eq!(
            indicator(
                None,
                &advice,
                Some(&last),
                ALL_ON,
                &[],
                super::super::thumbs::SLOW_MS
            ),
            Some("slow-storage".to_owned())
        );
        let mut s = Settings::default();
        s.performance.display_cache = false;
        s.performance.prefetch_depth = 0;
        let opens = build(Some(&storage()), Some(&last), &s, Context::default());
        let cx = Context {
            thumbnails: true,
            no_thumb_cache: true,
            ..Context::default()
        };
        let thumbs = build(Some(&storage()), Some(&last), &s, cx);
        assert_ne!(opens.bottleneck, thumbs.bottleneck);
        assert_ne!(opens.detail, thumbs.detail);
        assert_ne!(
            opens.last.as_ref().unwrap().0,
            thumbs.last.as_ref().unwrap().0
        );
        // The viewer's prefetch and display cache are not offered for thumbnails; that the
        // thumbnail cache is missing is said.
        let keys = |i: &Insights| {
            i.now
                .iter()
                .filter_map(|n| n.action.as_ref().map(|a| a.1.clone()))
                .collect::<Vec<_>>()
        };
        assert_eq!(keys(&opens), ["set:prefetch", "set:display-cache"]);
        assert!(keys(&thumbs).is_empty());
        assert_eq!(thumbs.now.len(), 2);
        assert_eq!(
            thumbs.now[1].text,
            t!("open-report-insights-thumbnail-cache")
        );
        assert_eq!(thumbs.hardware.len(), 1);
        assert_eq!(
            thumbs.dismiss.as_ref().map(|d| d.1.as_str()),
            Some("dismiss:storage")
        );
    }

    #[test]
    fn saving_power_is_mentioned_for_decoding_only() {
        let s = Settings::default();
        let slow = slow_heic();
        let cx = Context {
            saving_power: true,
            ..Context::default()
        };
        let power = t!("open-report-insights-power");
        let i = build(Some(&software(true)), Some(&slow), &s, cx);
        // It replaces "Nothing in the settings changes this".
        assert_eq!(i.now.len(), 1);
        assert_eq!(i.now[0].text, power);
        let i = build(Some(&storage()), None, &s, cx);
        assert!(i.now.iter().all(|n| n.text != power));
    }

    #[test]
    fn thumbnails_of_small_files_show_no_throughput() {
        let text = |mb_s, bytes| {
            thumbs_pattern_text(&Pattern::SlowStorage {
                mb_s,
                bytes,
                archive: false,
                count: 5,
                of: 5,
            })
            .unwrap()
        };
        let small = text(0.02, 12_000);
        assert!(
            !small.contains("MB/s") && !small.contains("open-report"),
            "{small}"
        );
        assert!(text(40.0, 8_000_000).contains("MB/s"));
    }
}
