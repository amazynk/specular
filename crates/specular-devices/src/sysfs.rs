//! What Linux says about input devices in sysfs, without opening them: each
//! `/sys/class/input/{eventN,jsN}/device/` has `name`, `id/{vendor,product}` and
//! `capabilities/{ev,key,abs,rel}` (bitmaps as hexadecimal `unsigned long` words, the most
//! significant first) and `properties`.
//!
//! The joystick node `jsN` and the event node `eventN` of one device share the `device`
//! directory; [`scan`] pairs them.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use crate::Kind;
use crate::pad::{BTN_JOYSTICK, BTN_SOUTH};

/// A capability bitmap.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Bits(Vec<u64>);

impl Bits {
    /// Parses sysfs's form ("1 0 3": words of `word_bits` bits, most significant first).
    pub fn parse(s: &str, word_bits: u32) -> Bits {
        let mut out = Vec::new();
        for (k, w) in s.split_whitespace().rev().enumerate() {
            let Ok(v) = u64::from_str_radix(w, 16) else {
                continue;
            };
            for i in 0..word_bits.min(64) {
                if v >> i & 1 == 1 {
                    let bit = k as u64 * u64::from(word_bits) + u64::from(i);
                    let (word, b) = ((bit / 64) as usize, bit % 64);
                    if out.len() <= word {
                        out.resize(word + 1, 0);
                    }
                    out[word] |= 1 << b;
                }
            }
        }
        Bits(out)
    }

    /// The bitmap with these bits set (tests and fake trees).
    pub fn from_codes(codes: &[u16]) -> Bits {
        let mut b = Bits::default();
        for &c in codes {
            let w = usize::from(c / 64);
            if b.0.len() <= w {
                b.0.resize(w + 1, 0);
            }
            b.0[w] |= 1 << (c % 64);
        }
        b
    }

