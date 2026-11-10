#!/usr/bin/env python3
"""The OCIO oracle for specular-ocio: configs, LUT files and PyOpenColorIO's answers.

Usage: ocio-oracle.py OUT_DIR

Writes into OUT_DIR:
  * the ACES cg and studio configs (ACES 1.3) that PyOpenColorIO carries built in,
    serialised to .ocio files (no download);
  * luts/: .cube (1D + 3D), .spi1d, .spi3d and CLF files made here, and files.ocio, a
    config whose colour spaces use them and every other transform type read in-tree;
  * expected.jsonl: per case (config, input colour space, display and view, or a
    destination colour space), test ramps (greys from 2^-12 to 2^8, colours) and
    PyOpenColorIO's CPU output for them.

Run with a Python that has PyOpenColorIO (pip install opencolorio) and numpy.
"""
import json
import os
import sys

import numpy as np
import PyOpenColorIO as ocio

OUT = sys.argv[1]
os.makedirs(os.path.join(OUT, "luts"), exist_ok=True)
rng = np.random.default_rng(7)

CONFIGS = {
    "cg": "cg-config-v2.2.0_aces-v1.3_ocio-v2.4",
    "studio": "studio-config-v2.2.0_aces-v1.3_ocio-v2.4",
}
for short, name in CONFIGS.items():
    cfg = ocio.Config.CreateFromBuiltinConfig(name)
    with open(os.path.join(OUT, f"{short}.ocio"), "w") as f:
        f.write(cfg.serialize())

# Ramps: greys over 20 stops, saturated colours, random colours up to 16, a few negatives.
greys = [[2.0**e] * 3 for e in np.arange(-12, 8.01, 0.5)]
colours = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [1, 1, 0], [0, 1, 1], [1, 0, 1], [0.18, 0.18, 0.18]]
rand = rng.uniform(0, 1, (60, 3)) ** 3 * 16
ramp = np.array(greys + colours + rand.tolist() + [[0, 0, 0]], dtype=np.float64)
unit = np.concatenate([rng.uniform(0, 1, (80, 3)), np.linspace(0, 1, 11)[:, None].repeat(3, 1)])

# LUT files.
def write(name, text):
    with open(os.path.join(OUT, "luts", name), "w") as f:
        f.write(text)


n = 9
cube = ["LUT_1D_SIZE 5", "LUT_3D_SIZE %d" % n]
for i in range(5):
    v = (i / 4) ** 0.8
    cube.append(f"{v:.6f} {v:.6f} {v:.6f}")
for b in range(n):
    for g in range(n):
        for r in range(n):
            x = np.array([r, g, b]) / (n - 1)
            y = np.clip([x[0] * 0.9 + x[1] * 0.1, x[1] ** 1.2, x[2] * 0.8 + 0.1 * x[0]], 0, 1)
            cube.append("%.6f %.6f %.6f" % tuple(y))
write("look.cube", "\n".join(cube) + "\n")
spi1d = ["Version 1", "From -0.125 1.125", "Length 33", "Components 1", "{"]
for i in range(33):
    x = -0.125 + 1.25 * i / 32
    spi1d.append(f"{max(x, 0) ** 0.45:.7f}")
spi1d.append("}")
write("curve.spi1d", "\n".join(spi1d) + "\n")
m = 7
spi3d = ["SPILUT 1.0", "3 3", f"{m} {m} {m}"]
for r in range(m):
    for g in range(m):
        for b in range(m):
            x = np.array([r, g, b]) / (m - 1)
            y = [x[1], x[2], x[0]]
            spi3d.append(f"{r} {g} {b} {y[0]:.6f} {y[1]:.6f} {y[2]:.6f}")
write("rotate.spi3d", "\n".join(spi3d) + "\n")
clf = """<?xml version="1.0" encoding="UTF-8"?>
<ProcessList id="sci2" compCLFversion="3.0">
  <Matrix inBitDepth="32f" outBitDepth="32f"><Array dim="3 3">0.8 0.15 0.05 0.1 0.85 0.05 0.02 0.08 0.9</Array></Matrix>
  <LUT1D inBitDepth="32f" outBitDepth="32f"><Array dim="5 1">0 0.3 0.55 0.8 1</Array></LUT1D>
  <Range inBitDepth="32f" outBitDepth="32f"><minInValue>0</minInValue><maxInValue>1</maxInValue><minOutValue>0.05</minOutValue><maxOutValue>0.95</maxOutValue></Range>
  <ASC_CDL inBitDepth="32f" outBitDepth="32f" style="Fwd"><SOPNode><Slope>1.1 1.0 0.9</Slope><Offset>0.01 0 -0.01</Offset><Power>1.2 1.0 0.9</Power></SOPNode><SatNode><Saturation>0.9</Saturation></SatNode></ASC_CDL>
</ProcessList>
"""
write("grade.clf", clf)

