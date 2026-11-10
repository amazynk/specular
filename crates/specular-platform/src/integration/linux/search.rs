//! Library search in the desktop's search: GNOME Shell (`org.gnome.Shell.SearchProvider2`)
//! and KRunner (`org.kde.krunner1`), both over D-Bus.
//!
//! The option writes the providers' registration files and a D-Bus service that starts
//! `specular --search-provider` on demand. That process is small (no window, no GPU): it
//! answers queries from a [`SearchSource`] and exits after a minute without queries. Picking a
//! result starts `specular -- FILE`, which goes to the running viewer when single instance is
//! on; GNOME's "more results" starts `specular --search QUERY` (the Library with the query).
//!
//! The source is the app's: the Library's catalogue, read-only, through its query language,
//! pictures only. Only paths it returned are opened, so another program on the bus cannot
//! have the provider open anything else.
//!
//! Known gap: GNOME Shell reads search providers only from the system data folders
//! (`/usr/share/gnome-shell/search-providers`), so a per-user install is not picked up by it;
//! a distribution package ships the same `.ini` there. KRunner reads the per-user folder.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use specular_log::{SETTINGS, debug, info};
use zbus::zvariant::Value;

use super::dbus::{reload_bus, service_entry};
use super::{APP_ID, MARKER, Record, install, is_ours, uninstall};
use crate::integration::{Env, Error, OptionId, Result, State};

/// Argument that starts the search provider process.
pub const SERVICE_ARG: &str = "--search-provider";
/// Argument that opens the Library searching for the next argument.
pub const SEARCH_ARG: &str = "--search";
const GNOME_BUS: &str = "app.specular.Specular.SearchProvider";
const GNOME_PATH: &str = "/app/specular/Specular/SearchProvider";
const KRUNNER_BUS: &str = "app.specular.Specular.KRunner";
const KRUNNER_PATH: &str = "/app/specular/KRunner";
/// Results per query (the desktop shows a handful).
pub const LIMIT: usize = 10;
/// Results remembered for `GetResultMetas` and opening (several queries' worth).
const REMEMBERED: usize = 200;
/// KRunner's action that opens the result's folder.
const FOLDER_ACTION: &str = "folder";

/// A desktop search front end.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrontEnd {
    GnomeShell,
    KRunner,
}

impl FrontEnd {
    pub fn name(self) -> &'static str {
        match self {
            FrontEnd::GnomeShell => "GNOME Shell",
            FrontEnd::KRunner => "KRunner",
        }
    }
}

/// The search front ends installed.
pub fn front_ends(env: &Env) -> Vec<FrontEnd> {
    let mut v = Vec::new();
    if env.find("gnome-shell").is_some() {
        v.push(FrontEnd::GnomeShell);
    }
    if env.find("krunner").is_some() || env.find("plasmashell").is_some() {
        v.push(FrontEnd::KRunner);
    }
    v
}

// ----- Registration files -----------------------------------------------------------------

fn gnome_ini(env: &Env) -> PathBuf {
    env.data_home
        .join("gnome-shell/search-providers")
        .join(format!("{APP_ID}.search-provider.ini"))
}

fn krunner_file(env: &Env) -> PathBuf {
    env.data_home
        .join("krunner/dbusplugins")
        .join(format!("{APP_ID}.desktop"))
}

fn service_file(env: &Env, bus: &str) -> PathBuf {
    env.data_home
        .join("dbus-1/services")
        .join(format!("{bus}.service"))
}

/// The list of recently viewed files earlier versions kept for this option: removed.
fn old_recent_list(env: &Env) -> PathBuf {
    env.state_home.join("specular/recent-images")
}

/// The files the option installs on this desktop (what removing it takes away).
pub fn files(env: &Env) -> Vec<PathBuf> {
    let mut v = Vec::new();
    for fe in front_ends(env) {
        match fe {
            FrontEnd::GnomeShell => v.extend([gnome_ini(env), service_file(env, GNOME_BUS)]),
            FrontEnd::KRunner => v.extend([krunner_file(env), service_file(env, KRUNNER_BUS)]),
        }
    }
    v
}

const WHY: &str = "Settings > Integration > Desktop search";

