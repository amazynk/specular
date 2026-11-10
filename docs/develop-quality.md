# Develop quality

How close Develop's views are to its export, and its GPU ops to their references.
CIEDE2000 throughout; 1 is about a just-noticeable difference.

## Preview against export

`crates/specular-develop/tests/quality_sweep.rs`, run on 2026-11-02 (debug build, machine under load: load average about 15–30).

- Inputs: one synthetic scene as a Bayer DNG, an X-Trans sensor, a JPEG and a 16-bit TIFF, 1800×1200.
- Each row: one module at a typical setting, or a stack, against the full-size export (f32, 16-bit sRGB).
- **1:1 tiles**: the full-size view (f16) in 256 px tiles. Bar: p99 < 0.5.
- **Fit view**: the refined view at half size (a raw demosaiced at full size and area-reduced; a non-raw given at its real size, as the app reads it up to 2560 px, and reduced by the developer), against the export area-reduced to its size. Bar: p95 < 1 for scale-stable rows. The first picture of a raw on screen is binned at half size; the refined one follows when nothing is dragged.
- **Thumbnail**: the 512 px edited thumbnail against the export reduced to its size. Bar: p95 < 1. A non-raw starts at 1024 px, or 2048 px when the edit's warp resamples it (`thumb::source_long`), as the app reads it. `SPECULAR_SWEEP_THUMB_RAW_LONG=1024` develops raw thumbnails at 1024 px first (rounded to a whole multiple of 512).
- "detail" rows change detail a reduced view cannot show (check at 100%): reported, not held to the bar.
- The scene has hard edges and 12 px stripes on purpose; smooth charts (the export and edited-thumbnail tests) do better.
- Not in the sweep: AI denoise, AI remove, depth blur and relight (they need a downloaded model), calibration frames (a frame file), borders and watermark (exports only).

Quick run (a subset, every input, held to every bar): `cargo test -p specular-develop --test quality_sweep`.
Full table: add `-- --ignored --nocapture quality_sweep_table`.

