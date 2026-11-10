//! User commands: external programs run with the current file (DESIGN.md §11, "Integration
//! and automation"). They are a list, so they live outside the typed schema as an array of
//! tables in `settings.toml`:
//!
//! ```toml
//! [[commands]]
//! name = "Edit in GIMP"
//! program = "gimp"
//! args = ["{file}"]          # default; placeholders: {file} {files} {dir} {name} {stem} {ext} {uri} {page}
//! key = "Ctrl+Shift+G"       # optional
//! reload = true              # optional: reload the image when the program exits
//! ```
//!
//! Programs are started directly with these arguments, never through a shell.
//!
//! **Recipes** are user commands whose output comes back: a command with an `output` (in
//! `settings.toml`, or one recipe per file in the `recipes` folder of the config folder, with
//! the same keys at the top level). The viewer runs it like any command, waits for it (with a
//! time limit) and loads the output: a file to open, GeoJSON shapes, or a label image drawn as
//! outlines. `{out}` names a fresh, empty folder for the run.
//!
//! ```toml
//! name = "Segment nuclei"
//! program = "segment-nuclei"
//! args = ["{file}", "{out}/labels.png"]
//! output = "{out}/labels.png"
//! load = "labels"            # "open" (default), "overlay" (GeoJSON) or "labels"
//! timeout = 600              # seconds (default 300)
//! series = true              # optional: {files} is the whole listing
//! shell = false              # default; true runs `program` as a shell command line
//! ```

use toml_edit::{DocumentMut, InlineTable, Item, Value};

/// What a recipe does with its output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Load {
    /// Opens it, like any file.
    Open,
    /// GeoJSON shapes, drawn over the source image.
    Overlay,
    /// A label image (one integer per pixel, 0 is background), drawn as outlines over the
    /// source image.
    Labels,
}

/// The parts of a user command that make it a recipe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recipe {
    /// The output's path template (placeholders as in `args`, and `{out}`).
    pub output: String,
    pub load: Load,
    /// Seconds before the program is stopped.
    pub timeout: u32,
    /// `{files}` is every file of the listing, not just the selection.
    pub series: bool,
    /// `program` is a shell command line (placeholders quoted), run by `sh -c`.
    pub shell: bool,
}

/// The default and the longest time a recipe may run, in seconds.
pub const RECIPE_TIMEOUT: u32 = 300;
pub const RECIPE_TIMEOUT_MAX: u32 = 86_400;

/// Top-level key of the array of tables.
pub const COMMANDS_KEY: &str = "commands";

/// One `[[commands]]` entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserCommand {
    /// Shown in the palette and the "Open with…" menu.
    pub name: String,
    /// Program name (looked up on `PATH`) or path.
    pub program: String,
    /// Argument templates with placeholders (`{file}`, `{dir}`, …).
    pub args: Vec<String>,
    /// Optional shortcut (`Ctrl+Shift+G`), parsed by the input layer.
    pub key: Option<String>,
    /// Reload the current image when the program exits (for editors).
    pub reload: bool,
    /// Set for a recipe (an `output` was given).
    pub recipe: Option<Recipe>,
}

/// A `[[commands]]` entry that could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandProblem {
    /// 1-based position in the file.
    pub index: usize,
    pub reason: String,
}

impl std::fmt::Display for CommandProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[[commands]] #{}: {}", self.index, self.reason)
    }
}

