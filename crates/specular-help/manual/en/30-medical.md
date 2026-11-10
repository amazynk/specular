---
id: medical
title: Medical images
section: modes
contexts: medical
keywords: dicom, ct, mr, mri, pet, x-ray, ultrasound, window, level, hounsfield, slices, series, study, measure, anonymise, nifti, analyze, oct, cirrus, parrec, ecat, veterinary
---
Opens DICOM files, folders and discs, NIfTI and Analyze volumes, Philips PAR/REC and ECAT 7 PET volumes, and Zeiss Cirrus OCT exports (`.img`: each B-scan is a slice).

> Specular is not a medical device. Do not use it for diagnosis.

## Window and level {#window}
Drag with the right mouse button, or press [[key:window-level]] and drag. This changes brightness (level) and contrast (window).

- [[key:window-preset:1]]: first preset, such as lung, bone or brain; the following keys choose the others
- On images that are not CT, the same keys pick the windows the file suggests
- [[key:invert-grey]]: swaps black and white

All presets are also in the palette.

## Slices {#slices}
- [[key:next-slice]]: next slice
- [[key:prev-slice]]: previous slice
- [[key:cine]]: plays the slices as a film

The mouse wheel also moves through slices. In a series with one image per file, these keys step through the files.

## Series and studies {#series}
- [[key:next-series]]: next series
- [[key:prev-series]]: previous series
- [[key:studies]]: lists the opened patients, studies and series, such as a disc's contents

## Measuring {#measure}
- [[key:measure-length]]: length
- [[key:measure-angle]]: angle
- [[key:measure-ellipse]]: ellipse
- [[key:measure-rectangle]]: rectangle
- [[key:clear-measurements]]: removes all measurements. Undo brings them back.
- [[key:measure-templates]]: vet, orthopaedic and dental templates, such as VHS, Norberg and Steiner. See [geometry](measure#geometry).

Results are in millimetres when the file gives the pixel size, otherwise in pixels.

Ultrasound images are measured in the region the points are in.

Drag a point or a region's corner to move it. Drop it on another point or corner to remove the measurement.

## Tags {#tags}
[[key:dicom-tags]] lists every DICOM tag, with a search field.

Unavailable for files that are not DICOM.

## Overlays and presentation states {#presentation-states}
- [[key:overlays]]: shows or hides the file's overlay planes, presentation state notes and orientation labels. Shutters stay.
- [[key:orientation-labels]]: patient directions
- [[key:presentation-state]]: applies a saved presentation state (window, annotations, shutters) when the study includes one

Veterinary scans marked QUADRUPED use Le/Rt, D/V and Cr/Cd. Limbs use Pr/Di; below the carpus and tarsus, D/Pa or D/Pl.

## Label volumes {#label-volumes}
[[key:label-volume]] lays a label file (NIfTI, MGH and others) over the series or volume.

- It is placed by both files' patient coordinates, oblique or not.
- Colours come from an ITK-SNAP or FreeSurfer colour table beside it, if there is one.
- [[key:segments]]: show or hide the labels
- [[key:segment-opacity]]: fill opacity, down to outlines only

Click a label in the list to hide it.

## PET and time curves {#analysis}
- PET images show SUV in the readout when the file holds what it needs. Otherwise the readout says what is missing.
- [[key:tmtv]]: tumour volume (TMTV) in a sphere or box you drag: mL, SUVmax, SUVmean and TLG
- [[key:tmtv-threshold]]: what counts: SUV 2.5, or 41 % of SUVmax
- [[key:time-curve]]: how the last region's values change over time
- [[key:export-curve]]: saves that curve as CSV

Nothing is detected for you. You draw every region.

## Anonymised copies {#anonymise}
[[key:export-anonymised]] saves a copy without names, dates of birth or other identifying details.

[[key:export-patient-disc]] saves the study as a folder that can be burnt to a patient disc, anonymised if you choose.

To anonymise a whole study, see [Sharing a study without personal details](howto-anonymise).

## Send to PACS {#send-to-pacs}
1. Add the server in **DICOMweb servers…** (palette): a DICOMweb address, or a DICOM (DIMSE) host, port and AE titles.
2. Choose **Verify** to check that it answers.
3. In the Studies browser, choose **Send to PACS…**. Or run **Send to PACS…** on a series.
4. Pick the series, the study or everything, and the server.
5. Leave **Anonymise** on to remove patient details from the copies sent.
6. Confirm. Each file is counted as it goes; **Stop** ends the send.

The summary names every file that was not sent, and why.
Your files are never changed.

Turn on **Secure connection (TLS)** for a DIMSE server that speaks DICOM over TLS (often port 2762).
Its certificate must be trusted by the system, as for a DICOMweb address.

**Browse** searches a DIMSE server too, by patient, date, modality or accession number.
Double-click a series, or press Enter, to open it.
The series comes in whole, then opens; the Downloads indicator counts the images.

**Retrieve with** sets how a DIMSE server sends a series:

- **C-GET** (the default) needs nothing on the server or the firewall.
- **C-MOVE** is for servers without C-GET: the server connects back to this computer.

For C-MOVE, enter this computer's address and a port to receive on.
On the server, add our AE title with that address and port.

Specular listens there only while a series comes in, and only for our AE title.

A series opened before opens from the cache when the server cannot be reached.

## Keys {#keys}
::: keys medical
