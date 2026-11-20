# Changelog

All notable changes to Specular are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- A purple Discord button opens the Specular community from the start screen and the
  default viewer toolbar.
- **Check for updates** in About and the command palette checks once and shows the result
  in Settings › Updates.

### Fixed

- Update checks now read published GitHub releases. The previous feed returned a 404.

## [0.1.2] - 2026-11-16

### Added

- **Streams from the command line.** `specular http://…` (or `--stream URL`) plays an address
  as File › Open network stream does, and `--title TEXT` names it in the window's title. A
  window already open plays it instead of a new one opening (the single-instance setting),
  on every platform.
- **Streams over remote control.** `open-url` takes an `http://` or `https://` address and a
  `title` (`specular ipc url --title TEXT ADDRESS`), and a `specular://stream?url=…&title=…`
  link plays a stream served on this computer.
- **Streams from this computer.** An address at `127.0.0.1`, `[::1]` or `localhost` plays
  with No network on, since nothing leaves the computer. Specular waits for it however long
  a part takes, showing “Buffering…” meanwhile, and keeps it out of the recent streams. A
  part far ahead that is held back (a seek, a cover's frame) is waited for on a connection
  of its own, so playing along goes on meanwhile. This lets a downloader on the same
  computer play a file it is still fetching.
- **HLS recordings in progress.** A playlist that only grows (`EXT-X-PLAYLIST-TYPE:EVENT`)
  plays with a seek bar over everything recorded so far, which grows with it. It starts
  where `EXT-X-START` says, else near its end.
- `specular --version` prints the version without opening a window.
- The Python client has `open_url(url, title=None)`.

### Changed

- A network stream's window title is its title or its address, not the name of the part
  playing. “Buffering…” stays on screen for as long as a stream waits.

### Fixed

- Seeking back in an HLS or DASH stream after its last part was fetched no longer waits for
  ever.
- A web file whose server sends it a little at a time (slower than it plays) is fetched in
  one request as it comes. It used to be given up and asked for again at every read, many
  times a second.

## [0.1.1] - 2026-11-13

### Changed

- **No network is on by default.** Specular connects to nothing until Settings › Privacy › No
  network is turned off, also after updating from 0.1.0 unless the switch was changed there.

### Fixed

- **macOS:** Command is the shortcuts' Ctrl, as the app shows them: ⌘F searches, and ⌘A, ⌘C,
  ⌘V and ⌘Z work in text fields. The Control key takes the place of Super.

## [0.1.0] - 2026-11-12

The first public release, 0.1.0, under MIT OR Apache-2.0. Everything is new; the main parts are
listed here.

### Added

- **Viewer.** GPU drawing on Metal, Direct3D 12 and Vulkan, with the overview on screen before the
  file has finished decoding, neighbours decoded ahead, colour management through ICC profiles and
  OpenColorIO, and HDR output where the display supports it.
- **Formats.** More than 400, most of them decoded by Specular's own Rust code: photos and camera
  RAW, HDR and gain maps, layered and painting files, video and sound, documents, and medical,
  scientific, engineering and map formats. C and C++ libraries run in a sandboxed helper process.
- **Culling.** Pick, reject, rate and label from the keyboard, burst grouping with the best frame
  suggested, duplicate and similar-shot finding, tethered capture and follow-a-folder.
- **Develop.** Non-destructive RAW development on the GPU: tone, colour grading, masks (including
  AI subject, sky and object masks), retouch, denoise, lens and perspective correction, camera
  profiles from colour charts, and merges to HDR, panorama and focus stack. Edits are kept in XMP
  sidecars.
- **Library.** A catalogue of folders, network shares and removable disks, with albums, smart
  albums, a timeline, keyword and IPTC editing, local copies of offline originals, and the
  sidecars of darktable, Lightroom, Camera Raw, Capture One, digiKam and RawTherapee.
- **Compare, export and share.** Split, overlay, flicker and difference views with SSIM and
  ΔE2000; export recipes; printing; static client galleries whose picks come back as marks.
- **Profiles** for medical (DICOM, volumes, MPR and 3D, fusion, measurements, anonymised export),
  science (FITS and astronomy, microscopy and whole-slide images, HDF5 and NetCDF, thermal),
  engineering (DXF, PDF drawing sets, IFC, STEP and other models, point clouds) and maps.
- **Local AI**, off until asked for and run on the computer: search by description, text in
  images, culling assistance, people, and AI tools in Develop.
- **Scripting.** A command line for headless export and comparison, remote control of a running
  window, an IPC protocol with a Python client, events and approved hooks.
- **Everywhere.** 19 languages with right-to-left layouts, screen-reader support, a keymap with
  presets in the style of Vim, Lightroom and Photo Mechanic, an offline manual, and no telemetry.
