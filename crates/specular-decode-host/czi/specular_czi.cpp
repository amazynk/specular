// specular-czi: Specular's bridge to libCZI (Zeiss CZI reader), loaded by `specular-decode-host`.
//
// libCZI is a C++ library (LGPL-3.0), so it is only ever linked dynamically: this small library
// is Specular's own code, links libCZI's shared library, and is itself loaded at run time by the
// sandboxed decode host (`src/czi.rs`), before the sandbox is applied. Without libCZI on the
// computer it fails to load and CZI files are refused with that reason. Nothing here writes CZI.
//
// The C interface (every function catches every exception):
//
//   uint32_t specular_czi_abi(void);
//   int specular_czi_init(char* version, size_t version_cap);
//   int specular_czi_open(SpecularCziRead read, void* ctx, uint64_t size, void** doc,
//                         char* error, size_t error_cap);
//   void specular_czi_close(void* doc);
//   int specular_czi_info(void* doc, uint64_t max_xml, SpecularCziWrite write, void* ctx,
//                         char* error, size_t error_cap);
//   int specular_czi_read(void* doc, const SpecularCziRegion* region, SpecularCziPlane* out,
//                         SpecularCziWrite write, void* ctx, char* error, size_t error_cap);
//   int specular_czi_attachment(void* doc, int32_t index, uint32_t max_side, uint64_t max_bytes,
//                               SpecularCziPlane* out, SpecularCziWrite write, void* ctx,
//                               char* error, size_t error_cap);
//
// A document reads the file only through `read` (the host's descriptor or handle): libCZI
// never opens a path. `specular_czi_info` writes the document's facts in the format
// `specular_sandbox::czi` reads:
//
//   magic "SPCZII01", then records: u8 tag · u32 length · body, ending with tag 0.
//   1 HEADER   u32 major · u32 minor · u32 sub-blocks · i32 min M · i32 max M
//   2 DIM      u8 letter · i32 start · i32 size                    (each valid dimension)
//   3 BOX      i32 x y w h (all layers) · i32 x y w h (layer 0)
//   4 SCENE    i32 index · i32 x y w h (all) · i32 x y w h (layer 0)
//   5 PYRAMID  i32 scene (-1: none) · u32 n · n × (u8 minification · u8 layer · u32 count)
//   6 CHANNEL  i32 index · u8 pixel type (255 unknown) · u8 flags (1 display settings,
//              2 enabled, 4 tinted) · u8 r g b · f32 black · f32 white · u8 gradation
//              · f32 gamma · i32 bits (-1) · str name · str id
//   7 SCALING  f64 x y z (metres per pixel; NaN when unknown)
//   8 ATTACH   i32 index · str content type · str name
//   9 CODING   i32 raw compression · u32 sub-blocks
//  10 PIXEL    u8 pixel type · u32 sub-blocks
//  11 DOC      str name · str title · str description · str created · str user · str comment
//  12 XML      str metadata XML (left out when longer than `max_xml`)
// All little endian; strings are a u32 length and UTF-8 bytes.
//
// `specular_czi_read` composes one channel of a region of one plane at a zoom with libCZI's
// scaling tile accessor (pyramid layers included) and writes its rows, tightly packed, in the
// channel's pixel type. `specular_czi_attachment` writes an attachment's bytes (kind 0: JPG,
// PNG, ...) or, for an embedded CZI (label, slide preview), its first channel composed to fit
// `max_side` (kind 1), with that channel's display settings.

#include <libCZI/libCZI.h>

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <limits>
#include <map>
#include <memory>
#include <new>
#include <stdexcept>
#include <string>
#include <vector>

#if defined(_WIN32)
#define SPECULAR_EXPORT extern "C" __declspec(dllexport)
#else
#define SPECULAR_EXPORT extern "C" __attribute__((visibility("default")))
#endif

