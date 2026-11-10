---
id: develop
title: Develop
section: modes
contexts: develop
keywords: develop, f6, regions, filmstrip, light, profile, colour grading, add a tool, tools, edit, editing, raw, raw detail, demosaic, x-trans, tone equalizer, contrast and range, crop, straighten, rotate, flip, aspect ratio, guides, lens correction, lens profile, lensfun, optics, distortion, vignetting, chromatic aberration, exposure, auto exposure, contrast, white balance, tone, highlights, shadows, texture, clarity, local contrast, curve, levels, colour wheels, grading, vibrance, saturation, chroma, brilliance, colour intensity, colour brightness, colour mixer, hsl, point colour, black and white, lut, cube, 3dl, clf, history, undo, before, after, detail, noise, noise reduction, denoise, sharpen, sharpening, capture sharpening, masking, speed edit, non-destructive, export, preset, colour space, tiff, exr, look, sigmoid, filmic, agx, hdr, hdr limit, sdr preview, pq, hlg, gain map, ultra hdr, style, styles, presets, snapshot, snapshots, version, versions, virtual copy, copy edits, paste edits, sync, match to reference, auto-apply, compress history, next capture, corrections, highlight recovery, blown highlights, clipped, hot pixels, dead pixels, defringe, purple fringe, fringe, flat field, flat frame, dark frame, calibration, mask, masks, local adjustment, brush, gradient, graduated filter, ellipse, radial filter, luminance range, colour range, refine edges, blend mode, overlay, soft light, instance, layer, layers, detail mask, texture mask, use mask from, mask from module, borrowed mask, mask preset, mask presets, pen pressure, tablet, stylus, smoothing, edge radius, grow edge, blur mask, polygon point, edited thumbnail, edited badge, edited:, module:, style:, version:, sidecar, xmp, conflict, lightroom, camera raw, crs, import, darktable, open in darktable, other tools, rename, move, scopes, histogram, waveform, luma, parade, rgb parade, vectorscope, skin tone line, clipping, clipping warning, highlight clipping, shadow clipping, scene clipping, display clipping, gamut, gamut warning, out of gamut, soft proof, soft-proofing, proof, printer profile, icc, paper white, ink black, black point compensation, rendering intent, perceptual, relative colorimetric, print, retouch, heal, healing, spot removal, clone, clone stamp, fill, blur brush, frequency separation, dust, blemish, spot outlines, effects, vignette, graduated nd, nd filter, grain, film grain, glow, bloom, soften, orton, dehaze, haze, fog, add haze, negative, film negative, density range, paper grade, paper gloss, invert, film base, orange mask, paper grade, border, borders, frame, canvas, square, watermark, svg watermark, copyright, surface blur, bilateral, edge-aware, skin smoothing, diffuse, diffuse or sharpen, soften and sharpen, diffusion, deblur, lens blur, inpaint, fill highlights, iterations, anisotropy, merge to hdr, hdr merge, bracket, bracketing, exposure bracket, aeb, auto bracket, align, alignment, ghost, ghosts, deghost, ghost reduction, float dng, floating point dng, hdr dng, ai mask, ai masks, ai denoise, ai noise reduction, ffdnet, select subject, select sky, subject mask, sky mask, object mask, click to mask, segment, segmentation, tone zones, zones, raw+jpeg, pair, cmyk, transparency, alpha, remove object, object removal, ai remove, generative remove, distraction removal, erase, inpainting, lama, depth, depth map, depth blur, bokeh, background blur, portrait blur, focus, depth of field, relight, relighting, near, far, depth mask, depth anything, refining, draft, preview while dragging, smooth sliders, slow sliders
---
Develop edits a photo without changing its file. Edits are kept apart from the file.

Press [[key:develop]] in the viewer to open it, and again to leave. Esc also leaves.

Develop is off in the Medical and Science profiles.

For a RAW+JPEG pair, Develop edits the RAW.

Photos with a colour profile (Display P3, Adobe RGB) keep their colours. Transparency is kept in PNG, TIFF, AVIF, JPEG XL and EXR exports. CMYK photos are edited in RGB.

## The panel {#panel}
The panel holds a card per task. Click a card's title to open or close it.

The basic cards (Light, White balance, Tone zones, Local contrast, Curve, Colour, Colour mixer, Colour grading) have no switch: reset their sliders to undo them. Effects and tools have a switch that turns them off.

Tools most photos never need wait in Add a tool, the last card: Point colour, LUT, Surface blur, Soften and sharpen, Film negative, Graduated filter, Glow, Borders and watermark, Relight and Depth blur. Press one to add its card. A photo whose edit uses a tool shows its card from the start.

- Drag a slider. Hold Shift for fine steps.
- On a slow computer the photo looks softer while you drag. Settings › Performance › Preview while dragging changes this.
- Double-click a slider to reset it.
- To type a value, type a number while the slider has the keyboard, then press Enter.

