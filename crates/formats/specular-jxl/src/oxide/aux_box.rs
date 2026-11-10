use crate::bitstream::ParseEvent;
use crate::bitstream::container::ContainerBoxType;

use crate::oxide::Result;

mod exif;

pub use exif::*;

#[derive(Debug, Default)]
pub struct AuxBoxReader {
    data: DataKind,
    done: bool,
}

#[derive(Default)]
enum DataKind {
    #[default]
    Init,
    NoData,
    Raw(Vec<u8>),
    /// Brotli-compressed (`brob`) Exif or XML box, buffered until it ends; then decompressed
    /// into `Raw` (or dropped when it is not valid Brotli or too large).
    Brotli(Vec<u8>),
    /// Brotli-compressed (`brob`) box of any other type, or one too large: its data is dropped.
    Skipped,
}

/// Limit of a buffered Brotli-compressed box, compressed and decompressed: well above any
/// real Exif or XMP block.
const MAX_BROTLI_BOX: usize = 16 << 20;

impl std::fmt::Debug for DataKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Init => write!(f, "Init"),
            Self::NoData => write!(f, "NoData"),
            Self::Raw(buf) => f
                .debug_tuple("Raw")
                .field(&format_args!("{} byte(s)", buf.len()))
                .finish(),
            Self::Brotli(buf) => f
                .debug_tuple("Brotli")
                .field(&format_args!("{} byte(s)", buf.len()))
                .finish(),
            Self::Skipped => write!(f, "Skipped"),
        }
    }
}

impl AuxBoxReader {
    pub(super) fn new() -> Self {
        Self::default()
    }

    pub(super) fn ensure_raw(&mut self) {
        if self.done {
            return;
        }

        match self.data {
            DataKind::Init => {
                self.data = DataKind::Raw(Vec::new());
            }
            DataKind::NoData | DataKind::Skipped | DataKind::Brotli(_) => {
                self.data = DataKind::Raw(Vec::new());
            }
            DataKind::Raw(_) => {}
        }
    }

    pub(super) fn ensure_brotli(&mut self) {
        if !self.done {
            self.data = DataKind::Brotli(Vec::new());
        }
    }

    pub(super) fn ensure_skipped(&mut self) {
        if !self.done {
            self.data = DataKind::Skipped;
        }
    }
}

impl AuxBoxReader {
    pub fn feed_data(&mut self, data: &[u8]) -> Result<()> {
        if self.done {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "cannot feed into finalized box",
            )
            .into());
        }

        match self.data {
            DataKind::Init => {
                self.data = DataKind::Raw(data.to_vec());
            }
            DataKind::NoData | DataKind::Skipped => {}
            DataKind::Raw(ref mut buf) => {
                buf.extend_from_slice(data);
            }
            DataKind::Brotli(ref mut buf) => {
                if buf.len() + data.len() > MAX_BROTLI_BOX {
                    self.data = DataKind::Skipped;
                } else {
                    buf.extend_from_slice(data);
                }
            }
        }
        Ok(())
    }

    pub fn finalize(&mut self) -> Result<()> {
        if self.done {
            return Ok(());
        }

        match std::mem::replace(&mut self.data, DataKind::NoData) {
            DataKind::Init | DataKind::NoData | DataKind::Skipped => {}
            DataKind::Raw(buf) => self.data = DataKind::Raw(buf),
            DataKind::Brotli(buf) => {
                if let Ok(raw) = specular_brotli::decompress(&buf, MAX_BROTLI_BOX) {
                    self.data = DataKind::Raw(raw);
                }
            }
        }

        self.done = true;
        Ok(())
    }
}

impl AuxBoxReader {
    pub fn is_done(&self) -> bool {
        self.done
    }

    pub fn data(&self) -> AuxBoxData<&[u8]> {
        if !self.is_done() {
            return AuxBoxData::Decoding;
        }

        match &self.data {
            DataKind::Init | DataKind::Brotli(_) => AuxBoxData::Decoding,
            DataKind::NoData | DataKind::Skipped => AuxBoxData::NotFound,
            DataKind::Raw(buf) => AuxBoxData::Data(buf),
        }
    }
}

/// Auxiliary box data.
pub enum AuxBoxData<T> {
    /// The box has data.
    Data(T),
    /// The box has not been decoded yet.
    Decoding,
    /// The box was not found.
    NotFound,
}

impl<T> std::fmt::Debug for AuxBoxData<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data(_) => write!(f, "Data(_)"),
            Self::Decoding => write!(f, "Decoding"),
            Self::NotFound => write!(f, "NotFound"),
        }
    }
}

impl<T> AuxBoxData<T> {
    pub fn has_data(&self) -> bool {
        matches!(self, Self::Data(_))
    }

    pub fn is_decoding(&self) -> bool {
        matches!(self, Self::Decoding)
    }

    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    pub fn unwrap(self) -> T {
        let Self::Data(x) = self else {
            panic!("cannot unwrap `AuxBoxData` which doesn't have any data");
        };
        x
    }