extern "C" {
// Reads `size` bytes at `offset` into `buf`; `*got` receives the count (short only at the end
// of the file). Returns 0, or anything else on an I/O error.
typedef int (*SpecularCziRead)(void* ctx, uint64_t offset, unsigned char* buf, uint64_t size,
                               uint64_t* got);
// Returns 0 to go on, anything else to stop.
typedef int (*SpecularCziWrite)(void* ctx, const unsigned char* data, size_t len);

// Must match `Region` in src/czi.rs.
struct SpecularCziRegion
{
  // In the raw sub-block coordinate system.
  int32_t  x, y, w, h;
  float    zoom;
  // Scene filter, -1: every scene.
  int32_t  scene;
  // Plane coordinate per dimension index (1 Z, 2 C, 3 T, 4 R, 5 S, 6 I, 7 H, 8 V, 9 B); bit i
  // of `coord_set` says whether coord[i] is given.
  int32_t  coord[10];
  uint32_t coord_set;
  // Wanted pixel type (libCZI's numbering).
  uint32_t pixel;
  // Largest output accepted.
  uint32_t max_w, max_h;
  uint64_t max_bytes;
  // Largest single sub-block (decoded bytes) read.
  uint64_t max_subblock_bytes;
  // Bytes of decoded sub-blocks kept between reads.
  uint64_t cache_bytes;
};

// Must match `Plane` in src/czi.rs.
struct SpecularCziPlane
{
  uint32_t width, height;
  uint32_t pixel;
  // 0: encoded bytes (attachment), 1: pixels.
  uint32_t kind;
  char     content_type[12];
  // Display settings of the channel (attachments of embedded CZI).
  uint32_t has_display;
  float    black, white, gamma;
};
}

namespace
{

// Bumped with any change to the C interface or the record format.
const uint32_t ABI = 1;

enum Status
{
  OK          = 0,
  MALFORMED   = 1,
  UNSUPPORTED = 2,
  TOO_LARGE   = 3,
  INTERNAL    = 4,
  STOPPED     = 5,
  IO          = 6,
};

struct Failure
{
  int         status;
  std::string message;
};

[[noreturn]] void fail(int status, const std::string& message)
{
  throw Failure{status, message};
}

void copy_error(char* dst, size_t cap, const std::string& s)
{
  if (dst == nullptr || cap == 0)
    return;
  size_t n = std::min(s.size(), cap - 1);
  std::memcpy(dst, s.data(), n);
  dst[n] = '\0';
}

// UTF-8 of a wide string (UTF-16 on Windows, UTF-32 elsewhere); bad code units become U+FFFD.
std::string utf8(const std::wstring& w)
{
  std::string out;
  out.reserve(w.size());
  auto put = [&out](uint32_t c) {
    if (c > 0x10FFFF || (c >= 0xD800 && c <= 0xDFFF))
      c = 0xFFFD;
    if (c < 0x80)
      out.push_back(char(c));
    else if (c < 0x800)
    {
      out.push_back(char(0xC0 | (c >> 6)));
      out.push_back(char(0x80 | (c & 0x3F)));
    }
    else if (c < 0x10000)
    {
      out.push_back(char(0xE0 | (c >> 12)));
      out.push_back(char(0x80 | ((c >> 6) & 0x3F)));
      out.push_back(char(0x80 | (c & 0x3F)));
    }
    else
    {
      out.push_back(char(0xF0 | (c >> 18)));
      out.push_back(char(0x80 | ((c >> 12) & 0x3F)));
      out.push_back(char(0x80 | ((c >> 6) & 0x3F)));
      out.push_back(char(0x80 | (c & 0x3F)));
    }
  };
  for (size_t i = 0; i < w.size(); ++i)
  {
    uint32_t c = uint32_t(w[i]);
    if (sizeof(wchar_t) == 2 && c >= 0xD800 && c <= 0xDBFF && i + 1 < w.size())
    {
      uint32_t d = uint32_t(w[i + 1]);
      if (d >= 0xDC00 && d <= 0xDFFF)
      {
        c = 0x10000 + ((c - 0xD800) << 10) + (d - 0xDC00);
        ++i;
      }
    }
    put(c);
  }
  return out;
}

// Little-endian body builder.
struct Body
{
  std::vector<unsigned char> b;

  template <typename T>
  void put(T v)
  {
    unsigned char tmp[sizeof(T)];
    std::memcpy(tmp, &v, sizeof(T));
    b.insert(b.end(), tmp, tmp + sizeof(T));
  }

  void u8(uint8_t v) { b.push_back(v); }