[[key:develop-panel]] moves the keyboard to the panel. Tab goes from control to control. Esc comes back to the image.

[[key:develop-next-region]] moves the keyboard on: picture, panel, filmstrip. [[key:develop-prev-region]] goes back. In the filmstrip, the arrows open the next or previous photo.

On a slider, arrows step, Shift steps more, Alt steps less. Delete resets.

A control that needs another setting first turns it on as you move it. Example: moving Red shift sets Fringes to Manual.

A dimmed control does nothing yet. Point at it, or tab to it, to see what it needs.

## Light {#tone}
Light holds the six sliders most photos need, in Lightroom's order: Exposure, Contrast, Highlights, Shadows, Whites and Blacks.

- Auto exposure sets exposure from the photo's own brightness. The Exposure slider then adds to it.
- Exposure is in stops (EV). The other five go from −100 to +100, and 0 leaves the photo as it is.
- Highlights and Shadows brighten or darken whole areas. Edges stay sharp, so no glow appears around them.
- Whites and Blacks set where the brightest and darkest tones land.

Light has no on/off switch. To undo it, reset the sliders (double-click one, or use the ⋯ menu).

## Tone zones {#zones}
Tone zones sits under Light and starts closed. A dot on its header means a zone is set.

- Adjust on the picture: drag up or down on a spot to raise or lower its tone. Esc ends.
- One slider per zone, from −8 EV (black) to 0 EV (white).
- Smoothing sets how far each change spreads across an area.

Texture and Clarity are in Local contrast. Texture brings out fine detail, Clarity larger shapes. Below zero, both soften.

## Profile {#look}
Profile, at the top of Light, sets how scene brightness becomes screen brightness. Mid grey stays where it is.

- Standard: smooth and neutral. The default.
- Film: a film-like curve with a firm end at white.
- Natural: bright, strong colours fade smoothly to white, the way they do to the eye.

Contrast, Whites and Blacks shape each of them.

History calls this module Contrast and range. Darktable users know the profiles as sigmoid, filmic and AgX.

Black level, in Corrections, corrects the sensor's black on raw files.

## HDR {#hdr}
On an HDR screen, Develop shows the edit in HDR. Highlights go above SDR white.

- HDR limit: how far above SDR white, in EV. 2 EV is four times as bright.
- Show: HDR, or SDR to see the photo as SDR screens will.

The SDR picture is what an SDR export looks like.

## Detail {#detail}
A raw photo starts with Colour noise at 25% and Capture sharpening at 30%, much like the camera's own JPEG. Luminance noise starts at 0.

Resetting either slider goes back to its starting value. A saved edit keeps the values it had.

- Raw detail (raws) sets how the sensor's colours are filled in. Standard follows edges; Fast is quicker. Smooth flat areas follows edges on detail and smooths flat areas such as sky.
- Flat-area contrast: below this contrast, Smooth flat areas smooths. Raise it if flat skies look noisy.
- Luminance noise smooths grain. Noise detail keeps fine texture. Colour noise removes colour blotches.
- Noise reduction knows the camera's noise: from the raw file, or measured on the photo.
- AI denoise mixes in a network's result. AI share sets how much. See [AI denoise](#ai-denoise).
- Sharpen sharpens edges and texture. Sharpen radius sets the size of the detail.
- Masking leaves smooth areas such as sky and skin alone.
- Sharpen detail lets edges overshoot a little. At 0, edges get no light or dark rims.
- Capture sharpening undoes the lens and sensor blur. Capture radius 0 takes the blur from the lens.

## Surface blur {#surface-blur}
- Surface blur smooths areas and keeps their edges. Good for skin and skies.
- Radius sets how far it smooths, as a share of the photo's long side.
- Edge threshold sets how big a change in brightness counts as an edge.
- Amount mixes the smoothed photo in. Use a mask to smooth only part of the photo.

## Soften and sharpen {#diffuse}
- Soften and sharpen spreads detail out, or pulls it back together. darktable calls it diffuse or sharpen.
- Pick a preset: Sharpen, Lens blur, Local contrast, Bloom, Fill clipped highlights or Smooth.
- Positive speeds diffuse: they soften, glow and fill. Negative speeds sharpen and deblur.
- Speeds 1 and 2 work on the coarse layer, 3 and 4 on the fine layer.
- Direction above 0 works along edges, below 0 across them.
- Radius and Radius span pick the size of detail, as a share of the photo's long side.
- Passes repeat the work. More is stronger and slower.
- Edge sensitivity protects edges. Edge threshold keeps flat areas and their noise from sharpening.
- Only above changes only highlights brighter than this. 0 changes the whole photo.
- While you drag, a quick version shows. The full one follows when you stop.
- "Refining…" over the picture means the full version is on its way.
- Very fine detail shows only at 100%. The card says so.

