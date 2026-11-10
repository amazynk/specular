---
id: mesh
title: 3-D models and point clouds
section: modes
contexts: mesh
keywords: 3d, model, walk, fly, section, cut, clipping, viewpoint, saved view, bcf, bcfzip, issue, topic, stl, obj, ply, gltf, glb, meshopt, usd, usdz, collada, dae, 3ds, lwo, fbx, mesh, point cloud, las, lidar, gaussian splat, splat, spz, 3d tiles, tileset, cesium, b3dm, i3dm, pnts, cmpt, molecule, pdb, orbit, wireframe, blend, skp, dwg, step, stp, iges, igs, cad, opencascade, ifc, ifczip, bim, building, x3d, vrml, wrl, rhino, 3dm, amf, off, ptx, vtk
---
For 3-D models, point clouds, Gaussian splats and molecules.

Commands that do not apply to a file are unavailable, such as point size for a solid model.

## Moving around {#orbit}
- Drag to orbit.
- Scroll to zoom.
- Drag with the middle button to pan.
- [[key:orbit-left]], [[key:orbit-right]], [[key:orbit-up]], [[key:orbit-down]] turn the view
- [[key:view-front]], [[key:view-top]], [[key:view-side]], [[key:view-iso]] jump to standard views

## Walk and fly {#walk}
- [[key:walk]]: walk on the model's floors. Walls stop you; steps up to 40 cm are climbed.
- [[key:fly]]: fly in any direction.
- W A S D or the arrows move. Drag to look around.
- Shift moves faster. When flying, Q and E go down and up.
- Esc, or the same key again, goes back to orbit.

## Cuts {#sections}
- [[key:section-plane]]: add a plane facing you, up to six.
- [[key:section-box]]: show only what is inside a box.
- Drag a handle to move its plane or box face.
- [[key:section-flip]]: cut away the other side.
- [[key:section-clear]]: remove every cut.
- Cut faces are filled and hatched in each material's colour.
- [[key:measure-cut]]: click two points on the cut to measure between them.

## Saved views {#views}
- [[key:save-view]]: save the camera, cuts and hidden parts under a name.
- [[key:views]]: the list. Enter goes to a view, F2 renames, Delete removes, P saves a PNG.
- Views are kept next to the model in a file ending in `.specular-views.json`.
- The model itself is never changed.

## BCF topics {#bcf}
- [[key:bcf-topics]]: the BCF panel. O opens a `.bcfzip` or `.bcf` file.
- Each topic shows its title, status, priority, assignee and comments.
- Enter goes to the topic's view: camera, cuts and, in IFC models, the parts it shows, hides or colours.
- ← and → step through a topic's views.
- N adds a topic, C a comment, both with the current view and a snapshot.
- S changes the status, E the title.
- Ctrl+S exports a `.bcfzip` (BCF 3.0, or the version opened).

## Model checks {#model-checks}
- [[key:model-checks]]: the checks panel of an IFC model. Tab or 1–4 switch between its parts.
- Enter goes to the chosen element: it is selected, the rest ghosted. P shows its properties.
- Ctrl+S exports BCF topics (quantities: an XLSX workbook), Ctrl+E a CSV table.
- **IDS**: O opens an `.ids` file. Each specification shows what passes and fails.
- Enter lists the failures and why; Backspace goes back.
- **Clashes**: A and B choose two sets: all, a class or a storey. M switches hard and clearance.
- \+ and − change the tolerance or the distance. R runs the test.
- S changes a clash's status. Statuses are kept next to the model in a file ending in `.specular-clashes.json`.
- **Changes**: O chooses another version of the model.
- Added elements are green, changed amber, removed red. F filters the list.
- **Quantities**: count, length, area and volume of each element. G groups by class, storey or type.
- Values from the file's quantity sets say "from file"; the others are computed from the shape.

## Several models {#models}
- **Add model…** in the palette, or A in the model list, adds another IFC file to the scene.
- **Add models from a folder…** (Shift+A in the list) adds every IFC file of a folder.
- Models are placed by their map coordinates, else their sites' latitude and longitude, else their shared project coordinates.
- If a model can't be aligned, Specular says why. You can place it at the origin.
- [[key:models]]: the list of models. Space shows or hides one, C colours each model, Delete removes one.
- Removing a model never touches its file.
- Each model counts against the memory Specular may use. A model that doesn't fit is refused.
- In the checks, G and H choose the models of clash sets A and B. Clashes name both models.
- BCF topics of clashes between models name both files.
- IDS, changes and quantities are about one model: M picks it.

