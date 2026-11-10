#!/usr/bin/env python3
"""Prints what ezdxf's HP-GL/2 add-on (MIT) plots for each HP-GL / HP-GL/2 file, as TSV.

Usage: hpgl_oracle.py CORPUS_DIR_OR_FILE... > out.tsv
  A directory is walked recursively (dot files skipped); paths are printed relative to it.
  Run with the oracle venv: target/hpgl/venv/bin/python -I hpgl_oracle.py tools/corpus/hpgl

Columns:
  path      file path relative to the directory given (or as given for a file)
  status    ok | empty (no HP-GL/2 commands found) | error:<exception type>
  strokes   pen-down strokes outside polygon mode: maximal runs of connected pen-down
            segments. A run continues while each drawing call starts where the previous one
            ended (within 1e-6 plu), with no pen up (PU, PE "<", any pen-up state) and no
            pen change (SP, PE ":") in between. Each CI circle, arc and Bezier is one call.
  segments  pen-down line segments in those strokes (points - 1 per drawing call). Arcs and
            circles count their chord segments as tessellated by ezdxf (default chord angle
            5 degrees, or the command's), a cubic Bezier counts as 1.
  polygons  FP and EP outputs: number of closed sub-paths filled or edged
  xmin ymin xmax ymax
            extents of all pen-down geometry (strokes and FP/EP polygons) in plotter units
            (40 plu per mm, page coordinates after IP/IR/SC scaling), rounded to 0.01;
            "-" when nothing is drawn. Text (LB) is never drawn. Arc extents are those of
            the chord vertices; Bezier extents are exact.
  ignored   commands ezdxf parsed but does not interpret, comma separated (e.g. LB, RO,
            IW, IN, RA, RR, EA, ER, EW, WG, LT, DF, PG): a file listing a geometry or
            state command here is not a fair comparison for that aspect
  notes     oracle caveats hit by this file, comma separated:
              sc-without-ip  SC used with ezdxf's default P1/P2 (0,0)-(1189,841) plu, which
                             is not any real device's hard-clip default
              scaled-arc     CI/AA/AR/AT/RT drawn while user scaling was active: ezdxf emits
                             arc vertices in user units, not plotter units (ezdxf bug)
              wrapped        no ESC%#B in the file: the oracle stripped UEL/PJL lines,
                             7475A ESC. device-control sequences and leading ";"/blanks and
                             fed the rest as HP-GL/2

What ezdxf understands (ezdxf.addons.hpgl2, see its Interpreter docstring): PU PD PA PR
(implicit pen moves), PE (all flags: ":" pen, "<" pen up, ">" fraction bits, "=" absolute,
"7" 7-bit base 32; 8-bit base 64 by default), CI AA AR AT RT BZ BR, PM FP EP (polygon mode,
even-odd/non-zero), IP IR SC (anisotropic, isotropic, point factor), PS, SP NP PC PW FT MC.
Not interpreted: IN/DF (no state reset between pages), RO (no rotation), IW (no clipping),
RA RR EA ER EW WG (rectangles, wedges), LB and the whole character group, LT (dashes), and
anything else. The byte-stream reader takes HP-GL/2 after any ESC%-?[0-3]B (PCL5 enter
HP-GL/2), stops at any other escape (ESC%#A, ESC E, raster) until the next ESC%#B, and
accepts plot files that start with BP/IN/INPS/INDF directly. Plain HP-GL files without that
are wrapped here (see "wrapped" above). The default 7-bit LB terminator ETX and DT are
honoured so label text is skipped. ezdxf's tokenizer only starts a mnemonic at an uppercase
letter after the previous command's parameters: lowercase mnemonics, and stray separators
such as ";;", show up as bogus entries in "ignored" (e.g. "Co", "Q;") and their parameters
are lost.

ezdxf is only an oracle for the tests; nothing of it is in the reader.
"""

import os
import re
import sys

from ezdxf.addons.hpgl2 import tokenizer
from ezdxf.addons.hpgl2.backend import Backend
from ezdxf.addons.hpgl2.interpreter import Interpreter
from ezdxf.addons.hpgl2.plotter import Plotter
from ezdxf.path import bbox as path_bbox

EPS = 1e-6
ENTER_HPGL2 = re.compile(rb"\x1b%-?[0-3]B")
UEL = b"\x1b%-12345X"
PJL_LINE = re.compile(rb"@PJL[^\n]*\n")
# 7475A device control: ESC.( ESC.) ESC.Y ESC.Z ESC.K ESC.O ESC.E ESC.L ESC.B ESC.R ...
# and the parameterised ones ESC.@ ESC.H ESC.I ESC.M ESC.N ESC.T ending with ":".
DEVICE_CONTROL = re.compile(rb"\x1b\.[@HIMNST][^:\x1b]*:?|\x1b\.[A-Za-z()]")


