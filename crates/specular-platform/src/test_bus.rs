//! A private D-Bus daemon for tests (never the user's session bus): a session-type bus on a
//! socket in the test's temporary folder, stopped and removed on drop. `None` without
//! `dbus-daemon` (the tests then skip).

use std::io::Read;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

pub struct Bus {
    child: Child,
    pub address: String,
    pub dir: PathBuf,
}

impl Bus {
    pub fn start(name: &str) -> Option<Bus> {
        let path = std::env::var_os("PATH")?;
        let daemon = std::env::split_paths(&path)
            .map(|d| d.join("dbus-daemon"))
            .find(|p| p.is_file())?;
        let dir = std::env::temp_dir().join(format!("tb{name}{}", std::process::id() % 100_000));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).ok()?;
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
        .ok()?;
        let mut child = Command::new(daemon)
            .arg(format!("--config-file={}", config.display()))
            .args(["--nofork", "--print-address=1", "--nopidfile"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let mut out = child.stdout.take()?;
        let mut address = String::new();
        let mut b = [0u8; 1];
        while out.read(&mut b).ok()? == 1 && b[0] != b'\n' {
            address.push(b[0] as char);
        }
        if address.is_empty() {
            let _ = child.kill();
            return None;
        }
        Some(Bus {
            child,
            address,
            dir,
        })
    }

    pub fn connect(&self) -> zbus::blocking::Connection {
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