fn string(v: Option<&Item>) -> Option<String> {
    v.and_then(Item::as_str)
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Reads every `[[commands]]` entry of `doc`. Bad entries are skipped and reported.
pub fn parse_commands(doc: &DocumentMut) -> (Vec<UserCommand>, Vec<CommandProblem>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let Some(item) = doc.get(COMMANDS_KEY) else {
        return (out, problems);
    };
    let tables: Vec<_> = match item {
        Item::ArrayOfTables(a) => a
            .iter()
            .map(|t| Some(t.clone().into_inline_table()))
            .collect(),
        Item::Value(Value::Array(a)) => a.iter().map(|v| v.as_inline_table().cloned()).collect(),
        _ => {
            problems.push(CommandProblem {
                index: 0,
                reason: "`commands` must be a list of [[commands]] tables".into(),
            });
            return (out, problems);
        }
    };
    for (i, t) in tables.into_iter().enumerate() {
        let index = i + 1;
        let Some(t) = t else {
            problems.push(CommandProblem {
                index,
                reason: "not a table".into(),
            });
            continue;
        };
        let (cmd, notes) = parse_table(&t);
        problems.extend(
            notes
                .into_iter()
                .map(|reason| CommandProblem { index, reason }),
        );
        out.extend(cmd);
    }
    (out, problems)
}

/// Reads one recipe file (the keys of a `[[commands]]` table at the top level). Notes are
/// problems that did not stop it.
pub fn parse_recipe(text: &str) -> Result<(UserCommand, Vec<String>), String> {
    let doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let t = doc.as_table().clone().into_inline_table();
    let (cmd, mut notes) = parse_table(&t);
    match cmd {
        Some(c) if c.recipe.is_some() => Ok((c, notes)),
        Some(_) => Err("a recipe needs `output`".into()),
        None => Err(notes.drain(..).next().unwrap_or_default()),
    }
}

/// One command table: the command (`None` when unusable) and the problems found.
fn parse_table(t: &InlineTable) -> (Option<UserCommand>, Vec<String>) {
    let mut notes = Vec::new();
    let get = |k: &str| t.get(k).cloned().map(Item::Value);
    let Some(name) = string(get("name").as_ref()) else {
        return (None, vec!["missing `name`".into()]);
    };
    let Some(program) = string(get("program").as_ref()) else {
        return (None, vec!["missing `program`".into()]);
    };
    let args = match t.get("args") {
        None => vec!["{file}".to_owned()],
        Some(Value::Array(a)) => {
            let v: Option<Vec<String>> = a.iter().map(|x| x.as_str().map(str::to_owned)).collect();
            match v {
                Some(v) => v,
                None => return (None, vec!["`args` must be a list of strings".into()]),
            }
        }
        Some(_) => return (None, vec!["`args` must be a list of strings".into()]),
    };
    let key = string(get("key").as_ref());
    let flag = |k: &str| -> Result<bool, String> {
        match t.get(k) {
            None => Ok(false),
            Some(v) => v
                .as_bool()
                .ok_or_else(|| format!("`{k}` must be true or false")),
        }
    };
    let reload = match flag("reload") {
        Ok(b) => b,
        Err(e) => return (None, vec![e]),
    };
    let recipe = match string(get("output").as_ref()) {
        None => None,
        Some(output) => {
            let load = match t.get("load").map(|v| v.as_str()) {
                None => Load::Open,
                Some(Some("open")) => Load::Open,
                Some(Some("overlay")) => Load::Overlay,
                Some(Some("labels")) => Load::Labels,
                _ => {
                    return (
                        None,
                        vec!["`load` is \"open\", \"overlay\" or \"labels\"".into()],
                    );
                }
            };
            let timeout = match t.get("timeout") {
                None => RECIPE_TIMEOUT,
                Some(v) => match v
                    .as_integer()
                    .filter(|s| (1..=i64::from(RECIPE_TIMEOUT_MAX)).contains(s))
                {
                    Some(s) => s as u32,
                    None => {
                        return (
                            None,
                            vec![format!("`timeout` is 1 to {RECIPE_TIMEOUT_MAX} seconds")],
                        );
                    }
                },
            };
            let (series, shell) = match (flag("series"), flag("shell")) {
                (Ok(a), Ok(b)) => (a, b),
                (Err(e), _) | (_, Err(e)) => return (None, vec![e]),
            };
            Some(Recipe {
                output,
                load,
                timeout,
                series,
                shell,
            })
        }
    };
    let known: &[&str] = if recipe.is_some() {
        &[
            "name", "program", "args", "key", "reload", "output", "load", "timeout", "series",
            "shell",
        ]
    } else {
        &["name", "program", "args", "key", "reload"]
    };
    for k in t.iter().map(|(k, _)| k) {
        if !known.contains(&k) {
            notes.push(format!("unknown key `{k}` (ignored)"));
        }
    }
    (
        Some(UserCommand {
            name,
            program,
            args,
            key,
            reload,
            recipe,
        }),
        notes,
    )
}

// ----- Hooks ------------------------------------------------------------------------------

/// Top-level key of the `[[hooks]]` array of tables: programs run when something happens in
/// the viewer (`docs/ipc.md` §10).
///
/// ```toml
/// [[hooks]]
/// name = "Upload picks"
/// on = ["marked"]              # event names, or "*"
/// when = { flag = "pick" }     # optional: equality filters on the event
/// program = "/home/u/bin/upload"
/// args = ["{files}"]           # default; placeholders: {file} {files} {dir} {event} {recipe} {outputs} {stars}
/// timeout = 120                # seconds (default 60)
/// enabled = true               # Settings › Integration switches it
/// ```
///
/// There is no `shell` key: a hook is always started directly. The event goes to the
/// program's standard input as one JSON line.
pub const HOOKS_KEY: &str = "hooks";

/// The events a hook may name (the IPC protocol's, `specular_client::protocol::EVENTS`).
pub const HOOK_EVENTS: &[&str] = &[
    "opened",
    "marked",
    "keywords",
    "exported",
    "imported",
    "captured",
    "develop-saved",
];

/// The default and the longest time a hook may run, in seconds.
pub const HOOK_TIMEOUT: u32 = 60;
pub const HOOK_TIMEOUT_MAX: u32 = 86_400;

/// A value a hook's `when` compares with.
#[derive(Clone, Debug, PartialEq)]
pub enum WhenValue {
    Text(String),
    Number(f64),
    Bool(bool),
}

/// One `[[hooks]]` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct Hook {
    pub name: String,
    /// Event names; `*` is every event.
    pub on: Vec<String>,
    /// Members of the event that must equal these (`flag`, `mark.stars`, `recipe`…).
    pub when: Vec<(String, WhenValue)>,
    pub program: String,
    pub args: Vec<String>,
    /// Seconds before the program is stopped.
    pub timeout: u32,
    pub enabled: bool,
    /// 1-based position among the `[[hooks]]` entries (to switch or remove it).
    pub index: usize,
}

