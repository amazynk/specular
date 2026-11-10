#!/usr/bin/env python3
"""Writes the small synthetic KiCad fixtures under tests/data (deterministic; run with -I).

They cover the format differences between KiCad 6, 7, 8 and 9 boards and schematics that
the reader handles, the legacy files it refuses, and broken input.
"""
import os

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, 'data')


def write(name, text):
    os.makedirs(OUT, exist_ok=True)
    with open(os.path.join(OUT, name), 'w', newline='\n') as f:
        f.write(text)


LAYERS6 = '''  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (34 "B.Paste" user)
    (35 "F.Paste" user)
    (36 "B.SilkS" user "B.Silkscreen")
    (37 "F.SilkS" user "F.Silkscreen")
    (38 "B.Mask" user)
    (39 "F.Mask" user)
    (44 "Edge.Cuts" user)
    (49 "F.Fab" user)
  )
'''

OUTLINE = '''  (gr_line (start 0 0) (end 50 0) (layer "Edge.Cuts") (width 0.1) (tstamp 00000000-0000-0000-0000-00000000e001))
  (gr_line (start 50 0) (end 50 30) (layer "Edge.Cuts") (width 0.1) (tstamp 00000000-0000-0000-0000-00000000e002))
  (gr_line (start 50 30) (end 0 30) (layer "Edge.Cuts") (width 0.1) (tstamp 00000000-0000-0000-0000-00000000e003))
  (gr_line (start 0 30) (end 0 0) (layer "Edge.Cuts") (width 0.1) (tstamp 00000000-0000-0000-0000-00000000e004))
'''

# KiCad 6: fp_text with bare hide, (width), tstamp, unquoted pad types.
BOARD6 = '''(kicad_pcb (version 20211014) (generator pcbnew)
  (general (thickness 1.6))
  (paper "A4")
  (title_block (title "Fixture six") (date "2026-10-19") (rev "A") (company "Specular"))
''' + LAYERS6 + '''  (net 0 "")
  (net 1 "GND")
  (net 2 "VCC")
  (footprint "Fixture:Pads" (layer "F.Cu") (tedit 0) (tstamp 00000000-0000-0000-0000-000000000001)
    (at 20 15 90)
    (fp_text reference "U1" (at 0 -4) (layer "F.SilkS")
      (effects (font (size 1 1) (thickness 0.15))))
    (fp_text value "Pads" (at 0 4) (layer "F.Fab") hide
      (effects (font (size 1 1) (thickness 0.15))))
    (fp_text user "${REFERENCE}" (at 0 0) (layer "F.Fab")
      (effects (font (size 0.8 0.8) (thickness 0.12))))
    (fp_line (start -3 -3) (end 3 -3) (layer "F.SilkS") (width 0.12))
    (fp_arc (start -3 3) (mid 0 4.2) (end 3 3) (layer "F.SilkS") (width 0.12))
    (fp_circle (center 0 0) (end 1 0) (layer "F.Fab") (width 0.1) (fill none))
    (fp_rect (start -4 -4) (end 4 4) (layer "F.Fab") (width 0.05) (fill none))
    (fp_poly (pts (xy -1 -1) (xy 1 -1) (xy 0 1)) (layer "F.SilkS") (width 0) (fill solid))
    (pad "1" smd rect (at -2 0 90) (size 1 0.6) (layers "F.Cu" "F.Paste" "F.Mask") (net 1 "GND"))
    (pad "2" smd roundrect (at -1 0 90) (size 1 0.6) (layers "F.Cu" "F.Paste" "F.Mask") (roundrect_rratio 0.25) (net 2 "VCC"))
    (pad "3" thru_hole circle (at 0 0 90) (size 1.6 1.6) (drill 0.8) (layers *.Cu *.Mask))
    (pad "4" thru_hole oval (at 1 0 90) (size 1.2 2) (drill oval 0.6 1.2 (offset 0 0.1)) (layers *.Cu *.Mask))
    (pad "5" smd trapezoid (at 2 0 90) (size 1 1) (rect_delta 0 0.3) (layers "F.Cu" "F.Mask"))
    (pad "6" smd roundrect (at 2 2 90) (size 1 1) (layers "F.Cu" "F.Mask") (roundrect_rratio 0) (chamfer_ratio 0.25) (chamfer top_left bottom_right))
    (pad "7" smd custom (at -2 2 90) (size 0.5 0.5) (layers "F.Cu" "F.Mask")
      (options (clearance outline) (anchor circle))
      (primitives
        (gr_poly (pts (xy 0 0) (xy 1 0) (xy 1 1)) (width 0) (fill yes))
        (gr_line (start 0 0) (end 0 1.5) (width 0.2))))
    (pad "" np_thru_hole circle (at 0 -2.5 90) (size 1 1) (drill 1) (layers *.Cu *.Mask))
  )
  (gr_text "Board six" (at 25 27) (layer "F.SilkS") (tstamp 00000000-0000-0000-0000-00000000f001)
    (effects (font (size 1.5 1.5) (thickness 0.3))))
''' + OUTLINE + '''  (segment (start 5 5) (end 15 5) (width 0.25) (layer "F.Cu") (net 1) (tstamp 00000000-0000-0000-0000-00000000a001))
  (segment (start 15 5) (end 15 25) (width 0.5) (layer "B.Cu") (net 1) (tstamp 00000000-0000-0000-0000-00000000a002))
  (arc (start 30 5) (mid 35 10) (end 40 5) (width 0.3) (layer "F.Cu") (net 2) (tstamp 00000000-0000-0000-0000-00000000a003))
  (via (at 15 5) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 1) (tstamp 00000000-0000-0000-0000-00000000a004))
  (zone (net 1) (net_name "GND") (layer "B.Cu") (tstamp 00000000-0000-0000-0000-00000000b001) (hatch edge 0.508)
    (connect_pads (clearance 0.5))
    (min_thickness 0.25)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon (pts (xy 30 15) (xy 45 15) (xy 45 25) (xy 30 25)))
    (filled_polygon (layer "B.Cu") (pts (xy 30.2 15.2) (xy 44.8 15.2) (xy 44.8 24.8) (xy 30.2 24.8)))
  )
)
'''

