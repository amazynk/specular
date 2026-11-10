//! The `SPECULAR_LOG` / `--log` grammar.

use crate::COMPONENTS;
use tracing_core::LevelFilter;

/// Level of every component when nothing asks for more.
pub(crate) const DEFAULT_LEVEL: LevelFilter = LevelFilter::WARN;

/// A problem in a log spec. The rest of the spec still applies.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpecWarning {
    UnknownComponent(String),
    UnknownLevel { component: String, level: String },
}

impl std::fmt::Display for SpecWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecWarning::UnknownComponent(c) => {
                let names: Vec<&str> = COMPONENTS.iter().map(|(n, _)| *n).collect();
                write!(
                    f,
                    "SPECULAR_LOG: unknown component {c:?} (ignored). Components: all, {} (specular --log-components describes them)",
                    names.join(", ")
                )
            }
            SpecWarning::UnknownLevel { component, level } => write!(
                f,
                "SPECULAR_LOG: unknown level {level:?} for {component} (ignored). Levels: off, error, warn, info, debug, trace"
            ),
        }
    }
}

/// Per-component levels.
#[derive(Clone, Debug, PartialEq)]
pub struct Spec {
    /// One level per entry of [`COMPONENTS`], same order.
    levels: Vec<LevelFilter>,
    pub warnings: Vec<SpecWarning>,
}

impl Default for Spec {
    fn default() -> Spec {
        Spec {
            levels: vec![DEFAULT_LEVEL; COMPONENTS.len()],
            warnings: Vec::new(),
        }
    }
}

fn parse_level(s: &str) -> Option<LevelFilter> {
    Some(match s.trim().to_ascii_lowercase().as_str() {
        "off" | "none" => LevelFilter::OFF,
        "error" => LevelFilter::ERROR,
        "warn" | "warning" => LevelFilter::WARN,
        "info" => LevelFilter::INFO,
        "debug" | "" => LevelFilter::DEBUG,
        "trace" => LevelFilter::TRACE,
        _ => return None,
    })
}

impl Spec {
    /// Parses `all`, `decode,tiles`, `all:info,decode:trace,gpu` (also `name=level`).
    /// A component without a level means `debug`; items apply left to right, so the last
    /// one naming a component wins. Unknown names and levels are reported in
    /// [`Spec::warnings`] and skipped.
    pub fn parse(s: &str) -> Spec {
        let mut spec = Spec::default();
        spec.apply(s);
        spec
    }

    /// `SPECULAR_LOG`, then the `--log` flag.
    pub fn from_sources(env: Option<&str>, cli: Option<&str>) -> Spec {
        let mut spec = Spec::default();
        for s in [env, cli].into_iter().flatten() {
            spec.apply(s);
        }
        spec
    }

    fn apply(&mut self, s: &str) {
        for item in s.split(',').map(str::trim).filter(|i| !i.is_empty()) {
            let (name, level) = match item.split_once([':', '=']) {
                Some((n, l)) => (n.trim(), l),
                None => (item, ""),
            };
            let name = name.to_ascii_lowercase();
            if matches!(name.as_str(), "help" | "?" | "list") {
                continue;
            }
            let Some(level) = parse_level(level) else {
                self.warnings.push(SpecWarning::UnknownLevel {
                    component: name,
                    level: level.trim().to_owned(),
                });
                continue;
            };
            if name == "all" || name == "*" {
                self.levels.iter_mut().for_each(|l| *l = level);
            } else if let Some(i) = COMPONENTS
                .iter()
                .position(|(c, _)| *c == name || (name == "color" && *c == "colour"))
            {
                self.levels[i] = level;
            } else {
                self.warnings.push(SpecWarning::UnknownComponent(name));
            }
        }
    }

    /// The level of `component` (one of [`COMPONENTS`]); [`LevelFilter::OFF`] for others.
    pub fn level(&self, component: &str) -> LevelFilter {
        COMPONENTS
            .iter()
            .position(|(c, _)| *c == component)
            .map_or(LevelFilter::OFF, |i| self.levels[i])
    }

