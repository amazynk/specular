"""Tests of the Python client against a viewer's socket server.

Run by the viewer's own test suite (app/specular/src/remote/tests_python.rs), which starts the
server with a stand-in window, points SPECULAR_SOCKET at it and runs these in a virtual
environment under the target folder. Skipped when SPECULAR_SOCKET is not set.
"""

import json
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))

import specular  # noqa: E402

try:
    import numpy as np
except ImportError:  # NumPy is optional.
    np = None

SOCKET = os.environ.get("SPECULAR_SOCKET")


@unittest.skipUnless(SOCKET, "needs SPECULAR_SOCKET (run through cargo test)")
class ClientTests(unittest.TestCase):
    def setUp(self):
        self.v = specular.connect(SOCKET)

    def tearDown(self):
        self.v.close()

    def test_hello_and_capabilities(self):
        self.assertEqual(self.v.version, 3)
        self.assertEqual(self.v.info["protocols"], [1, 2, 3])
        self.assertEqual(list(specular.EVENTS), self.v.info["events"])
        self.assertIn("viridis", self.v.capabilities["colormaps"])
        self.assertIn("array-show", self.v.info["commands"])

    def test_commands_arrive_whole(self):
        # The stand-in window answers with the request it got.
        r = self.v.set_view(zoom=250, center=(10, 20.5), window=(40, 400), colormap="viridis", channel=1)
        self.assertEqual(r["cmd"], "view-set")
        self.assertIn("Percent(250.0)", r["debug"])
        self.assertIn("center: 40.0, width: 400.0", r["debug"])
        self.assertIn("Index(1)", r["debug"])
        self.assertIn("Fit", self.v.set_view(zoom="fit", window="auto")["debug"])
        self.assertEqual(self.v.view()["cmd"], "view-get")
        self.assertEqual(self.v.metadata()["cmd"], "metadata")
        self.assertEqual(self.v.rois()["cmd"], "rois")
        self.assertIn("Json", self.v.rois(format="json")["debug"])
        self.assertEqual(self.v.action("zoom-in")["cmd"], "action")
        self.assertEqual(self.v.state()["cmd"], "state")
        for step in (self.v.next, self.v.prev, self.v.first, self.v.last):
            self.assertEqual(step(), 0)
        self.assertEqual(self.v.goto(4), 4)
        r = self.v.call("overlay-add", layer="cells", shapes=[
            specular.point(1, 2, color="#ff0000"),
            specular.line((0, 0), (5, 5), width=3),
            specular.polyline([(0, 0), (1, 1), (2, 0)]),
            specular.polygon([(0, 0), (4, 0), (4, 4)], label="cell 1"),
            specular.rect((1, 1), (3, 3), color="#00ff0080"),
            specular.text(5, 5, "hello"),
        ])
        self.assertEqual(r["cmd"], "overlay-add")
        self.assertIn("Polygon", r["debug"])
        self.assertIn("cell 1", r["debug"])
        self.assertEqual(self.v.call("overlay-list")["cmd"], "overlay-list")
        self.assertEqual(self.v.call("overlay-clear")["cmd"], "overlay-clear")
        self.assertEqual(self.v.call("view-command", lang="python")["cmd"], "view-command")
        self.assertEqual(self.v.open(__file__, view={"zoom": 50})["opened"], 1)

    def test_errors(self):
        with self.assertRaises(specular.SpecularError) as e:
            self.v.call("rm -rf")
        self.assertEqual(e.exception.code, "unknown-command")
        with self.assertRaises(specular.SpecularError) as e:
            self.v.set_view()
        self.assertEqual(e.exception.code, "bad-arguments")
        with self.assertRaises(specular.SpecularError) as e:
            self.v.call("array-show", file="../x", shape=[1], dtype="u1")
        self.assertEqual(e.exception.code, "bad-arguments")
        with self.assertRaises(specular.SpecularError) as e:
            self.v.call("blob-new", bytes=1 << 40)
        self.assertEqual(e.exception.code, "bad-arguments")

    def test_blobs(self):
        data = bytes(range(256)) * 5000
        blob = self.v.upload(data)
        self.assertEqual(self.v.download(blob, len(data)), data)

    def test_buffers_without_numpy(self):
        raw = bytes(range(12))
        r = self.v.show_buffer(raw, (3, 4), "|u1", name="plain")
        self.assertTrue(r["file"].endswith("plain.npy"))
        with open(r["file"], "rb") as f:
            self.assertTrue(f.read().endswith(raw))
        r = self.v.show_buffer(raw, (12,), "|u1", via="file")
        with open(r["file"], "rb") as f:
            self.assertTrue(f.read().endswith(raw))

    @unittest.skipUnless(np is not None, "needs NumPy")
    def test_numpy_arrays(self):
        a = np.arange(24, dtype=np.float32).reshape(4, 6)
        for arr in (a, np.asfortranarray(a), a[::-1, ::2], a.astype(">i4")):
            r = self.v.show_array(arr, name="arr")
            back = np.load(r["file"])
            np.testing.assert_array_equal(back, arr)
        r = self.v.show_array(a, via="file")
        np.testing.assert_array_equal(np.load(r["file"]), a)
        with self.assertRaises(specular.SpecularError):
            self.v.show_array(np.zeros(3, dtype=np.complex64))

    def test_screenshot(self):
        png = self.v.screenshot()
        self.assertTrue(png.startswith(b"\x89PNG\r\n\x1a\n"))

    def test_version_1_fallback(self):
        v1 = specular.connect(SOCKET)
        v1.version = 1
        self.assertEqual(v1.call("hello")["protocol"], 1)
        with self.assertRaises(specular.SpecularError) as e:
            v1.call("view-get")
        self.assertEqual(e.exception.code, "version")
        v1.close()

    def test_events(self):
        # The stand-in window tells `marked` for each `mark` it is asked.
        w = specular.connect(SOCKET)
        seq = w.subscribe(["marked"])
        self.v.mark("/p/a.jpg", "/p/b c;d.jpg", flag="pick", stars=4)
        e = w.next_event()
        self.assertEqual(e["event"], "marked")
        self.assertEqual(e["files"], ["/p/a.jpg", "/p/b c;d.jpg"])
        self.assertEqual(e["mark"], {"flag": "pick", "stars": 4, "label": None})
        self.assertEqual(e["source"], "ipc")
        self.assertGreater(e["seq"], seq)
        self.assertNotIn("id", e)
        # Requests still work on a subscribed connection; events met meanwhile are kept.
        self.v.mark("/p/c.jpg", stars=1)
        self.assertEqual(w.state()["cmd"], "state")
        gen = w.events(["marked"])
        e2 = next(gen)
        self.assertEqual(e2["files"], ["/p/c.jpg"])
        self.assertGreater(e2["seq"], e["seq"])
        self.assertFalse(w.unsubscribe()["subscribed"])
        w.close()
        # Unknown events are refused; the new commands arrive whole.
        with self.assertRaises(specular.SpecularError) as err:
            self.v.subscribe(["clicked"])
        self.assertEqual(err.exception.code, "unknown-event")
        self.assertEqual(self.v.select("/p/a.jpg", add=True)["cmd"], "select")
        self.assertIn("Warm", self.v.apply_style("Warm", "/p/a.jpg")["debug"])
        self.assertIn("Web", self.v.export("Web", "/p/a.jpg")["debug"])

    def test_json_is_one_line(self):
        # Newlines inside strings are escaped: one request per line.
        r = self.v.call("action", name="a\nb")
        self.assertEqual(json.loads(json.dumps(r))["cmd"], "action")


if __name__ == "__main__":
    unittest.main()