files_cfg = """ocio_profile_version: 2.1
search_path: luts
roles:
  default: lin
  scene_linear: lin
displays:
  Test:
    - !<View> {name: Cube, colorspace: cube_cs}
    - !<View> {name: Curve, colorspace: spi1d_cs}
active_displays: [Test]
active_views: [Cube, Curve]
colorspaces:
  - !<ColorSpace>
    name: lin
  - !<ColorSpace>
    name: cube_cs
    from_scene_reference: !<FileTransform> {src: look.cube, interpolation: linear}
  - !<ColorSpace>
    name: cube_tetra
    from_scene_reference: !<FileTransform> {src: look.cube, interpolation: tetrahedral}
  - !<ColorSpace>
    name: spi1d_cs
    from_scene_reference: !<FileTransform> {src: curve.spi1d}
  - !<ColorSpace>
    name: spi3d_cs
    from_scene_reference: !<FileTransform> {src: rotate.spi3d, interpolation: linear}
  - !<ColorSpace>
    name: clf_cs
    from_scene_reference: !<FileTransform> {src: grade.clf}
  - !<ColorSpace>
    name: exp_cs
    from_scene_reference: !<ExponentTransform> {value: [2.2, 2.0, 1.8, 1], direction: inverse}
  - !<ColorSpace>
    name: log_cs
    from_scene_reference: !<LogAffineTransform> {base: 10, log_side_slope: 0.25, log_side_offset: 0.6, lin_side_slope: 1, lin_side_offset: 0.01}
  - !<ColorSpace>
    name: logcam_cs
    from_scene_reference: !<LogCameraTransform> {base: 2, log_side_slope: 0.0570776, log_side_offset: 0.554795, lin_side_slope: 1, lin_side_offset: 0, lin_side_break: 0.0078125}
  - !<ColorSpace>
    name: log2_cs
    from_scene_reference: !<LogTransform> {base: 2}
  - !<ColorSpace>
    name: range_cs
    from_scene_reference: !<RangeTransform> {min_in_value: 0, max_in_value: 2, min_out_value: 0.1, max_out_value: 0.9}
  - !<ColorSpace>
    name: cdl_cs
    from_scene_reference: !<CDLTransform> {slope: [1.2, 1, 0.8], offset: [0.02, 0, -0.01], power: [1.1, 1, 0.9], sat: 1.2}
  - !<ColorSpace>
    name: matrix_cs
    from_scene_reference: !<MatrixTransform> {matrix: [0.9, 0.1, 0, 0, 0.05, 0.9, 0.05, 0, 0, 0.2, 0.8, 0, 0, 0, 0, 1], offset: [0.01, 0, 0.02, 0]}
  - !<ColorSpace>
    name: group_cs
    from_scene_reference: !<GroupTransform>
      children:
        - !<MatrixTransform> {matrix: [2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1]}
        - !<ExponentWithLinearTransform> {gamma: 2.4, offset: 0.055, direction: inverse}
  - !<ColorSpace>
    name: cst_cs
    from_scene_reference: !<ColorSpaceTransform> {src: lin, dst: group_cs}
"""
with open(os.path.join(OUT, "files.ocio"), "w") as f:
    f.write(files_cfg)

cases = []


def run(cfg_name, proc, inputs, **what):
    cpu = proc.getDefaultCPUProcessor()
    out = [list(cpu.applyRGB([float(v) for v in x])) for x in inputs]
    cases.append({"config": cfg_name, **what, "in": inputs.tolist(), "out": out})


for short in CONFIGS:
    cfg = ocio.Config.CreateFromFile(os.path.join(OUT, f"{short}.ocio"))
    for src in ["ACEScg", "ACES2065-1", "ACEScct", "Linear Rec.709 (sRGB)", "sRGB - Texture", "Raw"]:
        try:
            cfg.getColorSpace(src).getName()
        except Exception:
            continue
        inputs = unit if src in ("ACEScct", "sRGB - Texture") else ramp
        for disp in ["sRGB - Display", "Display P3 - Display", "Rec.1886 Rec.709 - Display", "P3-D65 - Display"]:
            for view in ["ACES 1.0 - SDR Video", "Un-tone-mapped", "Raw"]:
                try:
                    p = cfg.getProcessor(src, disp, view, ocio.TRANSFORM_DIR_FORWARD)
                except Exception:
                    continue
                run(short, p, inputs, src=src, display=disp, view=view)

cfg = ocio.Config.CreateFromFile(os.path.join(OUT, "files.ocio"))
for dst in ["cube_cs", "cube_tetra", "spi1d_cs", "spi3d_cs", "clf_cs", "exp_cs", "log_cs",
            "logcam_cs", "log2_cs", "range_cs", "cdl_cs", "matrix_cs", "group_cs", "cst_cs"]:
    inputs = unit if dst not in ("log_cs", "logcam_cs", "log2_cs", "exp_cs", "matrix_cs") else ramp[:-1]
    run("files", cfg.getProcessor("lin", dst), inputs, src="lin", dst=dst)

with open(os.path.join(OUT, "expected.jsonl"), "w") as f:
    for c in cases:
        f.write(json.dumps(c) + "\n")
print(f"{len(cases)} OCIO cases")
