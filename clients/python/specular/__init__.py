"""Control a running Specular viewer from Python.

Talks to the viewer's local socket (protocol version 3, see docs/ipc.md). Standard library
only; NumPy is used when you pass NumPy arrays.

    import specular
    viewer = specular.connect()
    viewer.next()
    viewer.set_view(zoom=200, window=(40, 400), colormap="viridis")
    viewer.show_array(image, name="result")
    viewer.add_overlay([specular.polygon([(10, 10), (90, 10), (50, 70)], color="#ff0000")])
    png = viewer.screenshot()
    for e in viewer.events(["marked"]):
        print(e["files"], e["mark"])
"""

from ._client import (
    EVENTS,
    PROTOCOL,
    SpecularError,
    Viewer,
    connect,
    default_socket,
    line,
    point,
    polygon,
    polyline,
    rect,
    text,
)

__all__ = [
    "EVENTS",
    "PROTOCOL",
    "SpecularError",
    "Viewer",
    "connect",
    "default_socket",
    "line",
    "point",
    "polygon",
    "polyline",
    "rect",
    "text",
]

__version__ = "3.0.0"