    /// The most verbose component level.
    pub fn max_level(&self) -> LevelFilter {
        self.levels.iter().copied().max().unwrap_or(DEFAULT_LEVEL)
    }

    /// Nothing was asked for: every component at the default level.
    pub fn is_default(&self) -> bool {
        self.levels.iter().all(|l| *l == DEFAULT_LEVEL)
    }
}

impl std::fmt::Display for Spec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let parts: Vec<String> = COMPONENTS
            .iter()
            .zip(&self.levels)
            .filter(|(_, l)| **l != DEFAULT_LEVEL)
            .map(|((c, _), l)| format!("{c}:{}", l.to_string().to_ascii_lowercase()))
            .collect();
        f.write_str(&parts.join(","))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    #[test]
    fn all_turns_every_component_to_debug() {
        let s = Spec::parse("all");
        assert!(s.warnings.is_empty());
        for (c, _) in COMPONENTS {
            assert_eq!(s.level(c), LevelFilter::DEBUG, "{c}");
        }
    }

    #[test]
    fn a_list_turns_on_just_those() {
        let s = Spec::parse("decode, tiles");
        assert_eq!(s.level(DECODE), LevelFilter::DEBUG);
        assert_eq!(s.level(TILES), LevelFilter::DEBUG);
        assert_eq!(s.level(GPU), LevelFilter::WARN);
        assert_eq!(s.level(STARTUP), LevelFilter::WARN);
    }

    #[test]
    fn levels_and_last_match_wins() {
        let s = Spec::parse("all:info,decode:trace,gpu");
        assert_eq!(s.level(DECODE), LevelFilter::TRACE);
        assert_eq!(s.level(GPU), LevelFilter::DEBUG);
        assert_eq!(s.level(TILES), LevelFilter::INFO);
        // A later `all` overrides what came before.
        let s = Spec::parse("decode:trace,all:error");
        assert_eq!(s.level(DECODE), LevelFilter::ERROR);
        let s = Spec::parse("jobs=off,JOBS:Trace");
        assert_eq!(s.level(JOBS), LevelFilter::TRACE);
        assert_eq!(Spec::parse("colour:off").level(COLOUR), LevelFilter::OFF);
        assert_eq!(Spec::parse("all:warning").max_level(), LevelFilter::WARN);
    }

    #[test]
    fn the_flag_applies_after_the_environment() {
        let s = Spec::from_sources(Some("all"), Some("decode:trace,gpu:off"));
        assert_eq!(s.level(DECODE), LevelFilter::TRACE);
        assert_eq!(s.level(GPU), LevelFilter::OFF);
        assert_eq!(s.level(UI), LevelFilter::DEBUG);
    }

    #[test]
    fn unknown_names_and_levels_warn_and_list_the_valid_ones() {
        let s = Spec::parse("decode,decoder,tiles:loud,,");
        assert_eq!(s.level(DECODE), LevelFilter::DEBUG);
        assert_eq!(s.level(TILES), LevelFilter::WARN);
        assert_eq!(
            s.warnings,
            vec![
                SpecWarning::UnknownComponent("decoder".into()),
                SpecWarning::UnknownLevel {
                    component: "tiles".into(),
                    level: "loud".into()
                }
            ]
        );
        let text = s.warnings[0].to_string();
        for (c, _) in COMPONENTS {
            assert!(text.contains(c), "{text}");
        }
        assert!(text.contains("--log-components"));
    }

    #[test]
    fn empty_and_help_change_nothing() {
        assert!(Spec::parse("").is_default());
        assert!(Spec::parse("help").is_default());
        assert!(Spec::parse("help").warnings.is_empty());
        assert_eq!(
            Spec::parse("decode,gpu:trace").to_string(),
            "decode:debug,gpu:trace"
        );
    }
}
