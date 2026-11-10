//! A streaming PDF writer for raster pages.
//!
//! Each page is one RGB image XObject filling the MediaBox, in an `ICCBased` sRGB colour
//! space (the profile from [`crate::icc`]) that is also the document's output intent. The
//! image is Flate-compressed as its rows arrive, so a page never has to be held in memory:
//! the stream's `/Length` is an indirect object written after the stream, when it is known.
//!
//! The document is PDF 1.4 with a classic cross-reference table. Objects are written in
//! the order they complete; the page tree, whose kids are only known at the end, is written
//! last under the object number reserved for it at the start.

use std::io::{self, Write};

use miniz_oxide::deflate::core::{
    CompressorOxide, TDEFLFlush, TDEFLStatus, compress, create_comp_flags_from_zip_params,
};

/// zlib compression level of image data: fast; photos gain little from more.
const LEVEL: i32 = 2;

/// Object numbers fixed by [`PdfWriter::new`].
const CATALOG: u32 = 1;
const PAGES: u32 = 2;
const ICC: u32 = 3;
const INTENT: u32 = 4;
const INFO: u32 = 5;

/// A writer that counts what it has written (the cross-reference offsets).
struct Counting<W> {
    inner: W,
    pos: u64,
}

impl<W: Write> Write for Counting<W> {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(b)?;
        self.pos += n as u64;
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// The page whose image is being written.
struct OpenPage {
    image: u32,
    length: u32,
    width: u32,
    size_pt: (f64, f64),
    rows_left: u32,
    stream_start: u64,
    deflate: Box<CompressorOxide>,
    out: Vec<u8>,
}

/// What [`PdfWriter::finish`] reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Stats {
    pub pages: usize,
    pub bytes: u64,
}

/// Document metadata.
#[derive(Clone, Debug, Default)]
pub struct Meta {
    pub title: String,
    /// `D:YYYYMMDDHHmmSSZ`, or empty for none.
    pub date: String,
    /// Print through this printer profile: rows arrive as sRGB and are written in its device
    /// values; it is the document's colour space and output intent (sRGB when `None`).
    pub output: Option<std::sync::Arc<crate::profile::PrintProfile>>,
}

/// Writes a PDF to `W` one page (and one image row band) at a time.
pub struct PdfWriter<W: Write> {
    out: Counting<W>,
    /// Byte offset of every object (index: number − 1; 0 while unwritten).
    offsets: Vec<u64>,
    pages: Vec<u32>,
    page: Option<OpenPage>,
    output: Option<std::sync::Arc<crate::profile::PrintProfile>>,
    /// Converted rows (printing through a profile).
    converted: Vec<u8>,
}

/// A PDF string literal with `(`, `)` and `\` escaped; text outside ASCII goes out as
/// UTF-16BE with a byte-order mark (a hex string).
pub fn string(s: &str) -> String {
    if s.is_ascii() {
        let mut o = String::from("(");
        for c in s.chars() {
            match c {
                '(' | ')' | '\\' => {
                    o.push('\\');
                    o.push(c);
                }
                c if (c as u32) < 0x20 || c == '\x7f' => o.push_str(&format!("\\{:03o}", c as u32)),
                c => o.push(c),
            }
        }
        o.push(')');
        o
    } else {
        let mut o = String::from("<FEFF");
        for u in s.encode_utf16() {
            o.push_str(&format!("{u:04X}"));
        }
        o.push('>');
        o
    }
}