impl Hook {
    /// Whether the hook runs on event `name`.
    pub fn listens(&self, name: &str) -> bool {
        self.on.iter().any(|e| e == "*" || e == name)
    }
}

/// A `[[hooks]]` entry that could not be used, or a note about one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HookProblem {
    /// 1-based position in the file.
    pub index: usize,
    pub reason: String,
}

impl std::fmt::Display for HookProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[[hooks]] #{}: {}", self.index, self.reason)
    }
}

/// Reads every `[[hooks]]` entry of `doc`. Bad entries are skipped and reported.
pub fn parse_hooks(doc: &DocumentMut) -> (Vec<Hook>, Vec<HookProblem>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let tables: Vec<Option<InlineTable>> = match doc.get(HOOKS_KEY) {
        None => return (out, problems),
        Some(Item::ArrayOfTables(a)) => a
            .iter()
            .map(|t| Some(t.clone().into_inline_table()))
            .collect(),
        Some(Item::Value(Value::Array(a))) => {
            a.iter().map(|v| v.as_inline_table().cloned()).collect()
        }
        Some(_) => {
            problems.push(HookProblem {
                index: 0,
                reason: "`hooks` must be a list of [[hooks]] tables".into(),
            });
            return (out, problems);
        }
    };
    for (i, t) in tables.into_iter().enumerate() {
        let index = i + 1;
        let Some(t) = t else {
            problems.push(HookProblem {
                index,
                reason: "not a table".into(),
            });
            continue;
        };
        match parse_hook(&t, index) {
            Ok((h, notes)) => {
                problems.extend(
                    notes
                        .into_iter()
                        .map(|reason| HookProblem { index, reason }),
                );
                out.push(h);
            }
            Err(reason) => problems.push(HookProblem { index, reason }),
        }
    }
    (out, problems)
}

