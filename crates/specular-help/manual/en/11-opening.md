---
id: opening
title: Opening files
section: start
contexts: open
keywords: open, file, folder, drag, drop, paste, clipboard, formats, archive, recent, zarr, ome-zarr, s3, url, address, remote, stac, network stream, stream, hls, m3u8, dash, mpd, internet radio, shoutcast, icecast, yt-dlp, quality, seek, range, sixel, x cursor, mpo, stereo, quicktime image, qtif, macpaint, gem, degas, neochrome, atari, koala, c64, zx spectrum, scr, dr. halo, softimage, alias pix, rla, rpf, utah rle, quake wal, blp, usgs dem, off
---
Open a single file, a folder or an archive. After opening one image, you can step through the other images in its folder.

## Opening a file or folder {#open}
1. Press [[key:open]], or choose **Open** in the toolbar or the command palette.
2. Choose a file or a folder.

You can also drag files onto the window, or open them with Specular from your file manager.

Until an image is open, most commands are unavailable.

## Pasting {#paste}
Press [[key:paste]] to show the image on the clipboard. If the clipboard holds copied files or file paths, Specular opens them. If it holds a Zarr address, Specular opens that store.

## Formats {#formats}
Specular reads photos (JPEG, PNG, WebP, AVIF, HEIC, JPEG XL, TIFF, camera RAW), animations, short videos, [sound files](video#sound-files), paged documents, comic archives, DICOM, FITS and other scientific formats, maps, CAD drawings and 3D models.

Retro and game pictures open too: Sixel, X cursors, MacPaint, GEM, Atari ST (DEGAS, NEOchrome), Commodore 64 Koala, ZX Spectrum screens, Dr. Halo, Softimage PIC, Alias PIX, RLA, Utah RLE, Quake 2 WAL and Blizzard BLP.

An MPO stereo photo opens as two pages: the left view, then the right.

A Dr. Halo picture takes its colours from the .pal file beside it.

A Quake 2 texture takes them from the game's colormap.pcx, if found near it.

Without these, the picture shows in grey.

It also shows [text, tables and subtitle files](text): plain text and logs, Markdown, CSV, RTF, SRT and other subtitles.

A few formats, such as Blackmagic RAW and DWG, need a free decoder from their makers: see [Optional decoders](optional-decoders).

If a file does not open, see [When a file will not open](troubleshooting#wont-open).

## Archives {#archives}
ZIP, RAR, 7z, TAR and ISO archives open like folders, with their images in order. So do comic archives (CBZ, CBR, CB7, CBT).

For pages, [reading mode](reading) is offered.

An encrypted archive asks for its password. It is kept in memory only.

## Zarr stores on servers {#remote-zarr}
Open an OME-Zarr image or a Zarr store on a server or in an S3 bucket by its address: paste it, or give it on the command line.

- Addresses look like `https://…/name.zarr` or `s3://bucket/name.zarr`.
- Only what the view needs is downloaded: a coarse picture first, then detail as you zoom.
- A plate or a set of series opens as a folder: the arrow keys step through its images.
- Public buckets need nothing. For a private bucket, set up AWS credentials first (environment variables or `~/.aws/credentials`). Specular never saves or shows them.

## Open address {#open-address}
Press [[key:open-address]] to open something by its web address: a Zarr store, a Cloud-Optimized GeoTIFF (`.tif`), a COPC point cloud (`.copc.laz`), a STAC item, collection or catalog (`.json`), a 3D Tiles `tileset.json` or a DICOMweb server.

- Type or paste the address. It must start with `http://`, `https://` or `s3://`.
- Recent addresses are listed under the field. Only the server and path are kept: never a user name, password or anything after `?`.
- A DICOMweb address opens the DICOMweb sheet on that server, or ready to add it.
- With the network off in Settings › Privacy, nothing opens and the dialog says so.

## Network streams {#network-streams}
Press [[key:open-stream]] to play something from the network: a sound or video file on a web server, an HLS (`.m3u8`) or DASH (`.mpd`) stream, or internet radio (a stream address, `.pls` or `.m3u`).

- Pasting such an address, or opening it with [[key:open-address]], plays it too.
- A file plays once it is fetched. The download indicator shows how far.
- An MP4 or MOV file plays at once when its server allows partial downloads. Seek anywhere; the part you need is fetched first.
- An MKV or WebM file plays in parts when its server allows partial downloads. With an index in the file, seek anywhere; without one, seek within the part already fetched.
- Radio, HLS and DASH play in parts of a few seconds, one after another. The next part is opened before the one playing ends, so picture and sound run on.
- The sound runs on from one part into the next as one stream, without a gap.
- The seek bar of an HLS or DASH stream (not live) covers the whole stream. Seek anywhere in it.
- To pick the quality, use More › Quality in the player, or Stream quality. Automatic follows the connection.
- Radio shows what is playing as the station sends it.
- Streams protected by DRM are refused. Specular never plays them.
- Recent streams are listed under the field, by server and title. Select one and press Delete to forget it (Specular asks first).
- In the play queue, web addresses play as streams.
- Going to another file stops the stream. Its parts are deleted from the cache.
- With the network off in Settings › Privacy, nothing is fetched.

Video pages (not a stream address) need yt-dlp, which you install yourself:

- Turn on Settings › Privacy › Use yt-dlp for web pages. It is off by default.
- Specular looks for yt-dlp on PATH and shows where it found it. You can set its path instead.
- yt-dlp runs only when you open a page. Specular never bundles or downloads it.

## Reloading {#reload}
Press [[key:reload]] to read a file again after another app changed it.

Folders update by themselves when files are added, changed or removed.
