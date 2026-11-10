---
id: files
title: Working with files
section: modes
contexts: files
keywords: copy, import template, import keywords, import style, paste, path, trash, delete, undo, open with, open as, which kind, file type, extension, other app, file manager, rename, move, rotate, redo, wallpaper, share, email, import, memory card, camera, phone, eject, qr, qr code, data matrix, datamatrix, aztec, aztec code, barcode, read codes
---
Specular changes a file only when you ask: renaming, moving, or turning the file itself.

## Renaming {#rename}
- [[key:rename]]: rename this file
- [[key:batch-rename]]: rename the selected files with a pattern

Sidecars and the RAW or JPEG partner are renamed with the file.

## Copying and moving to a folder {#move}
- [[key:copy-to-folder]]: copy to a folder
- [[key:move-to-folder]]: move to a folder

When a name is taken, choose Skip, Keep both or Replace. Replace sends the old file to the trash.

## Turning the file {#rotate}
- [[key:rotate-file-left]] and [[key:rotate-file-right]]: turn the file itself

JPEG turns without loss. Other formats record the turn in the file. RAW files get it in a sidecar; the RAW file stays untouched.

[[key:rotate-left]] and [[key:rotate-right]] turn only the view.

## Undo {#undo}
- [[key:undo]]: undo the last change
- [[key:redo]]: redo it

Undo works only while the file is unchanged since.

## Wallpaper, sharing and email {#share}
- [[key:set-wallpaper]]: use the image as wallpaper
- [[key:share]]: share with another app
- [[key:send-by-email]]: attach to a new email
- [[key:share-with-zynk]]: make a web link to the file in the Zynk app
- [[key:send-with-zynk]]: send the file to a contact or one of your devices in the Zynk app

These need support from your desktop. When it is missing, the command says so.

## Copying {#copy}
- [[key:copy-image]]: copies the image as you see it
- [[key:copy-path]]: copies the file's location
- [[key:paste]]: opens an image copied elsewhere

## Moving to the trash {#trash}
- [[key:trash]]: moves the file to the system trash and shows the next image
- [[key:undo]]: puts it back

Files inside archives cannot be moved to the trash.

## Importing from a card or camera {#import}
1. Connect a memory card, camera or phone.
2. Press [[key:import]], or choose Import when Specular offers it.
3. Untick what you don't want, choose where it goes, press Import.

Only photos not imported before are shown. RAW and JPEG pairs stay together.

Each photo can get, as it is copied:

- a metadata template (made in Edit metadata),
- keywords, separated by `;` (`Places > Paris` for a level),
- a Develop style.

The template and keywords go into the photo's XMP sidecar.
The style is kept like any Develop edit.
The photo itself is not changed.

Copies are checked before anything is deleted from the card.

## Image servers {#servers}
- **OMERO servers…** in the palette: browse projects and images on an OMERO server or the Image Data Resource, and open one.
- **DICOMweb servers…** in the palette: search image archives and download studies. See [Send to PACS](medical#send-to-pacs).
- An OMERO password is kept only until Specular closes, never on disk.

## Opening in another app {#open-with}
[[key:open-with]] opens the image in another app, such as an editor.

Changes saved there show here straight away.

## Which kind of file? {#file-kind}
Some extensions are used by several formats (.pic, .pls, .img, .raw).
The content usually says which one; when it doesn't, Specular asks.

The likeliest format comes first. Files beside it count: other .pic files already read, or a header a .raw belongs to.

Choose how far the answer goes: this file, the extension in this folder, or everywhere.

[[key:open-as]] (Open as… in the file’s menu) asks again.

To forget a choice: Settings › Optional decoders, under Forget all file kinds….

Data no format reads (.bin, .dat) can open as raw samples once you give their layout.

## Reading codes {#codes}
Read codes finds the QR, Data Matrix and Aztec codes in the picture shown (from the palette).

Each code is outlined on the image and listed with its text. Choose a row to copy the text.

A link opens in your web browser only after you confirm its address. With No network on, links don't open.

## Saving copies {#save}
- To save in another format, size or colour profile, use [Convert](convert).
- To print, see [Printing](print).