| Module or stack | Input | 1:1 tiles p50 / p95 / p99 / max | tiles | Fit view p50 / p95 / p99 / max | Thumbnail p95 / max | Scale |
|---|---|---|---|---|---|---|
| default | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.50 / 15.52 | stable |
| exposure +0.7 EV | Bayer | 0.01 / 0.03 / 0.04 / 0.10 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.56 / 17.59 | stable |
| auto exposure | Bayer | 0.01 / 0.03 / 0.05 / 0.11 | 40 | 0.07 / 0.09 / 0.11 / 12.24 | 0.43 / 13.01 | stable |
| white balance 4300 K-ish | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.06 / 12.25 | 0.48 / 14.19 | stable |
| look: filmic | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.07 / 18.64 | 0.59 / 20.90 | stable |
| look: AgX | Bayer | 0.02 / 0.03 / 0.04 / 0.08 | 40 | 0.02 / 0.04 / 0.05 / 9.53 | 0.55 / 9.94 | stable |
| crop and straighten | Bayer | 0.02 / 0.04 / 0.06 / 0.14 | 24 | 0.02 / 0.05 / 0.97 / 18.19 | 1.02 / 20.86 | stable |
| lens: distortion, TCA, vignetting | Bayer | 0.02 / 0.04 / 0.05 / 0.15 | 40 | 0.02 / 0.05 / 7.48 / 22.63 | 2.07 / 20.65 | stable |
| tone equalizer | Bayer | 0.02 / 0.04 / 0.05 / 0.11 | 40 | 0.03 / 0.40 / 0.60 / 12.30 | 0.51 / 13.96 | stable |
| local contrast: clarity | Bayer | 0.02 / 0.05 / 0.07 / 0.16 | 40 | 0.06 / 0.34 / 1.00 / 13.28 | 1.14 / 15.62 | stable |
| local contrast: texture | Bayer | 0.02 / 0.05 / 0.07 / 0.15 | 40 | 0.03 / 0.17 / 0.80 / 14.11 | 0.66 / 15.88 | detail |
| colour balance and wheels | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.07 / 10.53 | 0.51 / 16.50 | stable |
| colour mixer | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.05 / 0.14 / 10.52 | 0.53 / 17.00 | stable |
| point colour | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 16.93 | 0.50 / 16.74 | stable |
| black and white | Bayer | 0.01 / 0.03 / 0.03 / 0.07 | 40 | 0.02 / 0.03 / 0.05 / 3.79 | 0.22 / 8.04 | stable |
| curve and levels | Bayer | 0.02 / 0.03 / 0.05 / 0.10 | 40 | 0.02 / 0.04 / 0.06 / 13.19 | 0.56 / 15.98 | stable |
| LUT (.cube) | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.07 / 12.08 | 0.53 / 17.66 | stable |
| noise reduction | Bayer | 0.02 / 0.04 / 0.06 / 0.19 | 40 | 0.03 / 0.11 / 0.28 / 12.96 | 0.58 / 15.52 | detail |
| sharpening | Bayer | 0.02 / 0.05 / 0.07 / 0.17 | 40 | 0.03 / 0.11 / 0.46 / 13.47 | 1.09 / 16.66 | detail |
| capture sharpening | Bayer | 0.02 / 0.05 / 0.06 / 0.17 | 40 | 0.05 / 0.64 / 3.38 / 13.38 | 0.86 / 16.86 | detail |
| diffuse: sharpen | Bayer | 0.02 / 0.05 / 0.08 / 0.35 | 40 | 0.02 / 0.82 / 4.85 / 25.06 | 1.68 / 20.21 | detail |
| diffuse: local contrast | Bayer | 0.03 / 0.06 / 0.08 / 0.29 | 40 | 0.04 / 0.36 / 2.33 / 21.34 | 1.63 / 29.50 | stable |
| diffuse: bloom | Bayer | 0.03 / 0.08 / 0.13 / 0.54 | 40 | 0.04 / 0.52 / 1.78 / 10.33 | 0.89 / 11.79 | stable |
| diffuse: fill highlights | Bayer | 0.02 / 0.04 / 0.05 / 0.16 | 40 | 0.02 / 0.05 / 0.21 / 15.99 | 0.63 / 16.63 | stable |
| surface blur | Bayer | 0.02 / 0.04 / 0.05 / 0.13 | 40 | 0.03 / 0.06 / 0.19 / 7.37 | 1.77 / 17.19 | stable |
| hot pixels | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.50 / 15.52 | detail |
| chromatic aberration (auto) | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.07 / 12.69 | 0.50 / 15.59 | stable |
| defringe | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.51 / 17.38 | stable |
| demosaic: dual | Bayer | 0.02 / 0.03 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.50 / 15.55 | detail |
| retouch: heal, clone, blur | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.07 / 12.77 | 0.52 / 15.52 | stable |
| negative | Bayer | 0.00 / 0.02 / 0.03 / 0.08 | 40 | 0.01 / 0.04 / 0.13 / 38.13 | 1.04 / 39.07 | stable |
| dehaze | Bayer | 0.02 / 0.04 / 0.05 / 0.16 | 40 | 0.04 / 0.29 / 0.57 / 13.06 | 0.61 / 15.20 | stable |
| graduated filter | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 13.03 | 0.50 / 15.24 | stable |
| vignette | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 12.76 | 0.46 / 14.79 | stable |
| glow: bloom | Bayer | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.03 / 0.38 / 0.95 / 10.08 | 0.84 / 11.59 | stable |
| glow: soften | Bayer | 0.02 / 0.03 / 0.04 / 0.08 | 40 | 0.03 / 0.21 / 0.52 / 4.71 | 0.55 / 11.91 | stable |
| grain | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.69 / 2.26 / 3.13 / 13.40 | 1.59 / 15.72 | detail |
| masked exposure (ellipse) | Bayer | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 12.77 | 0.50 / 15.52 | stable |
| stack: portrait | Bayer | 0.05 / 0.10 / 0.14 / 0.32 | 40 | 0.08 / 0.34 / 0.68 / 14.56 | 0.98 / 16.50 | detail |
| stack: landscape | Bayer | 0.03 / 0.05 / 0.07 / 0.22 | 28 | 0.03 / 0.10 / 1.97 / 20.00 | 0.87 / 21.86 | stable |
| stack: black-and-white film | Bayer | 0.02 / 0.04 / 0.05 / 0.08 | 40 | 0.28 / 2.20 / 3.16 / 7.14 | 1.65 / 6.73 | detail |
| default | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 7.31 | 0.59 / 10.38 | stable |
| exposure +0.7 EV | X-Trans | 0.02 / 0.03 / 0.05 / 0.10 | 40 | 0.02 / 0.04 / 0.06 / 5.68 | 0.63 / 14.78 | stable |
| auto exposure | X-Trans | 0.01 / 0.03 / 0.05 / 0.12 | 40 | 0.23 / 0.26 / 0.26 / 8.32 | 0.48 / 9.51 | stable |
| white balance 4300 K-ish | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 6.49 | 0.55 / 9.29 | stable |
| look: filmic | X-Trans | 0.02 / 0.04 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.07 / 10.27 | 0.63 / 15.18 | stable |
| look: AgX | X-Trans | 0.02 / 0.03 / 0.04 / 0.08 | 40 | 0.02 / 0.03 / 0.05 / 6.89 | 0.58 / 10.34 | stable |
| crop and straighten | X-Trans | 0.02 / 0.04 / 0.06 / 0.16 | 24 | 0.02 / 0.05 / 0.84 / 9.93 | 1.01 / 20.46 | stable |
| lens: distortion, TCA, vignetting | X-Trans | 0.02 / 0.04 / 0.05 / 0.14 | 40 | 0.02 / 0.05 / 5.06 / 16.02 | 1.20 / 15.61 | stable |
| tone equalizer | X-Trans | 0.02 / 0.04 / 0.05 / 0.12 | 40 | 0.03 / 0.41 / 0.60 / 9.46 | 0.56 / 10.79 | stable |
| local contrast: clarity | X-Trans | 0.03 / 0.05 / 0.07 / 0.18 | 40 | 0.07 / 0.34 / 1.07 / 6.37 | 1.32 / 10.14 | stable |
| local contrast: texture | X-Trans | 0.03 / 0.05 / 0.07 / 0.18 | 40 | 0.03 / 0.17 / 0.87 / 6.18 | 0.67 / 9.30 | detail |
| colour balance and wheels | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 5.37 | 0.56 / 9.97 | stable |
| colour mixer | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.05 / 0.08 / 21.11 | 0.58 / 19.61 | stable |
| point colour | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 8.40 | 0.59 / 10.38 | stable |
| black and white | X-Trans | 0.01 / 0.03 / 0.03 / 0.07 | 40 | 0.02 / 0.03 / 0.04 / 6.89 | 0.22 / 7.59 | stable |
| curve and levels | X-Trans | 0.02 / 0.03 / 0.05 / 0.11 | 40 | 0.02 / 0.04 / 0.06 / 5.48 | 0.66 / 13.26 | stable |
| LUT (.cube) | X-Trans | 0.02 / 0.04 / 0.05 / 0.12 | 40 | 0.02 / 0.04 / 0.06 / 6.18 | 0.53 / 12.22 | stable |
| noise reduction | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.03 / 0.11 / 0.25 / 7.33 | 0.61 / 10.39 | detail |
| sharpening | X-Trans | 0.03 / 0.05 / 0.07 / 0.18 | 40 | 0.04 / 0.12 / 0.40 / 6.26 | 1.08 / 9.40 | detail |
| capture sharpening | X-Trans | 0.02 / 0.05 / 0.07 / 0.17 | 40 | 0.05 / 0.63 / 3.38 / 6.12 | 0.89 / 8.76 | detail |
| diffuse: sharpen | X-Trans | 0.02 / 0.05 / 0.10 / 0.43 | 40 | 0.03 / 0.78 / 3.94 / 15.37 | 1.50 / 11.02 | detail |
| diffuse: local contrast | X-Trans | 0.03 / 0.06 / 0.09 / 0.29 | 40 | 0.04 / 0.32 / 2.28 / 17.64 | 1.49 / 29.58 | stable |
| diffuse: bloom | X-Trans | 0.03 / 0.09 / 0.15 / 0.64 | 40 | 0.04 / 0.52 / 1.76 / 4.05 | 0.91 / 8.60 | stable |
| diffuse: fill highlights | X-Trans | 0.02 / 0.04 / 0.06 / 0.18 | 40 | 0.02 / 0.05 / 0.10 / 7.31 | 0.61 / 15.90 | stable |
| surface blur | X-Trans | 0.03 / 0.05 / 0.07 / 0.15 | 40 | 0.03 / 0.07 / 0.23 / 5.38 | 1.60 / 14.59 | stable |
| hot pixels | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 7.31 | 0.59 / 10.38 | detail |
| chromatic aberration (auto) | X-Trans | 0.02 / 0.04 / 0.05 / 0.13 | 40 | 0.02 / 0.05 / 0.13 / 4.95 | 0.58 / 10.68 | stable |
| defringe | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 7.31 | 0.59 / 11.61 | stable |
| demosaic: dual | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 7.31 | 0.59 / 10.38 | detail |
| retouch: heal, clone, blur | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.08 / 7.39 | 0.61 / 17.98 | stable |
| negative | X-Trans | 0.01 / 0.02 / 0.03 / 0.08 | 40 | 0.01 / 0.07 / 0.14 / 27.01 | 1.06 / 26.17 | stable |
| dehaze | X-Trans | 0.02 / 0.04 / 0.06 / 0.14 | 40 | 0.04 / 0.35 / 0.90 / 8.11 | 0.69 / 8.33 | stable |
| graduated filter | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 9.47 | 0.58 / 10.38 | stable |
| vignette | X-Trans | 0.02 / 0.04 / 0.05 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 8.20 | 0.48 / 9.35 | stable |
| glow: bloom | X-Trans | 0.02 / 0.04 / 0.05 / 0.11 | 40 | 0.03 / 0.38 / 0.94 / 6.37 | 0.84 / 9.08 | stable |
| glow: soften | X-Trans | 0.02 / 0.03 / 0.04 / 0.08 | 40 | 0.04 / 0.24 / 0.48 / 6.36 | 0.59 / 11.14 | stable |
| grain | X-Trans | 0.02 / 0.04 / 0.05 / 0.12 | 40 | 0.77 / 2.40 / 3.27 / 7.34 | 1.63 / 10.34 | detail |
| masked exposure (ellipse) | X-Trans | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.02 / 0.04 / 0.07 / 7.31 | 0.59 / 10.38 | stable |
| stack: portrait | X-Trans | 0.05 / 0.10 / 0.14 / 0.33 | 40 | 0.08 / 0.33 / 0.66 / 10.08 | 0.97 / 10.25 | detail |
| stack: landscape | X-Trans | 0.03 / 0.05 / 0.08 / 0.23 | 28 | 0.03 / 0.11 / 1.86 / 13.98 | 0.91 / 20.89 | stable |
| stack: black-and-white film | X-Trans | 0.02 / 0.04 / 0.04 / 0.07 | 40 | 0.28 / 2.30 / 3.34 / 7.16 | 1.76 / 8.41 | detail |
| default | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 2.38 | 0.52 / 5.89 | stable |
| exposure +0.7 EV | JPEG | 0.01 / 0.03 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 5.14 | 0.55 / 5.75 | stable |
| auto exposure | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.10 / 0.12 / 0.13 / 1.48 | 0.46 / 5.69 | stable |
| white balance 4300 K-ish | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 3.26 | 0.49 / 6.56 | stable |
| look: filmic | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.06 / 3.44 | 0.53 / 7.38 | stable |
| look: AgX | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.03 / 0.04 / 4.44 | 0.56 / 4.38 | stable |
| crop and straighten | JPEG | 0.01 / 0.02 / 0.03 / 0.06 | 24 | 0.02 / 0.05 / 0.46 / 7.23 | 0.51 / 9.41 | stable |
| lens: distortion, TCA, vignetting | JPEG | 0.01 / 0.03 / 0.03 / 0.07 | 40 | 0.02 / 0.05 / 4.73 / 13.19 | 0.68 / 7.64 | stable |
| tone equalizer | JPEG | 0.02 / 0.04 / 0.05 / 0.11 | 40 | 0.03 / 0.12 / 0.49 / 2.22 | 0.51 / 6.04 | stable |
| local contrast: clarity | JPEG | 0.03 / 0.05 / 0.06 / 0.14 | 40 | 0.09 / 0.41 / 0.96 / 3.77 | 0.47 / 5.69 | stable |
| local contrast: texture | JPEG | 0.03 / 0.05 / 0.07 / 0.13 | 40 | 0.04 / 0.56 / 0.82 / 4.62 | 0.69 / 5.73 | detail |
| colour balance and wheels | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 3.99 | 0.47 / 6.14 | stable |
| colour mixer | JPEG | 0.01 / 0.02 / 0.03 / 0.04 | 40 | 0.02 / 0.04 / 0.05 / 13.95 | 0.49 / 9.73 | stable |
| point colour | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 2.38 | 0.52 / 5.89 | stable |
| black and white | JPEG | 0.01 / 0.01 / 0.02 / 0.03 | 40 | 0.01 / 0.03 / 0.03 / 6.94 | 0.21 / 4.44 | stable |
| curve and levels | JPEG | 0.01 / 0.03 / 0.05 / 0.06 | 40 | 0.02 / 0.04 / 0.05 / 4.40 | 0.54 / 5.69 | stable |
| LUT (.cube) | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 2.64 | 0.55 / 5.09 | stable |
| noise reduction | JPEG | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.04 / 0.11 / 0.20 / 2.39 | 0.56 / 5.99 | detail |
| sharpening | JPEG | 0.03 / 0.05 / 0.07 / 0.14 | 40 | 0.06 / 0.35 / 3.66 / 10.19 | 1.25 / 7.11 | detail |
| capture sharpening | JPEG | 0.02 / 0.04 / 0.05 / 0.11 | 40 | 0.08 / 0.48 / 5.50 / 11.02 | 0.81 / 6.55 | detail |
| diffuse: sharpen | JPEG | 0.02 / 0.05 / 0.08 / 0.22 | 40 | 0.02 / 0.82 / 3.20 / 8.90 | 0.96 / 5.86 | detail |
| diffuse: local contrast | JPEG | 0.03 / 0.06 / 0.08 / 0.35 | 40 | 0.04 / 0.35 / 1.34 / 11.63 | 0.59 / 6.18 | stable |
| diffuse: bloom | JPEG | 0.04 / 0.09 / 0.17 / 0.71 | 40 | 0.04 / 0.55 / 1.67 / 2.33 | 0.58 / 5.87 | stable |
| diffuse: fill highlights | JPEG | 0.02 / 0.04 / 0.05 / 0.15 | 40 | 0.02 / 0.04 / 0.06 / 2.38 | 0.49 / 5.89 | stable |
| surface blur | JPEG | 0.03 / 0.04 / 0.06 / 0.12 | 40 | 0.03 / 0.06 / 0.21 / 2.53 | 0.92 / 4.88 | stable |
| defringe | JPEG | 0.02 / 0.04 / 0.05 / 0.08 | 40 | 0.02 / 0.04 / 0.05 / 3.40 | 0.52 / 5.89 | stable |
| retouch: heal, clone, blur | JPEG | 0.02 / 0.04 / 0.05 / 0.08 | 40 | 0.02 / 0.04 / 0.06 / 6.92 | 0.52 / 13.42 | stable |
| negative | JPEG | 0.01 / 0.02 / 0.02 / 0.03 | 40 | 0.01 / 0.03 / 0.05 / 13.82 | 0.78 / 9.45 | stable |
| dehaze | JPEG | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.05 / 0.31 / 0.89 / 3.34 | 0.49 / 7.57 | stable |
| graduated filter | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.44 | 0.53 / 6.49 | stable |
| vignette | JPEG | 0.01 / 0.02 / 0.03 / 0.06 | 40 | 0.02 / 0.04 / 0.05 / 1.69 | 0.45 / 5.89 | stable |
| glow: bloom | JPEG | 0.02 / 0.04 / 0.05 / 0.09 | 40 | 0.03 / 0.32 / 1.00 / 2.43 | 0.50 / 5.32 | stable |
| glow: soften | JPEG | 0.02 / 0.03 / 0.04 / 0.08 | 40 | 0.03 / 0.17 / 0.38 / 3.58 | 0.52 / 4.63 | stable |
| grain | JPEG | 0.01 / 0.02 / 0.03 / 0.07 | 40 | 1.04 / 3.13 / 4.18 / 7.58 | 1.75 / 6.26 | detail |
| masked exposure (ellipse) | JPEG | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 2.38 | 0.52 / 5.68 | stable |
| stack: portrait | JPEG | 0.05 / 0.09 / 0.13 / 0.28 | 40 | 0.07 / 0.42 / 2.97 / 4.91 | 0.93 / 6.60 | detail |
| stack: landscape | JPEG | 0.02 / 0.04 / 0.06 / 0.13 | 28 | 0.04 / 0.14 / 1.46 / 12.10 | 0.45 / 11.59 | stable |
| stack: black-and-white film | JPEG | 0.01 / 0.02 / 0.03 / 0.07 | 40 | 0.51 / 2.35 / 3.36 / 7.87 | 1.34 / 4.26 | detail |
| default | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.66 | 0.51 / 5.81 | stable |
| exposure +0.7 EV | TIFF 16 | 0.01 / 0.03 / 0.04 / 0.06 | 40 | 0.02 / 0.04 / 0.05 / 3.64 | 0.56 / 5.67 | stable |
| auto exposure | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.11 / 0.13 / 0.13 / 1.34 | 0.44 / 5.85 | stable |
| white balance 4300 K-ish | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 2.37 | 0.57 / 6.58 | stable |
| look: filmic | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.06 | 40 | 0.02 / 0.04 / 0.06 / 3.36 | 0.52 / 7.50 | stable |
| look: AgX | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.07 | 40 | 0.02 / 0.03 / 0.05 / 4.16 | 0.56 / 4.40 | stable |
| crop and straighten | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.06 | 24 | 0.02 / 0.04 / 0.44 / 7.06 | 0.51 / 9.42 | stable |
| lens: distortion, TCA, vignetting | TIFF 16 | 0.01 / 0.03 / 0.03 / 0.07 | 40 | 0.02 / 0.05 / 4.68 / 13.16 | 0.67 / 7.73 | stable |
| tone equalizer | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.03 / 0.12 / 0.48 / 1.71 | 0.50 / 5.96 | stable |
| local contrast: clarity | TIFF 16 | 0.03 / 0.05 / 0.06 / 0.15 | 40 | 0.09 / 0.41 / 0.97 / 3.08 | 0.50 / 5.65 | stable |
| local contrast: texture | TIFF 16 | 0.03 / 0.05 / 0.06 / 0.15 | 40 | 0.03 / 0.56 / 0.83 / 4.16 | 0.69 / 5.67 | detail |
| colour balance and wheels | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 3.14 | 0.45 / 6.05 | stable |
| colour mixer | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 13.81 | 0.51 / 9.70 | stable |
| point colour | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.66 | 0.51 / 5.81 | stable |
| black and white | TIFF 16 | 0.01 / 0.01 / 0.02 / 0.05 | 40 | 0.01 / 0.03 / 0.03 / 6.65 | 0.22 / 4.42 | stable |
| curve and levels | TIFF 16 | 0.01 / 0.03 / 0.04 / 0.07 | 40 | 0.02 / 0.04 / 0.05 / 3.59 | 0.55 / 5.70 | stable |
| LUT (.cube) | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.88 | 0.51 / 5.01 | stable |
| noise reduction | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.11 | 40 | 0.03 / 0.06 / 0.09 / 1.63 | 0.53 / 6.17 | detail |
| sharpening | TIFF 16 | 0.03 / 0.05 / 0.06 / 0.13 | 40 | 0.05 / 0.32 / 3.63 / 10.11 | 1.23 / 7.09 | detail |
| capture sharpening | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.06 / 0.43 / 5.38 / 11.12 | 0.81 / 6.63 | detail |
| diffuse: sharpen | TIFF 16 | 0.02 / 0.04 / 0.07 / 0.25 | 40 | 0.02 / 0.64 / 2.87 / 8.27 | 0.84 / 5.60 | detail |
| diffuse: local contrast | TIFF 16 | 0.03 / 0.06 / 0.08 / 0.30 | 40 | 0.04 / 0.20 / 1.16 / 6.71 | 0.59 / 5.95 | stable |
| diffuse: bloom | TIFF 16 | 0.04 / 0.09 / 0.16 / 0.71 | 40 | 0.04 / 0.55 / 1.65 / 2.07 | 0.58 / 6.08 | stable |
| diffuse: fill highlights | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.13 | 40 | 0.02 / 0.04 / 0.05 / 1.66 | 0.52 / 5.81 | stable |
| surface blur | TIFF 16 | 0.03 / 0.04 / 0.06 / 0.11 | 40 | 0.03 / 0.05 / 0.21 / 2.37 | 0.93 / 4.79 | stable |
| defringe | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.02 / 0.04 / 0.05 / 3.34 | 0.52 / 5.81 | stable |
| retouch: heal, clone, blur | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.02 / 0.04 / 0.06 / 6.72 | 0.51 / 13.63 | stable |
| negative | TIFF 16 | 0.01 / 0.02 / 0.02 / 0.04 | 40 | 0.01 / 0.03 / 0.04 / 12.27 | 0.77 / 9.27 | stable |
| dehaze | TIFF 16 | 0.02 / 0.04 / 0.06 / 0.13 | 40 | 0.05 / 0.31 / 0.87 / 3.35 | 0.49 / 7.61 | stable |
| graduated filter | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.36 | 0.51 / 6.50 | stable |
| vignette | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.06 | 40 | 0.02 / 0.04 / 0.05 / 1.29 | 0.48 / 5.81 | stable |
| glow: bloom | TIFF 16 | 0.02 / 0.04 / 0.05 / 0.10 | 40 | 0.03 / 0.31 / 0.99 / 1.66 | 0.53 / 5.39 | stable |
| glow: soften | TIFF 16 | 0.02 / 0.03 / 0.04 / 0.09 | 40 | 0.03 / 0.17 / 0.38 / 2.68 | 0.52 / 4.62 | stable |
| grain | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.07 | 40 | 1.04 / 3.13 / 4.18 / 7.57 | 1.76 / 6.40 | detail |
| masked exposure (ellipse) | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.05 | 40 | 0.02 / 0.04 / 0.05 / 1.66 | 0.50 / 5.78 | stable |
| stack: portrait | TIFF 16 | 0.05 / 0.09 / 0.13 / 0.29 | 40 | 0.07 / 0.41 / 2.96 / 4.69 | 0.91 / 6.57 | detail |
| stack: landscape | TIFF 16 | 0.02 / 0.04 / 0.06 / 0.14 | 28 | 0.04 / 0.13 / 1.44 / 11.57 | 0.48 / 11.48 | stable |
| stack: black-and-white film | TIFF 16 | 0.01 / 0.02 / 0.03 / 0.07 | 40 | 0.52 / 2.35 / 3.36 / 7.84 | 1.33 / 4.26 | detail |