pub(super) fn apply(env: &Env) -> Result<()> {
    let key = OptionId::SearchProvider.key();
    let mut rec = Record::load(env, key).unwrap_or_default();
    for fe in front_ends(env) {
        match fe {
            FrontEnd::GnomeShell => {
                let ini = format!(
                    "# {MARKER}: installed by Specular ({WHY}); removed with it.\n\
                     [Shell Search Provider]\n\
                     DesktopId={APP_ID}.desktop\n\
                     BusName={GNOME_BUS}\n\
                     ObjectPath={GNOME_PATH}\n\
                     Version=2\n"
                );
                install(&mut rec, &gnome_ini(env), &ini, 0o644)?;
                let svc = service_entry(env, GNOME_BUS, SERVICE_ARG, WHY);
                install(&mut rec, &service_file(env, GNOME_BUS), &svc, 0o644)?;
            }
            FrontEnd::KRunner => {
                let plugin = format!(
                    "# {MARKER}: installed by Specular ({WHY}); removed with it.\n\
                     [Desktop Entry]\n\
                     Type=Service\n\
                     Name=Specular\n\
                     Comment=Pictures in the Specular Library\n\
                     Icon={APP_ID}\n\
                     X-KDE-ServiceTypes=Plasma/Runner\n\
                     X-KDE-PluginInfo-Name={APP_ID}\n\
                     X-KDE-PluginInfo-EnabledByDefault=true\n\
                     X-Plasma-API=DBus\n\
                     X-Plasma-API-Minimum-Version=2.0\n\
                     X-Plasma-DBusRunner-Service={KRUNNER_BUS}\n\
                     X-Plasma-DBusRunner-Path={KRUNNER_PATH}\n\
                     X-Plasma-Runner-Min-Letter-Count=3\n\
                     {MARKER}=true\n"
                );
                install(&mut rec, &krunner_file(env), &plugin, 0o644)?;
                let svc = service_entry(env, KRUNNER_BUS, SERVICE_ARG, WHY);
                install(&mut rec, &service_file(env, KRUNNER_BUS), &svc, 0o644)?;
            }
        }
    }
    rec.save(env, key)?;
    let _ = fs::remove_file(old_recent_list(env));
    reload_bus(env);
    Ok(())
}

pub(super) fn remove(env: &Env) -> Result<()> {
    let key = OptionId::SearchProvider.key();
    let mut rec = Record::load(env, key).unwrap_or_default();
    for p in [
        gnome_ini(env),
        krunner_file(env),
        service_file(env, GNOME_BUS),
        service_file(env, KRUNNER_BUS),
    ] {
        rec.add_file(&p);
    }
    uninstall(&rec);
    Record::delete(env, key);
    let _ = fs::remove_file(old_recent_list(env));
    reload_bus(env);
    Ok(())
}

pub(super) fn status(env: &Env) -> (State, Vec<String>) {
    let all = front_ends(env);
    let on: Vec<String> = all
        .iter()
        .filter(|fe| match fe {
            FrontEnd::GnomeShell => {
                is_ours(&gnome_ini(env)) && is_ours(&service_file(env, GNOME_BUS))
            }
            FrontEnd::KRunner => {
                is_ours(&krunner_file(env)) && is_ours(&service_file(env, KRUNNER_BUS))
            }
        })
        .map(|fe| fe.name().to_owned())
        .collect();
    let state = if on.is_empty() {
        State::Off
    } else if on.len() == all.len() {
        State::On
    } else {
        State::Partial
    };
    (state, on)
}

// ----- Sources ----------------------------------------------------------------------------

/// A thumbnail: straight RGBA8, `width × height`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Icon {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// One result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub path: PathBuf,
    /// The first line (the file name).
    pub name: String,
    /// The second line (its folder, the date taken …).
    pub description: String,
}

impl Hit {
    /// A hit described by its name and folder.
    pub fn of_path(path: PathBuf) -> Hit {
        let name = path.file_name().map_or_else(
            || path.to_string_lossy().into_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let description = path
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        Hit {
            path,
            name,
            description,
        }
    }
}

/// Something to search: read-only, quick (a query comes per key press), never touching the
/// files themselves.
pub trait SearchSource: Send + Sync {
    /// At most `limit` pictures matching `query` (as typed), best first.
    fn search(&self, query: &str, limit: usize) -> Vec<Hit>;
    /// The thumbnail of `path`, one of the hits, when one is at hand.
    fn icon(&self, _path: &Path) -> Option<Icon> {
        None
    }
}

/// What the provider starts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Launch {
    /// Show this picture.
    Open(PathBuf),
    /// Show this folder.
    Folder(PathBuf),
    /// The Library, searching for this.
    Library(String),
}