  void str(const std::string& s)
  {
    put<uint32_t>(uint32_t(s.size()));
    b.insert(b.end(), s.begin(), s.end());
  }

  void rect(const libCZI::IntRect& r)
  {
    put<int32_t>(r.x);
    put<int32_t>(r.y);
    put<int32_t>(r.w);
    put<int32_t>(r.h);
  }
};

static_assert(sizeof(float) == 4 && sizeof(double) == 8, "IEEE floats");

// Writes to the caller in chunks of up to 1 MiB.
class Out
{
public:
  Out(SpecularCziWrite write, void* ctx) : myWrite(write), myCtx(ctx) {}

  void raw(const void* p, size_t n)
  {
    const unsigned char* b = static_cast<const unsigned char*>(p);
    myBuf.insert(myBuf.end(), b, b + n);
    if (myBuf.size() >= (1u << 20))
      flush();
  }

  void record(uint8_t tag, const Body& body)
  {
    raw(&tag, 1);
    uint32_t n = uint32_t(body.b.size());
    raw(&n, 4);
    if (!body.b.empty())
      raw(body.b.data(), body.b.size());
  }

  void flush()
  {
    if (myBuf.empty())
      return;
    if (myWrite(myCtx, myBuf.data(), myBuf.size()) != 0)
      fail(STOPPED, "stopped");
    myBuf.clear();
  }

private:
  SpecularCziWrite           myWrite;
  void*                      myCtx;
  std::vector<unsigned char> myBuf;
};

// The host's file, read through its callback.
class CallbackStream : public libCZI::IStream
{
public:
  CallbackStream(SpecularCziRead read, void* ctx, uint64_t size)
      : myRead(read), myCtx(ctx), mySize(size)
  {
  }

