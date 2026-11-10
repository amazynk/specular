//! Header, colour-mode data, image resources and the layer list.

use crate::*;

/// Bounds-checked big-endian reader.
pub(crate) struct Rd<'a> {
    pub d: &'a [u8],
    pub pos: usize,
}

impl<'a> Rd<'a> {
    pub fn new(d: &'a [u8], pos: usize) -> Self {
        Rd { d, pos }
    }
    pub fn bytes(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.pos.checked_add(n)?;
        let s = self.d.get(self.pos..end)?;
        self.pos = end;
        Some(s)
    }
    pub fn skip(&mut self, n: u64) -> Option<()> {
        let n = usize::try_from(n).ok()?;
        let end = self.pos.checked_add(n)?;
        if end > self.d.len() {
            return None;
        }
        self.pos = end;
        Some(())
    }
    pub fn u8(&mut self) -> Option<u8> {
        Some(self.bytes(1)?[0])
    }
    pub fn u16(&mut self) -> Option<u16> {
        Some(u16::from_be_bytes(self.bytes(2)?.try_into().ok()?))
    }
    pub fn i16(&mut self) -> Option<i16> {
        Some(self.u16()? as i16)
    }
    pub fn u32(&mut self) -> Option<u32> {
        Some(u32::from_be_bytes(self.bytes(4)?.try_into().ok()?))
    }
    pub fn i32(&mut self) -> Option<i32> {
        Some(self.u32()? as i32)
    }
    pub fn u64(&mut self) -> Option<u64> {
        Some(u64::from_be_bytes(self.bytes(8)?.try_into().ok()?))
    }
    /// A section length: u32, or u64 in PSB where `wide`.
    pub fn len(&mut self, wide: bool) -> Option<u64> {
        if wide {
            self.u64()
        } else {
            self.u32().map(u64::from)
        }
    }
    /// Range of the next `n` bytes, clipped to the data (for sections that may run past EOF).
    pub fn range(&mut self, n: u64) -> Range<usize> {
        let start = self.pos.min(self.d.len());
        let end = usize::try_from(n)
            .ok()
            .and_then(|n| start.checked_add(n))
            .map_or(self.d.len(), |e| e.min(self.d.len()));
        self.pos = end;
        start..end
    }
}

fn trunc(what: &str) -> Error {
    Error::Truncated(what.to_string())
}