/// A number with at most 3 decimals and no trailing zeros, as PDF wants.
fn num(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

impl<W: Write> PdfWriter<W> {
    /// Starts a document: header, colour profile, output intent and info dictionary.
    pub fn new(out: W, meta: &Meta) -> io::Result<PdfWriter<W>> {
        let mut w = PdfWriter {
            out: Counting { inner: out, pos: 0 },
            offsets: vec![0; INFO as usize],
            pages: Vec::new(),
            page: None,
            output: meta.output.clone(),
            converted: Vec::new(),
        };
        // A comment of high bytes marks the file as binary for transfer tools.
        w.out.write_all(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")?;
        let (icc, n, alternate, cond) = match &meta.output {
            Some(p) => (
                p.icc.clone(),
                p.channels,
                match p.channels {
                    1 => "/DeviceGray",
                    4 => "/DeviceCMYK",
                    _ => "/DeviceRGB",
                },
                p.name.clone(),
            ),
            None => (
                crate::icc::srgb_v2(),
                3,
                "/DeviceRGB",
                crate::icc::DESCRIPTION.to_owned(),
            ),
        };
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(&icc, 9);
        w.begin_obj(ICC)?;
        write!(
            w.out,
            "<< /N {n} /Alternate {alternate} /Filter /FlateDecode /Length {} >>\nstream\n",
            packed.len()
        )?;
        w.out.write_all(&packed)?;
        w.out.write_all(b"\nendstream\nendobj\n")?;
        let cond = string(&cond);
        w.obj(
            INTENT,
            &format!(
                "<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier {cond} \
                 /Info {cond} /DestOutputProfile {ICC} 0 R >>"
            ),
        )?;
        let mut info = format!("<< /Producer {}", string("Specular"));
        if !meta.title.is_empty() {
            info.push_str(&format!(" /Title {}", string(&meta.title)));
        }
        if !meta.date.is_empty() {
            info.push_str(&format!(" /CreationDate {}", string(&meta.date)));
        }
        info.push_str(" >>");
        w.obj(INFO, &info)?;
        w.obj(
            CATALOG,
            &format!("<< /Type /Catalog /Pages {PAGES} 0 R /OutputIntents [{INTENT} 0 R] >>"),
        )?;
        Ok(w)
    }

    fn alloc(&mut self) -> u32 {
        self.offsets.push(0);
        self.offsets.len() as u32
    }

    fn begin_obj(&mut self, id: u32) -> io::Result<()> {
        self.offsets[id as usize - 1] = self.out.pos;
        writeln!(self.out, "{id} 0 obj")
    }

    fn obj(&mut self, id: u32, body: &str) -> io::Result<()> {
        self.begin_obj(id)?;
        self.out.write_all(body.as_bytes())?;
        self.out.write_all(b"\nendobj\n")
    }

    /// Starts a page of `size_pt` (points) whose image is `width`×`height` pixels; its rows
    /// follow through [`PdfWriter::write_rows`], top to bottom.
    pub fn begin_page(&mut self, size_pt: (f64, f64), width: u32, height: u32) -> io::Result<()> {
        if self.page.is_some() {
            return Err(io::Error::other("previous page not finished"));
        }
        let (width, height) = (width.max(1), height.max(1));
        let image = self.alloc();
        let length = self.alloc();
        self.begin_obj(image)?;
        write!(
            self.out,
            "<< /Type /XObject /Subtype /Image /Width {width} /Height {height} \
             /ColorSpace [/ICCBased {ICC} 0 R] /BitsPerComponent 8 /Interpolate false \
             /Filter /FlateDecode /Length {length} 0 R >>\nstream\n"
        )?;
        let flags = create_comp_flags_from_zip_params(LEVEL, 15, 0);
        self.page = Some(OpenPage {
            image,
            length,
            width,
            size_pt,
            rows_left: height,
            stream_start: self.out.pos,
            deflate: Box::new(CompressorOxide::new(flags)),
            out: vec![0; 256 * 1024],
        });
        Ok(())
    }

    /// Runs the compressor over `input` and writes what comes out.
    fn deflate(&mut self, mut input: &[u8], flush: TDEFLFlush) -> io::Result<()> {
        let p = self.page.as_mut().expect("open page");
        loop {
            let (status, read, written) = compress(&mut p.deflate, input, &mut p.out, flush);
            self.out.write_all(&p.out[..written])?;
            input = &input[read..];
            match status {
                TDEFLStatus::Done => return Ok(()),
                TDEFLStatus::Okay => {
                    // Without flushing, the compressor returns once it took the input; when
                    // finishing it keeps going until `Done`.
                    if input.is_empty() && flush == TDEFLFlush::None {
                        return Ok(());
                    }
                }
                TDEFLStatus::BadParam | TDEFLStatus::PutBufFailed => {
                    return Err(io::Error::other("deflate failed"));
                }
            }
        }
    }

    /// Appends whole rows of tightly packed RGB8 (sRGB) to the open page's image (converted
    /// to the output profile's device values when printing through one).
    pub fn write_rows(&mut self, rgb: &[u8]) -> io::Result<()> {
        let Some(p) = self.page.as_mut() else {
            return Err(io::Error::other("no open page"));
        };
        let row = p.width as usize * 3;
        if !rgb.len().is_multiple_of(row) {
            return Err(io::Error::other("partial row"));
        }
        let rows = (rgb.len() / row) as u32;
        if rows > p.rows_left {
            return Err(io::Error::other("more rows than the image has"));
        }
        p.rows_left -= rows;
        match self.output.clone() {
            Some(profile) => {
                let mut conv = std::mem::take(&mut self.converted);
                conv.clear();
                profile.convert_row(rgb, &mut conv);
                let r = self.deflate(&conv, TDEFLFlush::None);
                self.converted = conv;
                r
            }
            None => self.deflate(rgb, TDEFLFlush::None),
        }
    }

    /// Finishes the open page (its image must be complete).
    pub fn end_page(&mut self) -> io::Result<()> {
        match &self.page {
            None => return Err(io::Error::other("no open page")),
            Some(p) if p.rows_left > 0 => {
                return Err(io::Error::other("page image incomplete"));
            }
            Some(_) => {}
        }
        self.deflate(&[], TDEFLFlush::Finish)?;
        let p = self.page.take().expect("open page");
        let len = self.out.pos - p.stream_start;
        self.out.write_all(b"\nendstream\nendobj\n")?;
        self.obj(p.length, &len.to_string())?;
        let (w, h) = p.size_pt;
        let content = format!("q {} 0 0 {} 0 0 cm /Im0 Do Q\n", num(w), num(h));
        let contents = self.alloc();
        self.obj(
            contents,
            &format!(
                "<< /Length {} >>\nstream\n{content}endstream",
                content.len()
            ),
        )?;
        let page = self.alloc();
        self.obj(
            page,
            &format!(
                "<< /Type /Page /Parent {PAGES} 0 R /MediaBox [0 0 {} {}] \
                 /Resources << /XObject << /Im0 {} 0 R >> >> /Contents {contents} 0 R >>",
                num(w),
                num(h),
                p.image
            ),
        )?;
        self.pages.push(page);
        Ok(())
    }

    /// Writes the page tree, the cross-reference table and the trailer.
    pub fn finish(mut self) -> io::Result<(W, Stats)> {
        if self.page.is_some() {
            return Err(io::Error::other("page not finished"));
        }
        let kids: Vec<String> = self.pages.iter().map(|p| format!("{p} 0 R")).collect();
        self.obj(
            PAGES,
            &format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.join(" "),
                self.pages.len()
            ),
        )?;
        let xref = self.out.pos;
        let n = self.offsets.len() + 1;
        write!(self.out, "xref\n0 {n}\n0000000000 65535 f \n")?;
        for &o in &self.offsets {
            writeln!(self.out, "{o:010} 00000 n ")?;
        }
        write!(
            self.out,
            "trailer\n<< /Size {n} /Root {CATALOG} 0 R /Info {INFO} 0 R >>\nstartxref\n{xref}\n%%EOF\n"
        )?;
        self.out.flush()?;
        let stats = Stats {
            pages: self.pages.len(),
            bytes: self.out.pos,
        };
        Ok((self.out.inner, stats))
    }
}

/// `D:YYYYMMDDHHmmSSZ` for seconds since the Unix epoch (UTC).
pub fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let secs = unix % 86_400;
    // Civil date from days (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "D:{y:04}{m:02}{d:02}{:02}{:02}{:02}Z",
        secs / 3600,
        secs / 60 % 60,
        secs % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_escaped() {
        assert_eq!(string("a(b)\\c"), "(a\\(b\\)\\\\c)");
        assert_eq!(string("é"), "<FEFF00E9>");
        assert_eq!(string("\n"), "(\\012)");
    }

    #[test]
    fn numbers_are_short() {
        assert_eq!(num(595.2756), "595.276");
        assert_eq!(num(612.0), "612");
        assert_eq!(num(-0.0001), "0");
    }

    #[test]
    fn dates() {
        assert_eq!(date(0), "D:19700101000000Z");
        assert_eq!(date(1_791_590_400), "D:20261010000000Z");
        assert_eq!(date(951_782_400 + 3661), "D:20000229010101Z");
    }

    #[test]
    fn misuse_is_an_error() {
        let mut w = PdfWriter::new(Vec::new(), &Meta::default()).unwrap();
        assert!(w.write_rows(&[0; 3]).is_err());
        w.begin_page((100.0, 100.0), 2, 2).unwrap();
        assert!(w.begin_page((100.0, 100.0), 2, 2).is_err());
        assert!(w.write_rows(&[0; 5]).is_err());
        w.write_rows(&[0; 6]).unwrap();
        assert!(w.end_page().is_err(), "one row missing");
        assert!(w.write_rows(&[0; 12]).is_err(), "one row too many");
        w.write_rows(&[0; 6]).unwrap();
        w.end_page().unwrap();
        let (bytes, stats) = w.finish().unwrap();
        assert_eq!(stats.pages, 1);
        assert_eq!(stats.bytes as usize, bytes.len());
    }
}
