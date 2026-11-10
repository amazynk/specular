# Specular tools

Developer tools: nothing here is shipped. The sections below cover the JPEG test corpus, the
benchmarks and the checks that CI runs; the other folders have their own notes.

| Path | What's there |
|---|---|
| `specular-corpus/` | Generates the JPEG test corpus into `tools/corpus/out/`, with a `manifest.json` (below) |
| `specular-bench/` | The decode benchmark, the headline cases of the speed gate, and an opt-in viewer benchmark (below) |
| `bench/` | `perf-gate.py`, the instruction-count speed gate CI runs on every push, and measurement scripts |
| `corpus/` | `fetch-*.sh` scripts for public test files (downloaded, never committed) and the oracles that check decoders against reference implementations |
| `quality/` | Colour and image-quality oracles: ICC conformance, RAW against rawpy and darktable, HDR, DICOM display ([quality/README.md](quality/README.md)) |
| `fuzz/` | `run.sh` runs the `cargo-fuzz` targets of a format crate |
| `a11y/` | Automated screen-reader and input-method checks on Linux ([docs/accessibility.md](../docs/accessibility.md)) |
| `realuse/` | Real-use runs: the real app driven through user workflows in a private headless Wayland session |
| `ci/` | Helpers CI calls: the licence and advisory check (`deny.sh`), the no-network check, oracle pass rates |
| `release/` | PGO builds, benchmark comparison and `third-party.sh`, which writes `packaging/THIRD-PARTY.md` |
| `screenshots/` | `make.py` renders the README screenshots and the Linux store screenshots from the real app |
| `cross-check.sh` | Checks the Windows and macOS targets from Linux (below) |
| `gen-adobe-coeff.py` | Generates the camera RAW colour table (below) |

`tools/corpus/out/` is regenerated, never committed. `tools/corpus/real/` is for photos of your own
to benchmark with: it is gitignored, and the tools treat it as read-only.

## Requirements

- `cjpeg` / `djpeg` from libjpeg-turbo (2.1+). These provide the encoder, the arithmetic/progressive/restart options and the reference decoder.
- `magick` (ImageMagick 7) for the YCCK file.
- `python3` with Pillow, for the plain CMYK file.
- `exiftool` for EXIF orientation, GPS, the IFD1 thumbnail and ICC profiles.
- ICC profiles from `/usr/share/color/icc/` (Debian/Ubuntu package `icc-profiles-free`).
  These are the CC0 Compact-ICC-Profiles: sRGB v2/v4, DisplayP3, AdobeCompat (Adobe RGB 1998), and ProPhoto.
- `fincore` (util-linux) for the `--cold` residency check.
- `curl` for the public-domain photos. Pass `--no-network` to skip them.
- For the viewer benchmark only: an X display. Under GNOME that is Xwayland on `:0`.
  The viewers you want to compare (`feh`, `sxiv`, `eog`, `loupe`) must also be installed.

## Generating the corpus

```sh
cargo run --release -p specular-corpus              # small tier (CI): ~130 MB, ~8 s on 16 cores
cargo run --release -p specular-corpus -- --full    # everything: ~700 MB, ~75 s on 16 cores
cargo run --release -p specular-corpus -- --full --flip-set   # + 1000 x 12 MP files (~6 GB)
```

Other options:

- `--only SUBSTR` regenerates a subset, along with the base files it derives from.
- `--force` overwrites files that already exist. Without it, existing files are kept, so re-runs are cheap.
- `--dry-run` prints the plan.
- `--out DIR` writes somewhere else. The tool checks free disk space first and keeps 2 GiB in reserve.

The content is procedural and photo-like: sky gradient and clouds, fractal terrain with texture,
buildings with window grids, signs with small text, sensor noise and vignetting. Each image is a pure function of
`(width, height, seed)`. It is rendered in parallel bands and streamed straight into `cjpeg` as PPM, so the
200 MP and 650 MP files never exist uncompressed. At q92 4:2:0 this gives about 3.2 bpp, which matches
real camera JPEGs. Fully random noise would compress unrealistically badly, and flat gradients
unrealistically well. Camera files also carry a real-camera-like EXIF block (Make/Model/exposure) and a
160 px IFD1 thumbnail. JFIF is stripped from them, as cameras do.

