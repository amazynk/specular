"""pathology-oracle.py <dir>: checks specular-pathology's and specular-labelmap's output
against scikit-image, numpy, pycocotools, shapely, tifffile, nibabel and SimpleITK.

<dir> holds what the ignored dump tests write:
  SPECULAR_PATHOLOGY_ORACLE=<dir> cargo test -p specular-pathology --test oracle_dump -- --ignored
  SPECULAR_LABELMAP_ORACLE=<dir> cargo test -p specular-labelmap --test oracle_dump -- --ignored
Run with the oracle venv (python -I). Prints one line per check with its mismatch
count; exits 1 when any check fails.
"""
import json
import os
import sys

import numpy as np

D = sys.argv[1]
failed = []


def report(name, bad, total, note=""):
    ok = bad == 0
    print(f"{'ok  ' if ok else 'FAIL'} {name}: {bad} mismatches of {total}{' ' + note if note else ''}")
    if not ok:
        failed.append(name)


def f64(name):
    return np.fromfile(os.path.join(D, name), dtype="<f8")


def stains():
    from skimage.color import hdx_from_rgb, separate_stains

    px = np.fromfile(os.path.join(D, "pixels.rgb"), dtype=np.uint8).reshape(64, 64, 3)
    for set_id in ("he", "hdab", "hedab"):
        m = f64(f"{set_id}-matrix.f64").reshape(3, 3)
        ours = f64(f"{set_id}-conc.f64").reshape(64, 64, 3)
        # scikit-image: ln(rgb) / ln(1e-6) @ M^-1, clamped at 0; ours is log10, unclamped.
        theirs = separate_stains(px, np.linalg.inv(m)) * 6.0
        diff = np.abs(np.maximum(ours, 0) - theirs)
        report(f"stains {set_id} vs skimage.separate_stains (|d| > 1e-9)", int((diff > 1e-9).sum()), diff.size,
               f"max {diff.max():.2e}")
        # Haematoxylin alone, as light (skimage.combine_stains of the H channel).
        from skimage.color import combine_stains

        h_only = np.zeros_like(theirs)
        h_only[..., 0] = theirs[..., 0] / 6.0
        light = combine_stains(h_only, m)
        ours_light = f64(f"{set_id}-h-intensity.f64").reshape(64, 64, 3)
        d2 = np.abs(ours_light - light)
        report(f"stains {set_id} haematoxylin light vs skimage.combine_stains (|d| > 1e-6)",
               int((d2 > 1e-6).sum()), d2.size, f"max {d2.max():.2e}")
    # H-DAB against scikit-image's own matrix (same published vectors; rows unnormalised
    # there, which changes only the residual's scale).
    ours = f64("hdab-conc.f64").reshape(64, 64, 3)
    theirs = separate_stains(px, hdx_from_rgb) * 6.0
    d = np.abs(np.maximum(ours[..., :2], 0) - theirs[..., :2]) / np.maximum(1.0, theirs[..., :2])
    report("stains hdab H and DAB vs skimage hdx_from_rgb (relative > 2e-3)", int((d > 2e-3).sum()), d.size,
           f"max {d.max():.2e}")


def macenko_numpy(rgb, beta=0.15, alpha=1.0):
    od = -np.log10(np.maximum(rgb.astype(np.float64) / 255.0, 1e-6))
    od = od[~np.any(od < beta, axis=1)]
    _, vecs = np.linalg.eigh(np.cov(od.T))
    e = vecs[:, [2, 1]]
    for k in range(2):
        if e[:, k].sum() < 0:
            e[:, k] *= -1
    t = od @ e
    phi = np.arctan2(t[:, 1], t[:, 0])
    lo, hi = np.percentile(phi, alpha), np.percentile(phi, 100 - alpha)
    v1 = e @ np.array([np.cos(lo), np.sin(lo)])
    v2 = e @ np.array([np.cos(hi), np.sin(hi)])
    v1, v2 = [v if v.sum() >= 0 else -v for v in (v1, v2)]
    v1, v2 = v1 / np.linalg.norm(v1), v2 / np.linalg.norm(v2)
    he = np.array([v1, v2]) if v1[0] > v2[0] else np.array([v2, v1])
    c = np.linalg.lstsq(he.T, od.T, rcond=None)[0]
    return he, np.percentile(c, 99, axis=1)


