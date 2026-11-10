---
id: medical-3d
title: 3-D and multi-planar views
section: modes
contexts: mpr
keywords: mpr, multi-planar, axial, sagittal, coronal, xy, xz, yz, volume, z-stack, industrial ct, microscopy, rendering, mip, slab, stl, export, 3d, fusion, pet, ct, curved, cpr, segment, 3mf, dental, cbct, panoramic, nerve, evar, aneurysm, diameter, centreline, marks
---
Works with any volume, in every profile: DICOM series, NIfTI, Analyze, NRRD and MetaImage volumes, industrial CT, microscopy z-stacks and 3D cubes.

Unavailable for single images and for series whose slices are not evenly spaced.

## Multi-planar views {#mpr}
- [[key:mpr]]: three planes together, linked by crosshairs
- Axial, coronal and sagittal when the file gives patient orientation; XY, XZ and YZ otherwise.
- Scroll in a plane to move through the volume. Drag with the right button to set the brightness and contrast.
- [[key:mpr-maximise]]: shows one plane large, or all again
- [[key:mpr-slab]]: thickens the slice into a slab
- [[key:mpr-preset]]: chooses how the slab is shown, such as maximum intensity
- [[key:mpr-export-planes]]: saves the three planes as PNG images
- Microscopy: shows the channel and time point you are on. A composite shows its first visible channel.
- No voxel size in the file: voxels are shown as cubes, and the panel says so.
- A volume too large for free RAM is shown at lower resolution.

Measurements, fusion and the CT presets need the Medical profile.

## Volume rendering {#render}
- [[key:mpr-render]]: shows the volume in 3-D; drag to turn it
- [[key:mpr-clip]]: cuts it open
- [[key:mpr-export-stl]]: saves a surface as an STL file for 3-D printing

## Fusion {#fusion}
- [[key:fusion]]: lays a PET series over a CT of the same patient
- [[key:fusion-colormap]]: colours of the overlay
- [[key:fusion-blend]]: how the layers mix
- [[key:fusion-nudge]]: moves one layer when they are out of line

## Curved views {#curved}
- [[key:mpr-curve]]: follows a curved structure, such as a vessel or a root canal
- Click points along it in the plane view. Scroll the plane between clicks to follow it in depth.
- Drag a point to move it. Undo point removes the last one.
- The reformat shows the whole curve. [[key:mpr-curve-layout]]: straightened or stretched.
- The cross-section is square to the curve at the cursor. Scroll over either view to move along the curve.
- Ctrl+scroll zooms. Shift+scroll turns the straightened view about the curve.
- The slab buttons thicken it, as in the planes.
- [[key:mpr-curve-save]] and [[key:mpr-curve-load]]: keep the curve in a JSON file.
- Tab reaches the centreline buttons. Arrows move between them, Enter chooses one.
- F2 renames the one with the focus. Delete asks before deleting it.

Lengths on the cross-section need the Medical profile.

### Lengths along a vessel {#evar}
- Move the cursor to a landmark and click Mark here. Click it again on a mark to remove it.
- The cross-section lists the length along the centreline between marks, and in total.
- Draw diameters on a mark's cross-section with [[key:measure-length]]. The smallest and largest show under it.
- Marks are saved with the curve.

## Segments as STL {#segments}
- [[key:mpr-export-segments]]: saves each segment as its own STL file, named by its label
- Works with DICOM segmentations, structure sets and label volumes such as NIfTI label maps.
- Coordinates are the patient's, in millimetres.
- Files go into a new folder, with one 3MF file holding every segment in its colour.
- Smooth surface keeps each segment's volume. The triangles button sets the detail.

## Dental CBCT {#dental}
- [[key:mpr-dental]]: panoramic view along the dental arch, with numbered cross-sections
- The arch is suggested from the teeth. Drag its points in the axial view to adjust it.
- Cross-sections are a fixed distance apart, with millimetre rulers. The spacing button changes it.
- [[key:mpr-nerve]]: click along the nerve canal to draw it in every view.
- Display only. Not for diagnosis or treatment planning.