## Effects {#effects}
Each effect is a card of its own, with a switch and masks.

- Film negative turns a scanned film negative into a positive. It is off until switched on.
- Pick film base, then click the film's clear edge. The orange mask goes.
- White point sets where the picture turns white. Brightness darkens or lightens it.
- Contrast works like a darkroom paper grade. Black depth deepens the blacks.
- Film: Black and white for black-and-white negatives.
- Dehaze removes haze; below 0 it adds haze. Far, hazy parts change most.
- Graduated filter darkens (or lightens) from one side, like an ND filter. Tint colours it.
- Edit on picture shows three handles: drag the middle to move it, the ends to turn and soften it.
- Vignette darkens or lightens the edges. It follows the crop.
- Midpoint and Feather set where and how softly it starts. Roundness 100% makes a circle.
- Shape below 0 is a diamond, above 0 nearly a rectangle. Protect highlights spares bright skies and lamps.
- Glow: Bloom lets bright parts spill light. Soften gives a dreamy glow over the whole picture.
- Radius is a share of the photo's size; Threshold sets how bright a part must be to glow.
- Grain adds film grain. Size is a share of the photo, so grain looks the same zoomed out and exported.
- Tones moves the grain towards shadows or highlights. Each photo has its own grain, the same every time.

## Borders and watermark {#frame}
Borders and the watermark go on exports and prints only. The picture stays as it is.

- Show on picture shows them while you edit.
- Border is a share of the photo's long side. Canvas pads to 1:1, 4:5, 3:2 or 16:9.
- Watermark text… sets the text. $(title), $(date), $(filename) and $(exif.camera) become the photo's own.
- Also: $(exif.lens), $(exif.iso), $(exif.exposure), $(exif.aperture), $(exif.focal), $(exif.artist), $(exif.copyright).
- Choose SVG… uses an SVG file instead. Variables in its text work too.
- Place, Watermark size and Watermark opacity set where and how it shows.
- HDR exports get them too. With a gain map, the border and watermark keep the same light in both renditions.

## Curve {#curve}
- Click the curve to add a point, drag it to move it. Drag it out of the square to remove it.
- Arrows move the selected point. Page Up and Page Down pick another. Insert adds a point, Delete removes it.
- The curve keeps colours: it changes brightness, not hue.
- Black point, Midpoint and White point set the levels before the curve.

## Colour {#colour}
Apart from the wheels, colour changes never tint greys. Hue stays put unless you move it.

- Vibrance lifts dull colours more than vivid ones.
- Saturation also darkens colours as they get stronger, so they keep their weight.
- Colour grading, under the colour cards and closed at first, has the colour wheels: drag in one to tint shadows, midtones, highlights or the whole photo. Arrows move it too.
- Also in Colour grading: Colour intensity changes all colours evenly, and Colour brightness brightens or darkens colourful areas. Greys stay.
- Colour mixer: hue, saturation and brightness for eight colours. Changes blend smoothly between them.

## Camera profile {#camera-profile}
Camera profile, in the White balance card of a raw, sets how the camera's colours become the photo's.

- Camera standard: the file's own profile, or the camera's matrix.
- Matrix only: the camera's matrix, without the profile's colour tables.
- Profile file…: a DNG camera profile (.dcp), such as one made from a chart. The line under it says if the file is missing; the matrix is used then.

## Profile from a chart {#chart-profile}
Make profile from chart…, in the White balance card, measures a photo of a colour chart and makes a profile for that camera and light.

- Shoot the chart square on, evenly lit, filling part of the frame. No glare, nothing clipped.
- Chart: ColorChecker 24 (also Passport), ColorChecker SG or IT8.7/2. SpyderCHECKR isn't supported.
- Reference: the chart's reference file (CGATS text). It comes with the chart. X-Rite publishes ColorChecker values on its website; each IT8 batch has its own file from its maker.
- Click the centres of the four corner patches, in the order shown. Each click finds its patch. Esc closes.
- The report shows each patch: reference on top, the profile's colour below. ΔE2000 below 1 is hard to see; below 3 is good.
- Patches with glare, dust or clipping are left out and named. Uneven light across the greys is reported.
- Save… names the profile. A raw gets a .dcp in Specular's profile folder, and the photo uses it. Other raw editors read it too.
- Use for photos from this camera also saves a style with only the profile, and a rule that applies it to new photos from the camera.
- Other images get the profile as a colour matrix in White balance and a LUT (.cube) in the LUT card.

## Point colour {#point-colour}
- Press Pick a colour, then click the colour in the photo. Esc cancels.
- Hue, Saturation and Lightness shift move that colour and colours near it.
- Range sets how near a colour must be. Hue, saturation and lightness range narrow or widen each.
- While you drag a range slider, what moves keeps its colour and the rest turns grey.

