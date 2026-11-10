# shellcheck shell=bash
# ASTC oracle section for fetch-textures.sh (sourced; uses OUT, ORACLES, check_space, provenance,
# die). Builds ARM astc-encoder (astcenc) at a pinned commit into $ORACLES/astcenc, generates
# source images, encodes them at all 14 2D footprints and decodes them back with astcenc.
#
# Layout under $OUT/astc/:
#   src/<image>.png | .hdr          generated sources (python, deterministic)
#   gen/<image>_<WxH>_<mode>_<quality>.astc
#                                   astcenc output; mode is cl (LDR linear), cs (LDR sRGB),
#                                   ch (HDR colour, LDR alpha) or cH (HDR colour and alpha)
#   ktx2/<name>.astc                level 0, first image of the KTX-Software ASTC KTX2 files
#                                   (zstd undone), with the profile in the name (_ldr_ / _srgb_
#                                   / _hdr_)
#   <same path>.<d>.ktx             astcenc decode of each .astc with -d<d> (d = l, s or h):
#                                   uncompressed KTX1, RGBA8 (l, s) or RGBA16F (h)
# A .astc file is the astcenc container: magic 13 AB A1 5C, block w/h/d bytes, then x, y, z as
# 24-bit little-endian sizes, then the blocks.

ASTCENC_COMMIT=18b20e6641503c41b2aac55664a2400534ba7212

astc_build() {
  local dst="$ORACLES/astcenc" bin="$ORACLES/astcenc/bin/astcenc-none"
  [[ -x $bin ]] && return 0
  check_space
  rm -rf "$dst"
  mkdir -p "$dst"
  curl -fsSL --retry 3 -o "$dst/src.tar.gz" \
    "https://github.com/ARM-software/astc-encoder/archive/$ASTCENC_COMMIT.tar.gz"
  tar -xzf "$dst/src.tar.gz" -C "$dst"
  rm "$dst/src.tar.gz"
  nice cmake -S "$dst/astc-encoder-$ASTCENC_COMMIT" -B "$dst/build" -G Ninja \
    -DCMAKE_BUILD_TYPE=Release -DASTCENC_ISA_NONE=ON -DASTCENC_CLI=ON -DASTCENC_UNITTEST=OFF \
    -DCMAKE_INSTALL_PREFIX="$dst" >/dev/null
  nice ninja -C "$dst/build" -j 2 >/dev/null
  nice ninja -C "$dst/build" install >/dev/null
  [[ -x $bin ]] || die "astcenc build produced no $bin"
}

# Deterministic source images: smooth gradients with alpha, noise and colour patches, sharp
# shapes; one Radiance HDR with values up to ~60.
astc_sources() {
  local src=$1
  python3 -I - "$src" <<'PY'
import struct, sys, zlib
import numpy as np

out = sys.argv[1]
rng = np.random.default_rng(1234)
W, H = 101, 77  # not a multiple of any footprint: partial edge blocks

def png(path, a):
    h, w, c = a.shape
    ct = {3: 2, 4: 6}[c]
    raw = b"".join(b"\0" + a[y].tobytes() for y in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d))
    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, ct, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    open(path, "wb").write(data)

y, x = np.mgrid[0:H, 0:W].astype(np.float64)
# 1: smooth gradients + radial alpha
g = np.stack([x / W, y / H, 0.5 + 0.5 * np.sin(x / 9.0 + y / 13.0),
              np.clip(1.2 - np.hypot(x - W / 2, y - H / 2) / (W / 2), 0, 1)], -1)
