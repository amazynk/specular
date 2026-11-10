// Reference ETC1 / ETC2 / EAC decodes for the specular-texture tests, built against Ericsson's
// ETCPACK reference decoder (etcdec.cxx, compiled in from the oracle download).
//
// Usage: etc_ref MODE W H IN OUT
//   MODE  rgb | rgba1 | rgba8 | r11 | r11s | rg11 | rg11s
//   IN    the blocks of a W x H image in raster order
//   OUT   RGBA8 (W x H x 4) for rgb / rgba1 / rgba8; otherwise the 11-bit values as
//         little-endian int16, W x H x channels (unsigned 0..2047, signed -1023..1023)
#define PGMOUT 0
#include "etcdec.cxx"

#include <cstring>
#include <string>
#include <vector>

static unsigned be32(const unsigned char* p) {
  return (unsigned(p[0]) << 24) | (unsigned(p[1]) << 16) | (unsigned(p[2]) << 8) | p[3];
}

// The 11-bit value back from ETCPACK's 16-bit extension.
static int eleven(int16 v, bool sign) {
  if (!sign) return (unsigned short)v >> 5;
  return v < 0 ? -((-v) >> 5) : v >> 5;
}

int main(int argc, char** argv) {
  if (argc != 6) {
    fprintf(stderr, "usage: etc_ref MODE W H IN OUT\n");
    return 2;
  }
  std::string mode = argv[1];
  int w = atoi(argv[2]), h = atoi(argv[3]);
  FILE* f = fopen(argv[4], "rb");
  if (!f) return 1;
  std::vector<unsigned char> in;
  unsigned char buf[65536];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, f)) > 0) in.insert(in.end(), buf, buf + n);
  fclose(f);
  setupAlphaTable();
  bool colour = mode == "rgb" || mode == "rgba1" || mode == "rgba8";
  int bytes = (mode == "rgba8" || mode == "rg11" || mode == "rg11s") ? 16 : 8;
  int ch = colour ? 4 : (bytes == 16 ? 2 : 1);
  int bx = (w + 3) / 4, by = (h + 3) / 4;
  if (in.size() < size_t(bx) * by * bytes) {
    fprintf(stderr, "etc_ref: input too short\n");
    return 1;
  }
  std::vector<unsigned char> out8(colour ? size_t(w) * h * 4 : 0);
  std::vector<int16> out16(colour ? 0 : size_t(w) * h * ch);
  bool sign = mode == "r11s" || mode == "rg11s";
  formatSigned = sign;
  for (int y = 0; y < by; y++) {
    for (int x = 0; x < bx; x++) {
      const unsigned char* b = &in[(size_t(y) * bx + x) * bytes];
      unsigned char px[64];
      unsigned char alpha[16];
      int16 v[2][16];
      memset(px, 0, sizeof px);
      if (mode == "rgb") {
        decompressBlockETC2c(be32(b), be32(b + 4), px, 4, 4, 0, 0, 4);
        for (int i = 0; i < 16; i++) px[i * 4 + 3] = 255;
      } else if (mode == "rgba1") {
        decompressBlockETC21BitAlphaC(be32(b), be32(b + 4), px, alpha, 4, 4, 0, 0, 4);
      } else if (mode == "rgba8") {
        decompressBlockETC2c(be32(b + 8), be32(b + 12), px, 4, 4, 0, 0, 4);
        decompressBlockAlphaC((uint8*)b, px + 3, 4, 4, 0, 0, 4);
      } else {
        decompressBlockAlpha16bitC((uint8*)b, (uint8*)v[0], 4, 4, 0, 0, 1);
        if (ch == 2) decompressBlockAlpha16bitC((uint8*)b + 8, (uint8*)v[1], 4, 4, 0, 0, 1);
      }
      for (int yy = 0; yy < 4; yy++) {
        for (int xx = 0; xx < 4; xx++) {
          int X = x * 4 + xx, Y = y * 4 + yy;
          if (X >= w || Y >= h) continue;
          if (colour) {
            memcpy(&out8[(size_t(Y) * w + X) * 4], &px[(yy * 4 + xx) * 4], 4);
          } else {
            for (int c = 0; c < ch; c++)
              out16[(size_t(Y) * w + X) * ch + c] = int16(eleven(v[c][yy * 4 + xx], sign));
          }
        }
      }
    }
  }
  f = fopen(argv[5], "wb");
  if (!f) return 1;
  if (colour) {
    fwrite(out8.data(), 1, out8.size(), f);
  } else {
    for (int16 s : out16) {
      unsigned char le[2] = {(unsigned char)(s & 0xff), (unsigned char)((unsigned short)s >> 8)};
      fwrite(le, 1, 2, f);
    }
  }
  fclose(f);
  return 0;
}