### Contents (`--full`)

| Directory | What |
|---|---|
| `cam/` | 24 MP 4:2:0 / 4:2:2 / 4:4:4, 45 MP, 50 MP progressive, 61 MP, 100 MP without and with restart markers (RST every MCU row), 200 MP 16384×12288, 12 MP progressive / RST |
| `pano/` | 65000×10000 (650 MP) panorama |
| `variant/` | greyscale, non-transformed RGB, YCCK (ImageMagick), plain CMYK (Pillow), 12-bit SOF1 (own encoder, as cjpeg is 8-bit only), arithmetic sequential and progressive, q75, q100 4:4:4 |
| `orientation/` | EXIF orientations 1–8. Each chart shows an "F", coloured corners and the expected-result caption. Checked with `magick -auto-orient` |
| `icc/` | the same pixels tagged sRGB v2, sRGB v4, Display P3, Adobe RGB, ProPhoto |
| `gps/` | Lisbon, Tokyo, Buenos Aires, Point Nemo (mid-ocean), and EXIF without GPS |
| `broken/` | truncated at 10/50/90 % (baseline and progressive), bad SOI, missing DHT, a marker in the scan, early EOI, wrong RST sequence, 65535×65535 header, zero height, random garbage, empty |
| `real/` | three NASA public-domain photos from Wikimedia Commons. URL, page, credit and licence are in the manifest |
| `flip/` | (`--flip-set`) 1000 × 4000×3000 files for flip-through tests |

Your own photos in `tools/corpus/real/*.jpg` appear in the manifest with category `real` and tier `real-owner`.
Their paths point to `../real/<name>`, and the files are never copied or modified.

### manifest.json

Each entry records the properties actually parsed from the written file, not what the encoder was asked
for:

- `path`, `category`, `tier`, `file_size`
- `width`, `height`, `megapixels`, `bits_per_pixel`
- `sof`, `precision`, `sampling`, `progressive`, `arithmetic`
- `restart_interval_mcus`, `colour_model`
- `exif_orientation`, `has_gps` / `gps`, `has_exif_thumbnail`, `icc_profile`
- `expected_error`, `expected_note`, `purpose`
- for downloaded photos: `source_url`, `source_page`, `licence`, `credit`

Tests should load the manifest rather than hard-coding paths.

## Decode benchmark

```sh
cargo run --release -p specular-bench -- --quick          # CI: small-tier files, 3 runs, no RSS pass
cargo run --release -p specular-bench                     # all files, 7 runs, + peak RSS + robustness
cargo run --release -p specular-bench --features specular  # include specular-jpeg (see below)
cargo run --release -p specular-bench -- --cold --only 100mp,goats
```

On a shared machine, add `--max-load L`. Before each file it waits, for up to 15 min, until the 1-min load
average drops below L. The load at the time of each measurement is recorded in the JSON, and the range is printed in the
Markdown. Concurrent `rustc` jobs can slow the reference decoders down 5×.

Decoders and modes. The "view" modes decode at the smallest power-of-two DCT scale that still covers a fit to a
3840×2160 viewport (`--viewport WxH`). The `1/N` column of the output table shows that scale.

| Name | What |
|---|---|
| `djpeg-full` / `djpeg-view` | libjpeg-turbo `djpeg -dct int [-scale 1/N] -outfile /dev/null`. Process-spawn overhead, measured on a 16×16 file, is subtracted |
| `zune-full` | zune-jpeg 0.5. It has no DCT scaling and is single-threaded |
| `jpegdec-full` / `jpegdec-view` | jpeg-decoder 0.3 (rayon), with `scale()` for the view |
| `specular-full[-1t]`, `specular-view[-1t]` | specular-jpeg with automatic and single-threaded parallelism (needs `--features specular`) |

Each timing covers reading the file and decoding it.