fn when_value(v: &Value) -> Option<WhenValue> {
    match v {
        Value::String(s) => Some(WhenValue::Text(s.value().clone())),
        Value::Integer(i) => Some(WhenValue::Number(*i.value() as f64)),
        Value::Float(f) => Some(WhenValue::Number(*f.value())),
        Value::Boolean(b) => Some(WhenValue::Bool(*b.value())),
        _ => None,
    }
}

/// One hook table: the hook and notes, or why it cannot be used.
fn parse_hook(t: &InlineTable, index: usize) -> Result<(Hook, Vec<String>), String> {
    let mut notes = Vec::new();
    let get = |k: &str| t.get(k).cloned().map(Item::Value);
    let name = string(get("name").as_ref()).ok_or("missing `name`")?;
    let program = string(get("program").as_ref()).ok_or("missing `program`")?;
    if t.contains_key("shell") {
        return Err(
            "hooks never run through a shell: remove `shell` (to use one, name it as `program`)"
                .into(),
        );
    }
    let on: Vec<String> = match t.get("on") {
        Some(Value::String(s)) => vec![s.value().trim().to_owned()],
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| x.as_str().map(|s| s.trim().to_owned()))
            .collect::<Option<_>>()
            .ok_or("`on` must be a list of event names")?,
        Some(_) => return Err("`on` must be a list of event names".into()),
        None => return Err("missing `on` (the events it runs on)".into()),
    };
    if on.is_empty() {
        return Err("`on` names no event".into());
    }
    if let Some(bad) = on
        .iter()
        .find(|e| *e != "*" && !HOOK_EVENTS.contains(&e.as_str()))
    {
        return Err(format!(
            "unknown event `{bad}` (events: {})",
            HOOK_EVENTS.join(", ")
        ));
    }
    let when = match t.get("when") {
        None => Vec::new(),
        Some(Value::InlineTable(w)) => {
            let mut v = Vec::new();
            for (k, x) in w.iter() {
                let x = when_value(x)
                    .ok_or_else(|| format!("`when.{k}` must be text, a number or true/false"))?;
                v.push((k.to_owned(), x));
            }
            v
        }
        Some(_) => return Err("`when` must be a table, like { flag = \"pick\" }".into()),
    };
    let args = match t.get("args") {
        None => vec!["{files}".to_owned()],
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| x.as_str().map(str::to_owned))
            .collect::<Option<_>>()
            .ok_or("`args` must be a list of strings")?,
        Some(_) => return Err("`args` must be a list of strings".into()),
    };
    specular_platform::launch::validate_hook_args(&args).map_err(|e| e.to_string())?;
    let timeout = match t.get("timeout") {
        None => HOOK_TIMEOUT,
        Some(v) => v
            .as_integer()
            .filter(|s| (1..=i64::from(HOOK_TIMEOUT_MAX)).contains(s))
            .ok_or_else(|| format!("`timeout` is 1 to {HOOK_TIMEOUT_MAX} seconds"))?
            as u32,
    };
    let enabled = match t.get("enabled") {
        None => true,
        Some(v) => v.as_bool().ok_or("`enabled` must be true or false")?,
    };
    for k in t.iter().map(|(k, _)| k) {
        if ![
            "name", "on", "when", "program", "args", "timeout", "enabled",
        ]
        .contains(&k)
        {
            notes.push(format!("unknown key `{k}` (ignored)"));
        }
    }
    Ok((
        Hook {
            name,
            on,
            when,
            program,
            args,
            timeout,
            enabled,
            index,
        },
        notes,
    ))
}

