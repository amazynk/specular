//! The XDG Print portal, else `lp`.
//!
//! `PreparePrint(parent, title, settings, page_setup, options)` shows the desktop's print dialog
//! with our initial settings (GTK print settings and page setup as `a{sv}`; settings values are
//! strings) and answers with the chosen settings and a `token`; `Print(parent, title, fd,
//! options)` with that token sends the PDF. Both answer through an
//! `org.freedesktop.portal.Request` object's `Response` signal (0 done, 1 cancelled, 2 failed),
//! subscribed to before the call so the answer cannot be missed.

use std::collections::HashMap;
use std::ffi::OsString;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use specular_log::{SETTINGS, info, warn};
use zbus::zvariant::{Fd, OwnedValue, Value};

use super::{Error, Job, Printed};

const PORTAL_BUS: &str = "org.freedesktop.portal.Desktop";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_IFACE: &str = "org.freedesktop.portal.Print";
/// The system print dialog waits for the user.
const DIALOG_TIMEOUT: Duration = Duration::from_secs(30 * 60);
/// Handing the job over (the backend may still ask, e.g. for a password).
const PRINT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub(super) fn available() -> bool {
    crate::integration::linux::dbus::session_bus_exists() || super::find_program("lp").is_some()
}

pub(super) fn print(pdf: &Path, job: &Job) -> Result<Printed, Error> {
    if crate::integration::linux::dbus::session_bus_exists() {
        let conn = zbus::blocking::Connection::session().map_err(bus);
        match conn.as_ref().map_err(|e| Error::Bus(e.to_string())) {
            Ok(conn) => match Portal::new(conn) {
                Ok(portal) => {
                    return portal
                        .print(conn, pdf, job, DIALOG_TIMEOUT, PRINT_TIMEOUT)
                        .map(|()| Printed::Portal);
                }
                Err(e) => info!(target: SETTINGS, "print: no portal ({e}), trying lp"),
            },
            Err(e) => info!(target: SETTINGS, "print: no session bus ({e}), trying lp"),
        }
    }
    let Some(lp) = super::find_program("lp") else {
        return Err(Error::NoPrinting);
    };
    let out = Command::new(&lp)
        .args(lp_args(pdf, job))
        .output()
        .map_err(Error::Io)?;
    if out.status.success() {
        info!(target: SETTINGS, "print: sent to lp: {}", String::from_utf8_lossy(&out.stdout).trim());
        Ok(Printed::Lp(lp))
    } else {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_owned();
        warn!(target: SETTINGS, "print: lp failed: {err}");
        Err(Error::Tool(err))
    }
}

/// Asks CUPS about the default printer's paper sizes (`lpoptions -l`).
pub(super) fn borderless() -> super::Borderless {
    let Some(lpoptions) = super::find_program("lpoptions") else {
        return super::Borderless::Unknown;
    };
    match std::process::Command::new(lpoptions)
        .arg("-l")
        .env("LC_ALL", "C")
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
    {
        Ok(o) if o.status.success() => {
            super::borderless_from_lpoptions(&String::from_utf8_lossy(&o.stdout))
        }
        _ => super::Borderless::Unknown,
    }
}

fn bus(e: zbus::Error) -> Error {
    Error::Bus(e.to_string())
}

/// `lp` arguments: default printer, copies, title, media and orientation.
fn lp_args(pdf: &Path, job: &Job) -> Vec<OsString> {
    let mut a: Vec<OsString> = vec![
        "-n".into(),
        job.copies.max(1).to_string().into(),
        "-t".into(),
        job.title.clone().into(),
    ];
    let media = if job.paper.pwg.is_empty() {
        format!(
            "custom_{:.2}x{:.2}mm",
            job.paper.width_mm, job.paper.height_mm
        )
    } else {
        job.paper.pwg.clone()
    };
    a.extend(["-o".into(), format!("media={media}").into()]);
    if job.landscape {
        // The PDF's pages are already landscape; only tell CUPS not to rotate them back.
        a.extend(["-o".into(), "orientation-requested=4".into()]);
    }
    a.extend(["--".into(), pdf.as_os_str().to_owned()]);
    a
}

/// GTK print settings (values are strings).
fn settings(job: &Job) -> HashMap<&'static str, Value<'static>> {
    let mut s = HashMap::new();
    s.insert("n-copies", Value::from(job.copies.max(1).to_string()));
    s.insert(
        "orientation",
        Value::from(if job.landscape {
            "landscape"
        } else {
            "portrait"
        }),
    );
    s.insert("output-file-format", Value::from("pdf"));
    s
}

