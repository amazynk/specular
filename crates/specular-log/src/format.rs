//! The subscriber: a fixed per-component filter, the line format and the stderr/file writer.

use std::fmt;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use tracing::field::{Field, Visit};
use tracing::{Event, Level, Metadata, Subscriber};
use tracing_core::LevelFilter;
use tracing_core::subscriber::Interest;
use tracing_log::NormalizeEvent;
use tracing_subscriber::field::RecordFields;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::fmt::format::Writer;
use tracing_subscriber::fmt::{FmtContext, FormatEvent, FormatFields, MakeWriter};
use tracing_subscriber::layer::{Context, Layer, SubscriberExt};
use tracing_subscriber::registry::LookupSpan;

use crate::rotate::RotatingFile;
use crate::{COMPONENTS, Spec, component_of, ms_since_start};

/// Third-party crates that are noisy at `warn`: errors only unless `RUST_LOG` says otherwise.
const QUIET: &[&str] = &[
    "wgpu_core",
    "wgpu_hal",
    "wgpu",
    "naga",
    "sctk",
    "smithay_client_toolkit",
    "calloop",
    // SVG: one warning per unsupported element or attribute, every time a file is opened.
    "usvg",
    "resvg",
    "simplecss",
    "svgtypes",
    "fontdb",
    // PDF drawing: a warning for each path with a NaN coordinate (dropped), on every render.
    "vello_common",
    "vello_cpu",
];

/// Decides every callsite once, from its target and level: Specular components by the spec,
/// everything else by `RUST_LOG` (or the defaults).
pub(crate) struct ComponentFilter {
    levels: Vec<LevelFilter>,
    third_party: Targets,
}

impl ComponentFilter {
    pub(crate) fn new(spec: &Spec, rust_log: Option<&str>) -> ComponentFilter {
        let defaults = || {
            Targets::new()
                .with_default(LevelFilter::WARN)
                .with_targets(QUIET.iter().map(|t| (*t, LevelFilter::ERROR)))
        };
        let third_party = match rust_log.map(str::parse::<Targets>) {
            Some(Ok(t)) => t,
            Some(Err(e)) => {
                eprintln!("specular: RUST_LOG not understood ({e}); using the defaults");
                defaults()
            }
            None => defaults(),
        };
        ComponentFilter {
            levels: COMPONENTS.iter().map(|(c, _)| spec.level(c)).collect(),
            third_party,
        }
    }

    fn component_level(&self, target: &str) -> Option<LevelFilter> {
        let c = component_of(target)?;
        COMPONENTS
            .iter()
            .position(|(n, _)| *n == c)
            .map(|i| self.levels[i])
    }

    pub(crate) fn allows(&self, meta: &Metadata<'_>) -> bool {
        match self.component_level(meta.target()) {
            Some(l) => l >= *meta.level(),
            None => self.third_party.would_enable(meta.target(), meta.level()),
        }
    }

    pub(crate) fn max_level(&self) -> LevelFilter {
        let third = self
            .third_party
            .iter()
            .map(|(_, l)| l)
            .chain(self.third_party.default_level())
            .max();
        self.levels
            .iter()
            .copied()
            .chain(third)
            .max()
            .unwrap_or(LevelFilter::WARN)
    }
}

/// `log` records arrive through tracing-log's per-level callsites, whose target is "log";
/// their real target is checked per record (`enabled`).
fn is_log_callsite(meta: &Metadata<'_>) -> bool {
    meta.target() == "log" && meta.name() == "log event"
}

impl<S: Subscriber> Layer<S> for ComponentFilter {
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
        if is_log_callsite(meta) {
            Interest::sometimes()
        } else if self.allows(meta) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, meta: &Metadata<'_>, _ctx: Context<'_, S>) -> bool {
        is_log_callsite(meta) || self.allows(meta)
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(self.max_level())
    }
}

/// `   123.4 main         decode   img=7 2.jpeg header 494×360 key=value`
pub(crate) struct LineFormat;

