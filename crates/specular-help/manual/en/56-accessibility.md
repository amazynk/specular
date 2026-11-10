---
id: accessibility
title: Accessibility
section: reference
contexts: accessibility
keywords: accessibility, closed captions, audio description, screen reader, crosshair, pixel value, announce, orca, narrator, voiceover, keyboard, contrast, text size, motion, scale, status, progress, toasts, notices
---
## Keyboard {#keyboard}
Everything can be done from the keyboard.

- **Tab** moves between controls.
- **Enter** or **Space** activates.
- **Esc** closes.
- The [command palette](interface#palette) reaches every command by name.

## Screen readers {#screen-readers}
Controls, dialogs and images have names that screen readers announce. Moving to another image announces its name and position.

Settings › Accessibility › Screen reader detail sets how much is said.

[[key:announce-cursor]] reads the value under the pointer or crosshair: the pixel value, HU, temperature, physical value or map and sky position, as the readout shows it.

Settings › Accessibility › **Read the value under the cursor** reads it out each time the cursor stops.

Screen reader detail also sets how much of the readout is read: **Brief** reads its first line, **Normal** the lines about the point under the cursor, **Detailed** every line.

The [information panel](inspect#info) describes the image in words.

Progress over a picture or page is read out as it changes: "Developing…", "Refining…", "Reading the sound…", "Measuring loudness…". So are notices such as a graphics driver reset.

## Keyboard crosshair {#crosshair}
Inspect an image without a mouse.

- [[key:crosshair]] shows or hides the crosshair.
- **Arrow keys** move it one image pixel when zoomed in, one screen pixel when zoomed out.
- **Shift** with an arrow moves it ten times as far.
- The pixel inspector, readouts and measuring tools follow it.
- **Enter** or **Space** clicks where it is, for example to place a measuring point.
- **Esc** hides it.
- In Develop's Retouch, [[key:develop-spot-crosshair]] shows a crosshair that places spots: see [Retouch](develop#retouch).

## Seeing more easily {#seeing}
Settings › Accessibility has:

- **Text size** and **Interface scale**, on top of your system's display scaling.
  [[key:interface-larger]] and [[key:interface-smaller]] change the interface scale in steps of 10 %; [[key:interface-reset]] sets it back to 100 %. They leave the image's zoom alone.
- **Increase contrast**: stronger borders and text.
- **Always show the focus ring**.
- **Reduce motion**: instant changes instead of animations. Toasts appear and go at once; sound visuals and lyrics jump instead of gliding.

Light and dark themes follow your system. Press [[key:theme]] to choose one.

## Captions and audio description {#media}
Videos show closed captions and SDH subtitles: see [Closed captions](video#captions).

Settings › Viewing › **Prefer subtitles for the deaf and hard of hearing** picks them first.

Settings › Viewing › **Prefer audio description tracks** plays a track that describes the picture first.