/// GTK page setup.
fn page_setup(job: &Job) -> HashMap<&'static str, Value<'static>> {
    let p = &job.paper;
    let mut s = HashMap::new();
    if !p.pwg.is_empty() {
        s.insert("PPDName", Value::from(p.display.clone()));
        s.insert("Name", Value::from(p.pwg.clone()));
    }
    s.insert("DisplayName", Value::from(p.display.clone()));
    s.insert("Width", Value::from(p.width_mm));
    s.insert("Height", Value::from(p.height_mm));
    // The PDF's pages carry their own margins.
    for k in ["MarginTop", "MarginBottom", "MarginLeft", "MarginRight"] {
        s.insert(k, Value::from(0.0f64));
    }
    s.insert(
        "Orientation",
        Value::from(if job.landscape {
            "landscape"
        } else {
            "portrait"
        }),
    );
    s
}

/// A connection's proxy for the Print portal.
struct Portal<'c> {
    proxy: zbus::blocking::Proxy<'c>,
}

impl<'c> Portal<'c> {
    /// Fails when nothing implements the interface (no portal, or no print backend).
    fn new(conn: &'c zbus::blocking::Connection) -> Result<Self, Error> {
        let proxy = zbus::blocking::proxy::Builder::<zbus::blocking::Proxy>::new(conn)
            .destination(PORTAL_BUS)
            .and_then(|b| b.path(PORTAL_PATH))
            .and_then(|b| b.interface(PORTAL_IFACE))
            .map(|b| b.cache_properties(zbus::proxy::CacheProperties::No))
            .and_then(|b| b.build())
            .map_err(bus)?;
        proxy
            .get_property::<u32>("version")
            .map_err(|e| Error::Bus(format!("no Print portal: {e}")))?;
        Ok(Portal { proxy })
    }

    /// `PreparePrint`, then `Print` with the token it answered.
    fn print(
        &self,
        conn: &zbus::blocking::Connection,
        pdf: &Path,
        job: &Job,
        dialog_timeout: Duration,
        print_timeout: Duration,
    ) -> Result<(), Error> {
        let file = std::fs::File::open(pdf).map_err(Error::Io)?;
        let results = request(conn, dialog_timeout, |token| {
            let mut options: HashMap<&str, Value> = HashMap::new();
            options.insert("handle_token", Value::from(token));
            options.insert("modal", Value::from(true));
            self.proxy.call_method(
                "PreparePrint",
                &(
                    "",
                    job.title.as_str(),
                    settings(job),
                    page_setup(job),
                    options,
                ),
            )
        })?;
        let token = results
            .get("token")
            .and_then(|v| u32::try_from(v).ok())
            .ok_or_else(|| Error::Bus("PreparePrint answered without a token".into()))?;
        request(conn, print_timeout, |handle| {
            let mut options: HashMap<&str, Value> = HashMap::new();
            options.insert("handle_token", Value::from(handle));
            options.insert("modal", Value::from(true));
            options.insert("token", Value::from(token));
            self.proxy
                .call_method("Print", &("", job.title.as_str(), Fd::from(&file), options))
        })?;
        info!(target: SETTINGS, "print: handed to the portal");
        Ok(())
    }
}