## Black and white {#black-white}
Turn the card on to make the photo grey. Each colour's slider makes it lighter or darker in grey.

## LUT {#lut}
A LUT file applies a look made elsewhere: .cube, .3dl, .spi3d, .spi1d, CLF or CTF.

- Choose a LUT file… picks the file. The file is read, not copied: keep it where it is.
- Made for: the colours the LUT expects. Most looks are made for Photo (sRGB), Video (Rec.709) or ACES log (ACEScct). The file's name or notes usually say which.
- Method: Accurate suits most LUTs. Simple matches tools that use trilinear interpolation.
- Strength mixes the look with the photo.

## Crop and straighten {#crop}
[[key:develop-crop]] shows the whole photo with the crop over it. Enter or [[key:develop-crop]] keeps the crop. Esc goes back.

- Drag a corner or an edge to resize. Drag inside to move.
- Ctrl+drag along a line that should be level or upright. The photo turns to match.
- Arrows move the crop, Shift more. Ctrl+arrows resize it.
- `,` and `.` straighten by 0.1°, Shift by 1°. The Straighten slider does the same.
- A: next aspect (free, original, 1:1, 4:3, 3:2, 16:9, the current shape). X swaps it.
- O: next guide (thirds, golden ratio, diagonals, grid).
- `[` and `]` turn the photo. H and V flip it.
- Delete resets the crop and the angle.

The Crop card does the same without the tool:

- Aspect: free, original, 1:1, 4:3, 3:2, 16:9 or the current shape.
- Type a ratio…: any ratio, such as 5:4 or 1.85.
- Swap portrait and landscape, Turn left, Turn right and the two flips.

The crop always stays inside the photo.

## Perspective {#perspective}
The Perspective card makes leaning buildings stand up straight.

- Upright Off uses the sliders only. Level fixes the tilt of the horizon.
- Vertical makes upright lines plumb. Full also makes level lines level.
- Auto is Full, careful with weak level lines.
- Auto, Level, Vertical and Full look for straight lines in the photo when you choose them. With too few lines nothing changes, and you are told why.
- Guided uses lines you draw. Draw guides opens the crop tool on them.
- In the guides, drag along something that should be plumb or level, up to four lines. A loupe shows the end.
- Drag an end to move it. Delete removes the last one touched.
- Two upright guides fix the tilt. Two level guides fix the turn.
- Vertical and Horizontal tilt and turn the photo as a camera would. Rotate turns it up to 10°.
- Aspect makes the photo wider or taller. Scale zooms. Sideways and Up or down move it.
- Avoid empty edges zooms so the corrected photo fills the frame.
- In the crop tool, Alt+arrows change Vertical and Horizontal. Shift+R switches between crop and guides. The outline shows the corrected photo's edges.
- Reset perspective sets it all back. It asks first when there are guides.

## Optics {#lens}
The Optics card fixes what the lens did: distortion, darkened corners and colour fringes.

- Lens corrections: on by default, the corrections stored in the file. DNG files, and Sony and Fujifilm raws, carry them. Off turns all lens corrections off, the manual ones too.
- Lens profile: for other raws (Canon, Nikon, Pentax and more), a profile from the Lensfun database, matched to the lens the file names. The note under it says which profile applies.
- When no profile matches, or two fit equally well, none applies: pick the lens in Lens profile, or None.
- Distortion and Vignetting add manual corrections on top.
- The picture zooms slightly so no empty edges show.
- Chromatic aberration: Automatic measures red and blue fringes across the photo. Manual uses Red shift and Blue shift; moving either sets Manual. For raws it works on the sensor data, Bayer and X-Trans.
- Defringe removes purple and green fringes left beside strong edges. Purple hue, Green hue and Hue range pick the colours; Edge contrast picks the edges.
- These corrections act on small areas. Zoom in to see them. The histogram barely moves.
- Edits made before Optics may have Red fringe and Blue fringe set. These show on the card while they are in use; set them to 0 and use Red shift and Blue shift.

Sony files store a vignetting table. It is not applied: it does not match the cameras' own pictures. Use Vignetting.

## Corrections {#corrections}
The Corrections card fixes what the sensor got wrong. Raws only: it acts on the sensor data before demosaic.

- Clipped highlights: Rebuild colour (the default) gives blown areas the colour around them. They never turn magenta.
- Rebuild large areas also brightens big blown areas, such as the sun, towards their middle.
- Clip to white makes blown areas neutral white.
- Clipping level: values above this share of the sensor's maximum count as clipped.
- Hot pixels: on by default. Fixes single stuck or dead pixels. Raise the threshold if star points get removed.
- Calibration frames: a flat frame evens out vignetting and dust; a dark frame removes heat noise and hot pixels.
- Chosen files: pick a flat and a dark frame of the same camera.
- From folder: picks frames by camera, lens, aperture and ISO from the folder in Settings › Metadata and privacy › Calibration frames folder. Their file or folder names contain "flat" or "dark".
- Flat frame: Vignetting and dust corrects both. Vignetting only uses the flat frame's smooth falloff and leaves dust alone.