LAYERS8 = '''	(layers
		(0 "F.Cu" signal)
		(1 "In1.Cu" signal)
		(2 "In2.Cu" signal)
		(31 "B.Cu" signal)
		(36 "B.SilkS" user "B.Silkscreen")
		(37 "F.SilkS" user "F.Silkscreen")
		(38 "B.Mask" user)
		(39 "F.Mask" user)
		(44 "Edge.Cuts" user)
		(48 "B.Fab" user)
		(49 "F.Fab" user)
	)
'''

# KiCad 8: property fields, (hide yes), (stroke (width)), uuid, generator_version, a back-side
# footprint, inner layers, a text box and a dimension.
BOARD8 = '''(kicad_pcb
	(version 20240108)
	(generator "pcbnew")
	(generator_version "8.0")
	(general
		(thickness 1.2)
		(legacy_teardrops no)
	)
	(paper "A4")
''' + LAYERS8 + '''	(net 0 "")
	(net 1 "A")
	(footprint "Fixture:R"
		(layer "B.Cu")
		(uuid "00000000-0000-0000-0000-000000000011")
		(at 10 10 180)
		(property "Reference" "R7"
			(at 0 1.5 180)
			(layer "B.SilkS")
			(uuid "00000000-0000-0000-0000-000000000012")
			(effects (font (size 1 1) (thickness 0.15)) (justify mirror))
		)
		(property "Value" "10k"
			(at 0 -1.5 180)
			(layer "B.Fab")
			(uuid "00000000-0000-0000-0000-000000000013")
			(effects (font (size 1 1) (thickness 0.15)) (justify mirror))
		)
		(property "Footprint" "Fixture:R"
			(at 0 0 180)
			(layer "B.Fab")
			(hide yes)
			(uuid "00000000-0000-0000-0000-000000000014")
			(effects (font (size 1.27 1.27)) (justify mirror))
		)
		(attr smd)
		(fp_line (start -1 0.6) (end 1 0.6) (stroke (width 0.12) (type solid)) (layer "B.SilkS") (uuid "00000000-0000-0000-0000-000000000015"))
		(pad "1" smd roundrect (at -0.8 0 180) (size 0.8 0.9) (layers "B.Cu" "B.Mask") (roundrect_rratio 0.25) (net 1 "A") (uuid "00000000-0000-0000-0000-000000000016"))
		(pad "2" smd roundrect (at 0.8 0 180) (size 0.8 0.9) (layers "B.Cu" "B.Mask") (roundrect_rratio 0.25) (uuid "00000000-0000-0000-0000-000000000017"))
	)
	(footprint "Fixture:Hole"
		(layer "F.Cu")
		(uuid "00000000-0000-0000-0000-000000000021")
		(at 30 10)
		(property "Reference" "H1" (at 0 -3 0) (layer "F.SilkS") (uuid "00000000-0000-0000-0000-000000000022") (effects (font (size 1 1) (thickness 0.15))))
		(property "Value" "Hole" (at 0 3 0) (layer "F.Fab") (uuid "00000000-0000-0000-0000-000000000023") (effects (font (size 1 1) (thickness 0.15))))
		(attr through_hole)
		(pad "1" thru_hole circle (at 0 0) (size 3 3) (drill 1.6) (layers "*.Cu" "*.Mask") (uuid "00000000-0000-0000-0000-000000000024"))
	)
	(gr_rect (start 0 0) (end 40 20) (stroke (width 0.05) (type default)) (fill none) (layer "Edge.Cuts") (uuid "00000000-0000-0000-0000-000000000031"))
	(gr_text_box "Notes\\nline two" (start 2 14) (end 18 19) (layer "F.SilkS") (uuid "00000000-0000-0000-0000-000000000032")
		(effects (font (size 1 1) (thickness 0.15)) (justify left top))
		(border yes)
		(stroke (width 0.1) (type solid))
	)
	(dimension (type aligned) (layer "F.Fab") (uuid "00000000-0000-0000-0000-000000000033")
		(pts (xy 0 22) (xy 40 22)) (height 2)
		(gr_text "40.00 mm" (at 20 22.85 0) (layer "F.Fab") (uuid "00000000-0000-0000-0000-000000000034")
			(effects (font (size 1 1) (thickness 0.15))))
		(format (units 3) (units_format 1) (precision 2))
		(style (thickness 0.1) (arrow_length 1.27) (text_position_mode 0) (extension_height 0.58) (extension_offset 0.5) keep_text_aligned)
	)
	(segment (start 2 2) (end 20 2) (width 0.2) (layer "In1.Cu") (net 1) (uuid "00000000-0000-0000-0000-000000000041"))
	(segment (start 2 4) (end 20 4) (width 0.2) (layer "In2.Cu") (net 1) (uuid "00000000-0000-0000-0000-000000000042"))
	(via blind (at 20 2) (size 0.6) (drill 0.3) (layers "F.Cu" "In1.Cu") (net 1) (uuid "00000000-0000-0000-0000-000000000043"))
)
'''

