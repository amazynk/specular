#!/usr/bin/env python3
"""Reference outputs for Specular's OCR (crates/specular-infer/src/ocr), dev tool only.

Subcommands:

  tensor MODEL_DIR IN.f32 N,C,H,W OUT.f32
      Runs one PaddleOCR safetensors export (config.json + model.safetensors) through its
      Hugging Face transformers port on an RGB float32 input (raw little-endian, NCHW, already
      normalised); the channels are flipped to BGR as Paddle reads them. Writes the output:
      a detector's probability map [N,1,H,W], a recogniser's per-step probabilities [N,T,C]
      or the orientation classifier's probabilities [N,2]. Used by crates/specular-infer/tests/ocr.rs when SPECULAR_OCR_PYTHON is set.

  paddle DET_DIR REC_DIR IMAGES_DIR OUT.tsv [--side N] [--prefix P]
      The whole PaddleOCR 3 pipeline (paddlepaddle CPU, paddleocr; the Paddle inference
      exports of the same models) on every PNG/JPEG in IMAGES_DIR with the detector input
      limited to N pixels on the long side and the detector's own thresholds (as ours),
      orientation models off. Appends one line per text box to OUT.tsv: P + image, 8 corner
      coordinates, score, text.

  tess TESSDATA_DIR LANG LINES_DIR OUT.tsv
      Tesseract 5 (the `tesseract` binary on PATH, built from source) on each line image
      in LINES_DIR as a single line (--psm 7, --oem 1). One line per image: name, text.

  score TRUTH.tsv OURS.tsv REF.tsv
      Detection F1 (IoU ≥ 0.5 between quads, one-to-one) and recognition accuracy (exact
      line match and 1 − character error rate over matched boxes) of ours and the reference
      against the truth, and of ours against the reference.

Needs (venvs under target/, caches pointed there too, nothing downloaded at run time):
torch (CPU) + transformers ≥ 5.22 + numpy for `tensor`; paddlepaddle + paddleocr for
`paddle` (PADDLE_PDX_CACHE_HOME under target/, models given by directory).
"""

import os
import sys

os.environ.setdefault("HF_HUB_OFFLINE", "1")
os.environ.setdefault("TRANSFORMERS_OFFLINE", "1")


def tensor(model_dir, inp, shape, out):
    import numpy as np
    import torch
    import transformers as tf

    cfg = tf.AutoConfig.from_pretrained(model_dir)
    kinds = {
        "pp_ocrv6_small_det": tf.PPOCRV6SmallDetForObjectDetection,
        "pp_ocrv6_small_rec": tf.PPOCRV6SmallRecForTextRecognition,
        "pp_ocrv6_tiny_rec": tf.PPOCRV6TinyRecForTextRecognition,
        "pp_ocrv5_mobile_rec": tf.PPOCRV5MobileRecForTextRecognition,
        "pp_ocrv5_mobile_det": tf.PPOCRV5MobileDetForObjectDetection,
        "pp_lcnet": tf.PPLCNetForImageClassification,
    }
    model = kinds[cfg.model_type].from_pretrained(model_dir).eval()
    n, c, h, w = (int(v) for v in shape.split(","))
    x = np.fromfile(inp, dtype="<f4").reshape(n, c, h, w)[:, ::-1].copy()
    with torch.no_grad():
        y = model(pixel_values=torch.from_numpy(x)).last_hidden_state
        if cfg.model_type == "pp_lcnet":
            y = torch.softmax(y, dim=-1)
    np.asarray(y.numpy(), dtype="<f4").tofile(out)
    print(model_dir, tuple(y.shape))


def overlaps(a, b):
    """(intersection / area of a, intersection / area of b, IoU) of two quads."""
    from shapely.geometry import Polygon

    pa, pb = Polygon(a).buffer(0), Polygon(b).buffer(0)
    if pa.area <= 0 or pb.area <= 0:
        return 0.0, 0.0, 0.0
    inter = pa.intersection(pb).area
    return inter / pa.area, inter / pb.area, inter / (pa.area + pb.area - inter)


def read_boxes(path):
    """image → list of (quad, text) from a TSV of image, 8 coordinates, score, text."""
    boxes = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            p = line.rstrip("\n").split("\t")
            if len(p) < 10:
                continue
            q = [float(v) for v in p[1:9]]
            boxes.setdefault(p[0], []).append(([(q[0], q[1]), (q[2], q[3]), (q[4], q[5]), (q[6], q[7])], p[10] if len(p) > 10 else ""))
    return boxes


def cer(a, b):
    import difflib

    if not a and not b:
        return 0.0
    # Levenshtein distance.
    prev = list(range(len(b) + 1))
    for i, ca in enumerate(a, 1):
        cur = [i]
        for j, cb in enumerate(b, 1):
            cur.append(min(prev[j] + 1, cur[j - 1] + 1, prev[j - 1] + (ca != cb)))
        prev = cur
    return prev[-1] / max(len(a), len(b), 1)


