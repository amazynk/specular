//! A fake media renderer for tests, in-process and on loopback only: an SSDP responder (UDP,
//! answering `M-SEARCH` sent to it directly), its description, and AVTransport,
//! RenderingControl and ConnectionManager control endpoints. It records every action, keeps a
//! transport state and position like a renderer, and on Play fetches the URI it was given
//! (the whole file, then a range from the middle) so a test can check what the app served.

use std::io::{BufReader, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use parking_lot::Mutex;

use crate::http::{header, read_body, read_head};

/// How the fake behaves.
#[derive(Clone, Debug)]
pub struct Options {
    pub name: String,
    pub udn: String,
    /// Its sink ProtocolInfo list (comma-separated); `None`: no ConnectionManager.
    pub sink: Option<String>,
    /// Has RenderingControl (volume).
    pub rendering: bool,
    /// Actions it answers with UPnP fault 501.
    pub fail: Vec<String>,
    /// The play speeds its AVTransport description lists (empty: none listed).
    pub speeds: Vec<String>,
    /// Announces Samsung's `sec:` capabilities (subtitles as `sec:CaptionInfoEx`).
    pub sec: bool,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            name: "Test Renderer".into(),
            udn: "uuid:fake-renderer-1".into(),
            sink: Some(
                "http-get:*:video/mp4:*,http-get:*:audio/mpeg:*,http-get:*:audio/flac:*".into(),
            ),
            rendering: true,
            fail: Vec::new(),
            speeds: Vec::new(),
            sec: false,
        }
    }
}

/// What a fetch of the served URI got.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fetch {
    pub range: Option<String>,
    pub status: u16,
    pub content_type: Option<String>,
    pub content_range: Option<String>,
    pub body: Vec<u8>,
}

/// The fake's state, as a test reads it.
#[derive(Clone, Debug, Default)]
pub struct State {
    /// Every action called, with its arguments.
    pub actions: Vec<(String, Vec<(String, String)>)>,
    pub uri: Option<String>,
    pub metadata: Option<String>,
    /// `PLAYING`, `PAUSED_PLAYBACK`, `STOPPED`, `NO_MEDIA_PRESENT`.
    pub transport: String,
    /// Position at `since` (seconds), advancing while playing.
    pub position: f64,
    pub since: Option<Instant>,
    pub volume: u8,
    pub muted: bool,
    /// The speed of the last Play.
    pub speed: String,
    pub fetches: Vec<Fetch>,
    /// The address last fetched (a file set again is fetched again on its first play).
    pub fetched: Option<String>,
    /// M-SEARCH requests answered.
    pub searches: u32,
}

impl State {
    /// The position now.
    pub fn now(&self) -> f64 {
        match (self.transport.as_str(), self.since) {
            ("PLAYING", Some(t)) => self.position + t.elapsed().as_secs_f64(),
            _ => self.position,
        }
    }

    /// The names of the actions called, in order.
    pub fn action_names(&self) -> Vec<String> {
        self.actions.iter().map(|a| a.0.clone()).collect()
    }
}

/// A fake renderer running (stopped when dropped).
pub struct FakeRenderer {
    /// Where to send the search.
    pub ssdp: SocketAddr,
    /// Its HTTP endpoint.
    pub http: SocketAddr,
    pub state: Arc<Mutex<State>>,
    stop: Arc<AtomicBool>,
}

