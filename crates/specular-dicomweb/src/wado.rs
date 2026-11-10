//! WADO-RS retrieval (PS3.18 10.4): instances, frames, bulk data, metadata and rendered
//! previews.

use std::cell::RefCell;
use std::ops::ControlFlow;

use specular_remote::{CancelToken, Head, Progress, Request};

use crate::json::{JsonDataSet, parse_array};
use crate::multipart::{MAX_PART, Multipart, MultipartError, Part, media_type};
use crate::{DicomWeb, Error, Result, accept, uid};

const JSON: &str = "application/dicom+json, application/json;q=0.9";

/// The largest series metadata (DICOM JSON) read.
const MAX_METADATA: u64 = 256 << 20;

/// Appends `chunk` to a single-part body of at most `max` bytes: a server streaming without
/// end must not fill memory.
fn push_single(
    body: &mut Vec<u8>,
    chunk: &[u8],
    max: usize,
) -> std::result::Result<(), MultipartError> {
    if body.len().saturating_add(chunk.len()) > max {
        return Err(MultipartError::PartTooLarge);
    }
    body.extend_from_slice(chunk);
    Ok(())
}

impl DicomWeb {
    /// `GET …/series/{series}/metadata`: the series' instances as DICOM JSON, plus the raw
    /// body (the progressive loader caches it for offline use).
    pub fn series_metadata(
        &self,
        study: &str,
        series: &str,
        cancel: &CancelToken,
    ) -> Result<(Vec<JsonDataSet>, Vec<u8>)> {
        let url = self.url(
            &["studies", uid(study)?, "series", uid(series)?, "metadata"],
            &[],
        );
        let res = self.get(&self.request(url).accept(JSON).limit(MAX_METADATA), cancel)?;
        let sets = parse_array(&res.body)?;
        Ok((sets, res.body))
    }

    /// Streams a response, handing each body part to `on_part` as soon as it is complete (a
    /// non-multipart body is one part). `on_part` returning `Break` stops the download.
    pub fn stream_parts(
        &self,
        req: &Request,
        cancel: &CancelToken,
        mut on_part: impl FnMut(Part) -> ControlFlow<()>,
        mut on_bytes: impl FnMut(&Progress),
    ) -> Result<Head> {
        enum Mode {
            Multi(Multipart),
            Single(Part),
        }
        let mode: RefCell<Option<Mode>> = RefCell::new(None);
        let failed: RefCell<Option<Error>> = RefCell::new(None);
        let mut stopped = false;
        let head = self.client().get_stream_head(
            req,
            cancel,
            |h| {
                let ct = h
                    .headers
                    .content_type()
                    .unwrap_or("application/octet-stream");
                *mode.borrow_mut() = Some(if media_type(ct).0.starts_with("multipart/") {
                    match Multipart::new(ct) {
                        Ok(m) => Mode::Multi(m),
                        Err(e) => {
                            *failed.borrow_mut() = Some(e.into());
                            return ControlFlow::Break(());
                        }
                    }
                } else {
                    Mode::Single(Part {
                        headers: vec![("content-type".into(), ct.to_string())],
                        // The server's length is only a hint: the body grows as it arrives.
                        body: Vec::with_capacity(
                            h.headers.content_length().unwrap_or(0).min(16 << 20) as usize,
                        ),
                    })
                });
                ControlFlow::Continue(())
            },
            |chunk, p| {
                on_bytes(p);
                match mode.borrow_mut().as_mut() {
                    Some(Mode::Multi(m)) => {
                        let r = m.feed(chunk, |part| {
                            if !stopped && on_part(part).is_break() {
                                stopped = true;
                            }
                        });
                        if let Err(e) = r {
                            *failed.borrow_mut() = Some(e.into());
                            return ControlFlow::Break(());
                        }
                        if stopped || m.is_done() {
                            return ControlFlow::Break(());
                        }
                    }
                    Some(Mode::Single(s)) => {
                        if let Err(e) = push_single(&mut s.body, chunk, MAX_PART) {
                            *failed.borrow_mut() = Some(e.into());
                            return ControlFlow::Break(());
                        }
                    }
                    None => {}
                }
                ControlFlow::Continue(())
            },
        )?;
        if let Some(e) = failed.into_inner() {
            return Err(e);
        }
        match mode.into_inner() {
            Some(Mode::Multi(mut m)) if !stopped => m.finish()?,
            Some(Mode::Single(s)) => {
                let _ = on_part(s);
            }
            _ => {}
        }
        Ok(head)
    }

