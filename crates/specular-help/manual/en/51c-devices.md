---
id: devices
title: Gamepads and SpaceMouse
section: reference
keywords: gamepad, controller, joystick, xbox, playstation, spacemouse, space mouse, 3dconnexion, spacenavd, navigation device, slide, whole-slide image, wsi, pan, zoom, orbit
---
A gamepad or a 3Dconnexion SpaceMouse can pan and zoom large images and slides, and turn 3D models and volumes.

They are off until you turn them on in Settings › Devices.

## Turning them on {#on}
1. Open Settings › Devices.
2. Tick **Use gamepads and SpaceMouse**.
3. Connect the device. It shows in the list under **Look again**.

**Dead zone** sets how far a stick moves before anything happens. **Speed** sets how fast the view moves.

## Gamepad {#gamepad}
- Left stick: pan
- Right stick up and down, or the triggers: zoom
- In 3D: left stick turns, right stick pans, triggers zoom
- D-pad left and right, LB and RB: previous and next image
- A: fit
- X: 1:1
- Y: Info
- B: back
- D-pad up and down: zoom one step

A is the bottom face button on any pad, Cross on PlayStation.

## SpaceMouse {#spacemouse}
- Push sideways, forwards or back: pan
- Push down: zoom in. Pull up: zoom out
- In 3D: tilt and twist to turn, push forwards to zoom
- Button 1: fit
- Button 2: 1:1

On Linux, Specular uses spacenavd when it runs.

## Changing the buttons {#buttons}
1. Open Settings › Keys and start recording a key for an action.
2. Press the device button.

In keymap.toml, gamepad buttons are `Pad-A`, `Pad-LB`, `Pad-Up` and so on. SpaceMouse buttons are `Puck-1`, `Puck-2`.

## When a device isn’t found {#problems}
- “Can’t read …”: your account may not read the device. Join the group the page names, then log out and back in.
- Specular changes nothing on your system by itself.
- macOS isn’t supported yet.