### Over the bar

Every 1:1 row is under its bar (p99 ≤ 0.17), and every scale-stable fit view (p95 ≤ 0.55). Over it:

- **Raw thumbnails at 512 px** (12 rows, p95 1.01–2.07): crop and straighten, lens, clarity, diffuse local contrast, surface blur, negative; Bayer and X-Trans. A 512 px raw thumbnail mixes the chart's 12 px stripes before the tone curve and runs neighbourhood ops at 0.28 scale; the export mixes after the look. Developed at 1024 px and reduced, only Bayer lens stays over (1.42), at about the 150 ms budget (table below). Raws develop at 1024 px by default (`thumb::RAW_LONG`); Settings › Gallery › Sharper raw thumbnails off gives 512.
- **Bayer lens thumbnail from 1024 px** (1.42): vignetting brightens the stripes into the tone curve's shoulder before they are mixed.
- Detail rows (noise reduction, sharpening, capture sharpening, texture, grain, hot pixels, dual demosaic, diffuse sharpen, the portrait and film stacks): over by design; a reduced view cannot show what they change. The panel says to check at 100 % only for diffuse.

## GPU against reference

`crates/specular-develop/src/ops/tests_quality.rs`: each module alone on one photo-like scene, f32 on the GPU against its CPU reference (same arithmetic, f64 where it matters). "Change made" shows the setting did something. Bar: max < 0.5.