    /// Sends `req` with the explicit transfer-syntax list, and once more with
    /// `transfer-syntax=*` if the server answers 406 Not Acceptable.
    fn negotiate(
        &self,
        url: &str,
        accept_for: fn(bool) -> String,
        cancel: &CancelToken,
        on_part: &mut dyn FnMut(Part) -> ControlFlow<()>,
        on_bytes: &mut dyn FnMut(&Progress),
    ) -> Result<Head> {
        let req = self.request(url.to_string()).accept(accept_for(false));
        match self.stream_parts(&req, cancel, &mut *on_part, &mut *on_bytes) {
            Err(e) if e.status() == Some(406) => {
                let req = self.request(url.to_string()).accept(accept_for(true));
                self.stream_parts(&req, cancel, on_part, on_bytes)
            }
            other => other,
        }
    }

    /// `GET …/instances/{sop}`: the instance as a Part 10 file (the first DICOM part).
    pub fn instance(
        &self,
        study: &str,
        series: &str,
        sop: &str,
        cancel: &CancelToken,
        mut on_bytes: impl FnMut(&Progress),
    ) -> Result<Part> {
        let url = self.url(
            &[
                "studies",
                uid(study)?,
                "series",
                uid(series)?,
                "instances",
                uid(sop)?,
            ],
            &[],
        );
        let mut found = None;
        self.negotiate(
            &url,
            accept::instances,
            cancel,
            &mut |p| {
                if is_dicom(&p) {
                    found = Some(p);
                    ControlFlow::Break(())
                } else {
                    ControlFlow::Continue(())
                }
            },
            &mut on_bytes,
        )?;
        found.ok_or(Error::NoDicom)
    }

    /// `GET …/series/{series}`: every instance in one multipart response, each handed over as
    /// soon as it has arrived.
    pub fn series_instances(
        &self,
        study: &str,
        series: &str,
        cancel: &CancelToken,
        mut on_instance: impl FnMut(Part) -> ControlFlow<()>,
    ) -> Result<()> {
        let url = self.url(&["studies", uid(study)?, "series", uid(series)?], &[]);
        self.negotiate(
            &url,
            accept::instances,
            cancel,
            &mut |p| {
                if is_dicom(&p) {
                    on_instance(p)
                } else {
                    ControlFlow::Continue(())
                }
            },
            &mut |_| {},
        )?;
        Ok(())
    }

    /// `GET …/instances/{sop}/frames/{list}`: pixel data of frames (1-based), one part each.
    pub fn frames(
        &self,
        study: &str,
        series: &str,
        sop: &str,
        frames: &[u32],
        cancel: &CancelToken,
    ) -> Result<Vec<Part>> {
        let list = frames
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let url = self.url(
            &[
                "studies",
                uid(study)?,
                "series",
                uid(series)?,
                "instances",
                uid(sop)?,
                "frames",
                &list,
            ],
            &[],
        );
        let mut out = Vec::new();
        self.negotiate(
            &url,
            accept::frames,
            cancel,
            &mut |p| {
                out.push(p);
                ControlFlow::Continue(())
            },
            &mut |_| {},
        )?;
        Ok(out)
    }

