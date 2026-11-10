//! Linux: the mount table, udisks2 for which mounts are removable (and for eject), and gvfs's
//! FUSE folder for phones and cameras over MTP/PTP.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use specular_log::debug;
use zbus::zvariant::{OwnedValue, Value};

use super::{EjectError, Volume, VolumeKind};

const UDISKS_BUS: &str = "org.freedesktop.UDisks2";
const UDISKS_PATH: &str = "/org/freedesktop/UDisks2";
const BLOCK: &str = "org.freedesktop.UDisks2.Block";
const FILESYSTEM: &str = "org.freedesktop.UDisks2.Filesystem";
const DRIVE: &str = "org.freedesktop.UDisks2.Drive";

/// One line of `/proc/self/mountinfo`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Mount {
    pub point: PathBuf,
    pub fstype: String,
}

/// The mount points and types of a `mountinfo` table (octal escapes such as `\040` undone).
pub(crate) fn parse_mountinfo(text: &str) -> Vec<Mount> {
    text.lines()
        .filter_map(|l| {
            let mut f = l.split(' ');
            let point = f.nth(4)?;
            let mut rest = f.skip_while(|x| *x != "-");
            rest.next()?;
            let fstype = rest.next()?;
            Some(Mount {
                point: PathBuf::from(unescape(point)),
                fstype: fstype.to_owned(),
            })
        })
        .collect()
}

fn unescape(s: &str) -> String {
    use std::os::unix::ffi::OsStringExt;
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'\\'
            && i + 3 < b.len()
            && b[i + 1..i + 4].iter().all(|c| (b'0'..=b'7').contains(c))
        {
            out.push((b[i + 1] - b'0') * 64 + (b[i + 2] - b'0') * 8 + (b[i + 3] - b'0'));
            i += 4;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    std::ffi::OsString::from_vec(out)
        .to_string_lossy()
        .into_owned()
}

/// A mounted file system as udisks2 describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UdisksFs {
    pub block: String,
    pub drive: String,
    pub mounts: Vec<PathBuf>,
    pub label: String,
    /// A removable or ejectable drive, or one udisks does not count as a system device.
    pub removable: bool,
    /// The drive's vendor and model, for a name when the file system has no label.
    pub model: String,
}

fn value_str(v: Option<&OwnedValue>) -> String {
    match v.map(|v| &**v) {
        Some(Value::Str(s)) => s.as_str().to_owned(),
        Some(Value::ObjectPath(p)) => p.as_str().to_owned(),
        _ => String::new(),
    }
}

fn value_bool(v: Option<&OwnedValue>) -> bool {
    matches!(v.map(|v| &**v), Some(Value::Bool(true)))
}

fn value_bytes_list(v: Option<&OwnedValue>) -> Vec<Vec<u8>> {
    let Some(Value::Array(a)) = v.map(|v| &**v) else {
        return Vec::new();
    };
    a.inner()
        .iter()
        .filter_map(|x| match x {
            Value::Array(b) => Some(
                b.inner()
                    .iter()
                    .filter_map(|y| match y {
                        Value::U8(c) => Some(*c),
                        _ => None,
                    })
                    .collect(),
            ),
            _ => None,
        })
        .collect()
}

