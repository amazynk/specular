#!/usr/bin/env python3
"""Binary-size report for the viewer (not a gate yet: whether the
release ships stripped is not settled yet, and DESIGN.md §12's "< 30 MB (core, no C
bridges)" is read against the stripped size).

    tools/bench/binary-size.py [BIN] [--top N] [--json FILE]

BIN defaults to <target>/release/specular. Prints the file size, the stripped size (a stripped
copy is written beside the target's other files and removed), the sections, and the largest
crates by the size of their code and data symbols (from `nm`, demangled; a standard-library
generic such as drop_in_place<T> counts for the crate named inside it). Needs
binutils (size, nm, objcopy). Standard library only.
"""

import argparse
import json
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BUDGET_MB = 30.0


def target_dir() -> Path:
    t = os.environ.get("CARGO_TARGET_DIR")
    return Path(t) if t else ROOT / "target"


def mb(n: int) -> str:
    return f"{n / 1e6:.1f} MB"


def sections(binary: Path) -> list:
    out = subprocess.run(["size", "-A", str(binary)], capture_output=True, text=True, check=True).stdout
    rows = []
    for line in out.splitlines():
        parts = line.split()
        if len(parts) == 3 and parts[0].startswith(".") and parts[1].isdigit():
            rows.append((parts[0], int(parts[1])))
    return rows


CRATE = re.compile(r"^(?:<+|&|\*const |\*mut |dyn )*([A-Za-z_][A-Za-z0-9_]*)::")


INNER = re.compile(r"[<,(&\s]([A-Za-z_][A-Za-z0-9_]*)::")
STD = {"core", "alloc", "std"}


def crate_of(sym: str) -> str:
    """The crate a symbol's code belongs to: its path's first segment, or for a generic of the
    standard library (drop_in_place<T>, Vec<T>::push, ...) the first crate named inside it."""
    m = CRATE.match(sym)
    if not m:
        return "(C, no namespace)"
    c = m.group(1)
    if c in STD:
        for inner in INNER.findall(sym):
            if inner not in STD:
                return inner
    return c


def crates(binary: Path) -> dict:
    out = subprocess.run(
        ["nm", "-S", "--size-sort", "-C", "--defined-only", str(binary)],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    by = defaultdict(int)
    for line in out.splitlines():
        parts = line.split(" ", 3)
        if len(parts) < 4:
            continue
        try:
            size = int(parts[1], 16)
        except ValueError:
            continue
        name = parts[3]
        # Linker markers (__executable_start, _TLS_MODULE_BASE_, ...) span whole segments.
        if name.startswith(("__", "_TLS_", "_GLOBAL_", "_DYNAMIC")) or parts[2] not in "tTrRdDbB":
            continue
        by[crate_of(name)] += size
    return dict(by)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("bin", nargs="?", default=str(target_dir() / "release/specular"))
    ap.add_argument("--top", type=int, default=25)
    ap.add_argument("--json", help="also write the report as JSON")
    a = ap.parse_args()
    binary = Path(a.bin)
    if not binary.exists():
        sys.exit(f"{binary}: not built (cargo build --release -p specular)")
    size = binary.stat().st_size
    stripped_path = target_dir() / "binary-size.stripped"
    subprocess.run(["objcopy", "--strip-all", str(binary), str(stripped_path)], check=True)
    stripped = stripped_path.stat().st_size
    secs = sections(binary)
    by_crate = crates(binary)
    stripped_path.unlink()
    total_syms = sum(by_crate.values())
    print(f"{binary}")
    print(f"  file      {mb(size)}")
    print(f"  stripped  {mb(stripped)}   (DESIGN.md §12 budget < {BUDGET_MB:g} MB for the core; not gated)")
    print("  sections (largest):")
    for name, n in sorted(secs, key=lambda s: -s[1])[:8]:
        print(f"    {name:22} {mb(n):>10}")
    print(f"  crates by symbol size (top {a.top} of {len(by_crate)}, {mb(total_syms)} in symbols):")
    for name, n in sorted(by_crate.items(), key=lambda kv: -kv[1])[: a.top]:
        print(f"    {name:32} {mb(n):>10}  {n / max(total_syms, 1) * 100:5.1f} %")
    if a.json:
        Path(a.json).write_text(
            json.dumps(
                {
                    "binary": str(binary),
                    "file_bytes": size,
                    "stripped_bytes": stripped,
                    "budget_mb": BUDGET_MB,
                    "sections": dict(secs),
                    "crates": dict(sorted(by_crate.items(), key=lambda kv: -kv[1])),
                },
                indent=2,
            )
            + "\n"
        )
    return 0


if __name__ == "__main__":
    sys.exit(main())
