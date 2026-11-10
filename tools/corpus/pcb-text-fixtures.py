#!/usr/bin/env python3
"""Writes the PCB text board fixtures: ODB++ jobs with text, negative layers and step-and-repeat
panels, and IPC-2581 files with text, for tests/pcb_text_corpus.rs and pcb-text-oracle.py.

The ODB++ text uses a real stroke font: fonts/standard of the designodb_rigidflex sample
(tools/corpus/fetch-engineering.sh odb), copied into the generated jobs, so nothing from it is
committed. Output: OUT/odb/<job>/... folders and OUT/ipc2581/*.xml.

    python3 -I tools/corpus/pcb-text-fixtures.py ENGINEERING_CORPUS OUT
"""

import os
import sys
import tarfile


def standard_font(engineering):
    path = os.path.join(engineering, "odb", "designodb_rigidflex.tgz")
    with tarfile.open(path, "r:gz") as t:
        for m in t.getmembers():
            if m.isfile() and m.name.lower().endswith("fonts/standard"):
                return t.extractfile(m).read().decode("latin-1")
    raise SystemExit("no fonts/standard in " + path)


def write(root, files):
    for rel, text in files.items():
        p = os.path.join(root, rel)
        os.makedirs(os.path.dirname(p), exist_ok=True)
        with open(p, "w", encoding="latin-1", newline="\n") as f:
            f.write(text)


def matrix(steps, layers):
    out = []
    for i, s in enumerate(steps):
        out.append("STEP {\n   COL=%d\n   NAME=%s\n}\n" % (i + 1, s))
    for i, (name, ty, pol) in enumerate(layers):
        out.append(
            "LAYER {\n   ROW=%d\n   CONTEXT=BOARD\n   TYPE=%s\n   NAME=%s\n   POLARITY=%s\n}\n"
            % (i + 1, ty, name, pol)
        )
    return "".join(out)


PROFILE_MM = "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 80 0\nOS 80 50\nOS 0 50\nOS 0 0\nOE\nSE\n"


def text_job(font):
    # Every orient_def, the angle forms, both polarities, sizes and width factors.
    lines = ["UNITS=MM", "#"]
    y = 2.0
    for o in range(8):
        lines.append("T 5 %.1f standard P %d 1.2 1.6 1 'Ab9-%d' 1" % (y, o, o))
        y += 5
    lines.append("T 40 5 standard P 8 30 1 1.5 0.5 'ANGLE 30' 1")
    lines.append("T 40 15 standard N 9 135.5 2 1 2 'MIRROR' 1")
    lines.append("T 40 25 standard P 0 0.8 0.8 1 'mixed Case {x}' 0")
    top = "\n".join(lines) + "\n"
    inch = "UNITS=INCH\n#\nT 2 1.5 standard P 0 0.05 0.06 1 'INCH 42' 1\n"
    silk = "UNITS=MM\n#\nT 10 40 simplex P 0 2 2 1 'NO FONT 123' 1\nT 10 45 simplex P 1 1 1 0.5 'rot' 1\n"
    return {
        "matrix/matrix": matrix(
            ["pcb"],
            [
                ("top", "SIGNAL", "POSITIVE"),
                ("silk", "SILK_SCREEN", "POSITIVE"),
                ("bot", "SIGNAL", "POSITIVE"),
            ],
        ),
        "misc/info": "JOB_NAME=text\nODB_VERSION_MAJOR=8\nODB_VERSION_MINOR=1\n",
        "fonts/standard": font,
        "steps/pcb/stephdr": "UNITS=MM\n",
        "steps/pcb/profile": PROFILE_MM,
        "steps/pcb/layers/top/features": top,
        "steps/pcb/layers/bot/features": inch,
        "steps/pcb/layers/silk/features": silk,
    }