pub(crate) fn parse(data: &[u8]) -> Result<Psd> {
    if !probe(data) {
        return Err(Error::NotPsd);
    }
    let mut r = Rd::new(data, 4);
    let version = match r.u16() {
        Some(1) => Version::Psd,
        Some(2) => Version::Psb,
        _ => return Err(Error::NotPsd),
    };
    let psb = version == Version::Psb;
    r.skip(6).ok_or_else(|| trunc("header"))?;
    let channels = r.u16().ok_or_else(|| trunc("header"))?;
    let height = r.u32().ok_or_else(|| trunc("header"))?;
    let width = r.u32().ok_or_else(|| trunc("header"))?;
    let depth = r.u16().ok_or_else(|| trunc("header"))?;
    let mode_raw = r.u16().ok_or_else(|| trunc("header"))?;
    if !(1..=56).contains(&channels) {
        return Err(Error::Malformed(format!("{channels} channels")));
    }
    if width == 0 || height == 0 {
        return Err(Error::Malformed(format!("empty image {width}×{height}")));
    }
    let max = if psb {
        MAX_DIMENSION_PSB
    } else {
        MAX_DIMENSION_PSD
    };
    if width > max || height > max || width as u64 * height as u64 > MAX_PIXELS {
        return Err(Error::TooLarge);
    }
    let mode = ColorMode::from_u16(mode_raw)
        .ok_or_else(|| Error::Unsupported(format!("colour mode {mode_raw}")))?;
    let depth = match (mode, depth) {
        (ColorMode::Bitmap, 1) => 1u8,
        (ColorMode::Bitmap, d) => return Err(Error::Malformed(format!("{d}-bit Bitmap"))),
        (ColorMode::Indexed, 8) => 8,
        (ColorMode::Indexed, d) => return Err(Error::Malformed(format!("{d}-bit Indexed"))),
        (_, d @ (8 | 16 | 32)) => d as u8,
        (_, d) => return Err(Error::Unsupported(format!("{d}-bit samples"))),
    };
    let color_channels = mode.color_channels(channels);
    if (channels as usize) < color_channels {
        return Err(Error::Malformed(format!(
            "{mode:?} with only {channels} channels"
        )));
    }

    // Colour-mode data.
    let cm_len = r.u32().ok_or_else(|| trunc("colour mode data"))?;
    let cm = r.range(cm_len as u64);
    let mut palette = None;
    if mode == ColorMode::Indexed {
        let p = &data[cm];
        if p.len() < 768 {
            return Err(Error::Malformed("indexed image without a palette".into()));
        }
        palette = Some((0..256).map(|i| [p[i], p[256 + i], p[512 + i]]).collect());
    }

    // Image resources.
    let res_len = r.u32().ok_or_else(|| trunc("image resources"))?;
    let res = r.range(res_len as u64);
    let mut resources = Resources::default();
    let mut resolution = None;
    let mut transparent_index = None;
    let mut real_merged = None;
    let mut alpha_names = false;
    parse_resources(
        data,
        res,
        &mut resources,
        &mut resolution,
        &mut transparent_index,
        &mut real_merged,
        &mut alpha_names,
    );

    // Layer and mask information.
    let mut layers = Vec::new();
    let mut layer_count_raw: i32 = 0;
    let mut have_layer_section = false;
    let mut merged_alpha = false;
    let mut patterns = Vec::new();
    if let Some(lm_len) = r.len(psb) {
        let lm = r.range(lm_len);
        have_layer_section = !lm.is_empty();
        let (list, merged, pats) = parse_layer_and_mask(data, lm, psb, depth);
        patterns = pats;
        merged_alpha = merged;
        if let Some((count, list)) = list {
            layer_count_raw = count;
            layers = list;
        }
    }

    // Image data.
    let (compression, image_data) = match r.u16() {
        Some(c) => {
            let c = match c {
                0 => Compression::Raw,
                1 => Compression::Rle,
                2 => Compression::Zip,
                3 => Compression::ZipPrediction,
                _ => return Err(Error::Unsupported(format!("compression {c}"))),
            };
            (Some(c), r.pos..data.len())
        }
        None => (None, data.len()..data.len()),
    };

    let composite = if compression.is_none() {
        Composite::Missing
    } else if real_merged == Some(false) {
        Composite::NotMaximized
    } else {
        Composite::Present
    };
    let extra = channels as usize > color_channels;
    let has_alpha = extra
        && mode != ColorMode::Bitmap
        && (layer_count_raw < 0 || merged_alpha || (!have_layer_section && !alpha_names));
    let photoshop = resources
        .writer
        .as_deref()
        .is_some_and(|w| w.contains("Photoshop"));
    let info = Info {
        version,
        width,
        height,
        channels,
        depth,
        mode,
        compression,
        has_alpha,
        matted_on_white: has_alpha && photoshop,
        matte_unknown: has_alpha && resources.writer.is_none(),
        composite,
        layer_count: layers.len().max(layer_count_raw.unsigned_abs() as usize),
        resolution,
        palette,
        transparent_index,
    };
    let mut psd = Psd {
        info,
        resources,
        layers,
        image_data,
        patterns,
    };
    // A merged image every sample of which is the same, under layers with something to
    // show, is a placeholder some writers store instead of the composite.
    if psd.info.composite == Composite::Present
        && crate::compose::possible(&psd)
        && psd
            .layers
            .iter()
            .any(|l| l.visible && (l.has_pixels() || l.fill.is_some()))
        && crate::decode::blank(&psd, data)
    {
        psd.info.composite = Composite::Blank;
    }
    // No merged image: the composite is built from the layers, on a transparent canvas.
    if psd.info.composite != Composite::Present && crate::compose::possible(&psd) {
        psd.info.has_alpha = true;
    }
    Ok(psd)
}

