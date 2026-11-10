#!/usr/bin/env python3
"""AI denoise oracle: our NAFNet against the authors' reference implementation,
and the test set for PSNR/SSIM against Develop's classic denoise. Reference only: run in a
venv under the target dir with torch (CPU), safetensors, numpy and pillow; the
reference code is NAFNet_arch.py and arch_util.py from github.com/megvii-research/NAFNet
(MIT) in the folder named by NAFNET_SRC (imports of basicsr are stubbed). Never shipped,
outputs never committed. Weights are read only from safetensors (the f32 copies the Rust
oracle test writes from the downloaded checkpoints without running code).

  denoise-oracle.py prep OUT PHOTOS
      Centre crops (512×384) of each photo (fetch-denoise.py), linearised (sRGB curve),
      with Poisson–Gaussian noise at three ISO-like levels (the denoiser's noise model, white = 1):
        OUT/lin/<id>.clean.f32, OUT/lin/<id>.noisy.f32   (linear RGB)
        OUT/img/<id>.in.f32                              (the network input: noisy,
                                                          clamped, sRGB-encoded)
      Files: u32 width, u32 height, then three planes of f32 (R, G, B).
  denoise-oracle.py ref OUT MODELS
      Runs the reference NAFNet-SIDD width 32 (and 64 when MODELS has it) on every input:
        OUT/ref/<id>.w32.f32 (and .w64.f32), whole image in one pass; and FFDNet (deepinv's
        module, BSD-3, with KAIR's colour weights) at a noise level per ISO level:
        OUT/ref/<id>.ffd.f32.
  (then: SPECULAR_DENOISE_ORACLE=OUT SPECULAR_DENOISE_MODELS=MODELS cargo test
         -p specular-infer --test denoise_oracle -- --nocapture)
"""

import os
import struct
import sys
import types

import numpy as np

# The ISO-like levels: per channel variance = a·x + b (R, G, B).
LEVELS = {
    "iso400": ([1.6e-4, 1.0e-4, 1.3e-4], [8.0e-8, 4.0e-8, 6.0e-8]),
    "iso1600": ([6.4e-4, 4.0e-4, 5.2e-4], [1.2e-6, 6.0e-7, 9.0e-7]),
    "iso6400": ([2.56e-3, 1.6e-3, 2.08e-3], [2.0e-5, 1.0e-5, 1.5e-5]),
}
CROP = (512, 384)


def write(path, chw):
    c, h, w = chw.shape
    with open(path, "wb") as f:
        f.write(struct.pack("<II", w, h))
        f.write(np.ascontiguousarray(chw, np.float32).tobytes())


def read(path):
    with open(path, "rb") as f:
        w, h = struct.unpack("<II", f.read(8))
        return np.frombuffer(f.read(), np.float32).reshape(3, h, w)


def srgb_to_linear(v):
    return np.where(v <= 0.04045, v / 12.92, ((v + 0.055) / 1.055) ** 2.4)


def linear_to_srgb(v):
    v = np.clip(v, 0.0, 1.0)
    return np.where(v <= 0.0031308, v * 12.92, 1.055 * v ** (1 / 2.4) - 0.055)


def prep(out, photos):
    from PIL import Image

    for d in ("lin", "img", "ref"):
        os.makedirs(os.path.join(out, d), exist_ok=True)
    rng = np.random.default_rng(24)
    names = sorted(n for n in os.listdir(photos) if n.lower().endswith(".jpg"))
    for name in names:
        im = Image.open(os.path.join(photos, name)).convert("RGB")
        w, h = im.size
        cw, ch = CROP
        if w < cw or h < ch:
            continue
        x0, y0 = (w - cw) // 2, (h - ch) // 2
        a = np.asarray(im.crop((x0, y0, x0 + cw, y0 + ch)), np.float32) / 255.0
        clean = srgb_to_linear(a).transpose(2, 0, 1).astype(np.float32)
        stem = os.path.splitext(name)[0]
        for lvl, (ga, gb) in LEVELS.items():
            ga = np.array(ga, np.float64)[:, None, None]
            gb = np.array(gb, np.float64)[:, None, None]
            x = clean.astype(np.float64)
            # Poisson in units of a, plus Gaussian read noise.
            p = rng.poisson(np.maximum(x, 0) / ga) * ga
            noisy = (p + rng.normal(0.0, np.sqrt(gb), x.shape)).astype(np.float32)
            id_ = f"{stem}-{lvl}"
            write(os.path.join(out, "lin", id_ + ".clean.f32"), clean)
            write(os.path.join(out, "lin", id_ + ".noisy.f32"), noisy)
            write(os.path.join(out, "img", id_ + ".in.f32"), linear_to_srgb(noisy))
        print(name, flush=True)


