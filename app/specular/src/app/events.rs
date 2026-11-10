//! What happens in the viewer, told to whoever listens (`docs/ipc.md` §9): clients that
//! subscribed over the socket ([`crate::remote::events`]) and the hooks of settings.toml
//! ([`super::hooks`]).
//!
//! Each place that does something calls [`App::emit`] once per act, with every file the act
//! touched (marking 500 photos is one event). The payloads are built by the functions here,
//! which the tests check one by one. An event says where it came from: the user, a client
//! over the socket, or a hook's program (which names the hook, so hooks never set each other
//! off).
//!
//! `opened` waits until the image has rested on screen for [`OPENED_REST`]: stepping through
//! a folder tells only where the user stopped.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};

use super::App;
use crate::remote::events::{Bus, Event};
use crate::remote::protocol::{label_name, path_value};
use crate::remote::server::Origin;

/// How long an image stays on screen before `opened` is told.
pub(super) const OPENED_REST: Duration = Duration::from_millis(300);

/// Who did what an event reports.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) enum Source {
    #[default]
    User,
    Ipc,
    /// A hook's program, through the socket.
    Hook(String),
}

impl Source {
    pub(crate) fn of(o: &Origin) -> Source {
        match o {
            Origin::Ipc => Source::Ipc,
            Origin::Hook(h) => Source::Hook(h.clone()),
        }
    }

    /// The members every line of the event carries: `source`, and `hook` for a hook's.
    pub(crate) fn members(&self) -> Map<String, Value> {
        let mut m = Map::new();
        let s = match self {
            Source::User => "user",
            Source::Ipc => "ipc",
            Source::Hook(_) => "hook",
        };
        m.insert("source".into(), json!(s));
        if let Some(h) = self.hook() {
            m.insert("hook".into(), json!(h));
        }
        m
    }

    pub(crate) fn hook(&self) -> Option<&str> {
        match self {
            Source::Hook(h) => Some(h),
            _ => None,
        }
    }
}

/// The event state of a window.
pub(super) struct Events {
    pub bus: Arc<Bus>,
    /// Who is acting now: a request from the socket sets it while it is answered.
    pub source: Source,
    /// The image on screen and since when (`opened` once it rests).
    opened: Option<(PathBuf, Instant)>,
    /// The image `opened` was last told for (not told again while it stays).
    told: Option<PathBuf>,
    /// Exports started over the socket: their job and who asked, by the run they belong to.
    pub exports: Option<(u64, Source)>,
    /// Styles applied over the socket: the Develop batch's token, its job and who asked.
    pub styles: HashMap<u64, (u64, Source)>,
    next_job: u64,
}

impl Default for Events {
    fn default() -> Events {
        Events {
            bus: Bus::new(),
            source: Source::User,
            opened: None,
            told: None,
            exports: None,
            styles: HashMap::new(),
            next_job: 0,
        }
    }
}

impl Events {
    /// A fresh job number (answers to `export` and `apply-style`).
    pub fn job(&mut self) -> u64 {
        self.next_job += 1;
        self.next_job
    }
}

// ----- Payloads ---------------------------------------------------------------------------

/// `opened`: the image on screen, and how it is shown (`viewer`, `reading`, `slideshow`,
/// `develop`).
pub(super) fn opened(path: &Path, view: &str) -> Event {
    Event::new("opened", vec![path.to_path_buf()]).with("view", json!(view))
}

/// The `mark` member: flag, stars and label as the protocol names them.
pub(super) fn mark_value(rating: u8, label: u8, flag: i8) -> Value {
    json!({
        "flag": match flag {
            1 => Value::from("pick"),
            -1 => Value::from("reject"),
            _ => Value::Null,
        },
        "stars": rating,
        "label": label_name(label),
    })
}

/// `marked`: one event per mark set (an act that gives every photo the same mark is one
/// event), in the order the photos came.
pub(super) fn marked<'a>(changed: impl Iterator<Item = (&'a PathBuf, u8, u8, i8)>) -> Vec<Event> {
    let mut groups: Vec<((u8, u8, i8), Vec<PathBuf>)> = Vec::new();
    for (p, r, l, f) in changed {
        match groups.iter_mut().find(|g| g.0 == (r, l, f)) {
            Some(g) => g.1.push(p.clone()),
            None => groups.push(((r, l, f), vec![p.clone()])),
        }
    }
    groups
        .into_iter()
        .map(|((r, l, f), files)| Event::new("marked", files).with("mark", mark_value(r, l, f)))
        .collect()
}

