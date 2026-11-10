// Reference PVRTC1 decodes for the specular-texture tests, built against the PowerVR SDK's
// PVRTDecompress.cpp (compiled in from the oracle download).
//
// Usage: pvrtc_ref BPP W H IN OUT
//   BPP 2 or 4; IN the level's blocks (Morton order); OUT RGBA8, W x H x 4.
#include "PVRTDecompress.cpp"

#include <vector>

int main(int argc, char** argv) {
  if (argc != 6) {
    fprintf(stderr, "usage: pvrtc_ref BPP W H IN OUT\n");
    return 2;
  }
  int bpp = atoi(argv[1]);
  uint32_t w = uint32_t(atoi(argv[2])), h = uint32_t(atoi(argv[3]));
  FILE* f = fopen(argv[4], "rb");
  if (!f) return 1;
  std::vector<unsigned char> in;
  unsigned char buf[65536];
  size_t n;
  while ((n = fread(buf, 1, sizeof buf, f)) > 0) in.insert(in.end(), buf, buf + n);
  fclose(f);
  uint32_t tw = std::max(w, bpp == 2 ? 16u : 8u), th = std::max(h, 8u);
  if (in.size() < size_t(tw) * th * bpp / 8) {
    fprintf(stderr, "pvrtc_ref: input too short\n");
    return 1;
  }
  std::vector<uint8_t> out(size_t(w) * h * 4);
  pvr::PVRTDecompressPVRTC(in.data(), bpp == 2 ? 1 : 0, w, h, out.data());
  f = fopen(argv[5], "wb");
  if (!f) return 1;
  fwrite(out.data(), 1, out.size(), f);
  fclose(f);
  return 0;
}