fn parse_resources(
    data: &[u8],
    res: Range<usize>,
    out: &mut Resources,
    resolution: &mut Option<Resolution>,
    transparent_index: &mut Option<u8>,
    real_merged: &mut Option<bool>,
    alpha_names: &mut bool,
) {
    let d = &data[..res.end];
    let mut r = Rd::new(d, res.start);
    while r.pos + 12 <= res.end {
        let Some(sig) = r.bytes(4) else { break };
        if !matches!(sig, b"8BIM" | b"MeSa" | b"AgHg" | b"PHUT" | b"DCSR") {
            break;
        }
        let Some(id) = r.u16() else { break };
        let Some(nlen) = r.u8() else { break };
        // Pascal name, padded so length byte + name is even (the length byte is consumed).
        if r.skip(((nlen as u64 + 2) & !1) - 1).is_none() {
            break;
        }
        let Some(size) = r.u32() else { break };
        let Some(_) = r.bytes(size as usize) else {
            break;
        };
        let body = r.pos - size as usize..r.pos;
        if size % 2 == 1 && r.skip(1).is_none() {
            // Padding missing at the very end is harmless.
            r.pos = res.end;
        }
        handle_resource(
            data,
            id,
            body,
            out,
            resolution,
            transparent_index,
            real_merged,
            alpha_names,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_resource(
    data: &[u8],
    id: u16,
    body: Range<usize>,
    out: &mut Resources,
    resolution: &mut Option<Resolution>,
    transparent_index: &mut Option<u8>,
    real_merged: &mut Option<bool>,
    alpha_names: &mut bool,
) {
    let b = &data[body.clone()];
    match id {
        1005 if b.len() >= 16 => {
            let fx = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f64;
            let unit = if u16::from_be_bytes([b[4], b[5]]) == 2 {
                ResolutionUnit::PerCm
            } else {
                ResolutionUnit::PerInch
            };
            let (h, v) = (fx(0) / 65536.0, fx(8) / 65536.0);
            if h > 0.0 && v > 0.0 {
                *resolution = Some(Resolution {
                    h_dpi: h,
                    v_dpi: v,
                    unit,
                });
            }
        }
        1006 | 1045 if !b.is_empty() => *alpha_names = true,
        1028 => out.iptc = Some(body),
        1033 | 1036 if b.len() >= 28 => {
            let u = |o: usize| u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
            let (fmt, w, h, row_bytes) = (u(0), u(4), u(8), u(12));
            let format = match fmt {
                1 => ThumbnailFormat::Jpeg,
                0 => ThumbnailFormat::RawRgb { row_bytes },
                _ => return,
            };
            if w == 0 || h == 0 {
                return;
            }
            if let ThumbnailFormat::RawRgb { row_bytes } = format
                && ((row_bytes as u64) < w as u64 * 3
                    || row_bytes as u64 * h as u64 > (b.len() - 28) as u64)
            {
                return;
            }
            // Prefer 1036 over 1033 if both are present.
            if id == 1033 && out.thumbnail.is_some() {
                return;
            }
            out.thumbnail = Some(Thumbnail {
                format,
                width: w,
                height: h,
                data: body.start + 28..body.end,
                bgr: id == 1033,
            });
        }
        1039 if !b.is_empty() => out.icc = Some(body),
        1047 if b.len() >= 2 => {
            let i = u16::from_be_bytes([b[0], b[1]]);
            if i < 256 {
                *transparent_index = Some(i as u8);
            }
        }
        1057 if b.len() >= 5 => {
            *real_merged = Some(b[4] != 0);
            // Writer name: Unicode string (u32 count of UTF-16 units).
            let mut r = Rd::new(b, 5);
            if let Some(s) = unicode_string(&mut r) {
                out.writer = Some(s);
            }
        }
        1037 if b.len() >= 4 => {
            out.global_angle = Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]))
        }
        1049 if b.len() >= 4 => {
            out.global_altitude = Some(i32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        }
        1058 if !b.is_empty() => out.exif = Some(body),
        1059 if !b.is_empty() => out.exif3 = Some(body),
        1060 if !b.is_empty() => out.xmp = Some(body),
        _ => {}
    }
}

fn unicode_string(r: &mut Rd) -> Option<String> {
    let n = r.u32()? as usize;
    let raw = r.bytes(n.checked_mul(2)?)?;
    let units = raw
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]));
    let s: String = char::decode_utf16(units)
        .map(|c| c.unwrap_or('\u{FFFD}'))
        .collect();
    Some(s.trim_end_matches('\0').to_string())
}

/// Keys whose additional-layer-info length is 8 bytes in PSB.
fn wide_key(key: &[u8]) -> bool {
    matches!(
        key,
        b"LMsk"
            | b"Lr16"
            | b"Lr32"
            | b"Layr"
            | b"Mt16"
            | b"Mt32"
            | b"Mtrn"
            | b"Alph"
            | b"FMsk"
            | b"lnk2"
            | b"FEid"
            | b"FXid"
            | b"PxSD"
    )
}