# KiCad 9: new layer numbers, embedded fonts flag, a rule area (keepout, outline only), a
# polygon with an arc in its points.
BOARD9 = '''(kicad_pcb
	(version 20241229)
	(generator "pcbnew")
	(generator_version "9.0")
	(general (thickness 1.6) (legacy_teardrops no))
	(paper "A4")
	(layers
		(0 "F.Cu" signal)
		(2 "B.Cu" signal)
		(9 "F.Adhes" user "F.Adhesive")
		(5 "F.SilkS" user "F.Silkscreen")
		(1 "F.Mask" user)
		(25 "Edge.Cuts" user)
		(31 "F.CrtYd" user "F.Courtyard")
		(39 "User.1" user)
	)
	(net 0 "")
	(footprint "Fixture:C"
		(layer "F.Cu")
		(uuid "00000000-0000-0000-0000-000000000051")
		(at 10 5 45)
		(property "Reference" "C3" (at 0 -1.5 45) (layer "F.SilkS") (uuid "00000000-0000-0000-0000-000000000052")
			(effects (font (size 1 1) (thickness 0.15))))
		(property "Value" "1u" (at 0 1.5 45) (layer "F.Fab") (hide yes) (uuid "00000000-0000-0000-0000-000000000053")
			(effects (font (size 1 1) (thickness 0.15))))
		(fp_rect (start -1.5 -0.8) (end 1.5 0.8) (stroke (width 0.05) (type default)) (fill no) (layer "F.CrtYd") (uuid "00000000-0000-0000-0000-000000000054"))
		(pad "1" smd rect (at -0.5 0 45) (size 0.6 0.6) (layers "F.Cu" "F.Mask") (uuid "00000000-0000-0000-0000-000000000055"))
		(pad "2" smd rect (at 0.5 0 45) (size 0.6 0.6) (layers "F.Cu" "F.Mask") (uuid "00000000-0000-0000-0000-000000000056"))
		(embedded_fonts no)
	)
	(gr_poly (pts (xy 0 0) (xy 20 0) (arc (start 20 0) (mid 22 5) (end 20 10)) (xy 0 10))
		(stroke (width 0.1) (type solid)) (fill no) (layer "Edge.Cuts") (uuid "00000000-0000-0000-0000-000000000061"))
	(gr_circle (center 15 7) (end 16 7) (stroke (width 0.1) (type solid)) (fill yes) (layer "User.1") (uuid "00000000-0000-0000-0000-000000000062"))
	(zone (net 0) (net_name "") (layer "F.Cu") (uuid "00000000-0000-0000-0000-000000000063") (name "keepout") (hatch edge 0.5)
		(connect_pads (clearance 0))
		(min_thickness 0.25)
		(keepout (tracks not_allowed) (vias not_allowed) (pads allowed) (copperpour not_allowed) (footprints allowed))
		(fill (thermal_gap 0.5) (thermal_bridge_width 0.5))
		(polygon (pts (xy 2 2) (xy 6 2) (xy 6 6) (xy 2 6)))
	)
	(embedded_fonts no)
)
'''

