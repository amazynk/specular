#!/usr/bin/env python3
"""Generate crates/formats/specular-vp9/src/txfm_gen.rs: the 1D inverse transforms of the VP9
specification (section 8.7.1) as straight-line code.

The script runs the specification's processes step by step (the inverse DCT array permutation and
the recursive inverse DCT, the ADST input/output permutations, ADST4/8/16 with the butterfly
functions B, H, SB and SH) and records every butterfly as a line of Rust instead of computing it,
so the generated functions do exactly what the specification says, in its order, without loops
or index arithmetic at run time. The functions are generic over a lane type (`Lane`) so the same
code runs on SIMD vectors of 32-bit (8-bit video) or 64-bit (high bit depth) integers.

Usage: tools/corpus/specular-vp9-txfm.py
"""
import os
import subprocess

ROOT = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
OUT = os.path.join(ROOT, "crates/formats/specular-vp9/src/txfm_gen.rs")

# cos64_lookup from section 8.7.1.1 (also generated into tables.rs from the PDF; the unit tests
# check the two agree).
COS64 = [16384, 16364, 16305, 16207, 16069, 15893, 15679, 15426, 15137, 14811, 14449, 14053,
         13623, 13160, 12665, 12140, 11585, 11003, 10394, 9760, 9102, 8423, 7723, 7005, 6270,
         5520, 4756, 3981, 3196, 2404, 1606, 804, 0]
SINPI = {"SINPI_1_9": 5283, "SINPI_2_9": 9929, "SINPI_3_9": 13377, "SINPI_4_9": 15212}


def cos64(angle):
    a = angle & 127
    if a <= 32:
        return COS64[a]
    if a <= 64:
        return -COS64[64 - a]
    if a <= 96:
        return -COS64[a - 64]
    return COS64[128 - a]


def sin64(angle):
    return cos64(angle - 32)


def brev(nbits, x):
    return sum(((x >> i) & 1) << (nbits - 1 - i) for i in range(nbits))


class Gen:
    def __init__(self, size):
        self.lines = []
        self.size = size
        self.tmp = 0

    def emit(self, s):
        self.lines.append(s)

    def mul(self, v, c):
        if c == 0:
            return None
        if c == 1:
            return v
        if c == -1:
            return f"(-{v})"
        return f"{v}.mulc({c})"

    def comb(self, terms):
        """Sum of (sign, expr) pairs, skipping None (zero) terms."""
        out = ""
        for sign, e in terms:
            if e is None:
                continue
            if not out:
                out = e if sign > 0 else f"(-{e})"
            else:
                out += (" + " if sign > 0 else " - ") + e
        return out if out else "V::zero()"

    def rot(self, a, b, angle, arr_in, arr_out, rnd):
        """x = A*cos - B*sin; y = A*sin + B*cos, from arr_in[a], arr_in[b]."""
        c, s = cos64(angle), sin64(angle)
        ta, tb = f"{arr_in}[{a}]", f"{arr_in}[{b}]"
        n = self.tmp
        self.tmp += 1
        if abs(c) == abs(s) and c != 0:
            # angle = 16 + 32k: (A - B) * c or (A + B) * c, as section 8.7.1.1 allows.
            if s == c:
                x = f"({ta} - {tb}).mulc({c})"
                y = f"({ta} + {tb}).mulc({c})"
            else:  # s == -c
                x = f"({ta} + {tb}).mulc({c})"
                y = f"({tb} - {ta}).mulc({c})"
        else:
            x = self.comb([(1, self.mul(ta, c)), (-1, self.mul(tb, s))])
            y = self.comb([(1, self.mul(ta, s)), (1, self.mul(tb, c))])
        if rnd:
            x, y = f"({x}).r14()", f"({y}).r14()"
        self.emit(f"let x{n} = {x};")
        self.emit(f"let y{n} = {y};")
        return f"x{n}", f"y{n}"

    def B(self, a, b, angle, flip):
        x, y = self.rot(a, b, angle, "t", "t", True)
        if flip:
            x, y = y, x
        self.emit(f"t[{a}] = {x};")
        self.emit(f"t[{b}] = {y};")

    def H(self, a, b, flip):
        if flip:
            a, b = b, a
        n = self.tmp
        self.tmp += 1
        self.emit(f"let h{n} = t[{a}];")
        self.emit(f"t[{a}] = h{n} + t[{b}];")
        self.emit(f"t[{b}] = h{n} - t[{b}];")

    def SB(self, a, b, angle, flip):
        x, y = self.rot(a, b, angle, "t", "s", False)
        if flip:
            x, y = y, x
        self.emit(f"s[{a}] = {x};")
        self.emit(f"s[{b}] = {y};")

    def SH(self, a, b):
        n = self.tmp
        self.tmp += 1
        self.emit(f"let h{n} = s[{a}];")
        self.emit(f"t[{a}] = (h{n} + s[{b}]).r14();")
        self.emit(f"t[{b}] = (h{n} - s[{b}]).r14();")

    def perm(self, src):
        """t[i] = copy[src[i]]."""
        if all(src[i] == i for i in range(len(src))):
            return
        n = self.tmp
        self.tmp += 1
        self.emit(f"let c{n} = *t;")
        for i, j in enumerate(src):
            if i != j:
                self.emit(f"t[{i}] = c{n}[{j}];")


