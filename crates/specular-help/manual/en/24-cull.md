---
id: cull
title: Cull mode
section: modes
contexts: cull
keywords: cull, pick, reject, rate, stars, label, burst, shoot, raw, jpeg, sidecar, xmp, copy to, move to, subframes, fits, xisf, blink, hfd, fwhm, eccentricity, astrophotography, develop, edit, edited
---
Cull mode is for going through a shoot: pick, reject, rate and label.

Press [[key:cull]] to enter or leave it, in the viewer or the gallery.

## Marking {#marking}
- [[key:cull-pick]]: pick
- [[key:cull-reject]]: reject
- [[key:cull-unflag]]: clear the flag
- Number keys: rate from no stars to five
- Colour label keys: see the cheat sheet's cull tab

After a pick, a reject or a rating, the next image is shown. To stay, turn off Settings › Cull mode › Next image after marking.

A RAW file and its matching JPEG are one item: marks apply to both.

## Bursts {#bursts}
Similar shots taken in quick succession form a burst.

- [[key:cull-burst]]: open or close the burst
- [[key:cull-keep-best]]: keep the suggested best frame, reject the rest

## Developing {#develop}
Press [[key:develop]] to open the current photo in [Develop](develop). Leave Develop to come back to the same photo in cull mode.

- An **Edited** badge shows when the photo has an edit.
- [[key:develop-sync]] syncs the edit to the picks; [[key:develop-sync-some]] syncs only the modules you choose.

## New files {#new-files}
Files that appear while you cull, such as from a tethered camera, are added at the end.

To shoot with the camera on a cable, see [Tethered capture](tether).

- [[key:cull-follow]]: jump to each new file as it arrives
- Turn both off in Settings › Cull mode.

## Copying and moving keepers {#copy}
1. Set up to three destination folders in Settings › Cull mode: Folder 1, 2 and 3.
2. Press the matching key to copy or move the current image there.

Undo a move with [[key:undo]].

## Sorting subframes {#subframes}
FITS and XISF frames get a table and a chart.

Each frame's stars are measured: star count, HFD, FWHM (″ when the header gives the plate scale), eccentricity, background, SNR.

- [[key:cull-subframes]]: show or hide the table
- [[key:cull-sort-next]]: sort by the next column; [[key:cull-sort-reverse]] reverses. A click on a column sorts by it.
- Arrow keys blink through the frames, all with the first frame's stretch. [[key:cull-same-stretch]] stretches each on its own.
- [[key:cull-metric-next]]: chart the next metric

Auto-reject suggests frames more than k σ from the median, on the bad side.

- [[key:cull-sigma-down]] and [[key:cull-sigma-up]]: change k
- [[key:cull-auto-reject]]: reject the suggested frames (asks first)
- [[key:cull-move-rejects]]: move rejected frames to the folder “rejected” (asks first; nothing is deleted)

Undo works for both.

## Where marks are saved {#saved}
Marks are saved in Specular's database and, by default, in XMP sidecar files that Lightroom, Capture One, darktable and Photo Mechanic read. Image files are never changed.

For a step-by-step guide, see [Culling a shoot](howto-cull).

## Keys {#keys}
::: keys cull
