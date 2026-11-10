---
id: settings
title: Settings
section: reference
contexts: settings
keywords: settings, preferences, options, configuration, defaults, reset, preview while dragging
---
Press [[key:settings]] to open Settings. Changes apply straight away.

Each setting explains itself underneath. The search field finds a setting by name.

Text and folder settings are edited in place. Enter or leaving the field keeps the change; Esc undoes it.

**Choose…** picks a file or folder; **Clear** goes back to the default.

## Sections {#sections}
- **General**: language, theme, what happens at start, your name.
- **Profile**: what you use Specular for. See [Profiles](profiles).
- **Viewing**: zoom, background, toolbar, animations, videos, subtitles and sound.
- **Browsing** and **Gallery**: image order, reading order, thumbnails and their cache.
- **Library**: folders the Library skips. See [Skipped folders](library#skipped).
- **Slideshow**, **Cull mode**, **Convert** and **Import**: defaults for those features.
- **Medical images**: window/level, orientation labels, the mouse wheel, the grey display.
- **Maps**: coordinate format, units and online map services. See [Maps](maps).
- **Colour**: colour management, HDR and soft proof. See [Colour](colour).
- **Inspection**: clipping, focus peaking, histogram and false colour. See [Inspecting images](inspect).
- **Metadata and privacy**: the information panel, GPS positions, place names, the base map and Develop's files.
- **Local AI**: optional features that run only on this computer. See [Local AI](ai).
- **Performance**: memory, the GPU, caches and Develop's preview while dragging. See [Slow images](troubleshooting#slow).
- **Accessibility**: motion, contrast, text size, screen readers. See [Accessibility](accessibility).
- **Keys** and **Devices**: shortcuts, gamepads and SpaceMouse. See [Gamepads and SpaceMouse](devices).
- **Integration**: the default viewer, file manager, desktop search, SAMP and the plate solvers. See [Desktop search](library#desktop-search) and [Astronomy tools](science#astronomy).
- **Optional decoders**: software from a format's maker. See [Optional decoders](optional-decoders).
- **Drawings**: SHX font folders. See [SHX fonts](drawing#shx).
- **Privacy**: No network and yt-dlp. See [Privacy](privacy#no-network).
- **Updates**: checking for new versions, off by default. See [Updates](privacy#updates).
- **Advanced**: portable mode, logs, the audit log, settings.toml, import, export and reset.

## Profiles {#profiles}
A [profile](profiles) changes several settings at once for a kind of work.

## Audit log {#audit}
Settings › Advanced › Local audit log. Off by default.
- When on, each opened medical image, export, anonymised copy, print and send to a PACS is one line in `audit.jsonl` in the data folder.
- Each line is JSON: the time, what happened and the files.
- It stays on this computer.