/// `keywords`: the photos whose keywords changed, with the keywords that came and went on
/// any of them. `None` when nothing changed.
pub(super) fn keywords(changes: &[(PathBuf, Vec<String>, Vec<String>)]) -> Option<Event> {
    let mut files = Vec::new();
    let mut added = BTreeSet::new();
    let mut removed = BTreeSet::new();
    for (p, was, now) in changes {
        if was == now {
            continue;
        }
        files.push(p.clone());
        added.extend(now.iter().filter(|k| !was.contains(k)).cloned());
        removed.extend(was.iter().filter(|k| !now.contains(k)).cloned());
    }
    if files.is_empty() {
        return None;
    }
    Some(
        Event::new("keywords", files)
            .with("added", json!(added))
            .with("removed", json!(removed)),
    )
}

/// One result of an export run: the source, the recipe's name, and the file written or why
/// not.
pub(super) type Exported = (PathBuf, String, Result<PathBuf, String>);

/// `exported`: the sources, the recipes' names (`recipe` is the first, `recipes` all), the
/// pairs of source and output, and what failed with why. `job` when a client started it.
pub(super) fn exported(results: &[Exported], job: Option<u64>) -> Event {
    let mut files: Vec<PathBuf> = Vec::new();
    let mut recipes: Vec<&str> = Vec::new();
    let mut outputs = Vec::new();
    let mut failed = Vec::new();
    for (src, recipe, r) in results {
        if !files.contains(src) {
            files.push(src.clone());
        }
        if !recipes.contains(&recipe.as_str()) {
            recipes.push(recipe);
        }
        match r {
            Ok(out) => outputs.push(json!([path_value(src), path_value(out)])),
            Err(why) => failed.push(json!([path_value(src), why])),
        }
    }
    let mut e = Event::new("exported", files)
        .with("recipe", json!(recipes.first()))
        .with("recipes", json!(recipes))
        .with_list("outputs", outputs)
        .with_list("failed", failed);
    if let Some(j) = job {
        e = e.with("job", json!(j));
    }
    e
}

/// `imported`: the first files at the destination, the folder they went to, and how many.
pub(super) fn imported(files: Vec<PathBuf>, destination: &Path, count: usize) -> Event {
    Event::new("imported", files)
        .with("destination", path_value(destination))
        .with("count", json!(count))
}

/// `captured`: a tethered camera sent a photo.
pub(super) fn captured(path: &Path, camera: &str) -> Event {
    Event::new("captured", vec![path.to_path_buf()])
        .with("camera", json!(camera))
        .with("path", path_value(path))
}

/// `develop-saved`: Develop edits written (each photo's sidecar, `null` where none was
/// written), of version `version`.
pub(super) fn develop_saved(saved: &[(PathBuf, Option<PathBuf>)], version: u32) -> Event {
    let files = saved.iter().map(|(p, _)| p.clone()).collect();
    let sidecars: Vec<Value> = saved
        .iter()
        .map(|(_, s)| s.as_deref().map_or(Value::Null, path_value))
        .collect();
    let mut e = Event::new("develop-saved", files).with("version", json!(version));
    if let [(_, s)] = saved {
        e = e.with("sidecar", s.as_deref().map_or(Value::Null, path_value));
    }
    e.with_list("sidecars", sidecars)
}

// ----- Telling ----------------------------------------------------------------------------

impl App {
    /// Tells `e` to the subscribers and the hooks, as done by whoever is acting now.
    pub(super) fn emit(&mut self, e: Event) {
        let source = self.remote.events.source.clone();
        self.emit_as(e, &source);
    }

    /// Tells `e` as done by `source`.
    pub(super) fn emit_as(&mut self, e: Event, source: &Source) {
        if self.headless() {
            return;
        }
        let hooks = self.hooks_listening(e.name, source);
        if !hooks && !self.remote.events.bus.wanted(e.name) {
            // Nobody listens.
            return;
        }
        let lines = self.remote.events.bus.publish(&e, &source.members());
        if hooks {
            self.hooks_fire(e.name, &lines);
        }
    }

    /// Whether anyone would hear `name` (a payload is only worth building then).
    pub(super) fn events_listening(&mut self, name: &str) -> bool {
        !self.headless()
            && (self.remote.events.bus.wanted(name) || self.hooks_listening(name, &Source::User))
    }

