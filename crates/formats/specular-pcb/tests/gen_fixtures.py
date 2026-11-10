#!/usr/bin/env python3
"""Writes the synthetic Gerber and Excellon fixtures under tests/data/ (deterministic; run from
anywhere: python3 -I crates/formats/specular-pcb/tests/gen_fixtures.py).

Each file exercises one part of the formats; tests/fixtures.rs checks what Specular reads from
them, and tests/oracle.py can be run over tests/data/ to compare with gerbonara.
"""

import os

HERE = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HERE, "data")


def mm(v):
    """A coordinate in FSLAX46Y46 millimetres."""
    return str(int(round(v * 1e6)))


def xy(x, y):
    return f"X{mm(x)}Y{mm(y)}"


def gerber(name, body, function=None, header=True, end=True):
    lines = []
    if function:
        lines.append("%TF.GenerationSoftware,Specular,gen_fixtures,1*%")
        lines.append("%TF.ProjectId,fixture,1234,A*%")
        lines.append(f"%TF.FileFunction,{function}*%")
    if header:
        lines += ["%FSLAX46Y46*%", "%MOMM*%"]
    lines += body
    if end:
        lines.append("M02*")
    write(name, "\n".join(lines) + "\n")


def write(name, text):
    path = os.path.join(DATA, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w", newline="\n") as f:
        f.write(text)


def apertures():
    body = [
        "%ADD10C,1.0*%",
        "%ADD11C,1.5X0.5*%",
        "%ADD12R,2.0X1.0*%",
        "%ADD13R,2.0X1.5X0.6*%",
        "%ADD14O,3.0X1.0*%",
        "%ADD15O,1.0X3.0X0.4*%",
        "%ADD16P,2.0X5*%",
        "%ADD17P,2.0X6X15.0X0.5*%",
        "%ADD18C,0*%",
    ]
    x = 0.0
    for d in range(10, 18):
        body += [f"D{d}*", f"{xy(x, 0)}D03*"]
        x += 4.0
    # Draws with a circle, a rectangle and a zero-size aperture.
    body += ["D10*", f"{xy(0, 5)}D02*", "G01*", f"{xy(10, 5)}D01*", f"{xy(10, 10)}D01*"]
    body += ["D12*", f"{xy(15, 5)}D02*", f"{xy(25, 8)}D01*"]
    body += ["D18*", f"{xy(0, 12)}D02*", f"{xy(30, 12)}D01*"]
    gerber("apertures.gbr", body, "Copper,L1,Top")


def macros():
    body = [
        # Every primitive, with variables, expressions and exposure off.
        "%AMCIRCLES*0 two circles, the second off*1,1,$1,0,0*1,0,$1/2,0,0*%",
        "%AMVLINE*20,1,$1,-$2,0,$2,0,30*%",
        "%AMCLINE*21,1,$1,$2,0,0,45*%",
        "%AMOUTLINE*4,1,4,0,0,2,0,2,1,1,2,0,0,10*%",
        "%AMPOLY*5,1,$1,0,0,$2,0*%",
        "%AMTHERM*7,0,0,$1,$1x0.6,0.3,45*%",
        "%AMCALC*$4=$1x2*$5=($2+$3)x-1*21,1,$4,$1,0,0,0*1,1,-$5,0,$1*%",
        "%AMROUNDRECT*0 KiCad style*21,1,$1-$2x2,$3,0,0,0*21,1,$1,$3-$2x2,0,0,0*"
        "1,1,$2x2,$1/2-$2,$3/2-$2*1,1,$2x2,-$1/2+$2,$3/2-$2*"
        "1,1,$2x2,$1/2-$2,-$3/2+$2*1,1,$2x2,-$1/2+$2,-$3/2+$2*%",
        "%ADD20CIRCLES,2.0*%",
        "%ADD21VLINE,0.5X1.5*%",
        "%ADD22CLINE,2.0X0.5*%",
        "%ADD24OUTLINE*%",
        "%ADD25POLY,6X2.0*%",
        "%ADD26THERM,3.0*%",
        "%ADD28CALC,1.0X0.5X0.25*%",
        "%ADD29ROUNDRECT,3.0X0.5X2.0*%",
    ]
    x = 0.0
    for d in [20, 21, 22, 24, 25, 26, 28, 29]:
        body += [f"D{d}*", f"{xy(x, 0)}D03*"]
        x += 6.0
    gerber("macros.gbr", body, "Copper,L2,Inr")
    # Deprecated primitives: 2 (the old vector line), 22 (lower-left line), 6 (moire).
    legacy = ["%AMVLINE2*2,1,0.5,0,0,3,0,0*%", "%AMLLLINE*22,1,$1,$2,-$1/2,-$2/2,0*%",
              "%AMMOIRE*6,0,0,4,0.3,0.4,3,0.2,5,0*%", "%ADD10VLINE2*%", "%ADD11LLLINE,1.0X2.0*%",
              "%ADD12MOIRE*%", "D10*", f"{xy(0, 0)}D03*", "D11*", f"{xy(6, 0)}D03*", "D12*",
              f"{xy(12, 0)}D03*"]
    gerber("macros-legacy.gbr", legacy, "Copper,L1,Top")


def arcs():
    body = ["%ADD10C,0.3*%", "D10*"]
    # Multi-quadrant: a counter-clockwise half, a clockwise quarter, a full circle.
    body += ["G75*", f"{xy(5, 0)}D02*", "G03*", f"{xy(-5, 0)}I{mm(-5)}J0D01*"]
    body += ["G02*", f"{xy(-5, 0)}D02*", f"{xy(0, 5)}I{mm(5)}J0D01*"]
    body += [f"{xy(20, 0)}D02*", "G02*", f"{xy(20, 0)}I{mm(-3)}J0D01*"]
    # Single-quadrant: unsigned offsets, the centre found from the quadrant.
    body += ["G74*", f"{xy(30, 0)}D02*", "G03*", f"{xy(34, 4)}I{mm(4)}J0D01*"]
    body += ["G02*", f"{xy(40, 4)}D02*", f"{xy(44, 0)}I0J{mm(4)}D01*"]
    body += ["G01*"]
    gerber("arcs.gbr", body, "Copper,L1,Top")


def regions():
    body = ["G75*", "G36*"]
    # A square with a rounded corner.
    body += [f"{xy(0, 0)}D02*", "G01*", f"{xy(10, 0)}D01*", f"{xy(10, 8)}D01*", "G03*",
             f"{xy(8, 10)}I{mm(-2)}J0D01*", "G01*", f"{xy(0, 10)}D01*", f"{xy(0, 0)}D01*"]
    # A second contour in the same region, clockwise.
    body += [f"{xy(15, 0)}D02*", f"{xy(15, 5)}D01*", f"{xy(20, 5)}D01*", f"{xy(20, 0)}D01*",
             f"{xy(15, 0)}D01*", "G37*"]
    # A cut-in hole.
    body += ["G36*", f"{xy(30, 0)}D02*", f"{xy(40, 0)}D01*", f"{xy(40, 10)}D01*", f"{xy(30, 10)}D01*",
             f"{xy(30, 0)}D01*", f"{xy(33, 3)}D01*", f"{xy(33, 7)}D01*", f"{xy(37, 7)}D01*",
             f"{xy(37, 3)}D01*", f"{xy(33, 3)}D01*", f"{xy(30, 0)}D01*", "G37*"]
    # A circle region from one full arc.
    body += ["G36*", f"{xy(55, 5)}D02*", "G03*", f"{xy(55, 5)}I{mm(-5)}J0D01*", "G01*", "G37*"]
    gerber("regions.gbr", body, "Copper,L1,Top")


def polarity():
    body = ["%ADD10C,4.0*%", "%ADD11R,1.0X6.0*%", "%ADD12R,2.0X1.0*%"]
    body += ["%LPD*%", "D10*", f"{xy(0, 0)}D03*", "%LPC*%", "D11*", f"{xy(0, 0)}D03*",
             "%LPD*%", "D12*", f"{xy(0, 0)}D03*"]
    # Mirroring, rotation and scaling of apertures.
    body += ["%LMX*%", "%LR30*%", "%LS1.5*%", f"{xy(10, 0)}D03*", "%LMXY*%", f"{xy(15, 0)}D03*",
             "%LMN*%", "%LR0*%", "%LS1*%", f"{xy(20, 0)}D03*"]
    # A clear region over a dark one.
    body += ["G36*", f"{xy(25, -3)}D02*", "G01*", f"{xy(35, -3)}D01*", f"{xy(35, 3)}D01*",
             f"{xy(25, 3)}D01*", f"{xy(25, -3)}D01*", "G37*", "%LPC*%", "G36*",
             f"{xy(28, -1)}D02*", f"{xy(32, -1)}D01*", f"{xy(32, 1)}D01*", f"{xy(28, 1)}D01*",
             f"{xy(28, -1)}D01*", "G37*", "%LPD*%"]
    gerber("polarity.gbr", body, "Soldermask,Top")


def step_repeat():
    body = ["%ADD10C,1.0*%", "%ADD11R,0.5X0.5*%", "%SRX4Y3I3.0J2.5*%", "D10*", f"{xy(0, 0)}D03*",
            "D11*", f"{xy(1, 0)}D02*", "G01*", f"{xy(1.5, 1)}D01*", "%SR*%", "D10*",
            f"{xy(20, 0)}D03*"]
    gerber("stepnrepeat.gbr", body, "Copper,L1,Top")


def blocks():
    body = ["%ADD10C,1.0*%", "%ADD11R,2.0X2.0*%",
            "%ABD100*%", "D11*", f"{xy(0, 0)}D03*", "%LPC*%", "D10*", f"{xy(0, 0)}D03*", "%LPD*%",
            "%ABD101*%", "D10*", f"{xy(3, 0)}D03*", "%AB*%",
            "D101*", f"{xy(0, 3)}D03*", "%AB*%",
            "D100*", f"{xy(10, 0)}D03*", f"{xy(20, 0)}D03*", "%LPC*%", f"{xy(30, 0)}D03*", "%LPD*%",
            "%LR90*%", f"{xy(40, 0)}D03*", "%LR0*%"]
    gerber("blocks.gbr", body, "Copper,L1,Top")


def x2():
    body = ["%TF.CreationDate,2026-10-19T12:00:00+02:00*%", "%TF.SameCoordinates,Original*%",
            "%TF.FilePolarity,Positive*%",
            "%TA.AperFunction,SMDPad,CuDef*%", "%ADD10R,1.0X0.6*%", "%TA.AperFunction,Conductor*%",
            "%ADD11C,0.25*%", "%TD*%",
            "%TO.N,GND*%", "%TO.C,R1*%", "%TO.P,R1,1*%", "D10*", f"{xy(0, 0)}D03*", "%TD*%",
            "%TO.N,GND*%", "D11*", f"{xy(0, 0)}D02*", "G01*", f"{xy(5, 0)}D01*", "%TD*%"]
    gerber("x2.gbr", body, "Copper,L1,Top")


def deprecated():
    # Inches, trailing zero omission, G54, G70, deprecated image parameters, no M02.
    lines = ["G04 an old file*", "%FSTAX23Y23*%", "%MOIN*%", "%OFA0.1B0*%", "%SFA1B1*%",
             "%IPPOS*%", "%IR0*%", "%ADD10C,0.05*%", "%ADD11R,0.06X0.04*%", "G70*", "G90*",
             "G54D10*", "X01Y01D02*", "G01X02Y01D01*", "X02Y02D01*", "G54D11*", "X03Y01D03*",
             "X035D03*"]
    write("deprecated.gbr", "\n".join(lines) + "\n")
    # An RS-274-D file: apertures used, none defined.
    write("rs274d.gbr", "G04 RS-274-D*\nG90*\nD10*\nX1000Y1000D02*\nX2000Y1000D01*\nD11*\nX3000Y1000D03*\nM02*\n")
    # A negative image.
    gerber("negative.gbr", ["%IPNEG*%", "%ADD10C,1.0*%", "D10*", f"{xy(0, 0)}D03*", f"{xy(5, 5)}D03*"])


def drills():
    write("kicad.drl", "\n".join([
        "M48", "; DRILL file {KiCad 8.0.4} date 2026-10-19", "; FORMAT={-:-/ absolute / metric / decimal}",
        "; #@! TF.CreationDate,2026-10-19T12:00:00+02:00",
        "; #@! TF.GenerationSoftware,Kicad,Pcbnew,8.0.4", "; #@! TF.FileFunction,MixedPlating,1,2",
        "FMAT,2", "METRIC",
        "; #@! TA.AperFunction,Plated,PTH,ViaDrill", "T1C0.400",
        "; #@! TA.AperFunction,Plated,PTH,ComponentDrill", "T2C1.000",
        "; #@! TA.AperFunction,NonPlated,NPTH,ComponentDrill", "T3C3.200",
        "%", "G90", "G05",
        "T1", "X5.0Y5.0", "X15.0Y5.0", "X10.0Y8.5",
        "T2", "X2.54Y2.54", "X5.08Y2.54", "G00X8.0Y2.0", "M15", "G01X12.0Y2.0", "M16", "G05",
        "T3", "X3.0Y8.0", "X17.0Y8.0",
        "M30", ""]))
    write("altium-inch-lz.txt", "\n".join([
        "M48", ";Layer_Color=9474304", ";FILE_FORMAT=2:5", "INCH,LZ", ";TYPE=PLATED",
        "T1F00S00C0.0300", "T2F00S00C0.0400", "%", "T01", "X0100000Y0100000", "X015Y01",
        "T02", "X02Y02", "M30", ""]))
    write("eagle.drd", "\n".join([
        "%", "M48", "M72", "T01C0.0236", "T02C0.0394", "%", "T01", "X3000Y3000", "X4000Y3000",
        "T02", "X5000Y5000", "M30", ""]))
    write("routed.drl", "\n".join([
        "M48", "METRIC,TZ,000.000", "T1C1.0", "T2C0.6", "%", "T1",
        "G00X0Y0", "M15", "G01X10000Y0", "G03X20000Y0A5000", "G02X20000Y10000I0J5000", "M16", "G05",
        "T2", "X1000Y20000", "R4X2000", "X0Y30000G85X5000Y30000", "M30", ""]))


def board():
    """A two-layer board with KiCad's file names."""
    edge = ["%ADD10C,0.10*%", "D10*", f"{xy(0, 0)}D02*", "G01*", f"{xy(30, 0)}D01*",
            f"{xy(30, 18)}D01*", "G75*", "G03*", f"{xy(28, 20)}I{mm(-2)}J0D01*", "G01*",
            f"{xy(0, 20)}D01*", f"{xy(0, 0)}D01*",
            f"{xy(25, 10)}D02*", "G02*", f"{xy(25, 10)}I{mm(-1.5)}J0D01*", "G01*"]
    pads = [(5, 5), (10, 5), (5, 15), (10, 15)]
    top = ["%ADD10R,1.6X1.6*%", "%ADD11C,1.6*%", "%ADD12C,0.25*%", "%ADD13C,0.6*%", "D10*"]
    for x, y in pads:
        top += [f"{xy(x, y)}D03*"]
    top += ["D12*", f"{xy(5, 5)}D02*", "G01*", f"{xy(10, 5)}D01*", f"{xy(10, 15)}D01*",
            f"{xy(15, 15)}D01*", "D13*", f"{xy(15, 15)}D03*"]
    top += ["G36*", f"{xy(16, 2)}D02*", f"{xy(22, 2)}D01*", f"{xy(22, 8)}D01*", f"{xy(16, 8)}D01*",
            f"{xy(16, 2)}D01*", "G37*", "%LPC*%", "D13*", f"{xy(19, 5)}D03*", "%LPD*%"]
    bottom = ["%ADD11C,1.6*%", "%ADD12C,0.25*%", "%ADD13C,0.6*%", "D11*"]
    for x, y in pads:
        bottom += [f"{xy(x, y)}D03*"]
    bottom += ["D12*", f"{xy(5, 15)}D02*", "G01*", f"{xy(15, 15)}D01*", "D13*", f"{xy(15, 15)}D03*"]
    mask = ["%ADD10R,1.7X1.7*%", "D10*"] + [f"{xy(x, y)}D03*" for x, y in pads]
    silk = ["%ADD10C,0.15*%", "D10*", f"{xy(3, 3)}D02*", "G01*", f"{xy(12, 3)}D01*",
            f"{xy(12, 17)}D01*", f"{xy(3, 17)}D01*", f"{xy(3, 3)}D01*"]
    gerber("board/demo-Edge_Cuts.gbr", edge, "Profile,NP")
    gerber("board/demo-F_Cu.gbr", top, "Copper,L1,Top")
    gerber("board/demo-B_Cu.gbr", bottom, "Copper,L2,Bot")
    gerber("board/demo-F_Mask.gbr", mask, "Soldermask,Top")
    gerber("board/demo-B_Mask.gbr", mask, "Soldermask,Bot")
    gerber("board/demo-F_Silkscreen.gbr", silk, "Legend,Top")
    gerber("board/demo-F_Paste.gbr", ["%ADD10R,1.5X1.5*%", "D10*", f"{xy(5, 5)}D03*"], "Paste,Top")
    write("board/demo-PTH.drl", "\n".join([
        "M48", "; DRILL file {KiCad 8.0.4}", "; #@! TF.FileFunction,Plated,1,2,PTH", "METRIC",
        "T1C0.800", "T2C0.300", "%", "G90", "G05", "T1"] + [f"X{x:.1f}Y{y:.1f}" for x, y in pads]
        + ["T2", "X15.0Y15.0", "M30", ""]))
    write("board/demo-NPTH.drl", "\n".join([
        "M48", "; #@! TF.FileFunction,NonPlated,1,2,NPTH", "METRIC", "T1C3.000", "%", "G90", "G05",
        "T1", "X25.0Y10.0", "M30", ""]))
    write("board/demo-job.gbrjob", '{\n  "GeneralSpecs": {\n    "BoardThickness": 1.6\n  }\n}\n')


def main():
    apertures()
    macros()
    arcs()
    regions()
    polarity()
    step_repeat()
    blocks()
    x2()
    deprecated()
    drills()
    board()


if __name__ == "__main__":
    main()