/// Every mounted file system udisks2 knows, with its drive's facts.
pub(crate) fn udisks_filesystems(conn: &zbus::blocking::Connection) -> zbus::Result<Vec<UdisksFs>> {
    use std::os::unix::ffi::OsStringExt;
    let om = zbus::blocking::fdo::ObjectManagerProxy::builder(conn)
        .destination(UDISKS_BUS)?
        .path(UDISKS_PATH)?
        .build()?;
    let objects = om.get_managed_objects()?;
    let iface = |path: &str, name: &str| {
        objects
            .iter()
            .find(|(p, _)| p.as_str() == path)
            .and_then(|(_, i)| i.iter().find(|(n, _)| n.as_str() == name))
            .map(|(_, props)| props)
    };
    let mut out = Vec::new();
    for (path, ifaces) in &objects {
        let get = |name: &str| {
            ifaces
                .iter()
                .find(|(n, _)| n.as_str() == name)
                .map(|(_, p)| p)
        };
        let (Some(fs), Some(block)) = (get(FILESYSTEM), get(BLOCK)) else {
            continue;
        };
        let mounts: Vec<PathBuf> = value_bytes_list(fs.get("MountPoints"))
            .into_iter()
            .map(|mut b| {
                while b.last() == Some(&0) {
                    b.pop();
                }
                PathBuf::from(std::ffi::OsString::from_vec(b))
            })
            .filter(|p| !p.as_os_str().is_empty())
            .collect();
        if mounts.is_empty() {
            continue;
        }
        let drive = value_str(block.get("Drive"));
        let d = iface(&drive, DRIVE);
        let removable = d.is_some_and(|d| {
            value_bool(d.get("Removable"))
                || value_bool(d.get("Ejectable"))
                || value_bool(d.get("MediaRemovable"))
        }) || !value_bool(block.get("HintSystem"));
        let model = d
            .map(|d| {
                let v = value_str(d.get("Vendor"));
                let m = value_str(d.get("Model"));
                [v.trim(), m.trim()]
                    .iter()
                    .filter(|s| !s.is_empty())
                    .copied()
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .unwrap_or_default();
        out.push(UdisksFs {
            block: path.as_str().to_owned(),
            drive,
            mounts,
            label: value_str(block.get("IdLabel")).trim().to_owned(),
            removable,
            model,
        });
    }
    out.sort_by(|a, b| a.block.cmp(&b.block));
    Ok(out)
}

/// The places removable media are mounted by convention, for systems without udisks2.
fn conventional(point: &Path, user: &str) -> bool {
    let under = |root: &Path| point.parent() == Some(root);
    (!user.is_empty()
        && (under(&Path::new("/run/media").join(user)) || under(&Path::new("/media").join(user))))
        || (under(Path::new("/media")) && point != Path::new("/media").join(user))
}

/// A readable name from a gvfs mount folder (`mtp:host=Google_Pixel_7_2A1B…`,
/// `gphoto2:host=Canon_Inc._Canon_Digital_Camera`).
pub(crate) fn gvfs_name(folder: &str) -> Option<String> {
    let (scheme, rest) = folder.split_once(':')?;
    if scheme != "mtp" && scheme != "gphoto2" {
        return None;
    }
    let host = rest
        .split(',')
        .find_map(|kv| kv.strip_prefix("host="))
        .unwrap_or(rest);
    let host = percent_decode(host).replace('_', " ");
    let host = host.trim();
    Some(if host.is_empty() {
        folder.to_owned()
    } else {
        host.to_owned()
    })
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && i + 2 < b.len()
            && let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16)
        {
            out.push(v);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The volumes from the parts gathered by [`list`] (separate so tests can feed them).
pub(crate) fn assemble(
    mounts: &[Mount],
    udisks: Option<&[UdisksFs]>,
    user: &str,
    gvfs_root: &Path,
    gvfs_folders: &[String],
) -> Vec<Volume> {
    let mut out = Vec::new();
    for m in mounts {
        let Some(name) = m
            .point
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
        else {
            continue;
        };
        match udisks {
            Some(fs) => {
                let Some(f) = fs.iter().find(|f| f.mounts.contains(&m.point)) else {
                    continue;
                };
                if !f.removable || m.point == Path::new("/") {
                    continue;
                }
                let name = if !f.label.is_empty() {
                    f.label.clone()
                } else if !f.model.is_empty() {
                    f.model.clone()
                } else {
                    name
                };
                out.push(Volume {
                    mount: m.point.clone(),
                    name,
                    kind: VolumeKind::Removable,
                    udisks: Some((f.block.clone(), f.drive.clone())),
                });
            }
            None => {
                if conventional(&m.point, user) {
                    out.push(Volume::at(m.point.clone(), name, VolumeKind::Removable));
                }
            }
        }
    }
    for f in gvfs_folders {
        if let Some(name) = gvfs_name(f) {
            out.push(Volume::at(gvfs_root.join(f), name, VolumeKind::Device));
        }
    }
    out.sort_by(|a, b| a.mount.cmp(&b.mount));
    out.dedup_by(|a, b| a.mount == b.mount);
    out
}

fn gvfs_root() -> PathBuf {
    let uid = rustix::process::getuid().as_raw();
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{uid}")))
        .join("gvfs")
}

fn gvfs_folders(root: &Path) -> Vec<String> {
    let mut v: Vec<String> = std::fs::read_dir(root)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

fn user() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("LOGNAME"))
        .unwrap_or_default()
}

pub(super) fn stamp() -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::fs::read("/proc/self/mountinfo")
        .unwrap_or_default()
        .hash(&mut h);
    gvfs_folders(&gvfs_root()).hash(&mut h);
    h.finish()
}

fn system_bus() -> Option<zbus::blocking::Connection> {
    if !Path::new("/run/dbus/system_bus_socket").exists()
        && std::env::var_os("DBUS_SYSTEM_BUS_ADDRESS").is_none()
    {
        return None;
    }
    zbus::blocking::Connection::system()
        .inspect_err(|e| debug!("volumes: system bus: {e}"))
        .ok()
}

pub(super) fn list() -> Vec<Volume> {
    let text = std::fs::read_to_string("/proc/self/mountinfo").unwrap_or_default();
    let mounts = parse_mountinfo(&text);
    let udisks = system_bus().and_then(|c| {
        udisks_filesystems(&c)
            .inspect_err(|e| debug!("volumes: udisks2: {e}"))
            .ok()
    });
    let root = gvfs_root();
    assemble(
        &mounts,
        udisks.as_deref(),
        &user(),
        &root,
        &gvfs_folders(&root),
    )
}

pub(super) fn eject(v: &Volume) -> Result<(), EjectError> {
    let Some(conn) = system_bus() else {
        return Err(EjectError::Unsupported);
    };
    eject_on(&conn, v)
}

pub(crate) fn eject_on(conn: &zbus::blocking::Connection, v: &Volume) -> Result<(), EjectError> {
    let Some((block, drive)) = &v.udisks else {
        return Err(EjectError::Unsupported);
    };
    let fail = |e: zbus::Error| EjectError::Failed(e.to_string());
    let opts: HashMap<&str, Value<'_>> = HashMap::new();
    conn.call_method(
        Some(UDISKS_BUS),
        block.as_str(),
        Some(FILESYSTEM),
        "Unmount",
        &(&opts,),
    )
    .map_err(fail)?;
    if drive.is_empty() || drive == "/" {
        return Ok(());
    }
    match conn.call_method(
        Some(UDISKS_BUS),
        drive.as_str(),
        Some(DRIVE),
        "Eject",
        &(&opts,),
    ) {
        Ok(_) => Ok(()),
        // Card readers that cannot eject: power the drive off instead; the file system is
        // already unmounted, so the card is safe to pull either way.
        Err(e) => {
            debug!("volumes: eject {drive}: {e}; trying power-off");
            conn.call_method(
                Some(UDISKS_BUS),
                drive.as_str(),
                Some(DRIVE),
                "PowerOff",
                &(&opts,),
            )
            .map(|_| ())
            .or(Ok(()))
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Tests may start unnamed threads.
mod tests {
    //! udisks2 is faked on a private `dbus-daemon` with no service directories: nothing real is
    //! asked, unmounted or ejected.

    use std::io::Read;
    use std::process::{Child, Command, Stdio};
    use std::sync::{Arc, Mutex};

    use zbus::zvariant::OwnedObjectPath;

    use super::*;

    #[test]
    fn mountinfo_lines() {
        let t = "36 25 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw\n\
                 99 36 179:1 / /run/media/ann/EOS_DIGITAL rw,nosuid shared:50 - vfat /dev/mmcblk0p1 rw\n\
                 100 36 8:33 / /media/ann/My\\040Card rw shared:51 - exfat /dev/sdc1 rw\n\
                 bad line\n";
        let m = parse_mountinfo(t);
        assert_eq!(m.len(), 3);
        assert_eq!(m[1].point, Path::new("/run/media/ann/EOS_DIGITAL"));
        assert_eq!(m[1].fstype, "vfat");
        assert_eq!(m[2].point, Path::new("/media/ann/My Card"));
    }

    #[test]
    fn conventions_without_udisks() {
        let t = "1 0 8:1 / / rw - ext4 /dev/sda1 rw\n\
                 2 1 8:2 / /home rw - ext4 /dev/sda2 rw\n\
                 3 1 179:1 / /run/media/ann/EOS_DIGITAL rw - vfat /dev/mmcblk0p1 rw\n\
                 4 1 8:33 / /media/ann/CARD rw - exfat /dev/sdc1 rw\n\
                 5 1 8:34 / /media/usb0 rw - vfat /dev/sdd1 rw\n\
                 6 1 8:35 / /run/media/bob/OTHER rw - vfat /dev/sde1 rw\n";
        let v = assemble(
            &parse_mountinfo(t),
            None,
            "ann",
            Path::new("/run/user/1000/gvfs"),
            &[
                "mtp:host=Google_Pixel_7_2A1B".into(),
                "gphoto2:host=Canon_Inc._Canon_Digital_Camera".into(),
                "smb-share:server=nas,share=photos".into(),
            ],
        );
        let names: Vec<_> = v.iter().map(|v| (v.name.as_str(), v.kind)).collect();
        assert_eq!(
            names,
            [
                ("CARD", VolumeKind::Removable),
                ("usb0", VolumeKind::Removable),
                ("EOS_DIGITAL", VolumeKind::Removable),
                ("Canon Inc. Canon Digital Camera", VolumeKind::Device),
                ("Google Pixel 7 2A1B", VolumeKind::Device),
            ]
        );
        assert!(!v[0].can_eject(), "no udisks, no eject");
    }

    #[test]
    fn gvfs_names() {
        assert_eq!(
            gvfs_name("mtp:host=SAMSUNG_SAMSUNG_Android%20X").as_deref(),
            Some("SAMSUNG SAMSUNG Android X")
        );
        assert_eq!(gvfs_name("ftp:host=x"), None);
    }

    // The fake udisks2: a Block + Filesystem per partition and a Drive, under an ObjectManager.

    #[derive(Default)]
    struct Calls(Vec<String>);

    struct FakeBlock {
        label: String,
        drive: OwnedObjectPath,
        system: bool,
    }

    #[zbus::interface(name = "org.freedesktop.UDisks2.Block")]
    impl FakeBlock {
        #[zbus(property, name = "IdLabel")]
        fn id_label(&self) -> String {
            self.label.clone()
        }
        #[zbus(property, name = "Drive")]
        fn drive(&self) -> OwnedObjectPath {
            self.drive.clone()
        }
        #[zbus(property, name = "HintSystem")]
        fn hint_system(&self) -> bool {
            self.system
        }
    }

    struct FakeFs {
        mounts: Vec<Vec<u8>>,
        calls: Arc<Mutex<Calls>>,
        name: String,
    }

    #[zbus::interface(name = "org.freedesktop.UDisks2.Filesystem")]
    impl FakeFs {
        #[zbus(property, name = "MountPoints")]
        fn mount_points(&self) -> Vec<Vec<u8>> {
            self.mounts.clone()
        }
        fn unmount(&self, _options: HashMap<String, OwnedValue>) {
            self.calls
                .lock()
                .unwrap()
                .0
                .push(format!("unmount {}", self.name));
        }
    }

    struct FakeDrive {
        removable: bool,
        can_eject: bool,
        calls: Arc<Mutex<Calls>>,
        name: String,
    }

    #[zbus::interface(name = "org.freedesktop.UDisks2.Drive")]
    impl FakeDrive {
        #[zbus(property, name = "Removable")]
        fn removable(&self) -> bool {
            self.removable
        }
        #[zbus(property, name = "Ejectable")]
        fn ejectable(&self) -> bool {
            self.can_eject
        }
        #[zbus(property, name = "Vendor")]
        fn vendor(&self) -> String {
            "Generic".into()
        }
        #[zbus(property, name = "Model")]
        fn model(&self) -> String {
            "SD Reader".into()
        }
        fn eject(&self, _options: HashMap<String, OwnedValue>) -> zbus::fdo::Result<()> {
            if !self.can_eject {
                return Err(zbus::fdo::Error::NotSupported("no".into()));
            }
            self.calls
                .lock()
                .unwrap()
                .0
                .push(format!("eject {}", self.name));
            Ok(())
        }
        fn power_off(&self, _options: HashMap<String, OwnedValue>) {
            self.calls
                .lock()
                .unwrap()
                .0
                .push(format!("power-off {}", self.name));
        }
    }

    struct Bus {
        child: Child,
        address: String,
        dir: PathBuf,
    }

    impl Bus {
        fn start() -> Option<Bus> {
            let daemon = crate::print::find_program("dbus-daemon")?;
            let dir = std::env::temp_dir().join(format!("vb{}", std::process::id() % 100_000));
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
                .unwrap()
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

    fn path(s: &str) -> OwnedObjectPath {
        OwnedObjectPath::try_from(s).unwrap()
    }

    #[test]
    fn udisks_says_which_mounts_are_removable_and_ejects_them() {
        let Some(bus) = Bus::start() else {
            eprintln!("no dbus-daemon: skipped");
            return;
        };
        let calls = Arc::new(Mutex::new(Calls::default()));
        let root = "/org/freedesktop/UDisks2";
        let card = format!("{root}/block_devices/mmcblk0p1");
        let disk = format!("{root}/block_devices/sda2");
        let reader = format!("{root}/drives/Generic_SD_Reader");
        let ssd = format!("{root}/drives/Samsung_SSD");
        let server = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .serve_at(root, zbus::fdo::ObjectManager)
            .unwrap()
            .serve_at(
                card.as_str(),
                FakeBlock {
                    label: "EOS_DIGITAL".into(),
                    drive: path(&reader),
                    system: false,
                },
            )
            .unwrap()
            .serve_at(
                card.as_str(),
                FakeFs {
                    mounts: vec![b"/run/media/ann/EOS_DIGITAL\0".to_vec()],
                    calls: calls.clone(),
                    name: "card".into(),
                },
            )
            .unwrap()
            .serve_at(
                reader.as_str(),
                FakeDrive {
                    removable: true,
                    can_eject: false,
                    calls: calls.clone(),
                    name: "reader".into(),
                },
            )
            .unwrap()
            .serve_at(
                disk.as_str(),
                FakeBlock {
                    label: String::new(),
                    drive: path(&ssd),
                    system: true,
                },
            )
            .unwrap()
            .serve_at(
                disk.as_str(),
                FakeFs {
                    mounts: vec![b"/home\0".to_vec()],
                    calls: calls.clone(),
                    name: "home".into(),
                },
            )
            .unwrap()
            .serve_at(
                ssd.as_str(),
                FakeDrive {
                    removable: false,
                    can_eject: false,
                    calls: calls.clone(),
                    name: "ssd".into(),
                },
            )
            .unwrap()
            .name(UDISKS_BUS)
            .unwrap()
            .build()
            .unwrap();

        let conn = bus.connect();
        let fs = udisks_filesystems(&conn).unwrap();
        assert_eq!(fs.len(), 2);
        let card_fs = fs.iter().find(|f| f.block == card).unwrap();
        assert!(card_fs.removable);
        assert_eq!(card_fs.label, "EOS_DIGITAL");
        assert_eq!(card_fs.model, "Generic SD Reader");
        assert_eq!(
            card_fs.mounts,
            [PathBuf::from("/run/media/ann/EOS_DIGITAL")]
        );
        assert!(!fs.iter().find(|f| f.block == disk).unwrap().removable);

        let t = "1 0 8:2 / /home rw - ext4 /dev/sda2 rw\n\
                 2 1 179:1 / /run/media/ann/EOS_DIGITAL rw - vfat /dev/mmcblk0p1 rw\n";
        let v = assemble(&parse_mountinfo(t), Some(&fs), "ann", Path::new("/x"), &[]);
        assert_eq!(v.len(), 1, "{v:?}");
        assert_eq!(v[0].name, "EOS_DIGITAL");
        assert!(v[0].can_eject());

        // The reader cannot eject: unmount, then power off.
        eject_on(&conn, &v[0]).unwrap();
        assert_eq!(
            calls.lock().unwrap().0,
            ["unmount card", "power-off reader"]
        );
        drop(server);
    }
}
