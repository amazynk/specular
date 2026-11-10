"""Shows a NumPy array in Specular and outlines its bright spots.

    python show_array.py
"""

import numpy as np

import specular

# A test image: a few Gaussian spots on noise.
rng = np.random.default_rng(1)
y, x = np.mgrid[0:256, 0:256]
spots = [(60, 70), (180, 120), (100, 200)]
image = rng.normal(0, 0.05, (256, 256)).astype(np.float32)
for cy, cx in spots:
    image += np.exp(-((x - cx) ** 2 + (y - cy) ** 2) / (2 * 8.0**2)).astype(np.float32)

with specular.connect() as viewer:
    viewer.show_array(image, name="spots")
    viewer.set_view(zoom="fit", colormap="viridis")
    shapes = [
        specular.rect((cx - 16, cy - 16), (cx + 16, cy + 16), color="#ff3030", label="spot %d" % (i + 1))
        for i, (cy, cx) in enumerate(spots)
    ]
    viewer.add_overlay(shapes, layer="spots")
