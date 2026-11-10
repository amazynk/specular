//! The wire protocol between the viewer and `specular-decode-host`.
//!
//! **Framing.** Every message is one frame: an 8-byte header, then `len` bytes of body.
//!
//! ```text
//! u32 len (little endian) · u8 fds · 3 bytes zero · body[len]
//! ```
//!
//! `fds` is the number of file descriptors sent with the frame (Unix `SCM_RIGHTS`, attached to
//! the header bytes). A body starts with a one-byte message tag; all integers are little endian,
//! byte strings and UTF-8 strings are a `u32` length followed by the bytes. Every read is bounds
//! checked: a malformed frame is a [`ProtocolError`], never a panic, on either side.
//!
//! **Versioning.** The first exchange is [`Request::Hello`] / [`Response::Hello`]. Both carry
//! [`VERSION`]; the host refuses any other version and exits, so the viewer and its host binary
//! must come from the same build. Changing any message layout bumps [`VERSION`].
//!
//! **Bulk data.** Inputs and outputs travel as [`Input`] / [`Payload`]: either inline in the frame
//! or, on Linux, as a sealed `memfd` passed with the frame, so large pixel buffers are not pushed
//! through the socket.
//!
//! **Archives** are never sent as bytes: the viewer opens the file read-only and passes the open
//! file, as a descriptor with the frame (Unix) or as a handle it duplicated into the host
//! ([`Input::Handle`], Windows). The host cannot open files itself. When UnRAR wants another
//! volume, the host answers the request with [`Response::NeedVolume`]; the viewer replies with
//! [`Request::Volume`] (the file, or none) and keeps waiting for the real answer. Volume names
//! are synthetic ASCII names ([`volume_name`]), never paths.
//!
//! **CAD models** (STEP, IGES) are read by OpenCASCADE in a host started with `--occt`
//! ([`Request::CadRead`]); the answer is the tessellated model in the format [`crate::cad`]
//! reads, as a [`Payload`].
//!
//! **Zeiss CZI** files are read by libCZI in a host started with `--czi`. Like archives they
//! travel as the open file, with every request ([`Request::CziInfo`], [`Request::CziRead`],
//! [`Request::CziAttachment`]); the viewer's `key` names the document, so a host keeps a few
//! documents open between requests (it uses the file of the first request and closes the
//! others). Answers are the document's facts ([`Response::CziInfo`], the format
//! [`crate::czi`] reads) or one plane's pixels ([`Response::CziPlane`]).
//!
//! **SQLite** databases (GeoPackage, MBTiles) are read by SQLite in the host
//! ([`Request::SqlQuery`]): the open file travels with every request like a CZI file, with the
//! viewer's own SQL and parameters; the host keeps a few connections open by `key` and answers
//! with the rows ([`Response::Rows`], the format [`crate::sql`] reads). When it opens a
//! connection it asks for the database's WAL as volume "wal" ([`Response::NeedVolume`]); the
//! viewer answers with the `-wal` file beside the database, or none.
//!
//! **Passwords** of encrypted RAR archives travel only in the RAR requests, as a [`Secret`]
//! written last in the frame (so the encoder never reallocates a buffer that held it), and both
//! sides wipe the frames that carried one.

pub use crate::sql::{SqlLimits, SqlValue};

/// Protocol version. Bump on any layout change.
pub const VERSION: u16 = 8;

/// Overwrites `buf` with zeros in a way the compiler keeps (a received frame that carried a
/// [`Secret`]).
pub fn wipe(buf: &mut [u8]) {
    zeroize::Zeroize::zeroize(buf);
}

/// A password on its way to the host: wiped from memory when dropped, never shown by `Debug`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(zeroize::Zeroizing<String>);

impl Secret {
    /// Takes `s` over (moved, not copied).
    pub fn new(s: String) -> Secret {
        Secret(zeroize::Zeroizing::new(s))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

/// Sent in the hello so a wrong process on the other end fails fast.
pub const MAGIC: [u8; 8] = *b"SPCLHOST";

/// Size of the frame header.
pub const HEADER_LEN: usize = 8;

/// Largest frame body accepted. Inline payloads larger than this must use a shared-memory
/// descriptor.
pub const MAX_FRAME: usize = 1 << 30;

/// Most descriptors one frame may carry.
pub const MAX_FDS: usize = 2;

/// A malformed frame or message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("protocol: {0}")]
pub struct ProtocolError(pub String);

fn perr<T>(msg: impl Into<String>) -> Result<T, ProtocolError> {
    Err(ProtocolError(msg.into()))
}

/// Encodes a frame header.
pub fn header(len: usize, fds: usize) -> Result<[u8; HEADER_LEN], ProtocolError> {
    if len > MAX_FRAME {
        return perr(format!("frame of {len} bytes exceeds {MAX_FRAME}"));
    }
    if fds > MAX_FDS {
        return perr(format!("{fds} descriptors exceed {MAX_FDS}"));
    }
    let mut h = [0u8; HEADER_LEN];
    h[..4].copy_from_slice(&(len as u32).to_le_bytes());
    h[4] = fds as u8;
    Ok(h)
}

/// Decodes a frame header into (body length, descriptor count).
pub fn parse_header(h: &[u8; HEADER_LEN]) -> Result<(usize, usize), ProtocolError> {
    let len = u32::from_le_bytes([h[0], h[1], h[2], h[3]]) as usize;
    let fds = h[4] as usize;
    if h[5..] != [0, 0, 0] {
        return perr("reserved header bytes are not zero");
    }
    if len > MAX_FRAME {
        return perr(format!("frame of {len} bytes exceeds {MAX_FRAME}"));
    }
    if fds > MAX_FDS {
        return perr(format!("{fds} descriptors exceed {MAX_FDS}"));
    }
    Ok((len, fds))
}

/// Bulk input to the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// The first descriptor sent with the frame (a sealed memfd on Linux).
    Fd,
    Inline(Vec<u8>),
    /// A handle the viewer duplicated into the host (Windows): its value in the host's handle
    /// table. The host owns it from the moment the message arrives.
    Handle(u64),
}

/// Longest synthetic volume name.
pub const MAX_VOLUME_NAME: usize = 255;

/// Whether `s` is a valid synthetic volume name: 1 to [`MAX_VOLUME_NAME`] bytes of
/// `[A-Za-z0-9._-]`, not `.` or `..`. It is a bare name, never a path.
pub fn valid_volume_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_VOLUME_NAME
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
}

/// The synthetic volume name for a real file name: every character outside `[A-Za-z0-9.-]`
/// becomes `_` (one character for one, so positions line up with the real name), cut to
/// [`MAX_VOLUME_NAME`] characters from the front so the volume suffix survives. UnRAR derives
/// the next volume's name from it (`x.part1.rar` → `x.part2.rar`, `x.rar` → `x.r00`).
pub fn volume_name(real: &str) -> String {
    let chars: Vec<char> = real.chars().collect();
    let skip = chars.len().saturating_sub(MAX_VOLUME_NAME);
    let s: String = chars[skip..]
        .iter()
        .map(|&c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    if valid_volume_name(&s) {
        s
    } else {
        "archive.rar".into()
    }
}

/// Bulk output from the host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Payload {
    /// The first descriptor sent with the frame holds `len` bytes from offset 0.
    Fd {
        len: u64,
    },
    Inline(Vec<u8>),
}

/// Limits the host checks before allocating an image.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageLimits {
    /// Most pixels (width × height) decoded.
    pub max_pixels: u64,
    /// Longest side.
    pub max_side: u32,
}

impl Default for ImageLimits {
    fn default() -> Self {
        ImageLimits {
            max_pixels: 400_000_000,
            max_side: 65_535,
        }
    }
}

/// A CAD exchange format OpenCASCADE reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CadFormat {
    Step = 1,
    Iges = 2,
}

/// How finely, and how much, a CAD model is tessellated.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CadLimits {
    /// Linear deflection as a fraction of the model's bounding-box diagonal.
    pub relative_deflection: f64,
    /// Angular deflection, radians.
    pub angular_deflection: f64,
    /// Most triangles (all instances together); OpenCASCADE retries coarser above it.
    pub max_triangles: u64,
    /// Most bytes of model sent back.
    pub max_bytes: u64,
}

impl Default for CadLimits {
    fn default() -> Self {
        CadLimits {
            relative_deflection: 0.000_5,
            angular_deflection: 0.35,
            max_triangles: 30_000_000,
            max_bytes: 2 << 30,
        }
    }
}

// `Request` holds `CadLimits` (floats, never NaN from our side): equality is exact.
impl Eq for CadLimits {}

/// CZI dimension letters a plane coordinate may name (`S`, the scene, is a filter instead).
pub const CZI_PLANE_DIMS: &[u8] = b"ZCTRIHVB";