- Warm runs (the default) report the median of `--runs` (7) after `--warmup` (1).
- `--cold` drops the file from the page cache before every run, using `fdatasync` + `posix_fadvise(DONTNEED)`, and then checks residency with util-linux `fincore`.
- Peak RSS comes from one extra decode in a child process (`--decode-one`). In-process decoders report their own `VmHWM`;
  for djpeg it is the child's `getrusage(RUSAGE_CHILDREN)`. While each child runs, the harness also polls `/proc/<pid>/status`,
  so a child that crashes or times out still has a peak. That poll is what shows the multi-GB allocations on the 65535×65535 file.
- The tools contain no `unsafe`. Syscalls go through `rustix` (fadvise, fdatasync, statvfs, kill) and `nix` (getrusage).
- The robustness pass runs every decoder on each `expected_error` file in a child process with a 30 s timeout.
  The outcome is decoded / graceful error / crash (signal) / timeout, plus peak RSS.

Output goes to `<corpus>/bench/decode-<timestamp>.{md,json}` and to `decode-latest.*`. The JSON also records the CPU,
thread count, load average and djpeg version.

With specular enabled, the table has a "specular fastest?" column. Any file where specular-jpeg loses to a competitor, in either
the full or the view category, is flagged in bold and listed below the table.

**specular-jpeg rows.** `--features specular` adds a path dependency on `crates/formats/specular-jpeg`
(`src/specular_glue.rs`). "view" uses specular's own `choose_scale(info, 3840, 2160)`, and the decode runs to **planes**.
specular leaves upsampling and YCbCr→RGB to the GPU shader. The other decoders produce interleaved RGB, so specular
does less CPU work by design. That is the fair comparison for a GPU viewer, but it is not like-for-like with djpeg's RGB output.
Building with the feature also enables jpeg-decoder's `rayon` feature for specular-jpeg's fallback path, through Cargo feature
unification.

## Instruction-count gate (`tools/bench/perf-gate.py`)

The timing benchmark above needs a quiet, fixed machine to be a reliable gate. This gate counts
instructions instead, which are exact on any machine, so it runs on GitHub's hosted runners (CI job
`perf-gate`). It fails when any case needs more than 2 % more instructions than the committed
baseline, `tools/bench/perf-gate-baseline.json`.

```sh
./build.sh bench-gate                       # build, count, compare (same as tools/bench/perf-gate.py)
./build.sh bench-gate --only jpeg,heic      # case-name substrings
./build.sh bench-gate --bless               # accept: the counts become the baseline (commit it)
tools/bench/perf-gate.py --bless-from perf-gate-results.json   # accept CI's counts (job artifact)
tools/bench/perf-gate.py --repeat 3         # run every case 3 times and print the spread
```

It needs valgrind (`apt install valgrind`) and Python 3.10+. Each case is one open + decode with the
decoder's single-threaded option, in its own process (`specular-bench gate run CASE FILE`, built with
`--features gate` in the release profile). Callgrind counts only the function `perfgate_measure`
(`--toggle-collect`), so start-up, reading the file and exit are not counted. Results go to
`<target>/perfgate/results.json`; the callgrind files, for `callgrind_annotate`, are in
`<target>/perfgate/callgrind/`.

Two counts are kept per case. `ir` is every instruction in the decode, including glibc (malloc,
memcpy, libm). `ir_own` is only the instructions in the specular-bench binary, i.e. all Rust code. `ir_own`
depends only on the source, Cargo.lock and the pinned toolchain, so it is the same on every machine.
`ir` also depends on the distribution's glibc (up to ~20 % of the count for AVIF and JPEG XL). The gate
compares `ir` when the baseline was blessed with the same glibc, valgrind and rustc versions, and
`ir_own` otherwise, and says which one it used. To gate on `ir` in CI, bless from CI's
`perf-gate-results` artifact. A case whose input bytes changed (its SHA-256 is in the baseline)
fails until it is blessed again. A toolchain bump changes every count, so bless in the same commit.

Run to run, the counts vary by at most a few dozen instructions (< 0.0001 %). The cases:

