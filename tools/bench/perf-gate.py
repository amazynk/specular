#!/usr/bin/env python3
"""Instruction-count speed gate for Specular's decoders.

Runs every case of `specular-bench gate` (one single-threaded decode per process) under valgrind's
callgrind, counting only the decode itself (`--toggle-collect=*perfgate_measure*`). It writes the
counts to a results JSON, compares them with the committed baseline and fails when any case
needs more than --threshold (default 2 %) more instructions.

  tools/bench/perf-gate.py                    # build, run, compare (what CI does)
  tools/bench/perf-gate.py --only jpeg,png    # a subset (substring match on case names)
  tools/bench/perf-gate.py --bless            # run and make the results the new baseline
  tools/bench/perf-gate.py --bless-from F     # make a results file (e.g. CI's) the baseline
  tools/bench/perf-gate.py --repeat 2         # run each case twice and report the spread

Two counts are kept per case:
  ir      every instruction in the counted region, including glibc (malloc, memcpy, libm);
  ir_own  only instructions in the specular-bench binary (all Rust code: decoders and std).
A third, allocs, counts heap allocations (calls into malloc, calloc, realloc and the aligned
allocators) in the counted region; it fails at +5 %. Headline cases (headline.rs) also carry an
absolute instruction budget derived from DESIGN.md §12 (see
INSTR_PER_MS_PER_CORE) that fails whatever the baseline says.
ir_own depends only on the source and the pinned toolchain, so it is comparable across machines.
ir also depends on the glibc build, so it is gated only when the baseline was blessed in the
same environment (glibc, valgrind and rustc versions); otherwise the gate uses ir_own and says
so. Bless from CI's results artifact to gate on ir in CI.

Standard library only; needs valgrind and cargo.
"""

import argparse
import concurrent.futures
import hashlib
import json
import os
import platform
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
BASELINE = ROOT / "tools/bench/perf-gate-baseline.json"
TOGGLE = "*perfgate_measure*"
SCHEMA = 1
# Headline cases carry a budget (`gate list`'s columns 4-6: ms of a DESIGN.md §12 / §2.3
# target, the reference machine's cores the work spreads over in the app, and what the target
# is). It becomes an instruction budget at 3 instructions per ns per core: the reference is a
# mid-range 2023 laptop (DESIGN.md §12, 4 performance cores counted), whose cores retire 3-5
# per ns on this code, so a case over its budget would miss the target there too. The count
# is single-threaded, so the budget is a ceiling on the total work, whatever the split.
INSTR_PER_MS_PER_CORE = 3_000_000
# Allowed increase of a case's heap allocations (calls into malloc and friends).
ALLOC_THRESHOLD = 0.05


def target_dir() -> Path:
    t = os.environ.get("CARGO_TARGET_DIR")
    return Path(t) if t else ROOT / "target"


def run(cmd, **kw) -> str:
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kw).stdout


def environment() -> dict:
    def first_line(cmd):
        try:
            return run(cmd, cwd=ROOT).splitlines()[0].strip()
        except (OSError, subprocess.CalledProcessError, IndexError):
            return "unknown"

    osname = "unknown"
    try:
        for line in Path("/etc/os-release").read_text().splitlines():
            if line.startswith("PRETTY_NAME="):
                osname = line.split("=", 1)[1].strip('"')
    except OSError:
        pass
    cpu = platform.processor() or "unknown"
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                cpu = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    return {
        "libc": os.confstr("CS_GNU_LIBC_VERSION") if hasattr(os, "confstr") else "unknown",
        "valgrind": first_line(["valgrind", "--version"]),
        "rustc": first_line(["rustc", "-V"]),
        "os": osname,
        "cpu": cpu,  # informational: callgrind counts do not depend on the CPU model
    }


def same_env(a: dict, b: dict) -> bool:
    return all(a.get(k) == b.get(k) for k in ("libc", "valgrind", "rustc"))


def same_profile(a: dict, b: dict) -> bool:
    return a.get("profile", "release") == b.get("profile", "release")


# Heap entry points: a call from outside this set into it is one allocation (Rust's System
# allocator calls these; calls between them, e.g. realloc -> malloc, are not counted twice).
ALLOC_FNS = {"malloc", "calloc", "realloc", "posix_memalign", "aligned_alloc", "memalign"}


