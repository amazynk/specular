// specular-occt: Specular's bridge to OpenCASCADE (OCCT), loaded by `specular-decode-host`.
//
// OCCT is a C++ library (LGPL-2.1 with the OCCT exception), so it is only ever linked
// dynamically: this small library is Specular's own code, links OCCT's shared libraries, and
// is itself loaded at run time by the sandboxed decode host (`src/occt.rs`), before the
// sandbox is applied. Without OCCT on the computer it fails to load and STEP and IGES files
// are refused with that reason.
//
// The C interface (all functions catch every exception):
//
//   uint32_t specular_occt_abi(void);
//   int specular_occt_init(int threads, char* version, size_t version_cap);
//   int specular_occt_read(const unsigned char* data, size_t len, int format,
//                          const SpecularOcctParams* params,
//                          SpecularOcctWrite write, void* ctx, char* error, size_t error_cap);
//
// `specular_occt_read` reads a STEP (format 1) or IGES (format 2) file from memory with
// XCAF (assembly tree, names, colours), tessellates every distinct part once (BRepMesh, a
// linear deflection relative to the model's size), and writes the model through `write` in
// chunks, in the format `specular_sandbox::cad` reads:
//
//   magic "SPCADM01", then records: u8 tag · u32 length · body, ending with tag 0.
//   1 INFO  f64 unit (metres per coordinate unit) · str file unit · u32 solids · u32 faces
//           · u32 faces without a mesh · u32 instances · f64 deflection · f32 box[6]
//   2 NODE  u32 parent (0xFFFFFFFF: none) · u8 kind (0 assembly, 1 part) · u8 has colour
//           · f32 rgba[4] (linear) · str name
//   3 MESH  u32 node · u8 has colour · f32 rgba[4] · u32 vertices · u32 triangles
//           · f32 positions[3v] · f32 normals[3v] · u32 indices[3t]
//   4 NOTE  str
//   5 PMI   one annotation of the model's PMI (product manufacturing information, read with
//           XDE's GD&T mode from STEP AP242, AP214 and AP203); see `write_pmi` for the body
//   6 VIEW  one saved view: str name · u8 projection (0 none, 1 central, 2 parallel)
//           · f32 eye[3] · f32 direction[3] · f32 up[3] · f64 zoom · f64 window w · f64 h
//           · u32 n · u32 PMI[n] (indices of the PMI records it shows)
// All little endian; strings are a u32 length and UTF-8 bytes. Positions are in the
// document's unit with every assembly transformation applied; triangles wind counter-
// clockwise seen from outside.

#include <BRepAdaptor_Curve.hxx>
#include <BRepBndLib.hxx>
#include <BRepLib_ToolTriangulatedShape.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepTools.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GCPnts_TangentialDeflection.hxx>
#include <IGESCAFControl_Reader.hxx>
#include <IGESControl_Controller.hxx>
#include <IGESData_IGESModel.hxx>
#include <IMeshTools_Parameters.hxx>
#include <Interface_Static.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <OSD_ThreadPool.hxx>
#include <Poly_Connect.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPCAFControl_Controller.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPControl_Reader.hxx>
#include <StepData_StepModel.hxx>
#include <StepGeom_Axis2Placement3d.hxx>
#include <StepGeom_Direction.hxx>
#include <StepVisual_CameraModelD3.hxx>
#include <StepVisual_ViewVolume.hxx>
#include <StepDimTol_GeometricTolerance.hxx>
#include <StepBasic_ConversionBasedUnit.hxx>
#include <StepBasic_MeasureWithUnit.hxx>
#include <StepBasic_SiUnit.hxx>
#include <StepBasic_Unit.hxx>
#include <StepRepr_ReprItemAndMeasureWithUnit.hxx>
#include <StepRepr_ReprItemAndMeasureWithUnitAndQRI.hxx>
#include <StepShape_ToleranceValue.hxx>
#include <Standard_Failure.hxx>
#include <Standard_OutOfMemory.hxx>
#include <Standard_Version.hxx>
#include <TCollection_AsciiString.hxx>
#include <TColStd_SequenceOfAsciiString.hxx>
#include <TDF_Label.hxx>
#include <TDF_Tool.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <TopoDS_Face.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDimTolObjects_DatumObject.hxx>
#include <XCAFDimTolObjects_DimensionObject.hxx>
#include <XCAFDimTolObjects_GeomToleranceObject.hxx>
#include <XCAFDoc_Datum.hxx>
#include <XCAFDoc_DimTolTool.hxx>
#include <XCAFDoc_Dimension.hxx>
#include <XCAFDoc_GeomTolerance.hxx>
#include <XCAFDoc_View.hxx>
#include <XCAFDoc_ViewTool.hxx>
#include <XCAFView_Object.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFDoc_VisMaterial.hxx>
#include <XCAFPrs.hxx>
#include <XCAFPrs_DocumentExplorer.hxx>
#include <XCAFPrs_IndexedDataMapOfShapeStyle.hxx>
#include <XCAFPrs_Style.hxx>

#include <algorithm>
#include <array>
#include <cctype>
#include <cstdlib>
#include <cmath>
#include <cstdint>
#include <cstdio>
#include <cstring>
#include <istream>
#include <map>
#include <new>
#include <set>
#include <streambuf>
#include <string>
#include <vector>

#if defined(_WIN32)
#define SPECULAR_EXPORT extern "C" __declspec(dllexport)
#else
#define SPECULAR_EXPORT extern "C" __attribute__((visibility("default")))
#endif

extern "C" {
// Must match `Params` in src/occt.rs.
struct SpecularOcctParams
{
  // Linear deflection as a fraction of the model's bounding-box diagonal.
  double relative_deflection;
  // Angular deflection, radians.
  double angular_deflection;
  // Most triangles written (all instances together); a finer mesh is retried coarser.
  uint64_t max_triangles;
  // Most bytes written in all.
  uint64_t max_bytes;
};
// Returns 0 to go on, anything else to stop.
typedef int (*SpecularOcctWrite)(void* ctx, const unsigned char* data, size_t len);
}

namespace
{

// Bumped with any change to the C interface or the record format.
const uint32_t ABI = 2;

enum Status
{
  OK          = 0,
  MALFORMED   = 1,
  UNSUPPORTED = 2,
  TOO_LARGE   = 3,
  INTERNAL    = 4,
  STOPPED     = 5,
};

const uint32_t NO_PARENT = 0xFFFFFFFFu;

struct Failure
{
  int         status;
  std::string message;
};

void fail(int status, const std::string& message)
{
  throw Failure{status, message};
}

// A read-only stream over the caller's bytes (no copy).
class MemBuf : public std::streambuf
{
public:
  MemBuf(const unsigned char* data, size_t len)
  {
    char* p = const_cast<char*>(reinterpret_cast<const char*>(data));
    setg(p, p, p + len);
  }

protected:
  pos_type seekoff(off_type off, std::ios_base::seekdir dir, std::ios_base::openmode) override
  {
    char* base = dir == std::ios_base::beg   ? eback()
                 : dir == std::ios_base::cur ? gptr()
                                             : egptr();
    char* at = base + off;
    if (at < eback() || at > egptr())
      return pos_type(off_type(-1));
    setg(eback(), at, egptr());
    return pos_type(at - eback());
  }

  pos_type seekpos(pos_type pos, std::ios_base::openmode which) override
  {
    return seekoff(off_type(pos), std::ios_base::beg, which);
  }
};

// Little-endian record writer, flushing to the caller in chunks.
class Out
{
public:
  Out(SpecularOcctWrite write, void* ctx, uint64_t max_bytes)
      : myWrite(write),
        myCtx(ctx),
        myMax(max_bytes)
  {
  }

  void raw(const void* p, size_t n)
  {
    myTotal += n;
    if (myTotal > myMax)
      fail(TOO_LARGE, "the model's mesh is larger than the limit");
    const unsigned char* b = static_cast<const unsigned char*>(p);
    myBuf.insert(myBuf.end(), b, b + n);
    if (myBuf.size() >= (1u << 20))
      flush();
  }

  void flush()
  {
    if (myBuf.empty())
      return;
    if (myWrite(myCtx, myBuf.data(), myBuf.size()) != 0)
      fail(STOPPED, "stopped");
    myBuf.clear();
  }

  // One record: tag, length, body.
  void record(uint8_t tag, const std::vector<unsigned char>& body)
  {
    raw(&tag, 1);
    uint32_t n = uint32_t(body.size());
    raw(&n, 4);
    if (!body.empty())
      raw(body.data(), body.size());
  }

private:
  SpecularOcctWrite          myWrite;
  void*                      myCtx;
  uint64_t                   myMax;
  uint64_t                   myTotal = 0;
  std::vector<unsigned char> myBuf;
};

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
};

