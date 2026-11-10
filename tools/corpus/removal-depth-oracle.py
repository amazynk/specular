#!/usr/bin/env python3
"""Object removal and depth oracle: our Big-LaMa and Depth Anything V2 Small against the reference
implementations. Reference only: run in a venv under the target dir with torch
(CPU), transformers, safetensors, numpy and pillow. The LaMa reference is the authors'
`saicinpainting/training/modules/ffc.py` (github.com/advimman/lama, Apache-2.0) importable
from LAMA_SRC; weights are read from safetensors only. Never shipped, outputs never
committed.

  removal-depth-oracle.py prep OUT MODELS PHOTOS
      MODELS holds big-lama.safetensors and da2-small.safetensors (+ da2-config.json).
      PHOTOS is a fetch-depth.py folder (CC0 / CC BY). For each photo, at most 768 px long:
      OUT/img/<id>.rgba (u32 width, u32 height, RGBA8);
      depth: OUT/ref/<id>.da2.in.f32 (the reference processor's pixel values, with the size
      in OUT/ref/<id>.da2.size: "w h"), OUT/ref/<id>.da2.f32 (its predicted depth);
      inpainting: a generated hole OUT/img/<id>.hole.u8 (0/1, the photo's size) and, on a
      crop around it, OUT/ref/<id>.lama.in.f32 ([1, 4, H, W] as the reference feeds it, size
      in OUT/ref/<id>.lama.size: "x y w h"), OUT/ref/<id>.lama.f32 ([1, 3, H, W]).
  (then: SPECULAR_REMOVAL_DEPTH_ORACLE=OUT SPECULAR_REMOVAL_DEPTH_MODELS=MODELS
         cargo test -p specular-infer --test removal_depth_oracle -- --nocapture)
"""

import json
import os
import struct
import sys

import numpy as np


def write_rgba(path, rgb):
    h, w, _ = rgb.shape
    a = np.concatenate([rgb, np.full((h, w, 1), 255, np.uint8)], axis=2)
    with open(path, "wb") as f:
        f.write(struct.pack("<II", w, h))
        f.write(a.tobytes())


def load_photo(path, long=768):
    from PIL import Image
    im = Image.open(path).convert("RGB")
    s = long / max(im.size)
    if s < 1:
        im = im.resize((max(1, round(im.size[0] * s)), max(1, round(im.size[1] * s))),
                       Image.BICUBIC)
    return np.asarray(im, np.uint8)


def hole(h, w, rng):
    """A brush-like hole: a few thick strokes in one area, like a removed object."""
    m = np.zeros((h, w), np.uint8)
    cx, cy = rng.uniform(0.3, 0.7) * w, rng.uniform(0.3, 0.7) * h
    r = rng.uniform(0.03, 0.06) * max(w, h)
    yy, xx = np.mgrid[0:h, 0:w]
    for _ in range(3):
        x0, y0 = cx + rng.normal() * r, cy + rng.normal() * r
        x1, y1 = cx + rng.normal() * r, cy + rng.normal() * r
        for t in np.linspace(0, 1, 24):
            px, py = x0 + (x1 - x0) * t, y0 + (y1 - y0) * t
            m[(xx - px) ** 2 + (yy - py) ** 2 <= (0.5 * r) ** 2] = 1
    return m


