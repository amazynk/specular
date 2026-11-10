# Sourced by fetch-textures.sh: Basis Universal v2.50 files (UASTC HDR 6×6 intermediate,
# XUASTC LDR at every block size, XUBC7) and references from the opengl_basisu wheel (its
# bundled v2.x transcoder, run from a venv under target/; nothing is built).

BASISU_TAG=v2_50
BASISU_RAW="https://raw.githubusercontent.com/BinomialLLC/basis_universal/$BASISU_TAG"

src_basisu2() { # basisu v2.50 test files: 6x6i, XUASTC LDR 4x4..12x12, XUBC7 (Apache-2.0)
  local dest="$OUT/basisu2" f
  mkdir -p "$dest"
  for f in webgl/texture_test/assets/desk_6x6i.basis webgl/texture_test/assets/desk_6x6.basis \
    test_files/base_xuastc_arith.ktx2 test_files/base_xuastc_zstd.ktx2 \
    python/shader_deblocking/kodim26_xubc7.ktx2 python/shader_deblocking/kodim14_12x12.ktx2 \
    python/shader_deblocking/kodim26_12x12.ktx2 \
    shader_deblocking_glfw/bin/t{4x4,5x4,5x5,6x5,6x6,8x5,8x6,8x8,10x5,10x6,10x8,10x10,12x10,12x12}.ktx2; do
    check_space
    curl -fsSL --retry 3 -o "$dest/$(basename "$f")" "$BASISU_RAW/$f"
    provenance "basisu2/$(basename "$f")" "$BASISU_RAW/$f" 'Apache-2.0 (Basis Universal)'
  done
  # The same picture as plain ASTC HDR 6×6 and as the 6×6 intermediate (Git LFS).
  for f in Desk_uastc_hdr6x6i.ktx2 Desk_astc_hdr6x6.ktx2; do
    curl -fsSL --retry 3 -o "$dest/$f" \
      "https://media.githubusercontent.com/media/KhronosGroup/KTX-Software/main/tests/resources/ktx2/$f"
    provenance "basisu2/$f" "KhronosGroup/KTX-Software main tests/resources/ktx2/$f" 'Apache-2.0'
  done
}

oracle_basisu2() { # references from the opengl_basisu wheel (basisu v2.x transcoder)
  local venv="$TARGET/corpus/basisu-wheel-venv"
  if [[ ! -x $venv/bin/python ]]; then
    python3 -m venv "$venv"
    PIP_CACHE_DIR="$venv/pip-cache" "$venv/bin/pip" install -q 'opengl_basisu==0.1.0a2' numpy
  fi
  "$venv/bin/python" -I "$HERE/basisu2_ref.py" "$OUT/basisu2"
}
