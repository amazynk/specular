# Changelog

All notable changes to Specular are listed here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

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
