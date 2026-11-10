#!/usr/bin/env python3
"""People/Culling oracle: our face pipeline against MediaPipe, LiteRT and ONNX
Runtime. Reference only: run in a venv under the target dir with mediapipe,
ai-edge-litert, onnxruntime, numpy, pillow and scipy.

  face-oracle.py prep CORPUS MODELS OUT
      decodes the corpus (fetch-faces.py) to OUT/img/<id>.rgba, runs MediaPipe's
      FaceDetector (BlazeFace full range) and FaceLandmarker (blendshapes) on each photo
      into OUT/ref/, and lists the identity photos with one reference face in
      OUT/identity.tsv.
  (then: SPECULAR_FACE_ORACLE=OUT SPECULAR_FACE_MODELS=MODELS cargo test -p specular-infer
         --test face_oracle)
  face-oracle.py compare OUT MODELS
      recomputes every dumped network input with LiteRT / ONNX Runtime and compares; scores
      detection, eye-blink and grouping against the references; prints a summary.
"""

import glob
import os
import struct
import sys

import numpy as np

MIN_SCORE = 0.5


def faces_tsv(path):
    out = []
    if os.path.exists(path):
        for line in open(path):
            v = [float(x) for x in line.split()]
            if v:
                out.append(v)
    return out


def iou(a, b):
    x0, y0 = max(a[0], b[0]), max(a[1], b[1])
    x1, y1 = min(a[0] + a[2], b[0] + b[2]), min(a[1] + a[3], b[1] + b[3])
    inter = max(0, x1 - x0) * max(0, y1 - y0)
    u = a[2] * a[3] + b[2] * b[3] - inter
    return inter / u if u > 0 else 0


def match(ref, got, thr=0.5):
    """Greedy IoU matching of boxes (x, y, w, h): list of (ref index, got index)."""
    pairs = sorted(((iou(r, g), i, j) for i, r in enumerate(ref) for j, g in enumerate(got)),
                   reverse=True)
    used_r, used_g, out = set(), set(), []
    for v, i, j in pairs:
        if v < thr:
            break
        if i in used_r or j in used_g:
            continue
        used_r.add(i)
        used_g.add(j)
        out.append((i, j))
    return out