    /// The gallery set marks (`changed`: path, stars, label, flag).
    pub(super) fn events_marked<'a>(
        &mut self,
        changed: impl Iterator<Item = (&'a PathBuf, u8, u8, i8)>,
    ) {
        if !self.events_listening("marked") {
            return;
        }
        for e in marked(changed) {
            self.emit(e);
        }
    }

    /// An image was put on screen, or its pixels arrived: `opened` once it has rested there
    /// for [`OPENED_REST`] after the later of the two.
    pub(super) fn events_shown(&mut self) {
        let path = self.current.and_then(|i| self.entries.get(i)).cloned();
        let ev = &mut self.remote.events;
        match path {
            Some(p) if ev.told.as_ref() == Some(&p) => ev.opened = None,
            Some(p) => ev.opened = Some((p, Instant::now())),
            None => ev.opened = None,
        }
    }

    /// Once per turn: `opened` for an image that rested, and Develop's saves.
    pub(super) fn events_tick(&mut self) {
        self.events_develop_saved();
        let Some((p, at)) = self.remote.events.opened.clone() else {
            return;
        };
        if at.elapsed() < OPENED_REST {
            return;
        }
        self.remote.events.opened = None;
        let still = self.current.and_then(|i| self.entries.get(i)) == Some(&p);
        if !still || self.remote.events.told.as_ref() == Some(&p) {
            return;
        }
        self.remote.events.told = Some(p.clone());
        let view = if self.develop_active() {
            "develop"
        } else if self.slideshow.running() || self.present_show_running() {
            "slideshow"
        } else if self.in_book() {
            "reading"
        } else {
            "viewer"
        };
        if self.events_listening("opened") {
            self.emit_as(opened(&p, view), &Source::User);
        }
    }

    /// When the event loop must wake for `opened`.
    pub(super) fn events_wake(&self) -> Option<Instant> {
        self.remote
            .events
            .opened
            .as_ref()
            .map(|o| o.1 + OPENED_REST)
    }