def negative_job(font):
    gnd = (
        "UNITS=MM\n$0 r1500\n$1 rect2000x1000\n$2 r200\n#\n"
        "P 10 10 0 P 0 0\nP 20 10 1 N 0 1\nL 30 10 40 20 2 P 0\n"
        "T 50 30 standard P 0 2 2 1 'GND' 1\nT 50 40 standard N 0 2 2 1 'X' 1\n"
    )
    pwr = "UNITS=MM\n$0 r1000\n#\nP 5 5 0 P 0 0\nP 6 5 0 N 0 0\n"
    return {
        "matrix/matrix": matrix(
            ["pcb"],
            [
                ("top", "SIGNAL", "POSITIVE"),
                ("gnd", "POWER_GROUND", "NEGATIVE"),
                ("pwr", "POWER_GROUND", "NEGATIVE"),
                ("bottom", "SIGNAL", "POSITIVE"),
            ],
        ),
        "fonts/standard": font,
        "steps/pcb/stephdr": "UNITS=MM\n",
        "steps/pcb/profile": PROFILE_MM,
        "steps/pcb/layers/top/features": "UNITS=MM\n$0 r500\n#\nP 1 1 0 P 0 0\n",
        "steps/pcb/layers/gnd/features": gnd,
        "steps/pcb/layers/pwr/features": pwr,
        "steps/pcb/layers/bottom/features": "UNITS=MM\n$0 r500\n#\nP 2 2 0 P 0 0\n",
    }


def panel_job(font):
    board = "UNITS=MM\n$0 r300\n$1 rect1000x400\n#\nP 2 3 0 P 0 0\nP 8 3 1 P 0 1\nL 1 1 9 1 0 P 0\n"
    board += "T 1 5 standard P 0 1 1 1 'B' 1\n"
    panel_own = "UNITS=MM\n$0 r2000\n#\nP 5 5 0 P 0 0\nP 195 5 0 P 0 0\n"
    hdr = (
        "UNITS=MM\n"
        "STEP-REPEAT {\n NAME=pcb\n X=20\n Y=10\n DX=15\n DY=12\n NX=4\n NY=3\n ANGLE=0\n MIRROR=NO\n}\n"
        "STEP-REPEAT {\n NAME=pcb\n X=120\n Y=10\n DX=0\n DY=15\n NX=1\n NY=2\n ANGLE=90\n MIRROR=NO\n}\n"
        "STEP-REPEAT {\n NAME=pcb\n X=150\n Y=40\n DX=0\n DY=0\n NX=1\n NY=1\n ANGLE=180\n MIRROR=YES\n}\n"
    )
    return {
        "matrix/matrix": matrix(["panel", "pcb"], [("top", "SIGNAL", "POSITIVE")]),
        "fonts/standard": font,
        "steps/pcb/stephdr": "UNITS=MM\nX_DATUM=1\nY_DATUM=1\n",
        "steps/pcb/profile": "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 10 0\nOS 10 8\nOS 0 8\nOS 0 0\nOE\nSE\n",
        "steps/pcb/layers/top/features": board,
        "steps/panel/stephdr": hdr,
        "steps/panel/profile": "UNITS=MM\n#\nS P 0\nOB 0 0 I\nOS 200 0\nOS 200 60\nOS 0 60\nOS 0 0\nOE\nSE\n",
        "steps/panel/layers/top/features": panel_own,
    }


