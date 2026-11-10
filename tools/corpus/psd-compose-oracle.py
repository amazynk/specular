#!/usr/bin/env python3
"""Compares Specular's PSD composite built from the layers with the merged image Photoshop
stored, as psd-tools reads it (un-matted, straight alpha), or, for files saved without one,
with psd-tools' composite of the layers. Called by the ignored test
crates/formats/specular-psd/tests/compose_oracle.rs with its manifest:

    rel <TAB> path <TAB> WxHxC <TAB> raw-rgba      (our composite, 8-bit, C = 1 or 4)
    rel <TAB> path <TAB> skip|fail <TAB> reason

Files under `solo/` (tools/corpus/psd-adjust-solo.py: one adjustment layer left visible) are
compared with psd-tools' composite, as their merged image no longer matches.

Prints `ORACLE<TAB>PSD-compose/<group><TAB>rel<TAB>pass|fail|skip` per file, then a `#`
detail line. Both images are compared premultiplied (colour times alpha) and by alpha, in
0..255: a file passes when the mean difference is at most MEAN and at most FRAC of the
pixels differ by more than FAR.
"""
import sys

import numpy as np

MEAN = 1.5
FAR = 24
FRAC = 0.005


# Adjustment types psd-tools draws in its composite (the per-type copies in `solo/` of the
# others have no reference).
PSD_TOOLS_ADJUSTMENTS = {"brit", "levl", "curv", "expA", "hue2", "nvrt", "post", "thrs", "none"}


def reference(path, solo=False):
    from psd_tools import PSDImage

    psd = PSDImage.open(path)
    if psd.has_preview() and not solo:
        a = np.asarray(psd.numpy(), np.float64)
    else:
        # No merged image: psd-tools' own composite of the layers.
        a = np.asarray(psd.composite(ignore_preview=True).convert("RGBA"), np.float64) / 255.0
    if a.ndim == 2:
        a = a[..., None]
    if a.shape[2] in (1, 3):
        a = np.concatenate([a, np.ones(a.shape[:2] + (1,))], axis=2)
    elif a.shape[2] == 2:
        a = np.concatenate([a[..., :1], a[..., 1:2]], axis=2)
    return a * 255.0


def ours(raw, shape):
    w, h, c = (int(v) for v in shape.split("x"))
    a = np.frombuffer(open(raw, "rb").read(), np.uint8).astype(np.float64)
    return a.reshape(h, w, c)


def premul(a):
    alpha = a[..., -1:]
    return np.concatenate([a[..., :-1] * alpha / 255.0, alpha], axis=2)


def main():
    rows = [l.rstrip("\n").split("\t") for l in open(sys.argv[1]) if l.strip()]
    counts = {"pass": 0, "fail": 0, "skip": 0}
    for rel, path, what, extra in rows:
        group = rel.split("/", 1)[0] if "/" in rel else "."
        fmt = f"PSD-compose/{group}"
        solo = group == "solo"
        kind = rel.rsplit("-", 1)[-1].removesuffix(".psd") if solo else ""
        if what in ("skip", "fail"):
            status, detail = what, extra
        elif solo and kind not in PSD_TOOLS_ADJUSTMENTS:
            status, detail = "skip", f"psd-tools does not draw {kind} adjustments"
        else:
            try:
                ref = reference(path, solo)
            except Exception as e:  # noqa: BLE001 - the oracle could not read it
                status, detail = "skip", f"psd-tools: {e}"
            else:
                got = ours(extra, what)
                if got.shape[2] == 1:
                    got = np.concatenate([got, np.full(got.shape, 255.0)], axis=2)
                if ref.shape[2] == 2 and got.shape[2] == 4:
                    got = np.concatenate([got[..., :1], got[..., 3:]], axis=2)
                elif ref.shape[2] == 4 and got.shape[2] == 2:
                    got = np.concatenate([got[..., :1]] * 3 + [got[..., 1:]], axis=2)
                # A merged "transparency" that is zero everywhere is a saved selection psd-tools
                # takes for alpha (the colour is all there): compared as opaque.
                if ref.shape == got.shape and ref.shape[2] in (2, 4) and not ref[..., -1].any():
                    ref = ref.copy()
                    ref[..., -1] = 255.0
                if ref.shape != got.shape:
                    status, detail = "fail", f"shape {got.shape} vs {ref.shape}"
                else:
                    d = np.abs(premul(got) - premul(ref))
                    mean = d.mean()
                    far = (d.max(axis=2) > FAR).mean()
                    ok = mean <= MEAN and far <= FRAC
                    status = "pass" if ok else "fail"
                    detail = f"mean {mean:.2f}/255, {100 * far:.2f}% beyond {FAR}"
        counts[status] += 1
        print(f"ORACLE\t{fmt}\t{rel}\t{status}")
        print(f"# {rel}: {detail}")
    print(f"# psd-compose: {counts['pass']} pass, {counts['fail']} fail, {counts['skip']} skip")


if __name__ == "__main__":
    main()
