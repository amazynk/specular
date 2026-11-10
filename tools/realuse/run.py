#!/usr/bin/env python3
"""Real-use runs: the real specular binary driven through ten user workflows (scenarios.py)
in a private headless sway, with a private session bus, a private AT-SPI bus and a
FileChooser portal that answers the app's dialogs (portal.py).

Run it through tools/realuse/realuse.sh, which sets up the private buses and folders; this
script refuses to run on any other bus.

Each scenario writes $CARGO_TARGET_DIR/realuse/<label>/<scenario>/: report.json (each
check passed or failed, timings, "under load" when the load average was above a quarter of
the cores), the app's log and stderr (open-report lines), the frame-gate report and
screenshots. `--compare OTHER` prints what changed against an earlier label: checks that
changed status and timings that got twice as slow.

The app always runs with SPECULAR_NO_NETWORK=1 and a home of its own per scenario. Every run
checks the gate report (no frame without a cause), the log (no panics, no errors) and the
AT-SPI tree for dialogs.
"""

import argparse
import ctypes
import json
import os
import shutil
import signal
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools", "a11y"))
sys.path.insert(0, os.path.join(ROOT, "clients", "python"))
sys.path.insert(0, HERE)

import atspi_check as ac  # noqa: E402
from atspi_check import Atspi, GLib  # noqa: E402
import portal as portal_mod  # noqa: E402
from specular._client import SpecularError, Viewer  # noqa: E402

PR_SET_CHILD_SUBREAPER = 36
SIZE = "1200x760"


_ENGLISH = None


def english_only(lang):
    """English one-line messages whose translation into `lang` differs: seen on screen in that
    language, they are English leaks."""
    global _ENGLISH
    if _ENGLISH is None:
        base = os.path.join(ROOT, "crates", "specular-i18n", "i18n")

        def read(code):
            out = {}
            d = os.path.join(base, code)
            for n in os.listdir(d) if os.path.isdir(d) else []:
                if not n.endswith(".ftl"):
                    continue
                with open(os.path.join(d, n), encoding="utf-8") as f:
                    for line in f:
                        k, sep, v = line.partition(" = ")
                        if sep and k and not k.startswith((" ", "#", "-")) and "{" not in v:
                            out[k.strip()] = v.strip()
            return out
        en, other = read("en"), read(lang)
        _ENGLISH = {v for k, v in en.items()
                    if len(v) >= 4 and k in other and other[k] != v and any(c.isalpha() for c in v)}
    return _ENGLISH


def under_load():
    return os.getloadavg()[0] > (os.cpu_count() or 1) / 4


class Session:
    """One private sway and AT-SPI bus for the whole run; apps come and go in it."""

    def __init__(self, a):
        self.a = a
        self.r = ac.Run(a.out)
        self.base = ac.clean_env()
        self.wenv = None
        self.portal = None

    def setup(self):
        r, a = self.r, self.a
        self.portal = portal_mod.Portal()
        if not r.check("setup", "FileChooser portal on the private bus",
                       self.portal.error is None, self.portal.error):
            return False
        if not ac.a11y_bus(r, a, self.base):
            return False
        xdg = self.base["XDG_RUNTIME_DIR"]
        conf = os.path.join(a.out, "sway.conf")
        with open(conf, "w") as f:
            f.write("xwayland disable\noutput HEADLESS-1 resolution 1280x800\n"
                    "default_border none\nfocus_on_window_activation focus\n")
        senv = dict(self.base, WLR_BACKENDS="headless", WLR_LIBINPUT_NO_DEVICES="1",
                    WLR_RENDERER="pixman")
        before = set(os.listdir(xdg))
        r.spawn("sway", ["sway", "-c", conf], senv)
        sock = None
        for _ in range(100):
            new = [s for s in os.listdir(xdg) if s.startswith("wayland-")
                   and not s.endswith(".lock") and s not in before]
            if new:
                sock = new[0]
                break
            time.sleep(0.1)
        if not r.check("setup", "headless sway started", sock, "see sway.log"):
            return False
        self.wenv = dict(self.base, WAYLAND_DISPLAY=sock)
        ipc = [s for s in os.listdir(xdg) if s.startswith("sway-ipc.")]
        if ipc:
            self.wenv["SWAYSOCK"] = os.path.join(xdg, ipc[0])
        self.hold = r.spawn("wtype-hold", ac.HOLD_ARGV, self.wenv)
        self.hold_lost = []
        r.pump(1.0)
        return True

    def keyboard(self, where):
        """Keeps the seat's held virtual keyboard. Without it the seat has a keyboard only
        while a `wtype` runs, and the app's keyboard comes and goes with each one: keys sent
        at once are lost (the client gallery's Enter late in a full run). Should the holding
        `wtype` end (`ac.HOLD_ARGV`), it is started again, and said."""
        if self.hold.poll() is None:
            return True
        self.hold_lost.append((where, self.hold.returncode))
        print(f"  [keyboard] the held keyboard ended ({self.hold.returncode}) before {where};"
              " started again", flush=True)
        self.hold = self.r.spawn("wtype-hold", ac.HOLD_ARGV, self.wenv)
        self.r.pump(1.0)
        return False

    def sway(self, *args):
        return subprocess.run(["swaymsg", *args], env=self.wenv, capture_output=True,
                              text=True, timeout=10)

    def keys(self, *args, settle=0.4):
        self.keyboard("keys " + " ".join(args)[:40])
        subprocess.run(["wtype", *args], env=self.wenv, check=True, timeout=30)
        self.r.pump(settle)

    def type_text(self, text, settle=0.4):
        self.keys("-d", "15", text, settle=settle)