    /// Bulk data from a metadata `BulkDataURI` (absolute, or relative to the service root).
    /// Only URIs on this server are fetched.
    pub fn bulk_data(&self, uri: &str, cancel: &CancelToken) -> Result<Part> {
        let url = if uri.contains("://") {
            if !same_origin(uri, self.base()) {
                return Err(Error::Http(specular_remote::Error::Protocol(
                    "bulk data on another server".into(),
                )));
            }
            uri.to_string()
        } else {
            format!("{}/{}", self.base(), uri.trim_start_matches('/'))
        };
        let mut found = None;
        self.negotiate(
            &url,
            accept::frames,
            cancel,
            &mut |p| {
                found = Some(p);
                ControlFlow::Break(())
            },
            &mut |_| {},
        )?;
        found.ok_or(Error::NoDicom)
    }

    /// `GET …/instances/{sop}/rendered` (or `…/frames/{n}/rendered`): a consumer image (JPEG or
    /// PNG) for a quick preview. Returns (media type, bytes).
    pub fn rendered(
        &self,
        study: &str,
        series: &str,
        sop: &str,
        frame: Option<u32>,
        viewport: Option<(u32, u32)>,
        cancel: &CancelToken,
    ) -> Result<(String, Vec<u8>)> {
        let frame_s = frame.map(|f| f.to_string());
        let mut segs = vec![
            "studies",
            uid(study)?,
            "series",
            uid(series)?,
            "instances",
            uid(sop)?,
        ];
        if let Some(f) = &frame_s {
            segs.extend(["frames", f.as_str()]);
        }
        segs.push("rendered");
        let q: Vec<(&str, String)> = viewport
            .map(|(w, h)| vec![("viewport", format!("{w},{h}"))])
            .unwrap_or_default();
        let req = self
            .request(self.url(&segs, &q))
            .accept("image/jpeg, image/png;q=0.9")
            .limit(64 << 20);
        let res = self.get(&req, cancel)?;
        let ct = res
            .headers
            .content_type()
            .map_or_else(|| "image/jpeg".to_string(), |c| media_type(c).0);
        Ok((ct, res.body))
    }
}

fn is_dicom(p: &Part) -> bool {
    let ct = p.content_type();
    ct == "application/dicom"
        || (ct == "application/octet-stream" && specular_dicom::sniff(&p.body))
}

fn origin(url: &str) -> Option<&str> {
    let (scheme, rest) = url.split_once("://")?;
    let end = rest.find('/').unwrap_or(rest.len());
    Some(&url[..scheme.len() + 3 + end])
}

fn same_origin(a: &str, b: &str) -> bool {
    match (origin(a), origin(b)) {
        (Some(x), Some(y)) => x.eq_ignore_ascii_case(y),
        _ => false,
    }
}

/// Parses a buffered response body into parts by its Content-Type.
pub fn parts_of(content_type: Option<&str>, body: Vec<u8>) -> Result<Vec<Part>> {
    let ct = content_type.unwrap_or("application/octet-stream");
    if media_type(ct).0.starts_with("multipart/") {
        let mut m = Multipart::new(ct)?;
        let mut out = Vec::new();
        m.feed(&body, |p| out.push(p))?;
        m.finish()?;
        Ok(out)
    } else {
        Ok(vec![Part {
            headers: vec![("content-type".into(), ct.to_string())],
            body,
        }])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A single-part body stops growing at its limit (a server streaming without end).
    #[test]
    fn single_part_body_is_bounded() {
        let mut body = Vec::new();
        for _ in 0..4 {
            push_single(&mut body, &[0; 1000], 4096).unwrap();
        }
        assert!(matches!(
            push_single(&mut body, &[0; 1000], 4096),
            Err(MultipartError::PartTooLarge)
        ));
        assert_eq!(body.len(), 4000);
    }
    #[test]
    fn origins() {
        assert!(super::same_origin(
            "https://a:1/x/y",
            "https://A:1/dicom-web"
        ));
        assert!(!super::same_origin("https://evil/x", "https://a/dicom-web"));
    }
}