impl FakeRenderer {
    pub fn start(opts: Options) -> std::io::Result<FakeRenderer> {
        let udp = UdpSocket::bind("127.0.0.1:0")?;
        udp.set_read_timeout(Some(Duration::from_millis(50)))?;
        let tcp = TcpListener::bind("127.0.0.1:0")?;
        tcp.set_nonblocking(true)?;
        let (ssdp, http) = (udp.local_addr()?, tcp.local_addr()?);
        let state = Arc::new(Mutex::new(State {
            transport: "NO_MEDIA_PRESENT".into(),
            volume: 30,
            ..State::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let opts = Arc::new(opts);
        {
            let (st, stop, udn) = (state.clone(), stop.clone(), opts.udn.clone());
            specular_thread::spawn("test-cast-ssdp", move || {
                let mut buf = [0u8; 2048];
                while !stop.load(Ordering::Acquire) {
                    let Ok((n, from)) = udp.recv_from(&mut buf) else {
                        continue;
                    };
                    let req = String::from_utf8_lossy(&buf[..n]);
                    if !req.starts_with("M-SEARCH") || !req.contains("ssdp:discover") {
                        continue;
                    }
                    st.lock().searches += 1;
                    let reply = format!(
                        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=1800\r\nEXT:\r\n\
LOCATION: http://{http}/description.xml\r\nSERVER: Fake/1 UPnP/1.0\r\n\
ST: urn:schemas-upnp-org:device:MediaRenderer:1\r\nUSN: {}::urn:schemas-upnp-org:device:MediaRenderer:1\r\n\r\n",
                        udn
                    );
                    let _ = udp.send_to(reply.as_bytes(), from);
                }
            });
        }
        {
            let (st, stop, opts) = (state.clone(), stop.clone(), opts.clone());
            specular_thread::spawn("test-cast-http", move || {
                while !stop.load(Ordering::Acquire) {
                    match tcp.accept() {
                        Ok((c, _)) => {
                            let (st, opts) = (st.clone(), opts.clone());
                            specular_thread::spawn("test-cast-conn", move || {
                                let _ = handle(c, http, &st, &opts);
                            });
                        }
                        Err(_) => std::thread::sleep(Duration::from_millis(20)),
                    }
                }
            });
        }
        Ok(FakeRenderer {
            ssdp,
            http,
            state,
            stop,
        })
    }

    /// The description's URL.
    pub fn location(&self) -> String {
        format!("http://{}/description.xml", self.http)
    }

    pub fn state(&self) -> State {
        self.state.lock().clone()
    }

    /// Waits until `f` holds of the state (or `timeout` passes); whether it did.
    pub fn wait_for(&self, timeout: Duration, f: impl Fn(&State) -> bool) -> bool {
        let t0 = Instant::now();
        while t0.elapsed() < timeout {
            if f(&self.state.lock()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        f(&self.state.lock())
    }
}

impl Drop for FakeRenderer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
    }
}

fn description(o: &Options) -> String {
    let mut services = String::from(
        "<service><serviceType>urn:schemas-upnp-org:service:AVTransport:1</serviceType>\
<serviceId>urn:upnp-org:serviceId:AVTransport</serviceId><controlURL>/ctl/avt</controlURL>\
<eventSubURL>/evt/avt</eventSubURL><SCPDURL>/avt.xml</SCPDURL></service>",
    );
    if o.rendering {
        services.push_str(
            "<service><serviceType>urn:schemas-upnp-org:service:RenderingControl:1</serviceType>\
<controlURL>/ctl/rc</controlURL></service>",
        );
    }
    if o.sink.is_some() {
        services.push_str(
            "<service><serviceType>urn:schemas-upnp-org:service:ConnectionManager:1</serviceType>\
<controlURL>/ctl/cm</controlURL></service>",
        );
    }
    format!(
        "<?xml version=\"1.0\"?><root xmlns=\"urn:schemas-upnp-org:device-1-0\">\
<specVersion><major>1</major><minor>0</minor></specVersion><device>\
<deviceType>urn:schemas-upnp-org:device:MediaRenderer:1</deviceType>\
<friendlyName>{}</friendlyName><manufacturer>Test</manufacturer><modelName>Fake</modelName>\
<UDN>{}</UDN>{}<serviceList>{services}</serviceList></device></root>",
        crate::xml::escape(&o.name),
        crate::xml::escape(&o.udn),
        if o.sec {
            "<sec:ProductCap xmlns:sec=\"http://www.sec.co.kr/dlna\">Tuner,DCM10,getMediaInfo.sec,getCaptionInfo.sec</sec:ProductCap>"
        } else {
            ""
        }
    )
}

/// The AVTransport description: its play speeds.
fn scpd(o: &Options) -> String {
    let values: String = o
        .speeds
        .iter()
        .map(|v| format!("<allowedValue>{}</allowedValue>", crate::xml::escape(v)))
        .collect();
    let list = if values.is_empty() {
        String::new()
    } else {
        format!("<allowedValueList>{values}</allowedValueList>")
    };
    format!(
        "<?xml version=\"1.0\"?><scpd xmlns=\"urn:schemas-upnp-org:service-1-0\"><serviceStateTable>\
<stateVariable sendEvents=\"no\"><name>TransportPlaySpeed</name><dataType>string</dataType>{list}</stateVariable>\
</serviceStateTable></scpd>"
    )
}

fn reply(out: &mut TcpStream, status: &str, ctype: &str, body: &str) -> std::io::Result<()> {
    write!(
        out,
        "HTTP/1.1 {status}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
}

fn soap_ok(kind: &str, action: &str, args: &[(&str, String)]) -> String {
    let mut b = String::new();
    for (k, v) in args {
        b.push_str(&format!("<{k}>{}</{k}>", crate::xml::escape(v)));
    }
    format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body>\
<u:{action}Response xmlns:u=\"{kind}\">{b}</u:{action}Response></s:Body></s:Envelope>"
    )
}

fn soap_fault(code: u32, what: &str) -> String {
    format!(
        "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\"><s:Body><s:Fault>\
<faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail>\
<UPnPError xmlns=\"urn:schemas-upnp-org:control-1-0\"><errorCode>{code}</errorCode>\
<errorDescription>{what}</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>"
    )
}

fn handle(
    c: TcpStream,
    _me: SocketAddr,
    st: &Arc<Mutex<State>>,
    o: &Options,
) -> std::io::Result<()> {
    c.set_nonblocking(false)?;
    c.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut out = c.try_clone()?;
    let mut r = BufReader::new(c);
    let Some((line, headers)) = read_head(&mut r)? else {
        return Ok(());
    };
    // A request without a length or chunks has no body (it does not run to the close).
    let body = if header(&headers, "content-length").is_some()
        || header(&headers, "transfer-encoding").is_some()
    {
        read_body(&mut r, &headers, 1 << 20)?
    } else {
        Vec::new()
    };
    let mut parts = line.split_whitespace();
    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or(""));
    if method == "GET" && path == "/description.xml" {
        return reply(&mut out, "200 OK", "text/xml", &description(o));
    }
    if method == "GET" && path == "/avt.xml" {
        return reply(&mut out, "200 OK", "text/xml", &scpd(o));
    }
    if method != "POST" || !path.starts_with("/ctl/") {
        return reply(&mut out, "404 Not Found", "text/plain", "");
    }
    let soap_action = header(&headers, "soapaction")
        .unwrap_or("")
        .trim_matches('"');
    let Some((kind, action)) = soap_action.split_once('#') else {
        return reply(
            &mut out,
            "500 Internal Server Error",
            "text/xml",
            &soap_fault(401, "Invalid Action"),
        );
    };
    let text = String::from_utf8_lossy(&body);
    let args = crate::soap::parse_request(action, &text).unwrap_or_default();
    st.lock().actions.push((action.to_string(), args.clone()));
    if o.fail.iter().any(|f| f == action) {
        return reply(
            &mut out,
            "500 Internal Server Error",
            "text/xml",
            &soap_fault(501, "Action Failed"),
        );
    }
    let arg = |n: &str| crate::soap::out(&args, n).unwrap_or("").to_string();
    let mut outs: Vec<(&str, String)> = Vec::new();
    let mut play_uri = None;
    {
        let mut s = st.lock();
        let now = s.now();
        match action {
            "GetProtocolInfo" => {
                outs.push(("Source", String::new()));
                outs.push(("Sink", o.sink.clone().unwrap_or_default()));
            }
            "SetAVTransportURI" => {
                s.uri = Some(arg("CurrentURI"));
                s.metadata = Some(arg("CurrentURIMetaData"));
                s.transport = "STOPPED".into();
                s.position = 0.0;
                s.since = None;
            }
            "Play" => {
                if s.uri.is_none() {
                    drop(s);
                    return reply(
                        &mut out,
                        "500 Internal Server Error",
                        "text/xml",
                        &soap_fault(701, "Transition not available"),
                    );
                }
                if s.transport != "PLAYING" {
                    play_uri = s.uri.clone().filter(|u| s.fetched.as_ref() != Some(u));
                    let u = s.uri.clone();
                    s.fetched = u;
                }
                s.speed = arg("Speed");
                s.position = now;
                s.since = Some(Instant::now());
                s.transport = "PLAYING".into();
            }
            "Pause" => {
                s.position = now;
                s.since = None;
                s.transport = "PAUSED_PLAYBACK".into();
            }
            "Stop" => {
                s.position = 0.0;
                s.since = None;
                s.transport = "STOPPED".into();
            }
            "Seek" => {
                let t = crate::renderer::parse_time(&arg("Target")).unwrap_or(0.0);
                s.position = t;
                s.since = (s.transport == "PLAYING").then(Instant::now);
            }
            "GetTransportInfo" => {
                outs.push(("CurrentTransportState", s.transport.clone()));
                outs.push(("CurrentTransportStatus", "OK".into()));
                outs.push(("CurrentSpeed", "1".into()));
            }
            "GetPositionInfo" => {
                outs.push(("Track", "1".into()));
                outs.push(("TrackDuration", "0:03:36".into()));
                outs.push(("RelTime", crate::didl::duration_text(now)));
            }
            "GetVolume" => outs.push(("CurrentVolume", s.volume.to_string())),
            "SetVolume" => s.volume = arg("DesiredVolume").parse().unwrap_or(s.volume),
            "SetMute" => s.muted = arg("DesiredMute") == "1",
            _ => {
                drop(s);
                return reply(
                    &mut out,
                    "500 Internal Server Error",
                    "text/xml",
                    &soap_fault(401, "Invalid Action"),
                );
            }
        }
    }
    reply(
        &mut out,
        "200 OK",
        "text/xml; charset=\"utf-8\"",
        &soap_ok(kind, action, &outs),
    )?;
    drop(out);
    // As a renderer does: fetch what it was told to play (the whole file, then a range).
    if let Some(uri) = play_uri {
        for range in [None, Some("bytes=10-19".to_string())] {
            let f = fetch(&uri, range.as_deref());
            st.lock().fetches.push(f);
        }
        // A Samsung renderer fetches the subtitles named in the metadata.
        let caption = st.lock().metadata.as_deref().and_then(|m| {
            let doc = crate::xml::parse(m).ok()?;
            doc.descendants()
                .find(|n| n.tag_name().name() == "CaptionInfoEx")
                .and_then(|n| n.text())
                .map(str::to_string)
        });
        if o.sec
            && let Some(c) = caption
        {
            let f = fetch(&c, None);
            st.lock().fetches.push(f);
        }
    }
    Ok(())
}

/// GETs `uri` with an optional Range (raw, not through the client, which caps bodies).
fn fetch(uri: &str, range: Option<&str>) -> Fetch {
    let failed = |status| Fetch {
        range: range.map(str::to_string),
        status,
        content_type: None,
        content_range: None,
        body: Vec::new(),
    };
    let Some(u) = crate::http::Url::parse(uri) else {
        return failed(0);
    };
    let Ok(mut s) = TcpStream::connect_timeout(&u.addr(), Duration::from_secs(2)) else {
        return failed(0);
    };
    let _ = s.set_read_timeout(Some(Duration::from_secs(5)));
    let mut req = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        u.path,
        u.authority()
    );
    if let Some(r) = range {
        req.push_str(&format!("Range: {r}\r\n"));
    }
    req.push_str("\r\n");
    if s.write_all(req.as_bytes()).is_err() {
        return failed(0);
    }
    let mut r = BufReader::new(s);
    let Ok(Some((line, headers))) = read_head(&mut r) else {
        return failed(0);
    };
    let status = line
        .split_whitespace()
        .nth(1)
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let body = read_body(&mut r, &headers, 64 << 20).unwrap_or_default();
    Fetch {
        range: range.map(str::to_string),
        status,
        content_type: header(&headers, "content-type").map(str::to_string),
        content_range: header(&headers, "content-range").map(str::to_string),
        body,
    }
}
