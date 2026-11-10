---
id: optional-decoders
title: Optional decoders
section: reference
keywords: optional decoders, decoder, sdk, codec, plugin, vendor, blackmagic raw, braw, red, r3d, dwg, oda file converter, opencascade, step, iges, zeiss, czi, libczi, ecw, mrsid, sid, hexagon, extensis, lizardtech, needs decoder
---
A few formats open only with free software from the company that made them. Specular calls these optional decoders.

Specular never includes, downloads or installs them. You install them yourself, once.

## How you know {#signs}
- A file that needs one shows a card instead of the picture: “… needs …’s free decoder”.
- In the gallery, its thumbnail carries a **Needs decoder** badge.
- Settings › Optional decoders lists each one and whether it was found.

## Getting one {#get}
1. Press **Get it**. The maker’s download page opens in your browser.
2. Install it as the page says.
3. Come back to Specular. It looks again when its window comes to the front, so nothing needs restarting.

If it still isn’t found, press **I’ve installed it** or **Look again**.

Installed somewhere unusual? Press **Locate…** and pick its file or folder.

## Switching one off {#off}
A decoder that is found is used by itself.

Untick it in Settings › Optional decoders to stop using it. Its files then don’t open.

**Forget…** drops a decoder you picked by hand. Nothing on disk is removed.

## The decoders {#list}
- **Blackmagic RAW** (.braw): the Blackmagic RAW SDK, version 6.0 or later, from Blackmagic Design. Specular shows a clip’s first frame.
- **DWG**: the ODA File Converter, from the Open Design Alliance.
- **RED R3D**: not possible. RED’s licence lets only programs built with its SDK, under RED’s own terms, read these files.
- **STEP and IGES**: OpenCASCADE, from Open Cascade. Specular reads the models in its sandboxed helper.
- **Zeiss CZI** (.czi): libCZI, from ZEISS. Specular reads the slides and stacks in its sandboxed helper. Not on Windows yet.
- **ECW** (.ecw): not possible. Hexagon’s licence lets only programs that ship its SDK use it. JPEG 2000 files made with it open, map position included.
- **MrSID** (.sid): not possible. The MrSID SDK’s licence lets only programs that ship the SDK use it.
- **DJI thermal photos** (R-JPEG): temperatures not possible. DJI’s SDK licence covers only programs you build yourself. The photo and its metadata open.

## Safety {#safety}
Blackmagic RAW clips and Zeiss CZI files are read in Specular’s sandboxed helper. It can read only the file you open.

DWG drawings are converted from a copy in a private folder, which is deleted afterwards.