Order: dark and flat frames, hot pixels, highlights, chromatic aberration, demosaic. Lens corrections follow in the one geometry step.

## Masks {#masks}
Masks limit a card's change to part of the photo.

- New mask picks a kind; Add mask adds it to the card's Masks section.
- Shapes: ellipse, rectangle, polygon, linear gradient, radial gradient, brush.
- Ranges: luminance, hue, saturation, and a colour range from picked colours.
- Detail picks textured areas. Threshold and Softness set how much texture counts.
- Use mask from adds another module's mask as a part. It follows that mask as it changes.
- Deleting a module whose mask others use asks first. They keep a copy.
- Each part adds to, subtracts from or intersects the parts above it.
- Each part has its own opacity and Invert.
- Edit on the picture shows the shape's handles. Drag them.
- With the picture focused, Tab walks the handles and arrows move them. Shift moves further.
- A brush part paints on the picture. Brush size, feather, flow and Erase are on its card.
- Pen pressure changes the flow, the size or both. A mouse paints at full pressure.
- On Linux, pen pressure works on X11 only. On Wayland a pen paints at full pressure.
- Smoothing makes the stroke trail the pointer, for steadier lines.
- [[key:develop-brush-smaller]] and [[key:develop-brush-larger]] make the brush smaller or larger. [[key:develop-brush-softer]] and [[key:develop-brush-harder]] make it softer or harder.
- [[key:develop-brush-erase]] switches between painting and erasing.
- Polygon: click an edge to add a point. Alt+click a point to remove it, or click it and press Delete.
- With a point chosen, Insert adds one after it, and Feather sets that point's feather.
- Mask amount, Invert mask and Blend set how the change mixes in.
- Blend: normal, multiply, screen, overlay, soft light, lightness only, colour only.
- Refine edges makes the mask follow edges in the photo. Edge radius sets how far.
- Grow edge softens the mask outwards. Blur mask softens it both ways.
- Keep detail (+) keeps the mask on texture; (−) keeps it on smooth areas.
- Show mask tints the mask red, or shows it alone in grey. [[key:develop-mask-overlay]] shows or hides it.
- Save mask as preset keeps the mask by name. Mask preset puts it on another photo.
- Presets are stored upright, so they land on the same part of a turned photo.
- Mask presets are small `.xmp` files in `develop-mask-presets` in the settings folder.
- Deleting a preset asks first. Photos it was put on keep their masks.
- Masks stay on the subject when you crop, straighten or correct the lens.
- Removing a mask or a part asks first. The photo on disk never changes.

## AI masks {#ai-masks}
Subject, Sky and Object at click are mask parts found by Local AI on this computer.
- They need the Masks feature in Settings › Local AI. Subject and Sky are add-ons of it.
- Without its model a part says which add-on it needs and masks nothing. Nothing downloads from here.
- A new part is worked out once per photo, then kept. The panel says when it is ready.
- Object at click: edit the part, then click the object. Alt+click marks what is not part of it.
- Drag on the picture for a box around the object. Up to six points; a box counts as two.
- Clear points starts over. Each click is one step in History.
- AI parts combine with the others: add, subtract, intersect. Refine edges works on them too.
- Their edges follow the photo's edges at any size, so an export stays sharp.
- The sidecar keeps the part and its clicks, not the mask. Another computer works it out again.

## AI denoise {#ai-denoise}
AI denoise removes noise with a network on this computer, mixed with the classic result.
- It needs the Denoise feature in Settings › Local AI. Nothing downloads from here.
- Without its model the card says which add-on it needs. The classic result shows.
- It runs at 100% view, on the part on screen, and on the whole photo for an export.
- Until it is done the classic result shows. Stop keeps what is done.
- AI share 100% is the network's result only. Lower values mix in the classic result.
- The Luminance and Colour noise sliders set the classic result it mixes with.
- Results are kept in the cache folder. The sidecar keeps the mode, model and share.

## Layers {#instances}
A card can run its module more than once, as layers, each with its own mask.

- New layer, Duplicate layer and Delete layer are on the card. Delete asks first.
- Layer picks which one the card shows.
- Move earlier and Move later change their order.

## Retouch {#retouch}
Retouch removes spots, dust and small distractions.