| Module | GPU vs CPU ΔE2000 p50 / p95 / p99 / max | Change made, max |
|---|---|---|
| Exposure | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 11.14 |
| Auto exposure | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 9.66 |
| White balance (CAT16, gamut compression) | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 10.92 |
| Look: sigmoid | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 26.46 |
| Look: filmic | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 25.48 |
| Look: AgX | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 30.27 |
| Tone equalizer | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 15.40 |
| Local contrast | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 4.49 |
| Curve and levels | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 5.56 |
| Colour balance and wheels | 0.0000 / 0.0001 / 0.0001 / 0.0003 | 10.39 |
| Colour mixer | 0.0000 / 0.0001 / 0.0001 / 0.0004 | 5.78 |
| Point colour | 0.0000 / 0.0001 / 0.0001 / 0.0004 | 16.38 |
| Black and white | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 24.43 |
| LUT (.cube, tetrahedral) | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 24.36 |
| Vignette | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 15.69 |
| Graduated filter | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 29.12 |
| Grain | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 1.07 |
| Negative | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 89.34 |
| Glow: bloom | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 25.05 |
| Glow: soften | 0.0000 / 0.0000 / 0.0001 / 0.0026 | 47.37 |
| Dehaze | 0.0000 / 0.0002 / 0.0004 / 0.0016 | 18.94 |
| Noise reduction | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 1.37 |
| Sharpening | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 25.70 |
| Capture sharpening | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 10.30 |
| Diffuse: Sharpen | 0.0000 / 0.0000 / 0.0000 / 0.0002 | 39.99 |
| Diffuse: LocalContrast | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 21.48 |
| Diffuse: Bloom | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 51.58 |
| Diffuse: InpaintHighlights | 0.0000 / 0.0000 / 0.0000 / 0.0001 | 30.79 |
| Surface blur | 0.0000 / 0.0000 / 0.0000 / 0.0000 | 30.28 |

