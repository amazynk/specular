# Vet, orthopaedic and cephalometric geometry in the measuring tool. Every point is placed
# by hand; nothing is detected or graded.

measure-tool-geometry = Geometry
measure-tool-hint-geometry = Pick a template, then place each point it asks for.
measure-tool-template = Template: { $name }

geometry-prompt = { $template }, point { $n } of { $total }: { $point }
geometry-readout = { $name }: { $value }
geometry-value-angle = { $value }°
geometry-value-vertebrae = { $value } v
geometry-extrapolated = { $value } (past T9, estimated)
geometry-degenerate = Points too close or in a line. Move a point.
geometry-moved = Point moved. { $value }
# A placed point moved with the keyboard: [ and ] go to it, Space picks it up.
geometry-picked = { $point } picked up. Arrow keys move it, Shift for bigger steps. Space puts it down, Esc puts it back.
geometry-put-back = Point put back.

# Templates.
geometry-vhs = Vertebral heart score (VHS)
geometry-vlas = Vertebral left atrial size (VLAS)
geometry-norberg = Norberg angle
geometry-tpa = Tibial plateau angle (TPA)
geometry-distraction-index = Distraction index
geometry-femoral-head = Femoral head circle
geometry-femoral-offset = Femoral offset
geometry-leg-length = Leg length
geometry-acetabular-angle = Acetabular angle (Sharp)
geometry-steiner = Steiner analysis
geometry-downs = Downs analysis

# Points, in the order they are asked for.
geometry-pt-long-axis-carina = long axis, ventral carina
geometry-pt-long-axis-apex = long axis, cardiac apex
geometry-pt-short-axis = short axis, end { $n }
geometry-pt-t4-cranial = cranial end of T4
geometry-pt-vertebra-caudal = caudal end of T{ $n }
geometry-pt-vlas-carina = ventral carina
geometry-pt-vlas-atrium = left atrium at the caudal vena cava
geometry-pt-left-head-rim = left femoral head, rim point { $n } of 3
geometry-pt-right-head-rim = right femoral head, rim point { $n } of 3
geometry-pt-left-acetabular-rim = left craniolateral acetabular rim
geometry-pt-right-acetabular-rim = right craniolateral acetabular rim
geometry-pt-eminence = centre of the intercondylar eminences
geometry-pt-talus = centre of the talus
geometry-pt-plateau-cranial = cranial edge of the medial tibial plateau
geometry-pt-plateau-caudal = caudal edge of the medial tibial plateau
geometry-pt-head-rim = femoral head, rim point { $n } of 3
geometry-pt-acetabulum-rim = acetabulum, rim point { $n } of 3
geometry-pt-shaft-proximal = femoral shaft axis, upper point
geometry-pt-shaft-distal = femoral shaft axis, lower point
geometry-pt-reference = pelvic reference line, point { $n }
geometry-pt-left-landmark = left leg landmark
geometry-pt-right-landmark = right leg landmark
geometry-pt-teardrop = teardrop, this side
geometry-pt-teardrop-other = teardrop, other side
geometry-pt-acetabular-edge = lateral acetabular edge
geometry-pt-s = sella (S)
geometry-pt-n = nasion (N)
geometry-pt-a = point A
geometry-pt-b = point B
geometry-pt-pog = pogonion (Pog)
geometry-pt-go = gonion (Go)
geometry-pt-gn = gnathion (Gn)
geometry-pt-me = menton (Me)
geometry-pt-po = porion (Po)
geometry-pt-or = orbitale (Or)
geometry-pt-u1-tip = upper incisor tip
geometry-pt-u1-apex = upper incisor apex
geometry-pt-l1-tip = lower incisor tip
geometry-pt-l1-apex = lower incisor apex
geometry-pt-occlusal-front = occlusal plane, front point
geometry-pt-occlusal-back = occlusal plane, back point

# Results.
geometry-out-vhs = VHS
geometry-out-long-axis = Long axis
geometry-out-short-axis = Short axis
geometry-out-long-axis-length = Long axis length
geometry-out-short-axis-length = Short axis length
geometry-out-vlas = VLAS
geometry-out-vlas-length = VLAS length
geometry-out-norberg-left = Norberg left
geometry-out-norberg-right = Norberg right
geometry-out-tpa = TPA
geometry-out-distraction-index = Distraction index
geometry-out-centre-distance = Centre distance
geometry-out-head-radius = Head radius
geometry-out-head-diameter = Head diameter
geometry-out-offset = Offset
geometry-out-leg-difference = Difference (left − right)
geometry-out-leg-left = Left
geometry-out-leg-right = Right
geometry-out-acetabular-angle = Acetabular angle
geometry-out-sna = SNA
geometry-out-snb = SNB
geometry-out-anb = ANB
geometry-out-sn-gogn = SN–GoGn
geometry-out-u1-na-angle = U1–NA angle
geometry-out-u1-na-mm = U1–NA distance
geometry-out-l1-nb-angle = L1–NB angle
geometry-out-l1-nb-mm = L1–NB distance
geometry-out-pog-nb-mm = Pog–NB distance
geometry-out-interincisal = Interincisal angle
geometry-out-occlusal-sn = Occlusal plane–SN
geometry-out-facial-angle = Facial angle
geometry-out-convexity = Angle of convexity
geometry-out-ab-plane = A–B plane angle
geometry-out-mandibular-plane = Mandibular plane angle
geometry-out-y-axis = Y axis
geometry-out-occlusal-cant = Occlusal plane cant
geometry-out-l1-occlusal = L1 to occlusal plane
geometry-out-l1-mandibular = L1 to mandibular plane
geometry-out-u1-apog-mm = U1 to A–Pog
