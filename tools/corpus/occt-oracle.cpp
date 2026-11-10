// OpenCASCADE oracle for the STEP/IGES decoders: generates test models and measures files with
// OCCT's own reading (exact B-rep), for specular-decode-host's tests/occt.rs.
//
// Build against the OCCT the bridge uses (a dev tool, never shipped):
//   c++ -std=c++17 -O1 -I$OCCT/include/opencascade tools/corpus/occt-oracle.cpp -o occt-oracle \
//     -L$OCCT/lib -lTKDESTEP -lTKDEIGES -lTKDE -lTKXCAF -lTKVCAF -lTKLCAF -lTKCAF -lTKCDF \
//     -lTKXSBase -lTKMesh -lTKBO -lTKPrim -lTKShHealing -lTKTopAlgo -lTKGeomAlgo -lTKBRep \
//     -lTKGeomBase -lTKG3d -lTKG2d -lTKMath -lTKernel -Wl,-rpath,$OCCT/lib
//
//   occt-oracle generate DIR      writes the generated models into DIR
//   occt-oracle measure FILE...   one TSV line per file:
//     file, leaf instances, faces (all instances), solids, box (6, document units),
//     area, volume (exact, all instances), unit (metres)
//   occt-oracle styles FILE      the styles XCAF gives each free shape's sub-shapes
//   occt-oracle mesh FILE DEFLECTION ANGLE
//     triangles of every instance meshed with OCCT's BRepMesh at that absolute deflection

#include <BRepAlgoAPI_Cut.hxx>
#include <BRepBndLib.hxx>
#include <BRepBuilderAPI_Transform.hxx>
#include <BRepGProp.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <BRepPrimAPI_MakeBox.hxx>
#include <BRepPrimAPI_MakeCone.hxx>
#include <BRepPrimAPI_MakeCylinder.hxx>
#include <BRepPrimAPI_MakeSphere.hxx>
#include <BRepPrimAPI_MakeTorus.hxx>
#include <BRep_Builder.hxx>
#include <BRep_Tool.hxx>
#include <Bnd_Box.hxx>
#include <GProp_GProps.hxx>
#include <IGESCAFControl_Reader.hxx>
#include <IGESCAFControl_Writer.hxx>
#include <IGESControl_Controller.hxx>
#include <Interface_Static.hxx>
#include <Message.hxx>
#include <Message_Messenger.hxx>
#include <Poly_Triangulation.hxx>
#include <STEPCAFControl_Controller.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <STEPCAFControl_Writer.hxx>
#include <TDataStd_Name.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <TopoDS.hxx>
#include <TopoDS_Compound.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_ColorTool.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <XCAFPrs.hxx>
#include <XCAFPrs_DocumentExplorer.hxx>
#include <XCAFPrs_IndexedDataMapOfShapeStyle.hxx>
#include <gp_Ax2.hxx>
#include <gp_Trsf.hxx>

#include <cstdio>
#include <cstring>
#include <string>
#include <vector>

namespace
{

Handle(TDocStd_Document) new_doc()
{
  Handle(TDocStd_Document) doc;
  XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", doc);
  return doc;
}

void name(const TDF_Label& l, const char* n)
{
  TDataStd_Name::Set(l, TCollection_ExtendedString(n, true));
}

Quantity_Color rgb(double r, double g, double b)
{
  return Quantity_Color(r, g, b, Quantity_TOC_RGB);
}

TopLoc_Location at(double x, double y, double z, double angle = 0.0, bool mirror = false)
{
  gp_Trsf t;
  t.SetRotation(gp_Ax1(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), angle);
  gp_Trsf m;
  if (mirror)
    m.SetMirror(gp_Ax2(gp_Pnt(0, 0, 0), gp_Dir(1, 0, 0)));
  gp_Trsf tr;
  tr.SetTranslation(gp_Vec(x, y, z));
  return TopLoc_Location(tr * t * m);
}

bool write_step(const Handle(TDocStd_Document)& doc, const std::string& path, const char* schema)
{
  Interface_Static::SetCVal("write.step.schema", schema);
  STEPCAFControl_Writer w;
  w.SetColorMode(true);
  w.SetNameMode(true);
  if (!w.Transfer(doc, STEPControl_AsIs))
    return false;
  return w.Write(path.c_str()) == IFSelect_RetDone;
}

bool write_iges(const Handle(TDocStd_Document)& doc, const std::string& path, int brep)
{
  Interface_Static::SetIVal("write.iges.brep.mode", brep);
  IGESCAFControl_Writer w;
  w.SetColorMode(true);
  w.SetNameMode(true);
  if (!w.Transfer(doc))
    return false;
  return w.Write(path.c_str());
}

// A part with a hole: a block minus a cylinder.
TopoDS_Shape bracket()
{
  TopoDS_Shape block = BRepPrimAPI_MakeBox(40.0, 20.0, 10.0).Shape();
  TopoDS_Shape hole =
    BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(20, 10, -1), gp_Dir(0, 0, 1)), 5.0, 12.0).Shape();
  return BRepAlgoAPI_Cut(block, hole).Shape();
}