LIB_R = '''    (symbol "Device:R" (pin_numbers hide) (pin_names (offset 0)) (in_bom yes) (on_board yes)
      (property "Reference" "R" (id 0) (at 2.032 0 90) (effects (font (size 1.27 1.27))))
      (property "Value" "R" (id 1) (at 0 0 90) (effects (font (size 1.27 1.27))))
      (symbol "R_0_1"
        (rectangle (start -1.016 -2.54) (end 1.016 2.54) (stroke (width 0.254) (type default) (color 0 0 0 0)) (fill (type none))))
      (symbol "R_1_1"
        (pin passive line (at 0 3.81 270) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))
        (pin passive line (at 0 -3.81 90) (length 1.27) (name "~" (effects (font (size 1.27 1.27)))) (number "2" (effects (font (size 1.27 1.27)))))))
'''

LIB_U = '''    (symbol "Logic:NAND" (pin_names (offset 1.016)) (in_bom yes) (on_board yes)
      (property "Reference" "U" (id 0) (at 0 5.08 0) (effects (font (size 1.27 1.27))))
      (property "Value" "NAND" (id 1) (at 0 -5.08 0) (effects (font (size 1.27 1.27))))
      (symbol "NAND_1_1"
        (rectangle (start -3.81 3.81) (end 3.81 -3.81) (stroke (width 0.254) (type default) (color 0 0 0 0)) (fill (type background)))
        (pin input line (at -7.62 2.54 0) (length 3.81) (name "A" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))
        (pin input inverted (at -7.62 -2.54 0) (length 3.81) (name "B" (effects (font (size 1.27 1.27)))) (number "2" (effects (font (size 1.27 1.27)))))
        (pin output clock (at 7.62 0 180) (length 3.81) (name "Y" (effects (font (size 1.27 1.27)))) (number "3" (effects (font (size 1.27 1.27))))))
      (symbol "NAND_2_1"
        (circle (center 0 0) (radius 3.81) (stroke (width 0.254) (type default) (color 0 0 0 0)) (fill (type background)))
        (pin input line (at -7.62 0 0) (length 3.81) (name "C" (effects (font (size 1.27 1.27)))) (number "4" (effects (font (size 1.27 1.27))))))
      (symbol "NAND_0_1"
        (text "&" (at 0 0 0) (effects (font (size 1.27 1.27))))))
'''

