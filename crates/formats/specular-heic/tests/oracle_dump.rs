//! Writes the display conversion of HEIF files for an outside comparison (libheif through
//! pillow-heif, tools/corpus/heic-oracle.py): for each file in `SPECULAR_HEIC_DUMP`
//! (`:`-separated), `<name>.new.rgba` (the planes through `specular_core::yuv::to_rgb`,
//! chroma interpolated), `<name>.old.rgba` (the replicating `Image::to_rgba`) and
//! `<name>.size`, 8-bit RGBA, into `SPECULAR_HEIC_DUMP_OUT`. Ignored; run with `--ignored`.

use specular_core::{PixelLayout, hdr::HdrEncoding, yuv};
use specular_heic::{ConvertOptions, DecodeOptions, HeicFile};

#[test]
#[ignore = "writes files for tools/corpus/heic-oracle.py"]
fn oracle_dump() {
    let (Ok(files), Ok(out)) = (
        std::env::var("SPECULAR_HEIC_DUMP"),
        std::env::var("SPECULAR_HEIC_DUMP_OUT"),
    ) else {
        return;
    };
    for path in files.split(':').filter(|p| !p.is_empty()) {
        let name = std::path::Path::new(path)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .into_owned();
        let bytes = std::fs::read(path).unwrap();
        let f = HeicFile::open(bytes).unwrap();
        let img = f.decode(&DecodeOptions::default()).unwrap();
        let old = img.to_rgba(&ConvertOptions::default()).unwrap();
        let new = match img
            .to_yuv(&ConvertOptions::default(), HdrEncoding::SRGB)
            .unwrap()
        {
            Some(planes) => {
                let r = yuv::to_rgb(&planes).unwrap();
                let p = &r.planes[0];
                match r.layout {
                    PixelLayout::Rgba => p.data.clone(),
                    _ => p
                        .data
                        .chunks_exact(2)
                        .map(|b| ((u32::from(u16::from_ne_bytes([b[0], b[1]])) + 128) / 257) as u8)
                        .collect(),
                }
            }
            None => old.data.clone(),
        };
        let dir = std::path::Path::new(&out);
        std::fs::write(dir.join(format!("{name}.new.rgba")), &new).unwrap();
        std::fs::write(dir.join(format!("{name}.old.rgba")), &old.data).unwrap();
        std::fs::write(
            dir.join(format!("{name}.size")),
            format!("{} {}", old.width, old.height),
        )
        .unwrap();
    }
}