/// The layer and mask section: (raw layer count and records, when they parse) and whether a
/// "saving merged transparency" block (`Mtrn`, `Mt16`, `Mt32`) says the composite's first
/// extra channel is its transparency. A damaged layer list is dropped (the composite is
/// still decodable).
/// The layer records (raw count and list), merged transparency, pattern records.
type LayerSection = (Option<(i32, Vec<Layer>)>, bool, Vec<Range<usize>>);

fn parse_layer_and_mask(data: &[u8], lm: Range<usize>, psb: bool, depth: u8) -> LayerSection {
    let d = &data[..lm.end];
    let mut r = Rd::new(d, lm.start);
    let mut patterns = Vec::new();
    let Some(li_len) = r.len(psb) else {
        return (None, false, patterns);
    };
    let li = r.range(li_len);
    let mut layers = if li.is_empty() {
        None
    } else {
        parse_layer_info(d, li, psb)
    };
    let mut merged_alpha = false;
    // Global layer mask info, then tagged blocks: 16/32-bit documents keep the layer info in
    // an `Lr16` / `Lr32` block.
    let Some(gm_len) = r.u32() else {
        return (layers, false, patterns);
    };
    if r.skip(gm_len as u64).is_none() {
        return (layers, false, patterns);
    }
    while r.pos + 12 <= lm.end {
        let Some(sig) = r.bytes(4) else { break };
        if sig != b"8BIM" && sig != b"8B64" {
            break;
        }
        let Some(key) = r.bytes(4).and_then(|k| <[u8; 4]>::try_from(k).ok()) else {
            break;
        };
        let Some(len) = r.len(psb && wide_key(&key)) else {
            break;
        };
        let body = r.range(len);
        if layers.is_none()
            && ((key == *b"Lr16" && depth == 16) || (key == *b"Lr32" && depth == 32))
        {
            layers = parse_layer_info(d, body.clone(), psb);
        }
        merged_alpha |= matches!(&key, b"Mtrn" | b"Mt16" | b"Mt32");
        if matches!(&key, b"Patt" | b"Pat2" | b"Pat3") {
            crate::fill::pattern_records(d, body.clone(), &mut patterns);
        }
        // Blocks are padded to 4 bytes.
        let pad = (4 - (len % 4) as usize) % 4;
        r.pos = (r.pos + pad).min(lm.end);
    }
    (layers, merged_alpha, patterns)
}

/// Additional-layer-info keys of fill and adjustment layers (no pixels of their own).
fn adjustment_key(key: &[u8; 4]) -> bool {
    matches!(
        key,
        b"SoCo"
            | b"GdFl"
            | b"PtFl"
            | b"brit"
            | b"levl"
            | b"curv"
            | b"expA"
            | b"vibA"
            | b"hue "
            | b"hue2"
            | b"blnc"
            | b"blwh"
            | b"phfl"
            | b"mixr"
            | b"clrL"
            | b"nvrt"
            | b"post"
            | b"thrs"
            | b"grdm"
            | b"selc"
    )
}

/// Layer mask data: the user mask's bounds, default colour and flags, and the "real" user
/// mask when the block has one (a pixel mask beside a vector mask). Returns (user, real).
/// The user mask, the real user mask (beside a vector mask; `has_real`: the layer stores
/// its channel, −3) and the vector mask's density. The real mask's fields come before the
/// mask parameters (densities and feathers), as Photoshop writes them.
fn parse_mask_data(b: &[u8], has_real: bool) -> (Option<LayerMask>, Option<LayerMask>, u8) {
    fn rect(r: &mut Rd) -> Option<[i32; 4]> {
        Some([r.i32()?, r.i32()?, r.i32()?, r.i32()?])
    }
    let mk = |rc: [i32; 4], default: u8, flags: u8, channel: i16| LayerMask {
        top: rc[0],
        left: rc[1],
        bottom: rc[2],
        right: rc[3],
        default: if default >= 128 { 255 } else { 0 },
        disabled: flags & 2 != 0,
        channel,
        density: 255,
        rendered: flags & 8 != 0,
    };
    let mut r = Rd::new(b, 0);
    let Some(rc) = rect(&mut r) else {
        return (None, None, 255);
    };
    let (Some(default), Some(flags)) = (r.u8(), r.u8()) else {
        return (None, None, 255);
    };
    let mut user = mk(rc, default, flags, -2);
    let mut real = None;
    if has_real && b.len() >= 36 {
        real = (|| {
            let flags = r.u8()?;
            let default = r.u8()?;
            Some(mk(rect(&mut r)?, default, flags, -3))
        })();
    }
    let mut vector_density = 255;
    if flags & 0x10 != 0 {
        // Mask parameters: densities (1 byte) and feathers (8 bytes), as flagged.
        let _ = (|| {
            let p = r.u8()?;
            if p & 1 != 0 {
                user.density = r.u8()?;
            }
            if p & 2 != 0 {
                r.skip(8)?;
            }
            if p & 4 != 0 {
                vector_density = r.u8()?;
            }
            Some(())
        })();
    }
    if let Some(m) = real.as_mut() {
        m.density = user.density;
    }
    if user.rendered {
        // Drawn from the vector mask: it takes the vector mask's density.
        user.density = vector_density;
    }
    (Some(user), real, vector_density)
}