impl Launch {
    /// The arguments of `specular` that do it.
    pub fn args(&self) -> Vec<OsString> {
        match self {
            Launch::Open(p) | Launch::Folder(p) => vec!["--".into(), p.into()],
            Launch::Library(q) => vec![SEARCH_ARG.into(), q.into()],
        }
    }
}

/// Texts the provider shows, in the user's language.
#[derive(Clone, Debug, Default)]
pub struct Texts {
    /// KRunner's action on a result: open its folder.
    pub open_folder: String,
}

/// A provider: where results come from and how picking one is carried out.
pub struct Provider {
    pub source: Box<dyn SearchSource>,
    pub texts: Texts,
    pub launch: Box<dyn Fn(Launch) + Send + Sync>,
}

impl Provider {
    /// Picking a result starts `exe` with [`Launch::args`].
    pub fn spawning(source: Box<dyn SearchSource>, texts: Texts, exe: PathBuf) -> Provider {
        Provider {
            source,
            texts,
            launch: Box::new(move |l: Launch| {
                info!(target: SETTINGS, "search: {l:?}");
                let r = std::process::Command::new(&exe)
                    .args(l.args())
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn();
                if let Err(e) = r {
                    specular_log::warn!(target: SETTINGS, "search: cannot start {}: {e}", exe.display());
                }
            }),
        }
    }
}

// ----- The provider process ---------------------------------------------------------------

/// A result handed out, with its icon once looked for.
type Known = (Hit, Option<Option<Icon>>);

struct Shared {
    provider: Provider,
    /// Results handed out, by id (the path).
    hits: Mutex<HashMap<String, Known>>,
    last: AtomicU64,
    start: Instant,
}

impl Shared {
    fn touch(&self) {
        self.last
            .store(self.start.elapsed().as_millis() as u64, Ordering::Relaxed);
    }

    fn search(&self, query: &str) -> Vec<Hit> {
        self.touch();
        let query = query.trim();
        // A letter alone finds too much to be worth a query.
        if query.chars().filter(|c| !c.is_whitespace()).count() < 2 {
            return Vec::new();
        }
        let hits = self.provider.source.search(query, LIMIT);
        let mut map = self.hits.lock().unwrap_or_else(|e| e.into_inner());
        if map.len() > REMEMBERED {
            map.clear();
        }
        for h in &hits {
            map.entry(h.path.to_string_lossy().into_owned())
                .or_insert_with(|| (h.clone(), None));
        }
        hits
    }

    fn ids(&self, terms: &[String]) -> Vec<String> {
        self.search(&terms.join(" "))
            .into_iter()
            .map(|h| h.path.to_string_lossy().into_owned())
            .collect()
    }

    /// A result handed out earlier, with its icon (read once).
    fn hit(&self, id: &str) -> Option<(Hit, Option<Icon>)> {
        let known = self
            .hits
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .cloned()?;
        let (hit, icon) = known;
        let icon = match icon {
            Some(i) => i,
            None => {
                let i = self.provider.source.icon(&hit.path);
                if let Some(e) = self
                    .hits
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_mut(id)
                {
                    e.1 = Some(i.clone());
                }
                i
            }
        };
        Some((hit, icon))
    }

    /// Carries out `l` for result `id`; only results handed out are opened.
    fn open(&self, id: &str, folder: bool) {
        self.touch();
        let path = self
            .hits
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(id)
            .map(|(h, _)| h.path.clone());
        let Some(path) = path else {
            debug!(target: SETTINGS, "search: {id:?} is not one of the results");
            return;
        };
        let l = if folder {
            match path.parent() {
                Some(d) => Launch::Folder(d.to_path_buf()),
                None => return,
            }
        } else {
            Launch::Open(path)
        };
        (self.provider.launch)(l);
    }
}

/// `icon-data`: a pixbuf as `(iiibiiay)` (width, height, row stride, alpha, bits per sample,
/// channels, bytes).
fn icon_data(i: &Icon) -> Value<'static> {
    Value::from((
        i.width as i32,
        i.height as i32,
        (i.width * 4) as i32,
        true,
        8i32,
        4i32,
        i.rgba.clone(),
    ))
}

