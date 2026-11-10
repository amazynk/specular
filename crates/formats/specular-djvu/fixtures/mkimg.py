"""Synthetic test images for the DjVu fixtures (PPM/PGM/PBM written by hand)."""
import math, sys, os

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

def ppm(name, w, h, f):
    with open(os.path.join(out, name), 'wb') as fh:
        fh.write(b'P6 %d %d 255\n' % (w, h))
        fh.write(bytes(v for y in range(h) for x in range(w) for v in f(x, y)))

def pgm(name, w, h, f):
    with open(os.path.join(out, name), 'wb') as fh:
        fh.write(b'P5 %d %d 255\n' % (w, h))
        fh.write(bytes(f(x, y) for y in range(h) for x in range(w)))

def clamp(v):
    return max(0, min(255, int(v)))

# Smooth colour photo with an edge and a disc.
def photo(x, y):
    r = 128 + 100 * math.sin(x / 13.0)
    g = 128 + 100 * math.cos(y / 9.0)
    b = 255 * x / 211
    if (x - 140) ** 2 + (y - 60) ** 2 < 900:
        r, g, b = 250, 240, 20
    if y > 120:
        r, g, b = r * 0.3, g * 0.3, b * 0.3
    return clamp(r), clamp(g), clamp(b)

ppm('photo.ppm', 211, 157, photo)
pgm('gray.pgm', 190, 133, lambda x, y: clamp(128 + 90 * math.sin((x + y) / 11.0) + (60 if x > 120 else 0)))

# Palettised compound page: white paper, a few coloured glyph-like blobs and bars.
def compound(x, y):
    if 20 <= y < 30 and 10 <= x < 280:
        return (200, 0, 0)
    if 50 <= y < 90 and (x // 12) % 2 == 0 and 10 <= x < 200:
        return (0, 0, 160)
    if (x - 240) ** 2 + (y - 120) ** 2 < 400:
        return (0, 120, 0)
    if 140 <= y < 170 and 20 <= x < 120 and ((x + y) // 5) % 2 == 0:
        return (0, 0, 0)
    return (255, 255, 255)

ppm('compound.ppm', 300, 190, compound)
