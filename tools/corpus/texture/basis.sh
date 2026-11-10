# Basis Universal (ETC1S / BasisLZ and UASTC LDR 4x4) references. Sourced by
# tools/corpus/fetch-textures.sh, which provides OUT, ORACLES, ROOT, check_space, provenance, die.
#
# oracle_basis:
#   1. Fetches BinomialLLC/basis_universal at BASISU_COMMIT into $ORACLES/basis_universal.
#   2. Builds tools/corpus/texture/basisu_ref.cpp (our driver over the upstream transcoder
#      library) into $ORACLES/basisu_ref, and the upstream basisu encoder (cmake) into
#      $ORACLES/basis_universal/bin/basisu (cmake build dir
#      $ORACLES/basisu-build).
#   3. Encodes extra KTX2 test files with the encoder into $OUT/basis/gen/*.ktx2 (UASTC with and
#      without RDO / zstd, ETC1S with and without alpha, a normal map, mipmaps, an array).
#   4. For every Basis KTX2 (DFD colour model 163 ETC1S or 166 UASTC) under
#      $OUT/ktx-software/tests/resources/ktx2 and $OUT/basis/gen, writes the transcoder's RGBA32
#      output for every level, layer and face to
#        $OUT/basis/ref/<file stem>.l<level>.a<layer>.f<face>.rgba
#      (raw RGBA8, the level's width x height, rows top to bottom as stored), plus
#        $OUT/basis/ref/<file stem>.src   one line: the KTX2 path relative to $OUT.
#   Files the transcoder rejects (e.g. UASTC HDR) get no references.

BASISU_COMMIT=99f52d63aa6799cbdaecfe977111dc5ec3b31d47

oracle_basis() {
  local src="$ORACLES/basis_universal" gen="$OUT/basis/gen" ref="$OUT/basis/ref"
  check_space
  if [[ ! -f $src/.fetched-$BASISU_COMMIT ]]; then
    rm -rf "$src"
    git init -q "$src"
    git -C "$src" remote add origin https://github.com/BinomialLLC/basis_universal
    git -C "$src" fetch -q --depth 1 --filter=blob:none origin "$BASISU_COMMIT"
    git -C "$src" -c advice.detachedHead=false checkout -q FETCH_HEAD
    touch "$src/.fetched-$BASISU_COMMIT"
  fi
  local zstd_src=$src/zstd/zstddeclib.c
  [[ -f $zstd_src ]] || zstd_src=$src/zstd/zstd.c
  if [[ ! -x $ORACLES/basisu_ref || $ROOT/tools/corpus/texture/basisu_ref.cpp -nt $ORACLES/basisu_ref ]]; then
    echo "  building basisu_ref"
    nice g++ -O2 -std=c++17 -w -DBASISD_SUPPORT_KTX2_ZSTD=1 -I"$src/transcoder" -I"$src/zstd" \
      -o "$ORACLES/basisu_ref" "$ROOT/tools/corpus/texture/basisu_ref.cpp" \
      "$src/transcoder/basisu_transcoder.cpp" -x c "$zstd_src" -lpthread
  fi
  # The upstream CMakeLists puts the binary in the source tree's bin/.
  local basisu=$src/bin/basisu
  if [[ ! -x $basisu ]]; then
    echo "  building the basisu encoder"
    check_space
    nice cmake -S "$src" -B "$ORACLES/basisu-build" -DCMAKE_BUILD_TYPE=Release \
      -DBASISU_SSE=OFF -DBASISU_OPENCL=OFF >/dev/null
    nice cmake --build "$ORACLES/basisu-build" -j 2 --target basisu >/dev/null
  fi

  mkdir -p "$gen" "$ref"
  if [[ -x $basisu ]]; then
    local imgs=$src/test_files ktxres="$OUT/ktx-software/tests/resources"
    # gen NAME ARGS... (inputs are given with -file inside ARGS)
    gen() {
      local name=$1; shift
      [[ -f $gen/$name.ktx2 ]] && return 0
      (cd "$gen" && nice "$basisu" -ktx2 -no_multithreading "$@" -output_file "$gen/$name.ktx2" \
        >/dev/null 2>&1) || echo "  basisu failed: $name"
    }
    local kodim=$imgs/kodim03.png alpha=$imgs/alpha0.png normal=$imgs/kodim18.png
    local k1=$imgs/kodim01.png k2=$imgs/kodim02.png k5=$imgs/kodim05.png k6=$imgs/kodim06.png
    gen uastc_l0 -uastc -uastc_level 0 -file "$kodim"
    gen uastc_l2 -uastc -uastc_level 2 -file "$kodim"
    gen uastc_l4_zstd -uastc -uastc_level 4 -ktx2_zstandard_level 6 -file "$kodim"
    gen uastc_rdo_zstd -uastc -uastc_level 2 -uastc_rdo_l 1.0 -file "$kodim"
    gen uastc_alpha -uastc -uastc_level 3 -file "$alpha"
    gen uastc_alpha_mips -uastc -uastc_level 1 -mipmap -file "$alpha"
    gen uastc_rg_alpha -uastc -uastc_level 3 -separate_rg_to_color_alpha -file "$k1"
    gen uastc_npot -uastc -resample 77 53 -file "$kodim"
    gen uastc_array -uastc -tex_array -file "$k1" -file "$k2" -file "$kodim"
    gen etc1s_q64 -q 64 -file "$kodim"
    gen etc1s_q255_mips -q 255 -mipmap -file "$kodim"
    gen etc1s_comp1 -q 128 -comp_level 1 -file "$k5"
    gen etc1s_comp5 -q 200 -comp_level 5 -file "$k6"
    gen etc1s_alpha -q 128 -file "$alpha"
    gen etc1s_alpha_mips -q 190 -mipmap -file "$alpha"
    gen etc1s_rg_alpha -q 160 -separate_rg_to_color_alpha -mipmap -file "$k1"
    gen etc1s_normal -normal_map -q 200 -file "$normal"
    gen etc1s_npot -q 128 -resample 77 53 -file "$kodim"
    gen etc1s_array -q 128 -tex_array -file "$k1" -file "$k2" -file "$kodim"
    gen etc1s_cube -q 128 -cubemap -resample 128 128 -file "$k1" -file "$k2" -file "$kodim" \
      -file "$k5" -file "$k6" -file "$normal"
    # Texture video: P-frames (conditional replenishment) after the first I-frame.
    gen etc1s_video -q 128 -tex_type video -file "$k1" -file "$k1" -file "$k2" -file "$k2"
    gen etc1s_video_alpha -q 128 -tex_type video -separate_rg_to_color_alpha -file "$k1" \
      -file "$k1" -file "$k2"
    for f in "$gen"/*.ktx2; do provenance "basis/gen/$(basename "$f")" \
      "generated by basisu $BASISU_COMMIT from basis_universal test_files" \
      'Apache-2.0 (basis_universal); test images'; done
  else
    echo "  no basisu encoder: skipping generated files"
  fi

  local n=0 f
  for f in "$OUT"/ktx-software/tests/resources/ktx2/*.ktx2 "$gen"/*.ktx2; do
    [[ -f $f ]] || continue
    local stem
    stem=$(basename "$f" .ktx2)
    [[ $f == "$gen"/* ]] && stem=gen_$stem
    if "$ORACLES/basisu_ref" "$f" "$ref/$stem" >/dev/null 2>&1; then
      echo "${f#"$OUT"/}" > "$ref/$stem.src"
      n=$((n + 1))
    else
      rm -f "$ref/$stem".*.rgba "$ref/$stem.src"
    fi
  done
  echo "  basis references: $n files"
}