/// One region of one channel of a CZI plane, as libCZI's scaling tile accessor reads it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CziRegion {
    /// In the document's raw sub-block coordinates.
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    /// Output pixels per document pixel, in (0, 1].
    pub zoom: f32,
    /// Only this scene's sub-blocks.
    pub scene: Option<u32>,
    /// The plane: (dimension letter from [`CZI_PLANE_DIMS`], value); at most one each.
    pub coords: [(u8, i32); 8],
    pub coord_count: u8,
    /// libCZI's pixel type of the channel (0 Gray8, 1 Gray16, 2 Gray32Float, 3 Bgr24,
    /// 4 Bgr48, 8 Bgr96Float).
    pub pixel: u8,
    /// Most bytes of pixels sent back.
    pub max_bytes: u64,
}

// Floats are finite (checked when decoded): equality is exact.
impl Eq for CziRegion {}

impl CziRegion {
    /// The plane coordinates given.
    pub fn coords(&self) -> &[(u8, i32)] {
        &self.coords[..usize::from(self.coord_count).min(self.coords.len())]
    }
}

/// libCZI pixel types the host reads, with their bytes per pixel.
pub fn czi_pixel_bytes(pixel: u8) -> Option<usize> {
    match pixel {
        0 => Some(1),
        1 => Some(2),
        2 => Some(4),
        3 => Some(3),
        4 => Some(6),
        8 => Some(12),
        _ => None,
    }
}

/// A channel's display settings as an embedded CZI image gives them (black and white point
/// as fractions of the sample range, the gamma of a gamma gradation curve).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CziDisplay {
    pub black: f32,
    pub white: f32,
    pub gamma: Option<f32>,
}

impl Eq for CziDisplay {}

/// One plane from libCZI: a channel's pixels (`kind` [`CziPlaneKind::Pixels`]), or an
/// attachment's encoded bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CziPlane {
    pub kind: CziPlaneKind,
    pub width: u32,
    pub height: u32,
    /// libCZI's pixel type ([`czi_pixel_bytes`]); 255 for encoded bytes.
    pub pixel: u8,
    /// The attachment's content type ("JPG", "CZI", ...), at most 8 ASCII characters.
    pub content_type: String,
    pub display: Option<CziDisplay>,
    /// Rows top to bottom, `width × bytes per pixel` each, or the encoded bytes.
    pub payload: Payload,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CziPlaneKind {
    Encoded = 0,
    Pixels = 1,
}

/// Self-test operations, honoured only by a host started with `--test-hooks`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TestOp {
    /// Echo the bytes back (round-trip and restart checks).
    Echo(Vec<u8>),
    /// Abort the host.
    Crash,
    /// Overflow the host's stack (unbounded recursion: a parser without a depth limit).
    Overflow,
    /// Panic in the host (a decoder bug on a file).
    Panic,
    /// Sleep this long before answering.
    Hang { millis: u32 },
    /// Allocate and touch this many bytes (memory limit checks).
    Alloc { bytes: u64 },
    /// Try operations the sandbox must deny: read `read_path`, create a file in `write_dir`,
    /// open a socket, run a program, list `/`.
    Probe {
        read_path: String,
        write_dir: String,
    },
}

/// Viewer → host.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    Hello {
        version: u16,
    },
    /// Decode the primary image of a HEIF/HEIC file to RGBA8 (transformations applied).
    HeifDecode {
        input: Input,
        limits: ImageLimits,
    },
    /// List a RAR archive: `input` is its first (or only) volume, which UnRAR knows as
    /// `volume` (a [`valid_volume_name`]).
    RarList {
        input: Input,
        volume: String,
        max_entries: u32,
        /// For encrypted file names and headers (RAR `-hp`) and encrypted entries.
        password: Option<Secret>,
    },
    /// Read entry `index` (position in the [`Listing`], directories included), which must be
    /// named `name`, refusing more than `max_size` bytes.
    RarRead {
        input: Input,
        volume: String,
        index: u32,
        name: String,
        max_size: u64,
        password: Option<Secret>,
    },
    /// Read several entries in one pass over the archive (a solid archive is decompressed
    /// once, not once per entry): `items` are (`index`, `name`) as in [`Request::RarRead`], in
    /// ascending index order. Each is refused past `max_size` bytes, all of them together past
    /// `max_total`. Answered with [`Response::DataMany`].
    RarReadMany {
        input: Input,
        volume: String,
        items: Vec<(u32, String)>,
        max_size: u64,
        max_total: u64,
        password: Option<Secret>,
    },
    /// Read a STEP or IGES file with OpenCASCADE and tessellate it (a host started with
    /// `--occt`). Answered with [`Response::Model`].
    CadRead {
        input: Input,
        format: CadFormat,
        limits: CadLimits,
    },
    /// The facts of a CZI document (a host started with `--czi`), with its metadata XML when
    /// that is at most `max_xml` bytes. Answered with [`Response::CziInfo`].
    CziInfo {
        input: Input,
        key: u64,
        max_xml: u64,
    },
    /// One channel of a region of a CZI plane. Answered with [`Response::CziPlane`].
    CziRead {
        input: Input,
        key: u64,
        region: CziRegion,
    },
    /// Attachment `index` of a CZI document: its bytes, or an embedded CZI image's first
    /// channel composed to fit `max_side`. Answered with [`Response::CziPlane`].
    CziAttachment {
        input: Input,
        key: u64,
        index: u32,
        max_side: u32,
        max_bytes: u64,
    },
    Test(TestOp),
    /// Decode frame `frame` of a Blackmagic RAW clip with the Blackmagic RAW SDK the user
    /// installed (the host loaded it at start, [`HostInfo::braw`]). `input` is the clip as an
    /// open file, like a RAR archive: the SDK reads it through the host's file object, never
    /// by path. `scale` is 1, 2, 4 or 8 (the SDK's full, half, quarter or eighth resolution).
    BrawDecode {
        input: Input,
        frame: u64,
        scale: u8,
        limits: ImageLimits,
    },
    /// Run the viewer's `sql` with `params` on the SQLite database `input` (read-only, inside
    /// `limits`). `key` names the database, as for CZI. Answered with [`Response::Rows`].
    SqlQuery {
        input: Input,
        key: u64,
        sql: String,
        params: Vec<SqlValue>,
        limits: SqlLimits,
    },
    /// The answer to [`Response::NeedVolume`]: the volume, or `None` when there is none.
    Volume {
        input: Option<Input>,
    },
}

/// How far the host could confine itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SandboxLevel {
    /// No OS confinement (unsupported platform or everything failed).
    None = 0,
    /// Some layers active (e.g. seccomp without Landlock on an old kernel).
    Partial = 1,
    /// Every layer for this platform active.
    Full = 2,
}

/// The host's answer to the hello.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostInfo {
    pub version: u16,
    pub sandbox: SandboxLevel,
    /// What was applied, for logs and the about box ("seccomp, landlock v6, …").
    pub sandbox_detail: String,
    /// libheif version when it could be loaded.
    pub heif: Option<String>,
    /// Why libheif is missing, when it is.
    pub heif_error: String,
    /// The host was built with RAR support.
    pub rar: bool,
    /// OpenCASCADE's version, when this host loaded it (started with `--occt`).
    pub occt: Option<String>,
    /// Why OpenCASCADE is missing, when it is.
    pub occt_error: String,
    pub test_hooks: bool,
    /// The Blackmagic RAW SDK's version when the host was started with one and could load it.
    pub braw: Option<String>,
    /// Why the Blackmagic RAW SDK named at start could not be used (empty when none was
    /// named or it loaded).
    pub braw_error: String,
    /// libCZI's version, when this host loaded it (started with `--czi`).
    pub czi: Option<String>,
    /// Why libCZI is missing, when it is.
    pub czi_error: String,
}

/// What the Blackmagic RAW SDK says about a clip.
#[derive(Debug, Clone, PartialEq)]
pub struct ClipInfo {
    /// Full-resolution size.
    pub width: u32,
    pub height: u32,
    pub frame_count: u64,
    /// Frames per second (0 when unknown).
    pub frame_rate: f32,
    /// The camera that recorded it ("Blackmagic URSA Mini Pro 12K").
    pub camera: String,
}

/// A decoded image: `width × height` RGBA8, rows `width * 4` bytes, straight alpha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageReply {
    pub width: u32,
    pub height: u32,
    pub has_alpha: bool,
    /// Bit depth of the coded image (the output is always 8-bit).
    pub bit_depth: u8,
    pub icc: Vec<u8>,
    pub pixels: Payload,
}