static_assert(sizeof(float) == 4 && sizeof(double) == 8, "IEEE floats");

std::string utf8(const TCollection_ExtendedString& s)
{
  // Converts to UTF-8 (a zero replacement character means "convert, do not replace").
  TCollection_AsciiString a(s, '\0');
  return std::string(a.ToCString(), size_t(a.Length()));
}

std::string label_name(const TDF_Label& l)
{
  Handle(TDataStd_Name) n;
  if (l.IsNull() || !l.FindAttribute(TDataStd_Name::GetID(), n))
    return std::string();
  return utf8(n->Get());
}

// The surface colour of a style, linear RGBA: its colour, else its material's base colour.
bool style_colour(const XCAFPrs_Style& s, float rgba[4])
{
  Quantity_ColorRGBA c;
  if (s.IsSetColorSurf())
    c = s.GetColorSurfRGBA();
  else if (!s.Material().IsNull())
    c = s.Material()->BaseColor();
  else
    return false;
  Standard_Real r, g, b;
  c.GetRGB().Values(r, g, b, Quantity_TOC_RGB);
  rgba[0] = float(r);
  rgba[1] = float(g);
  rgba[2] = float(b);
  rgba[3] = c.Alpha();
  return true;
}

// The faces of one part and the style each has of its own (in the part's frame).
struct Part
{
  TopoDS_Shape                       shape;
  std::vector<TopoDS_Face>           faces;
  // Per face: index into `colours`, -1: the instance's colour.
  std::vector<int>                   colour;
  // The distinct colours the part's own styles give its faces, linear RGBA.
  std::vector<std::array<float, 4>>  colours;
  Bnd_Box                            box;
  uint64_t                           triangles = 0;
};

// Which face takes which style: styles of faces first, then of shells, solids and compounds
// for the faces that have none yet (smaller sub-shapes win).
void collect_part(const TDF_Label& label, Part& part)
{
  XCAFPrs_IndexedDataMapOfShapeStyle settings;
  XCAFPrs::CollectStyleSettings(label, TopLoc_Location(), settings);
  XCAFPrs_IndexedDataMapOfShapeStyle by_face;
  for (int t = TopAbs_FACE; t >= TopAbs_COMPOUND; --t)
  {
    for (XCAFPrs_IndexedDataMapOfShapeStyle::Iterator it(settings); it.More(); it.Next())
    {
      const TopoDS_Shape& key = it.Key();
      if (key.ShapeType() != TopAbs_ShapeEnum(t))
        continue;
      if (t == TopAbs_FACE)
      {
        if (!by_face.Contains(key))
          by_face.Add(key, it.Value());
        continue;
      }
      for (TopExp_Explorer e(key, TopAbs_FACE); e.More(); e.Next())
        if (!by_face.Contains(e.Current()))
          by_face.Add(e.Current(), it.Value());
    }
  }
  const TopLoc_Location own = part.shape.Location();
  const TopoDS_Shape    base = part.shape.Located(TopLoc_Location());
  for (TopExp_Explorer e(base, TopAbs_FACE); e.More(); e.Next())
  {
    const TopoDS_Face f = TopoDS::Face(e.Current());
    int               idx = -1;
    // The style keys are sub-shapes of the part as stored (its own location included).
    const XCAFPrs_Style* s = by_face.Seek(f);
    if (s == nullptr && !own.IsIdentity())
      s = by_face.Seek(f.Moved(own));
    std::array<float, 4> rgba;
    if (s != nullptr && style_colour(*s, rgba.data()))
    {
      for (size_t k = 0; k < part.colours.size() && idx < 0; ++k)
        if (part.colours[k] == rgba)
          idx = int(k);
      if (idx < 0)
      {
        part.colours.push_back(rgba);
        idx = int(part.colours.size()) - 1;
      }
    }
    part.faces.push_back(f);
    part.colour.push_back(idx);
  }
  BRepBndLib::Add(base, part.box, false);
}

struct Node
{
  uint32_t        parent;
  bool            assembly;
  std::string     name;
  bool            has_colour;
  float           rgba[4];
  TDF_Label       label;
  TopLoc_Location location;
  XCAFPrs_Style   style;
  int             part = -1; // index into the parts (leaves with geometry)
};

// `Label` is the node (an instance, or a free shape), `RefLabel` what it places (the part or
// sub-assembly itself).
std::string node_name(const XCAFPrs_DocumentNode& n)
{
  // An instance's own name, unless it is OCCT's generated "=>[0:1:1:2]".
  std::string inst = label_name(n.Label);
  if (!inst.empty() && inst.rfind("=>", 0) != 0 && !n.RefLabel.IsEqual(n.Label))
  {
    std::string proto = label_name(n.RefLabel);
    if (proto.empty() || proto == inst)
      return inst;
    return inst + " (" + proto + ")";
  }
  return label_name(n.RefLabel);
}

uint64_t count_triangles(Part& p)
{
  uint64_t n = 0;
  for (const TopoDS_Face& f : p.faces)
  {
    TopLoc_Location            loc;
    Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(f, loc);
    if (!t.IsNull())
      n += uint64_t(t->NbTriangles());
  }
  p.triangles = n;
  return n;
}


// ---------------------------------------------------------------------------------------------
// PMI (product manufacturing information): XDE's dimensions, geometric tolerances, datums and
// saved views, as plain data.

enum PmiKind : uint8_t
{
  PMI_DIMENSION = 0,
  PMI_TOLERANCE = 1,
  PMI_DATUM     = 2,
  PMI_TARGET    = 3,
  // A presentation only (a drawn annotation without semantic data).
  PMI_GRAPHIC = 4,
};

// Bits of `Pmi::flags`.
enum PmiFlag : uint8_t
{
  HAS_VALUE  = 1,
  PLUS_MINUS = 2,
  RANGE      = 4,
  FIT        = 8,
  HAS_TEXT   = 16,
  HAS_PLANE  = 32,
  HAS_POINT  = 64,
  HAS_POINT2 = 128,
};

// Most presentation points written for the whole model, and most PMI records.
const size_t MAX_PMI_POINTS = 4000000;
const size_t MAX_PMI        = 100000;

struct DatumRef
{
  std::string          letter;
  uint8_t              position = 0;
  std::vector<uint8_t> modifiers;
  uint8_t              with_value_kind = 0;
  double               with_value      = 0.0;
};

struct Pmi
{
  uint8_t                         kind = PMI_DIMENSION;
  uint8_t                         type = 0;
  std::string                     name;
  uint8_t                         flags = 0;
  double                          value = 0.0, upper = 0.0, lower = 0.0;
  uint8_t                         qualifier = 0, fit_hole = 0, fit_variance = 0, fit_grade = 0;
  uint8_t                         decimals_int = 0, decimals_frac = 0;
  uint8_t                         value_type = 0, material = 0, zone = 0;
  double                          zone_value = 0.0, max_value = 0.0;
  std::vector<uint8_t>            modifiers;
  std::vector<DatumRef>           datums;
  std::string                     letter;
  uint8_t                         target_type   = 0;
  uint32_t                        target_number = 0;
  double                          target_length = 0.0, target_width = 0.0;
  float                           text[3]  = {0, 0, 0};
  float                           plane[9] = {0, 0, 0, 0, 0, 0, 0, 0, 0};
  float                           point[3] = {0, 0, 0}, point2[3] = {0, 0, 0};
  std::vector<std::vector<float>> polylines;
  // The faces it applies to: (part, face index in the part).
  std::vector<std::pair<int, int>> faces;
};

struct View
{
  std::string           name;
  uint8_t               projection = 0;
  float                 eye[3] = {0, 0, 0}, direction[3] = {0, 0, 1}, up[3] = {0, 1, 0};
  double                zoom = 0.0, width = 0.0, height = 0.0;
  std::vector<uint32_t> pmi;
};

std::string ascii(const Handle(TCollection_HAsciiString)& s)
{
  return s.IsNull() ? std::string() : std::string(s->ToCString());
}

std::string entry_of(const TDF_Label& l)
{
  TCollection_AsciiString e;
  TDF_Tool::Entry(l, e);
  return std::string(e.ToCString());
}

void put3(float out[3], const gp_XYZ& p)
{
  out[0] = float(p.X());
  out[1] = float(p.Y());
  out[2] = float(p.Z());
}

void put_plane(Pmi& p, const gp_Ax2& a)
{
  p.flags |= HAS_PLANE;
  put3(p.plane, a.Location().XYZ());
  put3(p.plane + 3, a.XDirection().XYZ());
  put3(p.plane + 6, a.Direction().XYZ());
}

uint8_t u8_of(int v)
{
  return uint8_t(v < 0 ? 0 : v > 255 ? 255 : v);
}

