# Changelog

All notable changes to Specular are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- **The Windows installer replaces a package of the same version.** Installing a rebuilt
  package over the same version used to leave two entries in Settings › Apps. The entry
  there now also links to where to get help and where new versions are.

### Fixed

- **A start that fails on Windows says why.** When Specular could not start from the Start
  menu or Explorer (no usable graphics driver, for one), its window never came and nothing
  said so. A message now gives the reason and where the log is.
- **Another graphics adapter when the first one fails.** When the adapter Specular picks
  at the start gives no device (a broken or half-removed driver), it now tries the
  computer's other adapters, the system's software one last, instead of not starting.
  This is for every system.
- **Show in folder on Windows** now gives Explorer the path the way Explorer asks for it,
  for files whose path has a space in it.
- On Windows the display cache kept no record of which pictures were shown lately, so when
  it was full it dropped the ones written first, not the ones unused longest.
- The licence page of the Windows installer still named the Apache License; Specular has
  been under the MIT License alone since 0.1.3.

## [0.1.5] - 2026-11-24

### Added

- **More from Amazynk.** Settings has a section of that name, always in sight at the foot of
  its sidebar, with a row and an icon each for Zynk (sends files straight between your
  devices), Pagebara (a PDF viewer and editor) and, on Linux, Dozed (freezes background apps
  there). **Get Zynk** opens Zynk in your system's store, or its download on Linux. **Get
  Pagebara** and **Get Dozed**, there, in About and in the command palette, open that app's
  page in your browser, where its downloads are. Specular downloads nothing itself.
- **Settings in groups.** The sidebar's sections stand under five headings (Application,
  Viewer, Images and files, Specialised, System) instead of one long list, with Accessibility
  and Keys up beside General. `settings.toml` is unchanged.
- **A menu bar on macOS.** The Specular menu has About Specular, Check for updates,
  Settings… (⌘,), Services, Hide and Quit. File, Edit, View, Go and Help list the commands
  under the names the palette gives them, in your language, and Window has Minimise (⌘M),
  Zoom and Bring All to Front. Until now the bar had only About, Hide and Quit. The
  commands are greyed while a dialog is open. Keys other than the Specular and Window menus'
  are not shown in the menus yet; they work as before.

## [0.1.4] - 2026-11-23

### Fixed

- Specular no longer keeps a whole processor core busy while its window is hidden, minimised
  or covered by other windows. A window that started out of sight drew nothing and ran the
  core flat out until it was brought forward.
- The log now says when the window has shown nothing a few seconds after it opened, and why,
  and keeps the reason when Specular cannot start at all (no usable GPU, no window). Until
  now a start that showed nothing left an empty log.
- Quitting no longer writes a "peer check failed" warning to the log on macOS.

## [0.1.3] - 2026-11-20

### Added

- A purple Discord button opens the Specular community from the start screen and the
  default viewer toolbar.
- **Check for updates** in About and the command palette checks once and shows the result
  in Settings › Updates.
- Each installer and archive now has a permanent download link that follows the latest
  stable release. Versioned downloads are still available.

### Fixed

- Update checks now read published GitHub releases. The previous feed returned a 404.

### Changed

- Audio and video controls appear when the pointer moves near the bottom of the player.
  Tab reveals them from the keyboard.
- **Licence.** Specular is now under the MIT License alone. Releases up to 0.1.2 were under
  MIT OR Apache-2.0 and stay so.

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