class App:
    """One app process in the session, with its own home, log and remote control."""

    def __init__(self, s, sc, name, args, env=None, lang=None, gate=True):
        self.s, self.sc, self.name = s, sc, name
        self.dir = sc.dir
        home = os.path.join(sc.dir, "home")
        os.makedirs(home, exist_ok=True)
        self.log = os.path.join(sc.dir, f"{name}-log.txt")
        self.gate = os.path.join(sc.dir, f"{name}-gate.json") if gate else None
        e = dict(s.wenv)
        icd = "/usr/share/vulkan/icd.d/lvp_icd.json"
        e.update(HOME=home, XDG_CONFIG_HOME=os.path.join(home, "config"),
                 XDG_CACHE_HOME=os.path.join(home, "cache"),
                 XDG_DATA_HOME=os.path.join(home, "data"),
                 XDG_STATE_HOME=os.path.join(home, "state"),
                 XDG_PICTURES_DIR=os.path.join(home, "Pictures"),
                 VK_ICD_FILENAMES=icd, VK_DRIVER_FILES=icd,
                 SPECULAR_LOG_FILE=self.log, SPECULAR_NO_NETWORK="1",
                 SPECULAR_OPEN_REPORT="1")
        if self.gate:
            e["SPECULAR_GATE_REPORT"] = self.gate
        if s.a.log:
            e["SPECULAR_LOG"] = s.a.log
        e.update(env or {})
        argv = [s.a.bin, "--size", SIZE, "--set", "profile.shown=question"]
        if lang:
            argv += ["--set", f"general.language={lang}"]
        argv += args
        self.started = time.time()
        self.proc = s.r.spawn(f"{sc.key}-{name}", argv, e)
        self.stderr = os.path.join(s.a.out, f"{sc.key}-{name}.log")
        self.sock = os.path.join(s.base["XDG_RUNTIME_DIR"], "specular", "specular.sock")
        self.v = None

    def connect(self, timeout=60):
        end = time.time() + timeout
        while time.time() < end:
            if self.proc.poll() is not None:
                return False
            if os.path.exists(self.sock):
                try:
                    self.v = Viewer(self.sock, timeout=30)
                    return True
                except SpecularError:
                    pass
            self.s.r.pump(0.2)
        return False

    def call(self, cmd, timeout=30, **args):
        self.v._sock.settimeout(timeout)
        try:
            return self.v.call(cmd, **args)
        finally:
            self.v._sock.settimeout(30)

    def try_call(self, cmd, **args):
        try:
            return self.call(cmd, **args), None
        except SpecularError as e:
            return None, e

    def idle(self, quiet_ms=800, timeout_ms=30000):
        """Waits for the window to settle; the answer (settled, waited_ms, held_by)."""
        return self.call("idle", timeout=timeout_ms / 1000 + 10, quiet_ms=quiet_ms,
                         timeout_ms=timeout_ms)

    def ui(self):
        return self.call("ui-state")

    def action(self, name):
        return self.try_call("action", name=name)

    def wait_for(self, what, pred, timeout=60, every=0.25):
        """Polls ui-state until pred(state) holds; the state, or None after `timeout`."""
        end = time.time() + timeout
        st = None
        while time.time() < end:
            st = self.ui()
            try:
                if pred(st):
                    return st
            except (KeyError, TypeError, IndexError):
                pass
            self.s.r.pump(every)
        self.sc.note(f"timed out waiting for {what}", st)
        return None

    def focus(self):
        self.s.sway("[app_id=.*] focus")
        self.s.r.pump(0.3)

    def shot(self, label):
        """Screenshot of the next frame, kept for the loose layout comparison."""
        try:
            png = self.v.screenshot(os.path.join(self.dir, f"shot-{label}.png"))
            return len(png)
        except SpecularError as e:
            self.sc.note(f"screenshot {label}", str(e))
            return 0

    def quit(self, timeout=20):
        if self.v:
            try:
                self.v.call("quit")
            except SpecularError:
                pass
            self.v.close()
            self.v = None
        end = time.time() + timeout
        while time.time() < end and self.proc.poll() is None:
            self.s.r.pump(0.2)
        if self.proc.poll() is None:
            self.proc.send_signal(signal.SIGTERM)
            try:
                self.proc.wait(10)
            except subprocess.TimeoutExpired:
                self.proc.kill()
        return self.proc.returncode

    def finish(self, check_errors=True):
        """After quit: the gate report, the log and stderr, checked."""
        sc = self.sc
        if self.v and sc.lang and sc.lang != "en":
            # What is on screen at the end, read from the accessibility tree: no English.
            st, _ = self.try_call("ui-state")
            if st:
                eng = english_only(sc.lang)
                leaks = sorted({t for t in st["texts"] if t in eng})
                sc.check(f"{self.name}: no English left on screen ({sc.lang})", not leaks,
                         leaks[:8])
        rc = self.quit()
        sc.check(f"{self.name}: exits cleanly", rc == 0, rc)
        if self.gate:
            g = None
            try:
                with open(self.gate) as f:
                    g = json.load(f)
            except (OSError, ValueError) as e:
                sc.check(f"{self.name}: gate report written", False, e)
            if g is not None:
                sc.check(f"{self.name}: no frame without a cause", g.get("uncaused", 1) == 0,
                         g.get("uncaused_at_ms"))
                sc.time(f"{self.name}_peak_rss_mb", g.get("peak_rss_mb"))
                sc.time(f"{self.name}_first_present_ms", g.get("first_present_ms"))
        bad = []
        for path in (self.log, self.stderr):
            try:
                with open(path, errors="replace") as f:
                    for line in f:
                        low = line.lower()
                        if "panicked" in low or " error " in low or low.startswith("error"):
                            bad.append(line.strip()[:240])
            except OSError:
                pass
        if check_errors:
            sc.check(f"{self.name}: no panics or errors in the log", not bad, bad[:5])
        net = []
        try:
            with open(self.log, errors="replace") as f:
                net = [l.strip()[:200] for l in f if "network" in l.lower() and "refused" not in
                       l.lower() and "off" not in l.lower()][:3]
        except OSError:
            pass
        sc.info(f"{self.name}_network_lines", net)

    def open_reports(self):
        try:
            with open(self.stderr, errors="replace") as f:
                return [l.strip() for l in f if l.startswith("open report: ")]
        except OSError:
            return []


