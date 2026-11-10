//! The `.omero` file naming an image on a server: written to the cache when an image is
//! opened from the browser (one folder per dataset or plate, so the viewer's next and previous
//! walk it), read by the decoder. It holds the server address, user name, image id, where the
//! image sits, the plane and the rendering; never a password or session.

use serde_json::{Value, json};

use crate::render::{ChannelRender, Encoding, Render};

/// The most bytes read of a descriptor.
pub const MAX_DESCRIPTOR: usize = 1 << 20;

/// Where an image sits.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub project: Option<String>,
    pub dataset: Option<String>,
    pub screen: Option<String>,
    pub plate: Option<String>,
    /// The well and field ("B4, field 2").
    pub well: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Descriptor {
    /// The server's address (`https://idr.openmicroscopy.org`).
    pub server: String,
    /// The user signed in when the image was opened (`None`: public).
    pub user: Option<String>,
    pub image: u64,
    pub name: String,
    pub context: Context,
    pub owner: Option<String>,
    pub group: Option<String>,
    /// The plane shown first (the server's default without).
    pub z: Option<u32>,
    pub t: Option<u32>,
    /// The rendering chosen (the server's saved one without).
    pub render: Option<Render>,
    /// How tiles are fetched (JPEG for pyramids, PNG for whole images without).
    pub encoding: Option<Encoding>,
}

fn opt(v: &Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

impl Descriptor {
    pub fn to_json(&self) -> Vec<u8> {
        let render = self.render.as_ref().map(|r| {
            json!({
                "greyscale": r.greyscale,
                "channels": r.channels.iter().map(|c| json!({
                    "active": c.active,
                    "start": c.start,
                    "end": c.end,
                    "colour": format!("{:02X}{:02X}{:02X}", c.colour[0], c.colour[1], c.colour[2]),
                })).collect::<Vec<_>>(),
            })
        });
        let c = &self.context;
        let v = json!({
            "server": self.server,
            "user": self.user,
            "image": self.image,
            "name": self.name,
            "project": c.project,
            "dataset": c.dataset,
            "screen": c.screen,
            "plate": c.plate,
            "well": c.well,
            "owner": self.owner,
            "group": self.group,
            "z": self.z,
            "t": self.t,
            "render": render,
            "encoding": self.encoding.map(Encoding::id),
        });
        // The magic key first, whatever order the map keeps.
        let rest = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into());
        let rest = rest.strip_prefix('{').unwrap_or(&rest);
        format!("{{\"specular-omero\":1,{rest}").into_bytes()
    }

    /// Reads a descriptor; `None` when it is not one.
    pub fn parse(bytes: &[u8]) -> Option<Descriptor> {
        if bytes.len() > MAX_DESCRIPTOR {
            return None;
        }
        let v: Value = serde_json::from_slice(bytes).ok()?;
        v.get("specular-omero")?.as_u64()?;
        let server = crate::api::check_url(v.get("server")?.as_str()?)?;
        let render = v.get("render").filter(|r| r.is_object()).map(|r| Render {
            greyscale: r.get("greyscale").and_then(Value::as_bool).unwrap_or(false),
            channels: r
                .get("channels")
                .and_then(Value::as_array)
                .map(|a| {
                    a.iter()
                        .take(1024)
                        .map(|c| ChannelRender {
                            active: c.get("active").and_then(Value::as_bool).unwrap_or(true),
                            start: c.get("start").and_then(Value::as_f64).unwrap_or(0.0),
                            end: c.get("end").and_then(Value::as_f64).unwrap_or(255.0),
                            colour: crate::image::parse_colour(
                                c.get("colour").and_then(Value::as_str).unwrap_or(""),
                            ),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        });
        let n = |k: &str| {
            v.get(k)
                .and_then(Value::as_u64)
                .map(|n| n.min(u64::from(u32::MAX)) as u32)
        };
        Some(Descriptor {
            server,
            user: opt(&v, "user"),
            image: v.get("image")?.as_u64()?,
            name: opt(&v, "name").unwrap_or_default(),
            context: Context {
                project: opt(&v, "project"),
                dataset: opt(&v, "dataset"),
                screen: opt(&v, "screen"),
                plate: opt(&v, "plate"),
                well: opt(&v, "well"),
            },
            owner: opt(&v, "owner"),
            group: opt(&v, "group"),
            z: n("z"),
            t: n("t"),
            render,
            encoding: opt(&v, "encoding").and_then(|e| Encoding::from_id(&e)),
        })
    }

    /// `<name> (<id>).omero`, safe as a file name.
    pub fn file_name(&self) -> String {
        format!("{} ({}).omero", safe_name(&self.name), self.image)
    }
}

/// A name as one path component: no separators or control characters, at most 80 chars.
pub fn safe_name(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_control() || matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') {
                '_'
            } else {
                c
            }
        })
        .take(80)
        .collect();
    out = out.trim().trim_start_matches('.').trim().to_string();
    if out.is_empty() { "image".into() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_without_secrets() {
        let d = Descriptor {
            server: "https://idr.openmicroscopy.org".into(),
            user: Some("public".into()),
            image: 9_836_841,
            name: "a/b: c.tif".into(),
            context: Context {
                screen: Some("idr0001".into()),
                plate: Some("P1".into()),
                well: Some("B4".into()),
                ..Context::default()
            },
            z: Some(2),
            render: Some(Render {
                channels: vec![ChannelRender {
                    active: false,
                    start: 1.5,
                    end: 300.0,
                    colour: [0, 128, 255],
                }],
                greyscale: false,
            }),
            encoding: Some(Encoding::Png),
            ..Descriptor::default()
        };
        let bytes = d.to_json();
        assert!(bytes.starts_with(specular_decode::OMERO_MAGIC));
        assert_eq!(Descriptor::parse(&bytes), Some(d.clone()));
        assert_eq!(d.file_name(), "a_b_ c.tif (9836841).omero");
        let text = String::from_utf8(bytes).unwrap();
        assert!(!text.contains("password") && !text.contains("session"));
        assert_eq!(Descriptor::parse(b"{\"other\":1}"), None);
        assert_eq!(safe_name("..."), "image");
    }
}