| Case | Input | Decode |
|---|---|---|
| `jpeg-420-3mp` | 2048×1536 q90 4:2:0 baseline (generated) | to planes |
| `jpeg-420-3mp-quarter` | the same file | DCT-scaled 1/4 |
| `jpeg-444-progressive` | 1024×768 q95 4:4:4 progressive (generated) | to planes |
| `png-rgb8-photo` | 1024×768 RGB photo, adaptive filters (generated) | 8-bit |
| `png-rgba8-graphic` | 1024×768 RGBA flat-colour graphic (generated) | 8-bit |
| `webp-lossy` | 1024×768 `cwebp -q 80` (committed) | YCbCr planes |
| `webp-lossless` | the graphic, `cwebp -lossless -z 6` (committed) | RGBA |
| `avif-420` | 1024×768 8-bit 4:2:0 (aom via heif-enc, committed) | rav1d, 1 thread |
| `heic-grid` | `crates/specular-decode/data/heic-calibration.heic`, 2048×1024 grid | HEVC software core |
| `jxl-vardct` | `crates/formats/specular-jxl/tests/data/vardct_multigroup.jxl`, 520×300 | full |
| `jxl-modular` | `crates/formats/specular-jxl/tests/data/modular.jxl`, 66×44 | full |
| `gif-still` | 512×384 quantised photo (generated) | first frame |
| `gif-anim` | 256×192, 16 frames (generated) | all frames composited |
| `tiff-lzw`, `tiff-deflate` | 1024×768 RGB, horizontal predictor (generated) | 8-bit |
| `raw-dng-lj92` | 2048×1536 14-bit RGGB DNG, LJ92 tiles (generated) | full develop |

"Generated" inputs are written by `specular-bench gate gen DIR` from integer-only procedural images
with pure-Rust encoders pinned in Cargo.lock (jpeg-encoder, png, gif, tiff; the DNG writer of
`crates/formats/specular-raw/tests/common`), so they are byte-identical everywhere and nothing large is
committed. WebP and AVIF have no such encoder; their inputs are committed in `tools/bench/gate/` and
`tools/bench/gate/make-inputs.sh` records how they were made. To add a case, add it to `CASES` and
`perfgate_measure` in `tools/specular-bench/src/gate.rs` and bless. The whole run takes about 30 s
of CPU after the build.

## Viewer benchmark (opt-in)

```sh
cargo run --release -p specular-bench -- --viewers [--viewer-runs 3] [--viewer-list feh,sxiv,eog,loupe,specular] [--only 200mp]
```

Each viewer is started with `DISPLAY` (default `:0`), `GDK_BACKEND=x11` and `WINIT_UNIX_BACKEND=x11`, and with `WAYLAND_DISPLAY`
removed, so that every window is an X11 window on Xwayland. The harness then:

1. Finds the new window in `_NET_CLIENT_LIST`, preferring the one whose `_NET_WM_PID` matches. This needs an EWMH window manager; mutter is one.
2. Polls a centred 320×160 region with core-protocol `GetImage` every ~5 ms.
3. Records **first pixels**: the first frame in which more than 20 % of the region differs from its most common colour, i.e. no longer a flat background.
4. Records **sharp**: the first frame identical to the final frame, once that frame has stayed unchanged for 1.5 s.
5. Kills the viewer with SIGTERM, then SIGKILL after 2 s, before the next run.

feh is run with `-B black -g 1920x1080 --scale-down`, sxiv with `-g 1920x1080`. eog (`-n`, a new instance) and loupe have
no geometry option, so their window is resized to 1920×1080 with `ConfigureWindow` as soon as it appears.
loupe is skipped if an instance is already running, because GApplication would hand the file to the user's window.