// ---- STEP text: tolerance magnitudes OCCT's reader drops --------------------------------
//
// AP242 writers give a geometric tolerance's magnitude as a complex instance
// (LENGTH_MEASURE_WITH_UNIT and MEASURE_REPRESENTATION_ITEM), which OCCT's reader does not
// take for the attribute and leaves empty: the tolerance would come through without its
// value. The file's text says which instance it is (the third attribute of
// GEOMETRIC_TOLERANCE, in a simple or a complex instance); OCCT did read that instance, so
// it is put back before the transfer.

bool word_char(unsigned char c)
{
  return std::isalnum(c) != 0 || c == '_';
}

bool space_at(const std::string& s, size_t k)
{
  return k < s.size() && std::isspace(static_cast<unsigned char>(s[k])) != 0;
}

// The instance number the magnitude of one instance's text ("#id=" to ";") refers to, or 0.
int magnitude_ref(const std::string& s)
{
  size_t            open = std::string::npos;
  // A complex instance: its GEOMETRIC_TOLERANCE( part; a simple one: its only record.
  const std::string key = "GEOMETRIC_TOLERANCE";
  for (size_t at = s.find(key); at != std::string::npos; at = s.find(key, at + 1))
  {
    if (at > 0 && word_char(static_cast<unsigned char>(s[at - 1])))
      continue;
    size_t k = at + key.size();
    while (space_at(s, k))
      ++k;
    if (k < s.size() && s[k] == '(')
    {
      open = k;
      break;
    }
  }
  if (open == std::string::npos)
  {
    const size_t eq = s.find('=');
    if (eq == std::string::npos)
      return 0;
    size_t k = eq + 1;
    while (space_at(s, k))
      ++k;
    if (k >= s.size() || !word_char(static_cast<unsigned char>(s[k])))
      return 0;
    open = s.find('(', k);
    if (open == std::string::npos)
      return 0;
  }
  // The third attribute at depth 1.
  int    depth = 0, attr = 0;
  bool   quoted = false;
  size_t start  = open + 1;
  for (size_t k = open; k < s.size(); ++k)
  {
    const char c = s[k];
    if (quoted)
    {
      if (c == '\'')
        quoted = false; // a doubled quote opens again at once
      continue;
    }
    if (c == '\'')
    {
      quoted = true;
      continue;
    }
    if (c == '(')
    {
      ++depth;
      continue;
    }
    if (c != ')' && c != ',')
      continue;
    if (depth == 1)
    {
      if (attr == 2)
      {
        size_t a = start;
        while (a < k && space_at(s, a))
          ++a;
        if (a < k && s[a] == '#')
        {
          const long v = std::strtol(s.c_str() + a + 1, nullptr, 10);
          return v > 0 && v < 0x7FFFFFFF ? int(v) : 0;
        }
        return 0;
      }
      ++attr;
      start = k + 1;
    }
    if (c == ')' && --depth == 0)
      return 0;
  }
  return 0;
}

// For each instance number in `want`, its magnitude's instance number (where found).
std::map<int, int> magnitude_refs(const unsigned char* d, size_t n, const std::set<int>& want)
{
  std::map<int, int> out;
  if (want.empty())
    return out;
  const char* t      = reinterpret_cast<const char*>(d);
  size_t      start  = 0;
  bool        quoted = false;
  for (size_t i = 0; i < n; ++i)
  {
    const char c = t[i];
    if (quoted)
    {
      if (c == '\'')
        quoted = false;
      continue;
    }
    if (c == '\'')
    {
      quoted = true;
      continue;
    }
    if (c == '/' && i + 1 < n && t[i + 1] == '*')
    {
      size_t k = i + 2;
      while (k + 1 < n && !(t[k] == '*' && t[k + 1] == '/'))
        ++k;
      if (k + 1 >= n)
        break;
      i = k + 1;
      continue;
    }
    if (c != ';')
      continue;
    // One statement: "#id = ...".
    size_t a = start;
    start    = i + 1;
    while (a < i && std::isspace(static_cast<unsigned char>(t[a])) != 0)
      ++a;
    if (a >= i || t[a] != '#')
      continue;
    const long id = std::strtol(t + a + 1, nullptr, 10);
    if (id <= 0 || id >= 0x7FFFFFFF || want.count(int(id)) == 0)
      continue;
    const int ref = magnitude_ref(std::string(t + a, i - a));
    if (ref > 0)
      out[int(id)] = ref;
  }
  return out;
}

// Puts back the magnitudes OCCT's reader left empty; how many.
int restore_magnitudes(const Handle(StepData_StepModel)& model, const unsigned char* d, size_t n)
{
  if (model.IsNull())
    return 0;
  std::map<int, Handle(StepDimTol_GeometricTolerance)> missing;
  std::set<int>                                         want;
  const int                                             count = model->NbEntities();
  for (int i = 1; i <= count; ++i)
  {
    Handle(StepDimTol_GeometricTolerance) gt =
      Handle(StepDimTol_GeometricTolerance)::DownCast(model->Value(i));
    if (gt.IsNull() || !gt->Magnitude().IsNull())
      continue;
    const int id = model->IdentLabel(gt);
    if (id > 0)
    {
      missing[id] = gt;
      want.insert(id);
    }
  }
  if (want.empty())
    return 0;
  const std::map<int, int> refs = magnitude_refs(d, n, want);
  std::set<int>            targets;
  for (const auto& r : refs)
    targets.insert(r.second);
  std::map<int, Handle(StepBasic_MeasureWithUnit)> measures;
  for (int i = 1; i <= count && measures.size() < targets.size(); ++i)
  {
    Handle(StepRepr_ReprItemAndMeasureWithUnit) m =
      Handle(StepRepr_ReprItemAndMeasureWithUnit)::DownCast(model->Value(i));
    if (m.IsNull())
      continue;
    const int id = model->IdentLabel(m);
    if (targets.count(id) != 0 && !m->GetMeasureWithUnit().IsNull())
      measures[id] = m->GetMeasureWithUnit();
  }
  int restored = 0;
  for (const auto& r : refs)
  {
    auto m = measures.find(r.second);
    if (m == measures.end())
      continue;
    missing[r.first]->SetMagnitude(m->second);
    ++restored;
  }
  return restored;
}

// ---- Qualified values OCCT's reader leaves in the file's unit -----------------------------
//
// A dimension's value with a qualifier (QUALIFIED_REPRESENTATION_ITEM: decimal places,
// minimum, maximum) is read without its unit's conversion: in an inch file a ⌀0.438 stays
// 0.438 while every other length is in millimetres. Such values are converted in the model
// before the transfer: lengths to millimetres (the document's unit), plane angles to degrees.

enum class UnitKind
{
  Other,
  Length,
  Angle,
};

// What `u` measures and its factor to millimetres or degrees (at most a few conversions deep).
UnitKind unit_factor(const Handle(StepBasic_NamedUnit)& u, double& f, int depth = 0)
{
  if (u.IsNull() || depth > 4)
    return UnitKind::Other;
  Handle(StepBasic_SiUnit) si = Handle(StepBasic_SiUnit)::DownCast(u);
  if (!si.IsNull())
  {
    double p = 1.0;
    if (si->HasPrefix())
    {
      static const double prefixes[] = {1e18, 1e15, 1e12, 1e9, 1e6, 1e3, 1e2, 1e1, 1e-1,
                                        1e-2, 1e-3, 1e-6, 1e-9, 1e-12, 1e-15, 1e-18};
      const int           k          = int(si->Prefix());
      if (k < 0 || k >= int(sizeof(prefixes) / sizeof(prefixes[0])))
        return UnitKind::Other;
      p = prefixes[k];
    }
    if (si->Name() == StepBasic_sunMetre)
    {
      f = p * 1000.0;
      return UnitKind::Length;
    }
    if (si->Name() == StepBasic_sunRadian)
    {
      f = p * 180.0 / M_PI;
      return UnitKind::Angle;
    }
    return UnitKind::Other;
  }
  Handle(StepBasic_ConversionBasedUnit) cb = Handle(StepBasic_ConversionBasedUnit)::DownCast(u);
  if (cb.IsNull() || cb->ConversionFactor().IsNull())
    return UnitKind::Other;
  const Handle(StepBasic_MeasureWithUnit)& by = cb->ConversionFactor();
  const StepBasic_Unit                     inner = by->UnitComponent();
  if (inner.CaseNum(inner.Value()) != 1)
    return UnitKind::Other;
  double         g    = 1.0;
  const UnitKind kind = unit_factor(inner.NamedUnit(), g, depth + 1);
  f                   = by->ValueComponent() * g;
  return std::isfinite(f) && f > 0.0 ? kind : UnitKind::Other;
}