struct Gnome(Arc<Shared>);

#[zbus::interface(name = "org.gnome.Shell.SearchProvider2")]
impl Gnome {
    fn get_initial_result_set(&self, terms: Vec<String>) -> Vec<String> {
        self.0.ids(&terms)
    }

    fn get_subsearch_result_set(&self, _previous: Vec<String>, terms: Vec<String>) -> Vec<String> {
        self.0.ids(&terms)
    }

    fn get_result_metas(&self, ids: Vec<String>) -> Vec<HashMap<String, Value<'static>>> {
        self.0.touch();
        ids.into_iter()
            .filter_map(|id| {
                let (hit, icon) = self.0.hit(&id)?;
                let mut m = HashMap::new();
                m.insert("id".to_owned(), Value::from(id));
                m.insert("name".to_owned(), Value::from(hit.name));
                m.insert("description".to_owned(), Value::from(hit.description));
                match icon {
                    Some(i) => m.insert("icon-data".to_owned(), icon_data(&i)),
                    None => m.insert(
                        "gicon".to_owned(),
                        Value::from("image-x-generic".to_owned()),
                    ),
                };
                Some(m)
            })
            .collect()
    }

    fn activate_result(&self, id: String, _terms: Vec<String>, _timestamp: u32) {
        self.0.open(&id, false);
    }

    fn launch_search(&self, terms: Vec<String>, _timestamp: u32) {
        self.0.touch();
        let q = terms.join(" ").trim().to_owned();
        (self.0.provider.launch)(Launch::Library(q));
    }
}

struct KRunner(Arc<Shared>);

/// KRunner match relevance types.
const POSSIBLE_MATCH: i32 = 30;

/// One KRunner match: id, text, icon name, type, relevance, properties.
type KMatch = (
    String,
    String,
    String,
    i32,
    f64,
    HashMap<String, Value<'static>>,
);

#[zbus::interface(name = "org.kde.krunner1")]
impl KRunner {
    fn actions(&self) -> Vec<(String, String, String)> {
        vec![(
            FOLDER_ACTION.to_owned(),
            self.0.provider.texts.open_folder.clone(),
            "folder-open".to_owned(),
        )]
    }

    fn r#match(&self, query: String) -> Vec<KMatch> {
        let hits = self.0.search(&query);
        let n = hits.len().max(1) as f64;
        hits.into_iter()
            .enumerate()
            .map(|(i, hit)| {
                let id = hit.path.to_string_lossy().into_owned();
                let mut props = HashMap::new();
                props.insert("subtext".to_owned(), Value::from(hit.description.clone()));
                props.insert(
                    "urls".to_owned(),
                    Value::from(vec![crate::launch::file_uri(&hit.path)]),
                );
                if let Some((_, Some(icon))) = self.0.hit(&id) {
                    props.insert("icon-data".to_owned(), icon_data(&icon));
                }
                // Best first: relevance falls with the position.
                let relevance = 0.7 - 0.2 * (i as f64 / n);
                (
                    id,
                    hit.name,
                    APP_ID.to_owned(),
                    POSSIBLE_MATCH,
                    relevance,
                    props,
                )
            })
            .collect()
    }

    fn run(&self, match_id: String, action_id: String) {
        self.0.open(&match_id, action_id == FOLDER_ACTION);
    }

    fn teardown(&self) {
        self.0.touch();
    }
}

/// Serves both providers on the session bus until `idle` passes without a query, picking a
/// result starting `env.exe`. `Ok(false)`: another provider process already owns the names.
pub fn serve(
    env: &Env,
    source: Box<dyn SearchSource>,
    texts: Texts,
    idle: Duration,
) -> Result<bool> {
    serve_on(
        None,
        Provider::spawning(source, texts, env.exe.clone()),
        idle,
    )
}

