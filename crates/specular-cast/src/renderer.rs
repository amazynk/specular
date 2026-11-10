//! A renderer's actions: AVTransport:1 (set the URI, play, pause, seek, stop, where it is)
//! and RenderingControl:1 (volume, mute), and ConnectionManager:1's GetProtocolInfo.

use std::time::Duration;

use crate::device::Device;
use crate::soap::{call, out};
use crate::{Error, Result};

/// How long one action may take.
pub const TIMEOUT: Duration = Duration::from_secs(4);

/// The transport's state (AVTransport `TransportState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransportState {
    Playing,
    Paused,
    Stopped,
    /// Loading or buffering.
    Transitioning,
    NoMedia,
    Other,
}

impl TransportState {
    pub fn parse(s: &str) -> TransportState {
        match s.trim() {
            "PLAYING" => TransportState::Playing,
            "PAUSED_PLAYBACK" | "PAUSED_RECORDING" => TransportState::Paused,
            "STOPPED" => TransportState::Stopped,
            "TRANSITIONING" => TransportState::Transitioning,
            "NO_MEDIA_PRESENT" => TransportState::NoMedia,
            _ => TransportState::Other,
        }
    }
}

/// `H+:MM:SS[.F+]` (or `H+:MM:SS.F0/F1`) in seconds; `None` for `NOT_IMPLEMENTED` and the
/// like.
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim();
    let mut parts = s.split(':');
    let (h, m, sec) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let h: f64 = h.trim_start_matches('+').parse().ok()?;
    let m: f64 = m.parse().ok()?;
    let (whole, frac) = sec.split_once('.').unwrap_or((sec, ""));
    let mut secs: f64 = whole.parse().ok()?;
    if !frac.is_empty() {
        secs += match frac.split_once('/') {
            Some((a, b)) => {
                let (a, b): (f64, f64) = (a.parse().ok()?, b.parse().ok()?);
                if b > 0.0 { a / b } else { 0.0 }
            }
            None => format!("0.{frac}").parse::<f64>().ok()?,
        };
    }
    (h >= 0.0 && m < 60.0 && secs < 61.0).then_some(h * 3600.0 + m * 60.0 + secs)
}

/// `H:MM:SS`, as Seek's `REL_TIME` target takes it (whole seconds: some renderers refuse
/// fractions).
pub fn time_text(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
}