def disjoint(pairs):
    seen = set()
    for a, b in pairs:
        assert a not in seen and b not in seen and a != b, pairs
        seen.update((a, b))


def idct_core(g, n):
    n0, n1, n2, n3 = 1 << n, 1 << (n - 1), 1 << (n - 2), (1 << (n - 3)) if n >= 3 else 0
    if n == 2:
        g.B(0, 1, 16, 1)
    else:
        idct_core(g, n - 1)
    ops = [(n1 + i, n0 - 1 - i, 32 - brev(5, n1 + i)) for i in range(n2)]
    disjoint([(a, b) for a, b, _ in ops])
    for a, b, ang in ops:
        g.B(a, b, ang, 0)
    if n >= 3:
        ops = [(n1 + 4 * i + 2 * j, n1 + 1 + 4 * i + 2 * j, j) for i in range(n3) for j in range(2)]
        disjoint([(a, b) for a, b, _ in ops])
        for a, b, f in ops:
            g.H(a, b, f)
    if n == 5:
        ops = [(n0 - n + 3 - n2 * j - 4 * i, n1 + n - 4 + n2 * j + 4 * i, 28 - 16 * i + 56 * j)
               for i in range(2) for j in range(2)]
        disjoint([(a, b) for a, b, _ in ops])
        for a, b, ang in ops:
            g.B(a, b, ang, 1)
        ops = [(n1 + n3 * j + i, n1 + n2 - 5 + n3 * j - i, j & 1) for i in range(2) for j in range(4)]
        disjoint([(a, b) for a, b, _ in ops])
        for a, b, f in ops:
            g.H(a, b, f)
    if n >= 4:
        ops = [(n0 - n + 2 - i - n2 * j, n1 + n - 3 + i + n2 * j, 24 + 48 * j)
               for i in range(1 + (n == 5)) for j in range(2)]
        disjoint([(a, b) for a, b, _ in ops])
        for a, b, ang in ops:
            g.B(a, b, ang, 1)
        ops = [(n1 + n2 * j + i, n1 + n2 - 1 + n2 * j - i, j & 1)
               for i in range(2 * n - 7 + 1) for j in range(2)]
        disjoint([(a, b) for a, b, _ in ops])
        for a, b, f in ops:
            g.H(a, b, f)
    if n >= 3:
        ops = [(n0 - n3 - 1 - i, n1 + n3 + i) for i in range(n3)]
        disjoint(ops)
        for a, b in ops:
            g.B(a, b, 16, 1)
    ops = [(i, n0 - 1 - i) for i in range(n1)]
    disjoint(ops)
    for a, b in ops:
        g.H(a, b, 0)


def idct(n):
    g = Gen(1 << n)
    g.perm([brev(n, i) for i in range(1 << n)])
    idct_core(g, n)
    return g


def adst_in_perm(g, n):
    n0, n1 = 1 << n, 1 << (n - 1)
    src = [0] * n0
    for i in range(n1):
        src[2 * i] = n0 - 1 - 2 * i
        src[2 * i + 1] = 2 * i
    g.perm(src)


def adst_out_perm(g, n):
    src = [0] * (1 << n)
    if n == 4:
        for a in range(2):
            for b in range(2):
                for c in range(2):
                    for d in range(2):
                        src[8 * a + 4 * b + 2 * c + d] = 8 * (d ^ c) + 4 * (c ^ b) + 2 * (b ^ a) + a
    else:
        for a in range(2):
            for b in range(2):
                for c in range(2):
                    src[4 * a + 2 * b + c] = 4 * (c ^ b) + 2 * (b ^ a) + a
    g.perm(src)