def macenko():
    rgb = np.fromfile(os.path.join(D, "macenko.rgb"), dtype=np.uint8).reshape(-1, 3)
    ours = f64("macenko.f64")
    he, maxc = macenko_numpy(rgb)
    dv = np.abs(ours[:6].reshape(2, 3) - he).max()
    dc = np.abs(ours[6:8] - maxc).max()
    report("macenko vectors vs numpy reference (|d| > 1e-6)", int(dv > 1e-6), 6, f"max {dv:.2e}")
    report("macenko max concentrations vs numpy (|d| > 1e-6)", int(dc > 1e-6), 2, f"max {dc:.2e}")
    true = np.array([[0.650, 0.704, 0.286], [0.268, 0.570, 0.776]])
    true /= np.linalg.norm(true, axis=1, keepdims=True)
    ang = np.degrees(np.arccos(np.clip((ours[:6].reshape(2, 3) * true).sum(1), -1, 1)))
    report("macenko vs the synthetic slide's true vectors (> 2.5 deg)", int((ang > 2.5).sum()), 2,
           f"angles {ang[0]:.2f}, {ang[1]:.2f} deg")


def annotations():
    from pycocotools.coco import COCO
    from pycocotools import mask as mu
    import contextlib
    import io

    with contextlib.redirect_stdout(io.StringIO()):
        coco = COCO(os.path.join(D, "coco.json"))
    anns = coco.loadAnns(coco.getAnnIds())
    img = coco.loadImgs(1)[0]
    from shapely.geometry import Point, shape

    bad = 0
    total = 0
    edge = 0
    for i, a in enumerate(anns):
        ours = np.fromfile(os.path.join(D, f"mask-{i}.u8"), dtype=np.uint8).reshape(img["height"], img["width"])
        if not a["segmentation"]:
            # A point: its keypoint is in its pixel.
            x, y = a["keypoints"][:2]
            bad += int(ours[int(y), int(x)] != 1 or ours.sum() != 1)
            total += 1
            continue
        theirs = coco.annToMask(a)
        diff = np.argwhere(theirs != ours)
        edge += len(diff)
        # pycocotools rasterises polygon edges by its own rule (not pixel centres): a pixel
        # may differ only when its centre is within a pixel of the outline.
        outline = shape(json.load(open(os.path.join(D, "qupath.geojson")))["features"][i]["geometry"]).boundary
        bad += sum(int(outline.distance(Point(x + 0.5, y + 0.5)) > 1.0) for y, x in diff)
        total += ours.size
    report("COCO segmentations vs pycocotools annToMask (pixels off the outline)", bad, total,
           f"({edge} differ on the outline)")
    # Area and box.
    from shapely.geometry import Polygon

    bad_area = 0
    for a in anns:
        if isinstance(a["segmentation"], dict):
            continue
        if not a["segmentation"]:
            continue
        area = sum(Polygon(np.array(r).reshape(-1, 2)).area for r in a["segmentation"])
        bad_area += int(abs(area - a["area"]) > 2e-3)
        xs = np.concatenate([np.array(r)[0::2] for r in a["segmentation"]])
        ys = np.concatenate([np.array(r)[1::2] for r in a["segmentation"]])
        box = [xs.min(), ys.min(), xs.max() - xs.min(), ys.max() - ys.min()]
        bad_area += int(np.abs(np.array(box) - np.array(a["bbox"])).max() > 2e-3)
    report("COCO area and bbox vs shapely (coordinates at 1/1000 px)", bad_area, len(anns))
    rle = [a for a in anns if isinstance(a["segmentation"], dict)]
    bad_rle = 0
    for a in rle:
        r = mu.frPyObjects(a["segmentation"], img["height"], img["width"])
        bad_rle += int(abs(float(mu.area(r)) - (coco.annToMask(a).sum())) > 0)
    report("COCO RLE (holes) decodes in pycocotools", bad_rle, len(rle))

    # GeoJSON: shapely-valid geometries with the classes and areas the COCO file states.
    from shapely.geometry import shape

    g = json.load(open(os.path.join(D, "qupath.geojson")))
    bad_g = 0
    for f, a in zip(g["features"], anns):
        s = shape(f["geometry"])
        bad_g += int(not s.is_valid)
        if s.geom_type in ("Polygon", "MultiPolygon"):
            bad_g += int(abs(s.area - a["area"]) > 1e-3)
        cat = coco.loadCats(a["category_id"])[0]["name"]
        name = f["properties"].get("classification", {}).get("name", "Unclassified")
        bad_g += int(cat != name)
        bad_g += int(f["properties"]["objectType"] != "annotation")
    report("GeoJSON validity, areas and classes vs shapely and the COCO file", bad_g, len(g["features"]))

    import tifffile

    with tifffile.TiffFile(os.path.join(D, "labels.ome.tif")) as t:
        s = t.series[0]
        levels = [lv.asarray() for lv in s.levels]
        ome = t.ome_metadata or ""
    whole = np.fromfile(os.path.join(D, "labels.u8"), dtype=np.uint8).reshape(240, 320)
    bad_t = int((levels[0] != whole).sum())
    report("OME-TIFF level 0 vs the painted labels (tifffile)", bad_t, whole.size,
           f"{len(levels)} levels {[lv.shape for lv in levels]}, OME-XML {'yes' if '<OME' in ome else 'no'}")


