---
id: science
title: Scientific images
section: modes
contexts: science
keywords: fits, astronomy, microscopy, hdf5, netcdf, tiff, stretch, limits, planes, cube, channels, composite, wcs, coordinates, thermal, flir, testo, bmt, fluke, is2, mass spectrometry, bayer, raw frames, catalogue, diffraction, edf, smv, cbf, ccp4, mrc, density map, gaussian cube, orbital, img, numpy, npy, matlab, mat, zarr, tdms, vtk, arrays, fpack, planetary, vicar, pds, czi, zeiss, libczi, ds9, regions, reg, plate solving, astap, astrometry.net, samp, topcat, aladin, hips, sky survey, deltavision, fluoview, olympus, serialem, mdoc, tilt series
---
For telescope frames, spacecraft images, microscope stacks, climate fields, spectra and other measured data.

## Stretch and limits {#stretch}
- [[key:stretch]]: how values become brightness: linear, logarithmic, square root or asinh
- [[key:limits]]: range shown, from full range to percentile clips
- [[key:reset-look]]: back to the defaults
- [[key:import-lut]]: loads a colour table

## Planes and cubes {#planes}
- [[key:next-plane]]: next plane (depth, time or wavelength)
- [[key:prev-plane]]: previous plane
- [[key:next-axis]]: chooses which axis to step along
- [[key:mpr]]: a z-stack or cube in three planes. See [multi-planar views](medical-3d#mpr).

Unavailable for files with a single plane.

## Channels {#channels}
- [[key:composite]]: shows all channels together, each in its own colour
- [[key:channel-colours]]: changes the colours

Unavailable for files with a single channel.

## Coordinates {#coordinates}
[[key:coordinate-grid]] draws sky or map coordinates over the image. The pixel inspector then shows the pointer's coordinates.

Unavailable for files without coordinates.

## Special kinds of data {#special-data}
- [[key:hillshade]] and [[key:level-plane]]: elevation and surface height
- [[key:seismic-wiggle]]: seismic sections
- [[key:power-spectrum]]: frequency content of an image
- [[key:open-catalogue]]: marks star catalogue sources over a sky image; [[key:clear-catalogues]] removes them
- [[key:mesh-wireframe]]: the edges of an unstructured mesh, over its colours

Each view is available only for its kind of data.

## Diffraction frames and density maps {#diffraction}
- Detector frames (CBF, EDF, SMV, Bruker, MarCCD, R-AXIS): the pixel inspector shows q, d and 2θ; [[key:coordinate-grid]] draws resolution rings.
- EDF files with several frames or a volume: [[key:next-plane]] steps through them.
- CCP4/MRC maps and Gaussian cube files: [[key:next-plane]] steps through z; [[key:mpr]] shows the volume in three planes and in 3-D.
- MRC tilt series with a SerialEM `.mdoc` beside them: each tilt is a plane, named by its angle. The Info panel lists each tilt's values.
- DeltaVision files: wavelengths are channels; [[key:next-plane]] steps through z and time.
- Olympus FluoView TIFF stacks: channels, z and time as their own axes, with the pixel size.
- Cube files with several orbitals: [[key:next-axis]], then [[key:next-plane]], picks the orbital.
- A cube's atoms show on the plane nearest them, and faded on the planes either side.

An `.img` opens by what it holds. One that is none of the kinds Specular reads is refused with that list.

## Spacecraft images {#planetary}
VICAR, PDS3 and PDS4 files open from their label or their data file.

Each image in a product is a page. Bands are channels.

The Info panel shows the target, spacecraft, instrument, times and map projection.

## Satellite and radar images {#remote-sensing}
ENVI cubes open from their `.hdr` or their data file.

- A cube with default bands, or bands across the visible, opens as a colour composite; all bands are the next page
- Point at a pixel to plot its spectrum; bad bands (the file's `bbl`) are left out

Complex radar (SAR) images open in ENVI, NITF (SICD) and TIFF.

- The Part slider shows dB first, then magnitude, real, imaginary and phase
- The Looks slider averages blocks of pixels (multilook) to reduce speckle

Sentinel-2 products open as a `.SAFE` folder, its `MTD_MSIL2A.xml` or `MTD_MSIL1C.xml`, or a ZIP of the product.

- Pages are views at 60, 20 and 10 m: true colour, false colour, short-wave infrared, agriculture, all bands, scene classification and cloud probability
- Values are reflectance
- 10 m pages read the largest files: they take longest to open

STAC items, collections and catalogs (JSON) open as their GeoTIFF images, one page each. Open them from a file or by address.

## Mass spectrometry images {#mass-spec}
[[key:ion-image]] shows where one ion occurs across the sample, chosen by its mass.

Unavailable for other files.

## Thermal images {#thermal}
Opens FLIR, InfiRay, InfraTec, Testo (.bmt) and Fluke (.is2) images.

- [[key:temperature-unit]] Celsius, Fahrenheit or kelvin
- [[key:isotherm]]: marks a range of temperatures
- [[key:thermal-parameters]]: changes emissivity and distance
- [[key:export-temperatures]]: saves the temperatures as a table
- [[key:thermal-visible]]: shows the visible-light photo, when the camera took one
- [[key:thermal-report]]: saves a PDF report with the measurements, facts and your notes

Spots (the measuring tool's S) read one pixel's temperature.

Emissivity changes work on FLIR and Fluke images. The others store finished temperatures.

Older Testo files (testo 875 to 890) and older Fluke .is2 files don't open. Their layout isn't published.

DJI drone thermal photos open as photos with their metadata. Their temperatures need DJI's SDK, which Specular can't use.

## Thermal sequences {#thermal-sequences}
FLIR .seq and .csq files play frame by frame.

- [[key:thermal-play]]: plays or pauses at the recorded speed
- The frame slider moves through time.
- Spots, lines and areas are plotted over time, under the panel's play button.
- [[key:thermal-series-csv]]: saves each one's temperature in every frame
- [[key:export-temperatures]]: saves the frame shown

Each frame uses the emissivity and distances it was recorded with, until you change them.

## Camera frames {#camera-frames}
- [[key:debayer]]: turns colour reconstruction of unprocessed sensor frames on or off
- [[key:bayer-pattern]]: sets the sensor's pattern when the file does not say

## Arrays and lab data {#arrays}
NumPy, MATLAB, Zarr, NI TDMS and VTK files open here.

- Each array, variable, channel or data array is a page: [[key:next-page]] and [[key:prev-page]] step through them by name
- Struct fields and cells are named by their path, such as `s.field` or `c{2}`
- A one-dimensional array is a strip: point at it to plot all its values
- Complex values: step the Part axis for magnitude, real part, imaginary part or phase
- A Zarr store opens as a folder, a `.zarr.zip`, or its `.zarray` or `zarr.json` file
- Short texts, scalars and arrays that cannot be shown are listed in the Info panel

VTK surfaces and cells open as [3-D models](mesh).

## Flow cytometry {#cytometry}
FCS files open as a density plot of two parameters, FSC against SSC first.

- The X and Y sliders choose the parameters; Y's last position shows a histogram of X
- The X scale and Y scale sliders choose Auto, Linear, Log, Logicle or Arcsinh
- The Compensation slider turns the file's spillover matrix on or off, when it has one
- Point at the plot to read both values and the events in that bin
- Brighter colours mean more events; [[key:stretch]] changes how counts are shown
- Each dataset of a file is a page

The Info panel lists every keyword of the file.

## Zeiss CZI files {#czi}
CZI files open with libCZI, if it is installed. Specular never downloads it.

- Each scene is a page, then the label, slide preview and thumbnail
- Large slides load tile by tile as you zoom
- Stacks: channels, Z and time are planes, with ZEN's channel names and colours
- The scale bar uses the file's pixel size

Turn it off, or locate libCZI, in [Settings › Optional decoders](settings).

## Astronomy tools {#astronomy}
- [[key:load-regions]]: draws a DS9 region file (.reg) on the image. Dropping a .reg file works too.
- [[key:edit-regions]]: turns the regions into measurements you can move and change.
- [[key:export-regions]]: saves the measurements and regions as a DS9 region file.
- [[key:clear-regions]]: removes the regions.
- [[key:plate-solve]]: finds the sky coordinates with ASTAP or astrometry.net. Install one first. Run it again to stop.
- [[key:save-plate-solution]]: saves them beside the image as a .wcs file. The image itself is never changed.
- [[key:sky-survey]]: shows a sky survey around the image. Each press picks the next survey, then none. Tiles come from CDS in Strasbourg.
- [[key:samp-send-image]] and [[key:samp-send-position]]: send the image, or the sky position under the pointer, to DS9, TOPCAT, Aladin and other SAMP applications.

SAMP is off until you turn on "Connect to a SAMP hub" in [Settings › Integration](settings). Only a hub on this computer is used.

Images, tables and positions other applications send are opened, overlaid or shown. Clicking a source of a table they sent highlights it in them.

B1950 (FK4) images, catalogues and regions line up with ICRS and J2000 ones: positions are converted.

On a B1950 image the readout keeps B1950 and says when markers or SAMP positions are converted.

Regions in ecliptic or linear coordinates, and ruler, compass and projection regions, are not read. Pandas are drawn as their inner and outer edges.

## Annotations {#annotations}
[[key:annotations]] shows regions and labels stored in the file, such as cell outlines.

Unavailable when the file has none.

## Keys {#keys}
::: keys science