def reference(src):
    # The reference imports basicsr's package layout; stub the two modules it needs.
    sys.path.insert(0, src)
    for m in ("basicsr", "basicsr.models", "basicsr.models.archs", "basicsr.utils"):
        sys.modules[m] = types.ModuleType(m)
    sys.modules["basicsr.utils"].get_root_logger = lambda *a, **k: None
    import arch_util  # noqa: E402

    sys.modules["basicsr.models.archs.arch_util"] = arch_util
    local = types.ModuleType("basicsr.models.archs.local_arch")

    class LocalBase:  # unused here: NAFNetLocal is not run
        pass

    local.Local_Base = LocalBase
    sys.modules["basicsr.models.archs.local_arch"] = local
    import NAFNet_arch  # noqa: E402

    return NAFNet_arch.NAFNet


# The noise level FFDNet is told per level (units of the input's range).
FFD_SIGMA = {"iso400": 0.01, "iso1600": 0.02, "iso6400": 0.05}


def ffdnet_reference():
    """deepinv's FFDNet (BSD-3), its module loaded alone: the package's __init__ needs
    torchvision, which the oracle does not."""
    import importlib.util

    import torch

    spec = importlib.util.find_spec("deepinv")
    path = os.path.join(os.path.dirname(spec.origin), "models", "ffdnet.py")
    for m in ("deepinv", "deepinv.models"):
        sys.modules.setdefault(m, types.ModuleType(m))
    base = types.ModuleType("deepinv.models.base")
    base.Denoiser = torch.nn.Module
    drunet = types.ModuleType("deepinv.models.drunet")
    drunet.weights_init_drunet = lambda m: None
    utils = types.ModuleType("deepinv.models.utils")
    utils.get_weights_url = lambda **k: ""
    utils.load_state_dict_from_url = lambda *a, **k: {}
    for name, mod in (("base", base), ("drunet", drunet), ("utils", utils)):
        sys.modules["deepinv.models." + name] = mod
    s = importlib.util.spec_from_file_location("deepinv.models.ffdnet", path)
    mod = importlib.util.module_from_spec(s)
    s.loader.exec_module(mod)
    return mod.FFDNet


def ref_ffdnet(out, models):
    import torch
    from safetensors.torch import load_file

    path = os.path.join(models, "ffdnet_color_deepinv.f32.safetensors")
    if not os.path.exists(path):
        return
    FFDNet = ffdnet_reference()
    net = FFDNet(n_conv_layers=12, nf=96, img_channels=3, pretrained=None)
    net.load_state_dict(load_file(path), strict=True)
    net.eval()
    for name in sorted(os.listdir(os.path.join(out, "img"))):
        id_ = name[: -len(".in.f32")]
        dst = os.path.join(out, "ref", f"{id_}.ffd.f32")
        if os.path.exists(dst):
            continue
        x = torch.from_numpy(read(os.path.join(out, "img", name)).copy())[None]
        sigma = FFD_SIGMA[id_.rsplit("-", 1)[1]]
        with torch.no_grad():
            y = net(x, sigma)[0].numpy()
        write(dst, y)
        print(id_, "ffdnet", flush=True)


def ref(out, models):
    import torch
    from safetensors.torch import load_file

    ref_ffdnet(out, models)
    NAFNet = reference(os.environ["NAFNET_SRC"])
    nets = {}
    for tag, width in (("w32", 32), ("w64", 64)):
        path = os.path.join(models, f"NAFNet-SIDD-width{width}.f32.safetensors")
        if not os.path.exists(path):
            continue
        net = NAFNet(img_channel=3, width=width, middle_blk_num=12,
                     enc_blk_nums=[2, 2, 4, 8], dec_blk_nums=[2, 2, 2, 2])
        net.load_state_dict(load_file(path), strict=True)
        nets[tag] = net.eval()
    if not nets:
        sys.exit(f"no NAFNet-SIDD-width*.f32.safetensors in {models}")
    torch.set_num_threads(max(1, (os.cpu_count() or 2) // 2))
    for name in sorted(os.listdir(os.path.join(out, "img"))):
        x = torch.from_numpy(read(os.path.join(out, "img", name)).copy())[None]
        id_ = name[: -len(".in.f32")]
        for tag, net in nets.items():
            dst = os.path.join(out, "ref", f"{id_}.{tag}.f32")
            if os.path.exists(dst):
                continue
            with torch.no_grad():
                y = net(x)[0].numpy()
            write(dst, y)
        print(id_, flush=True)


def main():
    if len(sys.argv) < 4:
        sys.exit(__doc__)
    cmd, out, arg = sys.argv[1], os.path.abspath(sys.argv[2]), sys.argv[3]
    if cmd == "prep":
        prep(out, arg)
    elif cmd == "ref":
        ref(out, arg)
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