def crop_box(m, step=8):
    ys, xs = np.nonzero(m)
    h, w = m.shape
    x0, x1, y0, y1 = xs.min(), xs.max() + 1, ys.min(), ys.max() + 1
    side = max(x1 - x0, y1 - y0) * 3
    side = min(max(64, side), 512, w, h) // step * step
    cx, cy = (x0 + x1) // 2, (y0 + y1) // 2
    x = int(np.clip(cx - side // 2, 0, w - side))
    y = int(np.clip(cy - side // 2, 0, h - side))
    return x, y, side, side


def lama_model(models):
    import torch
    from safetensors.torch import load_file
    sys.path.insert(0, os.environ["LAMA_SRC"])
    from saicinpainting.training.modules.ffc import FFCResNetGenerator
    g = FFCResNetGenerator(
        4, 3, ngf=64, n_downsampling=3, n_blocks=18, add_out_act="sigmoid",
        init_conv_kwargs=dict(ratio_gin=0, ratio_gout=0, enable_lfu=False),
        downsample_conv_kwargs=dict(ratio_gin=0, ratio_gout=0, enable_lfu=False),
        resnet_conv_kwargs=dict(ratio_gin=0.75, ratio_gout=0.75, enable_lfu=False))
    sd = load_file(os.path.join(models, "big-lama.safetensors"))
    sd = {k[len("generator."):]: v for k, v in sd.items() if k.startswith("generator.")}
    g.load_state_dict(sd, strict=True)
    g.eval()
    torch.set_grad_enabled(False)
    return g


def depth_model(models):
    import torch
    from transformers import DepthAnythingConfig, DepthAnythingForDepthEstimation
    from safetensors.torch import load_file
    cfg = DepthAnythingConfig(**json.load(open(os.path.join(models, "da2-config.json"))))
    m = DepthAnythingForDepthEstimation(cfg)
    sd = load_file(os.path.join(models, "da2-small.safetensors"))
    # Newer transformers name the attention projections q/k/v/o_proj (the checkpoint keeps
    # the older names their loader maps on the fly).
    ren = {".attention.attention.query.": ".attention.q_proj.",
           ".attention.attention.key.": ".attention.k_proj.",
           ".attention.attention.value.": ".attention.v_proj.",
           ".attention.output.dense.": ".attention.o_proj."}
    want = set(m.state_dict())
    out = {}
    for k, v in sd.items():
        if k not in want:
            for a, b in ren.items():
                k = k.replace(a, b)
        out[k] = v
    m.load_state_dict(out, strict=True)
    m.eval()
    torch.set_grad_enabled(False)
    return m


def depth_inputs(rgb):
    from transformers import DPTImageProcessor
    p = DPTImageProcessor(do_resize=True, size={"height": 518, "width": 518},
                          keep_aspect_ratio=True, ensure_multiple_of=14, resample=3,
                          do_rescale=True, rescale_factor=1 / 255, do_normalize=True,
                          image_mean=[0.485, 0.456, 0.406], image_std=[0.229, 0.224, 0.225])
    return p(images=rgb, return_tensors="pt")["pixel_values"]


def prep(out, models, photos):
    import torch
    for d in ("img", "ref"):
        os.makedirs(os.path.join(out, d), exist_ok=True)
    lama = lama_model(models)
    da = depth_model(models)
    rng = np.random.default_rng(27)
    names = sorted(n for n in os.listdir(photos) if n.lower().endswith(".jpg"))
    for n in names:
        i = os.path.splitext(n)[0]
        rgb = load_photo(os.path.join(photos, n))
        h, w, _ = rgb.shape
        write_rgba(os.path.join(out, "img", i + ".rgba"), rgb)
        # Depth.
        px = depth_inputs(rgb)
        dep = da(pixel_values=px).predicted_depth
        px.numpy().astype("<f4").tofile(os.path.join(out, "ref", i + ".da2.in.f32"))
        with open(os.path.join(out, "ref", i + ".da2.size"), "w") as f:
            f.write(f"{px.shape[3]} {px.shape[2]}\n")
        dep.numpy().astype("<f4").tofile(os.path.join(out, "ref", i + ".da2.f32"))
        # Inpainting on a crop around a generated hole.
        m = hole(h, w, rng)
        m.tofile(os.path.join(out, "img", i + ".hole.u8"))
        x, y, cw, ch = crop_box(m)
        img = rgb[y:y + ch, x:x + cw].astype(np.float32) / 255
        mk = m[y:y + ch, x:x + cw].astype(np.float32)
        t = np.concatenate([(img * (1 - mk[..., None])).transpose(2, 0, 1), mk[None]], 0)[None]
        t = np.ascontiguousarray(t, np.float32)
        res = lama(torch.from_numpy(t)).numpy()
        t.astype("<f4").tofile(os.path.join(out, "ref", i + ".lama.in.f32"))
        with open(os.path.join(out, "ref", i + ".lama.size"), "w") as f:
            f.write(f"{x} {y} {cw} {ch}\n")
        res.astype("<f4").tofile(os.path.join(out, "ref", i + ".lama.f32"))
        print(f"{i}: {w}x{h}, depth {px.shape[3]}x{px.shape[2]}, crop {cw}x{ch}", flush=True)


def main():
    if len(sys.argv) < 5 or sys.argv[1] != "prep":
        sys.exit(__doc__)
    prep(*sys.argv[2:5])


if __name__ == "__main__":
    main()