/// One RAR header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListEntry {
    pub name: String,
    pub size: u64,
    pub encrypted: bool,
    pub directory: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    pub solid: bool,
    pub entries: Vec<ListEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeResult {
    pub name: String,
    /// The operation succeeded (a sandbox escape when expected denied).
    pub allowed: bool,
    pub detail: String,
}

/// Error classes the host reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    Unsupported = 1,
    /// The library for this format is not installed (libheif missing, RAR not built).
    NotInstalled = 2,
    Malformed = 3,
    TooLarge = 4,
    Io = 5,
    Protocol = 6,
    Internal = 7,
    /// Encrypted, and no password was given.
    Encrypted = 8,
    /// Encrypted, and the password given is wrong.
    BadPassword = 9,
}

impl ErrorKind {
    fn from_u8(v: u8) -> Result<ErrorKind, ProtocolError> {
        Ok(match v {
            1 => ErrorKind::Unsupported,
            2 => ErrorKind::NotInstalled,
            3 => ErrorKind::Malformed,
            4 => ErrorKind::TooLarge,
            5 => ErrorKind::Io,
            6 => ErrorKind::Protocol,
            7 => ErrorKind::Internal,
            8 => ErrorKind::Encrypted,
            9 => ErrorKind::BadPassword,
            _ => return perr(format!("unknown error kind {v}")),
        })
    }
}

impl std::fmt::Display for ErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ErrorKind::Unsupported => "unsupported",
            ErrorKind::NotInstalled => "not installed",
            ErrorKind::Malformed => "malformed",
            ErrorKind::TooLarge => "too large",
            ErrorKind::Io => "i/o",
            ErrorKind::Protocol => "protocol",
            ErrorKind::Internal => "internal",
            ErrorKind::Encrypted => "encrypted",
            ErrorKind::BadPassword => "wrong password",
        })
    }
}

/// Host → viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum Response {
    /// Boxed: it is the largest answer and the rarest.
    Hello(Box<HostInfo>),
    Image(ImageReply),
    Listing(Listing),
    Data(Payload),
    /// The entries of a [`Request::RarReadMany`], in its order, one after another in
    /// `payload`: `sizes[i]` bytes each.
    DataMany {
        sizes: Vec<u64>,
        payload: Payload,
    },
    Probe(Vec<ProbeResult>),
    /// A decoded frame of a Blackmagic RAW clip and the clip's facts.
    Clip {
        info: ClipInfo,
        image: ImageReply,
    },
    Echo(Vec<u8>),
    Ok,
    /// A tessellated CAD model ([`crate::cad::CadModel`]'s format).
    Model(Payload),
    /// A CZI document's facts ([`crate::czi::CziInfo`]'s format).
    CziInfo(Payload),
    /// A CZI plane or attachment.
    CziPlane(CziPlane),
    /// The rows of a [`Request::SqlQuery`]: `rows × columns` values in `payload`
    /// ([`crate::sql`]); `truncated` when the query had more rows than allowed.
    Rows {
        columns: u32,
        rows: u32,
        truncated: bool,
        payload: Payload,
    },
    /// Not the answer yet: UnRAR asked for volume `name` (a [`valid_volume_name`]), or SQLite
    /// for its database's WAL ("wal"); the viewer replies with [`Request::Volume`].
    NeedVolume {
        name: String,
    },
    Error {
        kind: ErrorKind,
        message: String,
    },
}

impl Response {
    pub fn error(kind: ErrorKind, message: impl Into<String>) -> Response {
        Response::Error {
            kind,
            message: message.into(),
        }
    }

    /// Descriptors this response expects to travel with it.
    pub fn fd_count(&self) -> usize {
        match self {
            Response::Image(ImageReply {
                pixels: Payload::Fd { .. },
                ..
            })
            | Response::Clip {
                image:
                    ImageReply {
                        pixels: Payload::Fd { .. },
                        ..
                    },
                ..
            }
            | Response::Data(Payload::Fd { .. })
            | Response::Model(Payload::Fd { .. })
            | Response::CziInfo(Payload::Fd { .. })
            | Response::CziPlane(CziPlane {
                payload: Payload::Fd { .. },
                ..
            })
            | Response::DataMany {
                payload: Payload::Fd { .. },
                ..
            }
            | Response::Rows {
                payload: Payload::Fd { .. },
                ..
            } => 1,
            _ => 0,
        }
    }
}

impl Request {
    /// Whether the request carries a password (its encoded frame should be wiped).
    pub fn has_secret(&self) -> bool {
        matches!(
            self,
            Request::RarList {
                password: Some(_),
                ..
            } | Request::RarRead {
                password: Some(_),
                ..
            } | Request::RarReadMany {
                password: Some(_),
                ..
            }
        )
    }

    /// Descriptors this request expects to travel with it.
    pub fn fd_count(&self) -> usize {
        match self {
            Request::HeifDecode {
                input: Input::Fd, ..
            }
            | Request::RarList {
                input: Input::Fd, ..
            }
            | Request::RarRead {
                input: Input::Fd, ..
            }
            | Request::RarReadMany {
                input: Input::Fd, ..
            }
            | Request::CadRead {
                input: Input::Fd, ..
            }
            | Request::BrawDecode {
                input: Input::Fd, ..
            }
            | Request::CziInfo {
                input: Input::Fd, ..
            }
            | Request::CziRead {
                input: Input::Fd, ..
            }
            | Request::CziAttachment {
                input: Input::Fd, ..
            }
            | Request::SqlQuery {
                input: Input::Fd, ..
            }
            | Request::Volume {
                input: Some(Input::Fd),
            } => 1,
            _ => 0,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Encoding

#[derive(Default)]
struct W(Vec<u8>);

impl W {
    fn u8(&mut self, v: u8) {
        self.0.push(v);
    }
    fn bool(&mut self, v: bool) {
        self.0.push(v as u8);
    }
    fn u16(&mut self, v: u16) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn u64(&mut self, v: u64) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f64(&mut self, v: f64) {
        self.u64(v.to_bits());
    }
    fn i32(&mut self, v: i32) {
        self.0.extend_from_slice(&v.to_le_bytes());
    }
    fn f32(&mut self, v: f32) {
        self.u32(v.to_bits());
    }
    fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend_from_slice(v);
    }
    fn str(&mut self, v: &str) {
        self.bytes(v.as_bytes());
    }
    /// Last in its message: see the module docs.
    fn secret(&mut self, v: Option<&Secret>) {
        match v {
            Some(s) => {
                self.0.reserve_exact(5 + s.expose().len());
                self.u8(1);
                self.str(s.expose());
            }
            None => self.u8(0),
        }
    }
    fn input(&mut self, v: &Input) {
        match v {
            Input::Fd => self.u8(0),
            Input::Inline(b) => {
                self.u8(1);
                self.bytes(b);
            }
            Input::Handle(h) => {
                self.u8(2);
                self.u64(*h);
            }
        }
    }
    fn image(&mut self, i: &ImageReply) {
        self.u32(i.width);
        self.u32(i.height);
        self.bool(i.has_alpha);
        self.u8(i.bit_depth);
        self.bytes(&i.icc);
        self.payload(&i.pixels);
    }
    fn payload(&mut self, v: &Payload) {
        match v {
            Payload::Fd { len } => {
                self.u8(0);
                self.u64(*len);
            }
            Payload::Inline(b) => {
                self.u8(1);
                self.bytes(b);
            }
        }
    }
}

struct R<'a>(&'a [u8]);

impl<'a> R<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], ProtocolError> {
        if self.0.len() < n {
            return perr(format!(
                "message truncated: need {n} bytes, {} left",
                self.0.len()
            ));
        }
        let (a, b) = self.0.split_at(n);
        self.0 = b;
        Ok(a)
    }
    fn u8(&mut self) -> Result<u8, ProtocolError> {
        Ok(self.take(1)?[0])
    }
    fn bool(&mut self) -> Result<bool, ProtocolError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            v => perr(format!("bad bool {v}")),
        }
    }
    fn u16(&mut self) -> Result<u16, ProtocolError> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, ProtocolError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn u64(&mut self) -> Result<u64, ProtocolError> {
        let mut a = [0u8; 8];
        a.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(a))
    }
    /// A finite float.
    fn f64(&mut self) -> Result<f64, ProtocolError> {
        let v = f64::from_bits(self.u64()?);
        if v.is_finite() {
            Ok(v)
        } else {
            perr("float is not finite")
        }
    }
    fn i32(&mut self) -> Result<i32, ProtocolError> {
        Ok(self.u32()? as i32)
    }
    /// A finite float.
    fn f32(&mut self) -> Result<f32, ProtocolError> {
        let v = f32::from_bits(self.u32()?);
        if v.is_finite() {
            Ok(v)
        } else {
            perr("float is not finite")
        }
    }
    fn bytes(&mut self) -> Result<Vec<u8>, ProtocolError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }
    fn str(&mut self) -> Result<String, ProtocolError> {
        String::from_utf8(self.bytes()?).or_else(|_| perr("string is not UTF-8"))
    }
    fn secret(&mut self) -> Result<Option<Secret>, ProtocolError> {
        match self.u8()? {
            0 => Ok(None),
            1 => {
                let n = self.u32()? as usize;
                let b = self.take(n)?;
                match std::str::from_utf8(b) {
                    Ok(s) => Ok(Some(Secret::new(s.to_owned()))),
                    Err(_) => perr("password is not UTF-8"),
                }
            }
            v => perr(format!("bad option tag {v}")),
        }
    }
    fn input(&mut self) -> Result<Input, ProtocolError> {
        match self.u8()? {
            0 => Ok(Input::Fd),
            1 => Ok(Input::Inline(self.bytes()?)),
            2 => Ok(Input::Handle(self.u64()?)),
            v => perr(format!("bad input kind {v}")),
        }
    }
    fn image(&mut self) -> Result<ImageReply, ProtocolError> {
        Ok(ImageReply {
            width: self.u32()?,
            height: self.u32()?,
            has_alpha: self.bool()?,
            bit_depth: self.u8()?,
            icc: self.bytes()?,
            pixels: self.payload()?,
        })
    }
    fn payload(&mut self) -> Result<Payload, ProtocolError> {
        match self.u8()? {
            0 => Ok(Payload::Fd { len: self.u64()? }),
            1 => Ok(Payload::Inline(self.bytes()?)),
            v => perr(format!("bad payload kind {v}")),
        }
    }
    /// A [`valid_volume_name`].
    fn volume(&mut self) -> Result<String, ProtocolError> {
        let s = self.str()?;
        if valid_volume_name(&s) {
            Ok(s)
        } else {
            perr("bad volume name")
        }
    }
    fn end(&self) -> Result<(), ProtocolError> {
        if self.0.is_empty() {
            Ok(())
        } else {
            perr(format!("{} trailing bytes", self.0.len()))
        }
    }
    /// A count of items that each take at least `min` bytes, checked against what is left so a
    /// hostile count cannot make the reader allocate.
    fn count(&mut self, min: usize) -> Result<usize, ProtocolError> {
        let n = self.u32()? as usize;
        if n.saturating_mul(min) > self.0.len() {
            return perr(format!("count {n} exceeds the message"));
        }
        Ok(n)
    }
}