def prep(corpus, models, out):
    import mediapipe as mp
    from PIL import Image, ImageOps
    from mediapipe.tasks.python import BaseOptions, vision
    os.makedirs(f"{out}/img", exist_ok=True)
    os.makedirs(f"{out}/ref", exist_ok=True)
    det = vision.FaceDetector.create_from_options(vision.FaceDetectorOptions(
        base_options=BaseOptions(model_asset_path=f"{models}/blaze_face_full_range.tflite"),
        min_detection_confidence=MIN_SCORE))
    lm = vision.FaceLandmarker.create_from_options(vision.FaceLandmarkerOptions(
        base_options=BaseOptions(model_asset_path=f"{models}/face_landmarker.task"),
        num_faces=10, output_face_blendshapes=True))
    ident = open(f"{out}/identity.tsv", "w")
    files = sorted(glob.glob(f"{corpus}/**/*.jpg", recursive=True))
    for n, path in enumerate(files):
        rel = os.path.relpath(path, corpus)[:-4]
        id_ = rel.replace(os.sep, "__")
        try:
            im = ImageOps.exif_transpose(Image.open(path)).convert("RGB")
        except Exception as e:
            print("skip", rel, e)
            continue
        w, h = im.size
        a = np.asarray(im)
        rgba = np.concatenate([a, np.full((h, w, 1), 255, np.uint8)], axis=2)
        with open(f"{out}/img/{id_}.rgba", "wb") as f:
            f.write(struct.pack("<II", w, h))
            f.write(rgba.tobytes())
        mi = mp.Image(image_format=mp.ImageFormat.SRGB, data=np.ascontiguousarray(a))
        r = det.detect(mi)
        with open(f"{out}/ref/{id_}.det.tsv", "w") as f:
            for d in r.detections:
                b = d.bounding_box
                kp = [v for k in d.keypoints for v in (k.x * w, k.y * h)]
                vals = [d.categories[0].score, b.origin_x, b.origin_y, b.width, b.height] + kp
                f.write("\t".join(f"{v:.4f}" for v in vals) + "\n")
        # The same photo shrunk to the model's 192 px by area averaging first (as ours is):
        # tells resampling differences from model differences.
        k = 192 / max(w, h)
        small = np.ascontiguousarray(np.asarray(
            im.resize((max(1, round(w * k)), max(1, round(h * k))), Image.BOX)))
        rs = det.detect(mp.Image(image_format=mp.ImageFormat.SRGB, data=small))
        with open(f"{out}/ref/{id_}.det192.tsv", "w") as f:
            for d in rs.detections:
                b = d.bounding_box
                vals = [d.categories[0].score, b.origin_x / k, b.origin_y / k,
                        b.width / k, b.height / k]
                f.write("\t".join(f"{v:.4f}" for v in vals) + "\n")
        res = lm.detect(mi)
        with open(f"{out}/ref/{id_}.blink.tsv", "w") as f:
            for pts, bs in zip(res.face_landmarks, res.face_blendshapes):
                xs = [p.x * w for p in pts]
                ys = [p.y * h for p in pts]
                names = {c.category_name: c.score for c in bs}
                if n == 0:
                    print("blendshapes:", [c.category_name for c in bs][:12])
                f.write(f"{min(xs):.2f}\t{min(ys):.2f}\t{max(xs) - min(xs):.2f}\t"
                        f"{max(ys) - min(ys):.2f}\t{names['eyeBlinkLeft']:.5f}\t"
                        f"{names['eyeBlinkRight']:.5f}\n")
                # Pixel landmarks for the blendshape input convention check.
        if rel.startswith("identity") and len(r.detections) == 1:
            person = rel.split(os.sep)[1]
            b = r.detections[0].bounding_box
            if b.width >= 40:
                ident.write(f"{id_}\t{person}\t{b.origin_x}\t{b.origin_y}\t{b.width}\t{b.height}\n")
        print(f"\r{n + 1}/{len(files)}", end="", flush=True)
    print()


def litert(path):
    from ai_edge_litert.interpreter import Interpreter
    it = Interpreter(model_path=path)
    it.allocate_tensors()
    return it


def run_tfl(it, x):
    it.set_tensor(it.get_input_details()[0]["index"], x.astype(np.float32))
    it.invoke()
    return [it.get_tensor(d["index"]) for d in it.get_output_details()]