// Converts the qualified values; how many.
int convert_qualified(const Handle(StepData_StepModel)& model)
{
  if (model.IsNull())
    return 0;
  int       n     = 0;
  const int count = model->NbEntities();
  // The bounds of plus/minus tolerances and tolerance magnitudes are converted by the
  // reader: left alone.
  std::set<const Standard_Transient*> bounds;
  for (int i = 1; i <= count; ++i)
  {
    const Handle(Standard_Transient)& e  = model->Value(i);
    Handle(StepShape_ToleranceValue)  tv = Handle(StepShape_ToleranceValue)::DownCast(e);
    if (!tv.IsNull())
    {
      bounds.insert(tv->LowerBound().get());
      bounds.insert(tv->UpperBound().get());
    }
    Handle(StepDimTol_GeometricTolerance) gt = Handle(StepDimTol_GeometricTolerance)::DownCast(e);
    if (!gt.IsNull() && !gt->Magnitude().IsNull())
      bounds.insert(gt->Magnitude().get());
  }
  for (int i = 1; i <= count; ++i)
  {
    if (bounds.count(model->Value(i).get()) != 0)
      continue;
    Handle(StepRepr_ReprItemAndMeasureWithUnitAndQRI) q =
      Handle(StepRepr_ReprItemAndMeasureWithUnitAndQRI)::DownCast(model->Value(i));
    if (q.IsNull() || q->GetMeasureWithUnit().IsNull())
      continue;
    Handle(StepBasic_MeasureWithUnit) m = q->GetMeasureWithUnit();
    if (bounds.count(m.get()) != 0)
      continue;
    const StepBasic_Unit u = m->UnitComponent();
    if (u.IsNull() || u.CaseNum(u.Value()) != 1)
      continue;
    double f = 1.0;
    if (unit_factor(u.NamedUnit(), f) == UnitKind::Other || std::abs(f - 1.0) < 1e-12)
      continue;
    m->SetValueComponent(m->ValueComponent() * f);
    ++n;
  }
  return n;
}

// The edges of a presentation as polylines (straight edges as they are, curves within
// `deflection`), at most `budget` points in all.
void polylines_of(const TopoDS_Shape&              s,
                  double                           deflection,
                  std::vector<std::vector<float>>& out,
                  size_t&                          budget)
{
  if (s.IsNull())
    return;
  for (TopExp_Explorer e(s, TopAbs_EDGE); e.More() && budget > 0; e.Next())
  {
    const TopoDS_Edge& edge = TopoDS::Edge(e.Current());
    if (BRep_Tool::Degenerated(edge))
      continue;
    std::vector<gp_Pnt> pts;
    try
    {
      BRepAdaptor_Curve c(edge);
      const double      f = c.FirstParameter(), l = c.LastParameter();
      if (!std::isfinite(f) || !std::isfinite(l))
        continue;
      if (c.GetType() == GeomAbs_Line)
      {
        pts.push_back(c.Value(f));
        pts.push_back(c.Value(l));
      }
      else
      {
        GCPnts_TangentialDeflection d(c, 0.25, deflection, 2);
        for (int k = 1; k <= d.NbPoints(); ++k)
          pts.push_back(d.Value(k));
      }
    }
    catch (const Standard_Failure&)
    {
      continue;
    }
    if (pts.size() < 2)
      continue;
    if (edge.Orientation() == TopAbs_REVERSED)
      std::reverse(pts.begin(), pts.end());
    // Joined to the last polyline where it goes on from its end.
    size_t from = 0;
    if (!out.empty())
    {
      const std::vector<float>& last = out.back();
      const size_t              k    = last.size();
      const gp_Pnt              end(last[k - 3], last[k - 2], last[k - 1]);
      if (end.Distance(pts[0]) <= 1e-6 * (1.0 + end.XYZ().Modulus()))
        from = 1;
    }
    if (from == 0)
      out.emplace_back();
    std::vector<float>& line = out.back();
    for (size_t k = from; k < pts.size() && budget > 0; ++k, --budget)
    {
      line.push_back(float(pts[k].X()));
      line.push_back(float(pts[k].Y()));
      line.push_back(float(pts[k].Z()));
    }
    if (line.size() < 6)
      out.pop_back();
  }
}

struct Model
{
  Handle(TDocStd_Document) doc;
  // A STEP file's cameras (read before the reader goes; XDE may skip some).
  std::vector<View> cameras;
  std::string              file_unit;
  std::vector<std::string> notes;

  Model() = default;
  Model(const Model&)            = delete;
  Model& operator=(const Model&) = delete;

  // The document is closed however the read ends.
  ~Model()
  {
    if (doc.IsNull())
      return;
    try
    {
      XCAFApp_Application::GetApplication()->Close(doc);
    }
    catch (...)
    {
    }
  }
};

// Every camera of a STEP file as a view (its name, direction, up and projection; no PMI).
std::vector<View> camera_views(const Handle(StepData_StepModel)& model)
{
  std::vector<View> out;
  if (model.IsNull())
    return out;
  const int count = model->NbEntities();
  for (int i = 1; i <= count && out.size() < 10000; ++i)
  {
    Handle(StepVisual_CameraModelD3) c = Handle(StepVisual_CameraModelD3)::DownCast(model->Value(i));
    if (c.IsNull() || c->ViewReferenceSystem().IsNull())
      continue;
    View v;
    v.name                                    = ascii(c->Name());
    const Handle(StepGeom_Axis2Placement3d)& a = c->ViewReferenceSystem();
    gp_Dir                                   z(0, 0, 1), x(1, 0, 0);
    try
    {
      auto dir = [](const Handle(StepGeom_Direction)& d, gp_Dir& o) {
        if (!d.IsNull() && d->NbDirectionRatios() >= 3)
          o = gp_Dir(d->DirectionRatiosValue(1), d->DirectionRatiosValue(2), d->DirectionRatiosValue(3));
      };
      if (a->HasAxis())
        dir(a->Axis(), z);
      if (a->HasRefDirection())
        dir(a->RefDirection(), x);
      // As XDE takes it: up is the direction crossed with the reference direction.
      const gp_Dir up = z.Crossed(x);
      put3(v.direction, z.XYZ());
      put3(v.up, up.XYZ());
    }
    catch (const Standard_Failure&)
    {
      continue;
    }
    const Handle(StepVisual_ViewVolume)& vol = c->PerspectiveOfVolume();
    if (!vol.IsNull())
    {
      v.projection = vol->ProjectionType() == StepVisual_copCentral ? 1 : 2;
      v.zoom       = vol->ViewPlaneDistance();
    }
    out.push_back(std::move(v));
  }
  return out;
}

class Reader
{
public:
  Reader(const SpecularOcctParams& p, Out& out)
      : myParams(p),
        myOut(out)
  {
  }

  void run(Model& m)
  {
    explore(m.doc);
    if (myParts.empty())
      fail(UNSUPPORTED, "the file has no surfaces or solids, only curves or points");
    mesh_all();
    collect_pmi(m.doc);
    collect_views(m.doc);
    more_views(m.cameras);
    write(m);
  }

private:
  void explore(const Handle(TDocStd_Document)& doc)
  {
    std::vector<uint32_t>         stack;
    std::map<std::string, int>    part_of_label;
    for (XCAFPrs_DocumentExplorer ex(doc, XCAFPrs_DocumentExplorerFlags_None); ex.More();
         ex.Next())
    {
      const XCAFPrs_DocumentNode& cur   = ex.Current();
      const size_t                depth = size_t(ex.CurrentDepth());
      if (stack.size() > depth)
        stack.resize(depth);
      Node n;
      n.parent   = stack.empty() ? NO_PARENT : stack.back();
      n.assembly = cur.IsAssembly;
      n.name     = node_name(cur);
      n.label    = cur.Label;
      n.location = cur.Location;
      n.style    = cur.Style;
      n.has_colour = style_colour(cur.Style, n.rgba);
      if (!n.has_colour)
        n.rgba[0] = n.rgba[1] = n.rgba[2] = n.rgba[3] = 1.0f;
      if (!cur.IsAssembly)
      {
        TCollection_AsciiString entry;
        TDF_Tool::Entry(cur.RefLabel, entry);
        const std::string key(entry.ToCString());
        auto              found = part_of_label.find(key);
        if (found != part_of_label.end())
        {
          n.part = found->second;
        }
        else
        {
          TopoDS_Shape s;
          if (XCAFDoc_ShapeTool::GetShape(cur.RefLabel, s) && !s.IsNull())
          {
            Part p;
            p.shape = s;
            collect_part(cur.RefLabel, p);
            if (!p.faces.empty())
            {
              myParts.push_back(std::move(p));
              n.part = int(myParts.size()) - 1;
            }
          }
          part_of_label[key] = n.part;
        }
      }
      const uint32_t id = uint32_t(myNodes.size());
      myNodes.push_back(std::move(n));
      stack.push_back(id);
      if (myNodes.size() > 4000000)
        fail(TOO_LARGE, "the assembly has more than 4 million parts");
    }
  }

