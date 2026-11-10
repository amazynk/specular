---
id: gallery
title: The gallery
section: modes
contexts: gallery
keywords: rename keyword, move keyword, merge keywords, thumbnails, grid, folder, select, sort, filter, exif, flash, white balance, quick look, layout, duplicates, similar, marks, rating, keywords, tags, synonyms, lightroom, edited, develop, sharper raw thumbnails, raw thumbnail size, versions, module, style, badges, metadata, iptc, xmp, caption, title, creator, copyright, credit, location, template
---
The gallery shows a folder or archive as thumbnails. No importing is needed.

Press [[key:gallery]] to switch between the viewer and the gallery. The current image stays in focus.

## Browsing {#browse}
- Arrow keys, Page Up, Page Down, Home, End: move through the grid
- Type the start of a file name: jump to it
- Enter or double-click: open in the viewer
- Backspace or Alt+Up: parent folder
- Ctrl+wheel, or + and −: thumbnail size

Folders and archives show as tiles.

## Selecting {#select}
- Click: select one
- Shift-click: select a range
- Ctrl-click: add or remove one
- Ctrl+A: select all

Commands such as Convert, Print and Move to trash act on the selection.

## Quick look {#quick-look}
Hold Q to see the focused image large over the grid. Release to return.

## Marks: picks, ratings and labels {#marks}
Pick or reject, rate one to five stars, or set a colour label. The cheat sheet's gallery tab lists the keys.

Marks are saved in Specular's database and, by default, in XMP sidecar files other photo apps read. Image files are never changed.

A sidecar another app wrote, such as darktable's, gets your marks too. Only the marks change in it.

Where you haven't set a mark, the other app's shows. See [Other apps' sidecars](library#other-apps).

## Keywords {#keywords}
Press [[key:keywords]] to open the keyword panel. It also works in the Library and in cull mode.

Type a keyword and press Enter to add it to the selected photos. Tab takes the first suggestion.

Use `>` for levels: `Places > France > Paris`.

A box shows whether all or some of the photos have a keyword. Click it to add it to all, or take it off.

The keyword list holds every keyword you used. Select one to make it private, give it synonyms or remove it.

Rename… and Move to… change a keyword on every photo that has it, in the Library too.
Moving onto a keyword that exists merges the two.
[[key:undo]] puts the photos back.

Private keywords are not written to `dc:subject`, which other apps show and exports carry.

"Import list…" and "Export list…" read and write Lightroom keyword lists.

Keywords are written to the XMP sidecar, as `dc:subject` and `lr:hierarchicalSubject`. Image files are never changed.

[[key:undo]] takes back the last keyword change.

Type `tag:` in the filter to show photos with a keyword.

## Metadata {#metadata}
Press [[key:edit-metadata]] to edit the IPTC fields of the selected photos. "Edit metadata…" in the Info panel and the right-click menu open it too.

Fields: title, headline, caption, keywords, persons shown, event, creator and contact details, copyright notice, credit line, source, usage terms, locations shown and created, instructions and job ID.

Lists take several values separated by `;`.

A field is saved when you leave it: Enter, Tab, the arrow keys or a click elsewhere. Esc puts it back.

With several photos selected, only the field you change is written. A field they differ on shows "Mixed: type to set on all". Backspace in it clears it on all.

Keywords show those all photos have. Typing adds or takes off only those.

Templates fill in fields you use often, such as copyright and creator. Click one to apply it. Its keywords are added, or replace the photos' own when it says so.

Fields go to the XMP sidecar. With "Write metadata into image files" on (Settings, Gallery), JPEG, TIFF and DNG files get them instead, with nothing else in them changed. Raw files always get a sidecar.

When a file's XMP and IPTC-IIM disagree, XMP wins.

Ratings and labels keep their own keys.

## Sorting, filtering and layout {#filter}
Use the bottom bar or the command palette to sort by name, date, size or type. Layouts are square tiles, justified rows or uncropped thumbnails.

The filter field finds images by name or property, such as `rating>=3` or `camera:X-T5`.

Exif fields:

- `flash:` yes, no, auto, redeye, forced.
- `wb:` auto, manual, daylight, cloudy, shade, tungsten, fluorescent, flash.
- `ev:` exposure compensation: `ev<0`, `ev:-1/3`.
- `program:` or `mode:` manual, auto, program, aperture, shutter, portrait, landscape, bracket.
- `metering:` average, centre, spot, multispot, matrix, partial.
- `focal35:` the 35 mm equivalent focal length.
- `make:` the camera maker. `camera:` is maker and model.
- `serial:`, `lensserial:`, `software:`, `artist:`, `copyright:`, `description:`.
- IPTC fields win over Exif for `artist:` (or `creator:`), `copyright:` and `caption:`.
- In the Library: `title:`, `headline:`, `credit:`, `source:`, `usage:`, `instructions:`, `job:`, `event:`, `contact:`. `place:` and `city:` also find IPTC locations, `person:` persons shown.
- `altitude:` in metres, below sea level negative: `altitude>1000`.
- In the Library, `exif:` finds any tag: `exif:SceneCaptureType=night`, `exif:DigitalZoomRatio>1`, `exif:BodySerialNumber`.

Quick filters show only picks, hide rejects, or show only RAW, animated, HDR, video or sound files.

## Edited photos {#edited}
A photo with a Develop edit shows the edit in its thumbnail.

- The thumbnail is made in the background. The old one shows until then.
- It is made again after an edit, a paste, a style or a sync.
- A raw photo's thumbnail is developed at 1024 px, then reduced. This makes it sharper.
- For faster thumbnails, turn off Settings › Gallery › Sharper raw thumbnails (512 px).
- An **Edited** badge marks it. Turn the badge off in the View menu.
- Versions stack under their photo. Press S on the stack to open or close it.
- Opening a version opens the photo in Develop at that version.

Find edited photos in the filter field:

- `edited:yes` or `edited:no`
- `module:exposure` (a module's name, in your language or English)
- `style:Punchy` (a style applied)
- `version:yes` (has versions), `version:copy` (is a version)

## Duplicates and similar shots {#duplicates}
The gallery can group exact duplicates and similar shots, and suggest which to keep. See [Finding duplicates](duplicates).

## Keys {#keys}
::: keys gallery