mod tag {
    pub const HELLO: u8 = 1;
    pub const HEIF_DECODE: u8 = 2;
    pub const RAR_LIST: u8 = 3;
    pub const RAR_READ: u8 = 4;
    pub const TEST: u8 = 5;
    pub const VOLUME: u8 = 6;
    pub const RAR_READ_MANY: u8 = 7;
    pub const CAD_READ: u8 = 8;
    pub const BRAW_DECODE: u8 = 9;
    pub const CZI_INFO: u8 = 10;
    pub const CZI_READ: u8 = 11;
    pub const CZI_ATTACHMENT: u8 = 12;
    pub const SQL_QUERY: u8 = 13;

    pub const R_HELLO: u8 = 0x81;
    pub const R_IMAGE: u8 = 0x82;
    pub const R_LISTING: u8 = 0x83;
    pub const R_DATA: u8 = 0x84;
    pub const R_PROBE: u8 = 0x85;
    pub const R_ECHO: u8 = 0x86;
    pub const R_OK: u8 = 0x87;
    pub const R_NEED_VOLUME: u8 = 0x88;
    pub const R_DATA_MANY: u8 = 0x89;
    pub const R_MODEL: u8 = 0x8A;
    pub const R_CLIP: u8 = 0x8B;
    pub const R_CZI_INFO: u8 = 0x8C;
    pub const R_CZI_PLANE: u8 = 0x8D;
    pub const R_ROWS: u8 = 0x8E;
    pub const R_ERROR: u8 = 0xFF;

    pub const T_ECHO: u8 = 1;
    pub const T_CRASH: u8 = 2;
    pub const T_HANG: u8 = 3;
    pub const T_ALLOC: u8 = 4;
    pub const T_PROBE: u8 = 5;
    pub const T_OVERFLOW: u8 = 6;
    pub const T_PANIC: u8 = 7;
}