  // The whole model's box: each part's box moved to each of its instances.
  void model_box()
  {
    myBox.SetVoid();
    for (const Node& n : myNodes)
    {
      if (n.part < 0 || myParts[size_t(n.part)].box.IsVoid())
        continue;
      myBox.Add(myParts[size_t(n.part)].box.Transformed(n.location.Transformation()));
    }
  }

  uint64_t total_triangles()
  {
    uint64_t total = 0;
    for (const Node& n : myNodes)
      if (n.part >= 0)
        total += myParts[size_t(n.part)].triangles;
    return total;
  }

  // Every distinct part once, in parallel, with one deflection for the model.
  void mesh_all()
  {
    model_box();
    double diag = 1.0;
    if (!myBox.IsVoid())
    {
      const double d = std::sqrt(myBox.SquareExtent());
      if (std::isfinite(d) && d > 0.0)
        diag = d;
    }
    BRep_Builder    b;
    TopoDS_Compound all;
    b.MakeCompound(all);
    for (const Part& p : myParts)
      b.Add(all, p.shape.Located(TopLoc_Location()));
    double rel   = myParams.relative_deflection;
    double angle = myParams.angular_deflection;
    for (int attempt = 0;; ++attempt)
    {
      myDeflection = std::max(diag * rel, 1e-7);
      IMeshTools_Parameters mp;
      mp.Deflection = myDeflection;
      mp.Angle      = angle;
      mp.Relative   = Standard_False;
      mp.InParallel = Standard_True;
      // Faces need not be meshed again where the file already carried a finer mesh.
      BRepMesh_IncrementalMesh mesher(all, mp);
      for (Part& p : myParts)
        count_triangles(p);
      const uint64_t total = total_triangles();
      if (total <= myParams.max_triangles)
        break;
      if (attempt >= 3)
        fail(TOO_LARGE,
             "the model needs more than " + std::to_string(myParams.max_triangles)
               + " triangles even at a coarse mesh");
      // Four times coarser, and drop the old mesh so it is redone.
      rel *= 4.0;
      angle = std::min(angle * 1.5, 1.2);
      BRepTools::Clean(all);
      myNotes.push_back("meshed coarser to stay within the triangle limit");
    }
    for (Part& p : myParts)
    {
      Poly_Connect connect;
      for (const TopoDS_Face& f : p.faces)
      {
        TopLoc_Location            loc;
        Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(f, loc);
        if (t.IsNull() || t->NbTriangles() == 0)
        {
          ++myFacesWithout;
          continue;
        }
        BRepLib_ToolTriangulatedShape::ComputeNormals(f, t, connect);
      }
    }
  }

  void write(const Model& m)
  {
    // Counted over every instance placed (as the model is seen).
    std::vector<uint32_t> part_solids(myParts.size(), 0);
    for (size_t i = 0; i < myParts.size(); ++i)
      for (TopExp_Explorer e(myParts[i].shape, TopAbs_SOLID); e.More(); e.Next())
        ++part_solids[i];
    uint32_t solids = 0, faces = 0, instances = 0;
    for (const Node& n : myNodes)
    {
      if (n.part < 0)
        continue;
      ++instances;
      faces += uint32_t(myParts[size_t(n.part)].faces.size());
      solids += part_solids[size_t(n.part)];
    }
    double unit = 0.001;
    Standard_Real metres = 0.0;
    if (XCAFDoc_DocumentTool::GetLengthUnit(m.doc, metres) && metres > 0.0)
      unit = metres;

    myOut.raw("SPCADM01", 8);
    {
      Body body;
      body.put<double>(unit);
      body.str(m.file_unit);
      body.put<uint32_t>(solids);
      body.put<uint32_t>(faces);
      body.put<uint32_t>(myFacesWithout);
      body.put<uint32_t>(instances);
      body.put<double>(myDeflection);
      double box[6] = {0, 0, 0, 0, 0, 0};
      if (!myBox.IsVoid())
        myBox.Get(box[0], box[1], box[2], box[3], box[4], box[5]);
      for (double v : box)
        body.put<float>(float(v));
      myOut.record(1, body.b);
    }
    for (const Node& n : myNodes)
    {
      Body body;
      body.put<uint32_t>(n.parent);
      body.u8(n.assembly ? 0 : 1);
      body.u8(n.has_colour ? 1 : 0);
      for (float v : n.rgba)
        body.put<float>(v);
      body.str(n.name);
      myOut.record(2, body.b);
    }
    for (size_t i = 0; i < myNodes.size(); ++i)
      if (myNodes[i].part >= 0)
        write_instance(uint32_t(i), myNodes[i]);
    for (const Pmi& p : myPmi)
      write_pmi(p);
    for (const View& v : myViews)
      write_view(v);
    std::vector<std::string> notes = m.notes;
    notes.insert(notes.end(), myNotes.begin(), myNotes.end());
    if (myFacesWithout > 0)
      notes.push_back("faces OpenCASCADE could not mesh, left out: " + std::to_string(myFacesWithout));
    for (const std::string& s : notes)
    {
      Body body;
      body.str(s);
      myOut.record(4, body.b);
    }
    myOut.record(0, std::vector<unsigned char>());
    myOut.flush();
  }

  // The faces of the shapes an XDE GD&T label refers to.
  void faces_of(const TDF_Label& l, Pmi& p)
  {
    TDF_LabelSequence first, second;
    if (!XCAFDoc_DimTolTool::GetRefShapeLabel(l, first, second))
      return;
    first.Append(second);
    if (myFaceOf.empty())
      for (size_t pi = 0; pi < myParts.size(); ++pi)
        for (size_t fi = 0; fi < myParts[pi].faces.size(); ++fi)
          myFaceOf[myParts[pi].faces[fi].TShape().get()].push_back({int(pi), int(fi)});
    for (TDF_LabelSequence::Iterator it(first); it.More(); it.Next())
    {
      const TopoDS_Shape s = XCAFDoc_ShapeTool::GetShape(it.Value());
      if (s.IsNull())
        continue;
      for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
      {
        auto found = myFaceOf.find(e.Current().TShape().get());
        if (found == myFaceOf.end())
          continue;
        for (const auto& pf : found->second)
          if (std::find(p.faces.begin(), p.faces.end(), pf) == p.faces.end())
            p.faces.push_back(pf);
      }
    }
  }

  uint32_t add_pmi(Pmi&& p, const TDF_Label& l)
  {
    if (myPmi.size() >= MAX_PMI)
      fail(TOO_LARGE, "the model has more than 100,000 PMI annotations");
    for (const auto& f : p.faces)
      myNeeded.insert(f);
    myPmi.push_back(std::move(p));
    const uint32_t i            = uint32_t(myPmi.size() - 1);
    myPmiOfLabel[entry_of(l)]   = i;
    return i;
  }