Covered by their own tests (float tolerances, not ΔE): raw front end against `ops/raw/cpu_ref.rs` (and rawpy in `tools/quality`), retouch against `ops/retouch/cpu_ref.rs`, depth blur and relight against `ops/depth/cpu.rs`, CAT16, OkLab and lens models against colour-science and camera JPEGs (`tests/*_oracle.rs`).

## darktable-cli oracle

`tools/quality/darktable-oracle.sh RAW_DIR OUT_DIR` renders both at defaults and prints ΔE per raw. It needs
darktable-cli, so it is a developer-machine check and not part of these tables.

## Raw thumbnail size

A second area reduction equals one from the sensor only by a whole factor. Reduced from 768 px to 512 (1.5×),
every other thumbnail pixel takes part of a developed pixel that also covers sensor pixels outside it, so the
two reductions blur more than the export's one. A raw is therefore developed at the nearest whole multiple of
the thumbnail (768 rounds to 1024), at exactly twice its width and height (1024 × 682, not 1024 × 683, for a
512 × 341 thumbnail): `Developer::size_multiple`, `preview::fit_multiple`, and the unit test
`two_reductions_equal_one_only_by_a_whole_factor`. Non-raw thumbnails are developed at a whole multiple of 512
too (at most 1024), and an edit whose warp resamples (lens, chromatic aberration, straighten) reads the image at
2048 px (`thumb::source_long`).