/// [`serve`] on the bus at `address` (None: the session bus) with `provider`.
pub fn serve_on(address: Option<&str>, provider: Provider, idle: Duration) -> Result<bool> {
    use zbus::fdo::{RequestNameFlags, RequestNameReply};
    let bus = |e: zbus::Error| Error::Bus(e.to_string());
    let shared = Arc::new(Shared {
        provider,
        hits: Mutex::new(HashMap::new()),
        last: AtomicU64::new(0),
        start: Instant::now(),
    });
    let builder = match address {
        Some(a) => zbus::blocking::connection::Builder::address(a).map_err(bus)?,
        None => zbus::blocking::connection::Builder::session().map_err(bus)?,
    };
    let conn = builder
        .serve_at(GNOME_PATH, Gnome(shared.clone()))
        .map_err(bus)?
        .serve_at(KRUNNER_PATH, KRunner(shared.clone()))
        .map_err(bus)?
        .build()
        .map_err(bus)?;
    let mut owned = 0;
    for name in [GNOME_BUS, KRUNNER_BUS] {
        match conn
            .request_name_with_flags(name, RequestNameFlags::DoNotQueue.into())
            .map_err(bus)?
        {
            RequestNameReply::PrimaryOwner | RequestNameReply::AlreadyOwner => owned += 1,
            r => debug!(target: SETTINGS, "search: {name} is taken ({r:?})"),
        }
    }
    if owned == 0 {
        return Ok(false);
    }
    info!(target: SETTINGS, "search: provider running");
    let tick = (idle / 4).clamp(Duration::from_millis(20), Duration::from_secs(1));
    loop {
        std::thread::sleep(tick);
        let last = Duration::from_millis(shared.last.load(Ordering::Relaxed));
        if shared.start.elapsed().saturating_sub(last) >= idle {
            info!(target: SETTINGS, "search: idle, exiting");
            return Ok(true);
        }
    }
}