def adst4():
    g = Gen(4)
    e = g.emit
    k = SINPI
    e(f"let s0 = t[0].mulc({k['SINPI_1_9']});")
    e(f"let s1 = t[0].mulc({k['SINPI_2_9']});")
    e(f"let s2 = t[1].mulc({k['SINPI_3_9']});")
    e(f"let s3 = t[2].mulc({k['SINPI_4_9']});")
    e(f"let s4 = t[2].mulc({k['SINPI_1_9']});")
    e(f"let s5 = t[3].mulc({k['SINPI_2_9']});")
    e(f"let s6 = t[3].mulc({k['SINPI_4_9']});")
    e("let v = t[0] - t[2] + t[3];")
    e(f"let s7 = v.mulc({k['SINPI_3_9']});")
    e("let x0 = s0 + s3 + s5;")
    e("let x1 = s1 - s4 - s6;")
    e("let x2 = s7;")
    e("let x3 = s2;")
    e("t[0] = (x0 + x3).r14();")
    e("t[1] = (x1 + x3).r14();")
    e("t[2] = x2.r14();")
    e("t[3] = (x0 + x1 - x3).r14();")
    return g


def adst8():
    g = Gen(8)
    adst_in_perm(g, 3)
    for i in range(4):
        g.SB(2 * i, 1 + 2 * i, 30 - 8 * i, 1)
    for i in range(4):
        g.SH(i, 4 + i)
    for i in range(2):
        g.SB(4 + 3 * i, 5 + i, 24 - 16 * i, 1)
    for i in range(2):
        g.SH(4 + i, 6 + i)
    for i in range(2):
        g.H(i, 2 + i, 0)
    for i in range(2):
        g.B(2 + 4 * i, 3 + 4 * i, 16, 1)
    adst_out_perm(g, 3)
    for i in range(4):
        g.emit(f"t[{1 + 2 * i}] = -t[{1 + 2 * i}];")
    return g


def adst16():
    g = Gen(16)
    adst_in_perm(g, 4)
    for i in range(8):
        g.SB(2 * i, 1 + 2 * i, 31 - 4 * i, 1)
    for i in range(8):
        g.SH(i, 8 + i)
    for i in range(4):
        g.SB(8 + 2 * i, 9 + 2 * i, 28 - 16 * i, 1)
    for i in range(4):
        g.SH(8 + i, 12 + i)
    for i in range(4):
        g.H(i, 4 + i, 0)
    for i in range(2):
        for j in range(2):
            g.SB(4 + 8 * i + 3 * j, 5 + 8 * i + j, 24 - 16 * j, 1)
    for i in range(2):
        for j in range(2):
            g.SH(4 + 8 * j + i, 6 + 8 * j + i)
    for i in range(2):
        for j in range(2):
            g.H(8 * j + i, 2 + 8 * j + i, 0)
    for i in range(2):
        for j in range(2):
            g.B(2 + 4 * j + 8 * i, 3 + 4 * j + 8 * i, 48 + 64 * (i ^ j), 0)
    adst_out_perm(g, 4)
    for i in range(2):
        for j in range(2):
            g.emit(f"t[{1 + 12 * j + 2 * i}] = -t[{1 + 12 * j + 2 * i}];")
    return g


def func(name, g, doc):
    uses_s = any("s[" in l for l in g.lines)
    body = []
    if uses_s:
        body.append(f"let mut s = [V::zero(); {g.size}];")
    body += g.lines
    out = [f"/// {doc}", "#[inline(always)]",
           f"pub(crate) fn {name}<V: Lane>(t: &mut [V; {g.size}]) {{"]
    out += ["    " + l for l in body]
    out.append("}")
    return "\n".join(out)


def main():
    parts = [
        "//! The 1D inverse transforms of the VP9 specification (section 8.7.1) as straight-line",
        "//! code, generated by `tools/corpus/specular-vp9-txfm.py`. Do not edit.",
        "#![allow(clippy::all)]",
        "",
        "use crate::itx::Lane;",
        "",
    ]
    for n in range(2, 6):
        parts.append(func(f"idct{1 << n}", idct(n),
                          f"Inverse DCT of length {1 << n} (permutation and process, 8.7.1.2-3)."))
        parts.append("")
    parts.append(func("iadst4", adst4(), "Inverse ADST4 (8.7.1.6)."))
    parts.append("")
    parts.append(func("iadst8", adst8(), "Inverse ADST8 (8.7.1.7)."))
    parts.append("")
    parts.append(func("iadst16", adst16(), "Inverse ADST16 (8.7.1.8)."))
    parts.append("")
    with open(OUT, "w") as f:
        f.write("\n".join(parts))
    try:
        subprocess.run(["rustfmt", "--edition", "2024", OUT], check=True)
    except (OSError, subprocess.CalledProcessError) as e:
        print(f"rustfmt failed: {e}")
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