png(f"{out}/gradient.png", (g * 255 + 0.5).astype(np.uint8))
# 2: noise and colour patches, opaque
n = rng.integers(0, 256, (H, W, 3), dtype=np.uint8)
patch = ((x // 13 + y // 11) % 7)[..., None] * np.array([37, 91, 53]) % 256
mix = np.where(((x // 20 + y // 20) % 2 == 0)[..., None], n, patch.astype(np.uint8))
png(f"{out}/noise.png", mix.astype(np.uint8))
# 3: sharp shapes, binary-ish alpha, a greyscale band
s = np.zeros((H, W, 4), np.uint8)
s[..., 3] = 255
s[(x - 30) ** 2 + (y - 30) ** 2 < 400] = [250, 20, 30, 255]
s[(x > 55) & (y > 10) & (y < 50)] = [20, 200, 60, 128]
s[(x + y) % 17 < 3] = [10, 10, 240, 0]
band = (y > 60)
s[band, 0] = s[band, 1] = s[band, 2] = (x[band] * 2.5).astype(np.uint8)
png(f"{out}/shapes.png", s)
# 4: HDR (Radiance RGBE, flat), values from 0 to ~60 plus some tiny ones
hdr = np.stack([np.exp2(x / W * 12 - 6), np.exp2(y / H * 10 - 7),
                0.02 + 3 * (0.5 + 0.5 * np.cos(x / 7.0 + y / 5.0)) ** 4], -1)
hdr[(x // 16 + y // 16) % 3 == 0] *= rng.uniform(0.1, 2.0, ((x // 16 + y // 16) % 3 == 0).sum())[:, None]
m = hdr.max(-1)
e = np.floor(np.log2(np.maximum(m, 1e-30))) + 1
sc = np.where(m > 1e-32, 256.0 / np.exp2(e), 0)
rgbe = np.concatenate([np.clip(hdr * sc[..., None], 0, 255).astype(np.uint8),
                       np.where(m > 1e-32, e + 128, 0)[..., None].astype(np.uint8)], -1)
with open(f"{out}/hdr.hdr", "wb") as f:
    f.write(b"#?RADIANCE\nFORMAT=32-bit_rle_rgbe\n\n-Y %d +X %d\n" % (H, W))
    f.write(rgbe.tobytes())
PY
}

# Level 0, image 0 of an ASTC KTX2 file → .astc (zstd via the zstd CLI).
astc_from_ktx2() {
  local in=$1 out=$2
  python3 -I - "$in" "$out" <<'PY'
import struct, subprocess, sys
src, dst = sys.argv[1], sys.argv[2]
d = open(src, "rb").read()
vk, _, w, h, dep, layers, faces, levels, scheme = struct.unpack_from("<9I", d, 12)
off, ln, uln = struct.unpack_from("<3Q", d, 80)
data = d[off:off + ln]
if scheme == 2:
    data = subprocess.run(["zstd", "-dcq"], input=data, capture_output=True, check=True).stdout
elif scheme != 0:
    sys.exit(f"{src}: supercompression {scheme}")
fp = {157: (4, 4), 159: (5, 4), 161: (5, 5), 163: (6, 5), 165: (6, 6), 167: (8, 5), 169: (8, 6),
      171: (8, 8), 173: (10, 5), 175: (10, 6), 177: (10, 8), 179: (10, 10), 181: (12, 10),
      183: (12, 12)}
k = vk
if 1000066000 <= vk <= 1000066013:
    bw, bh = list(fp.values())[vk - 1000066000]
else:
    bw, bh = fp[vk if vk % 2 == 1 else vk - 1]
n = -(-w // bw) * -(-h // bh) * 16
open(dst, "wb").write(bytes([0x13, 0xAB, 0xA1, 0x5C, bw, bh, 1]) + w.to_bytes(3, "little")
                      + h.to_bytes(3, "little") + (1).to_bytes(3, "little") + data[:n])
PY
}

oracle_astc() {
  astc_build
  local enc="$ORACLES/astcenc/bin/astcenc-none" dir="$OUT/astc"
  mkdir -p "$dir/src" "$dir/gen" "$dir/ktx2"
  astc_sources "$dir/src"
  provenance "astc/src" "generated by tools/corpus/texture/astc.sh" "generated"
  local fps="4x4 5x4 5x5 6x5 6x6 8x5 8x6 8x8 10x5 10x6 10x8 10x10 12x10 12x12"
  local fp img q mode name
  for fp in $fps; do
    for q in fast thorough; do
      for img in gradient noise shapes; do
        for mode in cl cs; do
          name="$dir/gen/${img}_${fp}_${mode}_${q}.astc"
          [[ -f $name ]] || nice "$enc" "-$mode" "$dir/src/$img.png" "$name" "$fp" "-$q" \
            -silent -j 2
        done
      done
      for mode in ch cH; do
        name="$dir/gen/hdr_${fp}_${mode}_${q}.astc"
        [[ -f $name ]] || nice "$enc" "-$mode" "$dir/src/hdr.hdr" "$name" "$fp" "-$q" \
          -silent -j 2
        # LDR source through the HDR encoder (HDR colour endpoints with alpha).
        name="$dir/gen/gradienth_${fp}_${mode}_${q}.astc"
        [[ -f $name ]] || nice "$enc" "-$mode" "$dir/src/gradient.png" "$name" "$fp" "-$q" \
          -silent -j 2
      done
    done
  done
  local k2="$OUT/ktx-software/tests/resources/ktx2" base
  for base in Desk_astc_hdr6x6:hdr Desk_uastc_hdr4x4_zstd_15:hdr astc_8x8_unorm_array_7:ldr \
              ktx_app_astc_8x8:srgb r8g8b8a8_srgb_mip_astc:srgb; do
    local f=${base%%:*} p=${base##*:}
    if [[ -f $k2/$f.ktx2 ]]; then
      astc_from_ktx2 "$k2/$f.ktx2" "$dir/ktx2/${f}_${p}_.astc"
    else
      echo "  (missing $f.ktx2: run src_ktx_software first)"
    fi
  done
  # References: LDR files decode with -dl and -ds, HDR files with -dh (and -dl, which shows the
  # LDR profile's error colour for HDR endpoints).
  local a ds d
  while IFS= read -r -d '' a; do
    case $a in
      *_ch_*|*_cH_*|*_hdr_*) ds="h l" ;;
      *) ds="l s" ;;
    esac
    for d in $ds; do
      [[ -f $a.$d.ktx ]] || nice "$enc" "-d$d" "$a" "$a.$d.ktx" -silent -j 2
    done
  done < <(find "$dir/gen" "$dir/ktx2" -name '*.astc' -print0)
  echo "  astc: $(find "$dir" -name '*.astc' | wc -l) files, $(find "$dir" -name '*.ktx' | wc -l) references"
}