def labelmap():
    meta_path = os.path.join(D, "labelmap.json")
    if not os.path.exists(meta_path):
        print("skip labelmap: no labelmap.json")
        return
    import nibabel as nib
    import SimpleITK as sitk

    meta = json.load(open(meta_path))
    for case in meta["cases"]:
        name = case["name"]
        img = nib.load(os.path.join(D, case["nifti"]))
        lab = np.asarray(img.dataobj).astype(np.int64)
        ras = img.affine  # nibabel's precedence: sform, then qform
        lps_from_ras = np.diag([-1.0, -1.0, 1.0, 1.0])
        # The DICOM grid: voxel (i, j, k) -> LPS.
        g = np.array(case["target_affine"]).reshape(4, 4)
        dims = case["target_dims"]
        ours = np.fromfile(os.path.join(D, case["ours"]), dtype="<u2").reshape(dims[2], dims[1], dims[0])
        # nibabel: target voxel -> LPS -> RAS -> label voxel, nearest.
        # The flip is its own inverse: RAS from LPS is the same diagonal.
        vox_from_lps = np.linalg.inv(ras) @ lps_from_ras
        k, j, i = np.meshgrid(np.arange(dims[2]), np.arange(dims[1]), np.arange(dims[0]), indexing="ij")
        pts = np.stack([i.ravel(), j.ravel(), k.ravel(), np.ones(i.size)])
        src = vox_from_lps @ g @ pts
        idx = np.floor(src[:3] + 0.5).astype(np.int64)
        inside = np.all((idx >= 0) & (idx < np.array(lab.shape[:3])[:, None]), axis=0)
        theirs = np.zeros(i.size, dtype=np.int64)
        theirs[inside] = lab[idx[0, inside], idx[1, inside], idx[2, inside]]
        theirs = theirs.reshape(dims[2], dims[1], dims[0])
        # Ours holds label values; voxels exactly halfway between two source voxels may round
        # either way, so those are counted separately.
        frac = np.abs((src[:3] + 0.5) - np.floor(src[:3] + 0.5) - 0.5).min(axis=0).reshape(theirs.shape)
        tie = frac > 0.5 - 1e-6
        bad = int(((ours != theirs) & ~tie).sum())
        report(f"labelmap {name} vs nibabel affine nearest-neighbour", bad, ours.size,
               f"({int(tie.sum())} voxels on a tie)")
        if not case["itk"]:
            print(f"     labelmap {name}: SimpleITK not compared ({case['nifti_form']})")
            continue
        # SimpleITK: the NIfTI read by ITK (LPS), resampled onto the target grid.
        moving = sitk.ReadImage(os.path.join(D, case["nifti"]))
        ref = sitk.Image(dims[0], dims[1], dims[2], sitk.sitkUInt16)
        spacing = np.linalg.norm(g[:3, :3], axis=0)
        ref.SetSpacing(spacing.tolist())
        ref.SetOrigin(g[:3, 3].tolist())
        ref.SetDirection((g[:3, :3] / spacing).ravel().tolist())
        res = sitk.Resample(moving, ref, sitk.Transform(), sitk.sitkNearestNeighbor, 0, sitk.sitkUInt16)
        itk = sitk.GetArrayFromImage(res).astype(np.int64)
        bad = int(((ours != itk) & ~tie).sum())
        report(f"labelmap {name} vs SimpleITK Resample (nearest)", bad, ours.size,
               f"(ITK direction from {case['nifti_form']})")


stains()
macenko()
annotations()
labelmap()
print("FAILED: " + ", ".join(failed) if failed else "all checks passed")
sys.exit(1 if failed else 0)
