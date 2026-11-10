# Specular — Design Document

*The design of Specular: goals, architecture, formats and the rules the code follows.*

Specular is a fast, GPU-accelerated, **read-only** viewer for Windows, macOS and Linux, written in Rust.
It opens photos and general images, medical images (DICOM, CT, MRI, research volumes), engineering and
architecture drawings and models, and a long tail of other formats. It looks and feels like IrfanView,
adds a library, and is free and open source (MIT OR Apache-2.0).

---

## Contents

1. [Goals and principles](#1-goals-and-principles)
2. [UI: our own toolkit on wgpu](#2-ui-our-own-toolkit-on-wgpu)
3. [Visual design and quality](#3-visual-design-and-quality)
4. [Architecture](#4-architecture)
5. [Component choices](#5-component-choices)
6. [Formats: photo and general](#6-formats-photo-and-general)
7. [Medical imaging](#7-medical-imaging)
8. [Engineering and architecture](#8-engineering-and-architecture)
9. [Long-tail formats](#9-long-tail-formats)
10. [Library](#10-library)
11. [Interaction and keyboard](#11-interaction-and-keyboard)
12. [Performance and rendering pipeline](#12-performance-and-rendering-pipeline)
13. [Security and sandboxing](#13-security-and-sandboxing)
14. [Licensing](#14-licensing)
15. [Milestones](#15-milestones)

---

## 1. Goals and principles

Specular is **one engine serving four domains**: photo/general, medical, engineering/architecture, and a
long tail. All domains share the window, input system, renderer, cache and library.

### Positioning

The world does not need another image viewer. Most people are content with the one their OS ships, and
they rarely switch. What is missing is a **fast, private tool for getting through large amounts of
photos**: one that opens everything instantly, on every OS, and helps decide what to keep. Specular
leads with that. The four domains above remain the long-term scope. The photo viewer and culler is
where it starts, and what it must be best at first.

**Who it is for, first**

1. **Photographers who shoot a lot.** Weddings, events, sport, wildlife, travel: thousands of frames
   per shoot, many of them near-identical. Today they choose between:
   - fast but shallow tools (Photo Mechanic, FastRawViewer);
   - deep but slow tools (Lightroom, Capture One, darktable);
   - AI culling services that work in the cloud or by subscription (Aftershoot, Narrative Select, Excire).
2. **People with years of phone and camera photos.** Large libraries full of duplicates, bursts and
   near-identical shots, spread across HEIC, JPEG, RAW and screenshots, with no good local way to clean
   them up.
3. **People on more than one OS, or on Linux.** The best viewers belong to one platform: FastStone and
   IrfanView run only on Windows, Preview only on macOS. Cross-platform viewers (XnView MP, nomacs,
   gThumb, Geeqie) are capable, but feel dated or slow.
4. **Anyone who opens odd files.** HEIC, AVIF, JPEG XL, many RAW variants, EXR and HDR gain maps open
   differently, or not at all, depending on the OS and its installed codecs.

**What we lead with**

| Promise | What the user notices | Where it is designed |
|---|---|---|
| Instant | A 50 MP RAW or HEIC is sharp before the key is released; a 100k-image folder shows its grid at once | §12, §10 Gallery |
| Every format, the same everywhere | The same file looks the same, with correct colour and HDR, on Windows, macOS and Linux, with nothing extra to install | §6, §6.1, §12 |
| Cull and clean up | Ratings and flags, compare, RAW+JPEG pairs, and similar shots with a suggested best pick and an explanation | §6.3, §10 Duplicates and similar shots |
| Private | Search, grouping and recognition run on the device; nothing is uploaded; face detection can be turned off and its data deleted | §6.6, §10 Local AI pack |
| Never touches your files unasked | Decisions live in the catalogue or in XMP sidecars that other photo tools read; files change only when you rename, move or turn them, and that can be undone | §1 Non-goals |

**What we do not lead with.** The library, AI search, slideshows, the medical and engineering domains
and the long-tail formats are depth that makes people stay. They are not why people try Specular, and
none of them ships before the five promises above hold up on all three platforms.

**How we know it works**

- **Speed people can feel.** Side-by-side comparisons with the tools listed above, on the same files
  and machine, published with the method (§12, §15 M1 exit criteria). Benchmarks alone do not win
  users; the first open of a large RAW or a full card does.
- **A real culling session.** A 2,000-frame shoot culled to its keepers in less time than with Photo
  Mechanic, without missing a frame the photographer would have kept.
- **A real cleanup.** On a library of 10,000+ phone photos, the similar-shots review finds the reclaimable
  space and finishes with no wrong suggestion on a photo the user rated, flagged or edited.

**What would make it fail**

- **Scope.** Building the whole of §6–§10 before the culling workflow is excellent. The promises above
  come first, and each milestone has to make them better.
- **Feeling instead of numbers.** Being measurably fastest but not feeling faster: animation, input
  latency and the first frame matter as much as decode throughput.
- **Trust.** One wrong suggestion on a photo the user cared about, or one crash on an odd file, costs
  more than any feature gains. Suggestions never act on their own, decoders are fuzzed and sandboxed
  (§13), and user decisions always win.

### Goals

- **The fastest image viewer available.** Open any supported file and show pixels in **under 100 ms**;
  moving to the next image feels instant. Use the newest CPU and GPU capabilities on each device (§12).
- Format support in this order: all major image formats, then medical formats **in depth**, then
  engineering and architecture formats, then everything else.
- **Look and feel like IrfanView**: one window, minimal chrome. A **library mode** sits alongside it.
- **Beautiful, sharp and high-end**: pixel-perfect image rendering, fluid motion, refined chrome (§3).
- Serve **novices and power users** with one keymap in two layers.
- Serve **photo, medical, engineering and science users from one app** without crowding any of them:
  **user profiles** (§11) show each person the tools of the domains they work in, while every format
  still opens for everyone.
- Run on Windows 10+, macOS 12+, Linux (X11 and Wayland).

### Non-goals

- **No destructive pixel editing**, and no saving over images. Specular may *export the current view*
  as a new file, and may store ratings, tags, measurements and annotations in its database or in
  sidecars (XMP, DICOM GSPS/SR), but **never writes into the source file**.
  - *Develop* (§6.10) is non-destructive photo editing within this rule: edits live in
    the library and in a `.specular.xmp` sidecar, the source is never written, and edited pixels
    leave only through Export, as new files.
  - *File operations the user asks for* are the one exception: rename, batch rename,
    copy and move to a folder, and **turning the file itself** (Alt+[ / Alt+]). Each is an explicit
    command, written atomically, undoable while the file is unchanged since. JPEG turns by the
    lossless DCT transform (trimming the edge only with consent); PNG, WebP, TIFF, JPEG XL, HEIF and
    AVIF get their orientation tag rewritten, pixels untouched; RAW files are never written, their
    turn goes into the XMP sidecar. Plain [ ] still turn only the view. The rule is documented on
    `specular_fileops::orient::rotate`.
  - *Segmentation editing* (OHIF, ITK-SNAP) is out of scope for v1. If added later, edits are saved
    only as a **new DICOM SEG** object, the same way GSPS is handled.
- **No diagnostic or clinical use.** Medical views carry a "Not for diagnostic use" notice.
- **No hospital workflow integration:** no Modality Worklist, MPPS or HL7 v2. Specular is a research
  workstation, not a node in a clinical workflow.
- **No third-party plugins.** All decoders are in-tree crates. Extensibility comes from external
  commands, a local IPC socket and the CLI (§11), not from loading code.
- **No telemetry at all,** not even opt-in: no usage statistics, no crash uploads, no "anonymous"
  pings. Diagnostics stay on the machine (principle 10).

**Deliberately not done.** These stay out of
the roadmap:
- **No automatic landmarking, scores or findings** (AI VHS, automatic cephalometry or nerve canal, CAD
  marks of our own, "abnormal" thresholds, dose pass/fail): under EU MDR Rule 11, interpretation would
  make Specular a Class IIa device. Every tool is user-placed geometry that shows its calibration source.
- **No implant or template libraries** (TraumaCad-style sizing): that is surgical planning; generic
  geometry only.
- **No "diagnostic quality" claims** for GSDF mode, mammography or lossy images, and no medical claims
  in marketing: intended use decides regulation, not the disclaimer.
- **No stacking or processing pipelines** for astronomy (Siril, PixInsight own them), and no full plate
  solver or Gwyddion-class AFM analysis: Specular is the fast triage step before them, and calls
  external tools through user commands.
- **No region-specific "beautify" adjustments, and no quantitative claims** from stain separation or
  gel densitometry: journal image-integrity rules allow only global, recorded display changes.
- **No authoring or "fixing" of IFC, PDF or DWG:** read-only is the promise; markups live in sidecars
  and exports are copies.
- **No Bluebeam clone:** no live cloud sessions, costing databases or estimating.
- **No reverse engineering** of RVT, NWD, RCP/RCS, SLDPRT B-rep or DGN V8: legal risk; users are asked
  for IFC, E57, STEP, DWG or PDF exports.
- **No bundling or auto-download** of ODA tools, the DJI Thermal SDK or Autodesk SHX fonts: their
  licences don't allow it; the user installs them, and Specular finds them.
- **No cloud uploads and no account:** being trusted offline is the edge over web viewers. Teaching-file
  uploads (§7.4) are one-off, user-started exports, never background sync.
- **No Jupyter or web embedding:** napari, jdaviz and ipyaladin own that; the edge is a native app.
- **No mobile or tablet app for now:** demand is unverified and it would be a separate product.

### Principles

1. **First pixel fast, then refine.** Show an embedded preview or low-res decode, then sharpen it.
2. **The GPU handles display work.** Scaling, colour, tone mapping, window/level, tiling, vectors, meshes
   and volumes render on the GPU. The CPU decodes.
3. **A Document, not an Image.** The core model holds rasters, volumes, vector scenes, 3D models and point
   clouds behind one interface.
4. **Domains load lazily.** Opening a JPEG never initialises DICOM networking or CAD kernels.
5. **Every file is hostile.** C/C++ decoders run in a sandboxed helper process.
6. **Use all the hardware.** Detect CPU and GPU capabilities at runtime and take the fastest path, with a
   portable fallback that produces the same result.
7. **One action system.** Keys, menus, the command palette and the CLI all dispatch the same `Action`.
8. **Always the highest-end option.** Whenever there is a choice — algorithm, library, data structure,
   GPU feature, filter, colour pipeline, UI detail — we pick the highest-quality, highest-performance one
   that is feasible, and treat "good enough" as a bug. Cheaper paths exist only as fallbacks for hardware
   or licences that can't support the best one, never as the default. When the best option costs more
   engineering time, we pay it; when two goals conflict, we measure and pick the one the user would notice.
9. **Modern, safe Rust.** We use the newest language features, including unstable ones on a pinned
   nightly toolchain (`std::simd`, and others as they help). `unsafe` is used only where absolutely
   necessary: when there is no safe way to do it (FFI, OS and GPU APIs), or when a safe version costs
   a measurable amount on a target benchmark. Each use is the smallest possible block with
   `#[allow(unsafe_code)]`, a `// SAFETY:` comment and an entry in `UNSAFE.md`, which reviews check.
   The workspace denies `unsafe_code` by default, and decoders of untrusted input aim for none at all.
10. **Local and private.** Specular sends nothing about its users anywhere: no telemetry of any kind
   . The first run needs no network, and a documented no-network mode
    turns off everything that could reach out (map tiles, model downloads, remote sources, any update
    check). Diagnostics are local: the hardware report (`--hw-report`, Settings → Advanced) and the open
    reports (§12.7) are shown to the user, who copies them into a bug report if they want to.
11. **Measurements say where their scale came from.** Every calibrated number carries its calibration
    source (file metadata, detector or geometry, fiducial, or the user), in every domain (§6.8).

---

## 2. UI: our own toolkit on wgpu

**Decision: wgpu is the only renderer.** Every pixel Specular shows (images, volumes, drawings, models,
and all UI chrome) is drawn by our own wgpu code in **one device, one render graph, one frame
scheduler**. We do not use a third-party UI toolkit. We do use small non-rendering libraries for the
problems that are solved and very hard to redo: text shaping, layout and accessibility.

### Why

- **No ceiling on visual quality.** Every effect (backdrop blur, soft shadows, springs, crossfades) and
  every pixel is ours, which serves the high-end goal (§3) directly.
- **One pipeline.** Canvas and chrome share frame pacing, colour management, HDR output and effects;
  the UI can blur the image beneath it with no cross-toolkit copies.
- **Lean.** Only the widgets Specular needs, with no style system to fight and fewer dependencies.

### What we build vs. what we use

| Layer | Built by us | Libraries used (no rendering) |
|---|---|---|
| GPU primitives (`specular-gfx`) | SDF rounded rects/borders, layered soft shadows, backdrop blur (dual Kawase), gradients, image quads, clip stack, batching | `wgpu` |
| Text (`specular-gfx`) | Glyph atlas (subpixel positioning, gamma-correct coverage, per-scale rasterisation), text drawing, caret and selection rendering | `parley` (shaping, bidi, line breaking, font fallback), `swash`/`skrifa` (glyph rasterisation) |
| Icons | Vector icons drawn through `vello` or tessellated paths | Lucide/Phosphor SVG sources |
| Layout | Widget tree and layout passes | `taffy` (flexbox and grid) |
| Widgets (`specular-widgets`) | Button, toggle, slider, text field, dropdown, menu, context menu, tooltip, popover, toast, scroll view, virtualised grid and list, tree, table, split panes and docking, tabs, command palette, dialogs | — |
| Animation | Springs and easing driven by theme tokens; animates only what changes | — |
| Accessibility | Builds the accessibility tree from widgets | `accesskit` (+ `accesskit_winit`) |
| Platform | Native menus (macOS), window effects | `winit`, `muda`, `window-vibrancy`, `arboard` (clipboard), `rfd` (dialogs) |

`vello` remains for **document** vector content (SVG, DXF, HPGL, PDF paths). It is a wgpu compute
renderer, so it runs inside the same device and frame.

### Toolkit model

- **Retained widget tree with simple, declarative updates.** Widgets keep their layout, scroll position,
  focus and animation state between frames. Application state changes produce a new view description
  that is diffed against the tree (Elm/React-style, kept small).
- **Damage tracking.** Only regions that changed are re-laid-out and redrawn; idle frames do no work.
- **Input.** All input becomes `Action`s via `specular-input` (§11) before reaching widgets, so keys,
  menus, the palette and the CLI share one path. Focus management and full keyboard navigation are built in.
- **Theming.** Every visual value comes from `specular-theme` tokens (§3.4); widgets have no hard-coded colours or sizes.

### Guardrails

1. **Purpose-built, not general.** The toolkit only grows to meet Specular's actual screens. The widget
   gallery (`specular --widget-gallery`) is its test bed and visual review surface.
2. **Hardest parts first.** Text rendering and the text field (selection, IME, undo, clipboard) land in
   **M0**, so the riskiest work is proven early.
3. **Accessibility from day one.** Every widget exposes an AccessKit node; screen reader checks run on
   all three OSes before each release.
4. **One settings schema.** Every setting is declared once and appears in the Settings window (§11),
   the command palette, `settings.toml` and the CLI. Form-heavy screens (PACS nodes, DICOM tags) reuse
   the table and tree widgets.
5. **World-ready from day one.** No user-visible string is hard-coded. Every widget lays out correctly
   in right-to-left languages, with long translations, and with any script (§11, "Languages").
6. **Budget.** The toolkit is estimated at an extra **6–10 weeks** for one developer across M0–M1,
   tracked as its own workstream.

### Alternatives considered

| Option | Why not |
|---|---|
| egui | Fast to build with, but its defaults look like a developer tool; reaching the high-end bar means fighting its styling, and its drawing would sit as a second system on top of ours |
| iced | Retained/Elm architecture is slower to build dense tool UIs; less control over a shared wgpu canvas |
| Slint | Good toolkit, but its licence (GPL or a paid royalty licence) does not fit a permissively licensed app |
| Xilem / Floem | Promising, but not mature enough to bet on; we borrow ideas and share libraries (`parley`, `vello`) |
| Native per platform | Three UIs to maintain, which defeats the point |

---

## 3. Visual design and quality

Specular should feel like a premium tool: **the image is the hero, and everything else is quiet, precise
and fast.** Our references are the restraint of Apple Preview/Photos, the speed of IrfanView, and the
precision of pro tools such as Lightroom, Figma, Linear and Blender's recent UI.

### 3.1 Image quality (the most important part)

Most of "sharp and beautiful" in a viewer comes from how pixels are drawn, not from the chrome.

| Concern | What we do |
|---|---|
| Downscaling | Gamma-correct (linear-light) resampling; precomputed mip pyramid generated with a Lanczos-3/Mitchell filter, not box; trilinear + anisotropic between levels. No shimmer or moiré while zooming |
| Upscaling | Bicubic (Catmull-Rom) or Lanczos in shader below about 300%; switches to **nearest-neighbour with an optional pixel grid** above that, so pixels stay crisp for inspection |
| 100% zoom | Exactly pixel-aligned: texel centres snapped to device pixels, no half-pixel blur |
| HiDPI | Correct per-monitor and fractional scaling (125%, 150%, 175%); "100%" means one image pixel per *device* pixel, with a setting for "per logical pixel" |
| Banding | Render in 16-bit float; blue-noise **dithering** to the 8/10-bit surface, so gradients and skies never band |
| Colour | Full ICC management to the display profile; wide gamut (P3) and HDR (EDR / scRGB / HDR10) output where the OS supports it |
| Optional sharpening | Subtle adaptive sharpening on strong downscale (off by default, toggleable) |
| Transparency | Refined checkerboard (low contrast, scales with zoom) or solid matte, user choice |
| Medical | Exact window/level maths, no filtering in measurement mode, optional GSDF-style display calibration |
| Vectors | Analytic anti-aliasing via vello; re-rasterised at every zoom level, so always crisp |

### 3.2 Motion

- **120 Hz-ready.** Rendering is driven by the display's refresh rate (ProMotion and high-refresh monitors),
  with stable frame pacing and no tearing.
- **Zoom and pan feel physical:** zoom anchored under the cursor or pinch point, critically damped spring
  animation, trackpad momentum with natural friction, rubber-band at edges.
- **Transitions:** a short crossfade (about 120 ms) between images, and none if you hold the key to flip
  fast. A smooth "zoom from thumbnail" when opening from the library.
- **Never a blank frame.** Show the previous image, then the preview, then the full image. Spinners only
  after 300 ms. No layout jumps.
- Animation durations sit between 120 and 220 ms with ease-out curves. "Reduce motion" turns them off; it
  is on by default, and can be set to follow the OS setting or turned off. It does
  not stop animated images: they autoplay per their own setting.

### 3.3 Chrome

- **Immersive by default.** In viewer mode the chrome auto-hides; the toolbar and info appear on mouse
  movement as translucent overlays with **backdrop blur** (frosted glass) and fade away after about 1.5 s idle.
- **Native window integration:**
  - macOS: transparent title bar with content behind it, vibrancy, traffic lights inset, native menu bar.
  - Windows 11: Mica/Acrylic backdrop, rounded corners, snap layouts, dark title bar.
  - Linux: our own client-side decorations, consistent with the app, or server-side decorations if the user prefers.
- **Background:** dark neutral by default (about #141414, a near-black that flatters images), with
  options for pure black (OLED, presentations), neutral 18% grey (colour-critical work) and light.
- **Typography:** Inter (OFL) for UI with tabular figures for numbers; JetBrains Mono (OFL) for DICOM tags,
  paths and metadata. One type scale (11/12/13/15/20 px), with weights limited to regular and semibold.
- **Icons:** one consistent outline set, Lucide (ISC) or Phosphor (MIT), drawn as vectors at every size.
  Custom glyphs for domain tools (window/level, MPR, section box), drawn in the same style.
- **Layout:** a 4 px spacing grid, consistent 6/10 px corner radii, 1 px hairline separators at low
  contrast, soft layered shadows. No heavy borders and no gradients on controls.
- **Colour:** neutral greys plus one accent colour (user-selectable). Semantic colours (warning, error,
  success) are used sparingly. All text meets WCAG AA contrast: the faint text colour is only for
  non-essential decoration, never for placeholders or secondary text; on selected or hovered rows,
  secondary text uses the full text colour; status written as text (keep/discard) uses text colours,
  not the badge fills.
- **Small details:** hover and press states on everything, focus rings for keyboard users, a smooth caret
  in the command palette, monospaced numbers so zoom percentages don't jitter, tooltips with shortcut chips,
  and polished empty states ("Drop images here" with a subtle illustration).

### 3.4 Design system (`specular-theme`)

- **Design tokens** cover colours, spacing, radii, typography, elevation and motion durations/curves.
  They are defined once in a TOML/RON file and used by every widget, the canvas and the wgpu effects.
- **Themes:** Dark (default), Black, Light, High contrast. A **Reading Room** preset for medical use
  (very dark, minimal chrome, dimmed accents).
- Every custom widget has a gallery page (`specular --widget-gallery`) for review.

### 3.5 Process

- **Design before code** for every screen: mockups (Figma or Penpot) for the viewer, library, medical layout,
  CAD views and command palette, approved before implementation.
- **Screenshot regression tests** at 1×, 1.5× and 2× scaling, in light and dark, across all three OSes in CI.
- **Image-quality tests:** reference renders (zone plates, gradients, test charts) compared against
  golden images with a perceptual metric, so a resampling regression fails CI.
- **Polish passes:** each milestone ends with a dedicated week for visual and interaction polish, not just bug fixing.
- **Frame-time HUD** (`Ctrl+Shift+F`) during development, so we never ship jank.

---

## 4. Architecture

### Layer diagram

```
┌───────────────────────────────────────────────────────────────────────────┐
│ app/specular: windows, modes, command line, platform integration          │
├───────────────────────────────────────────────────────────────────────────┤
│ specular-widgets (chrome, palette, panels) · -theme · specular-input      │
├─────────────────┬─────────────────┬───────────────────┬───────────────────┤
│ Develop:        │ Medical:        │ Drawings and maps:│ Library:          │
│ specular-develop│ specular-dimse, │ specular-vector,  │ specular-library, │
│                 │ -dicomweb,      │ -bim, -map,       │ -thumbs, -query   │
│                 │ -labelmap       │ -geodesy          │                   │
├─────────────────┴─────────────────┴───────────────────┴───────────────────┤
│ specular-render (images, tiles, colour) · specular-render-3d (meshes,     │
│ points, splats) · specular-render-volume (MPR, volume rendering)          │
├───────────────────────────────────────────────────────────────────────────┤
│ specular-gfx: wgpu device, UI primitives, text, final composite           │
├───────────────────────────────────────────────────────────────────────────┤
│ specular-core: sources, documents, rasters, jobs                          │
├───────────────────────────────────────────────────────────────────────────┤
│ specular-decode: Decoder trait, registry, probing ──► crates/formats/*    │
│                  └──► specular-sandbox ──► specular-decode-host           │
│                                            (C and C++ libraries)          │
└───────────────────────────────────────────────────────────────────────────┘
```

### Workspace layout

```
specular/
├─ app/specular          # The binary: windows, modes, input, command line, platform integration
├─ crates/specular-*     # The engine: core, decode registry, rendering, UI toolkit, library,
│                        #   Develop, colour, i18n, sandbox, AI and the other shared crates
├─ crates/formats/*      # One crate per format family: parsers and codecs, no UI
├─ clients/python        # The official IPC client
├─ packaging/            # Installers per platform and the third-party notices
├─ third_party/          # Patched third-party crates (UnRAR, resvg, libjxl's build)
└─ tools/                # Benchmarks, corpus fetchers, quality oracles, fuzzing, release scripts
```

[HACKING.md](../HACKING.md) maps the crates in more detail; each crate's `Cargo.toml` says what it
does.

### Core model

```rust
pub enum Content {
    Raster(Raster),            // 2D; u8/u16/f16/f32; 1–4 channels; colour space; optional gain map
    NdArray(NdSource),         // N-D scientific data: named axes (t, c, z, y, x, λ…), any channel
                               // count, per-channel display settings, multiscale chunks
    Animated(Animation),       // GIF/APNG/WebP/AVIF/JXL frames with timing
    MultiPage(PageSource),     // TIFF, PDF, ICO, DWF sheets: lazily decoded pages
    Pyramidal(TileSource),     // Gigapixel rasters, whole-slide images, scanned drawings
    Volume(Volume),            // Voxels + spacing + orientation + rescale + frame of reference
    Vector(VectorScene),       // DXF/DWG/HPGL/CGM/SVG/PDF: layers, sheets, units, extents
    Model(SceneGraph),         // IFC/STEP/glTF/STL/USD: meshes, hierarchy, properties, PMI
    PointCloud(OctreeSource),  // LAS/LAZ/COPC/E57: streamed LOD
    Splats(SplatSource),       // 3D Gaussian splats: PLY/SPZ/SOG/glTF KHR_gaussian_splatting
}

pub struct Document {
    pub source: SourceId,       // Local path, archive member, or remote URL (§12.8)
    pub content: Content,
    pub metadata: Metadata,     // EXIF/XMP/ICC, DICOM dataset, IFC psets, CAD header...
    pub units: Option<Units>,   // Physical calibration: mm per pixel, µm, arcsec, drawing units, metres
    pub geo: Option<GeoRef>,    // CRS + transform (GeoTIFF, IfcMapConversion, LAS WKT, WCS for FITS)
}

pub struct GainMap {            // Ultra HDR / ISO 21496-1 / Apple: applied in the shader,
    pub map: Raster,            // weighted by the display's live HDR headroom
    pub params: GainMapParams,
}

pub trait Decoder: Send + Sync {
    fn id(&self) -> &'static str;
    fn probe(&self, header: &[u8], ext: Option<&str>) -> Confidence;
    fn preview(&self, src: &Source) -> Option<Raster>;           // Embedded thumbnail/preview
    fn decode(&self, src: &Source, req: &DecodeRequest) -> Result<Document>;
}

pub struct DecodeRequest {
    pub target_size: Option<(u32, u32)>, // Lets decoders scale cheaply (JPEG DCT, J2K levels)
    pub region: Option<Rect>,
    pub page: u32,
    pub frame: u32,
    pub index: NdIndex,                  // Position on the non-spatial axes (c, z, t…) for NdArray
    pub channels: Option<ChannelSet>,    // Decode only the visible channels
    pub level: Option<u32>,              // Multiscale level, when the source has one
    pub cancel: CancelToken,
}
```

Each format crate registers its decoders with the registry. Probing uses magic bytes first and the
extension only to break ties. Decoders marked `sandboxed` are invoked through `specular-sandbox`.

**Sources are not always files.** A `Source` is a byte provider with random access: a local file, a
member of an archive (CBZ/ZIP/7z/RAR, DICOM ZIP), or a remote object read with HTTP range requests
(§12.8). Decoders never assume a path, which is what lets the same TIFF/COG, OME-Zarr or DICOM decoder
work locally, inside an archive and over the network.

**Two design rules this model enforces:**
- `Raster` stays the fast path for photos (≤ 4 channels). Anything with more channels, or with z/t
  axes outside DICOM, is an `NdArray`, so fluorescence, hyperspectral and FITS cubes never get squeezed
  into RGB.
- 3D content uses **f64 world coordinates** on the CPU and is rendered **relative to the eye** (camera
  origin subtracted in f64, then f32 on the GPU), with an optional global shift. Georeferenced BIM, GIS
  and lidar coordinates (millions of metres from the origin) never jitter. This mirrors the 2D view's
  tile-relative coordinates (§12.6).

---

## 5. Component choices

We prefer pure Rust with permissive licenses. C/C++ libraries are allowed only where no Rust option is
good enough. They are **dynamically linked** if LGPL and **run in the sandbox host** if they parse files.

**Image decoders are our own or optimised forks**, each an in-tree crate under `crates/formats/` with a
plain decode API; `specular-decode` only adapts them. Speed across all platforms is the product, and
upstream crates can't be tuned freely. Small formats are written from scratch; big codecs (AV1,
JPEG XL, HEVC) are forked from a Rust implementation and then optimised. Our own decoders and forks are
pure Rust. A C/C++ decoder may still be used where it clearly beats the Rust option (libheif stays only as a HEIC fallback),
under the rules above. Upstream crates stay as dev-dependency oracles in differential tests. The format
rows below name the upstream that is forked, ported or used as the oracle.

| Area | Component | License | Notes |
|---|---|---|---|
| Windowing | `winit` | Apache/MIT | |
| GPU | `wgpu` | Apache/MIT | Vulkan, Metal, DX12, GL fallback; WGSL; compute |
| UI toolkit | Our own (`specular-gfx`, `specular-widgets`) | — | See §2 |
| Text shaping and layout | `parley`, `swash` / `skrifa` | Apache/MIT | Bidi, CJK, emoji, fallback |
| UI layout | `taffy` | MIT | Flexbox and grid |
| Accessibility | `accesskit`, `accesskit_winit` | Apache/MIT | |
| Clipboard | `arboard` | Apache/MIT | |
| Window effects | `window-vibrancy` | Apache/MIT | Mica/Acrylic (Win 11), vibrancy (macOS) |
| Fonts | Inter, JetBrains Mono, Noto (fallback) | OFL | Bundled |
| Icons | Lucide or Phosphor (SVG → vello) | ISC / MIT | Plus custom domain glyphs |
| Animation | Own spring/easing module in `specular-theme` | — | Tokens for durations/curves |
| Menus, dialogs | `muda`, `rfd` | Apache/MIT | |
| 2D vector rendering | `vello` | Apache/MIT | GPU compute vector renderer |
| SVG | `usvg` / `resvg` | Apache/MIT | Parse into vector scene |
| Threads | `rayon`, `crossbeam-channel` | Apache/MIT | No async runtime in the core |
| SIMD | `std::simd` (portable SIMD, nightly); `std::arch` only where it is measurably faster | Apache/MIT | x86-64-v3 (AVX2) baseline needs no dispatch; AVX-512 and SVE via runtime dispatch where it pays |
| Allocator | `mimalloc` | MIT | |
| Async I/O | `io-uring` (Linux), Windows IoRing / DirectStorage via `windows` crate | Apache/MIT | Platform modules |
| Hardware decode | Vulkan Video, D3D12 Video/Media Foundation, VideoToolbox/ImageIO, VA-API/oneVPL; optional NVDEC/nvJPEG | Platform APIs | Via wgpu-hal interop; NVIDIA SDKs only if their terms allow |
| CPU topology | `hwlocality` or own detection | MIT | P/E cores, caches, NUMA |
| Networking (M7) | `tokio` + `reqwest`, isolated in `specular-dicom-net` | MIT | Only loaded when used |
| Colour | `moxcms` (fallback `lcms2`) | BSD/MIT | ICC transforms |
| Config | `serde`, `toml`, `toml_edit` (keeps user comments when saving) | Apache/MIT | Schema in `specular-settings` |
| Localisation | `fluent-bundle`, `fluent-langneg`, `unic-langid` (Project Fluent) | Apache/MIT | Plurals, gender, selectors; translators edit `.ftl` files |
| Locale formatting | ICU4X (`icu` 2.x: datetime, decimal, plurals, collator, list, segmenter, displaynames) | Unicode-3.0 | Dates, numbers, units, locale-aware sort, language and country names |
| OS locale | `sys-locale` | Apache/MIT | Initial language on first run |
| Bidi and shaping | `parley` (bidi, shaping via `swash`/HarfRust), `unicode-bidi` | Apache/MIT | RTL text, mixed-direction strings |
| UI fonts | Inter (Latin/Greek/Cyrillic), Noto Sans Arabic, Hebrew, Devanagari, Thai and others, Noto Sans CJK (on demand) | OFL-1.1 | System fonts used first via `fontique`; bundled fonts fill gaps |
| Database | `rusqlite` (WAL, FTS5) | MIT | Library and study index |
| FS watch | `notify` | CC0/MIT | |
| JPEG | Own (`specular-jpeg`) | — | SIMD, DCT scaling, row index; `zune-jpeg` as oracle |
| PNG/APNG | Own (`specular-png`) | — | `png`, `zune-png` as oracles |
| GIF, BMP, ICO, TGA, PNM, QOI | Own | — | Upstream crates as oracles |
| WebP | Fork of `image-webp` (`specular-webp`) | Apache/MIT | |
| AVIF | Own (`specular-avif`): own HEIF parser over a vendored `rav1d` fork (Rust port of dav1d) | BSD-2 | `unsafe` inside rav1d only (UNSAFE.md) |
| JPEG XL | Fork of `jxl-oxide` (`specular-jxl`), no `unsafe` | Apache/MIT | Upstream `jxl-oxide` as oracle |
| HEIC/HEIF | Own `specular-heic` (pure Rust: heic-rs core with Oxideav fallback; VA-API through `specular-hwdec`, chosen by the path selector, §12.3.1) | MIT / Apache | In-process. `libheif` (LGPL, dynamic) in the sandboxed host only as a fallback for files it rejects (`SPECULAR_HEIC_LIBHEIF_FALLBACK=0` disables) |
| TIFF | Own (`specular-tiff`) | — | Tiled, BigTIFF; JPEG-in-TIFF via `specular-jpeg`; `tiff` as oracle |
| EXR / HDR | Own (`specular-exr`, `specular-radiance`) | — | Radiance RGBE and PFM; `exr` as oracle |
| Camera RAW | Own (`specular-raw`): embedded previews, own sensor decoders, demosaic; optional LibRaw later | — / LGPL-CDDL | LibRaw would be dynamic + sandboxed |
| Metadata | `kamadak-exif`, XMP parser, maker-note lens tables, `c2pa` (read/verify) | BSD-2 / MIT/Apache | Decoded into plain language (§6.7) |
| Reverse geocoding | `reverse_geocoder` + GeoNames cities data | MIT/Apache-2.0 + CC BY 4.0 | Offline; attribution in About |
| DICOM | `dicom-rs` (`dicom-object`, `dicom-pixeldata`, `dicom-ul`) | Apache/MIT | |
| JPEG 2000 / HTJ2K | Own (`specular-jp2`, Part 1); HTJ2K open | — | Reduced-resolution and region decode |
| JPEG-LS | `charls` (C) | BSD-3 | |
| NIfTI / Analyze | `nifti` | Apache/MIT | |
| MINC / HDF5 | `hdf5` (C) | BSD-style | Optional feature |
| PDF | `pdfium-render` + PDFium | Apache/MIT + BSD | Evaluate pure-Rust `hayro` |
| DXF | `dxf` | MIT | |
| DWG | ODA File Converter (external, user-installed) | Free tool | Native reader is a research track |
| IFC | IfcOpenShell (C++) or web-ifc (C++) | LGPL / MPL-2.0 | Sandboxed |
| STEP / IGES | OpenCASCADE | LGPL-2.1 + exception | Sandboxed; watch pure-Rust `truck` |
| Meshes | `gltf`, `tobj`, `stl_io`, `ply-rs`, 3MF parser, `ufbx` | Apache/MIT | |
| Point clouds | `las`, `laz`, `e57` | MIT/Apache | |
| Whole-slide | OpenSlide (C) | LGPL-2.1 | Sandboxed |
| FITS | `fitsio` (cfitsio) | MIT-like | |
| Catch-all | libvips (C) | LGPL-2.1 | Sandboxed, late |
| Gain-map HDR | `ultrahdr-rs` (or own) | Apache/MIT | Ultra HDR XMP + ISO 21496-1 in JPEG/AVIF/HEIC/JXL |
| JPEG XL (alternative) | `jxl-rs` | BSD-3 | Benchmark against `jxl-oxide`; keep the faster |
| Archives | `zip`, `sevenz-rust`; `unrar` (sandboxed) | MIT / Apache / UnRAR licence | UnRAR permits decompression only, which is all we need |
| Zarr / object stores | `zarrs`, `object_store` | Apache/MIT | Zarr v3 sharding; S3/GCS/Azure/HTTP |
| Vendor microscopy | `libCZI` (C++), own readers for ND2/LIF/IMS (HDF5) | LGPL-3 / own | Bio-Formats is GPL Java: excluded |
| COPC | `las` + own COPC hierarchy reader | MIT | Opens with no octree conversion |
| IFC (alternative) | IFClite | MPL-2.0 | Rust, IFC5-ready; evaluate against IfcOpenShell/web-ifc |
| IDS / BCF | `openbim-ids`; own BCF 2.1/3.0 reader | MIT / own | |
| USD | `tinyusdz` (C++) | Apache-2.0 | Sandboxed; USDZ included |
| Mesh compression | Draco decoder, meshopt | Apache / MIT | glTF extensions |
| Colour management (film/VFX) | OpenColorIO v2.5 | BSD-3 | GPU shader generation; ACES 2.0 configs |
| QR / barcodes | `rqrr` | Apache/MIT | |
| Radiometric thermal | Own FLIR R-JPEG/FFF/SEQ reader (Planck conversion); DJI R-JPEG through the user's own DJI Thermal SDK install | — / DJI licence | The SDK is never bundled; it runs only in the sandbox host (§6.9) |
| SHX fonts and shapes | Own parser | — | Autodesk SHX files never bundled; user font paths (§8.1) |
| Annotation interchange | Own GeoJSON (QuPath convention), ImageJ `.roi`/`.zip`, CSV, BCF writers | — | `roifile` and QuPath exports as oracles (§6.8) |
| Slide decks | Own minimal PPTX writer (images and captions only) | — | Key images and figures (§6.5) |
| SAMP | Own hub client (XML-RPC over localhost) | — | DS9, TOPCAT, Aladin interop (§11) |
| OCR | OS APIs: Vision (macOS), Windows.Media.Ocr; Tesseract (Linux, optional) | Platform / Apache-2.0 | Local only |
| Map tiles | User-configurable tile server (OSM default, per its usage policy) | — | GPS panel and map view |
| Packaging | `cargo-dist`, Flatpak, AppImage, MSI/winget, notarised .dmg | | |
| License policy | `cargo-deny` | | Blocks GPL/AGPL in CI |
| Fuzzing | `cargo-fuzz` | | Every in-process decoder |

**Camera RAW colour** (`specular-raw`, on the CPU). A DNG uses its own colour tags: a camera profile
with `ProfileHueSatMap`/`ProfileLookTable` goes through the floating-point profile stage (camera →
XYZ D50 → ProPhoto → HueSatMap → exposure → LookTable; the profile tone curve is not applied),
otherwise its ForwardMatrix or ColorMatrix. Other files take the first that exists:
1. a built-in DNG profile of the body: from a camera-written CC0 DNG, or Adobe's own profile from a DNG
   made with Adobe DNG Converter (ColorMatrix only, since Adobe Standard's ForwardMatrix needs the
   HueSatMap to keep its saturation);
2. Adobe's ColorMatrix from the dcraw-era table, or that of a same-sensor sibling body;
3. a matrix embedded by the camera (Sony), else a generic matrix.

Model names match by prefix on whole words, so a later generation ("Mark II") never takes an older
body's data. `crates/formats/specular-raw/src/adobe_coeff.rs` is generated by
`tools/gen-adobe-coeff.py` from the manifest `tools/dng-colour-sources.tsv`, which pins every source DNG
by SHA-256.

---

## 6. Formats: photo and general

These formats ship by **M1**.

| Group | Formats | Special handling |
|---|---|---|
| Web and photo | JPEG, PNG, APNG, GIF, WebP, AVIF, JPEG XL, HEIC/HEIF | DCT-scaled JPEG previews; animation; JXL progressive |
| Simple | BMP, ICO, CUR, TGA, PNM/PAM, QOI | |
| Professional | TIFF (tiled, multi-page, BigTIFF, 16/32-bit float) | Pages; tiles feed the tiled renderer |
| HDR | OpenEXR (multi-layer), Radiance HDR | Exposure slider, shader tone mapping, layer/channel picker |
| Camera RAW | CR2, CR3, NEF, ARW, RAF, ORF, RW2, DNG, … | Embedded preview first, full demosaic on demand |
| Vector | SVG, SVGZ | Re-rendered by vello at every zoom |
| GPU textures | DDS, KTX2 | Uploaded compressed (BC/ASTC) when supported |
| Gain-map HDR | Ultra HDR JPEG (MPF + XMP), ISO 21496-1 in JPEG/AVIF/HEIC/JXL, Apple HEIC gain maps | See §6.1 |
| PNG 3rd edition | cICP (PQ/HLG), mDCV, cLLI chunks; APNG as standard | HDR PNGs shown as HDR, not clipped |
| Image sets | ZIP/CBZ, 7z/CB7, RAR/CBR, TAR | Opened as a folder; reading mode (§6.4) |
| Motion and video | Motion Photos (MP4 appended to JPEG/HEIC), Live Photos (paired MOV); common video (MP4/MOV/WebM) for basic playback | Hardware decode via `specular-video`; still image first |
| Other colour models | CMYK and Lab (TIFF, JPEG, PSD) via ICC; 5+ channel TIFF as `NdArray` | Soft-proof style conversion to display |

Metadata (EXIF, XMP, IPTC, ICC, maker notes, C2PA) is shown in the info panel in plain language (§6.7).
Orientation is applied on display.
Radiometric thermal JPEGs (FLIR, DJI) open here as ordinary photos; their temperatures are read by the
thermal tools of §6.9, which come with the Engineering or Science profile.
PNG colour precedence is cICP > iCCP > sRGB > gAMA/cHRM. A lone gAMA (with cHRM, else sRGB primaries)
becomes a matrix/shaper profile with a pure power curve, except gAMA ≈ 1/2.2 on sRGB primaries, which
is treated as sRGB. cHRM without gAMA is ignored.

### 6.1 HDR and gain maps

Most new phone photos (Android Ultra HDR, iPhone HEIC, newer cameras) store an SDR base image plus a
**gain map**. A viewer that ignores it shows a flat SDR picture.
- The gain map is applied in the display shader, weighted by the **display's current HDR headroom**
  (which changes with brightness and ambient light on laptops and phones). On SDR displays the SDR
  base image is shown unchanged.
- A status-bar badge shows "HDR" with the headroom in use; a toggle (`Shift+H`) switches HDR ↔ SDR
  rendition for comparison.
- Content that exceeds the display's headroom is **soft-clipped perceptually**, never hard-clipped.
- Thumbnails and the library always use the SDR rendition; the viewer upgrades to HDR on open.
- **One path for every format.** JPEG (Ultra HDR, ISO 21496-1, Apple), HEIC (ISO `tmap` items and
  Apple's auxiliary HDR gain map) and AVIF all hand the same gain map to the shader.
  - ISO 21496-1:
    - the weight is signed, so a map from an HDR base towards SDR works too;
    - the map is applied in the base or alternate colour space as signalled, then converted back.
  - HEIC: a `tmap` counts only when it derives from exactly [primary, map].
  - Apple: the headroom comes from the map's XMP, or else the maker notes.
  - AVIF: either direction. HDR stills (PQ or HLG, in BT.709, P3 or BT.2020) decode to the 16-bit
    signal, so an HDR base's map is applied in float: weight −1 on an SDR display, 0 at a headroom of
    2^B or more. Animations and other primaries are tone mapped to 8 bits, without the map.
  - Thumbnails of an HDR base with a map use its SDR rendition, not the generic tone map.
  - JPEG XL: the `jhgm` box (ISO/IEC 18181-2 gain map bundle: ISO 21496-1 metadata, the alternate's
    ColorEncoding or ICC, and a JPEG XL codestream for the map), applied either way like AVIF's.
    PQ/HLG stills with known primaries decode wide (`Rgba16`, the file's primaries) with or
    without a map; a map is used only on an HDR base. Linear stills with float samples, or with an
    intensity target above 255, decode to `Rgba16F` with 1.0 = reference white: samples are scaled
    by intensity_target/203 when the target is above 255 cd/m², otherwise unscaled. Both are
    rendered unbounded, as libjxl does: no SDR tone map and no XYB→sRGB gamut map. Animations, CMYK
    and HDR with unknown primaries are tone mapped to 8 bits. `brob` boxes holding Exif, XMP or
    the gain map bundle are decompressed by the in-tree `specular-brotli` (RFC 7932). Not read: an
    ICC-described alternate (the base's primaries are used), and gain maps on linear bases. Rendered
    planes may be larger than the image or offset from it (a frame cropped at a negative offset);
    conversion reads them from each channel's image origin, and samples outside a plane are 0.
- The information panel names the kind (Apple, ISO 21496-1, Ultra HDR) and "Up to N× SDR
  brightness".

### 6.2 Inspection tools

All are shader overlays (no re-decode) and work on every raster format, not only EXR. Grouped in
`specular-inspect`.

| Tool | Behaviour | Key |
|---|---|---|
| **Clipping warnings** | Blown highlights (red) and blocked shadows (blue), per channel, with adjustable thresholds | `j` |
| **Focus peaking** | Edge-energy overlay to judge sharpness at fit zoom | `Shift+F` |
| **Histogram overlay** | Live RGB/luma histogram of the visible region; for RAW, of the raw data | `Shift+G` |
| **Channel isolation** | R, G, B or luma as grey, unpremultiplied, after tone mapping (luma is relative luminance in the image's own primaries). `Shift+4` cycles alpha as grey → over checkerboard → over the matte colour (`inspection.alpha_matte_color`) → off; the alpha views are unavailable without an alpha channel ("This image has no transparency"). Pressing the active key again turns it off. The colour under fully transparent pixels shows too: the GPU stores straight colour and premultiplies each filter tap | `Shift+1…5` |
| **Pixel inspector** | Value under cursor in source units (8/16-bit, float, HU, physical); at high zoom, numbers drawn in each pixel; colour picker copies hex/RGB/float | `Shift+I` |
| **Region statistics** | Drag a rectangle: min/max/mean/σ per channel, pixel count; other shapes and the measurement list are in §6.8 | in inspector |
| **Line profile** | Values along a drawn line, per channel, in source units, updating live as the line, slice or frame changes; copy or CSV (§6.8) | in inspector |
| **Exposure / offset / gamma** | For every format, not only EXR; NaN/Inf/negative pixels highlighted | `e` then drag |
| **False colour** | Map luminance to a colour scale (also for HDR and science data); perceptual maps only by default (below) | `Shift+C` |
| **HiLo indicator** | Saturated pixels (at the file's real bit-depth maximum, e.g. 4095 for 12-bit data) and zero pixels marked in two colours; clipping warnings extended to N-bit science and medical data | in inspector |
| **Colour-vision simulation** | Shows the view as seen with protanopia, deuteranopia or tritanopia (display only) | palette |

**Colour-vision-safe defaults**. Jet and rainbow maps are still the most
used and the least readable, so:
- false colour and single-channel pseudocolour default to **viridis**, with **cividis** and the Crameri
  maps (batlow, roma, vik) offered next; jet and rainbow stay available but are never the default;
- two- and three-channel composites offer **magenta–green** and **magenta–yellow–cyan** presets beside
  red–green;
- every colour map has a legend with values in source units when shown or exported;
- the CVD simulation applies to the whole view and to exported figures' previews, never to the file.

### 6.3 Compare and cull

**Compare** (extends the 2-up mode, `c`):
- Up to **4 images** side by side or in a grid, synchronised zoom and pan (also across separate windows).
- **Flicker/overlay**: one image over another, toggled with a key or blended with a slider.
- **Difference view**: absolute, squared and relative error with false colour, plus summary metrics
  (MAE, MSE, PSNR) for the visible region, as in `tev`.
- Channel isolation (§6.2) applies to every compared image at once, like zoom and pan.

**Cull mode** (`Ctrl+Shift+K`, or automatically when opening a folder of RAW files, configurable):
- Full-screen, minimal chrome, metadata strip showing shutter, aperture, ISO, focal length, focus point.
- **Pick / reject / unflag** flags (`p` / `x` / `u` in cull mode), plus stars (`1`–`5`) and colour labels
  (`6`–`9`). **Auto-advance** after flagging (on by default).
- **RAW + JPEG (+ XMP) shown as one item**; flags apply to the whole group. A setting chooses which
  file is displayed.
- **Copy/move-to slots**: up to 9 destination folders (`Alt+1…9` copy, `Alt+Shift+1…9` move),
  undoable like delete.
- **Burst stacks**: images grouped by capture time and visual similarity (the *similar shots* of §10)
  collapse into a stack; `s` expands, with the suggested best frame on top.
- **Hot folder / tethered mode**: watch a folder and jump to each new file as it arrives (`notify`).
- Flags, ratings and labels write to the DB and, by default (`gallery.rating_sidecars`), XMP sidecars
  that Lightroom, Capture One, darktable and Photo Mechanic read: `xmp:Rating` (a reject is -1),
  `xmp:Label` and `xmpDM:pick`. An existing `IMG.xmp` or darktable's `IMG.CR3.xmp` is matched; with
  RAW+JPEG the stem form belongs to the RAW. Sidecars are written atomically; image files are never
  touched.
- **Not only photos.** The same mode triages astronomy subframes (blink, keep/reject, FWHM,
  eccentricity and star-count columns, §9.1), cryo-EM micrographs (keep/junk lists, §9.1) and
  inspection photo sets (§6.9). Domain columns come from the active profile (§11, "User profiles");
  keep/reject write a plain list or move files, never the files' contents.

### 6.4 Reading mode (comics, manga, scans)

Opening an archive or a folder of numbered pages offers reading mode (`Ctrl+R`):
- Single page, **two-page spread** with the cover shown alone, and automatic detection of
  wide pages that are already spreads.
- **Right-to-left** order for manga, per archive and remembered.
- **Vertical continuous scroll** (webtoon), with smooth momentum.
- Fit width / fit height / fit page; remembered position per archive.
- RAR is decoded in the sandbox host only.

### 6.5 Output and presentation

Specular never edits, but people expect to *produce* things from what they view. All outputs are new
files.
- **Batch convert / resize / rename** of a selection to JPEG, PNG, WebP, AVIF, JXL, TIFF, with metadata
  options and a rename pattern. Runs in the background on efficiency cores.
- **Print** (fit, actual size, multiple per page) and **contact sheets** with captions.
- **Slideshow**: interval, shuffle, recursive, transitions (cross-fade, Ken Burns), optional EXIF/caption
  overlay, loop; **export to MP4**; shows on a second monitor while the main window stays usable.
- **Set as wallpaper / lock screen** (Windows, macOS, GNOME/KDE).
- **Multi-monitor**: library on one screen, full-screen viewer or slideshow on another.
- **Figure export** (scientists, teaching, reports). The view as a
  figure: a **vector scale bar** from the file's calibration (refused with a reason when there is
  none), text labels and arrows, zoom **insets** with their outline on the overview, **split
  channels** beside the merge, colour-map legends, a timestamp or z/t label. Output as SVG and PDF
  (vectors stay vectors, the image is embedded at full resolution) or PNG/TIFF at a chosen DPI. The
  display settings (window/level or min/max per channel, LUTs, gamma, stretch) and provenance (Specular
  build, decoder versions, source file hash) are embedded in the output's metadata and can be shown as
  a caption.
- **Key images and slides.** Mark images or views as key images; copy one to the clipboard as PNG with
  overlays burned in, or export a set to **PPTX** (one image per slide, optional caption from chosen
  metadata) or a folder of PNGs. Medical key images go through the anonymiser first (§7.4).
- **Session and view-state files.** Save the open files, layout, zoom/pan, display settings, overlays
  and measurements as a small `.specular` JSON file that reopens exactly; also export a self-contained
  **PNG or HTML snapshot** to share with someone who doesn't have Specular. Paths are stored relative
  when possible; no image data is copied into the session file.
- All of these are local files the user chooses to write. Nothing is uploaded; there are no share
  links or accounts (§1, Non-goals).

### 6.6 Recognition (local, optional, off by default)

- **OCR / Live Text**: select and copy text in an image; detect URLs, emails and phone numbers. Uses
  the OS OCR engine where available.
- **QR and barcode** decoding, shown as a clickable chip.
- Library-level AI features are in §10.

### 6.7 Metadata display

**Every file with metadata shows it clearly, in human terms first and raw values second.** This
applies to all domains: EXIF/XMP/IPTC, ICC, C2PA, DICOM, IFC property sets, CAD headers, FITS headers,
OME-XML, GeoTIFF tags, PDF document info.

- **Two layers per panel:** a *Summary* card with the facts people care about, decoded and formatted,
  and an expandable *All fields* table (grouped, searchable, with the raw tag name and value). Clicking a row, or pressing it through a screen
  reader, copies the field's full value even when the table shows it shortened.
- **Decoded, not dumped.** Values are translated into words and units:

  | Raw | Shown as |
  |---|---|
  | `ExposureTime 1/250`, `FNumber 2.8`, `ISO 400`, `FocalLength 35`, `FocalLengthIn35mm 52` | 1/250 s · f/2.8 · ISO 400 · 35 mm (52 mm equiv.) |
  | `DateTimeOriginal`, `OffsetTimeOriginal` | Tuesday 14 May 2024, 18:32 (UTC+1), shown in the viewer's locale |
  | `GPSLatitude 38/1 42/1 2297/100 N`, `GPSLongitude 9/1 8/1 2051/100 W`, `GPSAltitude` | **Lisbon, Portugal** · 38.7230° N, 9.1390° W · 112 m above sea level |
  | `GPSImgDirection 287` | Facing west-northwest (287°) |
  | `Flash 0x19` | Flash fired, auto mode |
  | `Orientation 6` | Rotated 90° clockwise (applied) |
  | Lens / maker-note IDs | Lens name from the maker-note lens database |
  | ICC profile | "Display P3", with gamut shown on a small chromaticity diagram |
  | C2PA manifest | Content credentials: who made it, with which tool, edits listed, signature valid or not |
  | DICOM `(0010,0040) F`, `(0018,0050) 1.25` | Sex: Female · Slice thickness: 1.25 mm |

- **Places from GPS:** coordinates are reverse-geocoded **offline** to the nearest city or town,
  region and country with the `reverse_geocoder` crate (MIT/Apache-2.0; a k-d tree over the GeoNames
  cities dataset, CC BY 4.0, credited in About). Lookups take microseconds, need no network and never
  send locations anywhere. The dataset loads lazily on first use. The summary shows the place name;
  the full coordinates, altitude, accuracy and direction sit beneath, with "Copy coordinates" and
  "Show on map" (map view, §10). Place names are also indexed for library search.
- **Grouping:** Camera, Exposure, Lens, Date and time, Location, Image (size, colour, bit depth),
  File, Rights and description (IPTC), Content credentials, plus domain groups (Patient/Study/Series,
  Project/Storey/Element, Drawing/Layers, Observation/WCS).
- **Only what the file actually says:** a field, row or group that the file doesn't record is not
  shown at all. There are no placeholders like "No capture date recorded", "Time zone not recorded",
  "No rating" or "Unknown". That includes values that are present but carry no information: empty or
  whitespace strings, "unknown"/"not defined" enum codes, zero/unset rationals (e.g. focal length 0),
  rating 0, padding and offset tags. A group with no remaining rows disappears. "All fields" follows
  the same rule, with no toggle to show the empty ones.
- **Warnings shown inline,** only for things that are actually wrong or sensitive: date in the future,
  GPS present (privacy hint), orientation conflicts between EXIF and XMP, DICOM identifying fields
  present. A missing value is not a warning (a missing colour profile simply means sRGB, which the
  colour row then doesn't show).
- **Long values wrap, never overflow:** values wrap at words; a word too long for the column (digests,
  hashes, IDs, paths) breaks anywhere. Both margins are equal (16 px) and nothing touches the panel's
  edge. The title (file name) wraps to three lines, then is cut in the middle with the full name in
  a tooltip. The panel itself shortens no value.
- **Compare metadata:** in compare mode, fields that differ between images are highlighted.
- **Privacy:** "Copy without location" and an export option that strips GPS and personal fields from
  the new file (the source is never modified).
- Fields are localised through `specular-i18n` (§11, "Languages"): labels, units, dates, numbers and
  compass directions follow the interface language, and place and country names use the localised
  GeoNames/ICU4X display names when available. Every field can be pinned to the viewer overlay or
  status bar.
- **Acquisition summary** for science files: one card with objective, pixel size, channels with
  wavelengths, exposure, binning, z-step and time interval from OME-XML or the vendor header; for
  astronomy, object, filter, exposure, gain, temperature and binning from FITS keywords. Copyable as a
  methods-section paragraph.

### 6.8 Measurement layer (every domain)

One measurement layer (`specular-measure`) serves photo, medical, engineering and science views. The
research found that every audience asks for the same core: a line
profile, calibrated ROIs with statistics, a measurement list and export. Domain tools (§7.4, §8.1,
§9.1, §6.9) are built on it, not beside it.

- **Tools:** ruler (length), polyline, angle and Cobb angle, rectangle, ellipse, polygon and freehand
  ROI, point, circle, line profile (§6.2). Each shape works in 2D views, MPR planes and on any slice,
  frame or z/t position.
- **Statistics:** area/perimeter, min/max/mean/σ/median and pixel count per channel in source units
  (HU, counts, °C, ADU), and a histogram per ROI. Line profiles plot along the line, and through z or
  t at a point (time course, spectrum through a cube).
- **Measurement list:** every measurement on every open file in one flat, sortable table (file, slice,
  shape, value, unit, calibration source, author, time). Rows select and centre their shape; the list
  can be filtered to the current file.
- **Calibration source badge.** Every value says where its scale came from, in the list and on the
  overlay: *DICOM Pixel Spacing*, *Imager Pixel Spacing (at detector)*, *Estimated Radiographic
  Magnification Factor*, *calibrated by user (ball marker / known length)*, *OME-XML PhysicalSize*,
  *TIFF resolution tags*, *vendor tag (e.g. SEM pixel size)*, *drawing units*, or *uncalibrated
  (pixels)*. A user calibration (draw a line, type its real length, or pick a 25 mm ball marker) is
  stored with the measurement, never written back to the file.
- **Export and import:** CSV of the list; **GeoJSON** (the QuPath and OMERO interchange) and **ImageJ
  ROI** (`.roi`/`.zip`) in and out; COCO and simple label masks for training data; DICOM SR and GSPS
  for medical files (§7.4); BCF for BIM (§8.2). Round-trip tests against OHIF, Weasis, highdicom and
  QuPath outputs are part of the corpus.
- **Provenance:** exported measurements carry the Specular build, decoder version, file hash and the
  calibration source, so a number in a paper can be traced. A published validation suite (known
  phantoms and synthetic images with exact answers) checks every tool in CI.
- **Sidecar layers:** measurements and markups on non-DICOM files are saved in a sidecar
  (`IMG.ext.specular.json`, or GeoJSON beside the file) and listed as a layer that can be hidden,
  locked or exported. Source files are never modified.

### 6.9 Radiometric thermal and inspection photos

Thermal and inspection users (building surveyors, electricians, solar and roof inspectors, NDT; part of
the Engineering profile, §11) need temperatures from radiometric JPEGs and calibrated measurements on
photos.

- **FLIR R-JPEG, FFF and SEQ/CSQ** use our own decoder (`specular-fmt-thermal`): the embedded raw
  thermal image, Planck constants, emissivity, reflected and atmospheric temperatures and distance are
  read from the FFF records, and temperatures are computed on the GPU. SEQ/CSQ sequences (16-bit and
  JPEG-LS frames) play as a frame series, after stills.
- **DJI R-JPEG** (M2EA, M30T, H20T, Mavic 3T and later) needs DJI's Thermal SDK. We never bundle it:
  if the user has installed it, Settings → Plug-ins points at it and it runs in the sandbox host
  (§13), like the ODA File Converter. Without it the visible image and metadata still show, with a
  one-line note on how to enable temperatures.
- **Temperature tools:** spot, box (min/max/mean with locations), line profile in °C/°F/K (on the
  §6.8 layer); emissivity, reflected temperature, distance, humidity and atmospheric temperature
  editable per image (the file is never changed); palettes (iron, rainbow-HC, grey, and the
  perceptual maps of §6.2) with span/level and isotherms (above, below, interval).
- **Export:** a selection to **float32 TIFF** (temperature per pixel) or CSV, batch, for analysis in
  Python or ImageJ; a picture with the palette legend burned in.
- **Calibrated photo measurement:** crack width and defect size from a scale object in the photo (a
  ruler, a known target) or the drone's altitude and lens, with the calibration source shown (§6.8).
- **Drone sets:** map view (§10) with yaw arrows and the camera footprint, sorted by flight, and
  KML/KMZ export of the selection.
- **Inspection report:** a selection with captions, measurements, temperatures and a location map to
  PDF or DOCX, plus a KMZ, as a local file (§6.5). Site photos can be pinned to a sheet of a drawing
  (§8.1) and open from there.

### 6.10 Develop (non-destructive photo editing)

Features and techniques come from published papers and from studying how other tools behave,
never from their code.

- **Scope of v1:** the full module set ships together, not as an early preview.
- **Pipeline:** scene-referred, linear float, one view transform late in the chain; one module per
  job, no legacy display-referred modules. Every operation is wgpu compute in a new
  `specular-develop` crate, with fused point operations, cached checkpoints, partial re-render on a
  slider change, and tiling for huge images. View, export and thumbnails run the same graph.
- **Edits on disk:** the library holds the history; a separate `<file>.<ext>.specular.xmp` sidecar
  is written by default (a setting turns it off) and follows renames and moves.
- **darktable sidecars** are detected and their modules listed, with "Open in darktable"; their
  parameters are not imported.
- **Lens data:** embedded and DNG corrections plus manual settings; no downloaded database in v1.
- **Look:** a neutral sigmoid-style view transform by default; a punchier look is offered as the
  first style.
- **Tethering:** the existing hot folder only.
- **Profiles:** Develop shows in the General and Photography profiles, hidden in Medical and
  Science.
- **Process versions:** at most two kernel versions per module; older edits migrate with a notice.
- **Local AI features** (masks, denoise, removal) come last, through the model manager, opt-in and
  local only (§6.6).

---

## 7. Medical imaging

Medical support goes deep: Specular is a **research workstation and PACS-lite**, not just a viewer that
opens DICOM files. It is **not for diagnostic use**: a banner on medical views, a first-run
acknowledgement and a license clause all say so.

### 7.1 Formats and objects

| Category | Support |
|---|---|
| DICOM images | CT, MR, CR, DX, MG, US (incl. multi-frame cine), PET, NM, XA, RF, SC, OT; Enhanced CT/MR multi-frame with functional groups |
| Transfer syntaxes | Implicit/explicit VR LE/BE, deflate, RLE, JPEG baseline/extended/lossless, JPEG-LS, JPEG 2000, HTJ2K; **video**: MPEG-2, MPEG-4 AVC, HEVC (endoscopy, surgery, ultrasound) via hardware decode |
| Mammography | MG with CAD SR marks (shown as toggleable overlays); **breast tomosynthesis** (BTO) with slab/2D synthesised MIP |
| Segmentation | DICOM SEG (binary, fractional) and the newer **Label Map SEG** IOD (much leaner for large non-overlapping label sets) |
| 3D annotations | SR with SCOORD3D (points/polylines in patient space, shown in every view) |
| Media | DICOMDIR, CD/USB media, folder trees, ZIP; dedup by SOP Instance UID; **ISO images** opened directly; **lenient reading of damaged discs** (bad DICOMDIR, truncated files, wrong extensions) with a recovery report of what was read and what was skipped; discs that hold **only JPEG/HTML or a vendor viewer** show those images, labelled as non-DICOM; import, then **send to PACS** (§7.5) |
| Overlays | SEG (labelmaps), RTSTRUCT (contours), RTDOSE (isodoses), Parametric Maps |
| Annotation objects | GSPS (read and write), Key Object Selection, Structured Reports (TID 1500 measurements, tree view); **other SR templates** (OB/GYN and echo ultrasound SRs, common vendor SRs) shown as a readable tree and table, and SR or PDF objects with a wrong SOP class still shown |
| Other objects | Encapsulated PDF/CDA, waveforms (ECG) |
| Research formats | NIfTI-1/2 (.nii, .nii.gz), Analyze 7.5, NRRD, MetaImage (.mha/.mhd), MINC2, PAR/REC, ECAT |
| External AI label maps | A NIfTI or NRRD label map (TotalSegmentator, MONAI, nnU-Net) opened **over its source DICOM series**, with a geometry check (size, spacing, orientation, origin) that refuses a mismatch with the reason; class names from the TotalSegmentator/MONAI tables or a sidecar JSON |
| Intraoral dental | Proprietary sensor formats (.rvg, .dex, .img) are not read; intraoral images as TIFF or DICOM (IO) with the IHE dental display mapping are |
| Time series | Cardiac phases, DCE, fMRI (4D NIfTI) |
| Neuro research | Atlas lookup with labels under the cursor, Talairach/MNI coordinates, GIFTI and VTK surface overlays |
| Later | DTI tensors and FA colour maps, DICOM-WSI (pathology) |

### 7.2 Series assembly and correctness

- Group by Study, Series and (when present) Stack/Temporal IDs. Sort slices by the projection of
  `ImagePositionPatient` on the slice normal, **not** by `InstanceNumber`.
- Detect gantry tilt, irregular spacing and missing slices, and warn rather than silently resampling.
- Apply Modality LUT (rescale slope/intercept → HU, SUV for PET), VOI LUT / window, and
  Presentation LUT; invert MONOCHROME1; handle YBR colour; respect pixel padding.
- Orientation labels (L/R/A/P/S/I) and patient-space coordinates are available everywhere.
- Distinguish `PixelSpacing` from `ImagerPixelSpacing` (projection radiography), and label
  measurements accordingly.
- **Ultrasound regions:** use `SequenceOfUltrasoundRegions` so each region (B-mode, Doppler strip,
  M-mode) has its own physical scale and units. Measurements outside a calibrated region are refused.
- **Manual calibration:** draw a line of known length when spacing is absent or wrong; the result is
  flagged "user-calibrated" everywhere it is used. A **ball-marker** calibration (pick the marker,
  enter its diameter, 25 mm by default) serves orthopaedic and vet templating.
- **Calibration provenance** on every measurement (§6.8): Pixel Spacing, Imager Pixel Spacing (at the
  detector), Imager Pixel Spacing corrected by Estimated Radiographic Magnification Factor, fiducial,
  or user. Projection radiographs never silently use detector spacing as if it were patient spacing.

### 7.3 Visualisation

- **2D:** window/level by right-drag, presets (lung, mediastinum, bone, brain, stroke, soft tissue,
  liver), colour LUTs, HU/SUV under the cursor, cine, slice wheel, zoom/pan sync.
- **Linked views** (early, not only with hanging protocols): series
  sharing a Frame of Reference scroll together and show **reference lines** of each other's current
  slice; a 3D crosshair click moves every view to that point. For a prior with a different Frame of
  Reference, **one key aligns** the two at the current slice (manual sync by offset), and the offset
  is kept for the session. Moved here from the hanging-protocol bullet below.
- **MPR:** axial, sagittal and coronal views with linked crosshairs, then oblique and **curved MPR** (CPR).
  Thick-slab MIP/MinIP/AvgIP. Resampling runs on the GPU from a single 3D texture. Opening an axial
  CT volume can start in **3-plane MPR** automatically (a setting); MPR and 3D open as **tabs** of the
  study view, not new windows.
  **Curved MPR centreline:** placed manually as a spline by clicking points (early), or extracted
  semi-automatically between two seed points (vessel, airway, colon; later); straightened and
  stretched views, cross-sections along the path at a fixed spacing, **several centrelines** per
  study, and EVAR-style readouts (length along the centreline, minimum and maximum diameter of each
  cross-section).
- **Dental CBCT layout:** draw an arch polyline on an axial slice to get a **panoramic** reconstruction
  and **numbered cross-sections** at a fixed spacing with mm rulers; TMJ and airway presets; a manually
  traced **nerve canal** shown as a tube in every view. Plain geometry, no automatic tracing.
- **3D volume rendering:** GPU ray-marching with transfer function editor and presets, shading,
  clipping box/planes, empty-space skipping, **scalpel/sculpt** cutting.
- **Cinematic rendering:** path-traced volume rendering (volumetric scattering, ambient occlusion,
  area lights) that converges progressively while the view is still; falls back to the ray-marcher
  while moving. A showcase for GPU headroom.
- **Surface rendering:** isosurface extraction (GPU marching cubes), STL/OBJ/3MF export for 3D printing
  (**one mesh per segment** from SEG/label maps, decimate and smooth, a manifold/watertight check with
  a report); **virtual endoscopy** fly-through along a centreline.
- **Lightbox/mosaic:** many slices of one series in a grid.
- **Fusion:** PET/CT and PET/MR overlay with blend and colour map; registration via Frame of Reference,
  **manual rigid registration** (translate/rotate with live overlay), and automatic rigid registration
  (mutual information, GPU) for series without a shared frame of reference. Registrations are saved as
  DICOM Spatial Registration objects, never by resampling the source. A fused view can be exported
  **with its colour map kept**, as Secondary Capture or a Blending Softcopy Presentation State.
- **Segmentations in 3D:** shaded surfaces from SEG/RTSTRUCT, several SEG files merged, RTSTRUCT contours
  reprojected into non-acquisition planes.
- **AI results:** probability maps and heatmaps get their own colour map, threshold and opacity; SR
  findings appear as a tree, each item jumping to its image and location; overlays stay consistent
  across 2D, MPR and 3D. External models run as a **user command recipe** ("run on series → load
  output", §11): Specular exports the series, runs the user's command (e.g. TotalSegmentator) and opens
  the label map it writes over the source. Specular ships no medical model.
- **Hanging protocols:** layouts by modality, body part and priors, built on the linked views above.
  Protocols can **span several monitors**.
- **Teaching:** **case notes** stored with a study (local sidecar, never in the DICOM), saved
  **per-case layouts**, and a locked **quiz/presentation mode** that hides notes and patient data
  until revealed.
- **Mammography layouts:** CC/MLO pairs, mirrored alignment with locked zoom, quadrant zoom.
- **Display calibration:** optional DICOM Part 14 GSDF with measured Lmin/Lmax entered by the user,
  built-in TG18 test patterns, and a dark reading-room theme.

### 7.4 Tools (read-only analysis)

- Length, angle, Cobb angle, ellipse/rectangle/freehand ROI with mean/SD/min/max/area (HU, SUV), on
  the shared measurement layer (§6.8) with its calibration-source badge.
- **Measurement table** for a study: every measurement in one flat table (series, image, type, value,
  unit, calibration source), copy and CSV. Clicking a row or an SR finding **jumps to its image**; a
  lesion measured on priors shows its **size over time**.
- Threshold/region-grow volume estimates (shown, never saved into the source).
- **Time-intensity curves** from an ROI across dynamic/4D series (DCE, perfusion), several ROIs on one plot,
  exportable as CSV.
- **Specialty measurements:** cardiothoracic ratio, spine labelling, goniometry, leg-length and
  height difference, TT-TG distance, vertebral angles, 3D volume ROI.
- **Vet, orthopaedic and dental geometry** (no freeware viewer has vet
  tools). All are **manual geometry tools**: the user places every point, Specular computes the
  numbers, and nothing is detected or scored automatically (§1, Non-goals):
  - vet: **VHS** (vertebral heart score), **VLAS**, **Norberg angle**, **TPA** (tibial plateau angle
    for TPLO), hip distraction index;
  - orthopaedic: femoral-head circle, offset and **leg-length lines**, acetabular angle (generic
    templating geometry, no implant library);
  - cephalometric: a landmark template the user places by hand → **Steiner and Downs** angles and
    distances, CSV export.
- **DSA mask subtraction** for angiography, with mask frame selection and pixel shift.
- **PET:** SUVmax/SUVpeak/SUVmean, threshold-based 3D ROIs, total metabolic tumour volume (TMTV).
- **Radiotherapy:** dose-volume histograms per structure from RTDOSE + RTSTRUCT.
- **ECG/waveform viewer:** standard 12-lead layouts, gain and speed, calipers with interval readouts.
- Save measurements as **GSPS / SR** or a sidecar; create **Key Object Selection** notes.
- DICOM tag browser with private-tag dictionaries, search and copy.
- **Anonymised export** following the PS3.15 Basic Profile (with options), batch mode, and **masking of
  burned-in text** (detected regions shown for confirmation), plus export of the view as PNG/TIFF.
  An **anonymiser audit** in the test corpus covers file and folder names, private-creator blocks,
  text inside encapsulated PDFs and SR, 60xx overlays and icon image sequences.
- **Teaching export:** a trimmed, anonymised stack or key images as a **Radiopaedia** case upload
  (through its API, with the user's own account token entered for that upload) or an **IHE TCE**
  package. Anonymisation is forced and cannot be switched off; every upload is started by the user,
  one case at a time, and shows what will be sent first.
- **Key images** to the clipboard, PNG or **PPTX** (§6.5), with or without overlays, always through the
  anonymiser.
- **Local audit log** (optional, off by default): views, exports and prints as JSON lines in a local
  file for hospital IT. It never leaves the machine unless the user copies it.
- **Movie export** (MP4) of cine loops, MPR sweeps and 3D rotations; copy view to clipboard.
- **Media export to IHE PDI:** DICOMDIR, INDEX.HTM, optional encryption, and the portable Specular
  build on the media as the included viewer. Also export as DICOMDIR folder or ZIP.
- **Print:** paper print with film-sheet layouts (DICOM Print SCU later, if asked for).
- Convert JPEG/PDF to DICOM Secondary Capture / Encapsulated PDF (later).

### 7.5 Connectivity (PACS-lite)

- **Local study database** (patient → study → series → instance), part of the library.
- **DIMSE:** C-ECHO, C-FIND, C-MOVE, C-GET as SCU, and C-STORE SCP (receiver), with configurable AE titles
  and remote nodes. TLS is optional.
- **DICOMweb:** QIDO-RS, WADO-RS, STOW-RS. Tested against Orthanc, dcm4chee, and Google/AWS/Azure health APIs.
- Prefetch of priors, and background retrieval with **progress per series**, **pause and cancel**,
  and an option to **skip thin-slice series** (e.g. below 1 mm) when a thick reconstruction exists.
- **Send to PACS:** C-STORE SCU and STOW-RS of imported patient media or a selection.
- **Progressive streaming over DICOMweb:** request a lossy or partial HTJ2K (RPCL) byte range first
  (about 1/16–1/10 of each image), refine later; volumes load interleaved (every Nth slice first) so MPR
  is usable while data arrives. Per-server retrieve configuration.
- **Launch by URL:** a `specular://` scheme (and CLI equivalent) takes a manifest or DICOMweb query plus a
  short-lived token, so portals, EHRs and research platforms can open a study in Specular.
- **FHIR ImagingStudy / SMART Imaging Access** (draft standard): later, behind a feature flag.
- A **DICOM Conformance Statement** is published with each release that ships medical features.

---

## 8. Engineering and architecture

### 8.1 2D drawings

| Format | Approach |
|---|---|
| DXF (ASCII + binary) | `dxf` crate or own parser → `VectorScene` |
| DWG | ⚠ LibreDWG is GPL (incompatible). Phase 1: user-installed **ODA File Converter** (DWG→DXF), invoked automatically when found. Phase 2: native read-only reader from the public ODA spec (research track). Embedded DWG thumbnails are read natively from day one. |
| DWF / DWFx | DWFx is XPS (ZIP + XML), parsed directly; classic DWF from the published spec |
| PDF drawing sets | PDFium with tiled re-render on zoom, optional content groups (layers), sheet navigation, text search |
| HPGL/HPGL2/PLT | Own parser |
| CGM | Own parser (ISO 8632), used in aerospace/defence (S1000D) |
| Scanned drawings | TIFF G3/G4, JBIG2, huge mono TIFFs via the tiled renderer |
| DjVu | Own decoder later (DjVuLibre is GPL) |
| DGN (MicroStation) | **Corrected**: current files are **V8**, which is proprietary with no public spec and no compatible free library, and V7 is legacy. We don't reverse-engineer V8 (§1, Non-goals). Instead: embedded thumbnails where present; a clear message asking for a DWG, DXF, PDF or IFC export (MicroStation and Bentley viewers produce these); a request to Bentley for the V8 specification under terms we can use. A V7 reader from the public description only if it turns out cheap, for archives |
| SHX fonts and shapes | **Own SHX parser** (compiled shape and big-font files) for DWG/DXF text and complex linetypes. Fonts come from **user font paths** (e.g. an existing AutoCAD or ODA install, or a folder the user names); Autodesk SHX files are never bundled. A **missing-font report** per drawing lists what was substituted, with a substitution table the user can edit; the fallback is a metric-compatible TrueType font so text keeps its extent |
| Proxy graphics | Civil 3D, Plant 3D and other custom objects are drawn from their **proxy graphics**, labelled "proxy" in the inspector, so drawings no longer show blanks where those objects are |
| Gerber / ODB++ / IPC-2581 | PCB fabrication data: Gerber X2/X3 with drill files, ODB++ and IPC-2581 as a layer stack (copper, mask, silk, drill), layer on/off, measurement, net highlight where the format carries nets. Local, no upload (free viewers are mostly online) |

**Features:**
- Layer on/off, model space and paper-space layouts, a sheet list, xref resolution.
- Line-weight display, monochrome plot mode, black/white background swap.
- Measurement with snapping (endpoint, midpoint, intersection, perpendicular) in drawing units.
  Scanned drawings are calibrated by clicking a known dimension.
- **Scale from the file:** PDF viewport measure dictionaries (`/Measure`, `/VP`) and GeoPDF are read,
  so measurements are correct without calibration, including sheets with several viewports at
  different scales. Geographic coordinates are shown where the PDF is georeferenced.
- **More measurement types:** perimeter, polyline length, area with cut-outs, slope (pitch/grade),
  3-point radius, diameter, count, and volume (area × depth). Results export as CSV.
- **Revision overlay**: two drawings tinted red/green over each other, with a difference highlight.
  Overlays align **automatically or by three point pairs** (handles reissues at a different scale or
  offset), compare **more than two revisions**, or only a selected region.
- **Entity-level compare** (DXF/DWG): added, removed and changed entities grouped into change sets
  inside revision clouds; step through changes; filter out text-only changes.
- **Hyperlinks:** section/detail callouts and PDF/DWF links jump to the referenced sheet or view;
  back/forward history.
- **Embedded markups shown read-only:** PDF annotations (Bluebeam markups included), DWF redlines.
- **Markup creation as sidecar layers**: cloud, text, arrow,
  callout, stamp, highlight and measurement markups on drawings and photos, saved as a sidecar layer
  (§6.8), never into the source. Export a **flattened PDF** copy with the markups burned in, or a
  markup list as CSV. Deliberately small: no takeoff, no sets, no studio sessions (§1, Non-goals).
- **Site photos pinned to sheets:** drop photos onto a point of a sheet; the pin opens the photo, and
  the photo's info panel links back to the sheet (§6.9).
- **Print/plot to scale**, honouring page setups and CTB/STB plot styles; output to printer or PDF.
- **Title-block attributes** extracted for the sheet list and library search.

### 8.2 3D models and BIM

| Format | Approach |
|---|---|
| IFC 2x3 / 4 / 4.3 | IfcOpenShell or web-ifc in the sandbox → meshes + spatial tree + property sets |
| STEP / IGES | OpenCASCADE in the sandbox → tessellated meshes + assembly tree |
| STL, OBJ, PLY, glTF/GLB, 3MF | Native Rust parsers |
| STEP AP242 PMI | Semantic and graphic PMI (GD&T, datums, saved views) from OpenCASCADE |
| FBX | `ufbx` (optional) |
| OpenUSD / USDZ | `tinyusdz` in the sandbox; Alembic and VTK later |
| JT, Parasolid (x_t/x_b), ACIS (sat), Rhino 3DM, BREP | BREP via OpenCASCADE; 3DM via `openNURBS` (MIT); JT/Parasolid/ACIS only if a licence-compatible reader exists (research) |
| Revit (.rvt), SketchUp (.skp), Navisworks | Proprietary: embedded preview thumbnails only, plus a hint to export to IFC |
| SolidWorks (SLDPRT/SLDASM), Inventor (IPT/IAM), CATIA (CATPart/CATProduct), Creo | Proprietary: **embedded preview thumbnails only**, plus a hint to export STEP. Native display meshes are not reverse-engineered (§1, Non-goals) |
| 3D PDF (PRC, U3D) | Research: the PRC and U3D specs are public (ISO 14739-1, ECMA-363); a reader would show models that Acrobat disables by default. Large effort; after the core 3D work |

**Features:**
- Orbit/pan/zoom, view cube, standard views, perspective ↔ orthographic.
- **Walk and fly** first-person navigation (`w`): gravity, collision, step height, speed control,
  teleport to a clicked point; gamepad support.
- Section planes and section box, x-ray/ghost, hide/isolate, exploded view.
- **Saved viewpoints**: camera, section planes, visibility and colouring, named and listed.
- IFC: storey filter, class filter (walls, doors, MEP…), and click an element to see its property sets.
  A **storey plan mini-map** that can be clicked to move the camera.
- Distance, area and angle measurement with vertex/edge snapping; **mass properties** (volume, surface
  area, centroid) per part or selection.
- **PMI display**: GD&T annotations, datums and saved views from STEP AP242; selecting a tolerance
  highlights its feature. Checked in CI against the **NIST MBE PMI** and CAx-IF test files.
- **Mesh analysis** (STL/3MF/OBJ): open edges, flipped normals, non-manifold edges, wall-thickness and
  curvature colour maps.
- **Animation playback** for glTF/FBX/USD (skinning, node animation), with a timeline.
- Simulation results stored as vertex scalars shown as colour plots with a legend (later).

**BIM coordination** (openBIM, read and display only):
- **Federation**: several IFC/3D models overlaid in one scene, with collision-safe element IDs, per-model
  visibility, one combined spatial tree, and alignment by georeferencing (`IfcMapConversion`) or
  manual offset.
- **BCF 2.1/3.0**: open `.bcfzip` files or folders; list topics with comments and snapshots; selecting a
  topic restores its viewpoint (camera, visibility, colouring, clipping planes, markup lines).
  **Topic creation**: a new topic from the current view (viewpoint, snapshot, selected components,
  title, comment, status), written to a `.bcfzip` that other tools read; file naming and GUIDs follow
  what BIMcollab, Solibri and Bonsai expect, with round-trip tests.
- **Quantity export**: IFC quantities and properties (`Qto_*`, `Pset_*`) as a schedule grouped by class,
  storey or type, exported to CSV or XLSX. Read-only takeoff, not estimating.
- **IDS 1.0** validation: load an `.ids` file and colour the model by pass/fail, with a results list.
- **Clash results**: show clash sets from BCF or a simple built-in geometric clash pass; zoom to each,
  colour the two objects.
- **Model diff**: two versions coloured added/removed/changed/unchanged, with an opacity blend.
- **2D ↔ 3D**: overlay a drawing sheet on the model at its storey, or a split 2D | 3D view with linked
  selection.
- **Compact cache**: parsed IFC geometry and properties stored in a binary cache (in the spirit of
  That Open "Fragments" and xeokit XKT, often 10–30× smaller than the IFC), so a reopen skips parsing.

### 8.3 Point clouds

LAS/LAZ, E57, PLY, PCD and XYZ, converted on first open into an on-disk **octree cache** for LOD
streaming, which keeps billions of points interactive. Colour by RGB, intensity, classification or
elevation. Adjustable point size, Eye-Dome Lighting, clipping box, measurement.

- **COPC** (`.copc.laz`) is opened **directly** using its built-in octree: no conversion, also remotely
  over HTTP range requests.
- **Points visible while loading:** for non-COPC files, a GPU-built LOD (in the style of Schütz et al.,
  2024) shows the cloud as it streams in, and the on-disk octree is written in the background. The user
  never waits for preprocessing.
- **E57 scan stations**: list scanner positions; jump to a station and view its embedded panorama.
- **Analysis:** filter by classification, elevation profile along a line, volume above/below a
  surface, cloud-to-cloud and cloud-to-mesh distance as a colour scale.
- Orthomosaic and DSM rasters draped or shown alongside, with CAD overlays (later).

### 8.4 Gaussian splats and large scenes

- **3D Gaussian splats**: `.ply` (3DGS), `.splat`, `.spz`, `.sog`, and glTF `KHR_gaussian_splatting`.
  GPU sort and rasterise; LOD for large captures; measurements on splat scenes.
- **3D Tiles 1.1** streaming (photogrammetry meshes, BIM, splats) with screen-space-error LOD, from local
  disk or HTTP.
- **VR/XR walkthrough** via OpenXR (later).

### 8.5 GIS

GeoTIFF/COG (in M1 via TIFF), Shapefile, GeoJSON, GeoPackage. Coordinates are shown under the cursor.
ECW/MrSID only if the SDK terms allow it. COG and GeoPackage also open remotely (§12.8). COG codecs
include LERC, ZSTD, WebP and JPEG XL.

**Raster display (as in QGIS):** single-band pseudocolour with colour ramps, band-to-RGB mapping for
multiband imagery, paletted/classified values with a legend, hillshade and on-the-fly contours for
elevation, and nodata as transparency.

**Also:** **KML/KMZ** display (placemarks, paths, ground overlays) and export of a selection of
geotagged photos (§6.9); **LandXML** surfaces (TIN) and alignments shown in 2D and 3D, with
coordinates and station/offset under the cursor.

### 8.6 Inspection and NDT

Non-destructive testing reuses the medical stack with NDE wording.
- **DICONDE** (ASTM E2339) opens as an inspection mode: "component", "inspection" and "series" instead
  of "patient" and "study", NDE tags in the info panel, no medical banner. Everything in §7.2–7.4 that
  makes sense applies.
- **Radiograph evaluation** on 16-bit weld and casting scans: line profile and grey value, **optical
  density** where calibrated, **SNR** (normalised by basic spatial resolution), and **IQI** reading aids
  (wire and duplex-wire profiles); all on the §6.8 measurement layer. The inspector decides; Specular
  only measures.
- **Industrial CT**: TIFF stacks with voxel size from **`.xtekct`** (Nikon) and **`.vgi`** (Volume
  Graphics) headers, opened through the MPR and volume rendering of §7.3, with the calibration source
  shown. Raw volumes use the header wizard (§9).
- Thermal and drone inspection is in §6.9.

---

## 9. Long-tail formats

| Group | Formats |
|---|---|
| Scientific | FITS (incl. multi-extension and cubes, and **fpack `.fz`** tile compression: Rice, GZIP, HCOMPRESS, PLIO), XISF (PixInsight), **SER** (planetary video), HDF5, NetCDF, raw binary with a header wizard |
| Electron microscopy | **Gatan DM3/DM4** (with stacks), **MRC/MRCS**, Velox **EMD** (HDF5, with calibrations and spectrum-image axes); SEM TIFFs with vendor calibration tags (§9.1) |
| Scanning probe | AFM height maps: Gwyddion **.gwy** and Igor **.ibw** first, with plane levelling as a display option |
| Remote sensing | **ENVI** `.hdr` + raw (hyperspectral), **Sentinel-2 SAFE** as one product, **STAC** catalogues → COG assets |
| Cloud-native | **OME-Zarr / NGFF** (Zarr v2 and v3 with sharding), N5, Neuroglancer precomputed |
| Tile protocols | IIIF Image API 3, Zoomify, DZI, TMS/XYZ |
| Microscopy / pathology | OME-TIFF, SVS, NDPI, MRXS, SCN, BIF (OpenSlide), DICOM-WSI |
| Vendor microscopy | Zeiss CZI, Nikon ND2, Leica LIF/LOF, Olympus/Evident OIR/OIB/VSI, Imaris IMS, DeltaVision DV/R3D, MetaMorph STK/ND |
| Design | PSD/PSB (composite first, then layers), KRA, ORA, XCF |
| Documents | PDF (general), XPS, EPS (only via a non-GPL renderer, or not at all) |
| Textures | ASTC, PVR, VTF, Basis |
| Legacy | PCX, ICNS, SGI, Sun Raster, XPM/XBM, WBMP, JPEG XR, JPEG 2000 (standalone), Amiga IFF/ILBM, PICT |
| Catch-all | libvips in the sandbox, for anything not covered natively |
| Watch only | JPEG AI (ISO/IEC 6048, learned codec). WebP2 is a research project, not a format: no plan |

### 9.1 Scientific and microscopy viewing

These work on any `NdArray` source (§4), whatever the file format.
- **N-D navigation:** a slider per non-spatial axis (z, t, c, λ…) with play buttons; axis names and
  units from the file.
- **Multi-channel compositing:** per channel: colour or LUT, contrast limits, gamma, on/off. Additive,
  max, min and translucent blending; any number of channels. Stored rendering settings (OME-Zarr
  "omero" metadata, Imaris, CZI display settings) are applied on open.
- **Channel mini-views:** a small synced view per channel beside the composite.
- **Display stretch:** linear, log, sqrt, squared, asinh, sinh, histogram equalisation; auto-limits by
  min/max, percentile (e.g. 2–98%), zscale, mean ± kσ. Display only. Without these, 16/32-bit science
  data looks black.
- **Scale bar** from physical units (nm, µm, mm, arcsec), auto-sized, with a toggle and position.
- **Projections and reslicing** for non-DICOM stacks: max/mean Z-projection and orthogonal views, reusing
  the medical volume renderer.
- **Label and annotation layers:** OME-Zarr labels, segmentation label maps, GeoJSON and QuPath objects
  drawn over the image, each with opacity and colour by class.
- **Astronomy:** RA/Dec under the cursor from the WCS, a coordinate grid, DS9 region files, contours,
  cube playback, and blink/align by WCS across frames.
- **Coordinate transforms** between images (NGFF coordinate transformations, WCS) for synced views.
- **High-content screening:** plate → well → field browsing for HCS OME-Zarr (later).
- **Colour management for film/VFX:** OCIO v2.5 / ACES 2.0 display and view transforms for EXR.
- **Scripting socket:** renderers and scripts can push or update images into an open window (as with
  `tev`), via the IPC socket (§11) and the official Python client.
- **Measurement and output** come from §6.8 (line profile through z and t, ROI list, GeoJSON/ImageJ
  ROI, COCO) and §6.5 (figure export with vector scale bar and embedded display settings). The HiLo
  indicator and perceptual colour maps are in §6.2. High-bit greyscale opens **auto-ranged** by
  default, so a 16-bit TIFF is never black.

**Microscopy and pathology**:
- **Stain separation** for brightfield pathology (haematoxylin, eosin, DAB) as a display option, not a
  quantification.
- **Gamepad, SpaceMouse and trackball** navigation for whole-slide images (pan, zoom, next field),
  configurable in the key binding configurator.
- **Serial sections** with different stains synced after landmark alignment (three or more point
  pairs, affine), shown side by side or as an overlay.
- **Gel and blot lanes:** lane profiles with integrated density, background subtraction and CSV.
  Measurement only.
- **Acquisition summary** (§6.7) for vendor formats, plus per-plane timestamps.

**Astronomy and astrophotography**:
- **Subframe triage** in cull mode (§6.3): blink hundreds of subs, keep/reject, move rejects to a
  folder. **Star metrics** per sub (FWHM/HFD, eccentricity, star count, background) as sortable columns
  with a **σ auto-reject** suggestion the user confirms. FITS header fields (FILTER, EXPTIME, GAIN,
  CCD-TEMP, OBJECT, DATE-OBS) are columns and filters too (§10). No stacking (§1, Non-goals).
- **Debayer** honouring `BAYERPAT`, `ROWORDER` and `XBAYROFF`/`YBAYROFF`, with a per-file override;
  colour data opens with an **unlinked** (per-channel) STF-style auto-stretch by default, linked as an
  option.
- **MEF** from JWST/HST: SCI opened by `EXTNAME`, **DQ** bitmask as an overlay with a bit legend, ERR as
  a layer.
- **Aberration inspector** (a 3×3 mosaic of corners, edges and centre) and a sensor **tilt estimate**
  from star FWHM across the frame.
- **Spectrum at the cursor** for IFU cubes and 1D spectrum FITS.
- **Plate solving** for files without WCS by calling ASTAP or astrometry.net as a user command (§11);
  never bundled.
- **SAMP** hub client to exchange images, tables and coordinates with DS9, TOPCAT and Aladin (§11).
- **HiPS** survey backgrounds behind a solved image (later; remote tiles, so off in no-network mode).

**Electron microscopy and calibration**:
- **SEM pixel size from vendor TIFF tags**: Zeiss tag 34118 (the right field within it) and
  FEI/Thermo `[Scan] PixelWidth`; the scale bar and measurements use it and show the source (§6.8).
- DM3/DM4, MRC and EMD calibrations are read the same way; MRC micrographs triage in cull mode with
  keep/junk lists written as STAR or plain text.
- AFM height maps with a height colour bar and plane levelling (display only).

**Remote sensing and spectral**:
- **ENVI** cubes: spectrum at a clicked pixel, any three bands to RGB, wavelengths from the header.
- **Sentinel-2 SAFE** opened as one product: band stack, true and false colour, NDVI as a display
  formula. Needs a fast JPEG 2000 decoder (our own).
- **STAC** catalogue browsing → open COG assets (remote, §12.8).
- **Complex data**: magnitude, phase, real/imaginary and dB display, SAR multilook (also for MRI raw
  data and FFTs).
- **HDF5/NetCDF** lazy slicing with a **lat/lon map view** and coastlines.
- **Spectral EXR**, per-channel histogram stats and the true bit depth shown.

---

## 10. Library

There are **three modes in one window**: *Viewer* (IrfanView-style, the default when opening a file),
*Gallery* (thumbnails of the current folder, `g`) and *Library* (`Tab` or `L`).

### Gallery (thumbnail mode)

The **Gallery** is a thumbnail grid of the current folder, archive or other source: the folder the
viewer is showing, or one opened directly (`specular <folder>`). Unlike the Library it needs no
catalogue, no locations and no indexing. It works on any folder at once, so it ships before the
Library. The Library later reuses the same grid, tile renderer and thumbnail cache.

**Switching**
- `g` toggles Viewer ↔ Gallery. `Enter`, a double-click or a tap on a tile opens that image in the
  viewer. `Esc` or `g` in the viewer returns to the gallery with the same tile focused.
- `Esc` in the gallery closes quick look, compare or an empty filter bar first (a focused search field
  takes it first); otherwise it goes up one folder and focuses the folder it left, as `Backspace` and
  `Alt+↑` do. It ignores auto-repeat. At the filesystem root it clears the selection, then does
  nothing; it never quits.
- The switch is a **shared-element transition**: the image shrinks into its tile and the grid fades in
  around it, and the reverse on open. With reduced motion it is a cross-fade.
- The gallery keeps one position per source. Coming back puts the grid where it was, with the image
  last viewed in focus.

**Comfort features (on by default, visible)**
- **Instant:**
  - Embedded previews (EXIF, RAW, HEIC) and the thumbnail cache fill tiles first; then sharp
    thumbnails are made for visible tiles first, then the ones nearby in the scroll direction.
  - Tiles that scroll out of view are cancelled.
  - Never a spinner per tile: a placeholder in the image's average colour, then the image.
- **Thumbnail size:** `Ctrl` + wheel, pinch, `Ctrl` `+`/`-` or a slider in the bottom bar. Size
  changes smoothly; the tile under the pointer stays put.
- **Layouts:**
  - Square crops (the default, a calm uniform grid);
  - Justified rows (whole images, true aspect, Google Photos style);
  - Uncropped in square cells.
- **Folders and archives as tiles,** with a preview mosaic; a breadcrumb at the top; `Backspace` or
  `Alt+↑` to go to the parent folder. Archives are hidden by default (`gallery.show_archives`,
  View → Show archives); counts, recursive views and duplicates follow that choice.
- **Navigation:**
  - Arrow keys, `PgUp`/`PgDn`, `Home`/`End`.
  - Type to jump to a file name.
  - Smooth, inertial scrolling at the display's refresh rate.
  - A scrollbar with date or letter labels while dragging.
- **Selection:** click, `Shift` / `Ctrl` click, `Ctrl+A`, rubber-band drag, `Space` to toggle on touch
  and keyboard.
- **Quick look:** hold `q`, or `Space` with a pointer, shows a large preview over the grid without
  leaving it; release to return. It honours the channel views (§6.2, one setting shared with the
  viewer), and its caption names the active view. In the grid itself the channel actions do nothing,
  and `Shift+digit` stays with them rather than setting a rating.
- **Hover:** after a short delay, the file name, dimensions, size and date, and the full-size decode of
  that image starts (§12.5).
- **Sort:**
  - name (natural order), date taken, date modified, size, type, dimensions;
  - reverse;
  - remembered per folder, with a global default.
- **Captions:** file name under tiles (on/off), and small badges for what matters: RAW, RAW+JPEG pair,
  animated, HDR, video, multi-page, rating and colour label.
- **Live:** new, changed and deleted files appear without a refresh (the `notify` watcher), and the
  grid keeps its scroll position.
- **Context menu:** open, open in a new window, open with…, user commands, reveal in the file manager,
  copy, copy path, move to trash (undoable), properties.
- **Drag and drop** of the selection out to other apps, and onto folder tiles: a drop moves the files
  there, or copies them (with a modifier key, or onto another volume), undoably. Files dropped from
  other apps land on a folder tile only on X11, where the pointer position is known; elsewhere a drop
  opens the file.
- **Accessible:**
  - full keyboard use;
  - AccessKit labels ("IMG_4862.HEIC, 4032 by 3024, 12 March 2024, 3 stars");
  - focus ring;
  - honours reduced motion and high contrast.

**Advanced features (present but not in the way)**
They live in a **View** menu, the command palette (`Ctrl+K`), the bottom bar's overflow button and
Settings → Gallery. None show on the default screen.
- **Details view:** a list with columns that can be sorted, resized and reordered: name, type,
  dimensions, size, dates, camera, lens, exposure, ISO, focal length, rating, label, colour profile,
  bit depth, path. Profiles (§11) add domain columns: **FITS header fields** (OBJECT, FILTER, EXPTIME,
  GAIN, CCD-TEMP, DATE-OBS) and **star metrics** (FWHM, eccentricity, star count, background) for
  astronomy; pixel size and channels for microscopy; modality and series description for DICOM;
  temperature range for radiometric thermal images. Domain columns are also query fields
  (`filter:Ha exptime:>=300 fwhm:<3`).
- **Info overlay on tiles:** exposure, dimensions or any chosen field, set per layout.
- **Filter bar** (`Ctrl+F` or `/`):
  - name text;
  - format, date range, rating, label, orientation, minimum size, camera or lens;
  - "has GPS", "animated", "HDR".
  Shown as removable chips.
- **Group by** date (day, month, year), folder, camera, format or rating, with sticky group headers.
  Month headings use the locale's year-month form ("May 2024", "Květen 2024", "2024年5月"),
  capitalised. A
  view at the top stays at the top when capture dates arrive and regroup it.
- **Recursive view:** include subfolders, flattened, with a depth limit.
- **Stacks:** collapse RAW+JPEG pairs and bursts into one tile (§6.3), with a badge showing the count.
- **Cull from the grid:**
  - ratings `0`–`5`, labels `6`–`9`, pick/reject flags;
  - "next unrated" (§6.3 keys apply in the gallery);
  - filter to picks.
- **Compare:** selecting 2–4 images and pressing `c` opens compare (§6.3).
- **Batch:** run a user command or "open with" on the selection; copy the list of paths; batch
  convert (M2).
- **Thumbnail quality:**
  - prefer embedded previews (fast) or always render from the image (accurate);
  - colour-managed thumbnails;
  - HDR thumbnails on HDR displays;
  - show the clipping indicator on tiles.
- **Show hidden files** and **show non-image files.**
- **Per-folder view settings** (layout, size, sort, grouping), or one global set.
- **Cache:** size limit, location and "clear"; with `specular --thumbnail-cache` to warm it from the
  command line.
- **Advanced switching:** `Esc` at the end of the viewer's `Esc` ladder (§11) opens the gallery (off by
  default); opening a folder starts in the gallery; a single image opens in the viewer.

**Performance (budgets on the §12 reference laptop)**
- Folder of 10,000 images: grid shown in under 100 ms. The listing streams, and tiles fill in from the
  cache in under 1 s for a full screen.
- Scrolling at the display's refresh rate with 100k items: virtualised, and one draw call per
  atlas page.
- **A frame never waits.** The draw thread only `try_lock`s the thumbnail engine and the job queue
  (cancelling scrolled-away work included); when a lock is busy it retries next frame. No UI-thread
  check takes a lock shared with niced workers (e.g. the similar-shots idle check uses
  `Engine::try_idle` and skips when busy), and folder covers are listed on two low-priority
  "gallery-cover" threads behind a channel that never blocks, not on the loader pool.
  - Thumbnails with alpha are premultiplied (in linear light) on the worker, never on the draw
    thread; opaque ones skip the scan.
  - Uploads per frame: at most about 2 ms of predicted time and 4 MB, through one staging belt with
    one submit.
  - A frame that has to create an atlas page uploads nothing more. New pages (2048², 16 MB) are made
    on a helper thread: one batch as soon as a listing is in (before the frames that upload; a batch
    overlapping uploading frames costs a submit 11–45 ms), and more only at rest: every page the
    folder can use, up to the budget, each cleared there with a one-texel copy so wgpu's zero-fill
    never lands in a frame. Another thread alone is not enough: when a page needs a new device-memory block (about every 128 MB),
    the draw thread's writes and submit wait 15–60 ms on that allocation (RADV), so pages are never
    made mid-scroll.
  - The thumbnail wish list is handed to the engine's feeder thread, so the frame never takes the
    engine's or the job pool's locks (niced workers holding them would invert priority). Building
    the list is bounded (2048 requests, 1 ms past the visible tiles), and the metadata scan never
    passes over the whole folder in one frame.
  - Per-frame UI writes (globals, instances, glyph rows) go through a reused staging belt, never
    `queue.write_buffer`, which creates a staging buffer and takes the allocator lock.
  - The metadata scan for sort and filter works in chunks of at most 1024 requests and 8192 entries
    per frame.
- **Prefetch follows the scroll speed.** It looks ahead by 0.5 s of scrolling, clamped to 2–12
  viewports. Above 2.5 viewports/s, nothing behind the view is fetched or uploaded. Work that has
  scrolled out of view is cancelled.
- **Quality drops before frame rate.** While scrolling fast, at most 8 new captions are laid out per
  frame, within 2 ms (the rest next frame); the caption cache is preallocated to 4096 and cleared,
  never grown mid-scroll, since faulting fresh pages costs 18–90 ms under memory pressure, and tiles show placeholders until their uploads catch up.
- An uncached screen of 12 MP JPEGs gets sharp thumbnails in under 300 ms. Embedded previews, DCT-scaled
  JPEG decode and reduced-resolution RAW/HEIC paths are used; the scheduler sends each to the cheapest
  path (§12.3.1).
- **Memory:** a bounded GPU atlas (BC7 or ASTC where supported, §12.3); thumbnails outside a window
  around the viewport are evicted to the disk cache.
- **Cache:** content-hashed, in the platform cache directory, and shared with the Library and the OS
  thumbnailer integration (`--thumbnail`). A sandboxed format is thumbnailed through the decode host,
  as when it is viewed. Opaque thumbnails are stored as JPEG, thumbnails with alpha as PNG (lossless,
  keeping the colour under transparent pixels).
- **Background priority:** thumbnails decode in their own pool, below the viewer: nice +10 per worker
  on Linux, `BELOW_NORMAL` on Windows, QoS capped at Utility/Background on macOS.
- **Transparency:** thumbnails with alpha are drawn over what `viewing.transparency` selects, as in
  the viewer: a screen-fixed checkerboard in the theme's colours with the same 8 logical px cells,
  black, white, or the tile itself. An opaque embedded preview never stands in for an image with
  alpha.


### Layout

```
┌──────────────┬───────────────────────────────────────────┬──────────────────┐
│ Locations    │  [search / filter bar]                    │ Metadata         │
│  ▸ Pictures  │  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐       │  EXIF / DICOM /  │
│  ▸ Projects  │  │    │ │    │ │    │ │    │ │    │       │  IFC properties  │
│ Collections  │  └────┘ └────┘ └────┘ └────┘ └────┘       │ Histogram        │
│ Tags         │  ┌────┐ ┌────┐ ┌────┐ ┌────┐ ┌────┐       │ Rating ★★★★☆     │
│ Studies      │  │    │ │    │ │    │ │    │ │    │       │ Tags             │
│  ▸ PACS      │  └────┘ └────┘ └────┘ └────┘ └────┘       │                  │
└──────────────┴───────────────────────────────────────────┴──────────────────┘
```

### Behaviour

- **Locations, not imports.** Specular indexes folders where they already are and never moves or copies files.
- Background indexing with `notify` watching. **Viewer mode never waits on the library.**
- Grid, list and details views. The grid is virtualised to 100k+ items at 60 fps.
- **Thumbnails:** a content-hashed cache (file size + mtime + partial hash) in the platform cache dir.
  Uses embedded previews wherever they exist (EXIF, RAW, DICOM icon, DWG/RVT/SKP preview); otherwise
  renders one, including for STL, IFC and DXF.
- **Search:** filename, path, date, format, dimensions, camera/lens, rating, tags, colour label; DICOM
  modality, body part, study date, patient, description; drawing title-block attributes and IFC project
  name. Full text via FTS5. Smart collections are saved searches.
- **Ratings, tags, labels** go in the DB, optionally mirrored to XMP sidecars.
- **Studies** is the medical browser: patient → study → series, local and remote (PACS nodes).
- **RAW + JPEG pairs** and **burst stacks** shown as single items (§6.3).
- **Map view:** photos with GPS on a map, clustered; a GPS panel in the info sidebar with image
  direction; place names from offline reverse geocoding (§6.7) indexed, so "Lisbon" is searchable.
  Drone photos show a **yaw arrow** and relative altitude from DJI XMP, and a selection exports to
  **KML/KMZ** (§6.9). Map tiles are remote, so the map shows positions only (no base map) in
  no-network mode (§11).
- **Shell thumbnails** for science formats: the OS thumbnailers (§11) render FITS and XISF with the
  auto-stretch, and DICOM, NIfTI, MRC and DM3/DM4 with their default display, on Windows, macOS and
  Linux.
- **Remote locations:** S3/GCS/Azure buckets and HTTP folders can be added as locations (§12.8).
- **Duplicates and similar shots** as their own categories, with best-pick suggestions and a guided
  cleanup; see below.
- **Local AI (optional download, off by default, never leaves the machine):** text-to-image search,
  similar images, auto-tags, OCR, people and culling scores; see below.

### Duplicates and similar shots

Two separate categories in the sidebar (and the gallery's View menu), each with its count and the
space that cleanup would reclaim, e.g. "Similar shots: 312 groups, 4,180 photos, 38 GB":

- **Duplicates**: the *same image*, whatever the format, size or container.
- **Similar shots**: *almost the same image*: bursts, bracketing, the ten attempts at one pose,
  the same scene re-shot minutes later, phone "live" frames. Typically one or two are worth keeping.

**Detection** works on decoded pixels, not bytes, so HEIC↔JPEG, RAW↔its camera JPEG, PNG exports and
resized web copies match. Tiers, cheapest first; tiers 0–2 need no AI pack:

| Tier | Signal | Catches |
|---|---|---|
| 0 | Size + BLAKE3 of the file | Byte-identical copies |
| 1 | Hash of the decoded pixels at a canonical small size, after orientation | Same pixels in another container |
| 2 | 64-bit DCT perceptual hash + 256-bit difference hash + colour moments, from the thumbnail decode | Re-encodes, resizes, small crops, format changes |
| 3 | Image embedding (AI pack) | Edited versions, heavy crops, filters, screenshots of a photo; most similar shots |
| Verify | SSIM on 256 px thumbnails, for candidate pairs only (duplicates need 0.90 by default) | Keeps false matches out |

Tiers 0–2 and the SSIM check are built (`specular-similar`); tier 3 is a hook until the AI pack exists.
Only files whose size collides with another's are BLAKE3-hashed.

- **Metadata hints** raise or lower confidence: capture time with sub-seconds, camera serial, burst
  and bracketing tags from the maker notes, XMP `DocumentID`/`DerivedFrom`, GPS and focal length.
- **Candidates** come from multi-index hashing (or a BK-tree) over the 64-bit hashes, and from a GPU
  brute-force pass over embeddings; both answer in milliseconds at 100k+ files.
- **Duplicates** are pairs above a strict threshold. **Similar shots** are clusters of a
  similarity graph that combines visual distance with time and place distance, so the same scene
  shot a year apart stays separate unless the user widens the window. A **sensitivity slider**
  (tighter ↔ looser) regroups live.
- Without the AI pack, similar shots are found from tier 2 + time + EXIF; the pack improves recall.
- **RAW+JPEG(+XMP) pairs** and stacks count as one unit everywhere: grouped, scored and discarded
  together.

**Best-pick suggestions.** Each group ranks its members and marks the top one or two as *keep*:
- **Duplicates:** original over derivative, RAW over JPEG, then higher resolution, higher bit depth,
  less compression, metadata present, oldest.
- **Similar shots:** a quality score from sharpness on the subject (faces, the focus point, or
  saliency), eyes open and in focus, motion blur, exposure and clipping, noise, then resolution.
  The AI pack adds expression and composition terms.
- **The user's own data always wins:** any image with a rating, pick flag, label, tag or edit sidecar
  is kept and never suggested for discarding.
- Every suggestion says *why* ("sharpest eyes", "only one without blur", "RAW original").

**Review and cleanup** (opening a group, or `Enter` on the category):
- **Group view:** the suggested keepers large on top; the rest as a strip below, ordered by score,
  each with a red/green keep/discard badge.
- **Synchronised comparison:** any 2–4 members side by side with zoom and pan locked, jumping to
  100 % on the face or focus point (`z`), and a flicker key to spot the difference (§6.3).
- **Keys:** `p` keep, `x` discard, `u` undecided; `Space` accepts the suggestions for this group and
  moves to the next; `n` "not similar" splits a member off; `s` skips the group.
- **Bulk:** "Accept suggestions for all groups" (or for the selected ones) shows a summary of how many
  files go and how much space is reclaimed before anything happens.
- **What discarding does** (setting, default first): move to trash (undoable), move to a chosen
  folder, or only set the reject flag (and XMP) so another tool can act on it. **Nothing is ever
  removed automatically or without that confirmation.**
- **Remembered:** decisions, skips and "not similar" splits persist, so a group never comes back
  once resolved, and new files are only matched against what is left.
- **Progress:** a counter of groups left and space reclaimed; the work can stop and resume at any time.
- **Scanning:** a folder of up to 5,000 images is scanned on its own, 1.5 s after it last changed;
  larger ones when a category is opened. Signatures and decisions are kept in a `similar` folder
  next to the thumbnail cache. Signatures are computed once, inside the thumbnail engine, from the
  pixels as generated (before the cache's lossy coding), and stored with the cache entry, so a
  scan of a folder the gallery has shown decodes nothing.
- Zooming to 100 % (`z`) or choosing a channel view (Shift+1…5) shows the actual image, decoded and
  tiled as in the viewer, with zoom, pan and channel shared by the members shown; the thumbnail
  stands in while it loads.
- Decisions belong to the file's content as well as its name: a file renamed or moved outside
  Specular keeps its decision; files Specular itself moved or trashed are not followed.

In the gallery, similar shots can also be **collapsed into stacks** (the keeper as the cover tile, a
count badge), which extends the burst stacks of §6.3, and the filter bar gains `dupes:` and
`similar:` terms.

### Received media cleanup (R&D, not shipped)

**Status:** an R&D feature. It is not part of any release until it meets the
bar below, and it is reachable only through Advanced → Developer mode → "Experimental: received media
cleanup" (off by default). With the switch off, no category, facet or background work exists for it.

**The problem.** Phones fill up with media received through WhatsApp, Signal and Telegram:
- memes and greetings;
- forwarded videos and stickers;
- screenshots of other chats;
- the same viral image from five groups.

Deleting a whole messenger folder loses what matters: the grandchild's photo, the family dinner a
cousin took, a ticket. The feature separates the two and leaves every decision to the user.

**Principles**
- **The user's own photos set what matters.** Camera originals (camera EXIF, DCIM, the camera folder)
  define the people, places and things that matter. Received media are judged against them.
- **Every file gets reasons, not one opaque score.** A reason to keep a file beats every reason to
  remove it, as "the user's own data always wins" does in the duplicates cleanup.
- **A copy of one of the user's own photos is the safest deletion.** A sent or received copy of a
  photo the user took is redundant, because the original stays. Face and place protection must not
  keep it.
- **Repetition means junk.** An image received many times, or in several chats, is viral. One received
  photo of a known person is not.
- **Nothing is automatic.** Discarding uses the cleanup flow above: trash, move or reject flag, with a
  summary first, and decisions remembered by content.

**Where a file came from (no AI)**
- **Folders and names:**
  - WhatsApp media folders (Images, Video, Sent, Stickers, Animated Gifs, `.Statuses`);
  - Telegram's Images and Video folders and Desktop downloads;
  - Signal's saved-file names;
  - the messengers' file-name patterns.
  Every pattern is verified against freshly generated samples before use.
- **Without a telling path** (iOS saves to Photos; exports lose the folder): no camera EXIF,
  messenger-typical long sides, and the messenger's JPEG quantisation tables give "probably received",
  with lower confidence, and the reason says so.
- **Sent or received:** a Sent folder, or a match with an own original, means sent.
- **As a facet:** source is a library facet (`source:whatsapp`, `source:received`).
- **No messenger databases:** Specular never reads them. They are encrypted, and reading them would
  break the files-only promise.

**Signals**

| Keep (protects) | Needs |
|---|---|
| Rating, flag, label, tag or edit sidecar (always wins) | – |
| A face matching a person from the user's own photos | Face model, with People on |
| A person who recurs across distinct received images but is absent from own photos (asked once: "Keep photos of this person?") | Face model, with People on |
| Close in embedding space to own photos (home, pets, garden); medium strength | Embeddings |
| A camera-like photo received once, in one chat | Embeddings, counts |
| Documents (tickets, IDs, prescriptions, receipts): their own review bucket, never suggested in bulk | OCR, or zero-shot without it |

| Suggest (junk) | Needs |
|---|---|
| Copy of an own original (highest confidence) | Duplicate tiers 0–3 |
| Many identical or near copies across chats or dates ("Received 6 times") | Tiers 0–2 |
| Stickers, animated GIFs, status saves | Folder or name |
| Memes, greeting cards, text on images, chat screenshots, photos of screens, forwarded clips | Zero-shot prompts (localised bucket names) |
| Tiny, heavily compressed, blurry or dark (adds confidence, never decides alone) | Pixels, culling scores |
| Old and large (orders the review) | Metadata |

**Confidence.** Signals combine transparently into three tiers: Very likely, Probably and Unsure.
Protected files are listed separately with their reason. Thresholds start conservative.

**Local learning (optional, on the device).** Each keep or discard refits a small local model over
the same features, so Unsure shrinks over time. Nothing leaves the machine, and it can be reset.

**Review**
- **The category:** a sidebar entry next to Duplicates and Similar shots, e.g. "Received media:
  14,210 files, 9.3 GB · suggested 8,140, 5.1 GB".
- **Buckets** with count and size, largest first:
  - Copies of your photos;
  - Forwarded many times;
  - Videos (usually most of the space);
  - Memes and greetings;
  - Screenshots;
  - Stickers and GIFs;
  - Low quality;
  - Documents (review);
  - Unsure.
- **Confidence:** a slider per bucket regroups live, with the reclaimable space alongside.
- **Kept, and why:** protected items in a collapsible strip with their reason ("Anna appears",
  "Looks like your garden"). Overriding takes one key.
- **Keys and bulk** as in the duplicates cleanup (`x`, `p`, `u`, `Space`). The grid is denser, and
  videos show a three-frame strip.
- **Move to this computer:** archives the files on the computer, then deletes them from the phone.

**Reaching the phone.** The phone is what fills up, so a deletion must reach it:
- **Synced folders** propagate deletes. The review warns that the trash does not stop that.
- **A phone attached by USB (MTP)** has no trash, so "Move to this computer" is the default there,
  and the archive copy is the undo.
- **Exports and backups** only clean the copy. The review says so.
- **A mobile companion** is outside scope (§1).

**Risks**
- **Precious items discarded by mistake.** Mitigated by protection outranking junk, the
  recurring-person rule, and undo or the archive.
- **Faces are sensitive.** The feature follows the People switch, and without it the review says
  recall is lower.
- **Greeting-image culture varies by country.** Prompts are checked on varied samples.
- **There is no ground truth.**

**Phasing**
- **P0:** source detection, the facet, type buckets, copies of own originals, viral counts and the
  review. It needs no AI.
- **P1:** zero-shot buckets and near-own-photo protection.
- **P2:** face protection.
- **P3:** documents from OCR.
- **P4:** local learning.
- **P5:** moving files off a USB-attached phone.

**Bar to ship.** Measured on generated and consented corpora; the user's real files are read only in
place.
- At least 99% of "Very likely" suggestions are accepted on review.
- No protected class ever appears in "Very likely".
- About 1,000 files a minute of review throughput.
- Until all three hold, it stays behind the developer switch.

Open:
- how the phone's media reaches Specular (sync, MTP, export);
- recurring-person protection on by default;
- whether local learning is wanted;
- whether documents are protected or reviewed.

### Finding images: query, facets, grouping

- **Typed queries** in the filter bar, also editable as chips:
  `iso:>3200 lens:"24-70" taken:2025-06 is:raw rating:>=4 -tag:rejected`, `dupes:any`,
  `similar:this`, `text:"invoice"` (OCR), `"red car at night"` (AI pack).
- **Facets** in the sidebar with counts (camera, lens, year, place, format, people, tags); clicking
  one narrows the view.
- **Group by** date and **events** (clusters by time gaps, split on GPS jumps), place (offline reverse
  geocoding, §6.7), camera, lens, folder, format, orientation, colour label, dominant colour, and,
  with the AI pack, people, objects and scene.
- Saved queries are smart collections.
- **Built for the Gallery** (`specular-query`, no UI dependency, for the Library to reuse):
  - Fields: `rating`, `camera`, `lens`, `iso`, `date`, `ext`, `w`, `h`, `has:` (gps, alpha, hdr, …),
    `tag`, `label`; operators `= > >= < <=` (also `iso:<800`), comma lists, quoted values, `-` to
    negate; plain words match the file name; field names stay English in every locale.
  - The filter bar is a toolbar for screen readers: a search field (marked invalid on errors),
    chips as pressed buttons (pressing removes), Facets (expanded state) and Clear. Tab cycles field,
    chips, Facets, Clear, the facets panel and the grid. The match count is announced politely once
    the text has been still for 0.6 s.
  - Errors never block: each one is underlined in the filter bar, the first is described ("did you
    mean camera"), and the valid terms still filter. Active terms show as removable chips.
  - Facets (format, camera, lens, rating, year, orientation, has) dock at the trailing edge and the
    grid narrows. Counts are per file and disjunctive (a field's own terms don't narrow its counts).
    Format, camera and lens list the top 8 values plus any selected ones.
  - Counting does at most 4096 entries and 2 ms per frame. A recount started only because more
    metadata arrived runs at most every 0.75 s.
  - Events: a new event after a 3 h gap in capture time (mtime when there is none), or a move of
    more than 50 km between consecutive geotagged photos. A move counts when the next geotagged
    photo confirms the new place; at either end of a run, where nothing can confirm it, it must be
    possible at 1200 km/h or less. A single stray fix is ignored, positions are followed photo by
    photo (a road trip stays one event), photos without GPS never split and stay with their
    neighbours, and the cut goes at the longest pause before the move. Headers name the place
    (offline reverse geocoding, §6.7) when the metadata settings allow; otherwise the dates only.
  - `has:gps` and capture dates come from the Exif of JPEG, TIFF, PNG, WebP, HEIC/AVIF and JPEG XL,
    located without decoding (box and chunk headers plus the Exif bytes only). Brotli-compressed
    (`brob`) JPEG XL Exif is decompressed on the way.

### Local AI pack

Optional download, off by default; nothing ever leaves the machine.

| Feature | Model class | Notes |
|---|---|---|
| Text-to-image search, similar images, auto-tags, tier-3 matching | SigLIP 2 B/16-256 by default (B/32-256 faster; PE-Core-S16-384 English only, coming) | One 768-d f16 vector (1.5 KB) per image; 100k images ≈ 150 MB, searched exactly on the GPU. Queries in any of SigLIP 2's languages. Tags are a localised vocabulary matched zero-shot. |
| Text in images | PP-OCRv6 small (detection + recognition), per-script recognisers as add-ons, Tesseract LSTM for Hebrew (coming) | Makes screenshots, receipts and scans searchable |
| Culling scores | Sharpness, eyes open (MediaPipe Face Landmarker, coming), eye focus, expression | Feeds the best-pick ranking |
| People | BlazeFace detection + SFace (or AuraFace) embedding, clustered (coming) | Grouping faces into people is **opt-in per library**; "Delete all face data" asks first, names what goes and says the photos stay; deletable per person; never synced |

- **Runtime:** our own `specular-infer` (wgpu op set, CPU reference and fallback); choosing between
  them is a `PathSelector` path (§12.3.1). Runs on an NPU when one is reachable.
- **Choosing models:** each feature
  lists its models in a model manager (Settings › Local AI); the manifest's `[[choice]]` entries
  name each model, mark the feature's default, and give its languages, training data (and
  whether its terms are stated) and host. The user picks the model per feature ("Model":
  Default or a model by name); Default is the manifest's default wherever it fits the machine's
  memory and GPU, else the nearest smaller model. A model a later version runs is listed as
  coming, with the same facts and no button. Switching downloads the new model and re-indexes in
  the background; the old one is removed when the new one is ready.
- **Weights** download on demand, verified by hash.
- **Sideloading (offline and air-gapped sites):** a model file copied in by hand (Settings → Local AI →
  "Install from file", or a folder named in managed deployment) is checked against the sha256 in the
  manifest shipped with Specular and then used exactly as a downloaded one. No network is needed at
  any point, and the manifest lists the files and hashes so IT can fetch them separately.
- **Per profile:** the models suggested follow the active profiles (§11, "User profiles"): photo
  features for General; OCR and text search for Engineering; none of the photo culling scores are
  offered for Medical, and Specular ships no medical or diagnostic model (§1, Non-goals).
- **Scheduling:** a background job working from cached thumbnails (no second decode), paused while the
  user browses, following the battery and thermal rules.
- Removing the pack drops its tables.

### Storage

SQLite (WAL) in the platform data dir:
`files`, `folders`, `thumbs`, `tags`, `file_tags`, `collections`, `dicom_patients`, `dicom_studies`,
`dicom_series`, `dicom_instances`, `pacs_nodes`, `fts_index`, plus `image_hashes`,
`match_groups` (duplicates and similar shots, with decisions) and, with the AI pack, `embeddings`,
`faces`, `ocr_text`. Schema migrations are versioned in code.

---

## 11. Interaction and keyboard

### Two layers, one action system

All behaviour is an `Action` (e.g. `NextImage`, `ZoomTo(1.0)`, `WindowPreset(Lung)`). Keys, mouse,
menus, the palette and the CLI dispatch actions. Keymaps are per **mode** (viewer, library, medical,
engineering-2D, engineering-3D, compare), and the active mode follows the document type.

**For novices:**
- A menu bar (hideable) showing each item's shortcut, and tooltips with shortcuts.
- A **command palette** (`Ctrl/Cmd+K`) with fuzzy search over every action, showing its binding.
- A **cheat sheet** (`?`) that shows the bindings of the current mode.
- Standard OS shortcuts always work.

**For power users:**
- Single-key commands, counts (`5→` jumps 5 images) and chords (`r` then `1–5` to rate).
- An optional **vim preset** (hjkl pan, `gg`/`G`, `/` to search the library).
- A fully remappable `keymap.toml`. Conflicts are reported on load.
- Mouse gestures, trackpad pinch/rotate, and configurable wheel behaviour (zoom vs next image).

### Integration and automation

Specular has no plugins (§1), so it integrates outward instead.
- **Open with…** (`Ctrl+Shift+O`): a list of external editors (Photoshop, GIMP, Affinity, darktable,
  3D Slicer, FreeCAD…), detected or added by the user.
- **User commands:** shell commands with placeholders (`{file}`, `{files}`, `{dir}`, `{page}`), bound
  to keys or palette entries, with optional reload of the file afterwards.
- **IPC socket** (local only, per-user, authenticated by file permissions/named-pipe ACL): open files,
  navigate, query state, push images and overlays. The CLI uses it to talk to a running instance.
  - **Versioned protocol:** every message carries a protocol version; the spec is published with each
    release, old versions are kept working for at least a year, and additions never change the meaning
    of an existing field. A capability query lists what the running build supports.
  - **Official clients** (`clients/`): a **Python** package
    (numpy arrays in and out, overlays and ROIs as GeoJSON, state queries, waiting for user picks) and
    a **Rust** crate. Both are tested against the protocol in CI. They may carry a permissive licence
    (§14).
- **"Copy as command":** every action run from the palette or a menu can be copied as the equivalent
  CLI line or IPC call (Python client syntax), and the last N actions can be copied as a script. This
  is action recording without macros.
- **User command recipes:** shipped examples of user commands for common external tools, never the
  tools themselves: "run on series → load output" (export the series, run e.g. TotalSegmentator, open
  the label map it writes over the source, §7.3), plate solving with ASTAP or astrometry.net (§9.1),
  "open in Fiji / QuPath / napari / 3D Slicer".
- **SAMP** (Simple Application Messaging Protocol) hub client for astronomy: send and receive images,
  tables and sky coordinates with DS9, TOPCAT and Aladin. Local hub only; joining is a user action.
- **File lists:** read a list from stdin or a file; print marked files on exit (`nsxiv`-style).
- **Drag and drop** in (files, folders, URLs, images from browsers) and **out** (to other apps, as file
  or image). **Paste** an image from the clipboard to view it.
- **URL scheme** `specular://` for portals and EHRs (§7.5), validated and confirmed for non-local sources.
- **Platform integration:** file associations, Windows shell thumbnail provider for formats Windows
  can't preview, macOS Quick Look extension, freedesktop thumbnailer on Linux.
- **Deep desktop integration (opt-in):** Settings → Integration lists each option for the
  current platform.
  - Every option is per-user, needs no admin rights, and can be undone. Switching an option off, or
    uninstalling, removes every registration Specular made.
  - Code running inside the OS shell (Explorer, Finder, file managers) never parses files in that
    process. It uses only our own safe decoders, or hands the file to `specular-decode-host` in the
    sandbox.
  - COM and Objective-C glue that needs `unsafe` gets UNSAFE.md rows.
  - **Windows:**
    - register as a default app (Default Apps settings deep link) and Explorer "Open with";
    - a Windows 11 modern context menu through `IExplorerCommand` (sparse MSIX package identity),
      with the classic menu as fallback;
    - Explorer preview pane (`IPreviewHandler`) and thumbnails (`IThumbnailProvider`);
    - property handler, so Explorer can show dimensions, camera and date columns and search them;
    - jump lists (recent and pinned folders), taskbar progress for batch jobs, "Set as desktop
      background", the Share sheet;
    - optional WIC codecs, so other Windows apps can read JXL and other formats through our
      decoders.
  - **macOS:**
    - UTI/Launch Services declarations and a default-viewer option;
    - Quick Look preview and thumbnail extensions;
    - a Spotlight importer for metadata search;
    - Finder Quick Actions and Services menu entries (open in Specular, convert, compare);
    - Share extension, Dock menu (recent items), "Set Desktop Picture";
    - Shortcuts / App Intents (open, convert, query metadata).
  - **Linux:**
    - `.desktop` file with MIME types and `xdg-mime` default registration;
    - freedesktop thumbnailers;
    - D-Bus activation (`org.freedesktop.Application`) for single-instance launch;
    - file-manager actions (Nautilus scripts or extensions, Dolphin service menus, Thunar custom
      actions);
    - a GNOME Shell search provider and a KRunner plugin for library search;
    - xdg-desktop-portal for wallpaper, open-URI and file chooser when sandboxed;
    - Flatpak packaging with matching portal permissions.
- **Portable mode:** settings and caches next to the executable when a `portable` marker file exists.
- **Window options:** always on top, frameless "zen" window.
- **No-network mode** (Settings → Advanced, also a managed-deployment policy): no socket leaves the
  machine. Remote locations, PACS nodes, map tiles, HiPS, font and model downloads are off and say so
  where they would appear. The first run never needs the network in any mode (§1, principle 10).
- **Updates:** Specular sends no telemetry (§1). If an update check exists, it only fetches the public
  release list (no identifiers, no usage data), it can be switched off, and it is off in no-network
  mode and in managed deployments by default.
- **Managed deployment:** signed MSI (Windows) and PKG (macOS) for machine-wide installs, a per-user
  installer that needs no admin rights, Flatpak and AppImage on Linux, plus portable mode. Policies
  (no-network mode, default profiles, file associations, model folder, update check) can be set by
  MSI properties, a macOS configuration profile or a system-wide TOML.
- **Renderer info:** Settings → Advanced shows the active GPU backend, adapter, driver and display
  server (Wayland/X11), with a switch to the fallback backend (GL) for broken drivers.

### Settings window

`Ctrl+,` (macOS `Cmd+,`), the ⚙ toolbar button, the menu, or "Settings" in the palette opens one
**Settings** window. It is a real, polished window, not a raw config file editor.

- **One schema.** Each setting is declared once in `specular-settings` (key, type, default, range,
  label and help-text message IDs, category, "needs restart" flag). The window, the palette
  ("Set: wheel behaviour → next image"), `settings.toml`, `specular --set key=value` and the generated
  documentation all come from it, so they never drift apart.
- **Live.** Changes apply immediately (no Save/Apply button), with undo (`Ctrl+Z`) inside the window.
  The few settings that need a restart say so and offer "Restart now".
- **Search** across every setting's label, help text and key, in the current language and in English.
- **Modified markers** on changed settings, a per-setting "Reset", and "Reset section".
- **Import/export** of settings and keymap as one file. The file is watched and reloaded, keeps the
  user's comments (`toml_edit`), and unknown or invalid values are reported inline rather than
  discarded.
- **Built-in config editor** for power users ("Edit settings.toml", also for `keymap.toml`), instead of
  handing the file to an external editor:
  - **Editing:** syntax colouring from the theme tokens (tables, keys, strings, numbers, booleans,
    dates, comments), line numbers, current-line highlight, bracket and quote matching, undo/redo,
    find, and keyboard-only operation.
  - **Linting** runs as the user types, with the same parser and schema the loader uses, so the editor
    and the loader always agree. It checks:
    - TOML syntax;
    - unknown sections and keys, with a "did you mean" suggestion;
    - wrong types, values out of range and invalid enum values, listing the allowed values;
    - duplicate keys;
    - for keymaps, unknown actions and conflicting bindings.
  - **Showing problems:** each one is underlined in place and listed in a problems pane (line, message,
    jump to it). Messages are localised and also read out to screen readers.
  - **Saving** is refused while any problem remains, and the editor says why. A clean file is written
    atomically, exactly as typed, so comments and layout survive, and it then applies live.
  - **Conflicts:** if the file changes on disk, or a setting changes in the window, while the editor
    has unsaved edits, the editor offers "Reload" or "Keep mine". It never silently overwrites.
  - **Revert:** discards unsaved edits.
- **Settings presets** (later): named sets of settings, e.g. "Photo culling", "Reading", "Radiology",
  switched from the palette. Not the same as **user profiles** (below), which decide which tools
  exist; a preset only changes values.
- Fully keyboard-navigable and screen-reader labelled; lays out right-to-left in RTL languages.

Sections (Stage 1 ships the first six; others appear as their features land):

| Section | Settings |
|---|---|
| General | **Interface language** (live switch, no restart; "System default" follows the OS), region formats (dates, numbers, units — defaults from the language, overridable), theme (system / light / dark), what to open on start, single instance, file associations |
| Profile | Which capabilities are on (General, Engineering, Medical, Science and their sub-capabilities), applied live; see "User profiles" below |
| Viewing | Initial zoom (fit, fit if larger, 100%, fill), wheel behaviour (zoom vs next image), zoom animation and speed, background colour/checkerboard, pixel grid threshold, smoothing at high zoom, fullscreen on open, toolbar auto-hide delay, toolbar contents (order, spacers, separators) |
| Browsing | Sort order (natural, date taken, modified, size, type), loop at folder end, include subfolders, hold-to-accelerate speed, confirm before trash |
| Gallery | Layout and thumbnail size, captions and badges, sort order and grouping, per-folder view, open folders in the gallery, Esc opens the gallery, arrow keys wrap around at the ends (off by default; a held key still stops at the end), show hidden files / other files / archives (off by default), subfolder depth, stacks, info overlay, prefer embedded thumbnails, sharper raw thumbnails (edited raws developed at 1024 px, on by default), read and write rating sidecars (XMP), thumbnail cache size and location, clear cache |
| Colour | Colour management on/off, display profile (auto or file), rendering intent, dithering, HDR behaviour (later) |
| Metadata and privacy | Show the info panel on open, Summary vs All fields default, **reverse geocoding on/off**, hide GPS on screen, units (metric/imperial) |
| Performance | Where images are processed (Automatic, Prefer GPU, CPU only) with the "How Specular runs on this computer" panel (§12.3.1), RAM and VRAM cache budgets, prefetch depth, decoder threads (auto or fixed), GPU adapter choice, persistent display cache size and location, per-path overrides (auto/on/off), frame-time HUD |
| Accessibility | Reduce motion, increase contrast, UI text size and scale (boxes that hold text grow with it, the info panel widens, a settings row puts its control under the text when they don't fit side by side, and a shortened label keeps its full accessible name; in the gallery the captions, group headers, filter bar and quick look caption grow while thumbnails keep their size), an **interface zoom** from 75% to 300% independent of the OS scale, focus ring always visible, screen-reader verbosity, **announce the value under the cursor** (pixel value, HU, temperature or coordinates, read out after the pointer or keyboard cursor rests), colour-vision simulation (§6.2) |
| Keys | The key binding configurator (below) |
| Advanced | Portable mode, **no-network mode**, update check, log level, developer mode (which also shows the switches for R&D features such as received media cleanup, §10), renderer info and fallback backend, hardware report (the `--hw-report` data in a dialog, with Copy; it is never sent anywhere, the user copies it), local audit log (§7.4), reset everything |

### Languages and localisation (from day one)

Specular is localised from the first release, including right-to-left languages. Retrofitting this
later costs far more than building it in, so Stage 1 builds the whole pipeline.

**Strings and translation**
- All user-visible text lives in **Project Fluent** (`.ftl`) catalogues, one folder per locale, embedded
  in the binary. Fluent handles plurals, gender, grammatical case and per-language selectors, so
  translators aren't forced into English sentence structure. Code refers to message IDs only;
  a lint fails the build on string literals passed to text widgets.
- **Language switching is live**: changing the language re-lays out the whole UI on the next frame,
  with no restart. The first run uses the OS language (`sys-locale`), negotiated against available
  catalogues with fallback chains (e.g. `de-AT` → `de` → `en`). Portuguese has two catalogues, `pt-PT`
  and `pt-BR`, each falling back to the other before `en`; a bare `pt` or another `pt-*` tag resolves to
  `pt-PT`. Missing messages fall back per message, never to an empty string.
- **Formatting via ICU4X**: dates and times, numbers (`1,5 MB` vs `1.5 MB`), units, percentages, lists,
  relative times, and **locale-aware sorting** for file names (natural sort runs on top of the ICU4X
  collator). Language and country names in the language picker and the metadata panel come from ICU4X
  display names, so "Portugal" is shown as "ポルトガル" in Japanese.
- **The language picker** lists each language in its own name ("Deutsch", "العربية", "日本語") with its
  translation completeness, and is reachable without reading the current language (a globe icon,
  also in the palette as "Language").
- **Translation workflow:** `.ftl` files in the repo with an open translation platform (Weblate or
  Pontoon) synced by CI; a completeness and placeholder-consistency check per locale; screenshots of
  every screen in every language generated from the widget gallery for translator context and review.
- **Pseudo-localisation** locales are built in for testing: `en-XA` (accented, about 40% longer) and
  `ar-XB` (mirrored RTL). CI renders the widget gallery in both and fails on clipped or overflowing text.
  The language lists offer them only with Advanced → Developer mode on (or while one is in use).

**Right-to-left languages (Arabic, Hebrew, Persian, Urdu)**
- **Text:** full Unicode bidi (UAX #9) and shaping with `parley`, including mixed RTL/LTR runs, numbers
  inside RTL text, and file names in either direction. Unicode isolates (FSI/PDI) wrap every
  interpolated value (file names, camera models, paths) so they can't reorder the surrounding sentence.
- **Mirrored layout:** the toolkit lays out with logical properties (start/end, never left/right).
  In RTL the window mirrors: menus, panels (the info panel moves to the left), toolbar order, list and
  table columns, scroll bars, text fields, the command palette and tooltips.
- **Icons:** directional icons (back/forward, undo/redo, "next" chevrons, indent) mirror; non-directional
  ones (play, zoom, rotate, checkmarks, media) do not. Each icon declares which it is.
- **Navigation follows reading direction where users expect it:** in RTL, the "next image" toolbar
  button and edge arrow are on the left. Arrow keys keep their physical meaning by default (`→` is
  next), with a setting to follow reading direction; reading mode already handles RTL page order (§6.4).
- **Images are never mirrored**, and neither are coordinates, rulers, histograms, medical or
  engineering views: only the chrome around them.
- **Text input** (search, rename, palette) handles RTL caret movement, selection and IME correctly.

**Scripts and fonts**
- Inter for Latin, Greek and Cyrillic; bundled Noto Sans for Arabic, Hebrew, Devanagari, Thai and
  others; system fonts are preferred where present (`fontique`), with Noto Sans CJK downloaded on
  demand or taken from the OS for Chinese, Japanese and Korean, with correct per-locale glyph variants
  (Han unification: `zh-Hans`, `zh-Hant`, `ja`, `ko` select different forms).
- The glyph atlas and font fallback are per-script; text metrics come from each font, so line heights
  stay even when scripts mix.

**Launch languages:** English, Spanish, Portuguese (Brazil), French, German,
Italian, Dutch, Polish, Russian, Ukrainian, Turkish, Arabic, Hebrew, Persian, Hindi, Japanese,
Simplified Chinese, Traditional Chinese, Korean. Stage 1 ships English plus a first set with native
review (including Arabic and Hebrew to prove RTL); more follow as translators join. Catalogues today:
`en`, `ar`, `cs`, `da`, `de`, `es`, `fi`, `fr`, `he`, `it`, `ja`, `nl`, `pl`, `pt-BR`, `pt-PT`, `sv`,
`tr`, `uk`, `zh-Hans`. Machine-drafted catalogue files carry a marker comment until a native speaker
has reviewed them, and a test checks that every plural selector covers each CLDR category of its
locale.

### Key binding configurator (later stage)

`keymap.toml` stays the source of truth; the configurator is a visual editor for it (Settings → Keys,
or "Customise shortcuts" in the palette).
- **Every action listed**, grouped by mode (viewer, library, cull, compare, reading, medical,
  engineering-2D/3D), searchable by action name, description or key ("what does `Shift+F` do?").
  A **profile filter** shows the bindings of one capability or of everything that is on; bindings of
  capabilities that are off are listed dimmed, marked inactive, and don't count as conflicts (§11,
  "User profiles").
- **Record a binding** by pressing it: single keys, modifiers, **sequences/chords** (`g` `g`, `r` then
  `1`), counts, mouse buttons, wheel with modifiers, trackpad gestures and gamepad buttons. Sequences
  have a configurable timeout; a pending sequence shows a small hint of possible continuations
  (which-key style).
- **Conflict detection while typing:** clashes in the same mode, prefix clashes (binding `g` when `g g`
  exists), shadowing between global and mode keymaps, and OS-reserved shortcuts (e.g. `Cmd+Q`,
  `Alt+Tab`, IME keys). Each offers "swap", "replace" or "cancel".
- **Physical vs logical keys:** bindings default to the key's position (so they survive keyboard
  layouts), with a per-binding option to follow the printed character instead. The editor shows the
  keys as printed on the user's current layout. Today, digit-row bindings already match by position:
  `Shift+1…5` works on AZERTY and German layouts, and a plain digit whose typed character is unbound
  falls back to the digit.
- **Presets:** Default, IrfanView, XnView, FastStone, Lightroom-style culling, Photo Mechanic, vim, OsiriX
  (medical), AutoCAD/Navisworks (engineering). Users can start from one and diff against it
  ("changed from preset" markers, reset per binding, per mode or all).
- **Mouse mapping:** each mouse button (including side buttons) and button + modifier can be mapped to
  a tool per mode (e.g. right button = window/level, middle = pan, side = next series), saved in
  `keymap.toml` and kept across sessions. **SpaceMouse** and gamepad axes map to pan, zoom and orbit.
- **A visual keyboard map** showing what each key does in the selected mode, with free keys highlighted.
- **Import/export** keymaps as TOML; share a single mode's bindings. Hand-edited `keymap.toml` changes
  are picked up live and validated, with errors shown in the editor.
- Changes apply immediately; menus, tooltips, the cheat sheet and the palette always show the current
  bindings, because they all read the same action registry.
- Fully keyboard- and screen-reader accessible; the recorder can be cancelled with `Esc` held, so `Esc`
  itself can be bound.

### Cull, compare and reading keys

| Key | Action | Key | Action |
|---|---|---|---|
| `Ctrl+Shift+K` | Cull mode | `p` / `x` / `u` (cull) | Pick / reject / unflag |
| `Alt+1…9` | Copy to slot | `Alt+Shift+1…9` | Move to slot |
| `j` | Clipping warnings | `Shift+F` | Focus peaking |
| `Shift+I` | Pixel inspector | `Shift+H` | HDR ↔ SDR rendition |
| `d` (compare) | Difference view | `Ctrl+R` | Reading mode |
| `Ctrl+Shift+O` | Open with… | `Shift+G` | Histogram overlay |
| `Alt+G` | Pixel grid | | |

### On-screen viewer controls

Everything in the viewer can be done with the mouse alone. The controls are part of the auto-hiding
overlay chrome (§3.3): they appear on mouse movement and fade after about 1.5 s idle, but stay visible
while hovered or focused by keyboard.

**Overlay toolbar** (bottom centre, translucent with backdrop blur):

```
 ◀  ▶   │  −  [ 100% ▾ ]  +   ⤢ fit   │   ⟲  ⟳   │   ⓘ   ⛶   ⚙
prev next   zoom out/level/in            rotate      info  fullscreen  settings
```

| Control | Action | Behaviour |
|---|---|---|
| ◀ Previous / ▶ Next | `PrevImage` / `NextImage` | Disabled at the first/last image, or wraps around if "loop folder" is on. **Press and hold** repeats with acceleration, like holding the arrow key (previews only while flipping fast). The position `12 / 348` shows beside them; clicking it (or `Ctrl+G`) turns it into a field to type an image number, Enter goes there |
| ⛶ Fullscreen | `ToggleFullscreen` | The icon switches between "enter" and "exit" fullscreen. Also bound to `F11`, `Enter` and double-click on empty background (configurable); `Esc` exits once panels are closed and zoom is back at fit (see the `Esc` row below) |
| − / zoom level / + | Zoom out, zoom presets menu, zoom in | The level shows the current zoom; clicking it opens presets (fit, fill, 100%, 200%…) |
| ⤢ Fit | `ZoomFit` | |
| ⟲ / ⟳ | Rotate view | |
| ⓘ | Info panel | |
| ⚙ Settings | `Settings` | Opens the Settings window (also `Ctrl+,` / `Cmd+,` and the palette) |

**Enabled state.** Every action has one `enabled(&ViewerState)` rule, and the toolbar, edge arrows,
keyboard shortcuts, command palette, context menu and AccessKit all read it, so a control can never
look usable while its key does nothing (or the reverse):
- **No image** (empty window, empty folder, file still being opened before its header is read): zoom,
  fit, 100%, rotate, info, copy image/path, move to trash, loupe, navigator, zoom rectangle and pixel
  grid are disabled. Open, settings, palette, cheat sheet, language, theme and fullscreen stay enabled.
- **Header read, pixels still decoding:** info, copy path and trash are enabled, and zoom/fit/rotate work
  on the known dimensions. Copy image waits for pixels.
- **Decode failed:** the error card is shown; info (file and whatever metadata parsed), copy path and trash
  stay enabled; the pixel controls stay disabled.
- Disabled controls are dimmed and show no hover effect. They stay keyboard-focusable and announced as
  "unavailable", and their tooltip says why ("No image open"). In the palette they are listed greyed out
  with the same reason, so they can still be found. A disabled shortcut does nothing; it does not
  beep or show a toast.
- If an open panel's action becomes disabled (the info panel when the folder's last image is
  trashed), the panel closes.
- **Hidden** is separate from disabled: actions of a capability that is off (§11, "User profiles")
  are not shown at all, their keys are inactive, and AccessKit omits them; only palette search lists
  them, with a way to turn the capability on.

**Edge navigation arrows.** Large, subtle ◀ / ▶ arrows appear when the pointer nears the left or right
edge of the image area, in both windowed and fullscreen mode. Clicking them moves to the previous/next
image. They are hidden when there is only one image, and they never cover the image when it is zoomed and being panned.

**Fullscreen behaviour:**
- Windows and Linux use borderless fullscreen on the monitor the window is on (instant, no mode switch).
- macOS uses native fullscreen (its own Space) by default, with a setting for instant borderless fullscreen.
- In fullscreen all chrome hides, the background goes to the fullscreen background (black by default),
  and the toolbar and edge arrows reappear on mouse movement. The cursor hides after the same idle time.
- Zoom, pan position and the current image carry over when entering and leaving fullscreen.

All buttons dispatch the same `Action` as their keys, show their shortcut in a tooltip, have AccessKit
labels, and are reachable by keyboard focus. Domain modes add their own tools (W/L presets in medical
mode, layers and views in engineering mode).

**Customising the toolbar.**
- *In Settings → Viewing → Toolbar contents* (also opened by "Customise toolbar…" in the toolbar's
  context menu, the palette's Customise toolbar and the toolbar's screen-reader action): an ordered list with drag handles, Move up/Move down
  chevrons (disabled at the ends, absent on hidden rows), a ⋯ menu (Move to start, Move to end,
  Hide/Show, or Remove for spaces and separators; `Shift+F10` opens it) and a show/hide switch per
  button (mirrored in RTL).
  - The page shows its own preview of the bar only when the real bar is hidden (off, auto-hidden,
    fullscreen, gallery) or covered by the Settings sheet, decided each frame from the layout.
    Otherwise the real bar shows the changes, drawn above the dimmed backdrop with an accent outline,
    and doesn't auto-hide while the page is open.
  - Rows also move with `Alt+↑/↓` or the screen-reader actions Move up/Move down. Each move is
    announced and keeps focus on the moved row.
  - Flexible spaces and separators can be added any number of times.
  - "Reset to default" restores the default bar, and every change is one undo step.
- *Dragging on the bar, any time* (`viewing.toolbar_drag`, on by default): a press becomes a drag past
  6 px of movement or after a 0.4 s hold (never the hold on Previous/Next, which repeat when held:
  first after 0.4 s, then every 0.08 s; decided). The other buttons reflow live around an accent socket and the
  lifted button; `Esc` or a release off the bar puts it back, and a short click still presses it.
  Dragging never hides a button and nothing moves into or out of More; a drop is one undo step. It
  is off while a dialog is open and in the gallery.

**Overflow.** When the bar doesn't fit the window, the buttons that don't fit fold into a More (⋯)
button. Its popover lists them with icon, label and shortcut, aligned to More's side (mirrored in RTL).
It is a menu for screen readers, and works with `↑`/`↓`, `Home`/`End`, `Enter`/`Space` and `Esc`.

### Default keymap: global and viewer

| Key | Action | Key | Action |
|---|---|---|---|
| `→` / `Space` / `PgDn` | Next | `←` / `Backspace` / `PgUp` | Previous |
| `Home` / `End` | First / last | `Ctrl+K` | Command palette |
| `f` | Fit to window | `1` | 100% (1:1) |
| `2`–`4` | 200–400% | `+` / `-` / wheel | Zoom |
| Drag / arrows (zoomed) | Pan | `F11` / `Enter` | Fullscreen |
| `[` / `]` | Rotate view | `h` / `v` | Flip view |
| `i` | Info panel | `t` | Filmstrip |
| `Tab` / `L` | Viewer ↔ Library | `c` | Compare (2-up, synced) |
| `s` | Slideshow | `,` / `.` | Previous / next page, frame or slice |
| `p` | Play / pause | `Del` | Move to trash (undoable) |
| `Ctrl+Z` | Undo file operation | `Ctrl+C` | Copy image |
| `Ctrl+G` | Go to image number | `g` | Viewer ↔ Gallery (§10) |
| `Ctrl+Shift+C` | Copy path | `Ctrl+E` | Export view |
| `?` | Cheat sheet | `Esc` | Back out, one layer per press: close the open panel, palette, dialog or loupe; then reset zoom/pan to fit (animated) if zoomed; then exit fullscreen |

**Reaching the chrome by keyboard.** Tab/Shift+Tab move between controls; F6/Shift+F6 jump between
regions (toolbar, transport, info panel, insight indicator); past the last, the keyboard is back on
the image. The toolbar is one Tab stop: ←/→ move along it (mirrored right to left), Home/End go to
its ends, Enter/Space press, ↑/↓ open the zoom presets or the More menu. Previous/Next repeat while
Enter/Space is held, as when the button is held. Menus: ↑/↓, Home/End, Enter/Space, Esc. With focus
in the chrome, Esc closes a menu, then returns the keyboard to the image, then closes the info
panel. From the image, Tab and F6 enter the chrome only if the keymap doesn't bind them (Tab is
reserved for Viewer ↔ Library; when that lands, F6 remains the way in). The focused control carries
a focus ring and shows its tooltip; a click hands back to the pointer. In the insights panel Tab
cycles its buttons. An auto-hidden toolbar stays reachable: Tab or F6 brings it back, and it stays up
while it has the keyboard. Every dialog is a focus trap: Tab/Shift+Tab go round its buttons and
wrap, Enter activates the focused button (or the default one), Esc cancels, and closing returns
focus to where it was. In Settings the stops follow the settings themselves, not the last frame;
the toolbar-layout list is one stop (arrows inside it), then its Add buttons; dropdowns take
↑/↓/Home/End, Enter/Space commit, Esc closes. The info panel's All-fields rows are one stop:
↑/↓, Home/End, PageUp/PageDown move along them, the focused row scrolls into view, Enter or Ctrl+C
copies its value. In the gallery F6/Shift+F6 cycle the filter bar, the grid and the insight
indicator, skipping any not shown; the grid's arrow keys are unchanged; the focused tile shows its
ring only while the grid has the keyboard. In Settings ↑/↓ on a control move to the control
before or after it (↑ from the first returns to search); the first Tab from search goes to the
page's first control, or to the setting Settings was opened at; while a control has focus the
search field draws unfocused. A dialog's default button is shown by its fill; keyboard focus is a
2 px ring set 2 px off the button (≥ 3:1 against the sheet in both themes), so the two never look
alike. Open menus draw above toasts. In the go-to field Tab/F6 leave the field; the palette list
takes Home/End.
The palette and the language list have a Close (×) button, the Tab stop after their field; on
it Enter or Space closes, and the list's keys and typing go back to the field. While a Settings
control or Close has the keyboard, the search field draws unfocused but still takes typing and
input-method compositions (the input method stays on); a composition takes the keyboard back to
the field; while a dropdown is open no text is taken.

### Medical mode

| Key | Action | Key | Action |
|---|---|---|---|
| Right-drag | Window/level | `w` | W/L tool |
| `0`–`9` | W/L presets | `Ctrl+I` | Invert |
| Wheel / `,` `.` | Slice | `Shift+wheel` | Slab thickness |
| `m` | MPR layout | `3` | 3D volume view |
| `x` | Crosshair / reference lines | `Ctrl+M` | Measure |
| `p` | Cine | `Shift+T` | DICOM tags |
| `Shift+R` | MPR (built) | `Shift+S` | Cycle slab mode (MPR) |

`Shift+S` is also the engineering section box. The two never clash at once: the key belongs to the
mode of the document in view, and with a profile off its bindings are inactive (below).

### Engineering mode

| Key | Action | Key | Action |
|---|---|---|---|
| `Num 1/3/7` (`Ctrl` = opposite) | Front / right / top | `Num 5` | Persp ↔ ortho |
| `Num 0` / `z` | Zoom extents | `l` | Layers panel |
| `x` | X-ray | `Shift+S` | Section box/plane |
| `Alt+H` / `hh` | Unhide all / hide selected | `b` | Swap background |
| `Ctrl+M` | Measure (snapping) | `o` | Revision overlay |
| `w` | Walk / fly | `Shift+V` | Saved viewpoints |
| `Shift+B` | BCF topics | `Shift+D` | Model or drawing diff |

### User profiles

**Profiles decide what the app shows, never what it can open.** Specular serves photo, medical,
engineering and science users; without profiles each of them would
wade through the others' tools. A profile is a **capability set**, not a separate app or build.

| Capability | Always on? | Sub-capabilities | Brings |
|---|---|---|---|
| **General** | Yes | | Viewer, gallery, library, cull, compare, reading, photo inspection tools, the measurement basics of §6.8 (ruler, rectangle stats, line profile), output (§6.5) |
| **Engineering** | No | Drawings & BIM; **Inspection & thermal** | §8 tools and modes, §6.9 thermal and inspection, §8.6 NDT |
| **Medical** | No | | Medical mode, §7 tools, Studies, PACS, the not-for-diagnostic banner |
| **Science** | No | Microscopy & pathology; **Astronomy**; Electron microscopy; Remote sensing & spectral | §9.1 tools, domain columns, SAMP |

Several capabilities can be on at once (a vet radiologist who also does astrophotography turns on
Medical and Science → Astronomy). Turning on a parent turns on all its sub-capabilities; each can then
be turned off on its own.

**Choosing.**
- **First run** asks one question, "What will you use Specular for?", as a sheet over the empty window
  with General preselected and the others as checkboxes with a one-line description each. It is the
  only first-run question; choosing Medical shows the not-for-diagnostic acknowledgement on the same
  sheet (§7). "Skip" keeps General only.
- **Settings → Profile** changes it any time. Changes **apply live**, with no restart: menus, toolbars,
  palette and modes update on the next frame.
- The sheet and the Settings page are fully keyboard operable, screen-reader labelled and mirrored in
  RTL, like every other sheet.
- **Installers** offer file associations per profile (DICOM and NIfTI only with Medical, DWG/IFC/STEP
  only with Engineering, FITS/CZI only with Science); managed deployments set the profiles with a
  policy (§11, "Integration").

**What a profile controls.**
- which **modes** exist (medical, engineering-2D/3D, science views);
- **palette commands, menus, toolbars, on-screen controls** and **Settings pages**;
- **info panel groups** and the **default tools** of a view;
- the **file associations** offered at install;
- the **AI models** suggested (§10, "Local AI pack");
- **defaults** such as colour maps (perceptual for science), measurement units (HU, °C, µm, drawing
  units) and the not-for-diagnostic banner. A profile default never overrides a setting the user has
  changed.

**What it never controls: decoding.** Every format opens whatever profiles are on. When a file from a
capability that is off is shown, it gets **two tiers of tools**:
- **Tier 1, content-driven and automatic.** What is needed to see that file correctly appears for that
  image only, and goes away when a non-domain image is shown:
  - Medical: window/level and its presets, HU or modality value readout, orientation letters and
    scale, series slice scrolling, the tag browser, and the not-for-diagnostic banner;
  - Science (FITS, 16-bit microscopy): stretch, channels, scale bar;
  - Engineering (CAD): layers, units readout.
- **Tier 2, the workflows**, behind a small **toolbar chip** ("Medical tools", "Science tools",
  "Engineering tools") that shows while such an image is on screen. Tier 2 is the heavy tools: for
  Medical, MPR, 3D, measurements beyond the basic ruler, anonymised export, PACS and hanging
  protocols; for Science and Engineering, their equivalents. Clicking the chip, or invoking a hidden
  command from the palette, turns the capability on **for this session**. The first time, the chip
  also offers **"Always"**, which turns the profile on. That offer is one-time and dismissible, and is
  never a dialog.
- In the **gallery**, tier 1 and the chip follow the **selected image**, so a mixed folder never flips
  the whole app.

**Hidden is not removed.**
- **Palette search** still finds commands of capabilities that are off: they are listed greyed out with
  "Medical tools are off" and an action to turn them on (this session, or always).
- **Key bindings** of capabilities that are off are inactive, which frees those keys for others and
  removes clashes such as Medical `Shift+R`/`Shift+S` against other modes. The key binding
  configurator shows bindings per profile.
- **Hidden** is a third state beside enabled and disabled (§11, "Enabled state"): a disabled control
  is shown dimmed with a reason; a hidden one is not shown at all, except in palette search as above.

**Implementation sketch.**
- Each `Action` in `specular-input` gains a `Capability` (it already has a `Mode` and a `Group`);
  actions shared by all, and tier-1 actions, belong to General or carry a "content" flag that lets the
  document in view enable them.
- `enabled.rs` gains a **hidden-by-profile** state distinct from `Disabled`: availability becomes
  Enabled / Disabled(reason) / Hidden(capability), read by the toolbar, menus, palette, keymap
  dispatch and AccessKit alike.
- One settings key, `profile.capabilities` (a list), plus per-session overrides that are not saved.
- The AccessKit tree **omits hidden controls**, so screen readers announce only what is shown.
- **Screenshot goldens per profile** for the main chrome (viewer toolbar, menus, Settings, palette),
  in LTR and RTL.
- New strings are added in `en` only at implementation time; the other catalogues follow through the
  normal translation workflow (§11, "Languages").

---

## 12. Performance and rendering pipeline

**Goal: the fastest image viewer available**, measurably. We set the standard rather than chase other
viewers: the budgets below are the bar, and each gain is locked in by regression gates
(other viewers are not benchmarked). To get there, Specular **detects what the device offers at
runtime and uses the newest CPU and GPU capabilities available**, always with a portable fallback that
produces identical output.

### 12.1 Hardware capability detection

At startup (cached between runs, refreshed when hardware or driver changes) Specular builds a
`HardwareProfile`. It records:
- CPU: ISA extensions, core topology (performance vs efficiency cores), cache sizes, NUMA nodes.
- Memory: total and available RAM, unified memory (Apple Silicon, integrated GPUs).
- GPU: adapter, backend, wgpu features/limits, VRAM, Resizable BAR, hardware decode engines,
  compressed-texture formats, async compute queues.
- Storage: NVMe vs SATA vs network, and which I/O APIs are available. For remote sources (§12.8),
  measured latency and throughput per host or bucket.
- Display: HDR capability and current headroom (updated live), refresh rate, colour profile.
- Accelerators: NPU presence (for optional local AI features, §10).

Every hot path picks its fastest implementation from this profile. A debug view shows what was chosen
(`specular --hw-report`).

### 12.2 CPU

| Technology | Where it is used |
|---|---|
| **SIMD with runtime dispatch** | x86: SSE4.2 → AVX2/FMA → AVX-512 / AVX10 (VNNI, BW, VBMI). ARM: NEON → SVE/SVE2. The x86 baseline is x86-64-v3 (AVX2/FMA/BMI2, any CPU since 2013), so kernels written once with safe `std::simd` compile straight to AVX2 or NEON with no dispatch. Higher tiers (AVX-512/AVX10, SVE) are selected at runtime only for kernels where they measurably win, since that dispatch needs a small `unsafe` call. Used for entropy decoding helpers, IDCT, colour conversion, chroma upsampling, resampling, demosaicing, inflate, CRC/Adler, byte-swapping, DICOM rescale/LUT |
| **Hybrid core awareness** | Interactive decodes go to performance cores; background work (thumbnails, indexing, prefetch) goes to efficiency cores. macOS QoS classes, Windows thread priority + EcoQoS, Linux `SCHED_IDLE`/nice and cpusets |
| **Intra-image parallelism** | One image decoded on many cores: JPEG restart intervals and parallel scans, TIFF/JP2/HTJ2K tiles and code-blocks, JPEG XL groups, AVIF tiles, RAW strips, multi-frame DICOM frames, PNG with parallel filter/inflate where the stream allows. **Only when the scheduler predicts a real gain** (see below); small images decode on one core |
| **Cache- and NUMA-aware scheduling** | Work split into L2-sized chunks; buffers allocated on the decoding thread's NUMA node; thread pool sized to physical cores |
| **Memory** | `mimalloc`; large and huge pages for big buffers where the OS allows; buffer pools reused across decodes (no per-image allocation churn) |
| **Fast I/O** | Linux `io_uring` (built: neighbouring files are read into the page cache ahead of their decode, 8 × 1 MiB reads in flight, with a runtime fallback to plain page faults; `fadvise(WILLNEED)` measured slower than page faults and is not used); Windows IoRing (Windows 11) and unbuffered overlapped I/O; macOS `F_RDADVISE` + `dispatch_io`. Read-ahead tuned for NVMe; `mmap` for large random-access files |
| **Platform-accelerated libraries (optional)** | macOS Accelerate/vImage for resampling and colour conversion where faster than ours |
| **Build** | LTO (fat), `codegen-units = 1`, `panic = abort`; baseline `x86-64-v3` / `armv8.2-a` with higher tiers dispatched at runtime. PGO (trained on a generated corpus of every format) and BOLT are available but off by default: measured gains are mixed |

**When to parallelise one image: an adaptive scheduler, not a fixed rule.** Splitting a decode across
cores has a fixed cost (task dispatch, synchronisation, cache misses, stitching) that outweighs the gain
for small images. It also takes cores away from other useful work. So the `DecodeScheduler` decides per
decode, using a cost model:

1. **Estimate the single-thread decode time** from format, decoded pixel count, bit depth, compression
   settings and splittable units (tiles, restart markers, JXL groups) read from the header. Estimates come
   from **timings recorded on this machine** (a per-format, per-size model that keeps learning from real
   decodes), seeded by a calibration benchmark on first run.
2. **Estimate the parallel time** for 2…N workers, including the measured overhead of splitting, and
   capped by the number of independent units the stream offers.
3. **Split only if it wins clearly and matters to the user:**
   - The predicted saving is noticeable: more than about **8 ms** (half a frame at 60 Hz) and more than
     **25%** of the single-thread time.
   - The image is on the critical path: the one being viewed now, or the very next one while the user is
     flipping. Prefetch, thumbnails and indexing never split; they gain more from decoding *different*
     images in parallel.
4. **Respect the current state of the machine:**
   - Use free performance cores, not ones that are busy with other work. Fewer workers when the system is under load.
   - On battery or under thermal pressure, raise the bar (only split when the saving is large).
   - When the user is flipping fast (key held down), prefer the embedded preview or a reduced-size
     decode over splitting a full decode that will be thrown away.
5. **Choose the number of workers, not just yes or no.** Use the point where the predicted curve flattens.
   Each chunk is large enough to amortise dispatch (roughly 256 KB or more of output).

In practice this means small and typical web images (under a few megapixels) decode on one core, while
large photos, RAW files, big TIFFs and large DICOM frames split. The exact boundary differs by format and
machine, and the scheduler finds it itself. `specular --hw-report` shows the learned thresholds, and
benchmarks in CI check that the scheduler's choices are never slower than both fixed alternatives
(always single-threaded, always split).

### 12.3 GPU

| Technology | Where it is used |
|---|---|
| **Hardware image/video decode engines** | HEIC/HEIF (HEVC) and AVIF (AV1) still images, plus JPEG where the hardware supports it, decoded on the GPU's fixed-function decoders: Vulkan Video, D3D12 Video / Media Foundation (DXVA), VideoToolbox / ImageIO on Apple Silicon, VA-API / oneVPL on Linux and Intel, NVDEC / nvJPEG on NVIDIA (optional). Decoded surfaces stay on the GPU (zero-copy into wgpu via native interop) |
| **GPU compute decode stages** | IDCT, colour conversion, chroma upsampling and dequantisation for very large JPEGs; wavelet stages for JPEG 2000/HTJ2K; RAW demosaicing; DICOM rescale and LUT; all as wgpu compute shaders |
| **GPU decompression** | DirectStorage with GDeflate on Windows; `VK_NV_memory_decompression` where available; used for our own thumbnail and tile caches |
| **Compressed textures** | Thumbnail and tile caches stored as BC7 (desktop) or ASTC (Apple/ARM) for about 4× less VRAM and faster upload |
| **Zero-copy and direct upload** | Unified memory on Apple Silicon and integrated GPUs: decoders write straight into GPU-visible buffers. Resizable BAR on discrete GPUs: direct writes to VRAM, no staging copy |
| **Async compute and transfer queues** | Uploads, mip generation and decode stages overlap with rendering; timeline semaphores for synchronisation |
| **Shader features** | f16 arithmetic, subgroup operations (reductions for histograms, min/max, W/L auto), storage-texture atomics, bindless/descriptor indexing for thousands of tiles and thumbnails |
| **Presentation** | Low-latency present modes (mailbox), variable refresh rate (G-Sync/FreeSync/ProMotion), HDR swapchains, frame pacing tied to display refresh |
| **Heavy 3D (CAD, medical)** | Mesh shaders and GPU-driven culling for huge BIM/STEP models; hardware ray tracing for picking and optional ambient occlusion; empty-space skipping and bricking for volumes |

wgpu exposes much of this through optional `Features`. Where it doesn't (video decode, DirectStorage,
some vendor extensions), `specular-gfx` uses **wgpu-hal native interop** (raw Vulkan, D3D12 and Metal
handles) behind a small platform module.

**Hardware decode today** (`specular-hwdec`): VA-API on Linux, for baseline 8-bit JPEG, for the
intra 8/10-bit 4:2:0 HEVC pictures of HEIC files and for the H.264 and HEVC video of motion clips;
D3D12 Video on Windows for the same HEVC pictures and the same video (the DXVA parameter structs,
translated from the VA-API layouts the shared parsers build; a reference-only texture where the
driver asks for one; interlaced H.264 stays in software there; 24 MP iPhone grid on a Radeon 780M:
61 ms and 42 ms CPU against 65 ms and 790 ms CPU in software); macOS is stubbed. Surfaces are read
back. The slideshow export's hardware H.264 encoder is VA-API's on Linux and the GPU driver's
Media Foundation MFT on Windows (`mf_enc`), behind one `H264Encoder`.
A zero-copy dma-buf import of JPEG surfaces into Vulkan was built and measured (branch
`zero-copy-dmabuf`, not merged). It matched software and was 17–25 % faster than the readback, but
converting from the imported tiled memory costs about as much as the readback it saves. On the
integrated Radeon 860M, software decode was faster than both (12 MP: 41 ms software, 76 ms
zero-copy, 100 ms readback). It is worth revisiting for HEIC grids or discrete GPUs. HEIC goes through `VaapiCore`, an `HevcCore` whose software
core (`DEFAULT_CORE`) is both the reference and the per-picture fallback for pictures the hardware
can't take; a grid's tiles are pipelined, 4 in flight. Safety nets: a 2 s timeout per picture (the
device is then treated as wedged for the session), a crash canary file in the cache folder while the
hardware decodes (left behind, it blocklists the path on the next start), an exact comparison with
software on the first decode, and blocklisting on any driver error. The path selector
(`Task::HeicDecode`, `Task::JpegDecode`) and `performance.hardware_decode` decide when it is used.

### 12.3.1 Choosing between CPU and GPU paths (`PathSelector`)

Benchmarks on one development machine must not decide for every machine. An integrated GPU sharing
memory with a fast CPU and a discrete RTX or Radeon card with a slow laptop CPU give opposite answers.
So wherever Specular has both a CPU and a GPU way of doing the same work, each machine picks for itself
from its own measurements. Fixed defaults are only the starting guess.

**Scope: every dual path,** registered with one `PathSelector` (it extends the `DecodeScheduler` cost
model of §12.2 from "how many cores" to "which engine"). Today or planned:
- **Decode:** hardware decoders (JPEG, HEVC tiles for HEIC, AV1 for AVIF) vs our CPU decoders.
- **Decode stages:** GPU compute stages (YCbCr→RGB and chroma upsampling, IDCT, JPEG 2000/HTJ2K
  wavelets, RAW demosaic, DICOM rescale/LUT) vs their CPU versions.
- **Colour:** ICC transforms on the GPU (matrix, curves, 3D table) vs on the CPU before upload.
- **Rendering:** resampling, mip and overview generation, and BC/ASTC encoding of cache textures.
- **Inspection:** histogram, statistics and the other inspection tools.
- **Getting pixels to the GPU:** zero-copy import vs readback and upload. This is part of a path's
  cost, not a separate choice.

**How it decides:**
1. **Probe (cached).** Find which paths exist from the `HardwareProfile` (§12.1): decode engines and the
   profiles they support, compute features, the CPU tier. A blocklist of known-bad drivers applies here.
   The probe is redone only when the GPU, driver, CPU or app version changes. The measurements live in
   `<cache>/paths-v1.txt`, keyed by a fingerprint of GPU, driver, CPU and app version.
2. **Calibrate (once, in the background).** On first run and after any of those changes, time each
   available path on small built-in test inputs once the app has been idle for 3 s, for about a second
   in total: colour steps between frames on the UI thread, then the decoders on a `calibrate` thread
   (normal priority, so the timings match an image the user waits for) that stops as soon as the app
   gets busy. Each path is timed **end to end, until the pixels are usable in a GPU texture**
   (setup, transfer, readback or import included), because a fast engine with a slow copy back loses.
   Built-in inputs: two 4 MP PNGs (screenshot-like and photo-like) for wavefront vs row-by-row
   unfiltering; for HEIC, a 2048×1024 grid sample (`crates/specular-decode/data/heic-calibration.heic`),
   timed in software and on VA-API, with the hardware pictures compared bit-exactly before that path is
   trusted. JPEG has a built-in 2368×1776 baseline 4:2:0 sample, timed on the CPU and, when present,
   on hardware; the first hardware decode must match software (PSNR ≥ 40 dB) before that path is
   trusted, and the path is blocklisted otherwise. A calibrated table that lacks a newer decoder
   calibration reruns the decoder part once.
3. **Keep learning.** Every real run records its time per path, format and size bucket (a running
   average of ms per megapixel, plus fixed setup costs). Now and then a background job (prefetch,
   thumbnail, indexing) is sent down the path not currently chosen, so a path that has got faster (new
   driver, different power state) is noticed. The image the user is waiting for never explores.
4. **Decide per request, by what the request needs:**
   - Interactive (the image on screen, the next image while flipping): lowest predicted time to
     pixels on screen.
   - Background (prefetch, thumbnails, indexing): whatever leaves the CPU free for the user, as long as
     it keeps up (the PNG wavefront, for example, only when it also costs less processor time).
   - Battery or thermal pressure: prefer fixed-function engines and lower energy, even when they are
     slower.
   - Current load: busy CPU cores and queued GPU work shift the prediction.
5. **Safety rails.**
   - A path is eligible only after it passes its correctness check against the CPU reference (bit-exact
     for lossless, a PSNR floor for lossy, as with the hardware JPEG 40 dB gate).
   - An error, device loss or driver crash on a path blocklists it for that driver version. The work is
     redone on the CPU.
   - The pure-Rust CPU path always exists.
6. **Visible and overridable.**
   - Settings → Performance offers "Automatic" (the default), "Prefer GPU" and "CPU only"
     (`performance.processing`). `performance.fast_yuv_upload` and `performance.hardware_decode` are
     overrides of the automatic choice (`auto`/`on`/`off`); settings schema version 2 migrates their
     old booleans (hardware decoding that was on stays on; YUV upload keeps only "off").
   - Under it, a read-only panel, "How Specular runs on this computer", gives one plain-language row
     per task: the choice for interactive and background work, why (measured with numbers, not
     measured yet, your setting, blocked with date and cause, being checked, unavailable, exploring),
     and a detail line with ms, processor time, sample count and confidence. A blocked path offers
     "Try again"; "Measure again" (after a confirmation) forgets the table and blocklist and
     recalibrates. It refreshes every 2 s while open. It reads the selector's lock-free `snapshot`;
     `retry` and `reset_measurements` are the two actions.
   - `specular --hw-report` and the in-app hardware report show each path's measured costs and the
     current choice, with the reason.

**Testing.** Choices depend on the machine, so they are hard to reproduce. Timing tables can therefore
be recorded and replayed to test the decision logic. CI on the reference machines (§12.4 rule 3) checks
that the automatic choice is never slower than either fixed choice. "CPU only" gives a reproducible
baseline for bug reports.

### 12.4 Rules

1. **Fastest path first, always a fallback.** Every accelerated path has a portable CPU/WGSL path, and CI
   checks they produce matching output (bit-exact for lossless, within tolerance for lossy).
   Exception: Develop (§6.10) is GPU-only. Its CPU versions of each operation are test references
   only; without a usable GPU it runs on the software adapter (lavapipe, WARP), slowly.
2. **Measure, don't assume.** The `PathSelector` (§12.3.1), together with the `DecodeScheduler` cost
   model (§12.2), picks between CPU, multi-core, GPU compute and hardware-decoder paths per request,
   using timings learned on the user's machine, never fixed choices from our benchmarks. Small images
   often decode faster on one CPU core than through a hardware engine.
3. **Benchmarks in CI** on reference machines (Windows x86 with discrete GPU, Apple Silicon Mac, Linux
   laptop with integrated GPU), published per release and compared with our own previous releases.
4. **Power aware.** On battery, prefer efficiency cores and fixed-function decoders; never spin the GPU when idle.

### Budgets (reference: mid-range 2023 laptop, NVMe, integrated GPU)

| Metric | Target |
|---|---|
| Cold start to window (first present with the image; real-font text may follow) | < 150 ms |
| Open 24 MP JPEG to first pixel | < 100 ms (preview), < 250 ms (full) |
| Next image (prefetched) | < 16 ms (one frame) |
| Open 100k × 100k TIFF | First view < 300 ms |
| 512-slice CT series to first slice | < 500 ms; volume ready < 3 s |
| Library grid scroll, 100k items | 60 fps |
| Idle CPU / GPU | ~0% (no repaint) |
| Binary size (core, no C bridges) | < 30 MB |

These are ceilings, not goals: each release should beat the previous one on the same machine (§12.4).
Budgets are enforced by **benchmarks in CI**, with a regression fail threshold of +10%, and decode
speed also by **instruction counts** (exact on shared runners), failing at +2%.

### Pipeline

```
open(path)
  ├─ probe (first 4 KB, magic bytes)
  ├─ preview  ──► GPU upload ──► display          (instant, low-res)
  └─ decode(req: target_size = viewport) ──► display (sharp)
        └─ background: full-res / tiles / pyramid as zoom requires
```

- **Prefetch** N next/previous files in directory order, weighted by navigation direction.
- **Cache**: two-level LRU (CPU RAM and VRAM) with configurable budgets (default 25% of RAM, a VRAM cap);
  entries are keyed by `(source, page, frame, nd-index, channel set, scale level)`. Remote sources use
  the object's ETag/version instead of mtime.
- **Tiled pyramid** for anything larger than the GPU max texture size (512² tiles, mip levels,
  viewport-driven streaming).
- **Colour:** decode into linear float or 16-bit where needed; ICC → working space → display profile in a
  shader; HDR display output where the platform allows (scRGB/HDR10 on Windows, EDR on macOS).
- **Volumes:** a 3D texture (R16 / R16F), bricked for volumes above the VRAM budget.
- **Vectors:** vello scene, re-encoded only when visibility or layers change; zoom is a transform.
- **Meshes:** GPU buffers, frustum culling, instancing for repeated IFC elements, LOD for huge models.
- **Point clouds:** octree nodes streamed by screen-space error, under a point budget per frame; COPC
  read directly; GPU-built LOD while a non-COPC file is still loading (§8.3).
- **Splats:** GPU radix sort per frame, LOD for large captures, same budget model as point clouds.
- **3D precision:** f64 world coordinates, rendered relative to the eye (§4).
- **I/O:** memory-mapped for large files; directory scanning is async with natural sort.

### 12.5 Load-time refinements

These decide how quickly the first pixel appears and how instant flipping feels, which matters more than
raw decode speed. Items marked ★ are the highest impact.

**★ Never decode more pixels than the screen can show.** The first decode targets the viewport size:
JPEG DCT scaling (1/2, 1/4, 1/8), resolution levels in JPEG XL and JPEG 2000/HTJ2K, half-size RAW
demosaic, reduced-size HEIF/AVIF thumbnails or grid tiles. Full resolution is decoded only when the user
zooms past fit. A 50 MP photo on a 4K screen loads about 2–8× faster this way.

**★ Upload YUV planes; convert colour on the GPU.** JPEG, HEIC, AVIF and similar formats store YUV, often
with subsampled chroma. We upload the planes as-is (1.5 bytes/pixel for 4:2:0 instead of 4 for RGBA)
and convert, upsample chroma and colour-manage in the shader. Decoders write straight into a
persistently mapped upload ring buffer, so there are no intermediate copies.
Chroma is interpolated in `convert.wgsl` with `textureLoad`, not the sampler, because Vulkan allows
8-bit filter precision; the CPU Y'CbCr path keeps 4 fractional chroma bits.
16-bit PNG (every colour type, and APNG frames) decodes to `Rgba16` sRGB-coded samples without
narrowing, and its profile (iCCP, or gAMA/cHRM) is applied to the 16-bit samples. Wide SDR rasters
keep a half-float overview and are tiled like 8-bit ones (converted at full precision, rounded once
into the 8-bit tile array); only HDR layouts (PQ, HLG, linear, float) are overview-only and tone
mapped. CPU fallbacks (thumbnails, clipboard) tone-map HDR layouts only. A 16-bit PNG costs 8 B/px
decoded (2× 8-bit RGBA). 16-bit grey stays compact: `Gray16` (2 B/px) and `GrayAlpha16` (4 B/px)
carry sRGB-coded samples from PNG, JPEG 2000, TIFF, PGM/PPM/PAM and PSD/PSB to the GPU (R16Uint / Rg16Uint); the convert pass
splats grey to RGB before the source transform, so grey ICC profiles and gAMA apply as for 8-bit
grey. JPEG 2000 above 8 bits stays 16-bit at any size, also with a palette or an embedded ICC
profile; 16-bit TIFF, PGM/PPM/PAM and PSD/PSB decode to `Gray16`, `GrayAlpha16` or `Rgba16` like
PNG, grey staying one channel. The pixel inspector shows 16-bit source values (0–65535) for files
whose decoder keeps 16 bits (9–15-bit sources on the 16-bit scale); a decoder that narrows to 8
bits shows no source value rather than a misleading one.

**★ Persistent display cache.** Screen-resolution versions of recently viewed images are stored on disk
as BC7/ASTC, keyed by content hash (size + mtime + partial hash). Reopening a folder becomes a direct
GPU upload. The cache is shared with library thumbnails and has a configurable size (default 2 GB, LRU).

**★ Cold-start engineering:**
- Create the window immediately, already sized to the image's aspect ratio (no resize flash).
- Show the embedded preview before anything else is initialised.
- Use a persistent GPU pipeline cache; compile only the shaders the first frame needs, the rest in the background.
- Bake a pre-rasterised glyph atlas for the UI font into the binary.
- Load C libraries (libheif, PDFium, OpenCASCADE…) only on first use, never at startup.
- Keep dynamic libraries at launch to a minimum and static initialisation tiny (most noticeable on Windows and macOS).
- Optional **resident mode**: a background process keeps the GPU and caches warm, giving opens in under 30 ms.
- While the GPU comes up, the image renderer's pipelines are built beside the surface setup, and
  overview levels that queued meanwhile (thumbnail, 1/8, 1/2) are coalesced so only the best one is
  uploaded. Prefetch of neighbours is held until the current image is sharp, it failed, or 1 s has
  passed; so is the hardware-decoder probe. The first frame shows the best level decoded so far
  rather than forcing a low one: it barely helps first pixels, since the larger level is usually
  decoded by the time the renderer is ready, and each extra level is another upload and frame before sharp.
- Vulkan loader tuning (skipped drivers, and Mesa's device-select layer on single-GPU machines) is
  set on Specular's own environment only; programs it starts don't inherit it.

**★ Cancel stale work immediately.** Decoding uses a priority queue with cooperative cancellation checked
between chunks. When the user moves on, abandoned decodes stop within about 1 ms and the new image gets the cores.

**Predictive prefetch.** Learn the user's navigation pattern (direction, speed, sort order, jumps) and
prefetch accordingly. Hovering a thumbnail in the library or filmstrip starts its preview decode.

**Fast directory scanning.** Use bulk OS calls (`getdents64`, `NtQueryDirectoryFile`,
`getattrlistbulk`), show the opened file before the listing finishes, and sort incrementally. A folder of
100k files never delays the first pixel.

**Skip work that does nothing.** No ICC transform when image and display are both sRGB, no blending for
opaque images, no 16-bit intermediate for 8-bit sources, and orientation applied as a texture transform
rather than a pixel pass.

**Memory-pressure awareness.** Listen to the OS signals (macOS memory-pressure dispatch source, Windows
memory resource notifications, Linux PSI) and shrink caches before the system starts swapping.

**Slow-storage adaptation.** Detect throughput and latency per volume (SMB, NFS, USB, cloud-synced
folders). On slow storage, prefetch further ahead with larger sequential reads, and cache files locally.

**Reuse existing thumbnails.** Use embedded thumbnails (EXIF, RAW, HEIF, DICOM icon, DWG/RVT/SKP previews)
and the freedesktop thumbnail cache on Linux before generating our own.

**Input latency under one frame.** Zoom and pan input is handled late in the frame, just before
rendering, so the transform uses the latest input. Input-to-photon latency is measured in CI where possible.

**Animation.** Frames are decoded ahead into a ring buffer during playback. Huge GIF/APNG/WebP
animations decode on demand rather than all at once.

**Profiling built in.** `tracing` spans throughout, Tracy integration and the frame-time HUD, so every
slow open can be explained from a trace.

**Debug logging by component.** Every crate logs through `tracing` with a fixed component target, so a
user or developer can turn on exactly the part they need, or everything, without a debug build:

```
SPECULAR_LOG=all                          # everything at debug
SPECULAR_LOG=decode,tiles                 # just these components at debug
SPECULAR_LOG=all:info,decode:trace,gpu    # per-component levels; the last match wins
SPECULAR_LOG_FILE=path/to/log             # also write to a file (default: stderr only)
specular --log decode,tiles …             # the same, as a command-line flag
specular --log-components                 # list every component with a one-line description, then exit
SPECULAR_LOG=help specular                 # the same
```

| Component | Covers |
|---|---|
| `startup` | Process start, argument parsing, cold-start stages, shutdown |
| `io` | File open, mmap, reads, file watching |
| `folder` | Folder scan, sorting, include-subfolders, neighbour list, file operations (trash/undo) |
| `jobs` | Job pool: submit, start, finish, cancel, priority changes, gating, queue depth |
| `decode` | Format dispatch, decoder choice, scale and region requests, memory estimates, errors |
| `jpeg` | specular-jpeg internals: markers, scan script, parallel/speculative decisions, index builds |
| `cache` | RAM/VRAM caches, display cache, eviction, memory budget |
| `prefetch` | Prefetch decisions and hits/misses |
| `tiles` | Pyramid levels, tile requests, batching, region decodes, uploads |
| `gpu` | Adapter/device, surface, pipelines, uploads, presents, device loss |
| `render` | Frame scheduling, redraw reasons, present timing |
| `view` | Zoom, pan, fit, animations |
| `input` | Keys, mouse, touch, action dispatch, enabled/disabled decisions |
| `ui` | Widgets, panels, popovers, focus, tooltips, AccessKit |
| `text` | Font loading, shaping, glyph atlas |
| `colour` | ICC profiles, transforms, display profile |
| `metadata` | EXIF/XMP/ICC parsing, summary building, geocoding |
| `i18n` | Locale negotiation, catalogue loading, missing messages |
| `settings` | Settings load, save, watch, `--set`, validation |

- Each open image gets a short id that every log line about it carries (`img=7 2.jpeg`), along with
  its load state: `requested → opened → header → level 1/8 → … → sharp`, or `failed: reason`. So
  "stuck on a spinner" shows exactly which step never finished.
- **Stall watchdog:** if an image stays in the loading state longer than 2 s (configurable), the app
  logs a warning with that image's state, its pending jobs and the job queue. It does this even when
  `SPECULAR_LOG` is unset, so a stuck load always leaves a trace.
- Lines carry time since process start, thread name and component. `RUST_LOG` still works for
  third-party crates (wgpu, winit), and their noise is off unless asked for.
- Disabled logging costs nothing measurable: `tracing` callsites are checked once and cached.
  Hot-loop logging (per tile, per MCU row) is at `trace` level only.

### 12.6 Viewing large images: zoom, pan and tiling

The rule: **zoom and pan never wait for decoding.** The view always shows something immediately, and
sharpens as better data arrives.

#### View model

The view state is `center` (image coordinates, f64), `zoom` (stored as log₂ of screen pixels per image
pixel) and `rotation`/`flip`.

- **Logarithmic zoom** makes animation feel even: 100%→200% feels the same as 400%→800%.
- **Zoom anchors at the cursor or pinch point**; the image point under it stays fixed:
  `center' = p − (p − center) · (zoom_old / zoom_new)`.
- **Clamping:** an axis where the image is smaller than the view is centred; where it is larger, the
  image can't be panned past its edges, with a slight rubber-band for feedback.
- **Precision:** coordinates are f64 on the CPU and passed to the GPU relative to each tile's origin, so
  deep zoom into gigapixel images never jitters.

#### Controls

| Input | Action |
|---|---|
| Ctrl/Cmd + wheel (plain wheel configurable: zoom or next image) | Zoom at cursor, animated |
| Trackpad pinch / touch pinch | Continuous zoom, following the fingers 1:1; on release, glides onto fit or 100% when within 4% |
| Trackpad two-finger double tap | Toggle fit ↔ 100% at the pointer |
| `+` / `-` | Zoom steps: 10, 25, 33, 50, 67, 100, 150, 200, 300, 400, 800, 1600, 3200% |
| `f` / `1` / `2`–`4` | Fit / 100% / 200–400% |
| Double-click on image | Toggle fit ↔ 100% **at the clicked point** |
| `z` + drag | Zoom to a drawn rectangle |
| Left-drag, `Space` + drag, middle-drag | Pan |
| Trackpad two-finger scroll | Pan with momentum (always; the plain-wheel setting applies to notched wheels only) |
| Trackpad two-finger sideways swipe (macOS) | Next / previous image when the image is no wider than the window (AppKit's "Swipe between pages": follows that system setting, one image per swipe, reading direction respected); otherwise it pans |
| Arrows / `Shift` + arrows | Pan by a step / by a screen (when zoomed) |
| Hold `q` | Loupe: magnifier under the cursor at 100% (or 400% with `Shift`), view unchanged |
| `n` | Navigator (all modes): minimap overlay with the viewport rectangle; drag to pan, wheel to zoom |

**Feel:**
- Pan momentum: flicks glide with friction and stop instantly on touch or click.
- Wheel and keyboard zoom animate with a critically damped spring (about 150 ms); new input retargets
  the running animation, so there is no stutter.
- Zoom snaps softly to fit and 100% when passing close to them.
- "Reduce motion", or holding `Alt`, makes zoom and pan instant.
- Gesture sources: winit on macOS; on Linux, Wayland `zwp_pointer_gestures_v1` or X11 XInput 2.4
  pinch (older X servers have none); on Windows, precision-touchpad pinch arrives as Ctrl+wheel with
  fractional notches and zooms directly. Touchpad scrolls that arrive during a pinch (and 150 ms after)
  are dropped, so they don't pan against it. The rotation gesture is only logged (45° threshold);
  it doesn't turn the image.

#### Tiled pyramid

```
Level 0  (full res)   ████████████████   512² tiles; only visible ones resident
Level 1  (1/2)        ████████
Level 2  (1/4)        ████
  …
Level N  (≤ screen)   ██                 always resident: never a blank frame
```

- **Level selection:** the coarsest level with at least one image pixel per screen pixel. Sampling blends
  adjacent levels (trilinear), so zooming never pops or shimmers.
- **Loading priority:**
  1. The overview level (whole image at screen size or less) is always resident.
  2. Visible tiles at the needed level, from the centre of the view (or the cursor) outwards.
  3. A ring of tiles around the viewport, biased in the direction of pan velocity.
  4. While zooming in, the next finer level around the zoom anchor, so it is often ready when the
     animation lands.
- **Drawing:** coarse to fine each frame; the coarser level fills any gaps, and sharper tiles cross-fade
  in over about 80 ms. Never a hole or checkerboard. Tiles carry a 1–2 px border from their neighbours,
  so filtering shows no seams.
- **GPU storage:** a texture array (or sparse/tiled resources where supported) with an LRU cache, drawn
  with one instanced draw call.
- **Bounded memory:** a 4K screen needs roughly (screen pixels × 4/3) of tiles across levels, about
  50–100 MB including the prefetch ring, whatever the image size. A 100k × 100k (10 GP) image is viewable
  on a laptop.
- **Quality while moving:** during pan and zoom, resident levels are sampled directly to hold full frame
  rate. About 100 ms after motion settles, the visible area is refined with the high-quality filter
  (gamma-correct Lanczos). Above about 400–800% it switches to nearest-neighbour with an optional pixel
  grid, and the value under the cursor (RGB, or HU for DICOM) shows in the status bar.

#### Tile sources per format

| Source | How tiles are produced |
|---|---|
| Natively tiled / multi-resolution: tiled TIFF, BigTIFF, pyramidal TIFF/COG, JPEG 2000/HTJ2K, JPEG XL, whole-slide (SVS, NDPI…), DZI, DICOM-WSI | Decode exactly the needed region at the needed level; no preprocessing |
| Large JPEG | Instant overview by DCT scaling (1/8). A first pass indexes restart markers / Huffman positions so later region decodes jump straight there; without restart markers, one full decode streams into level-0 tiles |
| PNG, BMP and other sequential formats | Rows stream into tiles as they decode, so the image appears top-down; the pyramid is built on the GPU |
| Camera RAW | Embedded preview as the overview; full demosaic into tiles in the background |
| Vector (PDF, SVG, DXF, HPGL) | No fixed pyramid: tiles rendered on demand at the exact zoom, always sharp |

For files without native tiles, the **built pyramid is cached on disk** (BC7/ASTC, keyed by content
hash). The first open builds it; every later open, zoom and pan is as fast as a natively tiled file.

#### Budgets

- Pan and zoom hold the display refresh rate (60/120 Hz) on a 10 GP image on the integrated-GPU reference laptop.
- No blank tile is ever shown: CI replays scripted pan/zoom paths and checks every frame for gaps.
- Input to screen in under one frame.
- Tile memory stays within budget regardless of image size.

### 12.7 Hardware advisor

When a user regularly opens heavy files that load slowly **because of their hardware**, Specular tells
them which kind of hardware would make a marked difference, and by roughly how much. It is helpful,
local and never pushy.

#### Detecting a slow load and its cause

Every load is already traced (§12.5). The advisor splits each slow load into stages (**read, decode,
upload, render**) and attributes the bottleneck:

| Bottleneck | Signals | Typical advice |
|---|---|---|
| **Storage** | Read time dominates; low measured throughput or high latency (HDD, USB 2, SD card, network share) | "These files are on a slow drive (about 40 MB/s). An internal NVMe SSD would load them roughly 5–10× faster. Copying the folder to your internal drive helps too." |
| **No hardware decoder** | HEIC/AVIF decoded on the CPU because the GPU lacks HEVC/AV1 decode | "Your GPU can't decode AV1 images in hardware. GPUs with AV1 decode (for example Intel 11th-gen or newer, NVIDIA RTX 30-series or newer, AMD RX 6000-series or newer, Apple M3 or newer) would open these about 3–6× faster." |
| **CPU** | Decode dominates; few cores, no AVX2/NEON-class SIMD, or sustained thermal throttling | "Decoding these 100 MP RAW files is CPU-bound on 2 cores. A CPU with 8 or more performance cores would cut load time by about 60%." |
| **Memory** | Swapping or cache evictions during load; the working set exceeds RAM | "This 6 GB volume doesn't fit in memory (8 GB installed). 32 GB of RAM would keep it resident." |
| **GPU memory or capability** | VRAM thrashing of tiles/volumes; software rendering fallback; missing features (e.g. 3D textures too large) | "Volume rendering this CT needs about 4 GB of GPU memory; your GPU has 1.5 GB shared. A GPU with 8 GB or more would render it interactively." |
| **Power / thermal** | On battery or in a low-power mode, efficiency cores only | "Plugging in or switching to a performance power mode would make loading about 2× faster." |

The device's capabilities come from the `HardwareProfile` (§12.1).

#### Open report

The advisor's first step is built. Every opened image gets a timeline:

- read time, with mmap I/O estimated from major page faults on the decode thread;
- header parse and colour-management preparation;
- decode: which engine, the path selector's reason, any fallback, and the first-decode check;
- GPU upload, waiting for other work, and startup for the first image;
- first-pixels and sharp times.

A pure function turns the timeline into one verdict: the dominant stage, plus, when a decode ran on
the processor although the task has a hardware decoder, why. The reasons are:

- no device, or no decoder for this codec;
- hardware detection still running, or the path not checked yet;
- blocklisted (mismatch, error or lost);
- "Processor only" mode, or hardware decoding turned off in settings;
- lower power on battery, or measured faster on the processor;
- a fallback during this decode.

When one setting or action fixes the cause, a hint names it. Under 250 ms the report only says how
fast the image opened.

The report appears in the information panel's "Performance" section (collapsed by default) and is
logged on `decode` at debug level. `SPECULAR_OPEN_REPORT=1` prints it, and `--bench-open` includes it
in its JSON. It costs two `getrusage` calls per decode and a few timestamps.

Pattern detection is built over the last 64 reports. A pattern needs at least five slow opens (≥ 250 ms, not prefetched or from the display cache)
among the last 20 of a kind, at least 80 % alike: the same format decoded in software for the same
reason (not when software was chosen for speed, power or a reduced level), or reading the file as
the bottleneck below 150 MB/s, or unpacking archives. It names the change (a setting, Try again, a
local copy or extraction, or a hardware decoder for the codec) and estimates the gain as a range of
median per-open speedups: for hardware from the selector's estimate, allowed to be 25 % slower when
measured and 2× when guessed; for storage from 150 MB/s up to the fastest read seen this session
(at most 1500 MB/s). Marked advice adds rows to the Performance section; the rest, and a first open
that paid for start-up, is logged only. The rules are deterministic and unit-tested.
The history survives restarts: waited opens of the past week, at most 64, one line each in the
cache folder, never sent anywhere.

The gallery shows the same "Why is this slow?" indicator and panel for thumbnails. Each thumbnail the
engine makes from its file becomes a report like an open's: time spent waiting for the disk (page
faults, and idle time beyond CPU time) counts as reading. Thumbnail reports go into a session-only
history with a 150 ms threshold, where software decoding, slow storage and archive patterns are found
under the same settings. Thumbnail panels are worded for thumbnails, offer no prefetch or display-cache
remedies, and say so when the thumbnail cache is unavailable. The indicator's × hides that kind of
insight for good (kept beside the history; the panel then offers "Show hidden insights again"). When
the computer is saving power (battery, low-power mode, thermal pressure), panels about software
decoding say so.

#### Estimating the gain

- Specular ships a small **reference performance table**: our CI and release benchmarks per format, file
  size and hardware class (storage type, CPU class, GPU decode features, VRAM, RAM).
- Combined with the stage breakdown measured on the user's own machine, it gives an estimated speed-up
  as a **range** (e.g. "about 3–6× faster"), never a single promised number.
- Advice is only shown when the predicted improvement is **marked**: at least 2× for that kind of file,
  or bringing typical loads under 1 s.

#### When and how it appears

- **Not on one-off slow loads.** A hint appears only after a pattern: for example, five or more loads
  over 2 s of similar files in the past week, all with the same bottleneck.
- A small, dismissible indicator in the status bar ("Why is this slow?") opens a **Performance insights**
  panel showing:
  - a stage bar for the last slow load (read / decode / upload / render);
  - the bottleneck in plain language;
  - **what can be done now without new hardware**, first: move files to a local drive, enable the
    display cache or resident mode, plug in, close memory-heavy apps;
  - **which hardware would help**, with the estimated gain range. Hardware is described by capability
    (e.g. "a GPU with AV1 hardware decode", "an NVMe SSD"); vendor and model examples are shown only to
    make the capability concrete.
- Also reachable any time from the command palette ("Performance insights") and `specular --hw-report`.
- "Don't show again" per bottleneck type, and a global off switch.

#### Principles

- **Local only.** No data leaves the machine, and there is no telemetry behind the advisor (§1,
  principle 10). The open report and the hardware report are shown locally; they reach anyone else
  only if the user copies them.
- **Neutral.** No affiliate links, ads or brand preference. Examples are listed alphabetically by vendor.
- **Honest.** If the bottleneck is our software, it is logged as a performance bug in the trace, and the
  advisor does not blame the hardware.
- **Kept current.** The reference table updates with each release, along with the hardware examples
  (e.g. new GPU generations with AV1 decode).

### 12.8 Remote and cloud-native sources

Large scientific, pathology, GIS and medical data increasingly lives in object storage, not on disks.
`specular-source` reads it **without downloading whole files**:
- **Protocols:** HTTP(S) with range requests, S3 (and compatible), Google Cloud Storage, Azure Blob,
  DICOMweb. Credentials come from the OS keychain or standard environment/profile files; never stored
  in the library DB in plain text.
- **Formats that benefit:** COG, OME-TIFF and other tiled TIFFs, OME-Zarr (v3 shard indexes are read
  with one range request), COPC, 3D Tiles, IIIF, DICOMweb with HTJ2K.
- **Chunk cache:** remote chunks go into the same disk cache as tiles (§12.6), keyed by URL + ETag.
- **Latency-aware prefetch:** with high latency, request more tiles in parallel (HTTP/2 multiplexing,
  coalesced adjacent ranges) and further ahead of the pan direction; the overview level is fetched first
  so the view is never blank.
- **Progressive formats first:** HTJ2K and JPEG XL streams are requested in quality layers or
  resolution order, so a usable image arrives from a fraction of the bytes.
- **Offline behaviour:** cached chunks remain viewable offline; missing tiles show the coarser level
  with a subtle "remote" indicator.
- The hardware advisor (§12.7) treats the network as another storage tier ("your connection to this
  bucket averages 180 ms; data in a nearer region or a local copy would load about 4× faster").

---

## 13. Security and sandboxing

- Every in-process (Rust) decoder has **`cargo-fuzz` targets**, and so do the archive container parsers
  (`specular-archive`) and the EXIF/XMP/ICC parsers (`specular-metadata`). CI fuzzes every target on
  each push, keeping a growing corpus per crate.
- C/C++ libraries (libheif, only as the fallback for HEIC files `specular-heic` rejects; UnRAR; later LibRaw,
  CharLS, PDFium, OpenCASCADE, IfcOpenShell, OpenSlide, libvips, cfitsio, HDF5) run in **`specular-decode-host`**, a separate process:
  - IPC over a pipe plus a **shared-memory** pixel buffer, so pixel data is never copied twice.
  - Linux: seccomp-bpf + namespaces (Landlock where available). macOS: App Sandbox profile.
    Windows: AppContainer / restricted token, job object with memory limit.
  - It receives already opened, read-only file handles, never paths, and opens no file itself on
    any OS; it has no network and no write access. Unix: descriptors passed over the socket
    (`SCM_RIGHTS`); Windows: handles duplicated into the AppContainer with read rights only.
    Archives that are not files on disk (inside another archive, from the clipboard) are passed as a
    private copy (sealed memfd on Linux). UnRAR is built with a small handle-I/O patch
    (`third_party/unrar_sys/patches`) so that it reads only these handles.
  - Multi-volume RAR: UnRAR sees only synthetic volume names. The host asks the viewer for each
    further volume (`NeedVolume` → `Volume`, protocol v2); the viewer hands out only files in the
    archive's own folder with the same archive stem (`.partN.rar`, `.rar`, `.cbr`, `.r00`, `.s01`,
    `.001`), at most 4096 per request, so a compromised host cannot pull in other files.
  - Crash → error tile + log entry; the host restarts automatically.
- Memory caps per decode protect against decompression bombs (dimension × depth checks before allocating).
- DICOM networking only listens when the C-STORE SCP is enabled by the user, and is bound to configured
  interfaces.
- **Archives:** RAR, 7z and ZIP members are size- and ratio-checked before extraction (zip bombs);
  RAR decoding runs only in the sandbox host. Paths inside archives are never written to disk.
- **Remote sources:** HTTPS by default; credentials in the OS keychain; no automatic fetching of URLs
  embedded in files (e.g. SVG, PDF, xrefs) without the user's consent.
- **URL scheme and IPC:** `specular://` requests that point to non-local servers ask for confirmation
  the first time per host; the IPC socket is local and per-user only.
- **Local AI models** are downloaded only on request, verified by hash, and run in-process without
  network access. Sideloaded models (§10) pass the same sha256 check against the shipped manifest.
- **No telemetry, no phoning home** (§1, principle 10). Crash information is written to a local log
  that the user can open and copy; nothing is sent. **No-network mode** (§11) is enforced in one place
  (`specular-source` and every networking crate check it), and CI runs the test corpus with networking
  blocked to prove that nothing reaches out.
- **External vendor SDKs** the user installs (DJI Thermal SDK, §6.9; ODA File Converter, §8.1) run only
  in `specular-decode-host` or as a separate program, with the same handle-only, no-network rules.
- **Outgoing exports** (teaching-file uploads to Radiopaedia, §7.4; send to PACS, §7.5) are started by
  the user, one at a time, show what will be sent, and medical teaching exports always pass through
  the anonymiser, which cannot be switched off for them.

---

## 14. Licensing

- **Specular licence: MIT OR Apache-2.0**, at the user's option, the usual pair for Rust projects.
  Anyone may use, change and redistribute Specular, commercial use included.
- **Contributions** are accepted under the same terms (inbound = outbound): no CLA.
- **Medical disclaimer** in the About box, the manual and the medical views: Specular is not a
  medical device.
- Outputs are always open formats (TIFF, PNG, CSV, GeoJSON, DICOM, BCF), so no one's data is
  locked to Specular.

### Dependency rules (enforced by `cargo-deny`)

| Allowed | Conditions | Not allowed |
|---|---|---|
| MIT, Apache-2.0, BSD-2/3, zlib, ISC, MPL-2.0, CC0, Unicode | — | GPL-2/3, AGPL |
| LGPL-2.1/3 | Dynamically linked, user-replaceable library; notices shipped | Static linking of LGPL code |
| Free external tools (ODA File Converter) | User-installed, invoked as a separate program (EULA checked for commercial use) | Bundling them without permission |
| Vendor SDKs (DJI Thermal SDK) | User-installed, loaded only in the sandbox host, never redistributed | Bundling, or shipping its headers or binaries |
| Proprietary fonts (Autodesk SHX) | Read from the user's own font paths | Bundling them |

Known exclusions: LibreDWG (GPL), DjVuLibre (GPL), Ghostscript (AGPL), x265 (GPL), GPL builds of FFmpeg,
Bio-Formats (GPL), GDAL's GPL-only drivers. Data licences are tracked too (GeoNames CC BY 4.0 needs
attribution; map tiles follow the provider's usage policy).

---

## 15. Milestones

Each milestone ends with a tagged, packaged build on all three platforms. **Packaging starts at M1.**

| # | Milestone | Scope | Exit criteria |
|---|---|---|---|
| **M0** | Skeleton | Workspace, winit + wgpu, `specular-gfx` (primitives, glyph atlas, blur, shadows), text field with IME, AccessKit wiring, **`specular-i18n` (Fluent + ICU4X, live language switch, RTL mirroring, pseudo-locales)**, **`specular-settings` schema + Settings window**, widget gallery, decoder trait/registry, PNG/JPEG, fit/zoom/pan with springs, next/prev (keys, toolbar buttons, edge arrows), fullscreen toggle, overlay toolbar, action system, `Source` abstraction (file/archive/remote), `NdArray` and f64 3D coordinates in the core model, design tokens + mockups, high-quality resampling + dithering, viewport-sized JPEG decode + YUV upload, `HardwareProfile` + SIMD runtime dispatch, PGO/LTO build, benchmark harness, `cargo-deny`, CI on 3 OSes | 24 MP JPEG < 250 ms; next image < 16 ms prefetched; text input works with CJK IME on all 3 OSes; overlay chrome matches mockups at 1×/1.5×/2× |
| **M1** | Daily driver | All §6 formats, core widget set (menus, popovers, scroll, lists, tree, table, palette), hardware HEIC/AVIF/JPEG decode, GPU compute decode stages, async I/O, P/E-core scheduling, viewport-sized decoding, YUV upload, persistent display cache, instant cancellation, cold-start work (§12.5), large-image zoom/pan/navigator/loupe (§12.6), prefetch + cache, tiled pyramid, colour management, **gain-map HDR**, PNG 3rd-edition HDR, CMYK/Lab, keymap + palette + cheat sheet, info panel with **plain-language metadata and offline reverse geocoding** (§6.7), inspection tools (§6.2), archives + reading mode, Open with / user commands, drag-out and paste, portable mode, platform thumbnailers, installers | Opens a reference corpus of 2,000 files with zero crashes; budgets met on all 3 reference machines |
| **M2** | Library v1 | SQLite catalogue, watched locations, thumbnail cache, grid/list, search, ratings/tags/XMP, compare, slideshow, file operations with undo, predictive and hover prefetch, reuse of existing thumbnails, slow-storage adaptation, **cull mode** (flags, RAW+JPEG pairs, copy/move slots, bursts, hot folder), compare up to 4 + difference view, map view, batch convert, print/contact sheets, slideshow spec + MP4 export, multi-monitor, IPC socket, motion photos + basic video, **hardware advisor** (§12.7) | 100k-file library indexes in the background; grid at 60 fps; advisor identifies the bottleneck correctly on a test matrix of slow storage, no hardware decode, low RAM and low VRAM |
| **M3** | Medical 2D | DICOM + all transfer syntaxes, series assembly, W/L, HU, presets, tag browser, cine, DICOM video, ultrasound regions, manual and ball-marker calibration with provenance, NIfTI/Analyze/NRRD/MHA, anonymised export with burned-in text masking, movie export, IHE PDI media export, conformance statement, disclaimer; **user profiles** and the **measurement layer** (§6.8) first; **linked views** (scrolling, reference lines, manual prior sync; moved from M7), measurement table, **vet/ortho/ceph geometry tools** (moved from M8 specialty measurements), patient-media robustness, key images and teaching export | Passes a public DICOM conformance corpus; correct HU and orientation on test series; measurement validation suite passes |
| **M4** | Medical 3D | Volumes, MPR (orthogonal + oblique), slab MIP, volume rendering, cinematic rendering, isosurface + STL export (one mesh per segment, manifold check), measurements, ROI stats, time-intensity curves, GSPS/KOS save; **curved MPR with a manual centreline** and **dental CBCT** panoramic/cross-sections (moved from M8); auto 3-plane MPR, MPR/3D as tabs; NIfTI label maps over DICOM | 512³ CT interactive at 60 fps on reference GPU |
| **M5** | Engineering 2D | vello renderer, DXF, PDF drawing sets, HPGL, scanned drawings, layers, plot styles, calibrated measure, PDF scale dictionaries, extended measurement types, revision overlay with alignment, entity-level compare, hyperlinks, embedded markups, print to scale, DWG via ODA converter | Large architectural PDF/DXF sets pan/zoom at 60 fps |
| **M6** | Engineering 3D | Sandbox host, meshes (STL/OBJ/PLY/glTF/3MF), IFC with spatial tree + properties, STEP/IGES with PMI, section box, x-ray, walk/fly, saved viewpoints, **federation + georeferencing, BCF, IDS**, clash display, model diff, compact BIM cache, mass properties, mesh analysis, glTF animation, USD/USDZ | 500 MB IFC model opens and navigates smoothly |
| **M7** | PACS-lite | Study DB, DIMSE (ECHO/FIND/MOVE/GET/STORE), DICOMweb with progressive HTJ2K streaming, `specular://` launch, priors with per-series progress, pause/cancel and thin-slice skipping, send to PACS, hanging protocols (multi-monitor) on top of the M3 linked views | Query/retrieve against Orthanc and dcm4chee |
| **M8** | Advanced medical + point clouds | PET/CT fusion + SUV + TMTV (fused export keeps the colour map), rigid registration, SEG (incl. Label Map) / RTSTRUCT / RTDOSE + DVH, AI result display, SR viewer (more templates, jump to finding, lesion size over priors), 4D, **semi-automatic** curved MPR centreline, DSA, remaining specialty measurements, mammography/tomosynthesis + CAD marks, ECG, teaching layouts and quiz mode; LAS/LAZ/E57 octree streaming, **COPC**, GPU LOD while loading, E57 stations, **Gaussian splats** | 1-billion-point cloud interactive |
| **M9** | Long tail | Pathology/WSI, **remote/cloud sources (§12.8)**, OME-Zarr, vendor microscopy, N-D viewing + channel compositing + stretch + scale bar (§9.1), FITS/XISF + WCS, **astro subframe triage, debayer, fpack, SER, SAMP**, EM calibration (SEM tags, DM3/DM4, MRC, EMD), ENVI/SAFE/STAC, IIIF, scientific, PSD and design formats, legacy formats, GIS raster display, DWF/CGM, 3D Tiles, OCIO, local AI (§10), OCR/QR, libvips catch-all, native DWG research | Format coverage matrix published |

The key binding configurator (§11) lands with the library stage (M2), once enough actions exist to need it.

M3–M4 (medical) come before M5–M6 (engineering), so the shared 3D and measurement infrastructure is
built for medical first. User profiles (§11) and the measurement layer (§6.8) come first, ahead of the
domain stages, because every domain builds on them. Colour-vision-safe maps, the no-network mode and the
IPC clients can land at any time. DGN V8 is out (§8.1).