/// The allowed values of `TransportPlaySpeed` in an AVTransport service description.
pub fn play_speeds(scpd: &str) -> Vec<String> {
    let Ok(doc) = crate::xml::parse(scpd) else {
        return Vec::new();
    };
    doc.descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "stateVariable")
        .find(|v| crate::xml::child_text(*v, "name") == Some("TransportPlaySpeed"))
        .and_then(|v| {
            v.children()
                .find(|c| c.is_element() && c.tag_name().name() == "allowedValueList")
        })
        .map(|l| {
            l.children()
                .filter(|c| c.is_element() && c.tag_name().name() == "allowedValue")
                .filter_map(|c| c.text().map(str::trim))
                .filter(|t| !t.is_empty() && t.len() < 16)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// A speed as AVTransport spells it (`"2"`, `"1/2"`, `"-1"`) as a number.
pub fn speed_value(s: &str) -> Option<f64> {
    let s = s.trim();
    let v = match s.split_once('/') {
        Some((a, b)) => a.trim().parse::<f64>().ok()? / b.trim().parse::<f64>().ok()?,
        None => s.parse().ok()?,
    };
    v.is_finite().then_some(v)
}

/// Of the device's `speeds`, the one that is `want`.
pub fn speed_for(speeds: &[String], want: f64) -> Option<String> {
    speeds
        .iter()
        .find(|s| speed_value(s).is_some_and(|v| (v - want).abs() < 1e-3))
        .cloned()
}

/// Where the renderer is: its state, position and the media's length (when it says).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position {
    pub state: TransportState,
    pub position: Option<f64>,
    pub duration: Option<f64>,
}

/// A renderer to drive.
#[derive(Clone, Debug)]
pub struct Renderer {
    pub device: Device,
    pub timeout: Duration,
}

impl Renderer {
    pub fn new(device: Device) -> Renderer {
        Renderer {
            device,
            timeout: TIMEOUT,
        }
    }

    fn avt(&self, action: &str, args: &[(&str, &str)]) -> Result<Vec<(String, String)>> {
        let mut all = vec![("InstanceID", "0")];
        all.extend_from_slice(args);
        call(&self.device.av_transport, action, &all, self.timeout)
    }

    fn rc(&self, action: &str, args: &[(&str, &str)]) -> Result<Vec<(String, String)>> {
        let svc = self
            .device
            .rendering
            .as_ref()
            .ok_or(Error::Fault(401, "no RenderingControl".into()))?;
        let mut all = vec![("InstanceID", "0"), ("Channel", "Master")];
        all.extend_from_slice(args);
        call(svc, action, &all, self.timeout)
    }

    /// The types it plays (its sink ProtocolInfo list); empty when it does not say.
    pub fn sink(&self) -> Result<Vec<String>> {
        let Some(cm) = &self.device.connection else {
            return Ok(Vec::new());
        };
        let r = call(cm, "GetProtocolInfo", &[], self.timeout)?;
        Ok(crate::accept::split_sink(out(&r, "Sink").unwrap_or("")))
    }

    pub fn set_uri(&self, url: &str, metadata: &str) -> Result<()> {
        self.avt(
            "SetAVTransportURI",
            &[("CurrentURI", url), ("CurrentURIMetaData", metadata)],
        )
        .map(drop)
    }

    pub fn play(&self) -> Result<()> {
        self.avt("Play", &[("Speed", "1")]).map(drop)
    }

    /// Plays at `speed` as the device spells it (`"2"`, `"1/2"`).
    pub fn play_at(&self, speed: &str) -> Result<()> {
        self.avt("Play", &[("Speed", speed)]).map(drop)
    }

    /// The speeds it plays at: AVTransport's `TransportPlaySpeed` allowed values from its
    /// service description (only `"1"` when it does not list more).
    pub fn speeds(&self) -> Vec<String> {
        let Some(url) = &self.device.av_transport.scpd else {
            return vec!["1".into()];
        };
        let Ok(r) = crate::http::request("GET", url, &[], &[], self.timeout) else {
            return vec!["1".into()];
        };
        if r.status != 200 {
            return vec!["1".into()];
        }
        let list = play_speeds(&String::from_utf8_lossy(&r.body));
        if list.is_empty() {
            vec!["1".into()]
        } else {
            list
        }
    }

    pub fn pause(&self) -> Result<()> {
        self.avt("Pause", &[]).map(drop)
    }

    pub fn stop(&self) -> Result<()> {
        self.avt("Stop", &[]).map(drop)
    }

    pub fn seek(&self, secs: f64) -> Result<()> {
        self.avt(
            "Seek",
            &[("Unit", "REL_TIME"), ("Target", &time_text(secs))],
        )
        .map(drop)
    }

    pub fn transport_state(&self) -> Result<TransportState> {
        let r = self.avt("GetTransportInfo", &[])?;
        Ok(TransportState::parse(
            out(&r, "CurrentTransportState").unwrap_or(""),
        ))
    }

    /// Its state and position (two calls).
    pub fn position(&self) -> Result<Position> {
        let state = self.transport_state()?;
        let r = self.avt("GetPositionInfo", &[])?;
        Ok(Position {
            state,
            position: out(&r, "RelTime").and_then(parse_time),
            duration: out(&r, "TrackDuration")
                .and_then(parse_time)
                .filter(|d| *d > 0.0),
        })
    }

    /// Its volume (0–100), when it has RenderingControl.
    pub fn volume(&self) -> Result<u8> {
        let r = self.rc("GetVolume", &[])?;
        out(&r, "CurrentVolume")
            .and_then(|v| v.trim().parse::<u32>().ok())
            .map(|v| v.min(100) as u8)
            .ok_or(Error::BadReply("volume"))
    }

    pub fn set_volume(&self, v: u8) -> Result<()> {
        let v = v.min(100).to_string();
        self.rc("SetVolume", &[("DesiredVolume", &v)]).map(drop)
    }

    pub fn set_mute(&self, mute: bool) -> Result<()> {
        self.rc("SetMute", &[("DesiredMute", if mute { "1" } else { "0" })])
            .map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn times() {
        assert_eq!(parse_time("0:00:07"), Some(7.0));
        assert_eq!(parse_time("1:02:03.5"), Some(3723.5));
        assert_eq!(parse_time("00:01:00.250"), Some(60.25));
        assert_eq!(parse_time("0:00:01.1/4"), Some(1.25));
        assert_eq!(parse_time("NOT_IMPLEMENTED"), None);
        assert_eq!(parse_time(""), None);
        assert_eq!(parse_time("0:61:00"), None);
        assert_eq!(time_text(3723.4), "1:02:03");
        assert_eq!(time_text(-3.0), "0:00:00");
    }

    #[test]
    fn speeds() {
        let scpd = r#"<?xml version="1.0"?><scpd xmlns="urn:schemas-upnp-org:service-1-0"><serviceStateTable>
<stateVariable sendEvents="no"><name>TransportState</name><allowedValueList><allowedValue>PLAYING</allowedValue></allowedValueList></stateVariable>
<stateVariable sendEvents="no"><name>TransportPlaySpeed</name><dataType>string</dataType>
<allowedValueList><allowedValue>1</allowedValue><allowedValue>1/2</allowedValue><allowedValue> 2 </allowedValue></allowedValueList></stateVariable>
</serviceStateTable></scpd>"#;
        let l = play_speeds(scpd);
        assert_eq!(l, ["1", "1/2", "2"]);
        assert_eq!(speed_for(&l, 0.5).as_deref(), Some("1/2"));
        assert_eq!(speed_for(&l, 2.0).as_deref(), Some("2"));
        assert_eq!(speed_for(&l, 1.5), None);
        assert_eq!(speed_value("-1"), Some(-1.0));
        assert_eq!(speed_value("x"), None);
        assert!(play_speeds("<scpd/>").is_empty());
    }

    #[test]
    fn states() {
        assert_eq!(TransportState::parse("PLAYING"), TransportState::Playing);
        assert_eq!(
            TransportState::parse("PAUSED_PLAYBACK"),
            TransportState::Paused
        );
        assert_eq!(TransportState::parse("??"), TransportState::Other);
    }
}