def compare(out, models):
    import zipfile
    import onnxruntime as ort
    rd = lambda p: np.fromfile(p, "<f4")
    print("== networks on identical inputs ==")
    blaze = litert(f"{models}/blaze_face_full_range.tflite")
    z = zipfile.ZipFile(f"{models}/face_landmarker.task")
    tmp = f"{out}/models"
    os.makedirs(tmp, exist_ok=True)
    for n in ["face_landmarks_detector.tflite", "face_blendshapes.tflite"]:
        open(f"{tmp}/{n}", "wb").write(z.read(n))
    mesh = litert(f"{tmp}/face_landmarks_detector.tflite")
    shapes = litert(f"{tmp}/face_blendshapes.tflite")
    stats = {}
    for path in sorted(glob.glob(f"{out}/dump/blaze/*.in.f32")):
        x = rd(path).reshape(1, 3, 192, 192).transpose(0, 2, 3, 1)
        ours = rd(path.replace(".in.", ".out."))
        reg, cls = run_tfl(blaze, x)
        ref = np.concatenate([reg.ravel(), cls.ravel()])
        d = np.abs(ours - ref)
        so, sr = ours[2304 * 16:] > np.log(MIN_SCORE / (1 - MIN_SCORE)), cls.ravel() > np.log(MIN_SCORE / (1 - MIN_SCORE))
        s = stats.setdefault("blaze", [0, 0.0, 0, 0])
        s[0] += 1; s[1] = max(s[1], d.max()); s[2] += int((so != sr).sum()); s[3] += int(sr.sum())
    for path in sorted(glob.glob(f"{out}/dump/mesh/*.in.f32")):
        x = rd(path).reshape(1, 3, 256, 256).transpose(0, 2, 3, 1)
        ours = rd(path.replace(".in.", ".out."))
        o = run_tfl(mesh, x)
        ref = np.concatenate([o[0].ravel(), o[1].ravel()])
        s = stats.setdefault("mesh", [0, 0.0, 0.0])
        s[0] += 1
        s[1] = max(s[1], np.abs(ours[:1434] - ref[:1434]).max())
        s[2] = max(s[2], abs(ours[1434] - ref[1434]))
    for path in sorted(glob.glob(f"{out}/dump/shapes/*.in.f32")):
        x = rd(path).reshape(1, 146, 2)
        ours = rd(path.replace(".in.", ".out."))
        ref = run_tfl(shapes, x)[0].ravel()
        s = stats.setdefault("shapes", [0, 0.0])
        s[0] += 1
        s[1] = max(s[1], np.abs(ours - ref).max())
    for kind, file in [("sface", "face_recognition_sface_2021dec.onnx"), ("aura", "glintr100.onnx")]:
        paths = sorted(glob.glob(f"{out}/dump/{kind}/*.in.f32"))
        if not paths:
            continue
        sess = ort.InferenceSession(f"{models}/{file}", providers=["CPUExecutionProvider"])
        name = sess.get_inputs()[0].name
        cos = []
        for path in paths:
            x = rd(path).reshape(1, 3, 112, 112)
            ours = rd(path.replace(".in.", ".out."))
            ref = sess.run(None, {name: x})[0].ravel()
            cos.append(float(ours @ ref / np.linalg.norm(ours) / np.linalg.norm(ref)))
        stats[kind] = [len(cos), min(cos), float(np.mean(cos))]
    for k, v in stats.items():
        print(k, v)

    print("== detection vs MediaPipe FaceDetector (whole photo) ==")
    ids = [os.path.basename(p)[:-5] for p in glob.glob(f"{out}/img/*.rgba")]
    for sub in ["identity", "group", "eyes", ""]:
        tot = {"ref": 0, "whole": 0, "mw": 0, "tiled": 0, "mt": 0}
        for i in ids:
            if sub and not i.startswith(sub):
                continue
            ref = [r[1:5] for r in faces_tsv(f"{out}/ref/{i}.det.tsv")]
            w = [r[1:5] for r in faces_tsv(f"{out}/ours/{i}.whole.tsv")]
            t = [r[1:5] for r in faces_tsv(f"{out}/ours/{i}.tiled.tsv")]
            tot["ref"] += len(ref); tot["whole"] += len(w); tot["tiled"] += len(t)
            tot["mw"] += len(match(ref, w)); tot["mt"] += len(match(ref, t))
        if tot["ref"] == 0:
            continue
        print(f"{sub or 'all':9} reference {tot['ref']}: whole recall {tot['mw'] / tot['ref']:.4f} "
              f"precision {tot['mw'] / max(tot['whole'], 1):.4f} ({tot['ref'] - tot['mw']} missed, "
              f"{tot['whole'] - tot['mw']} extra); tiled recall {tot['mt'] / tot['ref']:.4f}, "
              f"{tot['tiled']} faces ({tot['tiled'] - tot['mt']} beyond the reference)")

    print("== detection, both from the photo shrunk to 192 px by area averaging ==")
    for sub in ["group", ""]:
        ref_n = got_n = m = 0
        for i in ids:
            if sub and not i.startswith(sub):
                continue
            ref = [r[1:5] for r in faces_tsv(f"{out}/ref/{i}.det192.tsv")]
            w = [r[1:5] for r in faces_tsv(f"{out}/ours/{i}.whole.tsv")]
            ref_n += len(ref)
            got_n += len(w)
            m += len(match(ref, w))
        if ref_n:
            print(f"{sub or 'all':9} reference {ref_n}: recall {m / ref_n:.4f} "
                  f"precision {m / max(got_n, 1):.4f} ({ref_n - m} missed, {got_n - m} extra)")

    print("== eye blink vs MediaPipe FaceLandmarker ==")
    diffs, agree, n = [], 0, 0
    for i in ids:
        ref = faces_tsv(f"{out}/ref/{i}.blink.tsv")
        got = faces_tsv(f"{out}/ours/{i}.tiled.tsv")
        eyes = faces_tsv(f"{out}/ours/{i}.eyes.tsv")
        for r in ref:
            rc = (r[0] + r[2] / 2, r[1] + r[3] / 2)
            best = None
            for g, e in zip(got, eyes):
                gc = (g[1] + g[3] / 2, g[2] + g[4] / 2)
                dd = np.hypot(rc[0] - gc[0], rc[1] - gc[1]) / max(r[2], 1)
                if dd < 0.25 and (best is None or dd < best[0]):
                    best = (dd, e)
            if best is None:
                continue
            e = best[1]
            n += 1
            diffs += [abs(e[0] - r[4]), abs(e[1] - r[5])]
            agree += (min(e[0], e[1]) > 0.5) == (min(r[4], r[5]) > 0.5)
    if n:
        d = np.array(diffs)
        print(f"{n} faces: |blink diff| mean {d.mean():.4f}, p95 {np.percentile(d, 95):.4f}, "
              f"max {d.max():.4f}; closed-eyes flag agrees {agree}/{n}")

    print("== grouping (pairwise precision / recall) ==")
    from scipy.cluster.hierarchy import fcluster, linkage
    for kind in ["sface", "aura"]:
        p = f"{out}/ours/identity.{kind}.f32"
        if not os.path.exists(p):
            continue
        labels = [l.split("\t")[1] for l in open(f"{out}/ours/identity.{kind}.tsv").read().split("\n") if l]
        v = rd(p).reshape(len(labels), -1)
        truth = np.unique(labels, return_inverse=True)[1]
        Z = linkage(v, "average", metric="cosine")
        for f in sorted(glob.glob(f"{out}/ours/identity.{kind}.clusters.*.tsv")):
            thr = float(f.rsplit(".", 2)[-2].replace("_", "."))
            ours = np.array([int(x) for x in open(f).read().split()])
            sp = fcluster(Z, 1 - thr, criterion="distance")
            pr = pairwise(ours, truth)
            same = pairwise(ours, sp)
            print(f"{kind} thr {thr:.2f}: {len(labels)} faces, {len(set(truth))} people, "
                  f"{len(set(ours))} groups: precision {pr[0]:.4f} recall {pr[1]:.4f}; "
                  f"vs scipy average linkage {same}")


def pairwise(a, b):
    a, b = np.asarray(a), np.asarray(b)
    sa = a[:, None] == a[None, :]
    sb = b[:, None] == b[None, :]
    iu = np.triu_indices(len(a), 1)
    sa, sb = sa[iu], sb[iu]
    tp = (sa & sb).sum()
    p = tp / max(sa.sum(), 1)
    r = tp / max(sb.sum(), 1)
    return round(float(p), 4), round(float(r), 4)


if __name__ == "__main__":
    if len(sys.argv) < 4 and not (len(sys.argv) == 4):
        sys.exit(__doc__)
    if sys.argv[1] == "prep":
        prep(*sys.argv[2:5])
    elif sys.argv[1] == "compare":
        compare(*sys.argv[2:4])
    else:
        sys.exit(__doc__)
