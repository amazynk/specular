# Specular Develop sidecar format

Frozen since version 1. Code: `crates/specular-develop/src/sidecar/`.
Fixture of every module kind: `crates/specular-develop/tests/fixtures/sidecar/every-module.specular.xmp`
(a test fails when the written form changes).

## Files

- `IMG_1234.CR2.specular.xmp`: the edit (version 1) of `IMG_1234.CR2`.
- `IMG_1234.CR2.vN.specular.xmp`: version N (N ≥ 2), with `spd:name`.
- Styles: the same form, no source, with `spd:name`.
- Rename, move, copy, delete and their undo carry all of these with the photo.
- Written atomically (temporary file, then rename). The library keeps its own copy.

## Namespace

- `http://ns.specular.app/develop/1/`, prefix `spd`.
- One `rdf:Description` with `spd:schema` is Develop's. Nothing else in the file is.

## Description attributes

| Attribute | Value |
|---|---|
| `spd:schema` | edit format version (now 1) |
| `spd:process` | process version: the algorithm generation the edit was made with (now 1) |
| `spd:writer` | `Specular <version>` that wrote it |
| `spd:source` | `size=<bytes> mtime=<ns> hash=<64 hex>`: the photo it was made for |
| `spd:customOrder` | `True` when modules are not in the recommended order |
| `spd:editHash` | 64 hex: the edit's output hash (thumbnails key on it) |
| `spd:name` | version or style name (optional) |

## Elements

- `spd:masks` (optional) → `rdf:Seq` → one `rdf:li` per shape, `spd:id` and `spd:shape`:
  - `ellipse`: `spd:centre`, `spd:radii`, `spd:angle`, `spd:feather`.
  - `rectangle`: `spd:centre`, `spd:half`, `spd:angle`, `spd:round`, `spd:feather`.
  - `polygon`: `spd:points` (x y feather per point), `spd:closed`.
  - `linear`: `spd:start`, `spd:end`. `radial`: `spd:centre`, `spd:inner`, `spd:outer`, `spd:angle`.
  - `brush`: `spd:strokes`, base64 of the catalogue codec (each stroke also
    carries what pen pressure changes: flow, size or both; older strokes read as flow).
  - Points are fractions of the image (0..1), numbers separated by spaces; angles in degrees.
- `spd:modules` → `rdf:Seq` → one `rdf:li` per module instance, in pipeline order:
  - `spd:module`: the registry name (below), or `kind_<number>`.
  - `spd:version`: the module's param version. `spd:instance`: 0, 1, …
  - `spd:enabled`: `True`/`False`. `spd:label`: optional.
  - `spd:params`: one `spd:<field>` attribute per named field.
  - `spd:blend` (optional): `spd:mode` (`normal`, `multiply`, `divide`, `add`, `subtract`,
    `lighten`, `darken`, `luminance`, `colour`, `hue`, `chroma`, `screen`, `overlay`,
    `soft-light`), `spd:opacity`, `spd:invert` (only when `True`), `spd:feather`, `spd:blur`,
    `spd:contrast`, `spd:offset`, `spd:edgeAware`, `spd:details`, `spd:radius` (the
    edge refinement radius; only when not 0.02), `spd:mask`.
  - `spd:mask` is an expression: `(all)`, `(shape ID)`,
    `(range CHANNEL input|output a b c d)` (channel `luminance`, `chroma`, `hue`, `red`,
    `green`, `blue`), `(colours DISTANCE FALLOFF r g b …)`, `(module NAME INSTANCE)`,
    `(ai "LABEL" KEY64HEX)`, `(not E)`, `(union E E)`, `(intersect E E)`,
    `(subtract E E)`, `(exclude E E)`, `(opacity K E)`, `(details THRESHOLD SOFTNESS)`
    (textured areas, EV). `(module NAME INSTANCE)` is a live reference to that
    instance's rows and invert.
  - AI rows: `(ai KIND "MODEL" [box X0 Y0 X1 Y1] [+ X Y] [- X Y] …)`, `KIND` one of
    `subject`, `sky`, `object`; `MODEL` the Local AI manifest id it was made with
    (`efficientsam-ti`); for objects the box and clicks (`+` on the object, `-` off it) in
    source-frame fractions, at most six points (a box counts two). The computed mask is
    never stored: a reader recomputes it, or masks nothing while the model is missing. The
    older placeholder `(ai "LABEL" KEY64HEX)` still reads (as a row of that kind with no
    model).
  - Depth rows: `(depth "MODEL" NEAR FAR FEATHER)`: the part of the photo whose
    depth (0 nearest, 1 farthest) is between NEAR and FAR, edges FEATHER wide; `MODEL`
    the depth model's manifest id. The depth map is never stored.
- `spd:snapshots` → `rdf:Seq` → `rdf:li spd:name="…"` holding its own `spd:masks`
  (optional) and `spd:modules`.