LIB_PWR = '''    (symbol "power:GND" (power) (pin_names (offset 0)) (in_bom yes) (on_board yes)
      (property "Reference" "#PWR" (id 0) (at 0 -6.35 0) (effects (font (size 1.27 1.27)) hide))
      (property "Value" "GND" (id 1) (at 0 -3.81 0) (effects (font (size 1.27 1.27))))
      (symbol "GND_0_1"
        (polyline (pts (xy 0 0) (xy 0 -1.27) (xy 1.27 -1.27) (xy 0 -2.54) (xy -1.27 -1.27) (xy 0 -1.27)) (stroke (width 0) (type default) (color 0 0 0 0)) (fill (type none))))
      (symbol "GND_1_1"
        (pin power_in line (at 0 0 270) (length 0) hide (name "GND" (effects (font (size 1.27 1.27)))) (number "1" (effects (font (size 1.27 1.27)))))))
'''

ROOT_UUID6 = '00000000-0000-0000-0000-0000000000a0'
SHEET_UUID6 = '00000000-0000-0000-0000-0000000000a1'

# KiCad 6: lib_symbols, bare hide in effects, symbol_instances in the root, a sub-sheet.
SCH6 = '''(kicad_sch (version 20211123) (generator eeschema)
  (uuid ''' + ROOT_UUID6 + ''')
  (paper "A4")
  (title_block (title "Fixture schematic") (date "2026-10-19") (rev "1") (company "Specular")
    (comment 1 "A comment"))
  (lib_symbols
''' + LIB_R + LIB_U + LIB_PWR + '''  )
  (junction (at 50.8 50.8) (diameter 0) (color 0 0 0 0) (uuid 00000000-0000-0000-0000-0000000000b1))
  (no_connect (at 80 40) (uuid 00000000-0000-0000-0000-0000000000b2))
  (wire (pts (xy 40.64 50.8) (xy 50.8 50.8)) (stroke (width 0) (type default) (color 0 0 0 0)) (uuid 00000000-0000-0000-0000-0000000000b3))
  (wire (pts (xy 50.8 50.8) (xy 50.8 60.96)) (stroke (width 0) (type default) (color 0 0 0 0)) (uuid 00000000-0000-0000-0000-0000000000b4))
  (wire (pts (xy 50.8 50.8) (xy 63.5 50.8)) (stroke (width 0) (type default) (color 0 0 0 0)) (uuid 00000000-0000-0000-0000-0000000000b5))
  (label "NET1" (at 55.88 50.8 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid 00000000-0000-0000-0000-0000000000b6))
  (global_label "CLK" (shape input) (at 40.64 50.8 180) (fields_autoplaced) (effects (font (size 1.27 1.27)) (justify right)) (uuid 00000000-0000-0000-0000-0000000000b7)
    (property "Intersheet References" "${INTERSHEET_REFS}" (id 0) (at 33 50.8 0) (effects (font (size 1.27 1.27)) (justify right) hide)))
  (hierarchical_label "OUT" (shape output) (at 63.5 50.8 0) (effects (font (size 1.27 1.27)) (justify left)) (uuid 00000000-0000-0000-0000-0000000000b8))
  (text "A note" (at 30 30 0) (effects (font (size 1.27 1.27)) (justify left bottom)) (uuid 00000000-0000-0000-0000-0000000000b9))
  (polyline (pts (xy 20 20) (xy 30 20)) (stroke (width 0) (type dash) (color 0 0 0 0)) (uuid 00000000-0000-0000-0000-0000000000ba))
  (symbol (lib_id "Device:R") (at 50.8 64.77 0) (unit 1) (in_bom yes) (on_board yes) (fields_autoplaced)
    (uuid 00000000-0000-0000-0000-0000000000c1)
    (property "Reference" "R?" (id 0) (at 53.34 63.5 0) (effects (font (size 1.27 1.27)) (justify left)))
    (property "Value" "10k" (id 1) (at 53.34 66.04 0) (effects (font (size 1.27 1.27)) (justify left)))
    (property "Footprint" "" (id 2) (at 49 64.77 90) (effects (font (size 1.27 1.27)) hide))
    (pin "1" (uuid 00000000-0000-0000-0000-0000000000c2))
    (pin "2" (uuid 00000000-0000-0000-0000-0000000000c3)))
  (symbol (lib_id "Logic:NAND") (at 100 50 90) (mirror x) (unit 2) (in_bom yes) (on_board yes)
    (uuid 00000000-0000-0000-0000-0000000000c4)
    (property "Reference" "U?" (id 0) (at 100 44 0) (effects (font (size 1.27 1.27))))
    (property "Value" "NAND" (id 1) (at 100 56 0) (effects (font (size 1.27 1.27)))))
  (symbol (lib_id "power:GND") (at 50.8 72.39 0) (unit 1) (in_bom yes) (on_board yes)
    (uuid 00000000-0000-0000-0000-0000000000c5)
    (property "Reference" "#PWR01" (id 0) (at 50.8 78.74 0) (effects (font (size 1.27 1.27)) hide))
    (property "Value" "GND" (id 1) (at 50.8 76.2 0) (effects (font (size 1.27 1.27)))))
  (symbol (lib_id "Missing:Thing") (at 120 80 0) (unit 1) (in_bom yes) (on_board yes)
    (uuid 00000000-0000-0000-0000-0000000000c6)
    (property "Reference" "X1" (id 0) (at 120 76 0) (effects (font (size 1.27 1.27)))))
  (sheet (at 140 40) (size 30 20) (fields_autoplaced)
    (stroke (width 0.1524) (type solid) (color 0 0 0 0))
    (fill (color 0 0 0 0.0000))
    (uuid ''' + SHEET_UUID6 + ''')
    (property "Sheet name" "power" (id 0) (at 140 39.3 0) (effects (font (size 1.27 1.27)) (justify left bottom)))
    (property "Sheet file" "sub6.kicad_sch" (id 1) (at 140 60.6 0) (effects (font (size 1.27 1.27)) (justify left top)))
    (pin "VIN" input (at 140 50 180) (effects (font (size 1.27 1.27)) (justify left))))
  (sheet_instances
    (path "/" (page "1"))
    (path "/''' + SHEET_UUID6 + '''" (page "2")))
  (symbol_instances
    (path "/00000000-0000-0000-0000-0000000000c5" (reference "#PWR01") (unit 1) (value "GND") (footprint ""))
    (path "/00000000-0000-0000-0000-0000000000c1" (reference "R1") (unit 1) (value "10k") (footprint ""))
    (path "/00000000-0000-0000-0000-0000000000c4" (reference "U1") (unit 2) (value "NAND") (footprint ""))
    (path "/''' + SHEET_UUID6 + '''/00000000-0000-0000-0000-0000000000d1" (reference "R5") (unit 1) (value "1k") (footprint "")))
)
'''

