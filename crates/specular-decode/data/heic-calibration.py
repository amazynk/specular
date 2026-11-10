# Generator of heic-calibration.heic and jpeg-calibration.jpg (see
# crates/specular-decode/src/calibrate.rs):
#   python3 -I heic-calibration.py 2048 1024 sample.png
#   heif-enc -q 50 --cut-tiles 512 -t 256 -o heic-calibration.heic sample.png
#   python3 -I heic-calibration.py 2368 1776 sample.ppm
#   cjpeg -quality 85 -sample 2x2 -optimize -outfile jpeg-calibration.jpg sample.ppm
# The output format follows the extension: PNG, or binary PPM (cjpeg's input).
# Procedural photo-like test image: fractal value noise, gradients, soft shapes and edges.
import numpy as np, zlib, struct, sys
w, h = int(sys.argv[1]), int(sys.argv[2]); out = sys.argv[3]
rng = np.random.default_rng(12345)
def noise(scale):
    gw, gh = w // scale + 2, h // scale + 2
    g = rng.random((gh, gw, 3))
    ys = np.arange(h) / scale; xs = np.arange(w) / scale
    y0 = ys.astype(int); x0 = xs.astype(int); fy = (ys - y0)[:, None, None]; fx = (xs - x0)[None, :, None]
    fy = fy * fy * (3 - 2 * fy); fx = fx * fx * (3 - 2 * fx)
    a = g[y0][:, x0]; b = g[y0][:, x0 + 1]; c = g[y0 + 1][:, x0]; d = g[y0 + 1][:, x0 + 1]
    return (a * (1 - fx) + b * fx) * (1 - fy) + (c * (1 - fx) + d * fx) * fy
img = np.zeros((h, w, 3))
amp = 0.5
for s in [256, 128, 64, 32, 16, 8, 4, 2]:
    img += amp * (noise(s) - 0.5); amp *= 0.62
yy, xx = np.mgrid[0:h, 0:w]
img += np.stack([xx / w, yy / h, 1 - xx / w], -1) * 0.5
for i in range(40):
    cx, cy, r = rng.random() * w, rng.random() * h, 20 + rng.random() * 200
    m = ((xx - cx) ** 2 + (yy - cy) ** 2) < r * r
    img[m] = img[m] * 0.6 + rng.random(3) * 0.4
img += rng.normal(0, 0.015, img.shape)
px = (np.clip(img + 0.25, 0, 1) * 255).astype(np.uint8)
if out.endswith('.ppm'):
    open(out, 'wb').write(b'P6\n%d %d\n255\n' % (w, h) + px.tobytes()); sys.exit()
raw = b''.join(b'\0' + px[y].tobytes() for y in range(h))
def chunk(t, d): return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
open(out, 'wb').write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))
