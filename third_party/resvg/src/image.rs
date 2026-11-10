// Copyright 2018 the Resvg Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

pub fn render(
    image: &usvg::Image,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    if !image.is_visible() {
        return;
    }

    render_inner(image.kind(), transform, image.rendering_mode(), pixmap);
}

pub fn render_inner(
    image_kind: &usvg::ImageKind,
    transform: tiny_skia::Transform,
    rendering_mode: usvg::ImageRendering,
    pixmap: &mut tiny_skia::PixmapMut,
) {
    match image_kind {
        usvg::ImageKind::SVG(tree) => {
            render_vector(tree, transform, pixmap);
        }
        // Specular: raster images are decoded by the embedding application.
        _ => {
            render_raster(image_kind, transform, rendering_mode, pixmap);
        }
    }
}

fn render_vector(
    tree: &usvg::Tree,
    transform: tiny_skia::Transform,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let mut sub_pixmap = tiny_skia::Pixmap::new(pixmap.width(), pixmap.height()).unwrap();
    crate::render(tree, transform, &mut sub_pixmap.as_mut());
    pixmap.draw_pixmap(
        0,
        0,
        sub_pixmap.as_ref(),
        &tiny_skia::PixmapPaint::default(),
        tiny_skia::Transform::default(),
        None,
    );

    Some(())
}

/// Specular: decodes the raster data of an `<image>` (whatever the application's href
/// resolver put in a raster [`usvg::ImageKind`]) to a premultiplied pixmap. Called on every
/// render of the image, so it should cache; `None` skips the image.
pub type RasterDecoder = fn(&usvg::ImageKind) -> Option<std::sync::Arc<tiny_skia::Pixmap>>;

static RASTER_DECODER: std::sync::OnceLock<RasterDecoder> = std::sync::OnceLock::new();

/// Specular: installs the process-wide raster image decoder (the first call wins). Without
/// one, raster images are skipped.
pub fn set_raster_decoder(decoder: RasterDecoder) {
    let _ = RASTER_DECODER.set(decoder);
}

/// Specular: upstream's `raster_images::render_raster` with the decoding replaced by the
/// installed [`RasterDecoder`].
fn render_raster(
    image: &usvg::ImageKind,
    transform: tiny_skia::Transform,
    rendering_mode: usvg::ImageRendering,
    pixmap: &mut tiny_skia::PixmapMut,
) -> Option<()> {
    let Some(decode) = RASTER_DECODER.get() else {
        log::debug!("No raster image decoder installed. Image skipped.");
        return None;
    };
    let raster = decode(image)?;

    let rect = tiny_skia::Size::from_wh(raster.width() as f32, raster.height() as f32)?
        .to_rect(0.0, 0.0)?;

    let quality = match rendering_mode {
        usvg::ImageRendering::OptimizeQuality => tiny_skia::FilterQuality::Bicubic,
        usvg::ImageRendering::OptimizeSpeed => tiny_skia::FilterQuality::Nearest,
        usvg::ImageRendering::Smooth => tiny_skia::FilterQuality::Bilinear,
        usvg::ImageRendering::HighQuality => tiny_skia::FilterQuality::Bicubic,
        usvg::ImageRendering::CrispEdges => tiny_skia::FilterQuality::Nearest,
        usvg::ImageRendering::Pixelated => tiny_skia::FilterQuality::Nearest,
    };

    let pattern = tiny_skia::Pattern::new(
        raster.as_ref().as_ref(),
        tiny_skia::SpreadMode::Pad,
        quality,
        1.0,
        tiny_skia::Transform::default(),
    );
    let mut paint = tiny_skia::Paint::default();
    paint.shader = pattern;

    pixmap.fill_rect(rect, &paint, transform, None);

    Some(())
}