SUB6 = '''(kicad_sch (version 20211123) (generator eeschema)
  (uuid 00000000-0000-0000-0000-0000000000d0)
  (paper "A5")
  (lib_symbols
''' + LIB_R + '''  )
  (hierarchical_label "VIN" (shape input) (at 20 20 180) (effects (font (size 1.27 1.27)) (justify right)) (uuid 00000000-0000-0000-0000-0000000000d2))
  (wire (pts (xy 20 20) (xy 30 20)) (stroke (width 0) (type default) (color 0 0 0 0)) (uuid 00000000-0000-0000-0000-0000000000d3))
  (symbol (lib_id "Device:R") (at 30 30 90) (unit 1) (in_bom yes) (on_board yes)
    (uuid 00000000-0000-0000-0000-0000000000d1)
    (property "Reference" "R?" (id 0) (at 30 25 90) (effects (font (size 1.27 1.27))))
    (property "Value" "1k" (id 1) (at 30 35 90) (effects (font (size 1.27 1.27)))))
)
'''

LIB_R8 = LIB_R.replace('(pin_numbers hide)', '(pin_numbers (hide yes)) (exclude_from_sim no)').replace(
    '(symbol "R_0_1"', '(property "Footprint" "" (at 0 0 0) (effects (font (size 1.27 1.27)) (hide yes)))\n      (symbol "R_0_1"')

