# specular: control Specular from Python

A small client for the viewer's local socket (protocol version 3, [`docs/ipc.md`](../../docs/ipc.md)).
Pure Python 3.8+, standard library only. NumPy is used when you pass NumPy arrays.

Install it into your environment with `pip install clients/python`, or put
`clients/python` on `PYTHONPATH`.

## Use

```python
import specular

viewer = specular.connect()          # the first running viewer, or $SPECULAR_SOCKET

viewer.open("scan.dcm", view={"zoom": 200, "window": (40, 400)})
viewer.next()
viewer.set_view(zoom="fit", colormap="viridis", channel="all")
print(viewer.view())                 # zoom, centre, window, colour map, channel
print(viewer.metadata()["calibration"])

viewer.show_array(labels, name="labels")   # any NumPy array, any strides
viewer.add_overlay([
    specular.polygon([(10, 10), (90, 10), (50, 70)], color="#ff0000", label="cell 1"),
    specular.text(5, 5, "hello"),
], layer="cells")

rois = viewer.rois()                 # the user's measurements as GeoJSON
png = viewer.screenshot("view.png")  # PNG bytes of the window
```

Coordinates are image pixels of the stored image, as NumPy indexes it: x is the column,
y the row.

Errors raise `specular.SpecularError` with a `code` from the protocol
(`bad-arguments`, `not-found`, `mode`, …) and a message.

Large arrays go over the socket in chunks. `via="file"` writes them into the viewer's
private shared folder instead; the viewer takes the file.

## Examples

- `examples/show_array.py`: show a NumPy array and outline its bright spots.
- `examples/batch_screenshots.py`: step through a folder and save a PNG of each view.

## Tests

`cargo test -p specular --bin specular python_client` runs `tests/` against a viewer's
socket server, in a virtual environment under the target folder.