/// A client of the provider, for tests (the app's against its catalogue): each call is one
/// D-Bus method call to the bus at `address`.
#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod client {
    use super::*;

    /// A result's metadata as GNOME Shell gets it.
    #[derive(Debug)]
    pub struct Meta {
        pub id: String,
        pub name: String,
        pub description: String,
        /// `icon-data`'s width and height.
        pub icon: Option<(i32, i32)>,
        /// `gicon` (without a thumbnail).
        pub gicon: Option<String>,
    }

    /// A KRunner match: id, text, subtext, whether it carries `icon-data`.
    pub type Match = (String, String, String, bool);

    fn conn(address: &str) -> zbus::blocking::Connection {
        zbus::blocking::connection::Builder::address(address)
            .unwrap()
            .build()
            .unwrap()
    }

    fn gnome(c: &zbus::blocking::Connection) -> zbus::blocking::Proxy<'static> {
        zbus::blocking::Proxy::new(c, GNOME_BUS, GNOME_PATH, "org.gnome.Shell.SearchProvider2")
            .unwrap()
    }

    fn krunner(c: &zbus::blocking::Connection) -> zbus::blocking::Proxy<'static> {
        zbus::blocking::Proxy::new(c, KRUNNER_BUS, KRUNNER_PATH, "org.kde.krunner1").unwrap()
    }

    /// Waits up to 10 s for the provider to own its names.
    pub fn wait_for(address: &str) {
        let c = conn(address);
        let dbus = zbus::blocking::fdo::DBusProxy::new(&c).unwrap();
        let t0 = Instant::now();
        while !dbus
            .name_has_owner(zbus::names::BusName::try_from(KRUNNER_BUS).unwrap())
            .unwrap_or(false)
        {
            assert!(t0.elapsed() < Duration::from_secs(10), "no provider");
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    pub fn initial(address: &str, terms: &[&str]) -> Vec<String> {
        let c = conn(address);
        gnome(&c)
            .call("GetInitialResultSet", &(terms.to_vec(),))
            .unwrap()
    }

    pub fn subsearch(address: &str, previous: &[String], terms: &[&str]) -> Vec<String> {
        let c = conn(address);
        gnome(&c)
            .call(
                "GetSubsearchResultSet",
                &(previous.to_vec(), terms.to_vec()),
            )
            .unwrap()
    }

    pub fn metas(address: &str, ids: &[String]) -> Vec<Meta> {
        use zbus::zvariant::OwnedValue;
        let c = conn(address);
        let v: Vec<HashMap<String, OwnedValue>> =
            gnome(&c).call("GetResultMetas", &(ids.to_vec(),)).unwrap();
        let s = |m: &HashMap<String, OwnedValue>, k: &str| -> Option<String> {
            m.get(k).and_then(|v| String::try_from(v.clone()).ok())
        };
        v.iter()
            .map(|m| Meta {
                id: s(m, "id").unwrap_or_default(),
                name: s(m, "name").unwrap_or_default(),
                description: s(m, "description").unwrap_or_default(),
                icon: m.get("icon-data").and_then(|v| {
                    let (w, h, _, _, _, _, _): (i32, i32, i32, bool, i32, i32, Vec<u8>) =
                        v.try_clone().ok()?.try_into().ok()?;
                    Some((w, h))
                }),
                gicon: s(m, "gicon"),
            })
            .collect()
    }

    pub fn activate(address: &str, id: &str) {
        let c = conn(address);
        let terms: Vec<String> = Vec::new();
        let _: () = gnome(&c)
            .call("ActivateResult", &(id, terms, 0u32))
            .unwrap();
    }

    pub fn launch_search(address: &str, terms: &[&str]) {
        let c = conn(address);
        let _: () = gnome(&c)
            .call("LaunchSearch", &(terms.to_vec(), 0u32))
            .unwrap();
    }

    pub fn krunner_actions(address: &str) -> Vec<(String, String, String)> {
        let c = conn(address);
        krunner(&c).call("Actions", &()).unwrap()
    }

    pub fn krunner_match(address: &str, query: &str) -> Vec<Match> {
        use zbus::zvariant::OwnedValue;
        let c = conn(address);
        #[allow(clippy::type_complexity)]
        let v: Vec<(
            String,
            String,
            String,
            i32,
            f64,
            HashMap<String, OwnedValue>,
        )> = krunner(&c).call("Match", &(query,)).unwrap();
        v.into_iter()
            .map(|(id, text, _, _, _, props)| {
                let sub = props
                    .get("subtext")
                    .and_then(|v| String::try_from(v.clone()).ok())
                    .unwrap_or_default();
                (id, text, sub, props.contains_key("icon-data"))
            })
            .collect()
    }

    pub fn krunner_run(address: &str, id: &str, action: &str) {
        let c = conn(address);
        let _: () = krunner(&c).call("Run", &(id, action)).unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_env;
    use super::*;

    fn env_with(name: &str, bins: &[&str]) -> Env {
        let mut env = test_env::env(name);
        let bin = env.data_home.parent().unwrap().join("path");
        fs::create_dir_all(&bin).unwrap();
        for f in bins {
            fs::write(bin.join(f), "").unwrap();
        }
        env.path = vec![bin];
        env
    }

    #[test]
    fn option_files() {
        let env = env_with("search", &["gnome-shell", "krunner"]);
        assert_eq!(front_ends(&env), [FrontEnd::GnomeShell, FrontEnd::KRunner]);
        assert_eq!(status(&env).0, State::Off);
        // A list kept by earlier versions goes.
        let old = old_recent_list(&env);
        fs::create_dir_all(old.parent().unwrap()).unwrap();
        fs::write(&old, "/a.jpg\n").unwrap();
        apply(&env).unwrap();
        assert!(!old.exists());
        assert_eq!(
            status(&env),
            (State::On, vec!["GNOME Shell".into(), "KRunner".into()])
        );
        assert_eq!(files(&env).len(), 4);
        assert!(files(&env).iter().all(|f| is_ours(f)));
        let svc = fs::read_to_string(service_file(&env, KRUNNER_BUS)).unwrap();
        assert!(svc.contains("Name=app.specular.Specular.KRunner\n"));
        assert!(svc.contains("specular' --search-provider\n"), "{svc}");
        let ini = fs::read_to_string(gnome_ini(&env)).unwrap();
        assert!(ini.contains("BusName=app.specular.Specular.SearchProvider\n"));
        assert!(ini.contains("Version=2\n"));
        let plugin = fs::read_to_string(krunner_file(&env)).unwrap();
        assert!(plugin.contains("X-Plasma-API=DBus\n"));
        assert!(plugin.contains("X-Plasma-DBusRunner-Path=/app/specular/KRunner\n"));
        apply(&env).unwrap();
        remove(&env).unwrap();
        assert_eq!(status(&env).0, State::Off);
        assert!(!gnome_ini(&env).exists() && !krunner_file(&env).exists());
        assert!(!service_file(&env, GNOME_BUS).exists());
        assert!(!service_file(&env, KRUNNER_BUS).exists());
        test_env::cleanup(&env);
    }

    #[test]
    fn foreign_files_stay() {
        let env = env_with("search-foreign", &["krunner"]);
        let p = krunner_file(&env);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "[Desktop Entry]\nName=Someone else's\n").unwrap();
        assert!(
            apply(&env).is_err(),
            "another program's file is not replaced"
        );
        remove(&env).unwrap();
        assert!(p.exists(), "nor removed");
        test_env::cleanup(&env);
    }

    #[test]
    fn launch_arguments() {
        assert_eq!(
            Launch::Open("/p/-x.jpg".into()).args(),
            [OsString::from("--"), OsString::from("/p/-x.jpg")]
        );
        assert_eq!(
            Launch::Library("tag:sea -is:raw".into()).args(),
            [
                OsString::from("--search"),
                OsString::from("tag:sea -is:raw")
            ]
        );
    }

    /// A fixed list, by name.
    struct Fixed(Vec<PathBuf>);

    impl SearchSource for Fixed {
        fn search(&self, query: &str, limit: usize) -> Vec<Hit> {
            self.0
                .iter()
                .filter(|p| p.to_string_lossy().contains(query))
                .take(limit)
                .cloned()
                .map(Hit::of_path)
                .collect()
        }
        fn icon(&self, path: &Path) -> Option<Icon> {
            path.to_string_lossy().contains("beach").then(|| Icon {
                width: 2,
                height: 1,
                rgba: vec![255; 8],
            })
        }
    }

    #[test]
    fn methods_on_a_private_bus() {
        let Some(bus) = crate::test_bus::Bus::start("srch") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let launched = Arc::new(Mutex::new(Vec::new()));
        let l = launched.clone();
        let provider = Provider {
            source: Box::new(Fixed(vec![
                "/p/beach 1.jpg".into(),
                "/p/beach 2.jpg".into(),
                "/q/hill.png".into(),
            ])),
            texts: Texts {
                open_folder: "Open folder".into(),
            },
            launch: Box::new(move |x| l.lock().unwrap().push(x)),
        };
        let address = bus.address.clone();
        let server = specular_thread::spawn("srch-test", move || {
            serve_on(Some(&address), provider, Duration::from_millis(1500))
        });
        let a = bus.address.as_str();
        client::wait_for(a);

        let ids = client::initial(a, &["beach"]);
        assert_eq!(ids, ["/p/beach 1.jpg", "/p/beach 2.jpg"]);
        assert_eq!(
            client::subsearch(a, &ids, &["beach", "2"]),
            ["/p/beach 2.jpg"]
        );
        assert_eq!(client::subsearch(a, &ids, &["hill"]), ["/q/hill.png"]);
        assert!(client::initial(a, &["b"]).is_empty(), "one letter: nothing");

        let metas = client::metas(
            a,
            &[
                "/p/beach 1.jpg".into(),
                "/q/hill.png".into(),
                "/etc/passwd".into(),
            ],
        );
        assert_eq!(metas.len(), 2, "unknown ids get no metadata");
        assert_eq!(metas[0].name, "beach 1.jpg");
        assert_eq!(metas[0].description, "/p");
        assert_eq!(metas[0].icon, Some((2, 1)));
        assert_eq!(metas[1].icon, None);
        assert_eq!(metas[1].gicon.as_deref(), Some("image-x-generic"));

        client::activate(a, "/p/beach 1.jpg");
        client::activate(a, "/etc/passwd");
        client::launch_search(a, &["tag:sea", "dunes"]);

        assert_eq!(
            client::krunner_actions(a),
            [("folder".into(), "Open folder".into(), "folder-open".into())]
        );
        let m = client::krunner_match(a, "hill");
        assert_eq!(
            m,
            [("/q/hill.png".into(), "hill.png".into(), "/q".into(), false)]
        );
        assert!(client::krunner_match(a, "beach")[0].3, "thumbnail sent");
        client::krunner_run(a, "/q/hill.png", "");
        client::krunner_run(a, "/q/hill.png", "folder");
        client::krunner_run(a, "/nowhere.png", "");

        assert_eq!(
            *launched.lock().unwrap(),
            [
                Launch::Open("/p/beach 1.jpg".into()),
                Launch::Library("tag:sea dunes".into()),
                Launch::Open("/q/hill.png".into()),
                Launch::Folder("/q".into()),
            ]
        );
        // Exits once idle.
        assert!(matches!(server.join().unwrap(), Ok(true)));
    }
}