def parse_callgrind(path: Path, binary: Path) -> tuple[int, int, int]:
    """Total Ir, the Ir spent in `binary` itself (exclusive costs grouped by object) and the
    number of heap allocations (calls into ALLOC_FNS from other functions)."""
    total = None
    by_obj: dict[str, int] = {}
    names: dict[str, str] = {}  # compressed "(id) name" strings
    ob = ""
    fn = ""
    cfn = ""
    allocs = 0
    skip_next = False
    with path.open() as f:
        for line in f:
            if not line or line[0] == "\n":
                continue
            c = line[0]
            if c.isdigit() or c in "+-*":
                if skip_next:  # inclusive cost of the preceding calls= line
                    skip_next = False
                    continue
                parts = line.split()
                if len(parts) >= 2:
                    by_obj[ob] = by_obj.get(ob, 0) + int(parts[1])
                continue
            if line.startswith("ob="):
                ob = resolve(line[3:].strip(), names)
            elif line.startswith("calls="):
                skip_next = True
                if base_fn(cfn) in ALLOC_FNS and base_fn(fn) not in ALLOC_FNS:
                    allocs += int(line[6:].split()[0])
            elif line.startswith("fn="):
                fn = resolve(line[3:].strip(), names)
            elif line.startswith("cfn="):
                cfn = resolve(line[4:].strip(), names)
            elif line.startswith(("cob=", "cfi=", "cfl=", "fl=", "fi=", "fe=")):
                resolve(line.split("=", 1)[1].strip(), names)
            elif line.startswith(("totals:", "summary:")):
                total = int(line.split()[1])
    own = sum(v for k, v in by_obj.items() if Path(k).name == binary.name)
    if total is None:
        total = sum(by_obj.values())
    return total, own, allocs


def base_fn(name: str) -> str:
    """`malloc'2` (callgrind's recursion suffix) and `__libc_malloc` count as `malloc`."""
    name = name.split("'", 1)[0]
    return name.removeprefix("__libc_")


def resolve(s: str, names: dict) -> str:
    m = re.match(r"\((\d+)\)(?: (.*))?$", s)
    if not m:
        return s
    if m.group(2) is not None:
        names[m.group(1)] = m.group(2)
        return m.group(2)
    return names.get(m.group(1), s)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def count_case(binary: Path, name: str, file: Path, outdir: Path, rep: int) -> tuple[int, int, int]:
    out = outdir / f"{name}.{rep}.callgrind"
    env = dict(os.environ, RAYON_NUM_THREADS="1")
    cmd = [
        "valgrind",
        "--tool=callgrind",
        f"--callgrind-out-file={out}",
        "--collect-atstart=no",
        f"--toggle-collect={TOGGLE}",
        "--compress-strings=no",
        "--compress-pos=no",
        str(binary),
        "gate",
        "run",
        name,
        str(file),
    ]
    p = subprocess.run(cmd, text=True, capture_output=True, env=env)
    if p.returncode != 0:
        raise RuntimeError(f"{name}: valgrind exited with {p.returncode}\n{p.stderr[-4000:]}")
    ir, own, allocs = parse_callgrind(out, binary)
    if ir == 0:
        raise RuntimeError(f"{name}: nothing counted; is {TOGGLE} still in the binary?")
    return ir, own, allocs


