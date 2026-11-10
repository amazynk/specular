---
id: export-recipes
title: How do I export with recipes?
section: tasks
contexts: export-recipes
keywords: export, recipe, preset, web, print, email, watermark, sharpen, resize, metadata, copyright, gps, rename, batch
---
A recipe is a saved way to export. Run several at once, for example web and full-size TIFF.
The originals are never changed.

## Exporting {#steps}
1. Select images in the [gallery](gallery) or the Library, or open one.
2. Choose **Export with recipes…** in the palette. In Develop, choose **Export with recipes…** there.
3. Tick the recipes to run.
4. Choose **Export**.

Photos with a Develop edit are exported with it.

The export runs in the background and can be stopped. At the end, anything that failed is listed with the reason.

## Recipes {#recipes}
Three come with Specular: web, print and email. Edit or delete them as you like.

**New**, **Edit…** and **Duplicate** open the recipe. **Delete…** asks first; exported files stay on disk.

## What a recipe holds {#choices}
- Format, quality and bit depth.
- Colour space: sRGB, Display P3, Adobe RGB or ProPhoto, with or without its profile.
- Size: long edge, short edge, megapixels, exact width × height (cropped to fit), or percent. Small images are not enlarged unless you allow it.
- Output sharpening for screen, matte or glossy paper.
- Metadata: all, copyright only, copyright and contact, or none. Location and the people shown can be removed.
- A text or image watermark: position, opacity and size.
- File name, folder, and what to do when the name is taken.
- Open the folder when done, or nothing.

## File names {#names}
`{name}` original name, `{date}` or `{date:%Y%m%d}`, `{seq}` or `{seq:3}` number, `{camera}`, `{rating}`, `{text}` the recipe's custom text.

## Taken names {#taken}
- **Add a number**: `photo-2.jpg`.
- **Replace**: asks first, listing the files. Originals are never replaced.
- **Skip**: the existing file stays.

> Nothing is uploaded. Export recipes never use the network.
