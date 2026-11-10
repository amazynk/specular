#!/usr/bin/env python3
"""AI masks oracle: our EfficientSAM, IS-Net and UperNet against the authors'
reference implementations. Reference only: run in a venv under the target dir with
torch (CPU), onnxruntime, transformers, numpy and pillow; the EfficientSAM reference code
is a checkout of github.com/yformer/EfficientSAM (Apache-2.0) named by EFFICIENTSAM_SRC.
Never shipped, never committed outputs.

  masks-oracle.py prep OUT MODELS [PHOTOS]
      writes the test set to OUT/img/<id>.rgba (u32 width, u32 height, RGBA8) with its
      ground truth OUT/gt/<id>.<kind>.u8 (generated scenes: subject, sky, object) and the
      clicks OUT/img/<id>.clicks.tsv (x y kind, fractions), then runs the references:
      OUT/ref/<id>.sam.emb.f32, .sam.masks.f32, .sam.iou.f32 (EfficientSAM-Ti, 1024²),
      OUT/ref/<id>.isnet.f32 (1024², sigmoid), OUT/ref/<id>.upernet.f32 (sky probability).
      PHOTOS (optional): a folder of fetch-masks.py photos, prepared the same way without
      ground truth.
  (then: SPECULAR_MASKS_ORACLE=OUT SPECULAR_MASKS_MODELS=MODELS cargo test -p specular-infer
         --test masks_oracle -- --nocapture)
"""

import os
import struct
import sys

import numpy as np

MEAN = np.array([0.485, 0.456, 0.406], np.float32)
STD = np.array([0.229, 0.224, 0.225], np.float32)


def write_rgba(path, rgb):
    h, w, _ = rgb.shape
    a = np.concatenate([rgb, np.full((h, w, 1), 255, np.uint8)], axis=2)
    with open(path, "wb") as f:
        f.write(struct.pack("<II", w, h))
        f.write(a.tobytes())


def read_rgba(path):
    with open(path, "rb") as f:
        w, h = struct.unpack("<II", f.read(8))
        return np.frombuffer(f.read(), np.uint8).reshape(h, w, 4)[..., :3]