int generate(const std::string& dir)
{
  int bad = 0;
  auto check = [&](bool ok, const std::string& f) {
    std::printf("%s %s\n", ok ? "wrote" : "FAILED", f.c_str());
    bad += ok ? 0 : 1;
  };
  // One coloured box per schema.
  const char* schemas[][2] = {{"AP203", "box_ap203.step"},
                              {"AP214IS", "box_ap214.step"},
                              {"AP242DIS", "box_ap242.step"}};
  for (auto& s : schemas)
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) ct  = XCAFDoc_DocumentTool::ColorTool(doc->Main());
    TDF_Label l = st->AddShape(BRepPrimAPI_MakeBox(10.0, 20.0, 30.0).Shape(), false);
    name(l, "Box");
    ct->SetColor(l, rgb(0.8, 0.1, 0.1), XCAFDoc_ColorSurf);
    check(write_step(doc, dir + "/" + s[1], s[0]), s[1]);
  }
  // Curved solids, one per file.
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    name(st->AddShape(BRepPrimAPI_MakeSphere(gp_Pnt(5, 5, 5), 25.0).Shape(), false), "Ball");
    check(write_step(doc, dir + "/sphere.step", "AP214IS"), "sphere.step");
  }
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    name(st->AddShape(BRepPrimAPI_MakeTorus(40.0, 8.0).Shape(), false), "Ring");
    check(write_step(doc, dir + "/torus_ap242.step", "AP242DIS"), "torus_ap242.step");
  }
  // An assembly: a bracket placed three times (one mirrored), a cone with a red face.
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) ct  = XCAFDoc_DocumentTool::ColorTool(doc->Main());
    TDF_Label asmb = st->NewShape();
    name(asmb, "Fixture");
    TDF_Label br = st->AddShape(bracket(), false);
    name(br, "Bracket");
    ct->SetColor(br, rgb(0.2, 0.4, 0.9), XCAFDoc_ColorSurf);
    TopoDS_Shape cone =
      BRepPrimAPI_MakeCone(gp_Ax2(gp_Pnt(0, 0, 0), gp_Dir(0, 0, 1)), 10.0, 2.0, 25.0).Shape();
    TDF_Label cl = st->AddShape(cone, false);
    name(cl, "Cone");
    ct->SetColor(cl, rgb(0.9, 0.9, 0.2), XCAFDoc_ColorSurf);
    // The first face of the cone in red.
    TopExp_Explorer fe(cone, TopAbs_FACE);
    TDF_Label       face = st->AddSubShape(cl, fe.Current());
    if (!face.IsNull())
      ct->SetColor(face, rgb(1.0, 0.0, 0.0), XCAFDoc_ColorSurf);
    name(st->AddComponent(asmb, br, at(0, 0, 0)), "Bracket left");
    name(st->AddComponent(asmb, br, at(60, 0, 0, M_PI / 2)), "Bracket right");
    name(st->AddComponent(asmb, br, at(-60, 0, 0, 0.0, true)), "Bracket mirrored");
    name(st->AddComponent(asmb, cl, at(20, 40, 0)), "Cone 1");
    st->UpdateAssemblies();
    check(write_step(doc, dir + "/assembly.step", "AP214IS"), "assembly.step");
    check(write_iges(doc, dir + "/assembly_brep.igs", 1), "assembly_brep.igs");
  }
  // IGES faces (no B-rep entities): a torus and a cylinder, coloured.
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) ct  = XCAFDoc_DocumentTool::ColorTool(doc->Main());
    TDF_Label t = st->AddShape(BRepPrimAPI_MakeTorus(30.0, 6.0).Shape(), false);
    name(t, "Torus");
    ct->SetColor(t, rgb(0.1, 0.7, 0.2), XCAFDoc_ColorSurf);
    TDF_Label c = st->AddShape(
      BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, 20), gp_Dir(0, 0, 1)), 10.0, 30.0).Shape(),
      false);
    name(c, "Post");
    check(write_iges(doc, dir + "/faces.iges", 0), "faces.iges");
  }
  // A large assembly: 40 × 40 brackets with holes, each a separate instance of one part.
  {
    Handle(TDocStd_Document)  doc = new_doc();
    Handle(XCAFDoc_ShapeTool) st  = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
    Handle(XCAFDoc_ColorTool) ct  = XCAFDoc_DocumentTool::ColorTool(doc->Main());
    TDF_Label asmb = st->NewShape();
    name(asmb, "Grid");
    TopoDS_Shape part = BRepAlgoAPI_Cut(BRepPrimAPI_MakeSphere(20.0).Shape(),
                                        BRepPrimAPI_MakeCylinder(gp_Ax2(gp_Pnt(0, 0, -30),
                                                                        gp_Dir(0, 0, 1)),
                                                                 8.0,
                                                                 60.0)
                                          .Shape())
                          .Shape();
    TDF_Label p = st->AddShape(part, false);
    name(p, "Bead");
    ct->SetColor(p, rgb(0.6, 0.6, 0.6), XCAFDoc_ColorSurf);
    for (int i = 0; i < 40; ++i)
      for (int j = 0; j < 40; ++j)
        st->AddComponent(asmb, p, at(i * 50.0, j * 50.0, 0));
    st->UpdateAssemblies();
    check(write_step(doc, dir + "/grid_1600.step", "AP214IS"), "grid_1600.step");
  }
  return bad == 0 ? 0 : 1;
}

