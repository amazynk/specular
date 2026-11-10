// A second reader for OpenEXR luminance-chroma (Y, RY, BY) files: OpenEXR's own RGBA
// interface (Imf::RgbaInputFile, which reconstructs the subsampled chroma and converts to
// the file's RGB as the library defines it), linked against the system's OpenEXR 3.1
// shared library. No OpenEXR headers are needed: the few declarations below match the
// library's exported symbols (Imf_3_1 namespace, OpenEXR 3.1.x ABI).
//
// Build (tools/corpus/fetch-fidelity.sh exr does this into $CARGO_TARGET_DIR/bin):
//   c++ -O2 -o exr-rgba-oracle exr-rgba-oracle.cc -l:libOpenEXR-3_1.so.30
// Use: exr-rgba-oracle FILE X0 Y0 X1 Y1 > out.f16  (data window from the header; writes
// (Y1-Y0+1)*(X1-X0+1) RGBA half floats, little-endian).
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <vector>

namespace Imf_3_1 {
struct Rgba {
    uint16_t r, g, b, a;
};
class RgbaInputFile {
  public:
    RgbaInputFile(const char name[], int numThreads);
    virtual ~RgbaInputFile();
    void setFrameBuffer(Rgba *base, size_t xStride, size_t yStride);
    void readPixels(int scanLine1, int scanLine2);

  private:
    // Room for the library's members (its own object is smaller).
    char pad_[1024];
};
} // namespace Imf_3_1

int main(int argc, char **argv) {
    if (argc != 6) {
        std::fprintf(stderr, "usage: exr-rgba-oracle FILE X0 Y0 X1 Y1\n");
        return 2;
    }
    long x0 = std::atol(argv[2]), y0 = std::atol(argv[3]);
    long x1 = std::atol(argv[4]), y1 = std::atol(argv[5]);
    long w = x1 - x0 + 1, h = y1 - y0 + 1;
    if (w <= 0 || h <= 0 || w * h > (1L << 28)) {
        return 2;
    }
    std::vector<Imf_3_1::Rgba> px(static_cast<size_t>(w * h));
    auto *f = new Imf_3_1::RgbaInputFile(argv[1], 1);
    // The frame buffer is addressed by absolute pixel coordinates.
    f->setFrameBuffer(px.data() - x0 - y0 * w, 1, static_cast<size_t>(w));
    f->readPixels(static_cast<int>(y0), static_cast<int>(y1));
    delete f;
    std::fwrite(px.data(), sizeof(Imf_3_1::Rgba), px.size(), stdout);
    return 0;
}