ROOT8 = '00000000-0000-0000-0000-0000000001a0'
SHEET8 = '00000000-0000-0000-0000-0000000001a1'

# KiCad 8: (hide yes), per-symbol instances, (exclude_from_sim), rectangle and text box
# graphics, a bus with an entry, a sheet used twice (two instances of one file).
SCH8 = '''(kicad_sch
	(version 20231120)
	(generator "eeschema")
	(generator_version "8.0")
	(uuid "''' + ROOT8 + '''")
	(paper "A3")
	(lib_symbols
''' + LIB_R8 + '''	)
	(bus (pts (xy 20 20) (xy 60 20)) (stroke (width 0) (type default)) (uuid "00000000-0000-0000-0000-0000000001b1"))
	(bus_entry (at 30 20) (size 2.54 2.54) (stroke (width 0) (type default)) (uuid "00000000-0000-0000-0000-0000000001b2"))
	(wire (pts (xy 32.54 22.54) (xy 32.54 40)) (stroke (width 0) (type default)) (uuid "00000000-0000-0000-0000-0000000001b3"))
	(rectangle (start 100 100) (end 140 120) (stroke (width 0) (type default)) (fill (type none)) (uuid "00000000-0000-0000-0000-0000000001b4"))
	(text_box "Boxed note" (exclude_from_sim no) (at 150 100 0) (size 30 10) (stroke (width 0) (type default)) (fill (type none))
		(effects (font (size 1.27 1.27)) (justify left top)) (uuid "00000000-0000-0000-0000-0000000001b5"))
	(symbol (lib_id "Device:R") (at 32.54 45 0) (unit 1) (exclude_from_sim no) (in_bom yes) (on_board yes) (dnp no)
		(uuid "00000000-0000-0000-0000-0000000001c1")
		(property "Reference" "R?" (at 35 44 0) (effects (font (size 1.27 1.27)) (justify left)))
		(property "Value" "47k" (at 35 46 0) (effects (font (size 1.27 1.27)) (justify left)))
		(property "Footprint" "" (at 30 45 90) (effects (font (size 1.27 1.27)) (hide yes)))
		(pin "1" (uuid "00000000-0000-0000-0000-0000000001c2"))
		(pin "2" (uuid "00000000-0000-0000-0000-0000000001c3"))
		(instances (project "fixture" (path "/''' + ROOT8 + '''" (reference "R9") (unit 1)))))
	(sheet (at 200 40) (size 30 20) (fields_autoplaced yes) (stroke (width 0.1524) (type solid)) (fill (color 0 0 0 0.0000))
		(uuid "''' + SHEET8 + '''")
		(property "Sheetname" "chan" (at 200 39.3 0) (effects (font (size 1.27 1.27)) (justify left bottom)))
		(property "Sheetfile" "sub8.kicad_sch" (at 200 60.6 0) (effects (font (size 1.27 1.27)) (justify left top)))
		(instances (project "fixture" (path "/''' + ROOT8 + '''" (page "2")))))
	(sheet (at 200 80) (size 30 20) (fields_autoplaced yes) (stroke (width 0.1524) (type solid)) (fill (color 0 0 0 0.0000))
		(uuid "00000000-0000-0000-0000-0000000001a2")
		(property "Sheetname" "chan2" (at 200 79.3 0) (effects (font (size 1.27 1.27)) (justify left bottom)))
		(property "Sheetfile" "sub8.kicad_sch" (at 200 100.6 0) (effects (font (size 1.27 1.27)) (justify left top)))
		(instances (project "fixture" (path "/''' + ROOT8 + '''" (page "3")))))
	(sheet (at 250 40) (size 30 20) (stroke (width 0.1524) (type solid)) (fill (color 0 0 0 0.0000))
		(uuid "00000000-0000-0000-0000-0000000001a3")
		(property "Sheetname" "gone" (at 250 39.3 0) (effects (font (size 1.27 1.27)) (justify left bottom)))
		(property "Sheetfile" "missing.kicad_sch" (at 250 60.6 0) (effects (font (size 1.27 1.27)) (justify left top))))
	(sheet_instances (path "/" (page "1")))
)
'''

