// Reference dumper for the specular-texture Basis Universal tests (built by
// tools/corpus/texture/basis.sh against the upstream transcoder library).
//
// Usage: basisu_ref IN.ktx2 OUT_PREFIX
// Writes OUT_PREFIX.l<level>.a<layer>.f<face>.rgba (raw RGBA8, level width x height) for every
// level, layer and face, transcoded to cTFRGBA32. Exits non-zero for files the transcoder
// rejects or that are not ETC1S / UASTC LDR.
#include <cstdio>
#include <cstdlib>
#include <string>
#include <vector>

#include "basisu_transcoder.h"

static bool read_file(const char *path, std::vector<unsigned char> &out) {
  FILE *f = std::fopen(path, "rb");
  if (!f) return false;
  std::fseek(f, 0, SEEK_END);
  long n = std::ftell(f);
  std::fseek(f, 0, SEEK_SET);
  out.resize(n > 0 ? size_t(n) : 0);
  bool ok = n > 0 && std::fread(out.data(), 1, out.size(), f) == out.size();
  std::fclose(f);
  return ok;
}

int main(int argc, char **argv) {
  if (argc != 3) {
    std::fprintf(stderr, "usage: basisu_ref IN.ktx2 OUT_PREFIX\n");
    return 2;
  }
  std::vector<unsigned char> data;
  if (!read_file(argv[1], data)) return 1;
  basist::basisu_transcoder_init();
  basist::ktx2_transcoder tr;
  if (!tr.init(data.data(), uint32_t(data.size()))) return 1;
  if (!tr.is_etc1s() && !tr.is_uastc()) return 1;
  if (!tr.start_transcoding()) return 1;
  const uint32_t levels = tr.get_levels() ? tr.get_levels() : 1;
  const uint32_t layers = tr.get_layers() ? tr.get_layers() : 1;
  const uint32_t faces = tr.get_faces() ? tr.get_faces() : 1;
  int written = 0;
  for (uint32_t l = 0; l < levels; l++)
    for (uint32_t a = 0; a < layers; a++)
      for (uint32_t f = 0; f < faces; f++) {
        basist::ktx2_image_level_info info;
        if (!tr.get_image_level_info(info, l, a, f)) return 1;
        const uint32_t w = info.m_orig_width, h = info.m_orig_height;
        std::vector<unsigned char> px(size_t(w) * h * 4);
        if (!tr.transcode_image_level(l, a, f, px.data(), w * h,
                                      basist::transcoder_texture_format::cTFRGBA32, 0, w, h))
          return 1;
        std::string name = std::string(argv[2]) + ".l" + std::to_string(l) + ".a" +
                           std::to_string(a) + ".f" + std::to_string(f) + ".rgba";
        FILE *o = std::fopen(name.c_str(), "wb");
        if (!o) return 1;
        std::fwrite(px.data(), 1, px.size(), o);
        std::fclose(o);
        written++;
      }
  std::printf("%s: %d images\n", argv[1], written);
  return 0;
}