Handle(TDocStd_Document) read(const std::string& path)
{
  Handle(TDocStd_Document) doc = new_doc();
  const bool iges = path.size() > 4
                    && (path.compare(path.size() - 4, 4, ".igs") == 0
                        || path.compare(path.size() - 5, 5, ".iges") == 0);
  if (iges)
  {
    IGESCAFControl_Reader r;
    r.SetColorMode(true);
    r.SetNameMode(true);
    if (r.ReadFile(path.c_str()) != IFSelect_RetDone || !r.Transfer(doc))
      return Handle(TDocStd_Document)();
  }
  else
  {
    STEPCAFControl_Reader r;
    r.SetColorMode(true);
    r.SetNameMode(true);
    if (r.ReadFile(path.c_str()) != IFSelect_RetDone || !r.Transfer(doc))
      return Handle(TDocStd_Document)();
  }
  return doc;
}

// Every leaf instance with faces, located.
std::vector<TopoDS_Shape> leaves(const Handle(TDocStd_Document)& doc)
{
  std::vector<TopoDS_Shape> out;
  for (XCAFPrs_DocumentExplorer ex(doc, XCAFPrs_DocumentExplorerFlags_OnlyLeafNodes); ex.More();
       ex.Next())
  {
    TopoDS_Shape s;
    // Leaves with surfaces only (a part of curves alone has nothing to show).
    if (!XCAFDoc_ShapeTool::GetShape(ex.Current().RefLabel, s) || s.IsNull()
        || !TopExp_Explorer(s, TopAbs_FACE).More())
      continue;
    out.push_back(s.Located(ex.Current().Location));
  }
  return out;
}