SUB8 = '''(kicad_sch
	(version 20231120)
	(generator "eeschema")
	(generator_version "8.0")
	(uuid "00000000-0000-0000-0000-0000000001d0")
	(paper "A4")
	(lib_symbols
''' + LIB_R + '''	)
	(symbol (lib_id "Device:R") (at 50 50 0) (unit 1) (in_bom yes) (on_board yes) (dnp no)
		(uuid "00000000-0000-0000-0000-0000000001d1")
		(property "Reference" "R?" (at 53 49 0) (effects (font (size 1.27 1.27)) (justify left)))
		(property "Value" "100" (at 53 51 0) (effects (font (size 1.27 1.27)) (justify left)))
		(instances (project "fixture"
			(path "/''' + ROOT8 + '/' + SHEET8 + '''" (reference "R101") (unit 1))
			(path "/''' + ROOT8 + '''/00000000-0000-0000-0000-0000000001a2" (reference "R201") (unit 1)))))
)
'''

CYCLE = '''(kicad_sch (version 20231120) (generator "eeschema") (uuid "00000000-0000-0000-0000-0000000002a0") (paper "A4")
	(lib_symbols)
	(sheet (at 20 20) (size 20 10) (uuid "00000000-0000-0000-0000-0000000002a1")
		(property "Sheetname" "self" (at 20 19 0) (effects (font (size 1.27 1.27))))
		(property "Sheetfile" "cycle.kicad_sch" (at 20 31 0) (effects (font (size 1.27 1.27)))))
)
'''

KICAD5 = '''(kicad_pcb (version 20171130) (host pcbnew 5.1.10)
  (general (thickness 1.6) (drawings 0) (tracks 0) (zones 0) (modules 0) (nets 1))
  (page A4)
  (layers (0 F.Cu signal) (31 B.Cu signal))
  (net 0 "")
)
'''

LEGACY_SCH = '''EESchema Schematic File Version 4
EELAYER 30 0
EELAYER END
$Descr A4 11693 8268
$EndDescr
$EndSCHEMATC
'''

PCBNEW4 = '''PCBNEW-BOARD Version 1 date 1/1/2010
# Created by Pcbnew
$GENERAL
$EndGENERAL
$EndBOARD
'''

write('board6.kicad_pcb', BOARD6)
write('board8.kicad_pcb', BOARD8)
write('board9.kicad_pcb', BOARD9)
write('sch6.kicad_sch', SCH6)
write('sub6.kicad_sch', SUB6)
write('sch8.kicad_sch', SCH8)
write('sub8.kicad_sch', SUB8)
write('cycle.kicad_sch', CYCLE)
write('kicad5.kicad_pcb', KICAD5)
write('legacy.sch', LEGACY_SCH)
write('pcbnew4.brd', PCBNEW4)
write('truncated.kicad_pcb', BOARD6[: len(BOARD6) * 2 // 3])
write('not_kicad.kicad_pcb', '(kicad_wks (version 20220228) (generator pl_editor))\n')