/// The settings file `text` with hook `index` (1-based) switched on or off; formatting and
/// comments elsewhere are kept.
pub fn set_hook_enabled(text: &str, index: usize, on: bool) -> Result<String, String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let i = index.checked_sub(1).ok_or("no hook #0")?;
    let done = match doc.get_mut(HOOKS_KEY) {
        Some(Item::ArrayOfTables(a)) => a.get_mut(i).map(|t| {
            if on {
                t.remove("enabled");
            } else {
                t.insert("enabled", toml_edit::value(false));
            }
        }),
        Some(Item::Value(Value::Array(a))) => {
            a.get_mut(i).and_then(Value::as_inline_table_mut).map(|t| {
                if on {
                    t.remove("enabled");
                } else {
                    t.insert("enabled", Value::from(false));
                }
            })
        }
        _ => None,
    };
    done.ok_or_else(|| format!("no hook #{index}"))?;
    Ok(doc.to_string())
}

/// The settings file `text` without hook `index` (1-based).
pub fn remove_hook(text: &str, index: usize) -> Result<String, String> {
    let mut doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| e.to_string())?;
    let i = index.checked_sub(1).ok_or("no hook #0")?;
    let left = match doc.get_mut(HOOKS_KEY) {
        Some(Item::ArrayOfTables(a)) if i < a.len() => {
            a.remove(i);
            a.len()
        }
        Some(Item::Value(Value::Array(a))) if i < a.len() => {
            a.remove(i);
            a.len()
        }
        _ => return Err(format!("no hook #{index}")),
    };
    if left == 0 {
        doc.remove(HOOKS_KEY);
    }
    Ok(doc.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_parse() {
        let text = r#"
version = 1

[[hooks]]
name = "Upload picks"
on = ["marked"]
when = { flag = "pick", stars = 4 }
program = "/home/u/bin/upload"
timeout = 120

[[hooks]]
name = "All"
on = "*"
program = "log-it"
args = ["{event}", "{files}"]
enabled = false
colour = "red"

[[hooks]]
name = "Shell"
on = ["marked"]
program = "sh"
shell = true

[[hooks]]
name = "Bad event"
on = ["clicked"]
program = "x"

[[hooks]]
name = "Bad args"
on = ["exported"]
program = "x"
args = ["a{outputs}"]

[[hooks]]
on = ["opened"]
program = "nameless"
"#;
        let doc: DocumentMut = text.parse().unwrap();
        let (hooks, problems) = parse_hooks(&doc);
        assert_eq!(hooks.len(), 2);
        let h = &hooks[0];
        assert_eq!(
            (h.name.as_str(), h.timeout, h.enabled, h.index),
            ("Upload picks", 120, true, 1)
        );
        assert_eq!(h.args, ["{files}"]);
        assert_eq!(
            h.when,
            [
                ("flag".to_owned(), WhenValue::Text("pick".into())),
                ("stars".to_owned(), WhenValue::Number(4.0))
            ]
        );
        assert!(h.listens("marked") && !h.listens("opened"));
        assert!(hooks[1].listens("captured") && !hooks[1].enabled);
        assert_eq!(hooks[1].timeout, HOOK_TIMEOUT);
        let idx: Vec<usize> = problems.iter().map(|p| p.index).collect();
        assert_eq!(idx, [2, 3, 4, 5, 6], "{problems:?}");
        assert!(problems[0].reason.contains("colour"));
        assert!(problems[1].reason.contains("shell"));
        assert!(problems[2].reason.contains("clicked"));
        assert!(problems[3].reason.contains("outputs"));

        // Switched off and on, then removed; the rest of the file stays.
        let off = set_hook_enabled(text, 1, false).unwrap();
        let (h, _) = parse_hooks(&off.parse().unwrap());
        assert!(!h[0].enabled);
        assert!(off.contains("version = 1"));
        let on = set_hook_enabled(&off, 1, true).unwrap();
        assert!(parse_hooks(&on.parse().unwrap()).0[0].enabled);
        let gone = remove_hook(text, 1).unwrap();
        let (h, _) = parse_hooks(&gone.parse().unwrap());
        assert_eq!(h[0].name, "All");
        assert_eq!(h[0].index, 1);
        assert!(set_hook_enabled(text, 9, false).is_err());
        assert!(remove_hook(text, 0).is_err());
        // The last one gone: no `hooks` left at all.
        let one = "version = 1\nhooks = [{ name = \"A\", on = [\"opened\"], program = \"a\" }]\n";
        assert_eq!(parse_hooks(&one.parse().unwrap()).0.len(), 1);
        let none = remove_hook(one, 1).unwrap();
        assert!(!none.contains("hooks"), "{none}");
        let off = set_hook_enabled(one, 1, false).unwrap();
        assert!(!parse_hooks(&off.parse().unwrap()).0[0].enabled);
    }

    #[test]
    fn commands_parse() {
        let doc: DocumentMut = r#"
version = 1

[viewing]
zoom = "fit"

[[commands]]
name = "Edit in GIMP"
program = "gimp"
key = "Ctrl+Shift+G"
reload = true

[[commands]]
name = "Convert"
program = "/usr/bin/magick"
args = ["{file}", "-resize", "50%", "{dir}/{stem}-half.png"]
colour = "blue"

[[commands]]
program = "nameless"

[[commands]]
name = "Bad args"
program = "x"
args = "{file}"
"#
        .parse()
        .unwrap();
        let (cmds, problems) = parse_commands(&doc);
        assert_eq!(
            cmds,
            [
                UserCommand {
                    name: "Edit in GIMP".into(),
                    program: "gimp".into(),
                    args: vec!["{file}".into()],
                    key: Some("Ctrl+Shift+G".into()),
                    reload: true,
                    recipe: None,
                },
                UserCommand {
                    name: "Convert".into(),
                    program: "/usr/bin/magick".into(),
                    args: ["{file}", "-resize", "50%", "{dir}/{stem}-half.png"]
                        .map(String::from)
                        .to_vec(),
                    key: None,
                    reload: false,
                    recipe: None,
                },
            ]
        );
        let idx: Vec<usize> = problems.iter().map(|p| p.index).collect();
        assert_eq!(idx, [2, 3, 4]);
        assert!(problems[0].reason.contains("colour"));

        // Inline form, and none at all.
        let doc: DocumentMut = r#"commands = [{ name = "A", program = "a" }]"#.parse().unwrap();
        assert_eq!(parse_commands(&doc).0.len(), 1);
        let doc: DocumentMut = "version = 1".parse().unwrap();
        assert_eq!(parse_commands(&doc), (vec![], vec![]));
    }

    #[test]
    fn recipes_parse() {
        let (c, notes) = parse_recipe(
            r#"
name = "Segment"
program = "seg"
args = ["{file}", "{out}"]
output = "{out}/labels.png"
load = "labels"
timeout = 60
series = true
"#,
        )
        .unwrap();
        assert!(notes.is_empty());
        assert_eq!(
            c.recipe,
            Some(Recipe {
                output: "{out}/labels.png".into(),
                load: Load::Labels,
                timeout: 60,
                series: true,
                shell: false,
            })
        );
        let r = parse_recipe("name = \"A\"\nprogram = \"a\"\noutput = \"x\"\n").unwrap();
        assert_eq!(r.0.recipe.unwrap().load, Load::Open);
        for bad in [
            "name = \"A\"\nprogram = \"a\"\n",
            "program = \"a\"\noutput = \"x\"\n",
            "name = \"A\"\nprogram = \"a\"\noutput = \"x\"\nload = \"mesh\"\n",
            "name = \"A\"\nprogram = \"a\"\noutput = \"x\"\ntimeout = 0\n",
            "name = \"A\"\nprogram = \"a\"\noutput = \"x\"\nshell = \"yes\"\n",
            "not toml [",
        ] {
            assert!(parse_recipe(bad).is_err(), "{bad}");
        }
        // In settings.toml, a command with `output` is a recipe too.
        let doc: DocumentMut = r#"commands = [{ name = "A", program = "a", output = "{out}/a.geojson", load = "overlay" }]"#
            .parse()
            .unwrap();
        let (cmds, problems) = parse_commands(&doc);
        assert!(problems.is_empty());
        assert_eq!(cmds[0].recipe.as_ref().unwrap().load, Load::Overlay);
    }
}