    /// Back to sysfs's form with 64-bit words (fake trees).
    pub fn to_sysfs(&self) -> String {
        if self.0.is_empty() {
            return "0".into();
        }
        self.0
            .iter()
            .rev()
            .map(|w| format!("{w:x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn has(&self, code: u16) -> bool {
        self.0
            .get(usize::from(code / 64))
            .is_some_and(|w| w >> (code % 64) & 1 == 1)
    }

    /// The codes set, in increasing order.
    pub fn codes(&self) -> Vec<u16> {
        let mut v = Vec::new();
        for (k, w) in self.0.iter().enumerate() {
            for i in 0..64 {
                if w >> i & 1 == 1 {
                    v.push((k * 64 + i) as u16);
                }
            }
        }
        v
    }

    pub fn any_in(&self, lo: u16, hi: u16) -> bool {
        (lo..=hi).any(|c| self.has(c))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Caps {
    pub key: Bits,
    pub abs: Bits,
    pub rel: Bits,
    pub props: Bits,
}

/// One device as sysfs describes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub name: String,
    pub vendor: u16,
    pub product: u16,
    pub kind: Kind,
    pub caps: Caps,
    /// `/dev/input/jsN`, when the joystick interface has it.
    pub js: Option<PathBuf>,
    /// `/dev/input/eventN`.
    pub event: Option<PathBuf>,
}

/// `INPUT_PROP_ACCELEROMETER`: motion sensors of a pad, not sticks.
const PROP_ACCELEROMETER: u16 = 6;

/// 3Dconnexion's own vendor id (newer devices) and Logitech's (older ones).
pub const VENDOR_3DCONNEXION: u16 = 0x256f;
pub const VENDOR_LOGITECH: u16 = 0x046d;

/// Whether the ids are a 3Dconnexion device (Logitech-made ones are 0xc603–0xc640).
pub fn is_spacemouse(vendor: u16, product: u16) -> bool {
    vendor == VENDOR_3DCONNEXION
        || (vendor == VENDOR_LOGITECH && (0xc603..=0xc640).contains(&product))
}

/// What the device is, or `None` for anything else (keyboards, mice, touchpads, motion
/// sensors).
pub fn classify(vendor: u16, product: u16, caps: &Caps) -> Option<Kind> {
    if caps.props.has(PROP_ACCELEROMETER) {
        return None;
    }
    if is_spacemouse(vendor, product) {
        return Some(Kind::SpaceMouse);
    }
    let sticks = caps.abs.has(0) && caps.abs.has(1);
    if caps.key.any_in(BTN_SOUTH, 0x13e) && sticks {
        return Some(Kind::Gamepad);
    }
    if caps.key.any_in(BTN_JOYSTICK, 0x12f) && sticks {
        return Some(Kind::Joystick);
    }
    None
}

fn read_trim(p: &Path) -> Option<String> {
    fs::read_to_string(p).ok().map(|s| s.trim().to_owned())
}

fn read_hex(p: &Path) -> u16 {
    read_trim(p)
        .and_then(|s| u16::from_str_radix(&s, 16).ok())
        .unwrap_or(0)
}

fn read_bits(p: &Path) -> Bits {
    read_trim(p)
        .map(|s| Bits::parse(&s, usize::BITS))
        .unwrap_or_default()
}

/// The devices under `class` (`/sys/class/input`) that are gamepads, joysticks or
/// SpaceMice, with their nodes under `dev` (`/dev/input`), in a stable order.
pub fn scan(class: &Path, dev: &Path) -> Vec<Found> {
    let Ok(rd) = fs::read_dir(class) else {
        return Vec::new();
    };
    let mut by_device: BTreeMap<PathBuf, Found> = BTreeMap::new();
    let mut entries: Vec<_> = rd.filter_map(Result::ok).map(|e| e.file_name()).collect();
    entries.sort();
    for e in entries {
        let Some(node) = e.to_str() else {
            continue;
        };
        let is_js = node.starts_with("js") && node[2..].bytes().all(|b| b.is_ascii_digit());
        let is_event = node.starts_with("event") && node[5..].bytes().all(|b| b.is_ascii_digit());
        if !(is_js || is_event) || node.len() <= 2 {
            continue;
        }
        let device = class.join(node).join("device");
        let key = fs::canonicalize(&device).unwrap_or_else(|_| device.clone());
        let found = match by_device.get_mut(&key) {
            Some(f) => f,
            None => {
                let caps = Caps {
                    key: read_bits(&device.join("capabilities/key")),
                    abs: read_bits(&device.join("capabilities/abs")),
                    rel: read_bits(&device.join("capabilities/rel")),
                    props: read_bits(&device.join("properties")),
                };
                let vendor = read_hex(&device.join("id/vendor"));
                let product = read_hex(&device.join("id/product"));
                let Some(kind) = classify(vendor, product, &caps) else {
                    continue;
                };
                let name = read_trim(&device.join("name")).unwrap_or_default();
                by_device.entry(key).or_insert(Found {
                    name,
                    vendor,
                    product,
                    kind,
                    caps,
                    js: None,
                    event: None,
                })
            }
        };
        let path = dev.join(node);
        if is_js {
            found.js = Some(path);
        } else {
            found.event = Some(path);
        }
    }
    by_device.into_values().collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes a fake sysfs device `input<n>` with nodes `nodes` (e.g. `["js0", "event5"]`).
    #[cfg(unix)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn fake_device(
        root: &Path,
        n: u32,
        name: &str,
        ids: (u16, u16),
        key: &[u16],
        abs: &[u16],
        rel: &[u16],
        nodes: &[&str],
    ) {
        let dev = root.join(format!("devices/input{n}"));
        fs::create_dir_all(dev.join("id")).unwrap();
        fs::create_dir_all(dev.join("capabilities")).unwrap();
        fs::write(dev.join("name"), format!("{name}\n")).unwrap();
        fs::write(dev.join("id/vendor"), format!("{:04x}\n", ids.0)).unwrap();
        fs::write(dev.join("id/product"), format!("{:04x}\n", ids.1)).unwrap();
        for (f, c) in [("key", key), ("abs", abs), ("rel", rel)] {
            fs::write(
                dev.join("capabilities").join(f),
                Bits::from_codes(c).to_sysfs() + "\n",
            )
            .unwrap();
        }
        fs::write(dev.join("properties"), "0\n").unwrap();
        let class = root.join("class");
        fs::create_dir_all(&class).unwrap();
        for node in nodes {
            fs::create_dir_all(class.join(node)).unwrap();
            std::os::unix::fs::symlink(&dev, class.join(node).join("device")).unwrap();
        }
    }

    #[test]
    fn bitmaps_parse_like_sysfs() {
        // A real Xbox pad's key bitmap (64-bit words).
        let b = Bits::parse("7cdb000000000000 0 0 0 0", 64);
        for c in [
            0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e,
        ] {
            assert!(b.has(c), "{c:#x}");
        }
        assert!(!b.has(0x132));
        // 32-bit words: bit 32 is word 1's bit 0.
        assert!(Bits::parse("1 0", 32).has(32));
        let c = Bits::from_codes(&[0, 5, 0x130, 0x223]);
        assert_eq!(Bits::parse(&c.to_sysfs(), 64), c);
        assert_eq!(c.codes(), vec![0, 5, 0x130, 0x223]);
    }

    #[test]
    #[cfg(unix)]
    fn scan_pairs_nodes_and_classifies() {
        let root = scratch("scan");
        let pad_keys = [
            0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c,
        ];
        let pad_abs = [0, 1, 2, 3, 4, 5, 0x10, 0x11];
        fake_device(
            &root,
            3,
            "Xbox Wireless Controller",
            (0x045e, 0x0b13),
            &pad_keys,
            &pad_abs,
            &[],
            &["js0", "event5"],
        );
        fake_device(
            &root,
            4,
            "3Dconnexion SpaceNavigator",
            (0x046d, 0xc626),
            &[0x100, 0x101],
            &[],
            &[0, 1, 2, 3, 4, 5],
            &["event6"],
        );
        fake_device(
            &root,
            5,
            "Some Keyboard",
            (0x1234, 0x5678),
            &[30, 31, 32],
            &[],
            &[],
            &["event7"],
        );
        fake_device(
            &root,
            6,
            "Logitech Extreme 3D",
            (0x046d, 0xc215),
            &[0x120, 0x121],
            &[0, 1, 5, 6],
            &[],
            &["js1", "event8"],
        );
        let dev = Path::new("/fake/dev/input");
        let f = scan(&root.join("class"), dev);
        assert_eq!(f.len(), 3, "{f:#?}");
        let pad = f.iter().find(|d| d.kind == Kind::Gamepad).unwrap();
        assert_eq!(pad.name, "Xbox Wireless Controller");
        assert_eq!(pad.js.as_deref(), Some(dev.join("js0").as_path()));
        assert_eq!(pad.event.as_deref(), Some(dev.join("event5").as_path()));
        let sm = f.iter().find(|d| d.kind == Kind::SpaceMouse).unwrap();
        assert_eq!(sm.js, None);
        assert_eq!(sm.event.as_deref(), Some(dev.join("event6").as_path()));
        assert!(f.iter().any(|d| d.kind == Kind::Joystick));
        let _ = fs::remove_dir_all(&root);
    }

    /// A fresh scratch folder under the crate's target dir (never /tmp).
    #[cfg_attr(not(unix), allow(dead_code))] // only the Unix tests use it
    pub(crate) fn scratch(name: &str) -> PathBuf {
        let base = std::env::var_os("CARGO_TARGET_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target"));
        let p = base
            .join("devices-test")
            .join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }
}
