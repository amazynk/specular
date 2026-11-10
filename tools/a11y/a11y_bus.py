#!/usr/bin/env python3
"""A stand-in for at-spi-bus-launcher on the private session bus of atspi-check.sh.

Owns org.a11y.Bus, answers GetAddress with the private accessibility bus and reports
org.a11y.Status IsEnabled and ScreenReaderEnabled as true, which is what a running screen
reader sets and what makes AccessKit (and GTK) start exposing their trees. The real launcher
starts its bus through dbus-broker-launch, which needs systemd, so it cannot run in a private
session.

    a11y_bus.py <accessibility bus address>
"""

import sys

import gi

gi.require_version("Gio", "2.0")
from gi.repository import Gio, GLib  # noqa: E402

XML = """
<node>
  <interface name="org.a11y.Bus">
    <method name="GetAddress"><arg type="s" direction="out"/></method>
  </interface>
  <interface name="org.a11y.Status">
    <property name="IsEnabled" type="b" access="readwrite"/>
    <property name="ScreenReaderEnabled" type="b" access="readwrite"/>
  </interface>
</node>
"""


def main():
    address = sys.argv[1]
    state = {"IsEnabled": True, "ScreenReaderEnabled": True}
    info = Gio.DBusNodeInfo.new_for_xml(XML)
    loop = GLib.MainLoop()

    def call(conn, sender, path, iface, method, params, inv):
        if method == "GetAddress":
            inv.return_value(GLib.Variant("(s)", (address,)))
        else:
            inv.return_dbus_error("org.freedesktop.DBus.Error.UnknownMethod", method)

    def get(conn, sender, path, iface, prop):
        return GLib.Variant("b", state[prop])

    def set_(conn, sender, path, iface, prop, value):
        state[prop] = value.unpack()
        conn.emit_signal(None, path, "org.freedesktop.DBus.Properties", "PropertiesChanged",
                         GLib.Variant("(sa{sv}as)", (iface, {prop: value}, [])))
        return True

    def acquired(conn, name):
        for iface in info.interfaces:
            conn.register_object("/org/a11y/bus", iface, call, get, set_)

    Gio.bus_own_name(Gio.BusType.SESSION, "org.a11y.Bus", Gio.BusNameOwnerFlags.NONE,
                     acquired, lambda *a: print("org.a11y.Bus owned", flush=True),
                     lambda *a: loop.quit())
    loop.run()


if __name__ == "__main__":
    main()