int measure(int argc, char** argv)
{
  for (int i = 2; i < argc; ++i)
  {
    Handle(TDocStd_Document) doc = read(argv[i]);
    if (doc.IsNull())
    {
      std::printf("%s\tFAILED\n", argv[i]);
      continue;
    }
    std::vector<TopoDS_Shape> shapes = leaves(doc);
    Bnd_Box                   box;
    double                    area = 0, volume = 0;
    long                      faces = 0, solids = 0;
    for (const TopoDS_Shape& s : shapes)
    {
      BRepBndLib::AddOptimal(s, box, false, false);
      GProp_GProps a, v;
      BRepGProp::SurfaceProperties(s, a);
      area += a.Mass();
      for (TopExp_Explorer e(s, TopAbs_SOLID); e.More(); e.Next())
      {
        BRepGProp::VolumeProperties(e.Current(), v);
        volume += v.Mass();
        ++solids;
      }
      for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
        ++faces;
    }
    double b[6] = {0, 0, 0, 0, 0, 0};
    if (!box.IsVoid())
      box.Get(b[0], b[1], b[2], b[3], b[4], b[5]);
    double unit = 0.001;
    XCAFDoc_DocumentTool::GetLengthUnit(doc, unit);
    std::printf("%s\t%zu\t%ld\t%ld\t%.6f\t%.6f\t%.6f\t%.6f\t%.6f\t%.6f\t%.6f\t%.6f\t%g\n",
                argv[i],
                shapes.size(),
                faces,
                solids,
                b[0],
                b[1],
                b[2],
                b[3],
                b[4],
                b[5],
                area,
                volume,
                unit);
  }
  return 0;
}

int mesh(const char* path, double deflection, double angle)
{
  Handle(TDocStd_Document) doc = read(path);
  if (doc.IsNull())
    return 1;
  long total = 0;
  for (const TopoDS_Shape& s : leaves(doc))
  {
    BRepMesh_IncrementalMesh(s, deflection, false, angle, true);
    for (TopExp_Explorer e(s, TopAbs_FACE); e.More(); e.Next())
    {
      TopLoc_Location            loc;
      Handle(Poly_Triangulation) t = BRep_Tool::Triangulation(TopoDS::Face(e.Current()), loc);
      if (!t.IsNull())
        total += t->NbTriangles();
    }
  }
  std::printf("%ld\n", total);
  return 0;
}

// The styles XCAF gives each free shape's sub-shapes (for debugging colour mapping).
int styles(const char* path)
{
  Handle(TDocStd_Document) doc = read(path);
  if (doc.IsNull())
    return 1;
  TDF_LabelSequence free;
  XCAFDoc_DocumentTool::ShapeTool(doc->Main())->GetFreeShapes(free);
  for (const TDF_Label& l : free)
  {
    XCAFPrs_IndexedDataMapOfShapeStyle map;
    XCAFPrs::CollectStyleSettings(l, TopLoc_Location(), map);
    for (XCAFPrs_IndexedDataMapOfShapeStyle::Iterator it(map); it.More(); it.Next())
    {
      double r = -1, g = -1, b = -1;
      if (it.Value().IsSetColorSurf())
        it.Value().GetColorSurf().Values(r, g, b, Quantity_TOC_RGB);
      int faces = 0;
      for (TopExp_Explorer e(it.Key(), TopAbs_FACE); e.More(); e.Next())
        ++faces;
      std::printf("type %d faces %d colour %.3f %.3f %.3f\n",
                  int(it.Key().ShapeType()),
                  faces,
                  r,
                  g,
                  b);
    }
  }
  return 0;
}

} // namespace

int main(int argc, char** argv)
{
  Message::DefaultMessenger()->ChangePrinters().Clear();
  STEPCAFControl_Controller::Init();
  IGESControl_Controller::Init();
  if (argc >= 3 && std::strcmp(argv[1], "generate") == 0)
    return generate(argv[2]);
  if (argc >= 3 && std::strcmp(argv[1], "measure") == 0)
    return measure(argc, argv);
  if (argc == 3 && std::strcmp(argv[1], "styles") == 0)
    return styles(argv[2]);
  if (argc == 5 && std::strcmp(argv[1], "mesh") == 0)
    return mesh(argv[2], std::atof(argv[3]), std::atof(argv[4]));
  std::fprintf(stderr, "usage: occt-oracle generate DIR | measure FILE... | mesh FILE D A\n");
  return 2;
}
