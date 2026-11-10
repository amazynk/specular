---
id: tether
title: Tethered capture
section: modes
keywords: tether, tethered, camera, usb, ptp, capture, shutter, iso, aperture, white balance, udev, gvfs
---
Shoot with the camera on a USB cable: each shot is saved to a folder and shown as it arrives.

Open it with [[key:tether]] or the command palette.

## Connecting {#connect}
- Connect the camera and turn it on. If it has a USB mode, choose PTP or PC connection.
- With one camera, Specular connects by itself. With several, choose **Next camera**, then **Connect**.
- The panel shows the model, the battery and the card.

## Taking pictures {#capture}
- **Capture** or [[key:tether-capture]] takes a picture.
- Shots taken with the camera's own shutter button are saved too.
- Some cameras can't be triggered from a computer without their maker's own protocol. They say so, and only their own button works.

## Exposure {#exposure}
Mode, shutter, aperture, ISO, exposure compensation and white balance show when the camera reports them.

- − and + step through the values the camera offers.
- The camera's mode dial decides which ones it lets you change.

## Where shots go {#folder}
- **Folder…** chooses the folder. Unset, it is Pictures › Tethered.
- File names follow Settings › Cull mode › Tethered file names: the same tokens as Import. Empty keeps the camera's names.
- **Style…** chooses the Develop style new shots get, as Develop's Next capture does.
- In cull mode on that folder, each shot joins the run and is shown. Otherwise the viewer shows it.

## When the camera can't be opened {#permissions}
Specular never changes system files. The panel says what is wrong.

On Linux, the camera's USB device must be yours. A udev rule does that. As administrator, create `/etc/udev/rules.d/70-specular-camera.rules` with the line the panel shows, for example:

```
SUBSYSTEM=="usb", ATTR{idVendor}=="04a9", ATTR{idProduct}=="32b4", MODE="0660", TAG+="uaccess"
```

Then run `sudo udevadm control --reload` and plug the camera in again.

Only one program can use the camera at a time. The desktop's camera service (gvfs, KDE's camera support), gphoto2 or another photo program may hold it.

The panel names that program when it can. Unmount the camera in the file manager, or close that program, then **Try again**.

On macOS, quit Image Capture and Photos first. On Windows, the system's camera driver does not let other programs in yet.