impl<S, N> FormatEvent<S, N> for LineFormat
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        ctx: &FmtContext<'_, S, N>,
        mut w: Writer<'_>,
        event: &Event<'_>,
    ) -> fmt::Result {
        let normalized = event.normalized_metadata();
        let meta = normalized.as_ref().unwrap_or_else(|| event.metadata());
        // The text first, so identical lines can be held back ([`crate::repeat`]); writing
        // nothing leaves the line out.
        let mut text = String::new();
        ctx.format_fields(Writer::new(&mut text), event)?;
        let site = match (meta.file(), meta.line()) {
            (Some(f), Some(l)) => format!("{f}:{l}"),
            (Some(f), None) => f.to_owned(),
            _ => String::new(),
        };
        let verdict = crate::repeat::check(crate::repeat::Line {
            site: &site,
            target: meta.target(),
            level: *meta.level(),
            text: &text,
        });
        // Counts of lines left out, each on a line of its own, before this one.
        for s in &verdict.summaries {
            write_prefix(&mut w, &s.target, s.level)?;
            w.write_str(&s.text)?;
            w.write_str(&s.suffix())?;
            writeln!(w)?;
        }
        if !verdict.write {
            return Ok(());
        }
        write_prefix(&mut w, meta.target(), *meta.level())?;
        w.write_str(&text)?;
        writeln!(w)
    }
}

fn write_prefix(w: &mut impl fmt::Write, target: &str, level: Level) -> fmt::Result {
    let comp = component_of(target).unwrap_or_else(|| target.split("::").next().unwrap_or(target));
    let thread = std::thread::current();
    let name = thread.name().unwrap_or("?");
    write!(w, "{:>9.1} {name:<13} {comp:<8} ", ms_since_start())?;
    match level {
        Level::ERROR => w.write_str("ERROR "),
        Level::WARN => w.write_str("WARN "),
        _ => Ok(()),
    }
}

/// The message, then `name=value` for each other field (tracing-log's `log.*` left out).
pub(crate) struct Fields;

impl<'w> FormatFields<'w> for Fields {
    fn format_fields<R: RecordFields>(&self, mut w: Writer<'w>, fields: R) -> fmt::Result {
        let mut v = FieldVisitor {
            w: &mut w,
            first: true,
            result: Ok(()),
        };
        fields.record(&mut v);
        v.result
    }
}

struct FieldVisitor<'a, W: fmt::Write> {
    w: &'a mut W,
    first: bool,
    result: fmt::Result,
}

impl<W: fmt::Write> FieldVisitor<'_, W> {
    fn field(&mut self, field: &Field, value: fmt::Arguments<'_>) {
        if self.result.is_err() || field.name().starts_with("log.") {
            return;
        }
        let sep = if std::mem::take(&mut self.first) {
            ""
        } else {
            " "
        };
        self.result = if field.name() == "message" {
            write!(self.w, "{sep}{value}")
        } else {
            write!(self.w, "{sep}{}={value}", field.name())
        };
    }
}

impl<W: fmt::Write> Visit for FieldVisitor<'_, W> {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.field(field, format_args!("{value}"));
    }
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.field(field, format_args!("{value:?}"));
    }
}

/// Writes each line to stderr and to the log file (size-capped, rotated).
#[derive(Clone)]
pub(crate) struct Tee {
    file: Option<Arc<Mutex<RotatingFile>>>,
    /// Tests: lines go to this buffer instead of stderr.
    capture: Option<Arc<Mutex<Vec<u8>>>>,
}

pub(crate) struct TeeWriter<'a>(&'a Tee);

