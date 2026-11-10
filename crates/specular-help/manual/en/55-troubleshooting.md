---
id: troubleshooting
title: Troubleshooting
section: reference
contexts: troubleshooting, error
keywords: problem, error, slow, broken, corrupt, won't open, unsupported, gpu, driver, memory, blank, crash, gpu reset, driver reset, device lost
---
## An image won't open {#wont-open}
Specular says why. The usual reasons:

- **The file is damaged or incomplete**, for example an interrupted download. Specular shows as much as it can read.
- **The format or variant is not supported.** The [formats list](opening#formats) says what is.
- **The image needs more memory than is free.** Specular says how much. Close other apps and try again.
- **The file is password-protected.** Enter the password; it is kept in memory only.
- **It needs a free decoder from its maker**, such as Blackmagic RAW or DWG. See [Optional decoders](optional-decoders).

If an image that should open doesn't, save a [diagnostics bundle](diagnostics#bundle) and report it.

## Images open slowly {#slow}
- Large images show a sharp preview first, then full detail.
- Network shares and USB drives are slower than internal disks.
- **Why is this slow?** in the corner explains what holds things up and what would help.
- Settings › Performance shows which GPU is used and lets you give Specular more memory.

## Nothing shows, or colours look wrong {#display}
1. Update your graphics driver. Specular needs Vulkan, Metal or Direct3D 12.
2. Check the display profile in Settings › Colour.

If the graphics driver resets the GPU, Specular opens it again and says so. Work under way starts again.

## Starting over {#reset}
Settings › Advanced › Reset all settings… sets every setting back to its default. It asks first, and Undo brings them back.

Your images, marks and library are not affected.