  // Dimensions, geometric tolerances, datums and datum targets, then the annotations that
  // are only drawn (those whose drawing belongs to one already read are left out).
  void collect_pmi(const Handle(TDocStd_Document)& doc)
  {
    if (!XCAFDoc_DocumentTool::CheckDimTolTool(doc->Main()))
      return;
    Handle(XCAFDoc_DimTolTool)         tool = XCAFDoc_DocumentTool::DimTolTool(doc->Main());
    size_t                             budget = MAX_PMI_POINTS;
    // Presentation names of what was read, and the record each is.
    std::map<std::string, uint32_t>    by_drawing;
    std::vector<TDF_Label>             graphic;
    const double                       defl = std::max(myDeflection, 1e-6);
    auto drawing = [&](Pmi& p, const TopoDS_Shape& s, const std::string& name) {
      polylines_of(s, defl, p.polylines, budget);
      if (p.name.empty())
        p.name = name;
    };
    TDF_LabelSequence labels;
    tool->GetDimensionLabels(labels);
    for (TDF_LabelSequence::Iterator it(labels); it.More(); it.Next())
    {
      Handle(XCAFDoc_Dimension) a;
      if (!it.Value().FindAttribute(XCAFDoc_Dimension::GetID(), a))
        continue;
      Handle(XCAFDimTolObjects_DimensionObject) o = a->GetObject();
      if (o.IsNull())
        continue;
      const XCAFDimTolObjects_DimensionType t = o->GetType();
      if (t == XCAFDimTolObjects_DimensionType_CommonLabel
          || t == XCAFDimTolObjects_DimensionType_DimensionPresentation)
      {
        graphic.push_back(it.Value());
        continue;
      }
      Pmi p;
      p.kind = PMI_DIMENSION;
      p.type = u8_of(int(t));
      p.name = ascii(o->GetPresentationName());
      Handle(TColStd_HArray1OfReal) vals = o->GetValues();
      if (!vals.IsNull() && vals->Length() > 0)
      {
        p.flags |= HAS_VALUE;
        p.value = o->GetValue();
      }
      if (o->IsDimWithPlusMinusTolerance())
      {
        p.flags |= PLUS_MINUS;
        p.upper = o->GetUpperTolValue();
        p.lower = o->GetLowerTolValue();
      }
      else if (o->IsDimWithRange())
      {
        p.flags |= RANGE;
        p.upper = o->GetUpperBound();
        p.lower = o->GetLowerBound();
      }
      if (o->IsDimWithClassOfTolerance())
      {
        Standard_Boolean                        hole = Standard_False;
        XCAFDimTolObjects_DimensionFormVariance fv;
        XCAFDimTolObjects_DimensionGrade        grade;
        if (o->GetClassOfTolerance(hole, fv, grade))
        {
          p.flags |= FIT;
          p.fit_hole     = hole ? 1 : 0;
          p.fit_variance = u8_of(int(fv));
          p.fit_grade    = u8_of(int(grade));
        }
      }
      p.qualifier = u8_of(int(o->GetQualifier()));
      Standard_Integer dl = 0, dr = 0;
      o->GetNbOfDecimalPlaces(dl, dr);
      p.decimals_int  = u8_of(dl);
      p.decimals_frac = u8_of(dr);
      const XCAFDimTolObjects_DimensionModifiersSequence mods = o->GetModifiers();
      for (int k = 1; k <= mods.Length(); ++k)
        p.modifiers.push_back(u8_of(int(mods(k))));
      if (o->HasTextPoint())
      {
        p.flags |= HAS_TEXT;
        put3(p.text, o->GetPointTextAttach().XYZ());
      }
      if (o->HasPlane())
        put_plane(p, o->GetPlane());
      if (o->HasPoint())
      {
        p.flags |= HAS_POINT;
        put3(p.point, o->GetPoint().XYZ());
      }
      if (o->HasPoint2())
      {
        p.flags |= HAS_POINT2;
        put3(p.point2, o->GetPoint2().XYZ());
      }
      drawing(p, o->GetPresentation(), ascii(o->GetSemanticName()));
      faces_of(it.Value(), p);
      const std::string drawn = ascii(o->GetPresentationName());
      const uint32_t    i     = add_pmi(std::move(p), it.Value());
      if (!drawn.empty())
        by_drawing.emplace(drawn, i);
    }
    labels.Clear();
    tool->GetGeomToleranceLabels(labels);
    for (TDF_LabelSequence::Iterator it(labels); it.More(); it.Next())
    {
      Handle(XCAFDoc_GeomTolerance) a;
      if (!it.Value().FindAttribute(XCAFDoc_GeomTolerance::GetID(), a))
        continue;
      Handle(XCAFDimTolObjects_GeomToleranceObject) o = a->GetObject();
      if (o.IsNull())
        continue;
      Pmi p;
      p.kind       = PMI_TOLERANCE;
      p.type       = u8_of(int(o->GetType()));
      p.name       = ascii(o->GetPresentationName());
      p.flags     |= HAS_VALUE;
      p.value      = o->GetValue();
      p.value_type = u8_of(int(o->GetTypeOfValue()));
      p.material   = u8_of(int(o->GetMaterialRequirementModifier()));
      p.zone       = u8_of(int(o->GetZoneModifier()));
      p.zone_value = o->GetValueOfZoneModifier();
      p.max_value  = o->GetMaxValueModifier();
      const XCAFDimTolObjects_GeomToleranceModifiersSequence mods = o->GetModifiers();
      for (int k = 1; k <= mods.Length(); ++k)
        p.modifiers.push_back(u8_of(int(mods(k))));
      TDF_LabelSequence datums;
      XCAFDoc_DimTolTool::GetDatumWithObjectOfTolerLabels(it.Value(), datums);
      for (TDF_LabelSequence::Iterator d(datums); d.More(); d.Next())
      {
        Handle(XCAFDoc_Datum) da;
        if (!d.Value().FindAttribute(XCAFDoc_Datum::GetID(), da))
          continue;
        Handle(XCAFDimTolObjects_DatumObject) dob = da->GetObject();
        if (dob.IsNull())
          continue;
        DatumRef r;
        r.letter   = ascii(dob->GetName());
        r.position = u8_of(dob->GetPosition());
        const XCAFDimTolObjects_DatumModifiersSequence dm = dob->GetModifiers();
        for (int k = 1; k <= dm.Length(); ++k)
          r.modifiers.push_back(u8_of(int(dm(k))));
        XCAFDimTolObjects_DatumModifWithValue wk = XCAFDimTolObjects_DatumModifWithValue_None;
        Standard_Real                          wv = 0.0;
        dob->GetModifierWithValue(wk, wv);
        r.with_value_kind = u8_of(int(wk));
        r.with_value      = wv;
        p.datums.push_back(std::move(r));
      }
      std::stable_sort(p.datums.begin(), p.datums.end(), [](const DatumRef& x, const DatumRef& y) {
        return x.position < y.position;
      });
      if (o->HasPointText())
      {
        p.flags |= HAS_TEXT;
        put3(p.text, o->GetPointTextAttach().XYZ());
      }
      if (o->HasPlane())
        put_plane(p, o->GetPlane());
      if (o->HasPoint())
      {
        p.flags |= HAS_POINT;
        put3(p.point, o->GetPoint().XYZ());
      }
      drawing(p, o->GetPresentation(), ascii(o->GetSemanticName()));
      faces_of(it.Value(), p);
      const std::string drawn = ascii(o->GetPresentationName());
      const uint32_t    i     = add_pmi(std::move(p), it.Value());
      if (!drawn.empty())
        by_drawing.emplace(drawn, i);
    }
    // Datums: XDE keeps one per tolerance that refers to it; one each is kept.
    labels.Clear();
    tool->GetDatumLabels(labels);
    std::map<std::string, uint32_t> datum_of;
    for (TDF_LabelSequence::Iterator it(labels); it.More(); it.Next())
    {
      Handle(XCAFDoc_Datum) a;
      if (!it.Value().FindAttribute(XCAFDoc_Datum::GetID(), a))
        continue;
      Handle(XCAFDimTolObjects_DatumObject) o = a->GetObject();
      if (o.IsNull())
        continue;
      Pmi p;
      p.kind   = o->IsDatumTarget() ? PMI_TARGET : PMI_DATUM;
      p.letter = ascii(o->GetName());
      if (o->IsDatumTarget())
      {
        p.type          = u8_of(int(o->GetDatumTargetType()));
        p.target_type   = p.type;
        p.target_number = uint32_t(std::max(0, o->GetDatumTargetNumber()));
        p.target_length = o->GetDatumTargetLength();
        p.target_width  = o->GetDatumTargetWidth();
        const gp_Ax2 ax = o->GetDatumTargetAxis();
        p.flags |= HAS_POINT;
        put3(p.point, ax.Location().XYZ());
      }
      p.name = ascii(o->GetPresentationName());
      if (o->HasPointText())
      {
        p.flags |= HAS_TEXT;
        put3(p.text, o->GetPointTextAttach().XYZ());
      }
      if (o->HasPlane())
        put_plane(p, o->GetPlane());
      if (o->HasPoint() && !o->IsDatumTarget())
      {
        p.flags |= HAS_POINT;
        put3(p.point, o->GetPoint().XYZ());
      }
      faces_of(it.Value(), p);
      const std::string key = p.letter + (o->IsDatumTarget() ? "#" + std::to_string(p.target_number) : "");
      auto              seen = datum_of.find(key);
      if (seen != datum_of.end())
      {
        Pmi& kept = myPmi[seen->second];
        if (kept.faces.empty() && !p.faces.empty())
        {
          kept.faces = p.faces;
          for (const auto& f : p.faces)
            myNeeded.insert(f);
        }
        myPmiOfLabel[entry_of(it.Value())] = seen->second;
        continue;
      }
      const std::string drawn = ascii(o->GetPresentationName());
      drawing(p, o->GetPresentation(), std::string());
      const uint32_t i = add_pmi(std::move(p), it.Value());
      datum_of.emplace(key, i);
      if (!drawn.empty())
        by_drawing.emplace(drawn, i);
    }
    // Drawn only.
    for (const TDF_Label& l : graphic)
    {
      Handle(XCAFDoc_Dimension) a;
      l.FindAttribute(XCAFDoc_Dimension::GetID(), a);
      Handle(XCAFDimTolObjects_DimensionObject) o = a->GetObject();
      const std::string drawn = ascii(o->GetPresentationName());
      auto              twin  = by_drawing.find(drawn);
      if (!drawn.empty() && twin != by_drawing.end())
      {
        myPmiOfLabel[entry_of(l)] = twin->second;
        continue;
      }
      Pmi p;
      p.kind = PMI_GRAPHIC;
      p.type = u8_of(int(o->GetType()));
      p.name = drawn;
      if (o->HasTextPoint())
      {
        p.flags |= HAS_TEXT;
        put3(p.text, o->GetPointTextAttach().XYZ());
      }
      if (o->HasPlane())
        put_plane(p, o->GetPlane());
      drawing(p, o->GetPresentation(), std::string());
      faces_of(l, p);
      if (p.polylines.empty() && p.faces.empty())
        continue;
      add_pmi(std::move(p), l);
    }
    if (budget == 0)
      myNotes.push_back("PMI drawings cut short at 4 million points");
  }