impl std::io::Write for TeeWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        crate::recent::push(buf);
        match &self.0.capture {
            Some(c) => c.lock().extend_from_slice(buf),
            None => {
                let _ = std::io::stderr().lock().write_all(buf);
            }
        }
        if let Some(f) = &self.0.file {
            let _ = f.lock().write_all(buf);
        }
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if let Some(f) = &self.0.file {
            f.lock().flush()?;
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for Tee {
    type Writer = TeeWriter<'a>;
    fn make_writer(&'a self) -> TeeWriter<'a> {
        TeeWriter(self)
    }
}

pub(crate) fn install(
    spec: &Spec,
    file: Option<&Path>,
    rust_log: Option<&str>,
    capture: Option<Arc<Mutex<Vec<u8>>>>,
) -> bool {
    let file = file.and_then(
        |p| match RotatingFile::open(p, crate::rotate::DEFAULT_CAP) {
            Ok(f) => Some(Arc::new(Mutex::new(f))),
            Err(e) => {
                eprintln!("specular: log file {}: {e}", p.display());
                None
            }
        },
    );
    let filter = ComponentFilter::new(spec, rust_log);
    let max = filter.max_level();
    let fmt = tracing_subscriber::fmt::layer()
        .event_format(LineFormat)
        .fmt_fields(Fields)
        .with_writer(Tee { file, capture });
    let subscriber = tracing_subscriber::registry().with(filter).with(fmt);
    if tracing::subscriber::set_global_default(subscriber).is_ok() {
        let log_max = match max {
            LevelFilter::OFF => log::LevelFilter::Off,
            LevelFilter::ERROR => log::LevelFilter::Error,
            LevelFilter::WARN => log::LevelFilter::Warn,
            LevelFilter::INFO => log::LevelFilter::Info,
            LevelFilter::DEBUG => log::LevelFilter::Debug,
            _ => log::LevelFilter::Trace,
        };
        let _ = tracing_log::LogTracer::builder()
            .with_max_level(log_max)
            .init();
        true
    } else {
        false
    }
}

/// Formats one line the way the subscriber does (tests, snapshots).
#[cfg(test)]
pub(crate) fn line(target: &str, level: Level, message: &str) -> String {
    let mut s = String::new();
    write_prefix(&mut s, target, level).unwrap();
    s.push_str(message);
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta_allows(f: &ComponentFilter, target: &'static str, level: Level) -> bool {
        match f.component_level(target) {
            Some(l) => l >= level,
            None => f.third_party.would_enable(target, &level),
        }
    }

    #[test]
    fn filter_uses_the_spec_for_components_and_rust_log_for_the_rest() {
        let f = ComponentFilter::new(&Spec::parse("decode:trace,gpu"), None);
        assert!(meta_allows(&f, "decode", Level::TRACE));
        assert!(meta_allows(&f, "gpu", Level::DEBUG));
        assert!(!meta_allows(&f, "gpu", Level::TRACE));
        assert!(!meta_allows(&f, "tiles", Level::DEBUG));
        assert!(meta_allows(&f, "tiles", Level::WARN));
        assert!(meta_allows(&f, "specular_settings::store", Level::WARN));
        assert!(!meta_allows(&f, "wgpu_core::device", Level::WARN));
        assert!(meta_allows(&f, "wgpu_core::device", Level::ERROR));
        assert!(!meta_allows(&f, "vello_common::flatten", Level::WARN));
        assert!(meta_allows(&f, "winit::platform", Level::WARN));
        assert!(!meta_allows(&f, "winit::platform", Level::INFO));
        assert_eq!(f.max_level(), LevelFilter::TRACE);

        let f = ComponentFilter::new(&Spec::default(), Some("wgpu_core=info"));
        assert!(meta_allows(&f, "wgpu_core::device", Level::INFO));
        assert!(!meta_allows(&f, "decode", Level::DEBUG));
        assert_eq!(f.max_level(), LevelFilter::INFO);
        // With nothing asked for, `debug!` is filtered by the static max level.
        assert_eq!(
            ComponentFilter::new(&Spec::default(), None).max_level(),
            LevelFilter::WARN
        );
    }

    #[test]
    fn line_prefix() {
        let l = specular_thread::builder("dec-3")
            .spawn(|| line("decode", Level::DEBUG, "img=7 2.jpeg header"))
            .unwrap()
            .join()
            .unwrap();
        let cols: Vec<&str> = l.split_whitespace().collect();
        assert!(cols[0].parse::<f64>().is_ok(), "{l}");
        assert_eq!(&cols[1..], ["dec-3", "decode", "img=7", "2.jpeg", "header"]);
        let w = line("specular_settings::store", Level::WARN, "x");
        assert!(w.contains(" settings ") && w.contains("WARN x"), "{w}");
    }
}
