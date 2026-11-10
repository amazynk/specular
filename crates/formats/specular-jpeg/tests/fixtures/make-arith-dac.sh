#!/usr/bin/env bash
# Regenerates the arithmetic-coded fixtures with non-default conditioning (DAC) tables:
# arith_dac_*.jpg (8-bit) and dac12_*.jpg (12-bit), plus the Huffman-coded files they were
# transcoded from (huff_dac_*.jpg: the same coefficients). cjpeg cannot set the DC (L,U) and
# AC Kx conditioning, so a small libjpeg program transcodes the Huffman file's coefficients
# losslessly to arithmetic coding with the given tables (libjpeg-turbo writes them as a DAC
# segment). The 12-bit files take every coefficient times 16 (real 12-bit magnitudes, ~16x the
# 8-bit sample range) and are relabelled 12-bit afterwards; an 8-bit libjpeg cannot write
# 12-bit frames, but its arithmetic encoder codes any 16-bit coefficient.
# Each file stays under 64 KiB so the fuzz seed glob picks it up too.
# Needs: magick (ImageMagick 7), cjpeg, cc and the libjpeg(-turbo) headers. Writes next to
# this script.
set -euo pipefail
cd "$(dirname "$0")"
work=$(mktemp -d -p . dac-work.XXXXXX)
trap 'rm -rf "$work"' EXIT

cat >"$work/dac.c" <<'EOF'
/* dac IN OUT PROGRESSIVE RESTART MULT DC0_L DC0_U DC1_L DC1_U AC0_K AC1_K
 * Transcode IN's coefficients (times MULT) to arithmetic coding with the given conditioning;
 * MULT 16 relabels the frame 12-bit. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <jpeglib.h>

int main(int argc, char **argv) {
  if (argc != 12) {
    fprintf(stderr, "usage: dac IN OUT PROG RESTART MULT L0 U0 L1 U1 K0 K1\n");
    return 2;
  }
  FILE *in = fopen(argv[1], "rb");
  if (!in) return 1;
  struct jpeg_decompress_struct d;
  struct jpeg_compress_struct c;
  struct jpeg_error_mgr de, ce;
  d.err = jpeg_std_error(&de);
  jpeg_create_decompress(&d);
  jpeg_stdio_src(&d, in);
  jpeg_read_header(&d, TRUE);
  jvirt_barray_ptr *coefs = jpeg_read_coefficients(&d);
  int mult = atoi(argv[5]);
  if (mult != 1) {
    for (int ci = 0; ci < d.num_components; ci++) {
      jpeg_component_info *comp = &d.comp_info[ci];
      for (JDIMENSION by = 0; by < comp->height_in_blocks; by++) {
        JBLOCKARRAY row = (*d.mem->access_virt_barray)((j_common_ptr)&d, coefs[ci], by, 1, TRUE);
        for (JDIMENSION bx = 0; bx < comp->width_in_blocks; bx++)
          for (int k = 0; k < DCTSIZE2; k++) row[0][bx][k] = (JCOEF)(row[0][bx][k] * mult);
      }
    }
  }
  unsigned char *buf = NULL;
  unsigned long len = 0;
  c.err = jpeg_std_error(&ce);
  jpeg_create_compress(&c);
  jpeg_mem_dest(&c, &buf, &len);
  jpeg_copy_critical_parameters(&d, &c);
  c.arith_code = TRUE;
  c.optimize_coding = FALSE;
  if (atoi(argv[3])) jpeg_simple_progression(&c);
  c.restart_interval = (unsigned)atoi(argv[4]);
  c.arith_dc_L[0] = (UINT8)atoi(argv[6]);
  c.arith_dc_U[0] = (UINT8)atoi(argv[7]);
  c.arith_dc_L[1] = (UINT8)atoi(argv[8]);
  c.arith_dc_U[1] = (UINT8)atoi(argv[9]);
  c.arith_ac_K[0] = (UINT8)atoi(argv[10]);
  c.arith_ac_K[1] = (UINT8)atoi(argv[11]);
  jpeg_write_coefficients(&c, coefs);
  jpeg_finish_compress(&c);
  jpeg_finish_decompress(&d);
  if (mult != 1) {
    /* SOF9 / SOF10: FF Cx, length (2), precision. */
    unsigned long i;
    for (i = 0; i + 4 < len; i++)
      if (buf[i] == 0xFF && (buf[i + 1] == 0xC9 || buf[i + 1] == 0xCA)) break;
    if (i + 4 >= len || buf[i + 4] != 8) return 1;
    buf[i + 4] = 12;
  }
  FILE *out = fopen(argv[2], "wb");
  if (!out || fwrite(buf, 1, len, out) != len || fclose(out)) return 1;
  jpeg_destroy_compress(&c);
  jpeg_destroy_decompress(&d);
  fclose(in);
  free(buf);
  return 0;
}
EOF
cc -O2 -o "$work/dac" "$work/dac.c" -ljpeg

magick -seed 11 -size 203x141 plasma:fractal -blur 0x0.7 -attenuate 0.3 +noise Gaussian \
  -fill none -stroke black -strokewidth 2 -draw 'line 0,141 140,0' \
  -stroke white -draw 'circle 120,70 160,70' "ppm:$work/src.ppm"
cjpeg -quality 85 -sample 2x2 "$work/src.ppm" >huff_dac_420.jpg
cjpeg -quality 85 -sample 1x1 "$work/src.ppm" >huff_dac_444.jpg
cjpeg -quality 85 -grayscale "$work/src.ppm" >huff_dac_gray.jpg

#                                              prog rst mult  L0 U0  L1 U1  K0 K1
"$work/dac" huff_dac_420.jpg arith_dac_420.jpg 0 0 1 2 6 1 1 1 20
"$work/dac" huff_dac_444.jpg arith_dac_444_prog_rst.jpg 1 3 1 0 15 4 4 63 2
"$work/dac" huff_dac_gray.jpg arith_dac_gray.jpg 0 0 1 5 12 0 1 10 5
"$work/dac" huff_dac_420.jpg dac12_420_rst.jpg 0 4 16 3 9 0 2 3 40
"$work/dac" huff_dac_444.jpg dac12_444_prog.jpg 1 0 16 1 14 2 3 50 1
ls -l huff_dac_*.jpg arith_dac_*.jpg dac12_*.jpg