fn parse_layer_info(d: &[u8], li: Range<usize>, psb: bool) -> Option<(i32, Vec<Layer>)> {
    let d = &d[..li.end];
    let mut r = Rd::new(d, li.start);
    let count = r.i16()? as i32;
    let n = count.unsigned_abs() as usize;
    // Each record is at least 34 bytes: bounds the allocation by the data present.
    let mut layers = Vec::with_capacity(n.min((li.end - li.start) / 34));
    // Channel data lengths of each layer, in record order.
    let mut lengths: Vec<Vec<u64>> = Vec::with_capacity(layers.capacity());
    for _ in 0..n {
        let top = r.i32()?;
        let left = r.i32()?;
        let bottom = r.i32()?;
        let right = r.i32()?;
        let nch = r.u16()? as usize;
        // Each channel record is at least 6 bytes: bounds the allocation likewise.
        let cap = nch.min(li.end.saturating_sub(r.pos) / 6);
        let mut channels = Vec::with_capacity(cap);
        let mut lens = Vec::with_capacity(cap);
        for _ in 0..nch {
            let id = r.i16()?;
            lens.push(r.len(psb)?);
            channels.push(LayerChannel { id, data: 0..0 });
        }
        if r.bytes(4)? != b"8BIM" {
            return None;
        }
        let blend_mode: [u8; 4] = r.bytes(4)?.try_into().ok()?;
        let opacity = r.u8()?;
        let clipping = r.u8()? != 0;
        let flags = r.u8()?;
        let _filler = r.u8()?;
        let extra_len = r.u32()? as usize;
        let extra_end = r.pos.checked_add(extra_len).filter(|&e| e <= li.end)?;
        let mut e = Rd::new(&d[..extra_end], r.pos);
        r.pos = extra_end;
        // Layer mask data, blending ranges.
        let ml = e.u32()?;
        let has_real = channels.iter().any(|c| c.id == -3);
        let (user, real, vector_mask_density) = parse_mask_data(e.bytes(ml as usize)?, has_real);
        let bl = e.u32()?;
        let blend_if = blend_ranges(e.bytes(bl as usize)?);
        // Pascal name padded to a multiple of 4 (including the length byte).
        let nlen = e.u8()? as usize;
        let name_raw = e.bytes(nlen)?;
        let mut name: String = name_raw.iter().map(|&b| mac_roman(b)).collect();
        let used = 1 + nlen;
        e.skip(((4 - used % 4) % 4) as u64)?;
        let mut kind = LayerKind::Pixel;
        let mut adjustment = false;
        let mut fill_opacity = 255;
        let mut fill = None;
        let mut vector_mask = None;
        let mut blocks = Vec::new();
        let mut lsct_mode: Option<[u8; 4]> = None;
        while e.pos + 12 <= extra_end {
            let Some(sig) = e.bytes(4) else { break };
            if sig != b"8BIM" && sig != b"8B64" {
                break;
            }
            let Some(key) = e.bytes(4) else { break };
            let key: [u8; 4] = key.try_into().ok()?;
            let Some(len) = e.len(psb && wide_key(&key)) else {
                break;
            };
            let body = e.range(len);
            let b = &d[body.clone()];
            match &key {
                b"luni" => {
                    if let Some(s) = unicode_string(&mut Rd::new(b, 0)) {
                        name = s;
                    }
                }
                b"lsct" | b"lsdk" if b.len() >= 4 => {
                    kind = match u32::from_be_bytes([b[0], b[1], b[2], b[3]]) {
                        1 => LayerKind::GroupStart { open: true },
                        2 => LayerKind::GroupStart { open: false },
                        3 => LayerKind::GroupEnd,
                        _ => kind,
                    };
                    // A group's own blend mode (pass through is only said here).
                    if b.len() >= 12 && &b[4..8] == b"8BIM" {
                        lsct_mode = b[8..12].try_into().ok();
                    }
                }
                b"iOpa" if !b.is_empty() => fill_opacity = b[0],
                b"SoCo" | b"GdFl" | b"PtFl" => {
                    adjustment = true;
                    let k = match &key {
                        b"SoCo" => FillKind::Solid,
                        b"GdFl" => FillKind::Gradient,
                        _ => FillKind::Pattern,
                    };
                    fill = Some((k, body.clone()));
                }
                b"vmsk" | b"vsms" => vector_mask = Some(body.clone()),
                // A shape's fill kept beside its vector stroke: the fill's key, then its
                // settings. The layer stays a pixel layer (its stored pixels hold the shape).
                b"vscg" if b.len() > 4 && fill.is_none() => {
                    let k = match &b[..4] {
                        b"SoCo" => Some(FillKind::Solid),
                        b"GdFl" => Some(FillKind::Gradient),
                        b"PtFl" => Some(FillKind::Pattern),
                        _ => None,
                    };
                    fill = k.map(|k| (k, body.start + 4..body.end));
                }
                k if adjustment_key(k) => {
                    adjustment = true;
                    blocks.push((*k, body.clone()));
                }
                k if crate::adjust::OTHER_BLOCKS.contains(k) && blocks.len() < 64 => {
                    blocks.push((key, body.clone()));
                }
                _ => {}
            }
        }
        let blend_mode = match (kind, lsct_mode) {
            (LayerKind::GroupStart { .. }, Some(m)) => m,
            _ => blend_mode,
        };
        if adjustment && kind == LayerKind::Pixel {
            kind = LayerKind::Adjustment;
        }
        // The real user mask when its channel is there, else the user mask.
        let has = |id: i16| channels.iter().any(|c| c.id == id);
        let mask = real.filter(|_| has(-3)).or(user.filter(|_| has(-2)));
        layers.push(Layer {
            name,
            top,
            left,
            bottom,
            right,
            opacity,
            visible: flags & 2 == 0,
            blend_mode,
            kind,
            fill_opacity,
            clipping,
            channels,
            mask,
            fill,
            vector_mask,
            vector_mask_density,
            blocks,
            blend_if,
        });
        lengths.push(lens);
    }
    // Channel image data follows the records: every channel of every layer, in order.
    let end = d.len() as u64;
    let mut pos = r.pos as u64;
    for (layer, lens) in layers.iter_mut().zip(&lengths) {
        for (c, &len) in layer.channels.iter_mut().zip(lens) {
            let start = pos.min(end) as usize;
            pos = pos.saturating_add(len);
            c.data = start..pos.min(end) as usize;
        }
    }
    Some((count, layers))
}

