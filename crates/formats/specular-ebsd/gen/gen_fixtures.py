#!/usr/bin/env python3
"""Writes the synthetic EBSD fixtures in tests/data (standard library only, deterministic).

  square.ang  40 × 30 square grid, one cubic phase (nickel), 10 columns without headers,
              some points not indexed (CI = -1)
  hex.ang     hexagonal grid (31 / 30 points per row, 24 rows), titanium (6/mmm) and iron
              (m-3m), COLUMN_COUNT / COLUMN_HEADERS present
  mixed.ctf   40 × 30, nickel (Laue 11) and magnesium (Laue 9), some points not indexed
              (phase 0), Euler angles in degrees

Grains are Voronoi cells with random orientations plus a little scatter.

Usage: python3 -I gen/gen_fixtures.py [OUT_DIR]   (default: tests/data next to gen/)
"""

import math
import os
import random
import sys


def random_euler(rng):
    # Uniform on SO(3): φ1, φ2 uniform, cos Φ uniform.
    return (
        rng.uniform(0, 2 * math.pi),
        math.acos(rng.uniform(-1, 1)),
        rng.uniform(0, 2 * math.pi),
    )


def grains(rng, n, w, h, phases):
    seeds = []
    for _ in range(n):
        seeds.append((rng.uniform(0, w), rng.uniform(0, h), random_euler(rng), rng.choice(phases)))
    return seeds


def nearest(seeds, x, y):
    return min(seeds, key=lambda s: (s[0] - x) ** 2 + (s[1] - y) ** 2)


def scatter(rng, e, amount=0.01):
    return tuple(v + rng.gauss(0, amount) for v in e)


def ang_header(phases, grid, xstep, ystep, odd, even, rows, extra=""):
    out = [
        "# TEM_PIXperUM          1.000000",
        "# x-star                0.500000",
        "# y-star                0.400000",
        "# z-star                0.650000",
        "# WorkingDistance       15.000000",
        "#",
    ]
    for pid, name, sym, lat in phases:
        out += [
            f"# Phase {pid}",
            f"# MaterialName  \t{name}",
            f"# Formula     \t{name[:2]}",
            "# Info          ",
            f"# Symmetry              {sym}",
            f"# LatticeConstants      {lat}",
            "# NumberFamilies        1",
            "# hklFamilies   \t 1  1  1 1 0.000000 1",
            "# Categories0 0 0 0 0 ",
            "#",
        ]
    out += [
        f"# GRID: {grid}",
        f"# XSTEP: {xstep:.6f}",
        f"# YSTEP: {ystep:.6f}",
        f"# NCOLS_ODD: {odd}",
        f"# NCOLS_EVEN: {even}",
        f"# NROWS: {rows}",
        "#",
        "# OPERATOR: \tsynthetic",
        "#",
        "# SAMPLEID: \tfixture",
        "#",
        "# SCANID: \tspecular test",
        "#",
    ]
    if extra:
        out.append(extra)
    return "\n".join(out) + "\n"


def square_ang(rng):
    w, h, step = 40, 30, 0.5
    seeds = grains(rng, 12, w * step, h * step, [1])
    lines = []
    for r in range(h):
        for c in range(w):
            x, y = c * step, r * step
            s = nearest(seeds, x, y)
            e = scatter(rng, s[2])
            iq = 1500 + 800 * rng.random()
            ci = rng.random()
            if rng.random() < 0.05:
                ci = -1.0
                e = (4 * math.pi, 4 * math.pi, 4 * math.pi) if rng.random() < 0.5 else e
            # φ1 in 0..2π as OIM writes it.
            e = (e[0] % (2 * math.pi), e[1], e[2] % (2 * math.pi)) if ci >= 0 else e
            lines.append(
                f"  {e[0]:.5f}   {e[1]:.5f}   {e[2]:.5f}"
                f"     {x:.5f}     {y:.5f}  {iq:.1f}  {ci:.3f}  0  {rng.random() * 1000:.3f}  {rng.random() * 2:.3f}"
            )
    head = ang_header([(1, "Nickel", 43, "3.520 3.520 3.520  90.000  90.000  90.000")], "SqrGrid", step, step, w, w, h)
    return head + "\n".join(lines) + "\n"


