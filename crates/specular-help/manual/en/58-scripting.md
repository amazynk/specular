---
id: scripting
title: Scripting and recipes
section: reference
contexts: scripting
keywords: script, scripting, python, numpy, remote, socket, ipc, api, automation, recipe, segmentation, overlay, command, napari, events, hooks, watch, subscribe
---
A running viewer listens on a local socket. Only your user account can reach it. Nothing listens on the network.

Programs can open files, step through them, set the view, read metadata and measurements, show NumPy arrays, draw overlays and take screenshots.

They can also select and mark photos, apply a Develop style, export with a recipe, and hear what happens in the viewer.

To turn the socket off: Settings › Integration › Remote control and events.

The protocol is described in `docs/ipc.md`.

## From the command line {#command-line}
- `specular ipc next` steps to the next image. `specular --remote next` does the same.
- `specular ipc mark --flag pick --stars 4 FILE…` marks photos. Ctrl+Z in the gallery undoes it.
- `specular ipc select FILE…` selects photos in the gallery.
- `specular ipc apply-style STYLE FILE…` applies a Develop style.
- `specular ipc export RECIPE FILE…` exports with a recipe.
- `specular ipc watch` prints what happens, one line per file, until Ctrl+C. Add event names to pick, `--json` for whole events.
- `specular ipc events` lists the event names.
- `specular ipc raw` sends JSON requests from standard input.

A file named `ipc` opens as `./ipc`.

## From Python {#python}
The `specular` package is in `clients/python`. It needs only Python; NumPy is optional.

```
import specular
viewer = specular.connect()
viewer.show_array(image, name="result")
viewer.set_view(zoom=200, window=(40, 400))
for e in viewer.events(["marked"]):
    print(e["files"], e["mark"])
```

## Events {#events}
- `opened`: an image stayed on screen for a moment.
- `marked`: picks, rejects, stars or labels changed.
- `keywords`: keywords were written.
- `exported`: an export with recipes finished.
- `imported`: an import finished.
- `captured`: a tethered camera sent a photo.
- `develop-saved`: a Develop edit was written.

Marking 500 photos is one event with 500 files.

## Hooks {#hooks}
A hook runs a program when an event happens. Add it to `settings.toml`:

```
[[hooks]]
name = "Upload picks"
on = ["marked"]
when = { flag = "pick" }
program = "/home/me/bin/upload"
args = ["{files}"]
```

- The first time, the viewer asks before it runs it. A changed hook asks again.
- The program gets the files as arguments and the event as JSON on standard input.
- Placeholders: `{file}` `{files}` `{dir}` `{event}` `{recipe}` `{outputs}` `{stars}`.
- It starts directly, never through a shell.
- It is stopped after `timeout` seconds (60 by default).
- What the program does through `specular ipc` runs no hooks.

Settings › Integration lists the hooks. Each has a switch, and Remove… takes it out of `settings.toml`.

## Copy as command {#copy-as-command}
- [[key:copy-view-command]]: copies a shell command that opens this file with this view.
- [[key:copy-view-python]]: copies the same as Python.

The view is the zoom, the position, the window/level, the colour map and the channel.

## Recipes {#recipes}
A recipe runs a program on the current file and loads what it writes.

Put one recipe per `.toml` file in the `recipes` folder of the settings folder. Recipes show in the palette.

```
name = "Segment nuclei"
program = "segment-nuclei"
args = ["{file}", "{out}/labels.png"]
output = "{out}/labels.png"
load = "labels"
timeout = 600
```

- `load = "open"` opens the output.
- `load = "overlay"` draws its GeoJSON shapes over the image; a feature's `name` is its label.
- `load = "labels"` outlines each region of a label image over the image.
- `series = true` passes every file of the folder as `{files}`. A feature's `file` (a path or file name) puts it on that file.

`{out}` is an empty folder for each run. The other placeholders are those of user commands.

The first run asks before it starts. A changed recipe asks again.

Programs start directly, without a shell. Add `shell = true` to run `program` as a shell command line.

A recipe is stopped after its `timeout` in seconds (300 by default).