IPC_HEAD = """<?xml version="1.0" encoding="UTF-8"?>
<IPC-2581 revision="C" xmlns="http://webstds.ipc.org/2581">
<Content roleRef="Owner">
<DictionaryFont units="MILLIMETER">
<EntryFont id="EMB">
<FontDefEmbedded name="sticks">
<Glyph charCode="41" lowerLeftX="0" lowerLeftY="0" upperRightX="0.8" upperRightY="1.2">
<Polyline><PolyBegin x="0" y="0"/><PolyStepSegment x="0.4" y="1.2"/><PolyStepSegment x="0.8" y="0"/><LineDesc lineWidth="0.1" lineEnd="ROUND"/></Polyline>
<Line startX="0.2" startY="0.6" endX="0.6" endY="0.6"><LineDesc lineWidth="0.1" lineEnd="ROUND"/></Line>
</Glyph>
<Glyph charCode="4F" lowerLeftX="0" lowerLeftY="0" upperRightX="0.8" upperRightY="1.2">
<Arc startX="0.4" startY="0" endX="0.4" endY="0" centerX="0.4" centerY="0.6" clockwise="false"><LineDesc lineWidth="0.1" lineEnd="ROUND"/></Arc>
</Glyph>
<Glyph charCode="2D" lowerLeftX="0" lowerLeftY="0" upperRightX="0.8" upperRightY="1.2">
<Line startX="0.1" startY="0.6" endX="0.7" endY="0.6"><LineDesc lineWidth="0.1" lineEnd="SQUARE"/></Line>
</Glyph>
</FontDefEmbedded>
</EntryFont>
<EntryFont id="EXT"><FontDefExternal name="Arial"/></EntryFont>
</DictionaryFont>
</Content>
<Ecad name="text">
<CadHeader units="MILLIMETER"/>
<CadData>
<Layer name="SILK_TOP" layerFunction="SILKSCREEN" side="TOP"/>
<Layer name="TOP" layerFunction="CONDUCTOR" side="TOP"/>
<Step name="pcb">
<Profile><Polygon><PolyBegin x="0" y="0"/><PolyStepSegment x="60" y="0"/><PolyStepSegment x="60" y="40"/><PolyStepSegment x="0" y="40"/><PolyStepSegment x="0" y="0"/></Polygon></Profile>
"""

IPC_TAIL = "</Step>\n</CadData>\n</Ecad>\n</IPC-2581>\n"


def text_el(s, font, at, size, bbox=None, xform="", polarity=None):
    pol = ' polarity="%s"' % polarity if polarity else ""
    bb = ""
    if bbox:
        bb = '<BoundingBox lowerLeftX="%g" lowerLeftY="%g" upperRightX="%g" upperRightY="%g"/>' % bbox
    fr = '<FontRef id="%s"/>' % font if font else ""
    return (
        '<Features><Location x="%g" y="%g"/><Text textString="%s" fontSize="%g">%s%s%s</Text></Features>\n'
        % (at[0], at[1], s, size, xform, bb, fr),
        pol,
    )


def ipc_files():
    silk = []
    for s, font, at, size, bbox, xf in [
        ("AOA-A", "EMB", (5, 5), 2, (0, 0, 10, 2), ""),
        ("A-O", "EMB", (5, 10), 3, None, '<Xform rotation="90"/>'),
        ("OA", "EMB", (20, 10), 1.5, (0, 0, 6, 1.5), '<Xform rotation="45" mirror="true"/>'),
        ("Arial 12", "EXT", (5, 25), 2.5, (0, 0, 20, 2.5), ""),
        ("none", None, (30, 30), 1, None, '<Xform rotation="-30" scale="2"/>'),
    ]:
        silk.append(text_el(s, font, at, size, bbox, xf)[0])
    top = text_el("AO", "EMB", (40, 5), 2, (0, 0, 4, 2))[0]
    body = (
        '<LayerFeature layerRef="SILK_TOP"><Set>\n' + "".join(silk) + "</Set></LayerFeature>\n"
        '<LayerFeature layerRef="TOP"><Set polarity="NEGATIVE">\n' + top + "</Set></LayerFeature>\n"
    )
    return {"text.xml": IPC_HEAD + body + IPC_TAIL}


def main():
    engineering, out = sys.argv[1], sys.argv[2]
    font = standard_font(engineering)
    for name, files in [
        ("text", text_job(font)),
        ("negative", negative_job(font)),
        ("panel", panel_job(font)),
    ]:
        write(os.path.join(out, "odb", name), files)
    for name, text in ipc_files().items():
        write(os.path.join(out, "ipc2581"), {name: text})
    print("fixtures in", out)


if __name__ == "__main__":
    main()