- Open the Retouch card. A click on the picture places a spot.
- Its source is found for you nearby. Drag the source circle to choose another.
- Drag a spot to move it. Drag its edge handle to size it.
- Heal copies texture and blends the edge into its surroundings.
- Clone copies the source as it is.
- Fill takes tones from the surroundings and grain from the source.
- Blur softens the area.
- Area: Spot places round spots; Brush paints an area of any shape.
- Acts on: the whole picture, only its tones, or only its texture.
- Tones/texture split sets where tones end and texture starts.
- Size, Feather and Opacity change the selected spot, or the next one.
- `[` and `]` make the selected spot, or the next one, smaller or larger.
- The card lists the spots. A row selects its spot.
- Remove spot, or Delete on the picture, asks first. The photo on disk never changes.
- Esc clears the selection. [[key:develop-spot-outlines]] shows or hides the outlines.
- From the keyboard: pick a spot in the card's list, then Esc to the picture. Tab walks the spot's handles; arrows move one (Shift: ten times as far). Esc leaves them.
- Without a mouse, [[key:develop-spot-crosshair]] shows a crosshair. Arrows move it; Shift moves ten times as far.
- Enter places a spot of the chosen tool there; its source is found for you. Esc puts the crosshair away.
- Spots stay in place when you crop, straighten or correct the lens.

## Remove (AI) {#ai-remove}
Remove (AI) fills a brushed area with what could be behind it. It runs on this computer.
- Choose Remove (AI) as the Tool, Brush as the Area, then paint over what to go.
- Paint a little past its edge, shadow included.
- It needs the AI remove feature in Settings › Local AI. Nothing downloads from here.
- Until the fill is ready, or without the model, the area heals from a nearby source.
- The selected spot says what it is doing. Stop ends the work; Compute again restarts it.
- The fill blends into its surroundings in linear light, so its edge does not show.
- Large areas come out softer than small ones. Several small strokes work best.
- The sidecar keeps the stroke and model, not the fill. Another computer works it out again.

## Depth {#depth}
Depth blur, Relight and depth mask parts use the photo's depth map, found on this computer.
- They need the Depth feature in Settings › Local AI. Nothing downloads from here.
- The map is worked out once per photo, then kept. The cards say when it is ready.
- Depth runs from 0 (the nearest part of the photo) to 100 (the farthest).
- Depth blur blurs what is away from the focus, like a wide aperture. Lights become discs.
- Pick focus, then click what should be sharp. Focus range sets how much stays sharp.
- Blur the foreground off keeps what is nearer than the focus sharp.
- Relight brightens or darkens the near and far parts apart. Split sets where far begins.
- Near, Far and Depth range are mask parts for any card. From, To and Feather set the range.

## Before and after {#before-after}
- [[key:develop-before-after]]: the original, or the edit again
- [[key:develop-split]]: the original and the edit side by side
- Drag the divider to move it.

## History {#history}
Every change is a step in the History card. Click a step to go back to it.

- [[key:undo]]: undo
- [[key:redo]]: redo

A change made after going back replaces the later steps.

The History and snapshots card:

- Take snapshot ([[key:develop-snapshot]]) names the edit as it is now.
- Click a snapshot to compare it with the edit, split down the middle. Click again to stop.
- Right-click a snapshot to go back to it, rename it or delete it.
- Compress history drops steps that later ones replaced. The edit stays the same.

## Styles {#styles}
A style is a set of modules with their values, applied to any photo in one step.

- Point at a style, or move the keyboard to it, to see it on the photo.
- Click it to apply it. Right-click for Apply, replacing these modules, Export…, Rename and Delete.
- New style from this edit… saves the modules you choose.
- Import style… reads a style file another Specular saved.
- Apply to new photos from this camera adds an auto-apply rule: photos from that camera get the style the first time they open.
- Auto-apply rules… lists the rules. Choose one to change its maker, model, lens, ISO range, file type or style, or to delete it.

Styles are kept in the settings folder, one file each.

## Presets {#presets}
Right-click a module's card, or press Shift+F10 on it, for its presets.

- Choose a preset to set the module to it.
- Save as preset keeps the module's values under a name.
- Some modules come with presets, such as High ISO for noise reduction.

## Copy and paste {#copy-paste}
- [[key:develop-copy]]: copy the edit, all but crop and lens
- [[key:develop-copy-some]]: choose the modules to copy
- [[key:develop-paste]]: paste on the photo, or on the photos selected in the gallery
- [[key:develop-paste-some]]: choose the modules, and whether they add to or replace the edit

Each photo gets one history step, so one undo takes a paste back.

If some photos' edit files changed outside Specular, you are asked once for all of them:
- Keep the file's changes: the paste goes on top of them.
- Replace them: the paste goes on the library's copy.
- Skip those photos: they stay as they are.

In cull mode, [[key:develop-sync]] puts the current photo's edit on every picked photo. [[key:develop-sync-some]] lets you choose the modules. Paste edit on picks does the same from the command palette.

## Versions {#versions}
A version is another edit of the same photo, with its own history. No file is copied.

