"""Steps through the images of a folder and saves a PNG of each view.

    python batch_screenshots.py FOLDER OUT_FOLDER
"""

import os
import sys

import specular

folder, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
files = sorted(os.path.join(folder, f) for f in os.listdir(folder))

with specular.connect() as viewer:
    viewer.open(*files)
    count = viewer.state()["count"]
    for i in range(count):
        viewer.goto(i)
        viewer.set_view(zoom="fit")
        name = os.path.basename(viewer.state()["file"] or "image-%d" % i)
        viewer.screenshot(os.path.join(out, name + ".png"))
        print("saved", name)