  void Read(std::uint64_t offset, void* pv, std::uint64_t size, std::uint64_t* ptrBytesRead) override
  {
    uint64_t got = 0;
    if (size > 0 && offset < mySize)
    {
      uint64_t n = std::min<uint64_t>(size, mySize - offset);
      if (myRead(myCtx, offset, static_cast<unsigned char*>(pv), n, &got) != 0 || got > n)
        throw std::runtime_error("the file could not be read");
    }
    if (ptrBytesRead != nullptr)
      *ptrBytesRead = got;
  }

private:
  SpecularCziRead myRead;
  void*           myCtx;
  uint64_t        mySize;
};

struct Doc
{
  std::shared_ptr<libCZI::ICZIReader>                     reader;
  std::shared_ptr<libCZI::ISingleChannelScalingTileAccessor> accessor;
  std::shared_ptr<libCZI::ISubBlockCache>                 cache;
  libCZI::SubBlockStatistics                              stats;
};

int bytes_per_pixel(libCZI::PixelType p)
{
  switch (p)
  {
  case libCZI::PixelType::Gray8:
    return 1;
  case libCZI::PixelType::Gray16:
    return 2;
  case libCZI::PixelType::Gray32Float:
    return 4;
  case libCZI::PixelType::Bgr24:
    return 3;
  case libCZI::PixelType::Bgr48:
    return 6;
  case libCZI::PixelType::Bgr96Float:
    return 12;
  default:
    return 0;
  }
}

const char* pixel_name(libCZI::PixelType p)
{
  switch (p)
  {
  case libCZI::PixelType::Gray8:
    return "Gray8";
  case libCZI::PixelType::Gray16:
    return "Gray16";
  case libCZI::PixelType::Gray32Float:
    return "Gray32Float";
  case libCZI::PixelType::Bgr24:
    return "Bgr24";
  case libCZI::PixelType::Bgr48:
    return "Bgr48";
  case libCZI::PixelType::Bgr96Float:
    return "Bgr96Float";
  case libCZI::PixelType::Bgra32:
    return "Bgra32";
  case libCZI::PixelType::Gray64ComplexFloat:
    return "Gray64ComplexFloat";
  case libCZI::PixelType::Bgr192ComplexFloat:
    return "Bgr192ComplexFloat";
  case libCZI::PixelType::Gray32:
    return "Gray32";
  case libCZI::PixelType::Gray64Float:
    return "Gray64Float";
  default:
    return "unknown";
  }
}

// Rows of a bitmap, tightly packed.
void write_rows(const std::shared_ptr<libCZI::IBitmapData>& bm, Out& out)
{
  const int bpp = bytes_per_pixel(bm->GetPixelType());
  const uint32_t w = bm->GetWidth(), h = bm->GetHeight();
  const size_t row = size_t(w) * size_t(bpp);
  libCZI::ScopedBitmapLockerSP lck{bm};
  if (h > 0 && (lck.ptrDataRoi == nullptr || lck.stride < row))
    fail(INTERNAL, "libCZI returned no pixels");
  const unsigned char* p = static_cast<const unsigned char*>(lck.ptrDataRoi);
  for (uint32_t y = 0; y < h; ++y)
    out.raw(p + size_t(y) * lck.stride, row);
  out.flush();
}

// The display settings of channel `index` of a document, when it has them.
bool display_of(libCZI::ICZIReader& reader, int index, float& black, float& white, float& gamma,
                bool* enabled, uint8_t* tint)
{
  auto seg = reader.ReadMetadataSegment();
  if (!seg)
    return false;
  auto meta = seg->CreateMetaFromMetadataSegment();
  if (!meta || !meta->IsXmlValid())
    return false;
  auto info = meta->GetDocumentInfo();
  if (!info)
    return false;
  auto ds = info->GetDisplaySettings();
  if (!ds)
    return false;
  auto ch = ds->GetChannelDisplaySettings(index);
  if (!ch)
    return false;
  ch->GetBlackWhitePoint(&black, &white);
  gamma = std::numeric_limits<float>::quiet_NaN();
  if (ch->GetGradationCurveMode() == libCZI::IDisplaySettings::GradationCurveMode::Gamma)
  {
    float g;
    if (ch->TryGetGamma(&g))
      gamma = g;
  }
  if (enabled != nullptr)
    *enabled = ch->GetIsEnabled();
  if (tint != nullptr)
  {
    libCZI::Rgb8Color c;
    if (ch->TryGetTintingColorRgb8(&c))
    {
      tint[0] = 1;
      tint[1] = c.r;
      tint[2] = c.g;
      tint[3] = c.b;
    }
  }
  return true;
}

void write_info(Doc& d, uint64_t max_xml, Out& out)
{
  libCZI::ICZIReader& r = *d.reader;
  const libCZI::SubBlockStatistics& s = d.stats;
  out.raw("SPCZII01", 8);
  {
    Body b;
    auto h = r.GetFileHeaderInfo();
    b.put<uint32_t>(uint32_t(h.majorVersion));
    b.put<uint32_t>(uint32_t(h.minorVersion));
    b.put<uint32_t>(uint32_t(std::max(s.subBlockCount, 0)));
    b.put<int32_t>(s.minMindex);
    b.put<int32_t>(s.maxMindex);
    out.record(1, b);
  }
  s.dimBounds.EnumValidDimensions([&](libCZI::DimensionIndex dim, int start, int size) {
    Body b;
    b.u8(uint8_t(libCZI::Utils::DimensionToChar(dim)));
    b.put<int32_t>(start);
    b.put<int32_t>(size);
    out.record(2, b);
    return true;
  });
  {
    Body b;
    b.rect(s.boundingBox);
    b.rect(s.boundingBoxLayer0Only);
    out.record(3, b);
  }
  for (const auto& kv : s.sceneBoundingBoxes)
  {
    Body b;
    b.put<int32_t>(kv.first);
    b.rect(kv.second.boundingBox);
    b.rect(kv.second.boundingBoxLayer0);
    out.record(4, b);
  }
  {
    auto p = r.GetPyramidStatistics();
    for (const auto& kv : p.scenePyramidStatistics)
    {
      Body b;
      b.put<int32_t>(kv.first == (std::numeric_limits<int>::max)() ? -1 : kv.first);
      b.put<uint32_t>(uint32_t(kv.second.size()));
      for (const auto& l : kv.second)
      {
        b.u8(l.layerInfo.minificationFactor);
        b.u8(l.layerInfo.pyramidLayerNo);
        b.put<uint32_t>(uint32_t(std::max(l.count, 0)));
      }
      out.record(5, b);
    }
  }
  // Pixel types and codings of every sub-block.
  std::map<int32_t, uint32_t> codings;
  std::map<int, uint32_t>     pixels;
  r.EnumerateSubBlocks([&](int, const libCZI::SubBlockInfo& info) {
    ++codings[info.compressionModeRaw];
    ++pixels[int(info.pixelType)];
    return true;
  });
  for (const auto& kv : codings)
  {
    Body b;
    b.put<int32_t>(kv.first);
    b.put<uint32_t>(kv.second);
    out.record(9, b);
  }
  for (const auto& kv : pixels)
  {
    Body b;
    b.u8(uint8_t(kv.first));
    b.put<uint32_t>(kv.second);
    out.record(10, b);
  }
  // Metadata: channels, scaling, document facts, the XML itself.
  std::shared_ptr<libCZI::ICziMultiDimensionDocumentInfo> info;
  std::string                                             xml;
  try
  {
    auto seg = r.ReadMetadataSegment();
    auto meta = seg ? seg->CreateMetaFromMetadataSegment() : nullptr;
    if (meta && meta->IsXmlValid())
    {
      info = meta->GetDocumentInfo();
      xml = meta->GetXml();
    }
  }
  catch (const std::bad_alloc&)
  {
    throw;
  }
  catch (const std::exception&)
  {
    // A file without (readable) metadata still has pixels.
  }
  int c_start = 0, c_size = 0;
  bool has_c = s.dimBounds.TryGetInterval(libCZI::DimensionIndex::C, &c_start, &c_size);
  if (!has_c)
  {
    c_start = 0;
    c_size = 1;
  }
  std::shared_ptr<libCZI::IDisplaySettings>       ds;
  std::shared_ptr<libCZI::IDimensionsChannelsInfo> names;
  if (info)
  {
    try
    {
      ds = info->GetDisplaySettings();
    }
    catch (const std::bad_alloc&)
    {
      throw;
    }
    catch (const std::exception&)
    {
    }
    try
    {
      names = info->GetDimensionChannelsInfo();
    }
    catch (const std::bad_alloc&)
    {
      throw;
    }
    catch (const std::exception&)
    {
    }
  }
  for (int i = 0; i < c_size && i < 4096; ++i)
  {
    const int c = c_start + i;
    Body b;
    b.put<int32_t>(c);
    libCZI::SubBlockInfo sb;
    uint8_t pixel = 0xFF;
    if (r.TryGetSubBlockInfoOfArbitrarySubBlockInChannel(has_c ? c : 0, sb))
      pixel = uint8_t(sb.pixelType);
    b.u8(pixel);
    uint8_t flags = 0;
    uint8_t rgb[3] = {0, 0, 0};
    float black = 0, white = 1, gamma = std::numeric_limits<float>::quiet_NaN();
    uint8_t gradation = 0;
    std::shared_ptr<libCZI::IChannelDisplaySetting> ch;
    if (ds)
    {
      try
      {
        ch = ds->GetChannelDisplaySettings(i);
      }
      catch (const std::bad_alloc&)
      {
        throw;
      }
      catch (const std::exception&)
      {
      }
    }
    if (ch)
    {
      flags |= 1;
      if (ch->GetIsEnabled())
        flags |= 2;
      libCZI::Rgb8Color col;
      if (ch->TryGetTintingColorRgb8(&col))
      {
        flags |= 4;
        rgb[0] = col.r;
        rgb[1] = col.g;
        rgb[2] = col.b;
      }
      ch->GetBlackWhitePoint(&black, &white);
      gradation = uint8_t(ch->GetGradationCurveMode());
      float g;
      if (ch->GetGradationCurveMode() == libCZI::IDisplaySettings::GradationCurveMode::Gamma
          && ch->TryGetGamma(&g))
        gamma = g;
    }
    b.u8(flags);
    b.u8(rgb[0]);
    b.u8(rgb[1]);
    b.u8(rgb[2]);
    b.put<float>(black);
    b.put<float>(white);
    b.u8(gradation);
    b.put<float>(gamma);
    int bits = -1;
    std::wstring name, id;
    if (names && i < names->GetChannelCount())
    {
      auto ci = names->GetChannel(i);
      if (ci)
      {
        int bc;
        if (ci->TryGetComponentBitCount(&bc))
          bits = bc;
        ci->TryGetAttributeName(&name);
        ci->TryGetAttributeId(&id);
      }
    }
    b.put<int32_t>(bits);
    b.str(utf8(name));
    b.str(utf8(id));
    out.record(6, b);
  }
  if (info)
  {
    auto sc = info->GetScalingInfo();
    Body b;
    b.put<double>(sc.scaleX);
    b.put<double>(sc.scaleY);
    b.put<double>(sc.scaleZ);
    out.record(7, b);
    auto g = info->GetGeneralDocumentInfo();
    Body d;
    d.str(g.name_valid ? utf8(g.name) : std::string());
    d.str(g.title_valid ? utf8(g.title) : std::string());
    d.str(g.description_valid ? utf8(g.description) : std::string());
    d.str(g.creationDateTime_valid ? utf8(g.creationDateTime) : std::string());
    d.str(g.userName_valid ? utf8(g.userName) : std::string());
    d.str(g.comment_valid ? utf8(g.comment) : std::string());
    out.record(11, d);
  }
  r.EnumerateAttachments([&](int index, const libCZI::AttachmentInfo& a) {
    Body b;
    b.put<int32_t>(index);
    char type[9];
    std::memcpy(type, a.contentFileType, 8);
    type[8] = '\0';
    b.str(std::string(type));
    b.str(a.name);
    out.record(8, b);
    return true;
  });
  if (!xml.empty() && xml.size() <= max_xml)
  {
    Body b;
    b.str(xml);
    out.record(12, b);
  }
  uint8_t end = 0;
  out.raw(&end, 1);
  out.flush();
}

libCZI::DimensionIndex dim_of(int i)
{
  return static_cast<libCZI::DimensionIndex>(i);
}

int status_of(const std::exception& e)
{
  if (dynamic_cast<const libCZI::LibCZIIOException*>(&e) != nullptr)
    return IO;
  return MALFORMED;
}

template <typename F>
int guarded(char* err, size_t cap, F&& f)
{
  try
  {
    f();
    return OK;
  }
  catch (const Failure& x)
  {
    copy_error(err, cap, x.message);
    return x.status;
  }
  catch (const std::bad_alloc&)
  {
    copy_error(err, cap, "out of memory");
    return TOO_LARGE;
  }
  catch (const std::exception& e)
  {
    copy_error(err, cap, std::string("libCZI: ") + e.what());
    return status_of(e);
  }
  catch (...)
  {
    copy_error(err, cap, "libCZI failed");
    return INTERNAL;
  }
}

} // namespace