/// One portal request: subscribes to its `Response`, calls `call(handle_token)` and waits.
fn request(
    conn: &zbus::blocking::Connection,
    timeout: Duration,
    call: impl FnOnce(&str) -> zbus::Result<zbus::message::Message>,
) -> Result<HashMap<String, OwnedValue>, Error> {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let token = format!(
        "specular_print_{}_{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    );
    let sender = conn
        .unique_name()
        .map(|n| n.trim_start_matches(':').replace('.', "_"))
        .ok_or_else(|| Error::Bus("no unique name".into()))?;
    let path = format!("{PORTAL_PATH}/request/{sender}/{token}");
    let request = zbus::blocking::Proxy::new(
        conn,
        PORTAL_BUS,
        path.as_str(),
        "org.freedesktop.portal.Request",
    )
    .map_err(bus)?;
    let mut responses = request.receive_signal("Response").map_err(bus)?;
    call(&token).map_err(bus)?;

    let (tx, rx) = std::sync::mpsc::channel();
    specular_thread::builder("plat-print")
        .spawn(move || {
            if let Some(msg) = responses.next() {
                let _ = tx.send(
                    msg.body()
                        .deserialize::<(u32, HashMap<String, OwnedValue>)>(),
                );
            }
        })
        .map_err(Error::Io)?;
    match rx.recv_timeout(timeout) {
        Ok(Ok((0, results))) => Ok(results),
        Ok(Ok((1, _))) => Err(Error::Cancelled),
        Ok(Ok((code, _))) => Err(Error::Bus(format!("the portal answered {code}"))),
        Ok(Err(e)) => Err(Error::Bus(e.to_string())),
        Err(_) => Err(Error::Bus("no answer from the portal".into())),
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    //! The portal path against a fake `org.freedesktop.portal.Print` on a private `dbus-daemon`
    //! with no service directories (nothing real can be activated, nothing reaches a printer).
    //! `lp` is never run: only its arguments are checked.

    use std::io::Read;
    use std::path::PathBuf;
    use std::process::{Child, Stdio};
    use std::sync::{Arc, Mutex};

    use zbus::zvariant::{ObjectPath, OwnedFd};

    use super::super::Paper;
    use super::*;

    fn job() -> Job {
        Job {
            title: "Holiday 2026".into(),
            copies: 2,
            paper: Paper {
                pwg: "iso_a4_210x297mm".into(),
                display: "A4".into(),
                width_mm: 210.0,
                height_mm: 297.0,
            },
            landscape: true,
        }
    }

    #[test]
    fn lp_arguments() {
        let a = lp_args(Path::new("/x/sheet.pdf"), &job());
        let a: Vec<String> = a.iter().map(|s| s.to_string_lossy().into_owned()).collect();
        assert_eq!(
            a,
            [
                "-n",
                "2",
                "-t",
                "Holiday 2026",
                "-o",
                "media=iso_a4_210x297mm",
                "-o",
                "orientation-requested=4",
                "--",
                "/x/sheet.pdf"
            ]
        );
        let mut j = job();
        j.paper.pwg.clear();
        j.paper.width_mm = 100.0;
        j.landscape = false;
        let a = lp_args(Path::new("-odd.pdf"), &j);
        assert!(a.contains(&"media=custom_100.00x297.00mm".into()));
        // A file name starting with '-' stays a file name.
        assert_eq!(a[a.len() - 2], "--");
    }

    type Options = HashMap<String, OwnedValue>;

    #[derive(Default)]
    struct Seen {
        prepared: Option<(String, Options, Options)>,
        printed: Option<(String, u32, Vec<u8>)>,
    }

    struct FakePrint {
        seen: Arc<Mutex<Seen>>,
        /// The code `PreparePrint` answers with.
        prepare_code: u32,
    }

    /// Emits `Response(code, results)` on the caller's request object, from another thread
    /// (after the method has returned its handle).
    fn respond(
        conn: &zbus::Connection,
        sender: String,
        handle: &str,
        code: u32,
        results: HashMap<String, OwnedValue>,
    ) -> ObjectPath<'static> {
        let path = format!(
            "{PORTAL_PATH}/request/{}/{handle}",
            sender.trim_start_matches(':').replace('.', "_")
        );
        let conn = zbus::blocking::Connection::from(conn.clone());
        let p = path.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            conn.emit_signal(
                Some(sender.as_str()),
                p.as_str(),
                "org.freedesktop.portal.Request",
                "Response",
                &(code, results),
            )
            .unwrap();
        });
        ObjectPath::try_from(path).unwrap()
    }

    fn handle(options: &HashMap<String, OwnedValue>) -> String {
        String::try_from(options["handle_token"].try_clone().unwrap()).unwrap()
    }

    // The portal's PreparePrint has five arguments, and zbus adds the connection and header.
    #[allow(clippy::too_many_arguments)]
    #[zbus::interface(name = "org.freedesktop.portal.Print")]
    impl FakePrint {
        #[zbus(property, name = "version")]
        fn version(&self) -> u32 {
            3
        }

        fn prepare_print(
            &self,
            #[zbus(connection)] conn: &zbus::Connection,
            #[zbus(header)] hdr: zbus::message::Header<'_>,
            _parent: String,
            title: String,
            settings: HashMap<String, OwnedValue>,
            page_setup: HashMap<String, OwnedValue>,
            options: HashMap<String, OwnedValue>,
        ) -> ObjectPath<'static> {
            self.seen.lock().unwrap().prepared = Some((title, settings, page_setup));
            let mut results = HashMap::new();
            results.insert("token".to_owned(), OwnedValue::from(77u32));
            let sender = hdr.sender().unwrap().to_string();
            respond(conn, sender, &handle(&options), self.prepare_code, results)
        }

        fn print(
            &self,
            #[zbus(connection)] conn: &zbus::Connection,
            #[zbus(header)] hdr: zbus::message::Header<'_>,
            _parent: String,
            title: String,
            fd: OwnedFd,
            options: HashMap<String, OwnedValue>,
        ) -> ObjectPath<'static> {
            let mut data = Vec::new();
            let fd: std::os::fd::OwnedFd = fd.into();
            std::fs::File::from(fd).read_to_end(&mut data).unwrap();
            let token = u32::try_from(&options["token"]).unwrap();
            self.seen.lock().unwrap().printed = Some((title, token, data));
            let sender = hdr.sender().unwrap().to_string();
            respond(conn, sender, &handle(&options), 0, HashMap::new())
        }
    }

    /// A private bus: no service directories, so nothing is ever activated on it.
    struct Bus {
        child: Child,
        address: String,
        dir: PathBuf,
    }

    impl Bus {
        fn start(name: &str) -> Option<Bus> {
            let daemon = super::super::find_program("dbus-daemon")?;
            let dir =
                std::env::temp_dir().join(format!("pb{name}{}", std::process::id() % 100_000));
            let _ = std::fs::remove_dir_all(&dir);
            std::fs::create_dir_all(&dir).unwrap();
            let config = dir.join("bus.conf");
            std::fs::write(
                &config,
                format!(
                    r#"<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:path={}/s</listen>
  <auth>EXTERNAL</auth>
  <policy context="default">
    <allow send_destination="*" eavesdrop="true"/>
    <allow eavesdrop="true"/>
    <allow own="*"/>
  </policy>
</busconfig>
"#,
                    dir.display()
                ),
            )
            .unwrap();
            let mut child = Command::new(daemon)
                .arg(format!("--config-file={}", config.display()))
                .args(["--nofork", "--print-address=1", "--nopidfile"])
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .ok()?;
            let mut out = child.stdout.take().unwrap();
            let mut address = String::new();
            let mut b = [0u8; 1];
            while out.read(&mut b).ok()? == 1 && b[0] != b'\n' {
                address.push(b[0] as char);
            }
            assert!(!address.is_empty(), "dbus-daemon did not start");
            Some(Bus {
                child,
                address,
                dir,
            })
        }

        fn connect(&self) -> zbus::blocking::Connection {
            zbus::blocking::connection::Builder::address(self.address.as_str())
                .unwrap_or_else(|e| panic!("{e}: {:?}", self.address))
                .build()
                .unwrap()
        }
    }

    impl Drop for Bus {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    fn fake_portal(bus: &Bus, prepare_code: u32) -> (zbus::blocking::Connection, Arc<Mutex<Seen>>) {
        let seen = Arc::new(Mutex::new(Seen::default()));
        let conn = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(
                PORTAL_PATH,
                FakePrint {
                    seen: seen.clone(),
                    prepare_code,
                },
            )
            .unwrap()
            .name(PORTAL_BUS)
            .unwrap()
            .build()
            .unwrap();
        (conn, seen)
    }

    #[test]
    fn portal_prepares_then_prints_the_file() {
        let Some(bus) = Bus::start("ok") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let (_server, seen) = fake_portal(&bus, 0);
        let pdf = bus.dir.join("sheet.pdf");
        std::fs::write(&pdf, b"%PDF-1.4 test").unwrap();

        let conn = bus.connect();
        let portal = Portal::new(&conn).unwrap();
        let t = Duration::from_secs(10);
        portal.print(&conn, &pdf, &job(), t, t).unwrap();

        let seen = seen.lock().unwrap();
        let (title, settings, setup) = seen.prepared.as_ref().unwrap();
        assert_eq!(title, "Holiday 2026");
        assert_eq!(
            String::try_from(settings["n-copies"].try_clone().unwrap()).unwrap(),
            "2"
        );
        assert_eq!(
            String::try_from(setup["Name"].try_clone().unwrap()).unwrap(),
            "iso_a4_210x297mm"
        );
        assert_eq!(f64::try_from(&setup["Width"]).unwrap(), 210.0);
        assert_eq!(
            String::try_from(setup["Orientation"].try_clone().unwrap()).unwrap(),
            "landscape"
        );
        let (title, token, data) = seen.printed.as_ref().unwrap();
        assert_eq!(title, "Holiday 2026");
        assert_eq!(*token, 77, "Print carries PreparePrint's token");
        assert_eq!(
            data, b"%PDF-1.4 test",
            "the portal reads the PDF through the fd"
        );
    }

    #[test]
    fn cancelling_the_system_dialog_prints_nothing() {
        let Some(bus) = Bus::start("cancel") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let (_server, seen) = fake_portal(&bus, 1);
        let pdf = bus.dir.join("sheet.pdf");
        std::fs::write(&pdf, b"%PDF").unwrap();
        let conn = bus.connect();
        let portal = Portal::new(&conn).unwrap();
        let t = Duration::from_secs(10);
        assert!(matches!(
            portal.print(&conn, &pdf, &job(), t, t),
            Err(Error::Cancelled)
        ));
        assert!(seen.lock().unwrap().printed.is_none());
    }

    #[test]
    fn a_bus_without_the_portal_is_no_portal() {
        let Some(bus) = Bus::start("none") else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let conn = bus.connect();
        assert!(matches!(Portal::new(&conn), Err(Error::Bus(_))));
    }
}