/// Blend If ranges from a layer record's blending ranges: empty when every range lets
/// everything through (black 0..0, white 255..255).
fn blend_ranges(b: &[u8]) -> Vec<[u8; 8]> {
    let ranges: Vec<[u8; 8]> = b
        .chunks_exact(8)
        .take(1 + 16)
        .filter_map(|c| c.try_into().ok())
        .collect();
    let open = |r: &[u8; 8]| r == &[0, 0, 255, 255, 0, 0, 255, 255];
    if ranges.iter().all(open) {
        Vec::new()
    } else {
        ranges
    }
}

/// Mac OS Roman → char (legacy Pascal layer names).
fn mac_roman(b: u8) -> char {
    const HI: &str = "ÄÅÇÉÑÖÜáàâäãåçéèêëíìîïñóòôöõúùûü†°¢£§•¶ß®©™´¨≠ÆØ∞±≤≥¥µ∂∑∏π∫ªºΩæø¿¡¬√ƒ≈∆«»… ÀÃÕŒœ–—“”‘’÷◊ÿŸ⁄€‹›ﬁﬂ‡·‚„‰ÂÊÁËÈÍÎÏÌÓÔ\u{F8FF}ÒÚÛÙıˆ˜¯˘˙˚¸˝˛ˇ";
    if b < 0x80 {
        b as char
    } else {
        HI.chars().nth((b - 0x80) as usize).unwrap_or('?')
    }
}