def measure(args) -> dict:
    tdir = target_dir()
    pdir = "debug" if args.profile == "dev" else args.profile
    binary = Path(args.bin) if args.bin else tdir / pdir / "specular-bench"
    if not args.no_build:
        cmd = ["cargo", "build", "--profile", args.profile, "--locked", "-p", "specular-bench", "--features", "gate"]
        if args.cargo_jobs:
            cmd += ["-j", str(args.cargo_jobs)]
        print("+", " ".join(cmd), flush=True)
        subprocess.run(cmd, check=True, cwd=ROOT)
    work = tdir / "perfgate"
    inputs = work / "inputs"
    cg = work / "callgrind"
    cg.mkdir(parents=True, exist_ok=True)
    run([str(binary), "gate", "gen", str(inputs)])

    cases = []
    budgets: dict[str, tuple[float, int, str]] = {}
    for line in run([str(binary), "gate", "list"]).splitlines():
        name, kind, file, *rest = line.split("\t")
        path = inputs / file if kind == "generated" else ROOT / file
        if args.only and not any(s in name for s in args.only):
            continue
        cases.append((name, path))
        if len(rest) >= 3:
            budgets[name] = (float(rest[0]), int(rest[1]), rest[2])
    if not cases:
        sys.exit("no cases selected")

    jobs = args.jobs or max(1, min(len(cases), (os.cpu_count() or 2) // 2))
    print(f"counting {len(cases)} cases x {args.repeat} under callgrind, {jobs} at a time", flush=True)
    results: dict[str, dict] = {}
    with concurrent.futures.ThreadPoolExecutor(jobs) as pool:
        futs = {
            pool.submit(count_case, binary, n, p, cg, r): (n, r)
            for n, p in cases
            for r in range(args.repeat)
        }
        runs: dict[str, list] = {}
        for fut in concurrent.futures.as_completed(futs):
            n, _ = futs[fut]
            runs.setdefault(n, []).append(fut.result())
    for n, p in cases:
        rs = runs[n]
        irs = [r[0] for r in rs]
        owns = [r[1] for r in rs]
        results[n] = {
            "ir": min(irs),
            "ir_own": min(owns),
            "allocs": min(r[2] for r in rs),
            "input": str(p.relative_to(ROOT) if p.is_relative_to(ROOT) else p.name),
            "input_sha256": sha256(p),
        }
        if n in budgets:
            ms, cores, what = budgets[n]
            results[n]["budget_ms"] = ms
            results[n]["budget_cores"] = cores
            results[n]["budget_ir"] = int(ms * cores * INSTR_PER_MS_PER_CORE)
            results[n]["budget_for"] = what
        if len(rs) > 1:
            results[n]["ir_runs"] = sorted(irs)
            results[n]["ir_own_runs"] = sorted(owns)
    env = environment()
    env["profile"] = args.profile
    return {"schema": SCHEMA, "env": env, "cases": results}


def fmt(n) -> str:
    return "-" if n is None else f"{n:,}"


def pct(new, old) -> float | None:
    return None if not old or new is None else (new - old) / old


def compare(cur: dict, base: dict | None, threshold: float, partial: bool) -> tuple[bool, str, str]:
    lines = []
    md = []
    ok = True
    if base is None:
        lines.append("no baseline (run with --bless to create one); checking budgets only")
        base = {"env": cur["env"], "cases": {}}
    if not same_profile(cur["env"], base["env"]):
        lines.append(
            f"note: the baseline is from the {base['env'].get('profile', 'release')} profile, these counts from "
            f"{cur['env'].get('profile')}; only the budgets are checked."
        )
        base = {"env": cur["env"], "cases": {}}
    env_match = same_env(cur["env"], base["env"])
    metric = "ir" if env_match else "ir_own"
    if not env_match:
        diff = ", ".join(
            f"{k}: {base['env'].get(k)} -> {cur['env'].get(k)}"
            for k in ("libc", "valgrind", "rustc")
            if base["env"].get(k) != cur["env"].get(k)
        )
        lines.append(f"note: environment differs from the baseline's ({diff});")
        lines.append("      gating on ir_own (Rust code only). Bless in this environment to gate on ir.")
    hdr = ("case", f"baseline {metric}", f"current {metric}", "ir delta", "ir_own delta", "allocs", "budget", "status")
    rows = []
    for name, c in cur["cases"].items():
        b = base["cases"].get(name)
        budget = c.get("budget_ir")
        over_budget = budget is not None and c["ir"] > budget
        bud = "-" if budget is None else f"{c['ir'] / budget * 100:.0f} % of {budget / 1e6:g}M"
        allocs = c.get("allocs")
        if b is None:
            status = "FAIL: over budget" if over_budget else "new (not in baseline)"
            ok &= not over_budget
            rows.append((name, "-", fmt(c[metric]), "", "", fmt(allocs), bud, status))
            continue
        d = pct(c["ir"], b["ir"])
        do = pct(c["ir_own"], b["ir_own"])
        gate_d = d if metric == "ir" else do
        base_allocs = b.get("allocs")
        # Allocation counts repeat exactly for the decoders; cases that read folders and
        # hash with std's randomly seeded maps (the CT series) move by up to 2 %, hence 5 %,
        # and +2 for a one-off buffer in tiny cases.
        allocs_up = (
            allocs is not None
            and base_allocs is not None
            and allocs > base_allocs * (1 + ALLOC_THRESHOLD) + 2
        )
        if c["input_sha256"] != b["input_sha256"]:
            status = "FAIL: input changed, bless"
            ok = False
        elif over_budget:
            status = "FAIL: over budget"
            ok = False
        elif gate_d > threshold:
            status = f"FAIL: +{gate_d * 100:.2f} % > {threshold * 100:g} %"
            ok = False
        elif allocs_up:
            status = f"FAIL: allocations {base_allocs} -> {allocs}"
            ok = False
        elif gate_d < -threshold:
            status = "faster (bless to lock in)"
        else:
            status = "ok"
        al = fmt(allocs) if base_allocs is None or allocs == base_allocs else f"{base_allocs}->{allocs}"
        rows.append(
            (name, fmt(b[metric]), fmt(c[metric]), f"{d * 100:+.2f} %", f"{do * 100:+.2f} %", al, bud, status)
        )
    for name in base["cases"]:
        if name not in cur["cases"] and not partial:
            rows.append((name, fmt(base["cases"][name][metric]), "-", "", "", "", "", "gone (bless to drop)"))
    widths = [max(len(str(r[i])) for r in [hdr, *rows]) for i in range(len(hdr))]
    right = {1, 2, 3, 4, 5}

    def line(r):
        return "  ".join(
            str(v).rjust(widths[i]) if i in right else str(v).ljust(widths[i]) for i, v in enumerate(r)
        ).rstrip()

    lines.append(f"gated count: {metric}, threshold +{threshold * 100:g} %")
    lines.append(line(hdr))
    lines.append("  ".join("-" * w for w in widths))
    lines += [line(r) for r in rows]
    md.append(f"### Instruction-count gate ({'pass' if ok else 'FAIL'})\n")
    md.append(f"Gated count: `{metric}`, threshold +{threshold * 100:g} %.")
    if not env_match:
        md.append(" The baseline comes from another environment, so only Rust code is compared.")
    md.append("\n\n| " + " | ".join(hdr) + " |\n|" + "---|" * len(hdr) + "\n")
    md += ["| " + " | ".join(str(v) for v in r) + " |\n" for r in rows]
    return ok, "\n".join(lines), "".join(md)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--bless", action="store_true", help="write the results to the baseline")
    ap.add_argument("--bless-from", metavar="FILE", help="make FILE (a results JSON) the baseline")
    ap.add_argument("--baseline", default=str(BASELINE))
    ap.add_argument("--out", help="results JSON (default <target>/perfgate/results.json)")
    ap.add_argument("--markdown", metavar="FILE", help="append a Markdown table (e.g. $GITHUB_STEP_SUMMARY)")
    ap.add_argument("--threshold", type=float, default=0.02, help="allowed increase (default 0.02)")
    ap.add_argument("--only", type=lambda s: s.split(","), help="case-name substrings")
    ap.add_argument("--repeat", type=int, default=1, help="runs per case; the minimum is kept")
    ap.add_argument("-j", "--jobs", type=int, help="cases counted at once (default: half the CPUs)")
    ap.add_argument("--cargo-jobs", type=int, help="-j for the cargo build")
    ap.add_argument("--no-build", action="store_true", help="use the existing binary")
    ap.add_argument("--bin", help="specular-bench binary (default <target>/<profile>/specular-bench)")
    ap.add_argument("--profile", default="release", help="cargo profile (default release, as blessed)")
    ap.add_argument(
        "--suite",
        choices=("decode", "app", "all"),
        default="decode",
        help="decode: the instruction counts (default); app: the viewer's scripted runs in a "
        "headless sway (perfgate_app.py); all: both",
    )
    ap.add_argument("--app-bin", help="the viewer (default <target>/<profile>/specular)")
    ap.add_argument("--baseline-app", help="the app suite's baseline (default tools/bench/perf-gate-app-baseline.json)")
    args = ap.parse_args()
    baseline = Path(args.baseline)
    if args.suite in ("app", "all"):
        sys.path.insert(0, str(Path(__file__).resolve().parent))
        import perfgate_app

        app_args = argparse.Namespace(**vars(args))
        if args.suite == "all":
            app_args.out = None
        rc = perfgate_app.main(app_args, target_dir())
        if args.suite == "app":
            return rc
        args.no_build = True
        if rc:
            print("app suite failed; running the decode suite too")
    else:
        rc = 0

    if args.bless_from:
        data = json.loads(Path(args.bless_from).read_text())
        if data.get("schema") != SCHEMA:
            sys.exit(f"{args.bless_from}: not a perf-gate results file")
        write_baseline(baseline, data, merge=False)
        return 0

    cur = measure(args)
    out = Path(args.out) if args.out else target_dir() / "perfgate/results.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(cur, indent=2) + "\n")
    print(f"results: {out}")
    if args.repeat > 1:
        print("run-to-run spread (max - min) / min:")
        for n, c in cur["cases"].items():
            s = (c["ir_runs"][-1] - c["ir_runs"][0]) / c["ir_runs"][0]
            so = (c["ir_own_runs"][-1] - c["ir_own_runs"][0]) / c["ir_own_runs"][0]
            print(f"  {n:24} ir {s * 100:.5f} %   ir_own {so * 100:.5f} %")

    base = json.loads(baseline.read_text()) if baseline.exists() else None
    ok, text, md = compare(cur, base, args.threshold, bool(args.only))
    print(text)
    if args.markdown and md:
        with open(args.markdown, "a") as f:
            f.write(md)
    if args.bless:
        write_baseline(baseline, cur, merge=bool(args.only))
        return 0
    return 0 if ok and rc == 0 else 1


def write_baseline(path: Path, data: dict, merge: bool) -> None:
    data = json.loads(json.dumps(data))
    for c in data["cases"].values():  # keep the committed file small
        c.pop("ir_runs", None)
        c.pop("ir_own_runs", None)
    if merge and path.exists():
        old = json.loads(path.read_text())
        old["cases"].update(data["cases"])
        old["env"] = data["env"]
        data = old
    path.write_text(json.dumps(data, indent=2, sort_keys=False) + "\n")
    print(f"baseline written: {path} ({len(data['cases'])} cases)")


if __name__ == "__main__":
    sys.exit(main())