- `spd:history`: base64 of the catalogue codec (optional).

## Values

- `True` / `False`; integers; floats in shortest round-trip form (with `.` or exponent);
  text; float lists separated by spaces.
- A field's type comes from the module's defaults; unknown fields are read by their form.

## Module names

`flat_field`, `hot_pixels`, `chromatic_aberration`, `raw_front_end`, `denoise`, `defringe`,
`capture_sharpening`, `lens`, `crop_orientation`, `retouch`, `exposure`, `colour_calibration`,
`tone_equalizer`, `local_contrast`, `colour_balance`, `colour_equalizer`, `point_colour`,
`black_white`, `curves`, `lut_3d`, `sharpen`, `view_transform`,
`surface_blur` and `diffuse` (`iterations` an integer; `radius`, `span` and
surface blur's `radius` are fractions of the long side).
Effects: `negative`, `dehaze`, `graduated_nd`, `vignette`, `glow`, `grain`, `frame`
(borders and watermark: exports and prints only; `spd:svg` is a file path, `spd:text` a
template with `$(name)` variables; grain's `spd:seed` fixes its pattern).
Graduated ND and vignette positions are fractions of the cropped picture.
`perspective` (keystone and upright): `mode` (`off`, `guided`, `level`, `vertical`, `full`,
`auto`); `spd:guides` four floats per line, fractions of the source frame; `spd:solved` the
pitch, yaw and roll (degrees) the last automatic run found; `spd:focal_mm35` 35 mm
equivalent focal length. The last three are written only when set.
AI denoise: `denoise` may carry `spd:mode` (`classic` or `ai`), `spd:model` (the
Local AI manifest id, `ffdnet-color`) and `spd:blend` (the AI result's share, 0 … 1).
Missing fields mean classic. The AI result itself is never stored: a reader recomputes it,
or shows the classic result while the model is missing.
Field names and defaults: the fixture.

## Retouch spots

`retouch` keeps its spots as columns, one entry per spot, in order:

| Field | Per spot |
|---|---|
| `spd:spot_mode` | `heal`, `clone`, `fill`, `blur` or `remove` (words; the count of spots) |
| `spd:spot_layer` | `all`, `low` or `high` (frequency separation layer) |
| `spd:spot_target`, `spd:spot_source` | x y: fractions of the frame |
| `spd:spot_size` | radius, fraction of the frame's long side |
| `spd:spot_feather`, `spd:spot_opacity` | 0..1 |
| `spd:spot_blur` | blur σ, fraction of the long side (blur spots) |
| `spd:spot_points` | brush path points (0: a round spot) |
| `spd:spot_path` | the path points (x y), spot after spot |
| `spd:spot_model` | removal spots: the inpainting model's manifest id, `-` for others (written only when a spot names one) |

- `spd:split_radius`: the frequency separation σ in source pixels.
- A short column gives the missing spots their defaults; an unknown mode reads as `heal`.
- Spots are in source coordinates, so crop, rotate and lens changes keep them in place.
- A `remove` spot keeps its stroke, model and blend, never the fill; a reader
  recomputes the fill, and heals from `spot_source` until then. `relight` (`near`, `far`
  EV, `split`, `transition`, `model`) and `depth_blur` (`amount`, `focus`, `range`,
  `highlights`, `near`, `model`) read the photo's depth map the same way.

## Compatibility

- **Forward:** a module, field, attribute or element this build does not know is kept and
  written back unchanged. An unknown module stays after the module it followed.
- A field unknown to this build in an item of an older module version is dropped (a
  migration renamed it).
- Unknown modules or a migration: the history is not trusted and starts fresh.
- **Process version:** an edit keeps its process number when written. A newer process
  than this build's shows "may look different here". Old process numbers render with the
  kernels of their generation.
- In earlier sidecars, `spd:masks`, `spd:blend` and `spd:snapshots` were base64 attributes. They
  are still read and are written in the readable form.
- A mask shape, mask item or blend mode this build does not know is left out of the edit
  shown (a module whose blend it was is shown off), with a notice. Such a file, and one
  that does not read at all, is never written over: nothing of that edit is saved.
- **Other tools:** everything outside Develop's description stays byte for byte. Develop's
  description is added before `</rdf:RDF>` when the file has none.

## Conflicts

On open, a sidecar newer than the library's copy with a different edit hash asks which to
keep, naming both times and the writer. Nothing is overwritten until answered.

## Other tools' edits

- Lightroom / Camera Raw `crs:` (photo's `.xmp` or embedded XMP): imported on request, or
  by setting when no Specular edit exists. Approximate; mapping in `interop/crs.rs`.
  Never written back.
- darktable (`IMG.ext.xmp` with `darktable:history`): module names listed, never converted.