def scenes(n, size=1024):
    """Generated test scenes with exact ground truth: a sky gradient above a horizon, a
    textured ground, and one object (ellipse or rounded box) standing on it."""
    out = []
    for i in range(n):
        rng = np.random.default_rng(1000 + i)
        yy, xx = np.mgrid[0:size, 0:size].astype(np.float32) / size
        horizon = 0.35 + 0.25 * rng.random() + 0.05 * np.sin(xx * (3 + 4 * rng.random()) * np.pi)
        sky = yy < horizon
        top = np.array([0.25, 0.45, 0.85]) * (0.8 + 0.2 * rng.random())
        low = np.array([0.75, 0.85, 0.97])
        t = np.clip(yy / np.maximum(horizon, 1e-3), 0, 1)[..., None]
        img = top * (1 - t) + low * t
        ground = np.array([0.3, 0.45, 0.2]) if i % 2 == 0 else np.array([0.55, 0.45, 0.3])
        noise = rng.normal(0, 0.06, (size // 8, size // 8, 3))
        noise = np.kron(noise, np.ones((8, 8, 1)))
        img = np.where(sky[..., None], img, ground + noise)
        cx, cy = 0.3 + 0.4 * rng.random(), horizon.mean() + 0.1 + 0.15 * rng.random()
        rx, ry = 0.08 + 0.1 * rng.random(), 0.12 + 0.12 * rng.random()
        if i % 3 == 0:
            obj = ((xx - cx) / rx) ** 2 + ((yy - cy) / ry) ** 2 < 1
        else:
            obj = (np.abs(xx - cx) < rx) & (np.abs(yy - cy) < ry)
        colour = rng.random(3) * 0.6 + 0.3 if i % 2 else np.array([0.85, 0.2, 0.15])
        shade = 0.85 + 0.15 * (1 - (yy - cy + ry) / (2 * ry))[..., None]
        img = np.where(obj[..., None], colour * shade, img)
        rgb = (np.clip(img, 0, 1) * 255 + 0.5).astype(np.uint8)
        sky = sky & ~obj
        clicks = [(cx, cy, "pos")]
        if i % 2:
            clicks.append((cx + rx * 0.5, cy - ry * 0.4, "pos"))
        if i % 4 == 3:
            clicks = [(cx - rx, cy - ry, "tl"), (cx + rx, cy + ry, "br")]
        out.append((f"gen{i:02}", rgb, {"subject": obj, "sky": sky, "object": obj}, clicks))
    return out


def resize(rgb, w, h):
    from PIL import Image

    return np.asarray(Image.fromarray(rgb).resize((w, h), Image.BILINEAR))


def sam_ref(models, rgb, clicks):
    import torch

    sys.path.insert(0, os.environ["EFFICIENTSAM_SRC"])
    from efficient_sam.efficient_sam import build_efficient_sam

    m = build_efficient_sam(192, 3, None).eval()
    sd = torch.load(os.path.join(models, "efficient_sam_vitt.pt"), map_location="cpu",
                    weights_only=True)
    m.load_state_dict(sd["model"])
    x = torch.from_numpy(rgb.astype(np.float32) / 255).permute(2, 0, 1)[None]
    h, w = rgb.shape[:2]
    lab = {"pos": 1, "neg": 0, "tl": 2, "br": 3}
    pts = torch.tensor([[[[c[0] * w, c[1] * h] for c in clicks]]], dtype=torch.float32)
    labels = torch.tensor([[[lab[c[2]] for c in clicks]]], dtype=torch.float32)
    with torch.no_grad():
        emb = m.get_image_embeddings(x)
        masks, iou = m.predict_masks(emb, pts, labels, True, h, w, -1, -1)
    return emb.numpy(), masks.numpy(), iou.numpy()


def isnet_ref(models, rgb):
    import onnxruntime as ort

    s = ort.InferenceSession(os.path.join(models, "isnet-general-use.onnx"),
                             providers=["CPUExecutionProvider"])
    x = rgb.astype(np.float32) / 255
    x = (x - 0.5) / 1.0
    x = x.transpose(2, 0, 1)[None]
    out = s.run(None, {s.get_inputs()[0].name: x})[0]
    return out[0, 0]


def upernet_ref(models, rgb):
    import torch
    from transformers import UperNetConfig, UperNetForSemanticSegmentation
    from safetensors.torch import load_file

    cfg = UperNetConfig.from_json_file(os.path.join(models, "upernet-config.json"))
    m = UperNetForSemanticSegmentation(cfg).eval()
    sd = load_file(os.path.join(models, "upernet-convnext-tiny.safetensors"))
    missing, unexpected = m.load_state_dict(sd, strict=False)
    assert not missing and not unexpected, (missing, unexpected)
    x = (rgb.astype(np.float32) / 255 - MEAN) / STD
    x = np.ascontiguousarray(x.transpose(2, 0, 1))
    with torch.no_grad():
        feats = m.backbone.forward_with_filtered_kwargs(torch.from_numpy(x)[None]).feature_maps
        logits = m.decode_head(feats)[0]
    return x, torch.softmax(logits, 0).numpy()


def prep(out, models, photos=None):
    for d in ("img", "gt", "ref"):
        os.makedirs(os.path.join(out, d), exist_ok=True)
    items = scenes(8)
    if photos:
        from PIL import Image

        for name in sorted(os.listdir(photos)):
            if not name.lower().endswith((".jpg", ".jpeg", ".png")):
                continue
            im = Image.open(os.path.join(photos, name)).convert("RGB")
            rgb = np.asarray(im.resize((1024, 1024), Image.BILINEAR))
            items.append(("photo-" + os.path.splitext(name)[0], rgb, {}, [(0.5, 0.55, "pos")]))
    for ident, rgb, gts, clicks in items:
        write_rgba(os.path.join(out, "img", ident + ".rgba"), rgb)
        with open(os.path.join(out, "img", ident + ".clicks.tsv"), "w") as f:
            for c in clicks:
                f.write(f"{c[0]}\t{c[1]}\t{c[2]}\n")
        for k, g in gts.items():
            g.astype(np.uint8).tofile(os.path.join(out, "gt", f"{ident}.{k}.u8"))
        if os.environ.get("ONLY") == "upernet":
            r = os.path.join(out, "ref", ident)
            side = int(os.environ.get("UPERNET_SIDE", "384"))
            x, up = upernet_ref(models, resize(rgb, side, side))
            x.astype("<f4").tofile(r + ".upernet.in.f32")
            up[2].astype("<f4").tofile(r + ".upernet.f32")
            continue
        emb, masks, iou = sam_ref(models, rgb, clicks)
        r = os.path.join(out, "ref", ident)
        emb.astype("<f4").tofile(r + ".sam.emb.f32")
        masks.astype("<f4").tofile(r + ".sam.masks.f32")
        iou.astype("<f4").tofile(r + ".sam.iou.f32")
        isnet_ref(models, rgb).astype("<f4").tofile(r + ".isnet.f32")
        side = int(os.environ.get("UPERNET_SIDE", "384"))
        x, up = upernet_ref(models, resize(rgb, side, side))
        x.astype("<f4").tofile(r + ".upernet.in.f32")
        up[2].astype("<f4").tofile(r + ".upernet.f32")
        print(ident, "iou", np.round(iou.ravel(), 3))


if __name__ == "__main__":
    if len(sys.argv) >= 4 and sys.argv[1] == "prep":
        prep(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else None)
    else:
        print(__doc__)
        sys.exit(2)
