# Specular remote-control protocol (IPC), version 3

Status: **stable**. Each version is a superset of the one before; a viewer that speaks
version 3 answers version 1 and 2 requests exactly as before. Version 3 adds events
(section 9) and the commands `subscribe`, `unsubscribe`, `select`, `mark`, `apply-style` and
`export` (section 5.5). Hooks, programs the viewer runs on events, are in section 10.

Clients in this repository: Python, `clients/python/specular/`; Rust, `crates/specular-client/`;
the command line, `specular ipc …` (also `specular --remote …`).

## 1. Transport

* **Local only.** A stream socket on this computer: a Unix domain socket on Linux, BSD and
  macOS, an `AF_UNIX` socket on Windows 10 1803 and later. Nothing listens on the network,
  ever.
* **Per user.** The socket lives in a folder only its user may enter:
  1. `$XDG_RUNTIME_DIR/specular/` (Linux, BSD), when that folder is ours and mode 0700;
  2. macOS: `$TMPDIR/specular-<uid>/`;
  3. else `specular/run/` in the cache folder (`~/.cache` on Linux,
     `~/Library/Caches/Specular` on macOS, `%LOCALAPPDATA%\Specular\cache` on Windows);
  4. else, when that path is too long for a socket, `$TMPDIR/specular-<uid>/` or
     `/tmp/specular-<uid>/` (Unix) or `%LOCALAPPDATA%\Specular\run` (Windows).

  The folder is created mode 0700 (and refused when it is a link or someone else's); the socket
  is 0600. On Unix every connection's peer is checked to run as the same user
  (`SO_PEERCRED` / `getpeereid`); others are dropped. On Windows the folder's ACL admits only
  the user, and the viewer writes a random key next to its socket (`specular.sock.key`,
  readable only through that folder): a client's first request must be `hello` with
  `"key": "<the file's text>"`; anything else, or a wrong key, gets `refused`. The clients
  here do this by themselves.
* **Flatpak.** The sandboxed viewer's runtime folder is shared with the desktop
  (`--filesystem=xdg-run/specular:create`), so programs outside it find the socket in
  `$XDG_RUNTIME_DIR/specular/` as usual.
* **Off switch.** Settings › Integration › Remote control and events
  (`integration.ipc = false`): no socket at all after a restart.
* **Which viewer.** The first viewer binds `specular.sock`; later ones bind
  `specular-<pid>.sock` next to it. Clients use `$SPECULAR_SOCKET` when set, else the first
  folder above that holds `specular.sock`.
* **Connections.** At most 8 at once (a ninth gets a `busy` error line and is closed). A
  connection idle for 120 s is closed. Each request is answered within 5 s or gets `timeout`.
  Connections that subscribed to events (section 9) count apart: at most 4 more, with no idle
  limit.

## 2. Framing

One JSON object per line in each direction: UTF-8, ended by `\n` (`\r\n` is accepted), at
most 1 MiB (1 048 576 bytes) without the newline. Blank lines are ignored. Requests on a
connection are answered one at a time, in order. A line over the limit gets a `too-long`
error and the connection is closed (nothing after it can be trusted).

Binary data (arrays, screenshots) travels as **blobs** (section 5.3): byte buffers moved in
base64 chunks of at most 512 KiB of data each, so every line stays under the limit.

### Request

```json
{"v": 2, "id": 7, "cmd": "view-set", "zoom": 200}
```

| Member | Meaning |
|---|---|
| `v` | Protocol version of this request: `1`, `2` or `3`. Required. |
| `id` | Optional: a number or a string of at most 256 bytes; echoed in the response. |
| `cmd` | The command (section 5). |
| others | The command's arguments. Unknown members are ignored. |

### Response

```json
{"v": 2, "id": 7, "ok": true, "result": {…}}
{"v": 2, "id": 7, "ok": false, "error": {"code": "bad-arguments", "message": "…"}}
```

The response carries the request's version (or 3 when the request could not be read). The
message is English text for people; programs act on `code`.

A subscribed connection (version 3) also receives **event lines** between responses. They
carry `"event"` and never an `"id"`, which tells them from responses:

```json
{"v": 3, "event": "marked", "seq": 1043, "time": "2026-11-08T10:12:03.120Z", "files": ["/p/a.cr3"], "mark": {"flag": "pick", "stars": 4, "label": null}, "source": "user"}
```

## 3. Versions and capabilities

* A client sends `hello` with the newest version it speaks. A version 3 viewer answers with
  `protocol: 3`, `protocols: [1, 2, 3]`, the full command list, `events` (the names of
  section 9) and `capabilities`. An older viewer answers a newer request with a `version`
  error: the client then falls back to the version before.
* A version 1 request is answered in version 1 (its `hello` result is exactly version 1's); a
  version 2 request as before (`protocol: 2`, the version 2 commands; `protocols` lists every
  version the viewer speaks). A command newer than the request's version gets a `version`
  error.
* `hello` may also carry `key` (Windows, section 1) and `hook`: a program a hook started
  (it finds `$SPECULAR_HOOK` set) names the hook, and what the connection does is then reported
  as the hook's (section 10).
* `capabilities` (version 2):

| Key | Meaning |
|---|---|
| `max_line` | Longest line, bytes (1048576). |
| `max_chunk` | Most data bytes per `blob-write` / `blob-read` (524288). |
| `max_blob` | Largest blob, bytes (4 GiB; the memory budget may allow less). |
| `max_blobs` | Blobs one connection may hold (16). |
| `max_dims`, `dtypes` | Arrays: up to 8 dimensions; the NumPy types accepted. |
| `array_sources` | `["blob", "file"]`. |
| `shared_dir` | The folder for `array-show` with `file` (or `null`). |
| `max_shapes`, `max_points`, `shapes` | Overlays: shapes per request, points per shape, kinds. |
| `colormaps` | Colour map ids for `view-set`. |
| `roi_formats`, `screenshot`, `view_command` | Formats offered. |
| `max_subscribers` | Version 3: subscribed connections at once (4). |
| `event_queue` | Version 3: events queued per subscriber before they are lost (1024). |
| `max_part_items` | Version 3: list entries per event line (10000); more go in parts. |
| `ping_seconds` | Version 3: the longest silence a subscriber hears (60). |
| `labels` | Version 3: the colour label names (`red`, `yellow`, `green`, `blue`). |

Clients must ignore capabilities they do not know.

## 4. Errors

| Code | Meaning |
|---|---|
| `parse` | Not JSON, or not an object. |
| `version` | Unsupported `v`, or a command newer than the request's version. |
| `too-long` | A line (or a response) over the limit. |
| `unknown-command` | No such command. |
| `bad-arguments` | A missing, malformed or out-of-range argument; the message says which. |
| `unknown-action` | `action` names no action. |
| `disabled` | The action is unavailable right now (the message says why). |
| `not-found` | No such file, image, position or blob. |
| `refused` | Not allowed (a remote link; a shared file that is a link or not a regular file). |
| `mode` | Not possible here (e.g. window/level on a photo). |
| `busy` | Too many connections, blobs or overlay shapes. |
| `no-memory` | The memory budget cannot hold it (the message says how much is needed). |
| `timeout` | The window did not answer in time (e.g. a minimised window's screenshot). |
| `unknown-event` | Version 3: `subscribe` or `unsubscribe` names no event. |
| `unknown-style` | Version 3: `apply-style` names no Develop style. |
| `unknown-recipe` | Version 3: `export` names no export recipe. |
| `internal` | Anything else. |

Version 3 uses `refused` also for a missing key (Windows) and for an export recipe that asks
for its folder each time, and `busy` also for a fifth subscriber and an export while one runs.

## 5. Commands

Coordinates are **image pixels of the stored image** (as the file holds it, before any
rotation): x right, y down, `0,0` the top-left corner of the top-left pixel. Arrays use the same
grid: element `[y, x]` covers `x..x+1`, `y..y+1`.

### 5.1 Version 1 (unchanged)

| `cmd` | Arguments | `result` |
|---|---|---|
| `hello` | | `app`, `version`, `protocol`, `pid`, `commands` (+ version 2 members) |
| `open` | `files`: 1–10000 absolute paths or `file://` URLs; `window`: `"current"` (default) or `"new"`; v2: `view` (below) | `opened`, `missing` |
| `open-url` | `url`: a `specular:` URL | `status`: `"opened"` or `"confirm"` |
| `navigate` | exactly one of `to` (`next`, `prev`, `first`, `last`), `index` (0-based), `by` (signed) | `index` |
| `state` | | `file`, `index`, `count`, `zoom`, `fit`, `fullscreen`, `mode`, `folder`, `mark`, `marked` |
| `marked` | | `files` |
| `action` | `name`: a palette action id (`zoom-in`, `zoom-to:200`, `rotate-left`, …) | `{}` |
| `activate` | | `{}` |
| `quit` | | `{}` |

`open` with `view` (version 2) applies the view to the first file once it is shown, and once
its values are in for `window` (at most 15 s later; what cannot be applied by then is left).

### 5.2 View and image (version 2)

**`view-get`** → `file`, `index`, `image` {`width`, `height`}, `orientation` (EXIF 1–8),
`zoom` (percent), `fit`, `center` [x, y], `window` ({`center`, `width`, `unit`, `range`} or
`null`), `colormap` (id or `null`), `channel` (`"all"` or a number), `channels` (a list of
{`name`, `shown`} for multichannel science images, else `null`).

**`view-set`**: any of

| Argument | Values |
|---|---|
| `zoom` | `"fit"`, or a percentage 0.1–100000 |
| `center` | [x, y]: the image point at the centre of the view |
| `window` | `"auto"`, or {`center`, `width` > 0} in the image's units (e.g. HU): medical and science images only (`mode` error otherwise) |
| `colormap` | a colour map id, or `"none"` |
| `channel` | `"all"`, or a 0-based channel (photos: 0 red, 1 green, 2 blue, 3 alpha) |

Nothing changes when an argument is invalid. → the view, as `view-get`.

**`metadata`** → `file`, `format`, `width`, `height`, `bit_depth`, `has_alpha`, `coding`,
`page`, `pages`, `calibration` ({`pixel_size` [x, y], `unit`, `source`, `matrix`} or `null`),
`value_unit`, `science` ({`width`, `height`, `sample`, `unit`, `axes` [{`name`, `len`,
`unit`}]} or `null`), `summary` [{`group`, `key`, `label`, `value`}], `fields` [{`group`, `tag`,
`value`}] (at most 20000), `metadata_pending` (true while the metadata is still being read:
ask again).

**`screenshot`** → `blob`, `bytes`, `width`, `height`, `format` (`"png"`): a PNG of what the
window shows at its next frame, in a blob (read it with `blob-read`, then free it). A window
that does not draw (minimised) answers `timeout`.

**`view-command`** {`lang`: `"shell"` (default) or `"python"`} → `text`: a snippet that opens the
current file with the current view (as the palette's "Copy view as command").

### 5.3 Blobs and arrays (version 2)

Blobs belong to their connection and are freed when it closes. Each is reserved from the
viewer's memory budget when made (`no-memory` when it does not fit).

| `cmd` | Arguments | `result` |
|---|---|---|
| `blob-new` | `bytes` (1 – 4 GiB) | `blob`, `bytes` |
| `blob-write` | `blob`, `offset`, `data` (base64, at most 512 KiB decoded) | `written` |
| `blob-read` | `blob`, `offset`, `length` (clamped to 1 – 512 KiB) | `data` (base64), `bytes`, `total` |
| `blob-free` | `blob` | `{}` |

**`array-show`** shows an array as an image (2-D greyscale, a stack, channels: as Specular
shows a NumPy file):

| Argument | Values |
|---|---|
| `blob` or `file` | the data: a blob of this connection (taken by the command), or a bare file name in `capabilities.shared_dir` (read, then removed) |
| `shape` | 1–8 sizes, outermost first |
| `dtype` | a NumPy type string: `b1 u1 i1 u2 i2 u4 i4 u8 i8 f4 f8`, with optional `<`, `>`, `\|` or `=` |
| `strides` | optional, bytes per step of each dimension (NumPy's); negative strides are allowed: the data then starts at the lowest-addressed element. Absent: C order |
| `name` | optional, shown as the file name: up to 64 letters, digits, spaces, `-`, `_`, `.` |

The data must cover every element the shape and strides reach (`bad-arguments` otherwise). The
viewer gathers it into C order, writes a NumPy file into `arrays/` next to the socket (one
folder per array; the last 16 are kept, all are removed when the viewer quits) and opens it
like any file. → `file`, `shape`, `dtype`.

A shared file must be a regular file (not a link, not a folder, no other hard links): `refused`
otherwise.

### 5.4 Overlays and measurements (version 2)

Overlays are drawn over the current file's image, kept per file for this window, in layers.

**`overlay-add`** {`layer` (default `"default"`, 1–64 bytes), `shapes`: 1–10000 of}:

| Member | Values |
|---|---|
| `kind` | `point` (1 point), `line` (2), `polyline` (2+), `polygon` (3+, closed), `rect` (2 corners), `text` (1: its top-left) |
| `points` | [[x, y], …], finite, at most 100000 per shape and 1000000 per request |
| `text` | the text of `text`; a label next to other shapes (at most 1024 bytes) |
| `color` | `#rgb`, `#rrggbb` or `#rrggbbaa` (default `#ffd600`) |
| `width` | line width in screen pixels, 0.5–20 (default 2) |

→ `added`, `layer`. A window keeps at most 200000 shapes in all (`busy` beyond).

**`overlay-clear`** {`layer`: optional, all layers when absent} → `removed`.
**`overlay-list`** → `file`, `layers` [{`layer`, `shapes`}].

**`rois`** {`format`: `"geojson"` (default) or `"json"`}: the user's measurements on the
current image. GeoJSON: a FeatureCollection as Specular exports it (image pixel coordinates,
calibration and provenance in `properties`). JSON: `measurements` [{`id`, `kind`, `points`,
`label`, `frame`, `value`, `unit`, `calibrated`}].

### 5.5 The gallery, Develop and export (version 3)

| `cmd` | Arguments | `result` |
|---|---|---|
| `subscribe` | `events`: 1–64 names (section 9), or `["*"]` for all | `seq`: events after it follow |
| `unsubscribe` | `events`: names to stop; absent: all | `subscribed`: whether any are left |
| `select` | `files`: absolute paths; `add`: true keeps the selection and adds these | `selected`, `missing` (not shown in the gallery) |
| `mark` | `files`; at least one of `flag` (`pick`, `reject`, `none`), `stars` (0–5), `label` (`red`, `yellow`, `green`, `blue`, `none`); `null` clears | `marked`, `changed`, `missing` |
| `apply-style` | `files`; `style`: a Develop style's name | `job`, `style`, `files` (the photos Develop can edit) |
| `export` | `files`; `recipe`: an export recipe's name (or id) | `job`, `recipe`, `files` |

* `select` needs the gallery open (`mode` otherwise). The focus moves to the first file.
* `mark` goes through the gallery's marks like the culling keys: one undo step in the
  window, sidecars written when that setting is on, and a `marked` event.
* `apply-style` and `export` answer at once and run in the background, never holding the
  5 s answer limit. `develop-saved` (with the same `job`) tells when the edits are written;
  `exported` (with `job`) when the export is done, with what failed. A recipe that asks for its
  folder each time is refused (only the window can ask); an export that would replace existing
  files does not replace them and ends at once, its `exported` saying so in `failed`.
  Neither asks in the window: the socket is the user's own.

### 5.6 Test commands (debug builds only)

Debug builds answer two more commands from version 3 on, for scripted runs of the app
(`tools/realuse/`). Release builds answer `unknown-command`, and `hello` does not list them.

| `cmd` | Arguments | `result` |
|---|---|---|
| `idle` | `quiet_ms` (default 1000, at most 60000), `timeout_ms` (default 30000) | `settled`, `waited_ms`, `held_by` (what still held it: `drawing`, `frame-asked`, `timer`, `loading`, `gallery`, `calibrating`) |
| `ui-state` | | `gallery`, `gallery_dir`, `gallery_images`, `develop`, `develop_file`, `cull`, `mpr`, `library` (counts, progress, search), `jobs`, `accessibility`, `focus`, `dialogs`, `alerts`, `texts` (every label and description line) |

`idle` answers once nothing has drawn for `quiet_ms`, no frame or timer is pending, no tiles
or decodes are on their way and the gallery's visible tiles are in. `focus`, `dialogs` and
`alerts` come from the accessibility tree, which exists only while a screen reader listens.

## 6. Security

* Nothing listens on the network; the socket is reachable only by its user (section 1).
* No command reads a path a client names, beyond what `open` already allows (opening files the
  user can open, through the normal sandboxed decoders). `array-show` reads only bare file
  names inside the private shared folder, and refuses links and anything but regular files.
* No command runs a program. (`open` with `"window":"new"` starts Specular itself.) Hooks
  (section 10) run only programs the user's settings name, and each only after the user
  allowed it in the window.
* Events go only to connections that subscribed, through the same private socket. A slow
  subscriber loses events (and is told how many); it can never hold up the window.
* Every size is bounded: line length, files per `open`, blob size and count, array dimensions,
  overlay shapes and points; blobs are reserved from the memory budget before allocation.
* Requests are parsed without panicking (deeply nested JSON is refused by the parser), and every
  wait has a timeout: no client can stall the window.

## 7. Stability policy

* Within a version, commands, arguments, result members and error codes are never removed or
  changed in meaning. New result members and new optional arguments may be added: clients
  ignore members they do not know, and the viewer ignores request members it does not know.
* New commands come with a new protocol version, listed in `hello`'s `commands`; a viewer keeps
  answering every older version it lists in `protocols`.
* A breaking change needs a new version number, and the old version keeps working for at least
  one major release after it.
* Error messages are for people and may change; codes are stable.

## 8. Examples

```sh
specular --remote next
printf '%s\n' '{"v":2,"cmd":"view-set","zoom":400,"center":[512,384]}' | specular --remote raw
specular ipc mark --flag pick --stars 4 ~/Pictures/IMG_0042.CR3
specular ipc watch marked exported          # EVENT<TAB>FILE per line, until Ctrl+C
specular ipc watch --json | jq .files
specular ipc export "Web" ~/Pictures/*.jpg  # answers the job; `exported` follows
```

```python
import numpy as np, specular
viewer = specular.connect()
viewer.show_array(np.random.rand(256, 256).astype("float32"), name="noise")
viewer.add_overlay([specular.polygon([(10, 10), (100, 10), (60, 80)], color="#ff0000")])
png = viewer.screenshot()
for e in viewer.events(["marked"]):
    if e["mark"]["flag"] == "pick":
        print("picked", e["files"])
```

```rust
let mut c = specular_client::Client::connect_default()?;
for e in c.subscribe(&["exported"])? {
    let e = e?;
    println!("{} {}", e["recipe"], e["outputs"]);
}
```

## 9. Events (version 3)

`subscribe` starts them on the connection (more names can be added later); `unsubscribe`
stops them. The connection still takes requests: answers carry an `id`, events do not.

Every event line has `v` (3), `event`, `seq` (one more for each line the viewer sends, across
all events), `time` (UTC, RFC 3339 with milliseconds), `files`, and `source`: `user` (done in
the window), `ipc` (a client's request) or `hook` (a hook's program; `hook` then names the
hook).

| Event | When | Members besides `files` |
|---|---|---|
| `opened` | An image has been on screen for 300 ms (stepping quickly through a folder tells only where it stopped). | `view`: `viewer`, `reading`, `slideshow` or `develop` |
| `marked` | Picks, rejects, stars or labels changed (one event per mark set; an undo of mixed marks gives one per mark). | `mark`: {`flag`: `pick`, `reject` or `null`, `stars`, `label`: a name or `null`} |
| `keywords` | Keywords were written to photos (after the write succeeded). | `added`, `removed`: the keywords that came and went on any of `files` |
| `exported` | An export with recipes finished. | `recipe` (the first), `recipes`, `outputs` [[source, output]…], `failed` [[source, reason]…], `job` (when a client started it) |
| `imported` | An import finished. | `destination`, `count`; `files` holds the first file at the destination |
| `captured` | A tethered camera sent a photo. | `camera`, `path` |
| `develop-saved` | A Develop edit was written (the write's result, not the request). | `version`, `sidecars` (one per file, `null` where none was written), `sidecar` (for one file), `job` (from `apply-style`) |

* **One act, one line.** Marking 500 photos is one `marked` line. Lists are capped at 10 000
  entries per line (and well under the line limit): a larger act goes out in parts with `part`
  and `parts` (1-based), each numbered; join them by putting each list back together in order.
* **Pings.** A subscriber that heard nothing for a minute gets
  `{"v":3,"event":"ping","seq":N,"time":…}` (`seq`: the last event's). A client that hears
  nothing for longer can take the viewer for gone.
* **Lost events.** Each subscriber has a queue of 1024 lines. When it is full, newer events are
  dropped and counted; once the client has caught up it gets
  `{"v":3,"event":"lost","count":N,…}`. A subscriber that reads nothing for 30 s while events
  wait is disconnected.
* Events are not stored: what happened before `subscribe`, or while no one listened, is gone.
  `subscribe`'s `seq` is the last event numbered before it; every later one is sent (or counted
  lost).

## 10. Hooks

Hooks are programs settings.toml names, run when an event happens; no scripting language or
plugin is involved.

```toml
[[hooks]]
name = "Upload picks"
on = ["marked"]              # event names, or "*"
when = { flag = "pick" }     # optional: members that must be equal ("mark.stars" = 5 also works)
program = "/home/u/bin/upload"
args = ["{files}"]           # the default
timeout = 120                # seconds (default 60; the program is stopped after it)
enabled = true               # Settings › Integration switches it
```

* **Placeholders** in `args`: `{files}` (one argument per file; alone), `{file}` (the first),
  `{dir}`, `{name}`, `{stem}`, `{ext}`, `{uri}` (of the first), `{event}`, `{recipe}`,
  `{outputs}` (one argument per file written; alone), `{stars}`; `{{` and `}}` for braces.
* **Never a shell.** The program is started directly with these arguments; a file name is never
  read as shell syntax. There is no `shell` key (a hook with one is refused): to use a shell,
  name it as `program`.
* **Input.** The event line (as in section 9) on standard input, then end of file;
  `$SPECULAR_EVENT` is the event's name, `$SPECULAR_HOOK` the hook's, `$SPECULAR_SOCKET` the
  viewer's socket. A large act runs the hook once per part.
* **Consent.** The first time a hook would run, the window asks: "Run 'Upload picks'
  (/home/u/bin/upload) when photos are marked?", naming the arguments, with Allow and Not now.
  Allow is kept by a fingerprint of the program, arguments and events (`hooks-trusted.txt` in
  the config folder): a changed hook asks again. Not now holds until the viewer restarts; its
  events are dropped.
* **Limits.** At most 2 runs of a hook at once and 4 in all; later events wait, up to 64 per
  hook, then are dropped (logged). A failure is a quiet note in the window and the log.
* **No loops.** What a hook's program does through the socket (its clients send the hook's
  name, from `$SPECULAR_HOOK`) is reported with `source: "hook"` and runs no hooks.
* Hooks never run in screenshots, benchmarks or other runs without a window, and are not
  imported with "Import settings…". Settings › Integration lists them with an on/off switch
  each and Remove… (which asks first; the program and files stay).
