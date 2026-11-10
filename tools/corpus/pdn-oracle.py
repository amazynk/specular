"""Paint.NET oracle: each layer of every .pdn under a folder read by pypdn (an independent
reader, MIT), written beside it as <file>.layers.json and <file>.layers.rgba (the layers'
straight RGBA bytes, bottom first). The specular-pdn corpus test compares against these.

Usage: python3 -I tools/corpus/pdn-oracle.py DIR   (with a venv that has pypdn and numpy:
  python3 -m venv target/editors/venv && PIP_NO_CACHE_DIR=1 target/editors/venv/bin/pip install pypdn numpy)
"""
import json
import os
import sys

import pypdn

root = sys.argv[1]
ok = failed = 0
for dirpath, _, files in os.walk(root):
    for f in sorted(files):
        if not f.lower().endswith('.pdn'):
            continue
        p = os.path.join(dirpath, f)
        try:
            img = pypdn.read(p)
        except Exception as e:  # pypdn refuses or fails: no reference
            print(f'{p}: pypdn failed: {e}')
            failed += 1
            continue
        meta = {'width': img.width, 'height': img.height, 'layers': []}
        with open(p + '.layers.rgba', 'wb') as out:
            for l in img.layers:
                meta['layers'].append({
                    'name': l.name, 'visible': bool(l.visible), 'opacity': int(l.opacity),
                    'blend': int(l.blendMode), 'background': bool(l.isBackground),
                })
                out.write(l.image.astype('uint8').tobytes())
        with open(p + '.layers.json', 'w') as out:
            json.dump(meta, out)
        ok += 1
print(f'pypdn oracle: {ok} files, {failed} failed')
