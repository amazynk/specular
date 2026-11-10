#!/usr/bin/env python3
"""Reference outputs of SigLIP 2 for the specular-infer tests (crates/specular-infer/tests/siglip.rs).

Usage: python -I tools/corpus/siglip2-oracle.py DIR MODEL CHECKSET
  DIR      what tools/corpus/fetch-siglip2.sh fetched (tokenizer files, kodak/, MODEL/)
  MODEL    siglip2-b16-256 or siglip2-b32-256
  CHECKSET crates/specular-infer/tests/data/siglip-checkset.tsv

Needs torch (CPU), transformers, sentencepiece, pillow and numpy, in a venv under target/
(HF_HOME, XDG_CACHE_HOME and TORCH_HOME pointed under target/ too; nothing is downloaded:
HF_HUB_OFFLINE=1). Writes DIR/ref-MODEL/:

  tokens.tsv   hex(UTF-8 text) TAB 64 ids: the Hugging Face tokenizer (fast) on the
               lower-cased text, padded to 64 (the check set, edge cases, random strings)
  text.tsv     hex(text) TAB the text embedding (unit length), check-set queries
  images.tsv   kodimNN TAB the image embedding (unit length), all 24 photos
  pixels/      kodimNN.f32: the processor's [3, 256, 256] input, raw little-endian f32
  rgba/        kodimNN-WxH.rgba: the photo as RGBA8 (what the app feeds the graph)
  logit.txt    exp(logit_scale) and logit_bias

and prints how sentencepiece itself agrees with the Hugging Face tokenizer, and the
check set's recall with the reference embeddings.
"""

import os
import random
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")

import numpy as np
import sentencepiece as spm
import torch
from PIL import Image
from transformers import GemmaTokenizerFast, SiglipImageProcessor, SiglipModel

CONTEXT = 64

EDGE = [
    "", " ", "  dog  ", "dog\ncat", "tab\there", "Hello World", "ÉCOLE", "İstanbul",
    "STRASSE ẞ", "ΟΔΟΣ ΣΑΣ", "emoji 🐶🏖️", "2024-10-25 12:34", "<b>bold</b>", "a" * 200,
    "a photo of a dog playing in the snow next to a red car parked outside a small wooden "
    "cabin in the mountains at sunset with pink clouds",
    "Ⅻ", "ﬁne", "é", "zero​width", "東京 tower", "\x01ctl", "日本語の文章です。",
    "ภาษาไทย", "हिन्दी", "Привет мир", "مَرْحَبًا", "שָׁלוֹם", "x  y   z", "   ", "\n\n\n",
    "a\t\tb", "$100 & 50%", "C++ / C#", "über", "naïve café", "ДОМ", "ǅ", "ﬀ",
]


def hexs(s):
    return s.encode("utf-8").hex()


def stress(sp, n, seed):
    rng = random.Random(seed)
    pool = (
        "abcdefghijklmnopqrstuvwxyz0123456789 .,!?-'\"()"
        "àéîõüßçñøåæ" "абвгдежзий" "αβγδεζ" "אבגדהוזחט" "ابتثجحخ" "あいうえおかきくけこアイウ"
        "的一是不了人我在有他" "가나다라마바사" "ก ข ค" "क ख ग" "😀🐶🌍"
    )
    vocab = [sp.id_to_piece(i) for i in range(sp.get_piece_size())]
    normal = [p for i, p in enumerate(vocab) if sp.is_unknown(i) is False and not sp.is_control(i)]
    out = []
    for _ in range(n):
        k = rng.random()
        if k < 0.4:
            s = "".join(rng.choice(normal) for _ in range(rng.randint(1, 6))).replace("▁", " ")
        elif k < 0.8:
            s = "".join(rng.choice(pool) for _ in range(rng.randint(1, 30)))
        else:
            s = " ".join(
                "".join(rng.choice(pool[:36]) for _ in range(rng.randint(1, 9)))
                for _ in range(rng.randint(1, 5))
            )
        out.append(s)
    return out


def main():
    root, model_name, checkset = sys.argv[1], sys.argv[2], sys.argv[3]
    out = os.path.join(root, f"ref-{model_name}")
    os.makedirs(os.path.join(out, "pixels"), exist_ok=True)
    os.makedirs(os.path.join(out, "rgba"), exist_ok=True)
    fast = GemmaTokenizerFast.from_pretrained(root)
    sp = spm.SentencePieceProcessor(model_file=os.path.join(root, "tokenizer.model"))

    queries = []
    for line in open(checkset, encoding="utf-8"):
        if line.startswith("#") or not line.strip():
            continue
        lang, want, q = line.rstrip("\n").split("\t")
        queries.append((lang, [int(x) for x in want.split(",")], q))

    texts = [q for _, _, q in queries] + EDGE + stress(sp, 3000, 7)
    sp_diff = self_lower = 0
    with open(os.path.join(out, "tokens.tsv"), "w") as f:
        for t in texts:
            low = t.lower()
            ids = fast(low, padding="max_length", max_length=CONTEXT, truncation=True)["input_ids"]
            p_ids = (sp.encode(low)[: CONTEXT - 1] + [1] + [0] * CONTEXT)[:CONTEXT]
            sp_diff += p_ids != ids
            raw = fast(t, padding="max_length", max_length=CONTEXT, truncation=True)["input_ids"]
            self_lower += raw == ids and t != low
            f.write(f"{hexs(t)}\t{' '.join(map(str, ids))}\n")
    print(f"tokens: {len(texts)} texts; "
          f"sentencepiece differs from Hugging Face on {sp_diff}; "
          f"Hugging Face lower-cases by itself on {self_lower}")

    model = SiglipModel.from_pretrained(os.path.join(root, model_name), torch_dtype=torch.float32)
    model.eval()
    proc = SiglipImageProcessor.from_pretrained(os.path.join(root, model_name))
    with open(os.path.join(out, "logit.txt"), "w") as f:
        f.write(f"{model.logit_scale.exp().item()!r} {model.logit_bias.item()!r}\n")

    with torch.no_grad():
        names = [f"kodim{i:02d}" for i in range(1, 25)]
        img_e = []
        with open(os.path.join(out, "images.tsv"), "w") as f:
            for n in names:
                im = Image.open(os.path.join(root, "kodak", f"{n}.png")).convert("RGB")
                rgba = im.convert("RGBA")
                with open(os.path.join(out, "rgba", f"{n}-{im.width}x{im.height}.rgba"), "wb") as r:
                    r.write(rgba.tobytes())
                px = proc(images=im, return_tensors="pt")["pixel_values"]
                px.numpy().astype("<f4").tofile(os.path.join(out, "pixels", f"{n}.f32"))
                e = model.get_image_features(pixel_values=px)
                e = getattr(e, "pooler_output", e)
                e = torch.nn.functional.normalize(e, dim=-1)[0]
                img_e.append(e)
                f.write(n + "\t" + " ".join(f"{x:.9g}" for x in e.tolist()) + "\n")
        img_e = torch.stack(img_e)
        hits1 = hits3 = 0
        per_lang = {}
        with open(os.path.join(out, "text.tsv"), "w") as f:
            for lang, want, q in queries:
                ids = fast(q.lower(), padding="max_length", max_length=CONTEXT, truncation=True,
                           return_tensors="pt")["input_ids"]
                e = model.get_text_features(input_ids=ids)
                e = getattr(e, "pooler_output", e)
                e = torch.nn.functional.normalize(e, dim=-1)[0]
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