## How it looks {#look}
- [[key:shading]]: shaded, wireframe or both
- [[key:projection]]: perspective or orthographic
- [[key:ground-grid]]: grid under the model

## Point clouds {#point-clouds}
- LAS, LAZ, E57, PCD, XYZ, PTS and PTX scans open as point clouds.
- [[key:points-bigger]]: bigger points
- [[key:points-smaller]]: smaller points
- [[key:point-colour]]: colour by height, intensity or own colour
- [[key:eye-dome-lighting]]: shading that makes the shape easier to see

## Scene files {#scenes}
- glTF and GLB open, also when compressed with meshopt.
- USD, USDZ, COLLADA, 3DS, LightWave and FBX open as models, with their transforms and materials.
- Skinned and animated models show their rest pose.
- FBX: geometry and materials only.
- X3D and VRML 97 open with their materials, textures and Inline files from the same folder.
- AMF 3-D printing files open with their colours.
- OFF files open with their vertex or face colours.
- Rhino files show their meshes and saved render meshes.
- A Rhino file of NURBS surfaces without render meshes shows its saved preview.

## 3D Tiles {#tiles3d}
- Open a `tileset.json`, or the folder that holds it.
- Coarse tiles show from afar; finer tiles load as you move closer.
- Until the finer tiles are read, the coarser ones stay on screen.
- The readout says how many tiles are drawn and how many have been read.
- On a server: open the `tileset.json` address with [[key:open-address]]. Tiles come only from that server and are kept in the cache.
- The info panel lists the tileset's facts and any tile that could not be shown, with the reason.
- A tileset with both models and points shows the models only.

## CAD models {#cad}
- STEP and IGES open through OpenCASCADE, if it is installed. Specular never downloads it.
- Parts keep their colours; the info panel lists the assembly.
- Curved faces are shown as fine triangles, not exact surfaces.
- Turn it off in Settings › Optional decoders › Use OpenCASCADE.

## PMI {#pmi}
- STEP files can carry PMI: dimensions, geometric tolerances, datums and saved views.
- They show as labels with leaders to their faces. Tolerances show as feature control frames.
- [[key:annotations]]: show or hide them.
- [[key:pmi]]: the list. The chosen one's faces light up; Enter goes to it.
- The file's saved views come first. Enter turns to one and shows only its annotations; A shows all.
- D draws them as the file draws them.
- The info panel counts them.

## Building models {#ifc}
- IFC files (IFC2x3, IFC4, IFC4x3, also zipped) open as models.
- Walls show with their openings cut where Specular can.
- Colours come from the file, else from the element's kind.
- Spaces and openings are not drawn.
- The info panel lists the site, buildings and storeys, with what each holds.
- Curved NURBS solids need OpenCASCADE; without it, only their flat faces show.

## Gaussian splats {#splats}
- SPLAT, SPZ and splat PLY files open in the same view.
- Colours change with the viewing angle when the file stores that.
- Point and shading keys do not apply.

## Previews only {#previews}
- Blender and SketchUp files show their saved thumbnail, not the scene.
- Files saved without one cannot be shown.

## Not opened {#refused}
- ifcXML files: export the model as IFC instead.
- glTF files compressed with Draco.
- 3D Tiles parts compressed with Draco, in glTF 1.0, or vector tiles (`.vctr`, `.geom`).
- NVIDIA PTX GPU code (it shares the .ptx extension with Leica scans).
- VRML 1.0 files.
- STEP and IGES need OpenCASCADE: see [Optional decoders](optional-decoders).
- DWG needs the free ODA File Converter. Install it and Specular uses it: see [Optional decoders](optional-decoders).

## Brain surfaces and tracts {#neuro}
- GIFTI surfaces open as models. A function or label file beside a surface colours it.
- CIFTI files colour both hemispheres when their surfaces are beside them. Otherwise they open as a volume.
- TrackVis (.trk) and MRtrix (.tck) tracts are coloured by direction. `[` and `]` change their thickness.
- [[key:tract-opacity]]: next opacity of the tracts
- [[key:mpr-tract-slab]]: in the three-plane view, the tracts near each slice, from a thicker or thinner slab

## Molecules {#molecules}
- [[key:molecule-style]]: ball-and-stick, spheres or cartoon
- [[key:molecule-colour]]: colour by element, chain or residue

## Keys {#keys}
::: keys mesh