SPECULAR_EXPORT uint32_t specular_czi_abi(void)
{
  return ABI;
}

// Picks libCZI's own JPEG XR decoder (never a system codec) and says which libCZI this is.
// Called once, before the sandbox is applied.
SPECULAR_EXPORT int specular_czi_init(char* version, size_t version_cap)
{
  return guarded(version, version_cap, [&]() {
    libCZI::SetSiteObject(libCZI::GetDefaultSiteObject(libCZI::SiteObjectType::WithJxrDecoder));
    int major = 0, minor = 0, patch = 0;
    libCZI::GetLibCZIVersion(&major, &minor, &patch);
    copy_error(version, version_cap,
               std::to_string(major) + "." + std::to_string(minor) + "." + std::to_string(patch));
  });
}

SPECULAR_EXPORT int specular_czi_open(SpecularCziRead read, void* ctx, uint64_t size, void** doc,
                                      char* err, size_t err_cap)
{
  if (doc == nullptr)
    return INTERNAL;
  *doc = nullptr;
  return guarded(err, err_cap, [&]() {
    if (read == nullptr)
      fail(INTERNAL, "bad arguments");
    auto stream = std::make_shared<CallbackStream>(read, ctx, size);
    std::unique_ptr<Doc> d(new Doc());
    d->reader = libCZI::CreateCZIReader();
    libCZI::ICZIReader::OpenOptions opts;
    opts.SetDefault();
    d->reader->Open(stream, &opts);
    d->stats = d->reader->GetStatistics();
    if (d->stats.subBlockCount <= 0 || !d->stats.boundingBox.IsValid())
      fail(UNSUPPORTED, "the file has no image");
    d->accessor = d->reader->CreateSingleChannelScalingTileAccessor();
    d->cache = libCZI::CreateSubBlockCache();
    *doc = d.release();
  });
}

