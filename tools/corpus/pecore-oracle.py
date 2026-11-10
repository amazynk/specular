#!/usr/bin/env python3
"""Reference outputs of PE Core S16 for the specular-infer tests (crates/specular-infer/tests/pecore.rs).

Usage: python -I tools/corpus/pecore-oracle.py DIR CHECKSET
  DIR      what tools/corpus/fetch-pecore.sh fetched (open_clip_model.safetensors, kodak/)
  CHECKSET crates/specular-infer/tests/data/siglip-checkset.tsv

The reference is open_clip's PE-Core-S-16-384 (its own text tower and tokenizer, timm's
`vit_pe_core_small_patch16_384` image tower with 2-D RoPE) on timm's safetensors copy of
Meta's weights: an implementation and a weights file independent of ours, which reads
Meta's .pt. Needs torch (CPU), open_clip_torch, timm, safetensors, pillow and numpy in a venv
under target/ (TORCH_HOME, HF_HOME and XDG_CACHE_HOME pointed under target/ too; nothing is
downloaded: HF_HUB_OFFLINE=1). Writes DIR/ref/:

  tokens.tsv   hex(UTF-8 text) TAB 32 ids: the reference tokenizer (start, ids, end, zeros)
  text.tsv     hex(text) TAB the text embedding (unit length), check-set queries
  images.tsv   kodimNN TAB the image embedding (unit length), all 24 photos
  pixels/      kodimNN.f32: the reference's [3, 384, 384] input (bilinear squash, [-1, 1])
  rgba/        kodimNN-WxH.rgba: the photo as RGBA8 (what the app feeds the graph)
  logit.txt    exp(logit_scale)

and prints the check set's recall with the reference embeddings, per language.
"""

import os
import random
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")

import open_clip
import torch
from PIL import Image

NAME = "PE-Core-S-16-384"
CONTEXT = 32

EDGE = [
    "", " ", "  dog  ", "dog\ncat", "tab\there", "Hello World", "ÉCOLE", "a" * 200,
    "a photo of a dog playing in the snow next to a red car parked outside a small wooden "
    "cabin in the mountains at sunset with pink clouds",
    "2024-10-25 12:34", "$100 & 50%", "C++ / C#", "über", "naïve café", "don't stop",
    "it's 5 o'clock", "x  y   z", "東京 tower", "Привет мир", "emoji 🐶🏖️",
]


def hexs(s):
    return s.encode("utf-8").hex()


def stress(n, seed):
    rng = random.Random(seed)
    pool = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJ0123456789 .,!?-'\"()&/:;àéüßç"
    words = ["the", "a", "photo", "of", "dog", "beach", "red", "car", "sunset", "mountain",
             "children", "playing", "boats", "harbour", "flower", "parrot", "lighthouse"]
    out = []
    for _ in range(n):
        if rng.random() < 0.5:
            s = " ".join(rng.choice(words) for _ in range(rng.randint(1, 12)))
        else:
            s = "".join(rng.choice(pool) for _ in range(rng.randint(1, 60)))
        out.append(s)
    return out


def main():
    root, checkset = sys.argv[1], sys.argv[2]
    out = os.path.join(root, "ref")
    os.makedirs(os.path.join(out, "pixels"), exist_ok=True)
    os.makedirs(os.path.join(out, "rgba"), exist_ok=True)
    # The preprocessing of open_clip's PE config (timm/PE-Core-S-16-384 open_clip_config.json),
    # given here because a local weights file brings no config: squash, bilinear, [-1, 1].
    model, _, preprocess = open_clip.create_model_and_transforms(
        NAME, pretrained=os.path.join(root, "open_clip_model.safetensors"),
        image_mean=(0.5, 0.5, 0.5), image_std=(0.5, 0.5, 0.5),
        image_interpolation="bilinear", image_resize_mode="squash")
    print(preprocess)
    model.eval()
    tok = open_clip.get_tokenizer(NAME)

    queries = []
    for line in open(checkset, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        lang, want, q = line.rstrip("\n").split("\t")
        queries.append((lang, [int(x) for x in want.split(",")], q))

    texts = [q for _, _, q in queries] + EDGE + stress(2000, 7)
    with open(os.path.join(out, "tokens.tsv"), "w") as f:
        for t in texts:
            ids = tok([t])[0].tolist()
            assert len(ids) == CONTEXT
            f.write(f"{hexs(t)}\t{' '.join(map(str, ids))}\n")
    print(f"tokens: {len(texts)} texts")
    with open(os.path.join(out, "logit.txt"), "w") as f:
        f.write(f"{model.logit_scale.exp().item()!r}\n")

    with torch.no_grad():
        names = [f"kodim{i:02d}" for i in range(1, 25)]
        img_e = []
        with open(os.path.join(out, "images.tsv"), "w") as f:
            for n in names:
                im = Image.open(os.path.join(root, "kodak", f"{n}.png")).convert("RGB")
                with open(os.path.join(out, "rgba", f"{n}-{im.width}x{im.height}.rgba"), "wb") as r:
                    r.write(im.convert("RGBA").tobytes())
                px = preprocess(im)[None]
                px.numpy().astype("<f4").tofile(os.path.join(out, "pixels", f"{n}.f32"))
                e = model.encode_image(px, normalize=True)[0]
                img_e.append(e)
                f.write(n + "\t" + " ".join(f"{x:.9g}" for x in e.tolist()) + "\n")
        img_e = torch.stack(img_e)
        hits1 = hits3 = 0
        per_lang = {}
        with open(os.path.join(out, "text.tsv"), "w") as f:
            for lang, want, q in queries:
                e = model.encode_text(tok([q]), normalize=True)[0]
                f.write(hexs(q) + "\t" + " ".join(f"{x:.9g}" for x in e.tolist()) + "\n")
                rank = (img_e @ e).argsort(descending=True).tolist()
                top1 = rank[0] + 1 in want
                top3 = any(r + 1 in want for r in rank[:3])
                hits1 += top1
                hits3 += top3
                h = per_lang.setdefault(lang, [0, 0, 0])
                h[0] += top1
                h[1] += top3
                h[2] += 1
    n = len(queries)
    print(f"reference recall@1 {hits1}/{n}, recall@3 {hits3}/{n}")
    for lang, (a, b, c) in per_lang.items():
        print(f"  {lang}: @1 {a}/{c}, @3 {b}/{c}")


if __name__ == "__main__":
    main()