  void collect_views(const Handle(TDocStd_Document)& doc)
  {
    if (!XCAFDoc_DocumentTool::CheckViewTool(doc->Main()))
      return;
    Handle(XCAFDoc_ViewTool) tool = XCAFDoc_DocumentTool::ViewTool(doc->Main());
    TDF_LabelSequence        labels;
    tool->GetViewLabels(labels);
    for (TDF_LabelSequence::Iterator it(labels); it.More() && myViews.size() < 10000; it.Next())
    {
      Handle(XCAFDoc_View) a;
      if (!it.Value().FindAttribute(XCAFDoc_View::GetID(), a))
        continue;
      Handle(XCAFView_Object) o = a->GetObject();
      if (o.IsNull())
        continue;
      View v;
      v.name       = ascii(o->Name());
      v.projection = u8_of(int(o->Type()));
      put3(v.eye, o->ProjectionPoint().XYZ());
      put3(v.direction, o->ViewDirection().XYZ());
      put3(v.up, o->UpDirection().XYZ());
      v.zoom   = o->ZoomFactor();
      v.width  = o->WindowHorizontalSize();
      v.height = o->WindowVerticalSize();
      TDF_LabelSequence gdt;
      tool->GetRefGDTLabel(it.Value(), gdt);
      for (TDF_LabelSequence::Iterator g(gdt); g.More(); g.Next())
      {
        auto found = myPmiOfLabel.find(entry_of(g.Value()));
        if (found != myPmiOfLabel.end()
            && std::find(v.pmi.begin(), v.pmi.end(), found->second) == v.pmi.end())
          v.pmi.push_back(found->second);
      }
      myViews.push_back(std::move(v));
    }
  }

  // Cameras XDE has no view for (it stops at the first one it cannot place in a drawing
  // model): their names and directions, showing every annotation.
  void more_views(const std::vector<View>& cameras)
  {
    std::set<std::string> known;
    for (const View& v : myViews)
      known.insert(v.name);
    for (const View& v : cameras)
      if (myViews.size() < 10000 && known.count(v.name) == 0)
        myViews.push_back(v);
  }

  void write_pmi(const Pmi& p)
  {
    Body b;
    b.u8(p.kind);
    b.u8(p.type);
    b.str(p.name);
    b.u8(p.flags);
    b.put<double>(p.value);
    b.put<double>(p.upper);
    b.put<double>(p.lower);
    for (uint8_t v : {p.qualifier, p.fit_hole, p.fit_variance, p.fit_grade, p.decimals_int,
                      p.decimals_frac, p.value_type, p.material, p.zone})
      b.u8(v);
    b.put<double>(p.zone_value);
    b.put<double>(p.max_value);
    b.put<uint16_t>(uint16_t(std::min<size_t>(p.modifiers.size(), 255)));
    for (size_t k = 0; k < p.modifiers.size() && k < 255; ++k)
      b.u8(p.modifiers[k]);
    b.put<uint16_t>(uint16_t(std::min<size_t>(p.datums.size(), 255)));
    for (size_t k = 0; k < p.datums.size() && k < 255; ++k)
    {
      const DatumRef& d = p.datums[k];
      b.str(d.letter);
      b.u8(d.position);
      b.put<uint16_t>(uint16_t(std::min<size_t>(d.modifiers.size(), 255)));
      for (size_t j = 0; j < d.modifiers.size() && j < 255; ++j)
        b.u8(d.modifiers[j]);
      b.u8(d.with_value_kind);
      b.put<double>(d.with_value);
    }
    b.str(p.letter);
    b.u8(p.target_type);
    b.put<uint32_t>(p.target_number);
    b.put<double>(p.target_length);
    b.put<double>(p.target_width);
    for (float v : p.text)
      b.put<float>(v);
    for (float v : p.plane)
      b.put<float>(v);
    for (float v : p.point)
      b.put<float>(v);
    for (float v : p.point2)
      b.put<float>(v);
    b.put<uint32_t>(uint32_t(p.polylines.size()));
    for (const std::vector<float>& l : p.polylines)
    {
      b.put<uint32_t>(uint32_t(l.size() / 3));
      for (float v : l)
        b.put<float>(v);
    }
    std::vector<std::array<uint32_t, 3>> tris;
    for (const auto& f : p.faces)
    {
      auto found = myFaceTris.find(f);
      if (found != myFaceTris.end())
        tris.insert(tris.end(), found->second.begin(), found->second.end());
    }
    b.put<uint32_t>(uint32_t(tris.size()));
    for (const auto& t : tris)
      for (uint32_t v : t)
        b.put<uint32_t>(v);
    myOut.record(5, b.b);
  }

  void write_view(const View& v)
  {
    Body b;
    b.str(v.name);
    b.u8(v.projection);
    for (const float* a : {v.eye, v.direction, v.up})
      for (int k = 0; k < 3; ++k)
        b.put<float>(a[k]);
    b.put<double>(v.zoom);
    b.put<double>(v.width);
    b.put<double>(v.height);
    b.put<uint32_t>(uint32_t(v.pmi.size()));
    for (uint32_t i : v.pmi)
      b.put<uint32_t>(i);
    myOut.record(6, b.b);
  }

  // One MESH record per colour of this instance.
  void write_instance(uint32_t id, const Node& n)
  {
    const Part& p = myParts[size_t(n.part)];
    // Faces grouped by their style (-1: the instance's own).
    std::map<int, std::vector<size_t>> groups;
    for (size_t i = 0; i < p.faces.size(); ++i)
      groups[p.colour[i]].push_back(i);
    const gp_Trsf inst     = n.location.Transformation();
    for (const auto& g : groups)
    {
      float rgba[4] = {1, 1, 1, 1};
      bool  has     = false;
      if (g.first >= 0)
      {
        has = true;
        std::memcpy(rgba, p.colours[size_t(g.first)].data(), sizeof rgba);
      }
      if (!has && n.has_colour)
      {
        has = true;
        std::memcpy(rgba, n.rgba, sizeof rgba);
      }
      std::vector<float>    pos, nrm;
      std::vector<uint32_t> idx;
      std::vector<std::pair<std::pair<int, int>, std::array<uint32_t, 2>>> marked;
      for (size_t fi : g.second)
      {
        const std::pair<int, int> key(n.part, int(fi));
        const uint32_t            first_tri = uint32_t(idx.size() / 3);
        const TopoDS_Face&         f = p.faces[fi];
        TopLoc_Location            loc;
        Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(f, loc);
        if (t.IsNull() || t->NbTriangles() == 0)
          continue;
        gp_Trsf tr = inst;
        tr.Multiply(loc.Transformation());
        const bool flip =
          (f.Orientation() == TopAbs_REVERSED) != (tr.VectorialPart().Determinant() < 0.0);
        const bool reversed = f.Orientation() == TopAbs_REVERSED;
        const uint32_t first = uint32_t(pos.size() / 3);
        const int      nodes = t->NbNodes();
        for (int k = 1; k <= nodes; ++k)
        {
          gp_Pnt q = t->Node(k).Transformed(tr);
          pos.push_back(float(q.X()));
          pos.push_back(float(q.Y()));
          pos.push_back(float(q.Z()));
          gp_Dir d = t->HasNormals() ? t->Normal(k) : gp_Dir(0, 0, 1);
          d.Transform(tr);
          if (reversed)
            d.Reverse();
          nrm.push_back(float(d.X()));
          nrm.push_back(float(d.Y()));
          nrm.push_back(float(d.Z()));
        }
        for (int k = 1; k <= t->NbTriangles(); ++k)
        {
          int a, b, c;
          t->Triangle(k).Get(a, b, c);
          if (flip)
            std::swap(b, c);
          idx.push_back(first + uint32_t(a - 1));
          idx.push_back(first + uint32_t(b - 1));
          idx.push_back(first + uint32_t(c - 1));
        }
        if (myNeeded.count(key) != 0)
          marked.push_back({key, {first_tri, uint32_t(idx.size() / 3) - first_tri}});
      }
      if (idx.empty())
        continue;
      for (const auto& mk : marked)
        myFaceTris[mk.first].push_back({myMeshes, mk.second[0], mk.second[1]});
      ++myMeshes;
      Body body;
      body.b.reserve(41 + pos.size() * 8 + idx.size() * 4);
      body.put<uint32_t>(id);
      body.u8(has ? 1 : 0);
      for (float v : rgba)
        body.put<float>(v);
      body.put<uint32_t>(uint32_t(pos.size() / 3));
      body.put<uint32_t>(uint32_t(idx.size() / 3));
      for (float v : pos)
        body.put<float>(v);
      for (float v : nrm)
        body.put<float>(v);
      for (uint32_t v : idx)
        body.put<uint32_t>(v);
      myOut.record(3, body.b);
    }
  }