SPECULAR_EXPORT void specular_czi_close(void* doc)
{
  try
  {
    delete static_cast<Doc*>(doc);
  }
  catch (...)
  {
  }
}

SPECULAR_EXPORT int specular_czi_info(void* doc, uint64_t max_xml, SpecularCziWrite write,
                                      void* ctx, char* err, size_t err_cap)
{
  return guarded(err, err_cap, [&]() {
    if (doc == nullptr || write == nullptr)
      fail(INTERNAL, "bad arguments");
    Out out(write, ctx);
    write_info(*static_cast<Doc*>(doc), max_xml, out);
  });
}

SPECULAR_EXPORT int specular_czi_read(void* doc, const SpecularCziRegion* region,
                                      SpecularCziPlane* plane, SpecularCziWrite write, void* ctx,
                                      char* err, size_t err_cap)
{
  return guarded(err, err_cap, [&]() {
    if (doc == nullptr || region == nullptr || plane == nullptr || write == nullptr)
      fail(INTERNAL, "bad arguments");
    Doc&                     d = *static_cast<Doc*>(doc);
    const SpecularCziRegion& q = *region;
    const auto pixel = static_cast<libCZI::PixelType>(uint8_t(q.pixel));
    const int  bpp = bytes_per_pixel(pixel);
    if (bpp == 0)
      fail(UNSUPPORTED, std::string("pixel type ") + pixel_name(pixel) + " is not supported");
    if (q.w <= 0 || q.h <= 0 || !(q.zoom > 0.0f) || q.zoom > 1.0f || !std::isfinite(q.zoom))
      fail(INTERNAL, "bad region");
    libCZI::IntRect roi{q.x, q.y, q.w, q.h};
    const libCZI::IntSize size = d.accessor->CalcSize(roi, q.zoom);
    if (size.w == 0 || size.h == 0)
      fail(INTERNAL, "empty region");
    if (size.w > q.max_w || size.h > q.max_h
        || uint64_t(size.w) * uint64_t(size.h) * uint64_t(bpp) > q.max_bytes)
      fail(TOO_LARGE, "the region is larger than the limit");
    libCZI::CDimCoordinate coord;
    for (int i = 1; i <= 9; ++i)
      if ((q.coord_set >> i) & 1u)
        coord.Set(dim_of(i), q.coord[i]);
    // Refuse a sub-block that would not fit in memory before libCZI decodes it.
    bool too_big = false;
    d.reader->EnumSubset(&coord, &roi, false, [&](int, const libCZI::SubBlockInfo& info) {
      const int b = std::max(bytes_per_pixel(info.pixelType), 1);
      if (uint64_t(info.physicalSize.w) * uint64_t(info.physicalSize.h) * uint64_t(b)
          > q.max_subblock_bytes)
      {
        too_big = true;
        return false;
      }
      return true;
    });
    if (too_big)
      fail(TOO_LARGE, "a tile of the file is larger than the limit");
    libCZI::ISingleChannelScalingTileAccessor::Options o;
    o.Clear();
    o.backGroundColor.r = o.backGroundColor.g = o.backGroundColor.b = 0;
    o.sortByM = true;
    o.subBlockCache = d.cache;
    o.onlyUseSubBlockCacheForCompressedData = true;
    if (q.scene >= 0)
      o.sceneFilter = libCZI::Utils::IndexSetFromString(std::to_wstring(q.scene));
    auto bm = d.accessor->Get(pixel, roi, &coord, q.zoom, &o);
    if (!bm || bm->GetPixelType() != pixel)
      fail(INTERNAL, "libCZI returned another pixel type");
    plane->width = bm->GetWidth();
    plane->height = bm->GetHeight();
    plane->pixel = uint32_t(uint8_t(pixel));
    plane->kind = 1;
    plane->has_display = 0;
    Out out(write, ctx);
    write_rows(bm, out);
    bm.reset();
    libCZI::ISubBlockCacheControl::PruneOptions prune;
    prune.maxMemoryUsage = q.cache_bytes;
    d.cache->Prune(prune);
  });
}