Raw thumbnails developed larger, then reduced (p95, scale-stable raw rows over 1 in any column):

| Module or stack | Input | 512 | from 768 (1.5×) | from 768 or 1024 (2×) |
|---|---|---|---|---|
| default | Bayer | 0.50 | 1.02 | 0.44 |
| look: filmic | Bayer | 0.59 | 1.58 | 0.47 |
| crop and straighten | Bayer | 1.02 | 0.52 | 0.41 |
| lens: distortion, TCA, vignetting | Bayer | 2.07 | 2.55 | 1.42 |
| local contrast: clarity | Bayer | 1.14 | 1.45 | 0.44 |
| curve and levels | Bayer | 0.56 | 1.24 | 0.47 |
| diffuse: local contrast | Bayer | 1.63 | 1.45 | 0.74 |
| surface blur | Bayer | 1.77 | 1.63 | 0.89 |
| negative | Bayer | 1.04 | 1.30 | 0.73 |
| stack: landscape | Bayer | 0.87 | 0.52 | 0.40 |
| crop and straighten | X-Trans | 1.01 | 0.57 | 0.48 |
| lens: distortion, TCA, vignetting | X-Trans | 1.20 | 1.70 | 0.96 |
| local contrast: clarity | X-Trans | 1.32 | 1.18 | 0.47 |
| diffuse: local contrast | X-Trans | 1.49 | 1.29 | 0.71 |
| surface blur | X-Trans | 1.60 | 1.59 | 0.79 |
| negative | X-Trans | 1.06 | 1.25 | 0.81 |
| Rows over 1 (of 62) | | 12 | 33 | 1 |