- The Versions card lists them. Click one to open it.
- New version starts from the original. Duplicate this version copies the edit.
- Right-click a version to rename or delete it. Deleting asks first; the photo stays.
- The gallery shows how many versions a photo has. They stack under the photo, each with its own thumbnail.

Each version has its own sidecar, named like IMG_1234.CR2.v2.specular.xmp.

## Merge to HDR {#merge-hdr}
Merge raws of one scene at different exposures into one raw with more range.

1. In the gallery, select the raws, or right-click one of a bracket.
2. Choose Merge to HDR. One raw finds its bracket by time, camera and exposure.
3. Leave out photos if you like, then choose Merge.

- Align photos: handheld shots are lined up, shift and rotation.
- Reduce ghosts: where something moved, that part comes from one photo.
- Keep the raw mosaic: off saves demosaiced colour instead.
- Float depth: 16, 24 or 32 bits. 24 is plenty for most scenes.
- Open in Develop: the merge opens when done.

The merge is saved as a DNG next to the photos, named like IMG_1234-HDR.dng. Nothing is replaced.

When the photos are DNGs, the merge keeps their lens corrections and bad-pixel lists that still fit.
Sony and Fujifilm lens corrections go into the merge as DNG lens corrections.

It stacks with its photos in the gallery. Progress shows in the downloads list, with Stop.

When parts were taken from one photo, the toast offers Show ghosts.
It opens the merge with those parts tinted red, through crop and straighten.
Turn it off or on again in the Clipping card or with a right-click on the photo.

JPEG, HEIC and TIFF brackets merge too, by exposure fusion: the bracket's best-exposed parts in one picture.
- Format: 16-bit TIFF or lossless JPEG XL, named like IMG_1234-HDR.tif.
- Also save radiance (EXR): the scene's light as a float EXR beside it.

## Merge to panorama {#merge-pano}
Stitch overlapping photos into one wide picture.

1. In the gallery, select the photos, or right-click one of a panorama.
2. Choose Merge to panorama. One photo finds its panorama by time, camera and exposure.
3. Leave out photos if you like, then choose Stitch.

- Projection: Automatic picks rectilinear up to about 110° across, cylindrical beyond.
- Rectilinear keeps straight lines straight. Cylindrical suits wide rows.
- Crop to fill: off keeps the empty corners.
- Format, for JPEG, HEIC and TIFF photos: 16-bit TIFF or lossless JPEG XL.
- Open in Develop: the panorama opens when done.

Raws become one raw, saved as a DNG named like IMG_1234-Pano.dng. Nothing is replaced.

Lens distortion and vignetting are corrected first when the raws have lens data.
Brightness differences between photos are evened out. Seams go around things that moved.

Photos that overlap no others are left out, and the toast names them. Up to 60 photos.

## Focus stack {#focus-stack}
Merge raws focused at different distances into one raw sharp throughout.

1. In the gallery, select the raws, or right-click one of a focus bracket.
2. Choose Focus stack. One raw finds its stack by time, camera, exposure and focus.
3. Leave out photos if you like, then choose Stack.

- Method: Depth map picks the sharpest photo per area. Pyramid mixes fine detail; try it when edges show halos.
- Align photos: lines up handheld shots and focus breathing.
- Save depth map: a 16-bit grey PNG of which photo was sharpest, near dark, far light. Depth map method only.

Saved as a DNG named like IMG_1234-Stack.dng next to the photos. Up to 200 photos.

## Match to reference {#match}
Make a set of photos match one photo's exposure and white balance.

1. Open the reference photo and press [[key:develop-reference]].
2. Select the photos in the gallery, or pick them in cull mode.
3. Press [[key:develop-match]].

Each photo gets an exposure and a white balance, as if set by hand. Only the shared mid-tones are compared, not highlights or clipped areas.

## Next capture style {#next-capture}
In cull mode, new files arriving in the folder can get a style at once.

- Choose Next capture style… in the command palette.
- None, the previous photo's edit, or any style.

## Speed edit {#speed-edit}
Hold a key and scroll, or press arrows, anywhere over the image. A readout shows the value.

- [[key:develop-speed-exposure]]: exposure
- [[key:develop-speed-contrast]]: contrast
- [[key:develop-speed-highlights]], [[key:develop-speed-shadows]]: highlights, shadows
- [[key:develop-speed-whites]], [[key:develop-speed-blacks]]: whites, blacks
- [[key:develop-speed-temperature]]: white balance temperature
- [[key:develop-speed-vibrance]]: vibrance

Hold Shift for fine steps. Change the keys in Settings › Keys.

## Where edits are kept {#saving}
Edits save by themselves a moment after each change, and when you leave the photo.