SPECULAR_EXPORT int specular_czi_attachment(void* doc, int32_t index, uint32_t max_side,
                                            uint64_t max_bytes, SpecularCziPlane* plane,
                                            SpecularCziWrite write, void* ctx, char* err,
                                            size_t err_cap)
{
  return guarded(err, err_cap, [&]() {
    if (doc == nullptr || plane == nullptr || write == nullptr || max_side == 0)
      fail(INTERNAL, "bad arguments");
    Doc& d = *static_cast<Doc*>(doc);
    libCZI::AttachmentInfo info;
    if (index < 0 || !d.reader->TryGetAttachmentInfo(index, &info))
      fail(MALFORMED, "no such attachment");
    char type[9];
    std::memcpy(type, info.contentFileType, 8);
    type[8] = '\0';
    std::memset(plane->content_type, 0, sizeof(plane->content_type));
    std::memcpy(plane->content_type, type, std::strlen(type));
    plane->has_display = 0;
    auto att = d.reader->ReadAttachment(index);
    if (!att)
      fail(MALFORMED, "the attachment could not be read");
    Out out(write, ctx);
    if (std::strcmp(type, "CZI") != 0)
    {
      const void* p = nullptr;
      size_t      n = 0;
      att->DangerousGetRawData(p, n);
      if (n > max_bytes)
        fail(TOO_LARGE, "the attachment is larger than the limit");
      plane->kind = 0;
      plane->width = plane->height = 0;
      plane->pixel = 0xFF;
      if (n > 0)
        out.raw(p, n);
      out.flush();
      return;
    }
    // An embedded CZI document: its first channel, composed to fit.
    auto inner = libCZI::CreateCZIReader();
    inner->Open(libCZI::CreateStreamFromMemory(att.get()));
    auto st = inner->GetStatistics();
    if (st.subBlockCount <= 0 || !st.boundingBox.IsValid() || st.boundingBox.w <= 0
        || st.boundingBox.h <= 0)
      fail(MALFORMED, "the embedded image is empty");
    libCZI::CDimCoordinate coord;
    int start = 0, sz = 0;
    if (st.dimBounds.TryGetInterval(libCZI::DimensionIndex::C, &start, &sz))
      coord.Set(libCZI::DimensionIndex::C, start);
    libCZI::SubBlockInfo sb;
    if (!inner->TryGetSubBlockInfoOfArbitrarySubBlockInChannel(start, sb))
      fail(MALFORMED, "the embedded image has no pixels");
    const int bpp = bytes_per_pixel(sb.pixelType);
    if (bpp == 0)
      fail(UNSUPPORTED, std::string("pixel type ") + pixel_name(sb.pixelType) + " is not supported");
    const int32_t longest = std::max(st.boundingBox.w, st.boundingBox.h);
    const float   zoom = longest > int32_t(max_side) ? float(max_side) / float(longest) : 1.0f;
    auto          acc = inner->CreateSingleChannelScalingTileAccessor();
    const auto    size = acc->CalcSize(st.boundingBox, zoom);
    if (uint64_t(size.w) * uint64_t(size.h) * uint64_t(bpp) > max_bytes)
      fail(TOO_LARGE, "the embedded image is larger than the limit");
    libCZI::ISingleChannelScalingTileAccessor::Options o;
    o.Clear();
    o.backGroundColor.r = o.backGroundColor.g = o.backGroundColor.b = 0;
    auto bm = acc->Get(sb.pixelType, st.boundingBox, &coord, zoom, &o);
    if (!bm)
      fail(INTERNAL, "libCZI returned no pixels");
    plane->kind = 1;
    plane->width = bm->GetWidth();
    plane->height = bm->GetHeight();
    plane->pixel = uint32_t(uint8_t(bm->GetPixelType()));
    float black = 0, white = 1, gamma = std::numeric_limits<float>::quiet_NaN();
    try
    {
      if (display_of(*inner, 0, black, white, gamma, nullptr, nullptr))
      {
        plane->has_display = 1;
        plane->black = black;
        plane->white = white;
        plane->gamma = gamma;
      }
    }
    catch (const std::bad_alloc&)
    {
      throw;
    }
    catch (const std::exception&)
    {
    }
    write_rows(bm, out);
  });
}