def match(truth, got, deteval=True):
    """One-to-one greedy matching: DetEval's criterion (the box covers at least 80 % of the
    true line and the line at least 40 % of the box: boxes the detector grows around the
    ink still match) or, with deteval False, IoU >= 0.5. Returns (true text, got text) pairs."""
    pairs = []
    used = set()
    for tq, tt in truth:
        best, bi = 0.0, None
        for gi, (gq, gt) in enumerate(got):
            if gi in used:
                continue
            rt, rg, iou = overlaps(tq, gq)
            ok = (rt >= 0.8 and rg >= 0.4) if deteval else iou >= 0.5
            if ok and iou > best:
                best, bi = iou, gi
        if bi is not None:
            used.add(bi)
            pairs.append((tt, got[bi][1]))
    return pairs


def score(truth_path, ours_path, ref_path):
    truth, ours, ref = read_boxes(truth_path), read_boxes(ours_path), read_boxes(ref_path)
    images = sorted(truth)
    langs = sorted({im.split("/")[0] for im in images})

    def report(got, which):
        tp = nt = ng = exact = 0
        cers = []
        for im in which:
            t, g = truth.get(im, []), got.get(im, [])
            pairs = match(t, g)
            tp += len(pairs)
            nt += len(t)
            ng += len(g)
            for tt, gt in pairs:
                exact += tt == gt
                cers.append(cer(tt, gt))
        p = tp / ng if ng else 0.0
        r = tp / nt if nt else 0.0
        f1 = 2 * p * r / (p + r) if p + r else 0.0
        acc = 1 - sum(cers) / len(cers) if cers else 0.0
        return (f"boxes {ng:3}, truth {nt:3}, matched {tp:3}; P {p:.3f} R {r:.3f} F1 {f1:.3f}; "
                f"lines exact {exact:3}/{len(cers):3}, 1-CER {acc:.3f}")

    for lang in langs + ["all"]:
        which = images if lang == "all" else [im for im in images if im.startswith(lang + "/")]
        print(f"{lang:6} ours      {report(ours, which)}")
        print(f"{lang:6} reference {report(ref, which)}")
    # Ours against the reference, box by box (IoU >= 0.5) and text.
    tp = n_ref = n_ours = same = 0
    diffs = []
    for im in sorted(set(ref) | set(ours)):
        pairs = match(ref.get(im, []), ours.get(im, []), deteval=False)
        tp += len(pairs)
        n_ref += len(ref.get(im, []))
        n_ours += len(ours.get(im, []))
        same += sum(1 for a, b in pairs if a == b)
        diffs.extend((im, a, b) for a, b in pairs if a != b)
    print(f"ours vs reference: {tp} boxes matched of {n_ref} (ours {n_ours}); same text {same}/{tp}")
    for im, a, b in diffs[:int(os.environ.get("SHOW_DIFFS", "0"))]:
        print(f"  {im}: ref {a!r} ours {b!r}")


def paddle(det_dir, rec_dir, images_dir, out, side, prefix=""):
    os.environ.setdefault("PADDLE_PDX_DISABLE_MODEL_SOURCE_CHECK", "True")
    from paddleocr import PaddleOCR

    ocr = PaddleOCR(
        text_detection_model_dir=det_dir,
        text_detection_model_name="PP-OCRv6_small_det",
        text_recognition_model_dir=rec_dir,
        text_recognition_model_name=os.path.basename(rec_dir.rstrip("/")),
        use_doc_orientation_classify=False,
        use_doc_unwarping=False,
        use_textline_orientation=False,
        enable_mkldnn=False,
        text_det_limit_side_len=side,
        text_det_limit_type="max",
        # The detector's own post-processing values (its inference.yml), as ours.
        text_det_thresh=0.2,
        text_det_box_thresh=0.45,
        text_det_unclip_ratio=1.4,
    )
    with open(out, "a", encoding="utf-8") as f:
        for name in sorted(os.listdir(images_dir)):
            if not name.lower().endswith((".png", ".jpg", ".jpeg")):
                continue
            for res in ocr.predict(os.path.join(images_dir, name)):
                for poly, s, t in zip(res["rec_polys"], res["rec_scores"], res["rec_texts"]):
                    q = "\t".join(f"{v:.1f}" for p in poly for v in p)
                    f.write(f"{prefix}{name}\t{q}\t{s:.4f}\t{t}\n")


def tess(tessdata, lang, lines_dir, out):
    import subprocess

    with open(out, "w", encoding="utf-8") as f:
        for name in sorted(os.listdir(lines_dir)):
            if not name.endswith(".png"):
                continue
            r = subprocess.run(
                ["tesseract", os.path.join(lines_dir, name), "-", "--tessdata-dir", tessdata,
                 "-l", lang, "--psm", "7", "--oem", "1"],
                capture_output=True, text=True, check=False)
            f.write(f"{name}\t{r.stdout.strip()}\n")


def main():
    a = sys.argv[1:]
    if a[:1] == ["tensor"] and len(a) == 5:
        tensor(*a[1:])
    elif a[:1] == ["score"] and len(a) == 4:
        score(*a[1:])
    elif a[:1] == ["paddle"] and len(a) >= 5:
        side = int(a[a.index("--side") + 1]) if "--side" in a else 1920
        prefix = a[a.index("--prefix") + 1] if "--prefix" in a else ""
        paddle(a[1], a[2], a[3], a[4], side, prefix)
    elif a[:1] == ["tess"] and len(a) == 5:
        tess(*a[1:])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