- Every scale-stable raw row is better from 1024 than at 512 (default Bayer 0.50 → 0.44, X-Trans 0.59 → 0.51).
- Cost (`thumbnail/512` bench, 24 MP, best of 3 rounds, p50 / p95 ms, two alternating runs each, load 29–34):
  512 px Bayer 112 / 113 and 123 / 128, X-Trans 93 / 113 and 96 / 100; 1024 px Bayer 154 / 166 and 149 / 151,
  X-Trans 135 / 137 and 116 / 123. Budget 150.
- Raw thumbnails are developed at 1024 px by default; Settings › Gallery › Sharper raw thumbnails off gives 512.

## Known limitations

- DNG profile tables (HueSatMap/LookTable) are not exact in per-plane warps (exact for matrix profiles); list-3
  point opcodes other than FixVignetteRadial run before the warp.
- Calibration frames must match the crop and aren't scaled for exposure.
- Match to reference ignores gamut compression and black offset.
- Merged DNG: BaselineExposure only for float sensors, colour matrix from as-shot white without a camera table
  entry, no homography.
- AI remove's fill is made at preview size and clips above the 8-bit look: the network works in 8 bits.
- Non-raw HDR sources and EXR metadata are not developed as HDR; presets are file-only; the command line reads
  sidecars only.
- The sign of Lightroom's crop angle is assumed, not checked against a real Lightroom sidecar.
- Translations of Develop are machine-drafted and need review by native speakers.