  const SpecularOcctParams& myParams;
  Out&                      myOut;
  std::vector<Pmi>          myPmi;
  std::vector<View>         myViews;
  // The PMI record of each XDE label read (duplicates map to the record kept).
  std::map<std::string, uint32_t> myPmiOfLabel;
  // Faces some PMI applies to, (part, face), and their triangles as written: per MESH
  // record (counted from 0), its first triangle and how many.
  std::set<std::pair<int, int>>                                       myNeeded;
  std::map<std::pair<int, int>, std::vector<std::array<uint32_t, 3>>> myFaceTris;
  uint32_t                                                            myMeshes = 0;
  // Each face's parts and indices, by its geometry (made when PMI needs it).
  std::map<const TopoDS_TShape*, std::vector<std::pair<int, int>>> myFaceOf;
  std::vector<Node>         myNodes;
  std::vector<Part>         myParts;
  std::vector<std::string>  myNotes;
  Bnd_Box                   myBox;
  double                    myDeflection   = 0.0;
  uint32_t                  myFacesWithout = 0;
};

Handle(TDocStd_Document) new_document()
{
  Handle(TDocStd_Document) doc;
  XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", doc);
  if (doc.IsNull())
    fail(INTERNAL, "OpenCASCADE could not create a document");
  return doc;
}

void read_step(const unsigned char* data, size_t len, Model& m)
{
  STEPCAFControl_Reader r;
  r.SetColorMode(true);
  r.SetNameMode(true);
  r.SetLayerMode(false);
  r.SetPropsMode(false);
  r.SetGDTMode(true);
  r.SetMatMode(false);
  r.SetViewMode(true);
  MemBuf       buf(data, len);
  std::istream in(&buf);
  const IFSelect_ReturnStatus st = r.ReadStream("model.stp", in);
  if (st != IFSelect_RetDone)
    fail(MALFORMED, "OpenCASCADE could not read this STEP file (it is damaged or not STEP)");
  TColStd_SequenceOfAsciiString lengths, angles, solid_angles;
  r.ChangeReader().FileUnits(lengths, angles, solid_angles);
  if (lengths.Length() > 0)
    m.file_unit = lengths.First().ToCString();
  restore_magnitudes(r.ChangeReader().StepModel(), data, len);
  convert_qualified(r.ChangeReader().StepModel());
  m.doc = new_document();
  if (!r.Transfer(m.doc))
    fail(MALFORMED, "OpenCASCADE found no shapes it could read in this STEP file");
  m.cameras = camera_views(r.ChangeReader().StepModel());
}

#if defined(__linux__) || defined(__APPLE__)
// IGES is read by OCCT's C reader, which opens files by name or reads `stdin`: it gets the
// bytes as a stream on `stdin` (the decode host opens no files).
struct Cookie
{
  const unsigned char* data;
  size_t               len;
  size_t               at;
  bool                 closed;
};

size_t cookie_take(void* c, char* buf, size_t n)
{
  Cookie* k    = static_cast<Cookie*>(c);
  size_t  left = k->len - k->at;
  size_t  take = n < left ? n : left;
  std::memcpy(buf, k->data + k->at, take);
  k->at += take;
  return take;
}

#if defined(__linux__)
ssize_t cookie_read(void* c, char* buf, size_t n)
{
  return ssize_t(cookie_take(c, buf, n));
}
#else
int cookie_read(void* c, char* buf, int n)
{
  return n <= 0 ? 0 : int(cookie_take(c, buf, size_t(n)));
}
#endif

int cookie_close(void* c)
{
  static_cast<Cookie*>(c)->closed = true;
  return 0;
}

FILE* open_cookie(Cookie& k)
{
#if defined(__linux__)
  cookie_io_functions_t io;
  io.read  = cookie_read;
  io.write = nullptr;
  io.seek  = nullptr;
  io.close = cookie_close;
  return fopencookie(&k, "r", io);
#else
  return funopen(&k, cookie_read, nullptr, nullptr, cookie_close);
#endif
}

void read_iges(const unsigned char* data, size_t len, Model& m)
{
  IGESCAFControl_Reader r;
  r.SetColorMode(true);
  r.SetNameMode(true);
  r.SetLayerMode(false);
  Cookie k{data, len, 0, false};
  FILE*  f = open_cookie(k);
  if (f == nullptr)
    fail(INTERNAL, "no stream for the IGES reader");
  FILE* saved = stdin;
  stdin       = f;
  IFSelect_ReturnStatus st;
  try
  {
    st = r.ReadFile("");
  }
  catch (...)
  {
    stdin = saved;
    if (!k.closed)
      fclose(f);
    throw;
  }
  stdin = saved;
  if (!k.closed)
    fclose(f);
  if (st != IFSelect_RetDone)
    fail(MALFORMED, "OpenCASCADE could not read this IGES file (it is damaged or not IGES)");
  Handle(IGESData_IGESModel) model = Handle(IGESData_IGESModel)::DownCast(r.Model());
  if (!model.IsNull())
  {
    Handle(TCollection_HAsciiString) u = model->GlobalSection().UnitName();
    if (!u.IsNull())
      m.file_unit = u->ToCString();
  }
  m.doc = new_document();
  if (!r.Transfer(m.doc))
    fail(MALFORMED, "OpenCASCADE found no shapes it could read in this IGES file");
}
#else
void read_iges(const unsigned char*, size_t, Model&)
{
  fail(UNSUPPORTED, "IGES files cannot be opened with OpenCASCADE on Windows yet");
}
#endif

void copy_error(char* err, size_t cap, const std::string& s)
{
  if (err == nullptr || cap == 0)
    return;
  size_t n = s.size() < cap - 1 ? s.size() : cap - 1;
  std::memcpy(err, s.data(), n);
  err[n] = '\0';
}

} // namespace

SPECULAR_EXPORT uint32_t specular_occt_abi(void)
{
  return ABI;
}

// Loads OCCT's readers and quiets its messages (the host's stdout may be the viewer's pipe).
// Called once, before the sandbox is applied.
SPECULAR_EXPORT int specular_occt_init(int threads, char* version, size_t version_cap)
{
  try
  {
    Message::DefaultMessenger()->ChangePrinters().Clear();
    STEPCAFControl_Controller::Init();
    IGESControl_Controller::Init();
    XCAFApp_Application::GetApplication();
    OSD_ThreadPool::DefaultPool(threads > 0 ? threads : -1);
    copy_error(version, version_cap, OCC_VERSION_COMPLETE);
    return OK;
  }
  catch (const Standard_Failure& e)
  {
    copy_error(version, version_cap, e.GetMessageString());
  }
  catch (...)
  {
    copy_error(version, version_cap, "OpenCASCADE failed to start");
  }
  return INTERNAL;
}

SPECULAR_EXPORT int specular_occt_read(const unsigned char*      data,
                                       size_t                    len,
                                       int                       format,
                                       const SpecularOcctParams* params,
                                       SpecularOcctWrite         write,
                                       void*                     ctx,
                                       char*                     err,
                                       size_t                    err_cap)
{
  Model m;
  try
  {
    if (params == nullptr || write == nullptr || (data == nullptr && len > 0))
      fail(INTERNAL, "bad arguments");
    if (format == 1)
      read_step(data, len, m);
    else if (format == 2)
      read_iges(data, len, m);
    else
      fail(UNSUPPORTED, "unknown CAD format");
    Out    out(write, ctx, params->max_bytes);
    Reader reader(*params, out);
    reader.run(m);
    return OK;
  }
  catch (const Failure& f)
  {
    copy_error(err, err_cap, f.message);
    return f.status;
  }
  catch (const std::bad_alloc&)
  {
    copy_error(err, err_cap, "OpenCASCADE ran out of memory on this model");
    return TOO_LARGE;
  }
  catch (const Standard_OutOfMemory&)
  {
    copy_error(err, err_cap, "OpenCASCADE ran out of memory on this model");
    return TOO_LARGE;
  }
  catch (const Standard_Failure& e)
  {
    copy_error(err,
               err_cap,
               std::string("OpenCASCADE failed: ") + e.DynamicType()->Name() + ": "
                 + e.GetMessageString());
    return MALFORMED;
  }
  catch (...)
  {
    copy_error(err, err_cap, "OpenCASCADE failed on this file");
    return INTERNAL;
  }
}