    /// Develop's saves that were written: `develop-saved`, once per save (a batch's at once).
    fn events_develop_saved(&mut self) {
        let saved = super::develop::store::take_saved();
        if saved.is_empty() || !self.events_listening("develop-saved") {
            return;
        }
        // Grouped by batch (token) and version, in order.
        type Group = (Option<u64>, u32, Vec<(PathBuf, Option<PathBuf>)>);
        let mut groups: Vec<Group> = Vec::new();
        for s in saved {
            match groups
                .iter_mut()
                .find(|g| g.0 == s.token && g.1 == s.version && s.token.is_some())
            {
                Some(g) => g.2.push((s.path, s.sidecar)),
                None => groups.push((s.token, s.version, vec![(s.path, s.sidecar)])),
            }
        }
        for (token, version, files) in groups {
            let asked = token.and_then(|t| self.remote.events.styles.get(&t).cloned());
            let mut e = develop_saved(&files, version);
            match asked {
                Some((job, src)) => {
                    e = e.with("job", json!(job));
                    self.emit_as(e, &src);
                }
                None => self.emit_as(e, &Source::User),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    /// The line an event becomes, as the given source.
    fn line(e: &Event, s: &Source) -> Value {
        let bus = Bus::new();
        let l = bus.publish(e, &s.members());
        assert_eq!(l.len(), 1);
        l[0].value.clone()
    }

    #[test]
    fn opened_event() {
        let v = line(&opened(Path::new("/p/a.jpg"), "viewer"), &Source::User);
        assert_eq!(v["event"], "opened");
        assert_eq!(v["files"], json!(["/p/a.jpg"]));
        assert_eq!(v["view"], "viewer");
        assert_eq!(v["source"], "user");
        assert!(v.get("hook").is_none());
        assert!(v["time"].as_str().unwrap().ends_with('Z'));
    }

    #[test]
    fn marked_events_group_by_mark() {
        let files: Vec<PathBuf> = (0..500).map(|i| p(&format!("/p/{i}.cr3"))).collect();
        // One act, one mark: one event for all 500.
        let ev = marked(files.iter().map(|f| (f, 4, 0, 1)));
        assert_eq!(ev.len(), 1);
        let v = line(&ev[0], &Source::Ipc);
        assert_eq!(v["files"].as_array().unwrap().len(), 500);
        assert_eq!(
            v["mark"],
            json!({"flag": "pick", "stars": 4, "label": null})
        );
        assert_eq!(v["source"], "ipc");
        // Each photo its own mark (an undo): one event per mark.
        let two = [p("/a"), p("/b"), p("/c")];
        let ev = marked([(&two[0], 1, 2, -1), (&two[1], 0, 0, 0), (&two[2], 1, 2, -1)].into_iter());
        assert_eq!(ev.len(), 2);
        assert_eq!(ev[0].files, [p("/a"), p("/c")]);
        assert_eq!(
            ev[0].members["mark"],
            json!({"flag": "reject", "stars": 1, "label": "yellow"})
        );
        assert_eq!(
            ev[1].members["mark"],
            json!({"flag": null, "stars": 0, "label": null})
        );
    }

    #[test]
    fn a_hooks_events_name_it() {
        let ev = marked([(&p("/a"), 5, 0, 1)].into_iter());
        let v = line(&ev[0], &Source::Hook("Upload picks".into()));
        assert_eq!(
            (v["source"].clone(), v["hook"].clone()),
            (json!("hook"), json!("Upload picks"))
        );
    }

    #[test]
    fn keywords_event() {
        let ch = vec![
            (
                p("/a"),
                vec!["cat".into()],
                vec!["cat".into(), "sofa".into()],
            ),
            (p("/b"), vec!["dog".into()], vec![]),
            (p("/c"), vec!["x".into()], vec!["x".into()]),
        ];
        let e = keywords(&ch).unwrap();
        let v = line(&e, &Source::User);
        assert_eq!(v["files"], json!(["/a", "/b"]));
        assert_eq!(v["added"], json!(["sofa"]));
        assert_eq!(v["removed"], json!(["dog"]));
        assert!(keywords(&ch[2..]).is_none(), "nothing changed");
    }

    #[test]
    fn exported_event() {
        let r: Vec<Exported> = vec![
            (p("/a.cr3"), "Web".into(), Ok(p("/out/a.jpg"))),
            (p("/a.cr3"), "Print".into(), Ok(p("/out/a.tif"))),
            (p("/b.cr3"), "Web".into(), Err("disk full".into())),
        ];
        let v = line(&exported(&r, Some(7)), &Source::Ipc);
        assert_eq!(v["files"], json!(["/a.cr3", "/b.cr3"]));
        assert_eq!(v["recipe"], "Web");
        assert_eq!(v["recipes"], json!(["Web", "Print"]));
        assert_eq!(
            v["outputs"],
            json!([["/a.cr3", "/out/a.jpg"], ["/a.cr3", "/out/a.tif"]])
        );
        assert_eq!(v["failed"], json!([["/b.cr3", "disk full"]]));
        assert_eq!(v["job"], 7);
        let v = line(&exported(&[], None), &Source::User);
        assert!(v.get("job").is_none());
        assert_eq!(v["recipe"], Value::Null);
    }

    #[test]
    fn imported_event() {
        let v = line(
            &imported(vec![p("/pics/2026/a.cr3")], Path::new("/pics/2026"), 12),
            &Source::User,
        );
        assert_eq!(v["files"], json!(["/pics/2026/a.cr3"]));
        assert_eq!(
            (v["destination"].clone(), v["count"].clone()),
            (json!("/pics/2026"), json!(12))
        );
    }

    #[test]
    fn captured_event() {
        let v = line(
            &captured(Path::new("/t/IMG_1.CR3"), "Canon EOS R5"),
            &Source::User,
        );
        assert_eq!(v["files"], json!(["/t/IMG_1.CR3"]));
        assert_eq!(v["camera"], "Canon EOS R5");
        assert_eq!(v["path"], "/t/IMG_1.CR3");
    }

    #[test]
    fn develop_saved_event() {
        let one = [(p("/a.cr3"), Some(p("/a.cr3.xmp")))];
        let v = line(&develop_saved(&one, 2), &Source::User);
        assert_eq!(v["files"], json!(["/a.cr3"]));
        assert_eq!(v["version"], 2);
        assert_eq!(v["sidecar"], "/a.cr3.xmp");
        assert_eq!(v["sidecars"], json!(["/a.cr3.xmp"]));
        let many = [(p("/a"), None), (p("/b"), Some(p("/b.xmp")))];
        let v = line(&develop_saved(&many, 0), &Source::Ipc);
        assert!(v.get("sidecar").is_none());
        assert_eq!(v["sidecars"], json!([null, "/b.xmp"]));
    }

    #[test]
    fn sources() {
        assert_eq!(Source::of(&Origin::Ipc), Source::Ipc);
        assert_eq!(Source::of(&Origin::Hook("h".into())).hook(), Some("h"));
        assert_eq!(Source::User.members()["source"], "user");
    }
}
