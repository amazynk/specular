"""The connection and the commands. See docs/ipc.md for the protocol."""

import base64
import json
import os
import socket
import sys
import tempfile

PROTOCOL = 3
MAX_LINE = 1 << 20
# A subscriber hears at least a ping a minute; longer silence means the viewer is gone.
EVENT_SILENCE = 150.0
EVENTS = ("opened", "marked", "keywords", "exported", "imported", "captured", "develop-saved")
MAX_CHUNK = 512 * 1024
_DTYPES = {"b1", "u1", "i1", "u2", "i2", "u4", "i4", "u8", "i8", "f4", "f8"}


class SpecularError(Exception):
    """A request the viewer refused: `code` is the protocol's error code (docs/ipc.md),
    `message` says why. Local failures use the codes "io" and "parse"."""

    def __init__(self, code, message):
        super().__init__("%s (%s)" % (message, code))
        self.code = code
        self.message = message


def default_socket():
    """The first viewer's socket: $SPECULAR_SOCKET, else the usual places; None if none."""
    env = os.environ.get("SPECULAR_SOCKET")
    if env:
        return env
    dirs = []
    home = os.path.expanduser("~")
    if sys.platform == "win32":
        local = os.environ.get("LOCALAPPDATA")
        if local:
            dirs += [os.path.join(local, "Specular", "cache", "run"), os.path.join(local, "Specular", "run")]
    else:
        uid = os.getuid()
        runtime = os.environ.get("XDG_RUNTIME_DIR")
        if runtime:
            dirs.append(os.path.join(runtime, "specular"))
        if sys.platform == "darwin":
            dirs.append(os.path.join(os.environ.get("TMPDIR", "/tmp"), "specular-%d" % uid))
            dirs.append(os.path.join(home, "Library", "Caches", "Specular", "run"))
        else:
            cache = os.environ.get("XDG_CACHE_HOME") or os.path.join(home, ".cache")
            dirs.append(os.path.join(cache, "specular", "run"))
        dirs.append(os.path.join(tempfile.gettempdir(), "specular-%d" % uid))
        dirs.append("/tmp/specular-%d" % uid)
    for d in dirs:
        p = os.path.join(d, "specular.sock")
        if os.path.exists(p):
            return p
    return None


def _is_event(v):
    return isinstance(v, dict) and isinstance(v.get("event"), str) and "id" not in v


def _paths(paths):
    return [os.path.abspath(os.fspath(p)) for p in paths]


def connect(path=None, timeout=30.0):
    """Connects to the viewer at `path` (default: default_socket())."""
    path = path or default_socket()
    if not path:
        raise SpecularError("io", "no running viewer (start Specular, or set SPECULAR_SOCKET)")
    return Viewer(path, timeout)


def _shape(kind, points, color, width, label):
    s = {"kind": kind, "points": [[float(x), float(y)] for x, y in points]}
    if color is not None:
        s["color"] = color
    if width is not None:
        s["width"] = float(width)
    if label:
        s["text"] = label
    return s


def point(x, y, color=None, width=None, label=None):
    """A point (drawn as a small cross), in image pixels."""
    return _shape("point", [(x, y)], color, width, label)


def line(a, b, color=None, width=None, label=None):
    return _shape("line", [a, b], color, width, label)


def polyline(points, color=None, width=None, label=None):
    return _shape("polyline", points, color, width, label)


def polygon(points, color=None, width=None, label=None):
    return _shape("polygon", points, color, width, label)


def rect(a, b, color=None, width=None, label=None):
    """A rectangle from two opposite corners."""
    return _shape("rect", [a, b], color, width, label)


def text(x, y, string, color=None):
    """Text with its top-left corner at x, y."""
    return _shape("text", [(x, y)], color, None, string)