class Scenario:
    def __init__(self, s, key, title, label_dir, lang):
        self.s, self.key, self.title, self.lang = s, key, title, lang
        self.dir = os.path.join(label_dir, key)
        shutil.rmtree(self.dir, ignore_errors=True)
        os.makedirs(self.dir)
        self.checks = []
        self.times = {}
        self.infos = {}
        self.notes = []
        self.load = [os.getloadavg()[0]]

    def check(self, what, ok, detail=""):
        self.s.r.check(self.key, what, ok, detail)
        self.checks.append({"check": what, "ok": bool(ok),
                            "detail": json.dumps(detail, default=str)[:600]})
        return ok

    def time(self, name, value):
        if value is not None:
            self.times[name] = round(float(value), 2)

    def info(self, name, value):
        self.infos[name] = value

    def note(self, what, detail=None):
        self.notes.append({"note": what, "detail": json.dumps(detail, default=str)[:600]})
        print(f"  [note] {self.key}: {what}", flush=True)

    def app(self, name, args, env=None, gate=True):
        return App(self.s, self, name, args, env=env, lang=self.lang, gate=gate)

    def write(self, error=None):
        self.load.append(os.getloadavg()[0])
        rep = {
            "scenario": self.key,
            "title": self.title,
            "lang": self.lang or "en",
            "passed": sum(c["ok"] for c in self.checks),
            "failed": sum(not c["ok"] for c in self.checks),
            "checks": self.checks,
            "timings": self.times,
            "under_load": max(self.load) > (os.cpu_count() or 1) / 4,
            "load": self.load,
            "info": self.infos,
            "notes": self.notes,
            "error": error,
        }
        with open(os.path.join(self.dir, "report.json"), "w") as f:
            json.dump(rep, f, indent=1, ensure_ascii=False, default=str)
        return rep