- Photos in the library keep their edits in the library.
- Every photo also gets a small file beside it, named like IMG_1234.CR2.specular.xmp. It travels with the photo.
- Versions get their own file: IMG_1234.CR2.v2.specular.xmp.
- To stop writing these files, turn off Settings › Metadata and privacy › Save Develop edits next to the photo.
- Rename, move, copy and undo take these files along. Moving the photo to the trash takes them too; the question names them.
- Other programs' content in these files is kept as it is.
- A file from a newer Specular that this version can't fully read is never changed. Develop shows what it can, and changes are not saved.

If the file beside the photo changed outside Specular, Develop asks which edit to keep. It names when each was saved. Nothing is replaced until you choose.

## Other tools {#other-tools}
The Other tools card shows when a photo has another program's edit.

- **Lightroom and Camera Raw:** Import Lightroom edit brings in exposure, tone, white balance (raws), colour, HSL, colour grading, curves, detail, crop and lens settings. The result is close, not the same.
- Not carried over: Lightroom's Transform (perspective and Upright). Its exact behaviour is not published. Use the Perspective card.
- The import is one step in History, so [[key:undo]] takes it back. It never changes the Lightroom file.
- To import by itself when a photo has no Develop edit, turn on Settings › Metadata and privacy › Import Lightroom settings in Develop.
- **darktable:** the card and the Info panel list the darktable modules used. Specular does not convert them.
- Open in darktable starts darktable with the photo, when darktable is installed.

Developing needs a graphics card. Without a usable one, Develop runs on the processor, more slowly.

## Scopes {#scopes}
The scopes sit at the top of the panel. They show the picture as it is on screen.

- Histogram: how many pixels at each brightness, in red, green and blue.
- Waveform: brightness from bottom (0) to top (100), across the photo from left to right. The chip in its corner switches between luma and RGB.
- Parade: the red, green and blue waveforms side by side.
- Vectorscope: hue around the circle, saturation outwards. The line marks skin tones. The squares mark the screen's red, yellow, green, cyan, blue and magenta.

Click a tab to switch. [[key:develop-scopes]] steps through them, then hides them.

While soft-proofing, dots and a dashed line on the vectorscope mark the printer's range.

## Clipping {#clipping}
Clipping colours mark where detail is lost. They are only on screen, never in exports.

- Display clipping ([[key:develop-clip-display]]): red where the picture is pure white, blue where it is pure black.
- Scene clipping ([[key:develop-clip-scene]]): orange where the scene is brighter than the look's white, cyan where it is darker than its black.
- Gamut warning ([[key:develop-clip-gamut]]): green where colours do not fit the screen. While soft-proofing, where they do not fit the printer.

The Clipping card and a right-click on the photo turn them on and off too.

## Soft proof {#proof}
Soft proof shows the photo as a printer will print it. [[key:develop-soft-proof]] turns it on and off.

- Profile: the printer's colour profile. Profiles in the system's colour folders are listed. Choose a profile file… adds another.
- Intent: Perceptual fits all colours into the printer's range. Relative colorimetric keeps the colours that fit and clips the rest.
- Black point compensation: maps black to the printer's black, so shadows keep detail.
- Simulate paper white: shows the paper's own colour. It also shows the ink's black.
- Simulate ink black: shows how dark the ink's black really is.

The proof is a way of looking. It is not part of the edit.

The profile and these choices are remembered for next time, for every photo.

If the profile file is gone, soft proof stays off until you choose one.

## Print {#print}
Print ([[key:print]]) in Develop prints the developed photo at full size.

With a soft-proof profile chosen, the print dialog shows it under Colour profile, with the intent. The PDF then holds the photo in the printer's colours. Turn the printer's own colour management off when printing it.

Choose sRGB under Colour profile to print without the profile.

## Export {#export}
[[key:develop-export]] or the Export button at the top of the panel writes the edited photo as a new file. The photo and its edit stay as they are.

- Images: this photo, the folder or the picks. Other photos use their saved edits.
- Preset: a shipped one or your own. Save preset keeps the choices as a preset.
- Format: JPEG, PNG, AVIF, JPEG XL, TIFF (8, 16 or 32-bit float) or OpenEXR.
- Colour space: sRGB, Display P3, Rec. 2020 or Adobe RGB (compatible). Its profile goes into the file.
- HDR: PQ or HLG for AVIF, JPEG XL and PNG. These are HDR files in Rec. 2020.
- HDR: Gain map for JPEG, AVIF and JPEG XL. SDR screens show the SDR picture, HDR screens the HDR one.
- The HDR limit sets how bright an HDR file goes. HLG stops at 1000 nits.
- Exif and XMP are copied. The photo is turned upright.

Export runs in the background. Its progress shows in the downloads indicator, with Stop.

From a terminal: `specular develop export FILE… --preset web`. `specular --help` lists the options.

## Profiles {#profiles}
Develop is hidden while the Medical or Science profile is on. Journal and lab rules forbid local changes to such images.

## Keys {#keys}
::: keys develop