def hex_ang(rng):
    odd, rows, xstep = 31, 24, 1.0
    ystep = xstep * math.sqrt(3) / 2
    seeds = grains(rng, 10, odd * xstep, rows * ystep, [1, 2])
    lines = []
    for r in range(rows):
        n = odd if r % 2 == 0 else odd - 1
        x0 = 0.0 if r % 2 == 0 else xstep / 2
        for c in range(n):
            x, y = x0 + c * xstep, r * ystep
            s = nearest(seeds, x, y)
            e = scatter(rng, s[2], 0.005)
            e = (e[0] % (2 * math.pi), e[1], e[2] % (2 * math.pi))
            ci = rng.random()
            if rng.random() < 0.03:
                ci = -1.0
            lines.append(
                f"  {e[0]:.5f}   {e[1]:.5f}   {e[2]:.5f}"
                f"     {x:.5f}     {y:.5f}  {1000 + 500 * rng.random():.1f}  {ci:.3f}  {s[3]}  {rng.random() * 800:.3f}  {rng.random():.3f}"
            )
    head = ang_header(
        [
            (1, "Titanium", 62, "2.950 2.950 4.686  90.000  90.000 120.000"),
            (2, "Iron", 43, "2.870 2.870 2.870  90.000  90.000  90.000"),
        ],
        "HexGrid",
        xstep,
        ystep,
        odd,
        odd - 1,
        rows,
        "# COLUMN_COUNT: 10\n# COLUMN_HEADERS: phi1, PHI, phi2, x, y, IQ, CI, Phase index, SEM, Fit\n"
        "# COLUMN_UNITS: radians, radians, radians, microns, microns, , , , , degrees\n#",
    )
    return head + "\n".join(lines) + "\n"


def mixed_ctf(rng):
    w, h, step = 40, 30, 0.25
    seeds = grains(rng, 14, w * step, h * step, [1, 2])
    out = [
        "Channel Text File",
        "Prj\tC:\\data\\fixture.cpr",
        "Author\t[Unknown]",
        "JobMode\tGrid",
        f"XCells\t{w}",
        f"YCells\t{h}",
        f"XStep\t{step}",
        f"YStep\t{step}",
        "AcqE1\t0",
        "AcqE2\t0",
        "AcqE3\t0",
        "Euler angles refer to Sample Coordinate system (CS0)!\tMag\t2000\tCoverage\t100\tDevice\t0\tKV\t20\tTiltAngle\t70\tTiltAxis\t0",
        "Phases\t2",
        "3.524;3.524;3.524\t90;90;90\tNickel\t11\t225\t\t\tsynthetic",
        "3.209;3.209;5.211\t90;90;120\tMagnesium\t9\t194\t\t\tsynthetic",
        "Phase\tX\tY\tBands\tError\tEuler1\tEuler2\tEuler3\tMAD\tBC\tBS",
    ]
    for r in range(h):
        for c in range(w):
            x, y = c * step, r * step
            s = nearest(seeds, x, y)
            e = [math.degrees(v) for v in scatter(rng, s[2], 0.004)]
            e = [e[0] % 360, e[1], e[2] % 360]
            phase, err = s[3], 0
            if rng.random() < 0.05:
                phase, err, e = 0, 3, [0.0, 0.0, 0.0]
            out.append(
                f"{phase}\t{x:.4f}\t{y:.4f}\t{rng.randint(5, 10)}\t{err}\t"
                f"{e[0]:.4f}\t{e[1]:.4f}\t{e[2]:.4f}\t{rng.random():.4f}\t{rng.randint(50, 200)}\t{rng.randint(60, 255)}"
            )
    return "\r\n".join(out) + "\r\n"


def main():
    here = os.path.dirname(os.path.abspath(__file__))
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "tests", "data")
    os.makedirs(out, exist_ok=True)
    rng = random.Random(25178)
    for name, make in [("square.ang", square_ang), ("hex.ang", hex_ang), ("mixed.ctf", mixed_ctf)]:
        with open(os.path.join(out, name), "w", newline="") as f:
            f.write(make(rng))


if __name__ == "__main__":
    main()