def _view_args(zoom=None, center=None, window=None, colormap=None, channel=None):
    a = {}
    if zoom is not None:
        a["zoom"] = zoom if zoom == "fit" else float(zoom)
    if center is not None:
        a["center"] = [float(center[0]), float(center[1])]
    if window is not None:
        if window == "auto":
            a["window"] = "auto"
        elif isinstance(window, dict):
            a["window"] = {"center": float(window["center"]), "width": float(window["width"])}
        else:
            a["window"] = {"center": float(window[0]), "width": float(window[1])}
    if colormap is not None:
        a["colormap"] = colormap
    if channel is not None:
        a["channel"] = channel
    return a


class Viewer:
    """One connection to a viewer. Requests are answered one at a time, in order."""

    def __init__(self, path, timeout=30.0):
        if not hasattr(socket, "AF_UNIX"):
            raise SpecularError("io", "this Python has no AF_UNIX sockets")
        self._sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self._sock.settimeout(timeout)
        try:
            self._sock.connect(path)
        except OSError as e:
            self._sock.close()
            raise SpecularError("io", "no viewer at %s (%s)" % (path, e)) from None
        self._file = self._sock.makefile("rb")
        self._id = 0
        self._timeout = timeout
        # Events that arrived while an answer was awaited, oldest first.
        self._events = []
        self.version = PROTOCOL
        # Where the viewer keeps a key next to its socket (Windows), and when a hook started
        # this program: both go with the first hello.
        intro = {}
        try:
            with open(path + ".key", "r", encoding="ascii") as f:
                intro["key"] = f.read().strip()
        except OSError:
            pass
        if os.environ.get("SPECULAR_HOOK"):
            intro["hook"] = os.environ["SPECULAR_HOOK"]
        for v in (PROTOCOL, 2, 1):
            self.version = v
            try:
                self.info = self.call("hello", **intro)
                break
            except SpecularError as e:
                # An older viewer: the commands of its version still work.
                if e.code != "version" or v == 1:
                    raise
        self.capabilities = self.info.get("capabilities", {})

    # ----- plumbing -----------------------------------------------------------------------

    def close(self):
        try:
            self._file.close()
        finally:
            self._sock.close()

    def __enter__(self):
        return self

    def __exit__(self, *exc):
        self.close()

    def call(self, cmd, **args):
        """Sends one request and returns its result (a dict); raises SpecularError."""
        self._id += 1
        req = {"v": self.version, "id": self._id, "cmd": cmd}
        req.update({k: v for k, v in args.items() if v is not None})
        data = json.dumps(req, separators=(",", ":"), ensure_ascii=False).encode("utf-8")
        if len(data) > MAX_LINE:
            raise SpecularError("too-long", "request longer than %d bytes" % MAX_LINE)
        try:
            self._sock.sendall(data + b"\n")
        except OSError as e:
            raise SpecularError("io", str(e)) from None
        while True:
            v = self._read()
            if _is_event(v):
                self._events.append(v)
                del self._events[:-4096]
                continue
            break
        if v.get("ok") is True and v.get("id") == self._id:
            return v.get("result", {})
        err = v.get("error") or {}
        raise SpecularError(err.get("code", "parse"), err.get("message", "malformed response"))

    def _read(self):
        try:
            line = self._file.readline(MAX_LINE + 2)
        except OSError as e:
            raise SpecularError("io", str(e)) from None
        if not line:
            raise SpecularError("io", "the viewer closed the connection")
        if not line.endswith(b"\n"):
            raise SpecularError("too-long", "response too long")
        try:
            return json.loads(line)
        except ValueError as e:
            raise SpecularError("parse", str(e)) from None

    # ----- events (protocol version 3) ------------------------------------------------------

    def subscribe(self, events=None):
        """Starts receiving `events` (names from EVENTS; None: all). Returns the sequence
        number the events will follow."""
        names = list(events) if events else ["*"]
        seq = self.call("subscribe", events=names)["seq"]
        self._sock.settimeout(EVENT_SILENCE)
        return seq

    def unsubscribe(self, events=None):
        """Stops receiving `events` (None: all)."""
        r = self.call("unsubscribe", events=list(events) if events else None)
        if not r.get("subscribed"):
            self._sock.settimeout(self._timeout)
        return r

    def next_event(self):
        """The next event (a dict), waiting for it; pings are left out."""
        while True:
            v = self._events.pop(0) if self._events else self._read()
            if _is_event(v) and v.get("event") != "ping":
                return v

    def events(self, names=None):
        """Subscribes and yields events as they come, until the viewer goes:

            for e in viewer.events(["marked"]):
                print(e["files"], e["mark"])

        A large act comes in parts ("part", "parts"); "lost" says how many events were
        dropped because they were read too slowly."""
        self.subscribe(names)
        while True:
            try:
                yield self.next_event()
            except SpecularError as e:
                if e.code == "io" and "closed" in e.message:
                    return
                raise

    # ----- the gallery, Develop and export (protocol version 3) ----------------------------

    def select(self, *paths, add=False):
        """Selects these files in the gallery (add=True: as well as the selection)."""
        return self.call("select", files=_paths(paths), add=True if add else None)

    def mark(self, *paths, flag=None, stars=None, label=None):
        """Marks files: flag "pick", "reject" or "none"; stars 0-5; label "red", "yellow",
        "green", "blue" or "none". One undo step in the viewer."""
        return self.call("mark", files=_paths(paths), flag=flag, stars=stars, label=label)

    def apply_style(self, style, *paths):
        """Applies a Develop style by name, in the background; returns its job."""
        return self.call("apply-style", files=_paths(paths), style=style)

    def export(self, recipe, *paths):
        """Exports with a recipe by name, in the background; returns its job. An
        "exported" event with the same "job" says when it is done."""
        return self.call("export", files=_paths(paths), recipe=recipe)

    # ----- files and navigation -----------------------------------------------------------

    def open(self, *paths, view=None, new_window=False):
        """Opens files (absolute or relative to here). `view`: a dict of set_view's
        arguments, applied once the first file shows."""
        files = [os.path.abspath(os.fspath(p)) for p in paths]
        return self.call(
            "open",
            files=files,
            window="new" if new_window else None,
            view=_view_args(**view) if view else None,
        )

    def next(self):
        return self.call("navigate", to="next")["index"]

    def prev(self):
        return self.call("navigate", to="prev")["index"]

    def first(self):
        return self.call("navigate", to="first")["index"]

    def last(self):
        return self.call("navigate", to="last")["index"]

    def goto(self, index):
        """Shows the listing's image at 0-based `index`."""
        return self.call("navigate", index=int(index))["index"]

    def state(self):
        return self.call("state")

    def action(self, name):
        """Runs a palette action by id ("zoom-in", "rotate-left", "zoom-to:200", …)."""
        return self.call("action", name=name)

    # ----- view, metadata -----------------------------------------------------------------

    def view(self):
        """The view: zoom, center, window, colormap, channel and more (docs/ipc.md)."""
        return self.call("view-get")

    def set_view(self, zoom=None, center=None, window=None, colormap=None, channel=None):
        """zoom: "fit" or percent; center: (x, y) image pixels; window: "auto" or
        (center, width); colormap: an id or "none"; channel: "all" or a number."""
        return self.call("view-set", **_view_args(zoom, center, window, colormap, channel))

    def metadata(self):
        return self.call("metadata")

    def view_command(self, lang="python"):
        """A snippet ("python" or "shell") that reopens this file with this view."""
        return self.call("view-command", lang=lang)["text"]

    # ----- blobs, arrays, screenshots -----------------------------------------------------

    def upload(self, data):
        """Copies bytes into a new blob of this connection; returns its id."""
        data = memoryview(data).cast("B")
        blob = self.call("blob-new", bytes=len(data))["blob"]
        try:
            for off in range(0, len(data), MAX_CHUNK):
                chunk = base64.b64encode(data[off : off + MAX_CHUNK]).decode("ascii")
                self.call("blob-write", blob=blob, offset=off, data=chunk)
        except BaseException:
            self.call("blob-free", blob=blob)
            raise
        return blob

    def download(self, blob, size):
        """Reads a blob of `size` bytes and frees it."""
        out = bytearray()
        while len(out) < size:
            r = self.call("blob-read", blob=blob, offset=len(out), length=min(MAX_CHUNK, size - len(out)))
            piece = base64.b64decode(r["data"])
            if not piece:
                raise SpecularError("parse", "blob shorter than announced")
            out += piece
        self.call("blob-free", blob=blob)
        return bytes(out)

    def show_buffer(self, data, shape, dtype, strides=None, name=None, via="blob"):
        """Shows raw array data: `dtype` a NumPy type string ("<f4", "|u1", …), `strides`
        in bytes (None: C order). via="file" goes through the viewer's shared folder."""
        if via == "file" and self.capabilities.get("shared_dir"):
            fd, path = tempfile.mkstemp(dir=self.capabilities["shared_dir"], prefix="py-", suffix=".raw")
            with os.fdopen(fd, "wb") as f:
                f.write(memoryview(data).cast("B"))
            try:
                return self.call(
                    "array-show", file=os.path.basename(path), shape=list(shape),
                    dtype=dtype, strides=strides, name=name,
                )
            finally:
                if os.path.exists(path):
                    os.unlink(path)
        blob = self.upload(data)
        try:
            return self.call(
                "array-show", blob=blob, shape=list(shape), dtype=dtype,
                strides=None if strides is None else list(strides), name=name,
            )
        except SpecularError:
            try:
                self.call("blob-free", blob=blob)
            except SpecularError:
                pass
            raise

    def show_array(self, array, name=None, via="blob"):
        """Shows a NumPy array (2-D image, stack, channels) as Specular shows a .npy file."""
        import numpy as np

        a = np.asarray(array)
        if a.dtype == np.float16:
            a = a.astype(np.float32)
        kind = a.dtype.str.lstrip("<>|=")
        if kind not in _DTYPES:
            raise SpecularError("bad-arguments", "dtype %s is not supported" % a.dtype)
        if a.ndim == 0:
            a = a.reshape(1)
        if a.flags.c_contiguous:
            return self.show_buffer(a.tobytes(order="C"), a.shape, a.dtype.str, None, name, via)
        if a.flags.f_contiguous:
            # Sent as laid out in memory; the strides tell the viewer how.
            return self.show_buffer(a.tobytes(order="F"), a.shape, a.dtype.str, a.strides, name, via)
        a = np.ascontiguousarray(a)
        return self.show_buffer(a.tobytes(), a.shape, a.dtype.str, None, name, via)

    def screenshot(self, path=None):
        """PNG bytes of what the window shows; also written to `path` when given."""
        r = self.call("screenshot")
        png = self.download(r["blob"], r["bytes"])
        if path:
            with open(path, "wb") as f:
                f.write(png)
        return png

    # ----- overlays and measurements --------------------------------------------------------

    def add_overlay(self, shapes, layer="default"):
        """Draws shapes (see point(), polygon(), …) over the current image."""
        shapes = list(shapes)
        n = int(self.capabilities.get("max_shapes", 10000))
        added = 0
        for i in range(0, len(shapes), n):
            added += self.call("overlay-add", layer=layer, shapes=shapes[i : i + n])["added"]
        return added

    def clear_overlay(self, layer=None):
        return self.call("overlay-clear", layer=layer)["removed"]

    def overlays(self):
        return self.call("overlay-list")["layers"]

    def rois(self, format="geojson"):
        """The user's measurements: a GeoJSON FeatureCollection, or format="json"."""
        return self.call("rois", format=format)
