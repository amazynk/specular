//! UPnP control: SOAP 1.1 actions (UPnP Device Architecture §3.2), their replies and faults.

use std::time::Duration;

use crate::device::Service;
use crate::xml::{child_text, escape};
use crate::{Error, Result};

/// The envelope of `action` of service `kind` with its arguments, in order.
pub fn envelope(kind: &str, action: &str, args: &[(&str, &str)]) -> String {
    let mut body = String::new();
    for (k, v) in args {
        body.push_str(&format!("<{k}>{}</{k}>", escape(v)));
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\
<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\">\
<s:Body><u:{action} xmlns:u=\"{kind}\">{body}</u:{action}></s:Body></s:Envelope>"
    )
}

/// Reads the reply to `action`: its output arguments, or the device's fault.
pub fn parse_reply(action: &str, status: u16, xml: &str) -> Result<Vec<(String, String)>> {
    let doc = crate::xml::parse(xml);
    if status != 200 {
        // A UPnP fault: <UPnPError><errorCode/><errorDescription/></UPnPError>.
        if let Ok(doc) = &doc
            && let Some(e) = doc
                .descendants()
                .find(|n| n.is_element() && n.tag_name().name() == "UPnPError")
        {
            let code = child_text(e, "errorCode")
                .and_then(|c| c.parse().ok())
                .unwrap_or(0);
            let what = child_text(e, "errorDescription").unwrap_or("");
            return Err(Error::Fault(code, crate::xml::display_text(what, 120)));
        }
        return Err(Error::Status(status));
    }
    args_of(&doc?, &format!("{action}Response"))
}

/// The arguments of a request for `action` (the fake renderer reads them).
pub fn parse_request(action: &str, xml: &str) -> Result<Vec<(String, String)>> {
    args_of(&crate::xml::parse(xml)?, action)
}

fn args_of(doc: &roxmltree::Document<'_>, element: &str) -> Result<Vec<(String, String)>> {
    let reply = doc
        .descendants()
        .find(|n| n.is_element() && n.tag_name().name() == element)
        .ok_or(Error::BadReply("no reply element"))?;
    Ok(reply
        .children()
        .filter(|c| c.is_element())
        .map(|c| {
            (
                c.tag_name().name().to_string(),
                c.text().unwrap_or("").to_string(),
            )
        })
        .collect())
}

/// Calls `action` on `svc`.
pub fn call(
    svc: &Service,
    action: &str,
    args: &[(&str, &str)],
    timeout: Duration,
) -> Result<Vec<(String, String)>> {
    let body = envelope(&svc.kind, action, args);
    let soap_action = format!("\"{}#{action}\"", svc.kind);
    let r = crate::http::request(
        "POST",
        &svc.control,
        &[
            ("Content-Type", "text/xml; charset=\"utf-8\""),
            ("SOAPACTION", &soap_action),
        ],
        body.as_bytes(),
        timeout,
    )?;
    parse_reply(action, r.status, &String::from_utf8_lossy(&r.body))
}

/// The value of output `name`.
pub fn out<'a>(args: &'a [(String, String)], name: &str) -> Option<&'a str> {
    args.iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_envelopes() {
        let e = envelope(
            "urn:schemas-upnp-org:service:AVTransport:1",
            "SetAVTransportURI",
            &[
                ("InstanceID", "0"),
                ("CurrentURI", "http://10.0.0.2/a?b=1&c=2"),
                ("CurrentURIMetaData", "<DIDL-Lite/>"),
            ],
        );
        assert!(e.contains(
            "<u:SetAVTransportURI xmlns:u=\"urn:schemas-upnp-org:service:AVTransport:1\">"
        ));
        assert!(e.contains("<CurrentURI>http://10.0.0.2/a?b=1&amp;c=2</CurrentURI>"));
        assert!(e.contains("<CurrentURIMetaData>&lt;DIDL-Lite/&gt;</CurrentURIMetaData>"));
        // It parses as XML.
        assert!(crate::xml::parse(&e).is_ok());
    }

    #[test]
    fn reads_replies_and_faults() {
        let ok = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body>
<u:GetVolumeResponse xmlns:u="urn:schemas-upnp-org:service:RenderingControl:1"><CurrentVolume>27</CurrentVolume></u:GetVolumeResponse>
</s:Body></s:Envelope>"#;
        let a = parse_reply("GetVolume", 200, ok).unwrap();
        assert_eq!(out(&a, "CurrentVolume"), Some("27"));
        assert!(parse_reply("Play", 200, ok).is_err());
        let fault = r#"<s:Envelope xmlns:s="http://schemas.xmlsoap.org/soap/envelope/"><s:Body><s:Fault>
<faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail>
<UPnPError xmlns="urn:schemas-upnp-org:control-1-0"><errorCode>714</errorCode><errorDescription>Illegal MIME-type</errorDescription></UPnPError>
</detail></s:Fault></s:Body></s:Envelope>"#;
        assert_eq!(
            parse_reply("SetAVTransportURI", 500, fault).err(),
            Some(Error::Fault(714, "Illegal MIME-type".into()))
        );
        assert_eq!(parse_reply("Play", 404, "").err(), Some(Error::Status(404)));
    }
}
