---
id: library
title: The library
section: modes
contexts: library
keywords: library, desktop search, gnome shell, krunner, locations, index, search, catalogue, offline, local copy, proxy, removable, folders, skip, watches, inotify, edited, module, style, version, albums, smart albums, collections, timeline, sidecars, darktable, lightroom, rawtherapee, capture one, digikam, duplicates
---
The library shows every image in your library locations in one searchable grid. A location is a folder, including its subfolders.

Press [[key:library]] to open it.

## Adding locations {#locations}
1. Press [[key:library]].
2. Choose **Locations**, then **Add location…**.
3. Choose a folder.

Images are indexed in the background. Indexing pauses while you browse.

Add your Pictures folder rather than your whole home folder. The home folder holds program files and caches too; Specular asks before adding it.

If you stop indexing, what was indexed stays searchable. **Resume** continues from there.

## Skipped folders {#skipped}
The Library doesn't look in hidden folders, or in folders of programming tools: `target`, `node_modules`, `build`, code checkouts (with `.git`), Python environments and caches.

To change the list, open Settings › Library. The Library looks through every location again.

## Searching {#search}
Type in the search field to find images by name, camera, date, rating and more, as in the [gallery's filter](gallery#filter). Results update as you type.

Develop edits are searchable too: `edited:yes`, `module:`, `style:` and `version:`. See [Edited photos](gallery#edited).

`exif:` finds photos by any Exif tag, by its standard name:

- `exif:Contrast` has the tag.
- `exif:SceneCaptureType=night` or `=3`: words and numbers both work.
- `exif:ExposureTime<1/1000` compares numbers.
- `exif:Software="Photo Lab"` finds text in the value.
- Photos indexed before this are read again in the background.

**Albums** in the sidebar show their photos in one click.

- A manual album holds the photos you put in it. Right-click photos and choose **Add to album…**.
- B puts the selected photos in the last album used. Shift+B asks which album.
- A smart album keeps a search. Its rules join with And, Or and Except, top to bottom.
- **Save search as smart album…** keeps the search typed.
- Use / in a name to make groups: `Client X / selects`.
- `album:` finds photos by their manual albums.
- Deleting an album never deletes photos.

The timeline over the grid counts photos by year, month or day. Click a period to show only it.

Shift+click picks a run of periods. Click the picked period again to show all dates.

From the keyboard: Tab to the timeline, arrows between periods, Enter picks one, Shift+arrows pick a run.

To hide the timeline, turn off Settings › Library › Date timeline.

## Desktop search {#desktop-search}
On Linux, the GNOME and KDE search can find pictures in your Library.

- Turn it on in Settings › Integration › Desktop search. It is off until you do.
- Type as in the Library's search field: `beach tag:family -is:raw`.
- Only pictures are found, newest first; text files are left out.
- Thumbnails come from the gallery's cache.
- Pick a result to open it in Specular.
- In GNOME, clicking the Specular icon opens the Library with the search.
- In KRunner, **Open folder** shows the picture's folder.
- Nothing is indexed for it and nothing leaves this computer.
- GNOME reads search providers only from system folders: a per-user install shows in KRunner, a distribution package in both.
- Turning it off asks first. The Library and your pictures stay.

## Other apps' sidecars {#other-apps}
The Library reads the sidecars other photo apps keep beside your photos:

- darktable: `IMG_0001.CR2.xmp`
- Lightroom, Camera Raw, Capture One, digiKam: `IMG_0001.xmp`
- RawTherapee: `IMG_0001.CR2.pp3`
- Capture One: ratings and labels from its `CaptureOne` folder

Their ratings, colour labels, picks and rejects, keywords, titles and captions come in. darktable's own tags are left out.

Your own marks and keywords win. Other apps' values only fill in what you haven't set.

To let the other app win in a folder, add it to Settings › Library › Prefer other apps' sidecars in.

A changed sidecar is read again at the next look.

darktable duplicates (`IMG_0001_01.CR2.xmp`) count as versions of the photo.

In another app's sidecar, Specular changes only ratings, labels and picks.

Keywords and map positions go into Specular's own sidecar beside it.

The Info panel shows what came from another app's sidecar.

## Offline locations {#offline}
A location on a disconnected disk is shown as offline. Its images stay in the library and return when the disk is back.

## Local copies {#local-copies}
Keep a local copy to work on photos from a network share, USB disk or SD card while it is unplugged.

1. Show the location or album, or select photos.
2. Open the palette or the photo menu and choose **Keep a local copy…**.
3. Choose **Keep originals**, or **Keep proxies**: raws at 2,560 pixels, enough to develop.

Copies go to Specular's data folder, never beside the originals. Settings › Library shows the space used and sets the limit.

While the location is offline, the grid, the viewer, marks, keywords and Develop use the copies. A banner says the location is offline and how many changes wait.

When it is back, the waiting changes are written to the originals' sidecars. If a file or its sidecar changed meanwhile, Specular shows both versions and you pick one. Nothing is overwritten unasked.

Exporting from a proxy asks first: export at proxy size, or wait for the originals.

**Remove local copy…** asks first. The originals are not touched.

## Folder watches {#watches}
The Library watches its folders, so new and changed photos show up at once.

Linux limits how many folders each user can watch. When the Library has more, "Some folders aren't watched" shows under the locations. Changes in those folders show up at the next look, every 15 minutes.

To raise the limit, run these two commands in a terminal, then restart Specular:

`echo fs.inotify.max_user_watches=524288 | sudo tee /etc/sysctl.d/90-inotify.conf`

`sudo sysctl --system`

Specular uses half the limit, at most 65,536 folders. Beyond that, the look every 15 minutes keeps the Library current.

## The library's data {#data}
The index is stored only on your computer.

Removing a location removes its entries from the library. Specular asks first; the images are never touched.
