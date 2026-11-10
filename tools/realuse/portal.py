"""A FileChooser portal for the real-use runs: answers the app's open, save and folder dialogs
(rfd and our own open dialog go through org.freedesktop.portal.FileChooser) from a queue the
run fills, on the private session bus only (tools/realuse/realuse.sh starts it).

The portal runs on a thread of its own with its own bus connection and GLib main context, so
a run that blocks on the remote control still gets its dialogs answered. Nothing in the app
changes: this is the code path a desktop's portal takes.
"""

import os
import threading
import urllib.parse

from gi.repository import Gio, GLib

XML = """<node>
<interface name="org.freedesktop.portal.FileChooser">
  <method name="OpenFile">
    <arg type="s" name="parent_window" direction="in"/>
    <arg type="s" name="title" direction="in"/>
    <arg type="a{sv}" name="options" direction="in"/>
    <arg type="o" name="handle" direction="out"/>
  </method>
  <method name="SaveFile">
    <arg type="s" name="parent_window" direction="in"/>
    <arg type="s" name="title" direction="in"/>
    <arg type="a{sv}" name="options" direction="in"/>
    <arg type="o" name="handle" direction="out"/>
  </method>
  <method name="SaveFiles">
    <arg type="s" name="parent_window" direction="in"/>
    <arg type="s" name="title" direction="in"/>
    <arg type="a{sv}" name="options" direction="in"/>
    <arg type="o" name="handle" direction="out"/>
  </method>
  <property name="version" type="u" access="read"/>
</interface>
</node>"""


def uri(path):
    return "file://" + urllib.parse.quote(os.path.abspath(path))


class Portal:
    """`answer(paths)` queues the answer to the next dialog (a list of paths; [] cancels);
    `calls` lists the dialogs asked for: (method, title, options)."""

    def __init__(self):
        self.answers = []
        self.calls = []
        self.lock = threading.Lock()
        self.ready = threading.Event()
        self.error = None
        self.thread = threading.Thread(target=self._run, name="portal", daemon=True)
        self.thread.start()
        self.ready.wait(10)

    def answer(self, paths):
        with self.lock:
            self.answers.append([os.path.abspath(p) for p in paths])

    def clear(self):
        with self.lock:
            self.answers.clear()

    def pending(self):
        with self.lock:
            return len(self.answers)

    def taken(self):
        with self.lock:
            return list(self.calls)

    def _run(self):
        try:
            ctx = self.ctx = GLib.MainContext.new()
            ctx.push_thread_default()
            addr = os.environ["DBUS_SESSION_BUS_ADDRESS"]
            flags = (Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT
                     | Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION)
            self.conn = Gio.DBusConnection.new_for_address_sync(addr, flags, None, None)
            node = Gio.DBusNodeInfo.new_for_xml(XML)
            self.conn.register_object("/org/freedesktop/portal/desktop", node.interfaces[0],
                                      self._call, self._prop, None)
            r = self.conn.call_sync("org.freedesktop.DBus", "/org/freedesktop/DBus",
                                    "org.freedesktop.DBus", "RequestName",
                                    GLib.Variant("(su)", ("org.freedesktop.portal.Desktop", 4)),
                                    None, 0, -1, None)
            if r.unpack()[0] != 1:
                self.error = f"could not own the portal name ({r.unpack()[0]})"
            self.ready.set()
            GLib.MainLoop.new(ctx, False).run()
        except Exception as e:  # noqa: BLE001 - reported by the run
            self.error = str(e)
            self.ready.set()

    def _prop(self, conn, sender, path, iface, name):
        return GLib.Variant("u", 3) if name == "version" else None

    def _call(self, conn, sender, path, iface, method, params, inv):
        parent, title, options = params.unpack()
        token = options.get("handle_token", "t")
        handle = "/org/freedesktop/portal/desktop/request/%s/%s" % (
            sender[1:].replace(".", "_"), token)
        with self.lock:
            self.calls.append((method, title, {k: str(v) for k, v in options.items()}))
            paths = self.answers.pop(0) if self.answers else []
        inv.return_value(GLib.Variant("(o)", (handle,)))
        code = 0 if paths else 1
        results = {"uris": GLib.Variant("as", [uri(p) for p in paths])} if paths else {}

        def respond():
            conn.emit_signal(sender, handle, "org.freedesktop.portal.Request", "Response",
                             GLib.Variant("(ua{sv})", (code, results)))
            return False

        # After the method's answer, as a portal does once the user chose.
        src = GLib.timeout_source_new(100)
        src.set_callback(lambda *_: respond())
        src.attach(self.ctx)