impl Request {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W::default();
        match self {
            Request::Hello { version } => {
                w.u8(tag::HELLO);
                w.0.extend_from_slice(&MAGIC);
                w.u16(*version);
            }
            Request::HeifDecode { input, limits } => {
                w.u8(tag::HEIF_DECODE);
                w.input(input);
                w.u64(limits.max_pixels);
                w.u32(limits.max_side);
            }
            Request::RarList {
                input,
                volume,
                max_entries,
                password,
            } => {
                w.u8(tag::RAR_LIST);
                w.input(input);
                w.str(volume);
                w.u32(*max_entries);
                w.secret(password.as_ref());
            }
            Request::RarRead {
                input,
                volume,
                index,
                name,
                max_size,
                password,
            } => {
                w.u8(tag::RAR_READ);
                w.input(input);
                w.str(volume);
                w.u32(*index);
                w.str(name);
                w.u64(*max_size);
                w.secret(password.as_ref());
            }
            Request::RarReadMany {
                input,
                volume,
                items,
                max_size,
                max_total,
                password,
            } => {
                w.u8(tag::RAR_READ_MANY);
                w.input(input);
                w.str(volume);
                w.u32(items.len() as u32);
                for (index, name) in items {
                    w.u32(*index);
                    w.str(name);
                }
                w.u64(*max_size);
                w.u64(*max_total);
                w.secret(password.as_ref());
            }
            Request::CadRead {
                input,
                format,
                limits,
            } => {
                w.u8(tag::CAD_READ);
                w.input(input);
                w.u8(*format as u8);
                w.f64(limits.relative_deflection);
                w.f64(limits.angular_deflection);
                w.u64(limits.max_triangles);
                w.u64(limits.max_bytes);
            }
            Request::CziInfo {
                input,
                key,
                max_xml,
            } => {
                w.u8(tag::CZI_INFO);
                w.input(input);
                w.u64(*key);
                w.u64(*max_xml);
            }
            Request::CziRead { input, key, region } => {
                w.u8(tag::CZI_READ);
                w.input(input);
                w.u64(*key);
                w.i32(region.x);
                w.i32(region.y);
                w.u32(region.w);
                w.u32(region.h);
                w.f32(region.zoom);
                match region.scene {
                    Some(s) => {
                        w.u8(1);
                        w.u32(s);
                    }
                    None => w.u8(0),
                }
                let coords = region.coords();
                w.u8(coords.len() as u8);
                for (d, v) in coords {
                    w.u8(*d);
                    w.i32(*v);
                }
                w.u8(region.pixel);
                w.u64(region.max_bytes);
            }
            Request::CziAttachment {
                input,
                key,
                index,
                max_side,
                max_bytes,
            } => {
                w.u8(tag::CZI_ATTACHMENT);
                w.input(input);
                w.u64(*key);
                w.u32(*index);
                w.u32(*max_side);
                w.u64(*max_bytes);
            }
            Request::SqlQuery {
                input,
                key,
                sql,
                params,
                limits,
            } => {
                w.u8(tag::SQL_QUERY);
                w.input(input);
                w.u64(*key);
                w.str(sql);
                w.u32(params.len() as u32);
                for p in params {
                    p.encode(&mut w.0);
                }
                w.u32(limits.max_rows);
                w.u64(limits.max_bytes);
                w.u32(limits.max_value);
                w.bool(limits.allow_views);
                w.u32(limits.millis);
            }
            Request::Test(op) => {
                w.u8(tag::TEST);
                match op {
                    TestOp::Echo(b) => {
                        w.u8(tag::T_ECHO);
                        w.bytes(b);
                    }
                    TestOp::Crash => w.u8(tag::T_CRASH),
                    TestOp::Overflow => w.u8(tag::T_OVERFLOW),
                    TestOp::Panic => w.u8(tag::T_PANIC),
                    TestOp::Hang { millis } => {
                        w.u8(tag::T_HANG);
                        w.u32(*millis);
                    }
                    TestOp::Alloc { bytes } => {
                        w.u8(tag::T_ALLOC);
                        w.u64(*bytes);
                    }
                    TestOp::Probe {
                        read_path,
                        write_dir,
                    } => {
                        w.u8(tag::T_PROBE);
                        w.str(read_path);
                        w.str(write_dir);
                    }
                }
            }
            Request::BrawDecode {
                input,
                frame,
                scale,
                limits,
            } => {
                w.u8(tag::BRAW_DECODE);
                w.input(input);
                w.u64(*frame);
                w.u8(*scale);
                w.u64(limits.max_pixels);
                w.u32(limits.max_side);
            }
            Request::Volume { input } => {
                w.u8(tag::VOLUME);
                match input {
                    Some(i) => {
                        w.u8(1);
                        w.input(i);
                    }
                    None => w.u8(0),
                }
            }
        }
        w.0
    }

    pub fn decode(body: &[u8]) -> Result<Request, ProtocolError> {
        let mut r = R(body);
        let req = match r.u8()? {
            tag::HELLO => {
                if r.take(8)? != MAGIC {
                    return perr("bad magic");
                }
                Request::Hello { version: r.u16()? }
            }
            tag::HEIF_DECODE => Request::HeifDecode {
                input: r.input()?,
                limits: ImageLimits {
                    max_pixels: r.u64()?,
                    max_side: r.u32()?,
                },
            },
            tag::RAR_LIST => Request::RarList {
                input: r.input()?,
                volume: r.volume()?,
                max_entries: r.u32()?,
                password: r.secret()?,
            },
            tag::RAR_READ => Request::RarRead {
                input: r.input()?,
                volume: r.volume()?,
                index: r.u32()?,
                name: r.str()?,
                max_size: r.u64()?,
                password: r.secret()?,
            },
            tag::RAR_READ_MANY => {
                let input = r.input()?;
                let volume = r.volume()?;
                let n = r.count(8)?;
                let mut items = Vec::with_capacity(n);
                for _ in 0..n {
                    items.push((r.u32()?, r.str()?));
                }
                Request::RarReadMany {
                    input,
                    volume,
                    items,
                    max_size: r.u64()?,
                    max_total: r.u64()?,
                    password: r.secret()?,
                }
            }
            tag::CAD_READ => Request::CadRead {
                input: r.input()?,
                format: match r.u8()? {
                    1 => CadFormat::Step,
                    2 => CadFormat::Iges,
                    v => return perr(format!("unknown CAD format {v}")),
                },
                limits: CadLimits {
                    relative_deflection: r.f64()?,
                    angular_deflection: r.f64()?,
                    max_triangles: r.u64()?,
                    max_bytes: r.u64()?,
                },
            },
            tag::CZI_INFO => Request::CziInfo {
                input: r.input()?,
                key: r.u64()?,
                max_xml: r.u64()?,
            },
            tag::CZI_READ => {
                let input = r.input()?;
                let key = r.u64()?;
                let (x, y, w, h) = (r.i32()?, r.i32()?, r.u32()?, r.u32()?);
                let zoom = r.f32()?;
                if !(zoom > 0.0 && zoom <= 1.0) {
                    return perr("zoom outside (0, 1]");
                }
                let scene = match r.u8()? {
                    0 => None,
                    1 => Some(r.u32()?),
                    v => return perr(format!("bad option tag {v}")),
                };
                let n = r.u8()?;
                if usize::from(n) > CZI_PLANE_DIMS.len() {
                    return perr("too many plane coordinates");
                }
                let mut coords = [(0u8, 0i32); 8];
                for k in 0..usize::from(n) {
                    let d = r.u8()?;
                    if !CZI_PLANE_DIMS.contains(&d) || coords[..k].iter().any(|c| c.0 == d) {
                        return perr("bad plane dimension");
                    }
                    coords[k] = (d, r.i32()?);
                }
                let pixel = r.u8()?;
                if czi_pixel_bytes(pixel).is_none() {
                    return perr(format!("unknown pixel type {pixel}"));
                }
                Request::CziRead {
                    input,
                    key,
                    region: CziRegion {
                        x,
                        y,
                        w,
                        h,
                        zoom,
                        scene,
                        coords,
                        coord_count: n,
                        pixel,
                        max_bytes: r.u64()?,
                    },
                }
            }
            tag::CZI_ATTACHMENT => Request::CziAttachment {
                input: r.input()?,
                key: r.u64()?,
                index: r.u32()?,
                max_side: r.u32()?,
                max_bytes: r.u64()?,
            },
            tag::SQL_QUERY => {
                let input = r.input()?;
                let key = r.u64()?;
                let sql = r.str()?;
                let n = r.count(1)?;
                let mut params = Vec::with_capacity(n);
                for _ in 0..n {
                    params.push(SqlValue::decode(&mut r.0).map_err(ProtocolError)?);
                }
                Request::SqlQuery {
                    input,
                    key,
                    sql,
                    params,
                    limits: SqlLimits {
                        max_rows: r.u32()?,
                        max_bytes: r.u64()?,
                        max_value: r.u32()?,
                        allow_views: r.bool()?,
                        millis: r.u32()?,
                    },
                }
            }
            tag::TEST => Request::Test(match r.u8()? {
                tag::T_ECHO => TestOp::Echo(r.bytes()?),
                tag::T_CRASH => TestOp::Crash,
                tag::T_OVERFLOW => TestOp::Overflow,
                tag::T_PANIC => TestOp::Panic,
                tag::T_HANG => TestOp::Hang { millis: r.u32()? },
                tag::T_ALLOC => TestOp::Alloc { bytes: r.u64()? },
                tag::T_PROBE => TestOp::Probe {
                    read_path: r.str()?,
                    write_dir: r.str()?,
                },
                t => return perr(format!("unknown test op {t}")),
            }),
            tag::BRAW_DECODE => {
                let input = r.input()?;
                let frame = r.u64()?;
                let scale = r.u8()?;
                if !matches!(scale, 1 | 2 | 4 | 8) {
                    return perr(format!("bad scale {scale}"));
                }
                Request::BrawDecode {
                    input,
                    frame,
                    scale,
                    limits: ImageLimits {
                        max_pixels: r.u64()?,
                        max_side: r.u32()?,
                    },
                }
            }
            tag::VOLUME => Request::Volume {
                input: match r.u8()? {
                    0 => None,
                    1 => Some(r.input()?),
                    v => return perr(format!("bad option tag {v}")),
                },
            },
            t => return perr(format!("unknown request tag {t}")),
        };
        r.end()?;
        Ok(req)
    }
}

impl Response {
    pub fn encode(&self) -> Vec<u8> {
        let mut w = W::default();
        match self {
            Response::Hello(h) => {
                w.u8(tag::R_HELLO);
                w.0.extend_from_slice(&MAGIC);
                w.u16(h.version);
                w.u8(h.sandbox as u8);
                w.str(&h.sandbox_detail);
                match &h.heif {
                    Some(v) => {
                        w.u8(1);
                        w.str(v);
                    }
                    None => w.u8(0),
                }
                w.str(&h.heif_error);
                w.bool(h.rar);
                match &h.occt {
                    Some(v) => {
                        w.u8(1);
                        w.str(v);
                    }
                    None => w.u8(0),
                }
                w.str(&h.occt_error);
                w.bool(h.test_hooks);
                match &h.braw {
                    Some(v) => {
                        w.u8(1);
                        w.str(v);
                    }
                    None => w.u8(0),
                }
                w.str(&h.braw_error);
                match &h.czi {
                    Some(v) => {
                        w.u8(1);
                        w.str(v);
                    }
                    None => w.u8(0),
                }
                w.str(&h.czi_error);
            }
            Response::Image(i) => {
                w.u8(tag::R_IMAGE);
                w.image(i);
            }
            Response::Clip { info, image } => {
                w.u8(tag::R_CLIP);
                w.u32(info.width);
                w.u32(info.height);
                w.u64(info.frame_count);
                w.u32(info.frame_rate.to_bits());
                w.str(&info.camera);
                w.image(image);
            }
            Response::Listing(l) => {
                w.u8(tag::R_LISTING);
                w.bool(l.solid);
                w.u32(l.entries.len() as u32);
                for e in &l.entries {
                    w.str(&e.name);
                    w.u64(e.size);
                    w.bool(e.encrypted);
                    w.bool(e.directory);
                }
            }
            Response::Data(p) => {
                w.u8(tag::R_DATA);
                w.payload(p);
            }
            Response::DataMany { sizes, payload } => {
                w.u8(tag::R_DATA_MANY);
                w.u32(sizes.len() as u32);
                for s in sizes {
                    w.u64(*s);
                }
                w.payload(payload);
            }
            Response::Probe(results) => {
                w.u8(tag::R_PROBE);
                w.u32(results.len() as u32);
                for p in results {
                    w.str(&p.name);
                    w.bool(p.allowed);
                    w.str(&p.detail);
                }
            }
            Response::Echo(b) => {
                w.u8(tag::R_ECHO);
                w.bytes(b);
            }
            Response::Ok => w.u8(tag::R_OK),
            Response::Model(p) => {
                w.u8(tag::R_MODEL);
                w.payload(p);
            }
            Response::CziInfo(p) => {
                w.u8(tag::R_CZI_INFO);
                w.payload(p);
            }
            Response::CziPlane(p) => {
                w.u8(tag::R_CZI_PLANE);
                w.u8(p.kind as u8);
                w.u32(p.width);
                w.u32(p.height);
                w.u8(p.pixel);
                w.str(&p.content_type);
                match p.display {
                    Some(d) => {
                        w.u8(1);
                        w.f32(d.black);
                        w.f32(d.white);
                        match d.gamma {
                            Some(g) => {
                                w.u8(1);
                                w.f32(g);
                            }
                            None => w.u8(0),
                        }
                    }
                    None => w.u8(0),
                }
                w.payload(&p.payload);
            }
            Response::Rows {
                columns,
                rows,
                truncated,
                payload,
            } => {
                w.u8(tag::R_ROWS);
                w.u32(*columns);
                w.u32(*rows);
                w.bool(*truncated);
                w.payload(payload);
            }
            Response::NeedVolume { name } => {
                w.u8(tag::R_NEED_VOLUME);
                w.str(name);
            }
            Response::Error { kind, message } => {
                w.u8(tag::R_ERROR);
                w.u8(*kind as u8);
                w.str(message);
            }
        }
        w.0
    }