Specular is measured through its own `specular --size WxH --bench-open <file>` (same window size
as the other viewers; without `--size` the app uses 1280×800). The app prints a pretty-printed
JSON report; the bench takes `first_pixels_ms`/`sharp_ms` for the table and stores the whole
report (sharp scale, per-level events, RSS, GPU) in the `report` field of `viewers-<stamp>.json`.
The binary is found via `SPECULAR_BIN`, else `target/release/specular`, else
`$CARGO_TARGET_DIR/release/specular`. Its times are self-reported from process start, the
others are measured from spawn by X11 capture, so the specular column is not perfectly
like-for-like (it excludes exec/loader time, a few ms, and the compositor's final frame).

Limitations:

- Polling resolution is ~5–10 ms.
- GetImage reads the X server's copy of the window, not the composited screen. Under Xwayland that is the client's
  last committed buffer, so it misses the final compositor frame (about one refresh, ~7–16 ms).
- New windows take focus. Mutter's focus-stealing prevention does not stop a newly launched app from getting focus,
  so do not type while the benchmark runs.
- Start-up time is part of the measurement: process exec, toolkit init and window mapping.
  The `window` column shows how much of the total that accounts for.

## Cross-target checks (`cross-check.sh`)

`tools/cross-check.sh` type-checks and lints the workspace for Windows and macOS from Linux, so
platform-specific code (`#[cfg(windows)]`, `#[cfg(target_os = "macos")]`) can be checked before
pushing:

```sh
rustup target add x86_64-pc-windows-msvc aarch64-apple-darwin   # once
tools/cross-check.sh                    # cargo check --all-targets for both targets
tools/cross-check.sh clippy             # cargo clippy --all-targets -- -D warnings
tools/cross-check.sh both x86_64-pc-windows-msvc -- -p specular -p specular-sandbox
```

Some build scripts compile C/C++: libmimalloc-sys (the viewer's allocator) and unrar_sys (the
decode host). They need a C compiler and archiver for the target even for `cargo check`. The
script sets `CC_<target>`, `CXX_<target>` and `AR_<target>` to stand-ins that only create the
empty object files and archives the build scripts expect. Nothing is linked, so Rust errors and
lints are the same as with a real toolchain. C compile errors are not seen, and nothing can be
run or tested.

The default target dir is `target/cross-check`, so stub-built C objects never mix with real ones.
Set `CARGO_TARGET_DIR` and `CARGO_BUILD_JOBS` as usual for cargo. This is for local use only:
CI builds, lints and tests natively on Linux, Windows and macOS (`.github/workflows/ci.yml`).

## Dependency check (`ci/deny.sh`)

`tools/ci/deny.sh` runs cargo-deny (licences, advisories, duplicates, sources) with the policy
in `deny.toml`. It never changes `Cargo.lock`.

```sh
tools/ci/deny.sh --install --fetch      # first run: cargo-deny into target/tools, advisory snapshot
tools/ci/deny.sh                        # offline, on the snapshot in target/advisory-dbs
tools/ci/deny.sh licenses bans          # some sections only
```

Duplicates only warn. Every allowed licence and ignored advisory in `deny.toml` has its reason.


## Camera RAW colour data (`gen-adobe-coeff.py`)

`crates/formats/specular-raw/src/adobe_coeff.rs` is generated, never edited by hand. It holds
dcraw's `adobe_coeff()` table (Adobe's ColorMatrix values; Dave Coffin's own `/* DJC */`
measurements are left out) and full DNG colour profiles for newer bodies. The profiles come
from DNGs listed in `tools/dng-colour-sources.tsv`: camera-written DNGs from raw.pixls.us,
pinned by SHA-256, and DNGs made with Adobe DNG Converter, pinned by a digest of their colour
tags because the converter does not write the same file twice. To regenerate:

```sh
curl -o DIR/dcraw.c http://www.dechifro.org/dcraw/dcraw.c   # revision 1.478 (dcraw 9.28)
tools/corpus/fetch-conformance.sh large_raw large_raw_adobe # the DNGs and source raws
tools/corpus/adobe-dng.sh                                   # converter-made DNGs
python3 -I tools/gen-adobe-coeff.py DIR/dcraw.c \
    --dng-sources tools/dng-colour-sources.tsv --dng-root tools/corpus/large/raw \
    > crates/formats/specular-raw/src/adobe_coeff.rs
```

`adobe-dng.sh` needs Adobe DNG Converter 18.4 for Windows under Wine 10 and xvfb, in a private
`WINEPREFIX` under `target/adobe-dng` (the setup commands are in the script's header). Later
converter versions do not run under Wine 10. The check also requires the converter version
named in the manifest, so a converter update is a deliberate manifest edit. Without `dcraw.c`,
`gen-adobe-coeff.py` only checks the manifest against the DNGs.