    pub fn unwrap_or(self, or: T) -> T {
        match self {
            Self::Data(x) => x,
            _ => or,
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> AuxBoxData<U> {
        match self {
            Self::Data(x) => AuxBoxData::Data(f(x)),
            Self::Decoding => AuxBoxData::Decoding,
            Self::NotFound => AuxBoxData::NotFound,
        }
    }

    pub fn as_ref(&self) -> AuxBoxData<&T> {
        match self {
            Self::Data(x) => AuxBoxData::Data(x),
            Self::Decoding => AuxBoxData::Decoding,
            Self::NotFound => AuxBoxData::NotFound,
        }
    }
}

impl<T, E> AuxBoxData<std::result::Result<T, E>> {
    pub fn transpose(self) -> std::result::Result<AuxBoxData<T>, E> {
        match self {
            Self::Data(Ok(x)) => Ok(AuxBoxData::Data(x)),
            Self::Data(Err(e)) => Err(e),
            Self::Decoding => Ok(AuxBoxData::Decoding),
            Self::NotFound => Ok(AuxBoxData::NotFound),
        }
    }
}

/// Auxiliary box list of a JPEG XL container, which may contain Exif and/or XMP metadata.
#[derive(Debug)]
pub struct AuxBoxList {
    boxes: Vec<(ContainerBoxType, AuxBoxReader)>,
    jbrd_seen: bool,
    current_box_ty: Option<ContainerBoxType>,
    current_box: AuxBoxReader,
    last_box: bool,
}

impl AuxBoxList {
    pub(super) fn new() -> Self {
        Self {
            boxes: Vec::new(),
            jbrd_seen: false,
            current_box_ty: None,
            current_box: AuxBoxReader::new(),
            last_box: false,
        }
    }

    pub(super) fn handle_event(&mut self, event: ParseEvent) -> Result<()> {
        match event {
            ParseEvent::BitstreamKind(_) => {}
            ParseEvent::Codestream(_) => {}
            ParseEvent::NoMoreAuxBox => {
                self.current_box_ty = None;
                self.last_box = true;
            }
            ParseEvent::AuxBoxStart {
                ty,
                brotli_compressed,
                last_box,
            } => {
                self.current_box_ty = Some(ty);
                if ty != ContainerBoxType::JPEG_RECONSTRUCTION {
                    if brotli_compressed {
                        if ty == ContainerBoxType::EXIF || ty == ContainerBoxType::XML {
                            self.current_box.ensure_brotli();
                        } else {
                            self.current_box.ensure_skipped();
                        }
                    } else {
                        self.current_box.ensure_raw();
                    }
                }
                self.last_box = last_box;
            }
            ParseEvent::AuxBoxData(ty, buf) => {
                self.current_box_ty = Some(ty);
                if ty == ContainerBoxType::JPEG_RECONSTRUCTION {
                    self.jbrd_seen = true;
                } else {
                    self.current_box.feed_data(buf)?;
                }
            }
            ParseEvent::AuxBoxEnd(ty) => {
                self.current_box_ty = Some(ty);
                self.finalize()?;
            }
        }
        Ok(())
    }

    fn finalize(&mut self) -> Result<()> {
        match self.current_box_ty {
            Some(ContainerBoxType::JPEG_RECONSTRUCTION) => {}
            Some(ty) => {
                self.current_box.finalize()?;
                let finished_box = std::mem::replace(&mut self.current_box, AuxBoxReader::new());
                self.boxes.push((ty, finished_box));
            }
            None => {
                return Ok(());
            }
        }

        self.current_box_ty = None;
        Ok(())
    }

    pub(super) fn eof(&mut self) -> Result<()> {
        self.finalize()?;
        self.last_box = true;
        Ok(())
    }
}

impl AuxBoxList {
    /// Whether a JPEG reconstruction (`jbrd`) box was seen. The fork does not parse it.
    pub fn has_jpeg_reconstruction(&self) -> bool {
        self.jbrd_seen
    }

    fn first_of_type(&self, ty: ContainerBoxType) -> AuxBoxData<&[u8]> {
        let maybe = self.boxes.iter().find(|&&(ty_to_test, _)| ty_to_test == ty);
        let data = maybe.map(|(_, b)| b.data());

        if let Some(data) = data {
            data
        } else if self.last_box && self.current_box_ty != Some(ty) {
            AuxBoxData::NotFound
        } else {
            AuxBoxData::Decoding
        }
    }

    /// Returns the first Exif metadata, if any.
    pub fn first_exif(&self) -> Result<AuxBoxData<RawExif<'_>>> {
        let exif = self.first_of_type(ContainerBoxType::EXIF);
        exif.map(RawExif::new).transpose()
    }

    /// Returns the first XML metadata, if any.
    pub fn first_xml(&self) -> AuxBoxData<&[u8]> {
        self.first_of_type(ContainerBoxType::XML)
    }
}