class StrokeCounter(Backend):
    def __init__(self):
        self.strokes = 0
        self.segments = 0
        self.polygons = 0
        self.ext = [float("inf"), float("inf"), float("-inf"), float("-inf")]
        self.last_end = None  # end of the current stroke, None after a break

    def brk(self):
        self.last_end = None

    def _grow(self, x, y):
        e = self.ext
        e[0], e[1] = min(e[0], x), min(e[1], y)
        e[2], e[3] = max(e[2], x), max(e[3], y)

    def _stroke(self, start, end, nseg):
        if self.last_end is None or not start.isclose(self.last_end, abs_tol=EPS):
            self.strokes += 1
        self.segments += nseg
        self.last_end = end

    def draw_polyline(self, properties, points):
        if len(points) < 2:
            return
        for p in points:
            self._grow(p.x, p.y)
        self._stroke(points[0], points[-1], len(points) - 1)

    def draw_paths(self, properties, paths, filled):
        boxes = [p for p in paths if len(p)]
        if not boxes:
            return
        bb = path_bbox(boxes, fast=False)
        if bb.has_data:
            self._grow(bb.extmin.x, bb.extmin.y)
            self._grow(bb.extmax.x, bb.extmax.y)
        if filled or self._in_polygon_output:
            self.polygons += sum(1 for p in boxes for _ in p.sub_paths())
            self.brk()
        else:  # Bezier curve (BZ/BR) outside polygon mode
            for p in boxes:
                self._stroke(p.start, p.end, len(p))

    _in_polygon_output = False


class TracingPlotter(Plotter):
    """Reports pen-up, pen-change and polygon-edge events to the StrokeCounter."""

    def __init__(self, backend):
        super().__init__(backend)
        self.counter = backend
        self.notes = set()

    def pen_up(self):
        self.counter.brk()
        super().pen_up()

    def set_current_pen(self, index):
        self.counter.brk()
        super().set_current_pen(index)

    def move_to_rel(self, user_location):
        # Only PE "<" (pen-up move) reaches this; ezdxf never calls pen_up() for it.
        self.counter.brk()
        super().move_to_rel(user_location)

    def edge_polygon(self):
        self.counter._in_polygon_output = True
        try:
            super().edge_polygon()
        finally:
            self.counter._in_polygon_output = False

    def plot_abs_circle(self, radius, chord_angle):
        self._check_scaled_arc()
        self.counter.brk()
        super().plot_abs_circle(radius, chord_angle)
        self.counter.brk()

    def plot_abs_arc(self, center, sweep_angle, chord_angle):
        self._check_scaled_arc()
        super().plot_abs_arc(center, sweep_angle, chord_angle)

    def _check_scaled_arc(self):
        if self.is_pen_down and (self.page.user_scaling or self.page.user_origin.magnitude):
            self.notes.add("scaled-arc")


class TracingInterpreter(Interpreter):
    def __init__(self, plotter):
        super().__init__(plotter)
        self.saw_ip = False

    def cmd_ip(self, args):
        self.saw_ip = True
        super().cmd_ip(args)

    def cmd_ir(self, args):
        self.saw_ip = True
        super().cmd_ir(args)

    def cmd_sc(self, args):
        if any(a.strip() for a in args) and not self.saw_ip:
            self.plotter.notes.add("sc-without-ip")
        super().cmd_sc(args)


def load(data):
    notes = []
    if not ENTER_HPGL2.search(data) and not tokenizer.has_known_start_sequence(data):
        data = data.replace(UEL, b"")
        data = PJL_LINE.sub(b"", data)
        data = DEVICE_CONTROL.sub(b"", data)
        # ezdxf reads a leading ";" as the start of a mnemonic
        data = b"\x1b%1B" + data.lstrip(b"; \t\r\n")
        notes.append("wrapped")
    return tokenizer.hpgl2_commands(data), notes


def run(path):
    with open(path, "rb") as f:
        data = f.read()
    commands, notes = load(data)
    if not commands:
        return "empty", None, "", notes
    counter = StrokeCounter()
    plotter = TracingPlotter(counter)
    interpreter = TracingInterpreter(plotter)
    interpreter.run(commands)
    notes += sorted(plotter.notes)
    return "ok", counter, ",".join(sorted(interpreter.not_implemented_commands)), notes


def fmt(v):
    return f"{v:.2f}"


def row(rel, path):
    try:
        status, c, ignored, notes = run(path)
    except Exception as e:  # noqa: BLE001
        return f"{rel}\terror:{type(e).__name__}\t-\t-\t-\t-\t-\t-\t-\t-\t-"
    if c is None:
        return f"{rel}\t{status}\t0\t0\t0\t-\t-\t-\t-\t-\t{','.join(notes) or '-'}"
    if c.ext[0] == float("inf"):
        ext = ["-"] * 4
    else:
        ext = [fmt(v) for v in c.ext]
    return "\t".join(
        [rel, status, str(c.strokes), str(c.segments), str(c.polygons), *ext,
         ignored or "-", ",".join(notes) or "-"]
    )


def main(args):
    print("path\tstatus\tstrokes\tsegments\tpolygons\txmin\tymin\txmax\tymax\tignored\tnotes")
    for arg in args:
        if os.path.isdir(arg):
            files = []
            for dirpath, dirnames, filenames in os.walk(arg):
                dirnames[:] = [d for d in dirnames if not d.startswith(".")]
                files += [os.path.join(dirpath, f) for f in filenames if not f.startswith(".")]
            for p in sorted(files, key=lambda p: os.path.relpath(p, arg)):
                print(row(os.path.relpath(p, arg), p))
        else:
            print(row(arg, arg))


if __name__ == "__main__":
    main(sys.argv[1:])
