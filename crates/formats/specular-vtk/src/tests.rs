use crate::*;

fn mesh(v: &Vtk) -> &MeshData {
    match &v.dataset {
        Dataset::Mesh(m) => m,
        Dataset::Grid(_) => panic!("a grid"),
    }
}

fn grid(v: &Vtk) -> &Grid {
    match &v.dataset {
        Dataset::Grid(g) => g,
        Dataset::Mesh(_) => panic!("a mesh"),
    }
}

#[test]
fn sniffs() {
    assert_eq!(sniff(b"# vtk DataFile Version 3.0\n"), Some(Kind::Legacy));
    assert_eq!(
        sniff(b"<?xml version=\"1.0\"?>\n<VTKFile type=\"ImageData\" version=\"0.1\">"),
        Some(Kind::ImageData)
    );
    assert_eq!(
        sniff(b"\xef\xbb\xbf<VTKFile type='PolyData'>"),
        Some(Kind::PolyData)
    );
    assert_eq!(sniff(b"<VTKFile type=\"PImageData\">"), None);
    assert!(sniff_other_xml(b"<VTKFile type=\"PImageData\">"));
    assert_eq!(sniff(b"<svg><VTKFile type=\"PolyData\">"), None);
    assert_eq!(sniff(b"PK\x03\x04"), None);
}

#[test]
fn legacy_ascii_points_and_mesh() {
    let f = b"# vtk DataFile Version 2.0\nhello\nASCII\nDATASET STRUCTURED_POINTS\nDIMENSIONS 3 2 1\nORIGIN 1 2 3\nSPACING 0.5 1 1\nPOINT_DATA 6\nSCALARS v int 1\nLOOKUP_TABLE default\n1 2 3 4 5 6\nCELL_DATA 2\nVECTORS w float\n1 0 0 0 1 0\n";
    let v = parse(f).unwrap();
    assert_eq!(v.title.as_deref(), Some("hello"));
    assert_eq!(v.version, "2.0");
    let g = grid(&v);
    assert_eq!(g.dims, [3, 2, 1]);
    assert_eq!(g.origin, [1.0, 2.0, 3.0]);
    assert_eq!(g.point_data[0].values, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
    assert_eq!(g.cell_data[0].components, 3);
    let f = b"# vtk DataFile Version 3.0\nq\nASCII\nDATASET POLYDATA\nPOINTS 5 float\n0 0 0 1 0 0 1 1 0 0 1 0 2 2 2\nVERTICES 1 2\n1 4\nPOLYGONS 2 9\n4 0 1 2 3\n3 0 1 4\nCELL_DATA 3\nSCALARS c double\nLOOKUP_TABLE default\n7 8 9\n";
    let v = parse(f).unwrap();
    let m = mesh(&v);
    assert_eq!(m.triangles.len(), 3);
    assert_eq!(m.triangle_cell, [1, 1, 2]);
    assert_eq!(m.vertices, 1);
    assert_eq!(m.cells, 3);
}

#[test]
fn legacy_binary_51() {
    let mut f =
        b"# vtk DataFile Version 5.1\nb\nBINARY\nDATASET UNSTRUCTURED_GRID\nPOINTS 4 float\n"
            .to_vec();
    for p in [[0f32, 0., 0.], [1., 0., 0.], [0., 1., 0.], [0., 0., 1.]] {
        for c in p {
            f.extend_from_slice(&c.to_be_bytes());
        }
    }
    f.extend_from_slice(b"\nCELLS 2 4\nOFFSETS vtktypeint64\n");
    for o in [0i64, 4] {
        f.extend_from_slice(&o.to_be_bytes());
    }
    f.extend_from_slice(b"\nCONNECTIVITY vtktypeint64\n");
    for o in [0i64, 1, 2, 3] {
        f.extend_from_slice(&o.to_be_bytes());
    }
    f.extend_from_slice(b"\nCELL_TYPES 1\n");
    f.extend_from_slice(&10i32.to_be_bytes());
    f.extend_from_slice(b"\n");
    let v = parse(&f).unwrap();
    assert_eq!(v.encoding, "binary");
    let m = mesh(&v);
    assert_eq!(m.triangles.len(), 4);
}

#[test]
fn xml_refusals() {
    let e = parse(b"<VTKFile type=\"ImageData\" compressor=\"vtkLZMADataCompressor\"><ImageData WholeExtent=\"0 0 0 0 0 0\"></ImageData></VTKFile>").unwrap_err();
    assert_eq!(e.to_string(), "VTK LZMA compression is not supported");
    let e = parse(b"<VTKFile type=\"PUnstructuredGrid\"></VTKFile>").unwrap_err();
    assert!(matches!(e, Error::Unsupported(_)));
}

#[test]
fn garbage_never_panics() {
    let base: &[&[u8]] = &[
        b"# vtk DataFile Version 3.0\nq\nBINARY\nDATASET POLYDATA\nPOINTS 3 float\n",
        b"# vtk DataFile Version 3.0\nq\nASCII\nDATASET STRUCTURED_POINTS\nDIMENSIONS 99999 99999 99999\nPOINT_DATA 999999999999\nSCALARS a float\n1 2",
        b"<VTKFile type=\"ImageData\" header_type=\"UInt64\" compressor=\"vtkZLibDataCompressor\"><ImageData WholeExtent=\"0 9 0 9 0 9\"><Piece Extent=\"0 9 0 9 0 9\"><PointData><DataArray type=\"Float32\" format=\"binary\">AAAAAAAAAAE=</DataArray></PointData></Piece></ImageData></VTKFile>",
        b"<VTKFile type=\"PolyData\"><PolyData><Piece NumberOfPoints=\"2\" NumberOfPolys=\"1\"><Points><DataArray type=\"Float32\" NumberOfComponents=\"3\" format=\"appended\" offset=\"0\"/></Points></Piece></PolyData><AppendedData encoding=\"raw\">_\x10\x00\x00\x00",
    ];
    for b in base {
        for cut in 0..b.len() {
            let _ = parse(&b[..cut]);
        }
        let mut x = b.to_vec();
        for i in 0..x.len() {
            let old = x[i];
            x[i] = old.wrapping_add(0x41);
            let _ = parse(&x);
            x[i] = old;
        }
    }
}
