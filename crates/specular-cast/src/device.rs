//! A renderer's device description (UPnP Device Architecture §2): its name and the control
//! URLs of AVTransport, RenderingControl and ConnectionManager.

use std::time::Duration;

use crate::xml::{child_text, display_text};
use crate::{Error, Result};

/// The longest device name shown.
const MAX_NAME: usize = 64;

/// One of the device's services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Service {
    /// Its full type with version (`urn:schemas-upnp-org:service:AVTransport:1`).
    pub kind: String,
    /// Where its SOAP actions go (absolute).
    pub control: String,
    /// Its description (SCPD): the actions and the values its state takes.
    pub scpd: Option<String>,
}

/// A media renderer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Device {
    /// Its own name ("Living Room TV"), cleaned for display.
    pub name: String,
    /// Its unique device name (`uuid:…`).
    pub udn: String,
    /// The description's URL.
    pub location: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub av_transport: Service,
    pub rendering: Option<Service>,
    pub connection: Option<Service>,
    /// Samsung's DLNA extensions (`sec:` capabilities): subtitles go as `sec:CaptionInfoEx`.
    pub sec_captions: bool,
}

const AVT: &str = "urn:schemas-upnp-org:service:AVTransport:";
const RC: &str = "urn:schemas-upnp-org:service:RenderingControl:";
const CM: &str = "urn:schemas-upnp-org:service:ConnectionManager:";

/// Reads a description fetched from `location`. The first device (at any depth) with an
/// AVTransport service is the renderer; its control URLs must be on the description's host.
pub fn parse(location: &str, xml: &str) -> Result<Device> {
    let doc = crate::xml::parse(xml)?;
    let root = doc.root_element();
    let base = child_text(root, "URLBase")
        .filter(|b| !b.is_empty())
        .unwrap_or(location);
    let host = crate::http::Url::parse(location)
        .ok_or(Error::BadReply("location"))?
        .ip;
    for dev in root
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "device")
    {
        let Some(list) = dev
            .children()
            .find(|c| c.is_element() && c.tag_name().name() == "serviceList")
        else {
            continue;
        };
        let mut avt = None;
        let mut rc = None;
        let mut cm = None;
        for s in list
            .children()
            .filter(|c| c.is_element() && c.tag_name().name() == "service")
        {
            let (Some(kind), Some(ctl)) =
                (child_text(s, "serviceType"), child_text(s, "controlURL"))
            else {
                continue;
            };
            let control = crate::http::resolve(base, ctl);
            // A control URL elsewhere than the device is ignored.
            if crate::http::Url::parse(&control).is_none_or(|u| u.ip != host) {
                continue;
            }
            let scpd = child_text(s, "SCPDURL")
                .map(|u| crate::http::resolve(base, u))
                .filter(|u| crate::http::Url::parse(u).is_some_and(|u| u.ip == host));
            let svc = Service {
                kind: kind.to_string(),
                control,
                scpd,
            };
            if kind.starts_with(AVT) && avt.is_none() {
                avt = Some(svc);
            } else if kind.starts_with(RC) && rc.is_none() {
                rc = Some(svc);
            } else if kind.starts_with(CM) && cm.is_none() {
                cm = Some(svc);
            }
        }
        let Some(av_transport) = avt else { continue };
        let udn = child_text(dev, "UDN").unwrap_or(location).to_string();
        let text = |n: &str| {
            child_text(dev, n)
                .map(|t| display_text(t, MAX_NAME))
                .filter(|t| !t.is_empty())
        };
        let model = text("modelName");
        let name = text("friendlyName")
            .or_else(|| model.clone())
            .unwrap_or_else(|| host.to_string());
        return Ok(Device {
            name,
            udn,
            location: location.to_string(),
            manufacturer: text("manufacturer"),
            model,
            av_transport,
            rendering: rc,
            connection: cm,
            sec_captions: xml.contains("sec:ProductCap") || xml.contains("www.sec.co.kr/dlna"),
        });
    }
    Err(Error::NotARenderer)
}

/// Fetches and reads the description at `location`.
pub fn fetch(location: &str, timeout: Duration) -> Result<Device> {
    let r = crate::http::request("GET", location, &[], &[], timeout)?;
    if r.status != 200 {
        return Err(Error::Status(r.status));
    }
    let text = String::from_utf8_lossy(&r.body);
    parse(location, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESC: &str = r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
  <specVersion><major>1</major><minor>0</minor></specVersion>
  <device>
    <deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
    <friendlyName>  Living Room
      TV </friendlyName>
    <manufacturer>Acme</manufacturer>
    <modelName>Screen 9000</modelName>
    <UDN>uuid:1234</UDN>
    <serviceList>
      <service>
        <serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>
        <controlURL>/upnp/control/rc</controlURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>
        <controlURL>cm/control</controlURL>
      </service>
      <service>
        <serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>
        <controlURL>/upnp/control/avt</controlURL>
      </service>
    </serviceList>
  </device>
</root>"#;

    #[test]
    fn reads_a_renderer() {
        let d = parse("http://192.168.1.9:49152/dmr/desc.xml", DESC).unwrap();
        assert_eq!(d.name, "Living Room TV");
        assert_eq!(d.udn, "uuid:1234");
        assert_eq!(d.model.as_deref(), Some("Screen 9000"));
        assert_eq!(
            d.av_transport.control,
            "http://192.168.1.9:49152/upnp/control/avt"
        );
        assert_eq!(
            d.av_transport.kind,
            "urn:schemas-upnp-org:service:AVTransport:1"
        );
        assert_eq!(
            d.connection.unwrap().control,
            "http://192.168.1.9:49152/dmr/cm/control"
        );
        assert!(d.rendering.is_some());
    }

    #[test]
    fn nested_devices_and_url_base() {
        let xml = r#"<root xmlns="urn:schemas-upnp-org:device-1-0"><URLBase>http://10.0.0.7:1400/</URLBase>
<device><deviceType>urn:schemas-upnp-org:device:ZonePlayer:1</deviceType><friendlyName>Kitchen</friendlyName>
<deviceList><device><deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>
<friendlyName>Kitchen - Speaker</friendlyName><UDN>uuid:k_MR</UDN><serviceList><service>
<serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType><controlURL>MediaRenderer/AVTransport/Control</controlURL>
</service></serviceList></device></deviceList></device></root>"#;
        let d = parse("http://10.0.0.7:1400/xml/device_description.xml", xml).unwrap();
        assert_eq!(d.name, "Kitchen - Speaker");
        assert_eq!(
            d.av_transport.control,
            "http://10.0.0.7:1400/MediaRenderer/AVTransport/Control"
        );
        assert!(d.rendering.is_none());
    }

    #[test]
    fn refuses_what_is_not_a_renderer() {
        let server = DESC.replace("AVTransport", "ContentDirectory");
        assert_eq!(
            parse("http://192.168.1.9/d.xml", &server).err(),
            Some(Error::NotARenderer)
        );
        // Control on another host: ignored, so no AVTransport.
        let elsewhere = DESC.replace("/upnp/control/avt", "http://192.168.1.50/avt");
        assert_eq!(
            parse("http://192.168.1.9/d.xml", &elsewhere).err(),
            Some(Error::NotARenderer)
        );
        assert!(parse("http://192.168.1.9/d.xml", "not xml").is_err());
    }
}