    pub fn decode(body: &[u8]) -> Result<Response, ProtocolError> {
        let mut r = R(body);
        let resp = match r.u8()? {
            tag::R_HELLO => {
                if r.take(8)? != MAGIC {
                    return perr("bad magic");
                }
                let version = r.u16()?;
                let sandbox = match r.u8()? {
                    0 => SandboxLevel::None,
                    1 => SandboxLevel::Partial,
                    2 => SandboxLevel::Full,
                    v => return perr(format!("bad sandbox level {v}")),
                };
                let sandbox_detail = r.str()?;
                let heif = match r.u8()? {
                    0 => None,
                    1 => Some(r.str()?),
                    v => return perr(format!("bad option tag {v}")),
                };
                let heif_error = r.str()?;
                let rar = r.bool()?;
                let occt = match r.u8()? {
                    0 => None,
                    1 => Some(r.str()?),
                    v => return perr(format!("bad option tag {v}")),
                };
                let occt_error = r.str()?;
                let test_hooks = r.bool()?;
                let braw = match r.u8()? {
                    0 => None,
                    1 => Some(r.str()?),
                    v => return perr(format!("bad option tag {v}")),
                };
                Response::Hello(Box::new(HostInfo {
                    version,
                    sandbox,
                    sandbox_detail,
                    heif,
                    heif_error,
                    rar,
                    occt,
                    occt_error,
                    test_hooks,
                    braw,
                    braw_error: r.str()?,
                    czi: match r.u8()? {
                        0 => None,
                        1 => Some(r.str()?),
                        v => return perr(format!("bad option tag {v}")),
                    },
                    czi_error: r.str()?,
                }))
            }
            tag::R_IMAGE => Response::Image(r.image()?),
            tag::R_CLIP => {
                let info = ClipInfo {
                    width: r.u32()?,
                    height: r.u32()?,
                    frame_count: r.u64()?,
                    frame_rate: f32::from_bits(r.u32()?),
                    camera: r.str()?,
                };
                Response::Clip {
                    info,
                    image: r.image()?,
                }
            }
            tag::R_LISTING => {
                let solid = r.bool()?;
                let n = r.count(14)?;
                let mut entries = Vec::with_capacity(n);
                for _ in 0..n {
                    entries.push(ListEntry {
                        name: r.str()?,
                        size: r.u64()?,
                        encrypted: r.bool()?,
                        directory: r.bool()?,
                    });
                }
                Response::Listing(Listing { solid, entries })
            }
            tag::R_DATA => Response::Data(r.payload()?),
            tag::R_DATA_MANY => {
                let n = r.count(8)?;
                let mut sizes = Vec::with_capacity(n);
                for _ in 0..n {
                    sizes.push(r.u64()?);
                }
                Response::DataMany {
                    sizes,
                    payload: r.payload()?,
                }
            }
            tag::R_PROBE => {
                let n = r.count(9)?;
                let mut v = Vec::with_capacity(n);
                for _ in 0..n {
                    v.push(ProbeResult {
                        name: r.str()?,
                        allowed: r.bool()?,
                        detail: r.str()?,
                    });
                }
                Response::Probe(v)
            }
            tag::R_ECHO => Response::Echo(r.bytes()?),
            tag::R_OK => Response::Ok,
            tag::R_MODEL => Response::Model(r.payload()?),
            tag::R_CZI_INFO => Response::CziInfo(r.payload()?),
            tag::R_CZI_PLANE => {
                let kind = match r.u8()? {
                    0 => CziPlaneKind::Encoded,
                    1 => CziPlaneKind::Pixels,
                    v => return perr(format!("bad plane kind {v}")),
                };
                let (width, height, pixel) = (r.u32()?, r.u32()?, r.u8()?);
                let content_type = r.str()?;
                if content_type.len() > 8 || !content_type.bytes().all(|b| b.is_ascii_graphic()) {
                    return perr("bad content type");
                }
                let display = match r.u8()? {
                    0 => None,
                    1 => Some(CziDisplay {
                        black: r.f32()?,
                        white: r.f32()?,
                        gamma: match r.u8()? {
                            0 => None,
                            1 => Some(r.f32()?),
                            v => return perr(format!("bad option tag {v}")),
                        },
                    }),
                    v => return perr(format!("bad option tag {v}")),
                };
                Response::CziPlane(CziPlane {
                    kind,
                    width,
                    height,
                    pixel,
                    content_type,
                    display,
                    payload: r.payload()?,
                })
            }
            tag::R_ROWS => Response::Rows {
                columns: r.u32()?,
                rows: r.u32()?,
                truncated: r.bool()?,
                payload: r.payload()?,
            },
            tag::R_NEED_VOLUME => Response::NeedVolume { name: r.volume()? },
            tag::R_ERROR => Response::Error {
                kind: ErrorKind::from_u8(r.u8()?)?,
                message: r.str()?,
            },
            t => return perr(format!("unknown response tag {t:#x}")),
        };
        r.end()?;
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn requests() -> Vec<Request> {
        vec![
            Request::Hello { version: VERSION },
            Request::HeifDecode {
                input: Input::Fd,
                limits: ImageLimits::default(),
            },
            Request::HeifDecode {
                input: Input::Inline(vec![1, 2, 3]),
                limits: ImageLimits {
                    max_pixels: 7,
                    max_side: 9,
                },
            },
            Request::RarList {
                input: Input::Fd,
                volume: "a.part1.rar".into(),
                max_entries: 100,
                password: None,
            },
            Request::RarList {
                input: Input::Fd,
                volume: "h.rar".into(),
                max_entries: 1,
                password: Some(Secret::new("pässwörd".into())),
            },
            Request::RarRead {
                input: Input::Inline(b"Rar!".to_vec()),
                volume: "x.cbr".into(),
                index: 3,
                name: "ch1/p01.jpg".into(),
                max_size: 1 << 40,
                password: Some(Secret::new(String::new())),
            },
            Request::RarReadMany {
                input: Input::Fd,
                volume: "s.cbr".into(),
                items: vec![(1, "p01.jpg".into()), (7, "ü/p02.jpg".into())],
                max_size: 1 << 30,
                max_total: 64 << 20,
                password: Some(Secret::new("x".into())),
            },
            Request::RarReadMany {
                input: Input::Inline(vec![]),
                volume: "e.rar".into(),
                items: vec![],
                max_size: 0,
                max_total: 0,
                password: None,
            },
            Request::CadRead {
                input: Input::Fd,
                format: CadFormat::Step,
                limits: CadLimits::default(),
            },
            Request::CadRead {
                input: Input::Inline(b"ISO-10303-21;".to_vec()),
                format: CadFormat::Iges,
                limits: CadLimits {
                    relative_deflection: 0.25,
                    angular_deflection: 1.0,
                    max_triangles: 0,
                    max_bytes: u64::MAX,
                },
            },
            Request::CziInfo {
                input: Input::Fd,
                key: 7,
                max_xml: 1 << 20,
            },
            Request::CziRead {
                input: Input::Handle(5),
                key: u64::MAX,
                region: CziRegion {
                    x: -123_977,
                    y: 28_824,
                    w: 4096,
                    h: 1024,
                    zoom: 0.25,
                    scene: Some(1),
                    coords: [
                        (b'C', 2),
                        (b'Z', -1),
                        (0, 0),
                        (0, 0),
                        (0, 0),
                        (0, 0),
                        (0, 0),
                        (0, 0),
                    ],
                    coord_count: 2,
                    pixel: 1,
                    max_bytes: 1 << 30,
                },
            },
            Request::CziAttachment {
                input: Input::Inline(vec![1]),
                key: 0,
                index: 3,
                max_side: 2048,
                max_bytes: 64 << 20,
            },
            Request::SqlQuery {
                input: Input::Fd,
                key: 9,
                sql: "SELECT ?1, ?2".into(),
                params: vec![SqlValue::Int(3), SqlValue::Text("ü".into()), SqlValue::Null],
                limits: SqlLimits::default(),
            },
            Request::SqlQuery {
                input: Input::Handle(4),
                key: u64::MAX,
                sql: String::new(),
                params: vec![SqlValue::Blob(vec![1, 2]), SqlValue::Real(-0.5)],
                limits: SqlLimits {
                    max_rows: 1,
                    max_bytes: 2,
                    max_value: 3,
                    allow_views: true,
                    millis: 4,
                },
            },
            Request::Test(TestOp::Echo(vec![0; 300])),
            Request::Test(TestOp::Crash),
            Request::Test(TestOp::Overflow),
            Request::Test(TestOp::Panic),
            Request::Test(TestOp::Hang { millis: 5 }),
            Request::Test(TestOp::Alloc { bytes: 1 << 33 }),
            Request::Test(TestOp::Probe {
                read_path: "/etc/hostname".into(),
                write_dir: "ü/x".into(),
            }),
            Request::BrawDecode {
                input: Input::Fd,
                frame: 7,
                scale: 4,
                limits: ImageLimits::default(),
            },
            Request::BrawDecode {
                input: Input::Handle(5),
                frame: u64::MAX,
                scale: 1,
                limits: ImageLimits {
                    max_pixels: 1,
                    max_side: 1,
                },
            },
            Request::RarRead {
                input: Input::Handle(0x1234_5678_9ABC),
                volume: "big.rar".into(),
                index: 0,
                name: String::new(),
                max_size: 0,
                password: None,
            },
            Request::Volume {
                input: Some(Input::Fd),
            },
            Request::Volume {
                input: Some(Input::Handle(u64::MAX)),
            },
            Request::Volume { input: None },
        ]
    }

    fn responses() -> Vec<Response> {
        vec![
            Response::Hello(Box::new(HostInfo {
                version: VERSION,
                sandbox: SandboxLevel::Full,
                sandbox_detail: "seccomp".into(),
                heif: Some("1.21.2".into()),
                heif_error: String::new(),
                rar: true,
                occt: Some("7.9.3".into()),
                occt_error: String::new(),
                czi: Some("0.70.0".into()),
                czi_error: String::new(),
                test_hooks: false,
                braw: Some("6.0".into()),
                braw_error: String::new(),
            })),
            Response::Hello(Box::new(HostInfo {
                version: 9,
                sandbox: SandboxLevel::None,
                sandbox_detail: String::new(),
                heif: None,
                heif_error: "libheif.so.1: not found".into(),
                rar: false,
                occt: None,
                occt_error: "libTKernel.so.7.9: not found".into(),
                czi: None,
                czi_error: "liblibCZI.so.1: not found".into(),
                test_hooks: true,
                braw: None,
                braw_error: "libBlackmagicRawAPI.so: not found".into(),
            })),
            Response::Image(ImageReply {
                width: 3,
                height: 2,
                has_alpha: true,
                bit_depth: 10,
                icc: vec![9; 10],
                pixels: Payload::Fd { len: 24 },
            }),
            Response::Image(ImageReply {
                width: 1,
                height: 1,
                has_alpha: false,
                bit_depth: 8,
                icc: vec![],
                pixels: Payload::Inline(vec![1, 2, 3, 255]),
            }),
            Response::Listing(Listing {
                solid: true,
                entries: vec![
                    ListEntry {
                        name: "a/b.jpg".into(),
                        size: 5,
                        encrypted: false,
                        directory: false,
                    },
                    ListEntry {
                        name: "a".into(),
                        size: 0,
                        encrypted: true,
                        directory: true,
                    },
                ],
            }),
            Response::Listing(Listing {
                solid: false,
                entries: vec![],
            }),
            Response::Data(Payload::Fd { len: u64::MAX }),
            Response::Data(Payload::Inline(vec![7; 1000])),
            Response::DataMany {
                sizes: vec![3, 0, 5],
                payload: Payload::Fd { len: 8 },
            },
            Response::DataMany {
                sizes: vec![],
                payload: Payload::Inline(vec![]),
            },
            Response::Probe(vec![ProbeResult {
                name: "socket".into(),
                allowed: false,
                detail: "ENOSYS".into(),
            }]),
            Response::Echo(vec![]),
            Response::Ok,
            Response::error(ErrorKind::Encrypted, "password"),
            Response::error(ErrorKind::BadPassword, "wrong password"),
            Response::Model(Payload::Fd { len: 99 }),
            Response::Model(Payload::Inline(b"SPCADM01".to_vec())),
            Response::Clip {
                info: ClipInfo {
                    width: 12288,
                    height: 6480,
                    frame_count: 1 << 33,
                    frame_rate: 23.976,
                    camera: "Blackmagic URSA Mini Pro 12K".into(),
                },
                image: ImageReply {
                    width: 2,
                    height: 1,
                    has_alpha: false,
                    bit_depth: 12,
                    icc: vec![],
                    pixels: Payload::Fd { len: 8 },
                },
            },
            Response::CziInfo(Payload::Fd { len: 99 }),
            Response::CziPlane(CziPlane {
                kind: CziPlaneKind::Pixels,
                width: 2,
                height: 1,
                pixel: 4,
                content_type: String::new(),
                display: Some(CziDisplay {
                    black: 0.2,
                    white: 0.9,
                    gamma: Some(0.45),
                }),
                payload: Payload::Inline(vec![0; 12]),
            }),
            Response::CziPlane(CziPlane {
                kind: CziPlaneKind::Encoded,
                width: 0,
                height: 0,
                pixel: 255,
                content_type: "JPG".into(),
                display: None,
                payload: Payload::Fd { len: 5 },
            }),
            Response::Rows {
                columns: 3,
                rows: 2,
                truncated: true,
                payload: Payload::Inline(vec![0; 6]),
            },
            Response::Rows {
                columns: 0,
                rows: 0,
                truncated: false,
                payload: Payload::Fd { len: 1 << 20 },
            },
            Response::NeedVolume {
                name: "a.part02.rar".into(),
            },
        ]
    }

    #[test]
    fn requests_round_trip() {
        for req in requests() {
            let b = req.encode();
            assert_eq!(Request::decode(&b).unwrap(), req);
            // Every strict prefix is an error, never a panic.
            for n in 0..b.len() {
                assert!(Request::decode(&b[..n]).is_err(), "{req:?} prefix {n}");
            }
            let mut long = b.clone();
            long.push(0);
            assert!(Request::decode(&long).is_err());
        }
    }

    #[test]
    fn responses_round_trip() {
        for resp in responses() {
            let b = resp.encode();
            assert_eq!(Response::decode(&b).unwrap(), resp);
            for n in 0..b.len() {
                assert!(Response::decode(&b[..n]).is_err(), "{resp:?} prefix {n}");
            }
        }
    }

    #[test]
    fn fd_counts() {
        assert_eq!(requests()[1].fd_count(), 1);
        assert_eq!(requests()[2].fd_count(), 0);
        assert_eq!(responses()[2].fd_count(), 1);
        assert_eq!(responses()[3].fd_count(), 0);
        assert_eq!(requests()[6].fd_count(), 1, "batch read with a descriptor");
        assert_eq!(requests()[7].fd_count(), 0);
        assert_eq!(responses()[8].fd_count(), 1, "batch data in shared memory");
        assert_eq!(responses()[9].fd_count(), 0);
        assert_eq!(requests()[8].fd_count(), 1, "CAD model in shared memory");
        assert_eq!(requests()[9].fd_count(), 0);
        let models: Vec<usize> = responses()
            .iter()
            .filter(|r| matches!(r, Response::Model(_)))
            .map(Response::fd_count)
            .collect();
        assert_eq!(models, [1, 0]);
        let r = requests();
        assert_eq!(
            r[r.len() - 4].fd_count(),
            0,
            "handles do not travel as descriptors"
        );
        assert_eq!(r[r.len() - 3].fd_count(), 1);
        assert_eq!(r[r.len() - 2].fd_count(), 0);
        assert_eq!(r[r.len() - 1].fd_count(), 0);
        assert_eq!(responses().last().unwrap().fd_count(), 0);
        let braw: Vec<_> = r
            .iter()
            .filter(|q| matches!(q, Request::BrawDecode { .. }))
            .map(Request::fd_count)
            .collect();
        assert_eq!(braw, [1, 0], "a clip as a descriptor, then as a handle");
        let clip = responses()
            .into_iter()
            .find(|r| matches!(r, Response::Clip { .. }));
        assert_eq!(clip.unwrap().fd_count(), 1);
        let rows: Vec<usize> = responses()
            .iter()
            .filter(|r| matches!(r, Response::Rows { .. }))
            .map(Response::fd_count)
            .collect();
        assert_eq!(rows, [0, 1]);
        let sql: Vec<usize> = r
            .iter()
            .filter(|q| matches!(q, Request::SqlQuery { .. }))
            .map(Request::fd_count)
            .collect();
        assert_eq!(sql, [1, 0]);
    }

    #[test]
    fn braw_scales_are_checked() {
        let mut b = Request::BrawDecode {
            input: Input::Fd,
            frame: 0,
            scale: 2,
            limits: ImageLimits::default(),
        }
        .encode();
        // tag, input kind, frame (8 bytes), then the scale.
        b[10] = 3;
        assert!(Request::decode(&b).is_err());
    }

    #[test]
    fn volume_names() {
        for ok in ["a.rar", "x.part001.rar", "A-b_c.r00", "..x", "1"] {
            assert!(valid_volume_name(ok), "{ok}");
        }
        let long = "a".repeat(MAX_VOLUME_NAME + 1);
        for bad in [
            "", ".", "..", "a/b", "a\\b", "c:x", "ü.rar", "a b", "a\0b", &long,
        ] {
            assert!(!valid_volume_name(bad), "{bad:?}");
        }
        assert_eq!(volume_name("Comic Ü.part1.rar"), "Comic__.part1.rar");
        assert_eq!(volume_name("x/y"), "x_y");
        assert_eq!(volume_name(".."), "archive.rar");
        assert_eq!(volume_name(""), "archive.rar");
        let tail = format!("{}.part2.rar", "é".repeat(300));
        let v = volume_name(&tail);
        assert_eq!(v.len(), MAX_VOLUME_NAME);
        assert!(v.ends_with(".part2.rar"));
        // Same length in characters as the real name's kept tail: positions line up.
        assert_eq!(
            volume_name("日本.rar").chars().count(),
            "日本.rar".chars().count()
        );
    }

    #[test]
    fn bad_volume_names_are_protocol_errors() {
        let mut b = vec![tag::R_NEED_VOLUME];
        b.extend_from_slice(&5u32.to_le_bytes());
        b.extend_from_slice(b"../x2");
        assert!(Response::decode(&b).is_err());
        let ok = Response::NeedVolume {
            name: "x.r00".into(),
        }
        .encode();
        assert!(Response::decode(&ok).is_ok());
        let mut req = Request::RarList {
            input: Input::Fd,
            volume: "a.rar".into(),
            max_entries: 1,
            password: None,
        }
        .encode();
        // Patch the volume name's first byte to a separator.
        req[6] = b'/';
        assert!(Request::decode(&req).is_err());
        // An unknown option tag in a volume reply.
        assert!(Request::decode(&[tag::VOLUME, 2]).is_err());
        assert!(Request::decode(&[tag::VOLUME, 1, 9]).is_err());
    }

    #[test]
    fn hostile_bytes_never_panic() {
        // Deterministic xorshift noise behind every tag.
        let mut s = 0x2545_F491_4F6C_DD1Du64;
        for t in 0..=255u8 {
            for len in [0usize, 1, 5, 17, 64] {
                let mut b = vec![t];
                for _ in 0..len {
                    s ^= s << 13;
                    s ^= s >> 7;
                    s ^= s << 17;
                    b.push(s as u8);
                }
                let _ = Request::decode(&b);
                let _ = Response::decode(&b);
            }
        }
        // A huge declared count does not allocate.
        let mut b = vec![tag::R_LISTING, 0];
        b.extend_from_slice(&u32::MAX.to_le_bytes());
        assert!(Response::decode(&b).is_err());
        assert!(Request::decode(&[tag::HELLO, b'X', 0, 0, 0, 0, 0, 0, 0, 1, 0]).is_err());
    }

    #[test]
    fn mutated_messages_never_panic() {
        // Every byte of every valid message replaced by a few values: decoding either fails or
        // yields a message that re-encodes to the same bytes.
        let mut msgs: Vec<(bool, Vec<u8>)> =
            requests().iter().map(|r| (true, r.encode())).collect();
        msgs.extend(responses().iter().map(|r| (false, r.encode())));
        for (is_req, b) in msgs {
            for i in 0..b.len().min(64) {
                for v in [0u8, 1, 2, 0x2F, 0x7F, 0xFF] {
                    let mut m = b.clone();
                    m[i] = v;
                    if is_req {
                        if let Ok(r) = Request::decode(&m) {
                            assert_eq!(r.encode(), m);
                        }
                    } else if let Ok(r) = Response::decode(&m) {
                        assert_eq!(r.encode(), m);
                    }
                }
            }
        }
    }

    #[test]
    fn passwords_travel_last_and_stay_hidden() {
        let req = Request::RarList {
            input: Input::Fd,
            volume: "a.rar".into(),
            max_entries: 1,
            password: Some(Secret::new("hunter2".into())),
        };
        assert!(req.has_secret());
        assert!(!format!("{req:?}").contains("hunter2"));
        assert!(req.encode().ends_with(b"hunter2"));
        assert!(!requests()[0].has_secret());
        // Not UTF-8, or an unknown option tag.
        let mut b = Request::RarList {
            input: Input::Fd,
            volume: "a.rar".into(),
            max_entries: 1,
            password: Some(Secret::new("ab".into())),
        }
        .encode();
        let n = b.len();
        b[n - 1] = 0xFF;
        assert!(Request::decode(&b).is_err());
        b[n - 7] = 2;
        assert!(Request::decode(&b).is_err());
    }

    #[test]
    fn cad_limits_must_be_finite() {
        let mut b = Request::CadRead {
            input: Input::Fd,
            format: CadFormat::Step,
            limits: CadLimits::default(),
        }
        .encode();
        // tag, input kind, format, then the relative deflection.
        b[3..11].copy_from_slice(&f64::NAN.to_bits().to_le_bytes());
        assert!(Request::decode(&b).is_err());
        b[2] = 3;
        assert!(Request::decode(&b).is_err(), "unknown format");
    }

    #[test]
    fn czi_messages_are_checked() {
        let region = |zoom: f32, pixel: u8| Request::CziRead {
            input: Input::Fd,
            key: 1,
            region: CziRegion {
                x: 0,
                y: 0,
                w: 1,
                h: 1,
                zoom,
                scene: None,
                coords: [(b'T', 0); 8],
                coord_count: 1,
                pixel,
                max_bytes: 8,
            },
        };
        assert!(Request::decode(&region(1.0, 3).encode()).is_ok());
        for bad in [
            region(0.0, 3),
            region(1.5, 3),
            region(f32::NAN, 3),
            region(1.0, 9),
        ] {
            assert!(Request::decode(&bad.encode()).is_err(), "{bad:?}");
        }
        // A scene is no plane coordinate; a dimension named twice is refused.
        let mut twice = region(1.0, 0);
        if let Request::CziRead { region, .. } = &mut twice {
            region.coords[0] = (b'S', 0);
        }
        assert!(Request::decode(&twice.encode()).is_err());
        if let Request::CziRead { region, .. } = &mut twice {
            region.coords[0] = (b'Z', 0);
            region.coords[1] = (b'Z', 1);
            region.coord_count = 2;
        }
        assert!(Request::decode(&twice.encode()).is_err());
        // Response: content types are short ASCII, floats finite.
        let plane = |ct: &str, black: f32| {
            Response::CziPlane(CziPlane {
                kind: CziPlaneKind::Encoded,
                width: 0,
                height: 0,
                pixel: 255,
                content_type: ct.into(),
                display: Some(CziDisplay {
                    black,
                    white: 1.0,
                    gamma: None,
                }),
                payload: Payload::Inline(vec![]),
            })
        };
        assert!(Response::decode(&plane("PNG", 0.0).encode()).is_ok());
        for bad in [
            plane("TOOLONGXX", 0.0),
            plane("a b", 0.0),
            plane("JPG", f32::INFINITY),
        ] {
            assert!(Response::decode(&bad.encode()).is_err());
        }
    }

    #[test]
    fn headers() {
        let h = header(1234, 1).unwrap();
        assert_eq!(parse_header(&h).unwrap(), (1234, 1));
        assert!(header(MAX_FRAME + 1, 0).is_err());
        assert!(header(1, MAX_FDS + 1).is_err());
        let mut bad = h;
        bad[6] = 1;
        assert!(parse_header(&bad).is_err());
        let big = [0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0, 0];
        assert!(parse_header(&big).is_err());
    }
}