def compare(out, a_label, b_label):
    """Checks that changed status and timings twice as slow, `b` against `a`."""
    def load(label):
        reps = {}
        d = os.path.join(out, label)
        for k in sorted(os.listdir(d)) if os.path.isdir(d) else []:
            p = os.path.join(d, k, "report.json")
            if os.path.exists(p):
                with open(p) as f:
                    reps[k] = json.load(f)
        return reps
    a, b = load(a_label), load(b_label)
    lines = []
    for k in sorted(set(a) | set(b)):
        ra, rb = a.get(k), b.get(k)
        if not ra or not rb:
            lines.append(f"{k}: only in {'b' if rb else 'a'}")
            continue
        ca = {c["check"]: c["ok"] for c in ra["checks"]}
        for c in rb["checks"]:
            if c["check"] in ca and ca[c["check"]] != c["ok"]:
                lines.append(f"{k}: {c['check']}: {'pass' if ca[c['check']] else 'FAIL'} -> "
                             f"{'pass' if c['ok'] else 'FAIL'}")
            elif c["check"] not in ca:
                lines.append(f"{k}: new check {c['check']}: {'pass' if c['ok'] else 'FAIL'}")
        for t, v in rb["timings"].items():
            old = ra["timings"].get(t)
            if old and v and v > 2 * old:
                lines.append(f"{k}: {t} {old} -> {v} (2x slower"
                             f"{', under load' if rb['under_load'] else ''})")
    print("\n".join(lines) or "no changes")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--root", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--bin")
    ap.add_argument("--label", default=time.strftime("%Y%m%d-%H%M"))
    ap.add_argument("--only", default="")
    ap.add_argument("--lang", default=None)
    ap.add_argument("--compare", default=None)
    ap.add_argument("--log", default=None, help="SPECULAR_LOG for the app (e.g. render=debug)")
    ap.add_argument("--timeout", type=float, default=90)
    ap.add_argument("--unique", type=int, default=25000)
    ap.add_argument("--links", type=int, default=75000)
    a = ap.parse_args()
    if a.compare:
        compare(a.out, a.compare, a.label)
        return
    addr = os.environ.get("DBUS_SESSION_BUS_ADDRESS", "")
    if (not addr or f"/run/user/{os.getuid()}/bus" in addr
            or not os.environ.get("SPECULAR_A11Y_PRIVATE")):
        sys.exit("refusing to run outside tools/realuse/realuse.sh (private session bus)")
    target = os.environ.get("CARGO_TARGET_DIR", os.path.join(a.root, "target"))
    a.bin = a.bin or os.path.join(target, "debug", "specular")
    a.data = os.path.join(a.out, "data")
    label_dir = os.path.join(a.out, a.label + (f"-{a.lang}" if a.lang else ""))
    os.makedirs(label_dir, exist_ok=True)
    a.out = label_dir
    os.makedirs(a.data, exist_ok=True)

    import scenarios  # noqa: E402 - after the paths are set
    libc = ctypes.CDLL(None, use_errno=True)
    libc.prctl(PR_SET_CHILD_SUBREAPER, 1, 0, 0, 0)
    s = Session(a)
    only = {x.strip() for x in a.only.split(",") if x.strip()}
    reps = []
    try:
        if not s.setup():
            sys.exit(1)
        for key, title, fn in scenarios.ALL:
            if only and key.split("-")[0] not in only and key not in only:
                continue
            print(f"\n== {key}: {title}" + (f" ({a.lang})" if a.lang else ""), flush=True)
            sc = Scenario(s, key, title, label_dir, a.lang)
            s.portal.clear()
            if not s.keyboard(key):
                sc.note("the held keyboard had ended; started again", s.hold_lost[-1])
            err = None
            try:
                fn(s, sc, a)
            except Exception as e:  # noqa: BLE001 - one scenario's failure is reported
                import traceback
                err = traceback.format_exc()
                sc.check("ran to the end", False, str(e))
                print(err, flush=True)
            # Whatever the scenario left running stops before the next one.
            for name, p in list(s.r.procs):
                if name.startswith(key + "-") and p.poll() is None:
                    p.send_signal(signal.SIGTERM)
                    try:
                        p.wait(10)
                    except subprocess.TimeoutExpired:
                        p.kill()
            reps.append(sc.write(err))
    finally:
        s.r.stop_all()
    print()
    for rep in reps:
        print(f"{rep['scenario']}: {rep['passed']} passed, {rep['failed']} failed"
              f"{' (under load)' if rep['under_load'] else ''}")
    with open(os.path.join(label_dir, "summary.json"), "w") as f:
        json.dump([{k: r[k] for k in ("scenario", "passed", "failed", "under_load")}
                   for r in reps], f, indent=1)
    sys.exit(0 if all(r["failed"] == 0 for r in reps) else 1)


if __name__ == "__main__":
    main()
