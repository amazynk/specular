"""A virtual pointer in the private headless sway (wlr-virtual-pointer-unstable-v1), spoken
over the Wayland wire protocol directly: the headless session has no input devices, so this is
how a run presses, drags and lets go. Never used outside tools/realuse/realuse.sh."""

import os
import socket
import struct
import time

BTN_LEFT = 0x110


def _string(s):
    b = s.encode() + b"\0"
    return struct.pack("<I", len(b)) + b + b"\0" * (-len(b) % 4)


class VPointer:
    def __init__(self, env, extent=(1280, 800)):
        path = os.path.join(env["XDG_RUNTIME_DIR"], env["WAYLAND_DISPLAY"])
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(path)
        self.extent = extent
        self.next_id = 2
        self.buf = b""
        registry = self._new()
        self._send(1, 1, struct.pack("<I", registry))  # wl_display.get_registry
        globals_ = self._roundtrip(registry)
        found = [(n, v) for n, i, v in globals_ if i == "zwlr_virtual_pointer_manager_v1"]
        if not found:
            raise RuntimeError("the compositor has no virtual pointer manager")
        name, version = found[0]
        manager = self._new()
        iface = "zwlr_virtual_pointer_manager_v1"
        self._send(registry, 0, struct.pack("<I", name) + _string(iface)
                   + struct.pack("<II", min(version, 1), manager))
        self.ptr = self._new()
        # create_virtual_pointer(seat: null, id)
        self._send(manager, 0, struct.pack("<II", 0, self.ptr))
        self._roundtrip(None)

    def _new(self):
        i = self.next_id
        self.next_id += 1
        return i

    def _send(self, obj, opcode, payload):
        size = 8 + len(payload)
        self.sock.sendall(struct.pack("<II", obj, (size << 16) | opcode) + payload)

    def _roundtrip(self, registry):
        """wl_display.sync and read to its done event; the registry's globals seen."""
        cb = self._new()
        self._send(1, 0, struct.pack("<I", cb))
        globals_ = []
        while True:
            while len(self.buf) < 8:
                self.buf += self.sock.recv(65536)
            obj, word = struct.unpack("<II", self.buf[:8])
            size, opcode = word >> 16, word & 0xFFFF
            while len(self.buf) < size:
                self.buf += self.sock.recv(65536)
            body, self.buf = self.buf[8:size], self.buf[size:]
            if obj == 1 and opcode == 0:
                raise RuntimeError("wayland error: %r" % body)
            if obj == registry and opcode == 0:
                name, n = struct.unpack("<II", body[:8])
                iface = body[8:8 + n - 1].decode()
                (ver,) = struct.unpack("<I", body[8 + n + (-n % 4):12 + n + (-n % 4)])
                globals_.append((name, iface, ver))
            if obj == cb and opcode == 0:
                return globals_

    def _ms(self):
        return int(time.monotonic() * 1000) & 0xFFFFFFFF

    def move_to(self, x, y):
        w, h = self.extent
        self._send(self.ptr, 1, struct.pack("<IIIII", self._ms(), int(x), int(y), w, h))
        self._send(self.ptr, 4, b"")  # frame

    def button(self, pressed, button=BTN_LEFT):
        self._send(self.ptr, 2, struct.pack("<III", self._ms(), button, 1 if pressed else 0))
        self._send(self.ptr, 4, b"")

    def close(self):
        try:
            self._send(self.ptr, 8, b"")  # destroy
        except OSError:
            pass
        self.sock.close()
