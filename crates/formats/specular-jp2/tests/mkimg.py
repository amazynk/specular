import sys, math, random
def img(w, h, ch, seed, path, maxv=255):
    random.seed(seed)
    out = bytearray()
    big = maxv > 255
    for y in range(h):
        for x in range(w):
            for c in range(ch):
                v = (math.sin(x*0.05*(c+1)+y*0.03)*0.35 + 0.5 + ((x//17 + y//13) % 2)*0.1 + random.random()*0.08 - 0.04)
                if c == 3 or (ch == 2 and c == 1):
                    # Alpha: a soft-edged disc, transparent outside.
                    d = math.hypot(x - w / 2, y - h / 2) / (min(w, h) / 2)
                    v = max(0.0, min(1.0, (1.0 - d) * 3))
                v = int(max(0, min(1, v)) * maxv)
                if big: out += bytes([v >> 8, v & 255])
                else: out.append(v)
    if ch in (2, 4):
        tt = "GRAYSCALE_ALPHA" if ch == 2 else "RGB_ALPHA"
        hdr = f"P7\nWIDTH {w}\nHEIGHT {h}\nDEPTH {ch}\nMAXVAL {maxv}\nTUPLTYPE {tt}\nENDHDR\n".encode()
    else:
        hdr = f"P{6 if ch==3 else 5}\n{w} {h}\n{maxv}\n".encode()
    open(path, "wb").write(hdr + out)
img(int(sys.argv[1]), int(sys.argv[2]), int(sys.argv[3]), 1, sys.argv[4], int(sys.argv[5]) if len(sys.argv) > 5 else 255)
