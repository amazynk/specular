use super::*;
use specular_spectra::{SpectraFile, Technique};

const IR: &str = "##TITLE= test IR\r\n##JCAMP-DX= 4.24\r\n##DATA TYPE= INFRARED SPECTRUM\r\n\
##ORIGIN= lab\r\n##XUNITS= 1/CM\r\n##YUNITS= TRANSMITTANCE\r\n##XFACTOR= 1.0\r\n\
##YFACTOR= 0.001\r\n##FIRSTX= 4000\r\n##LASTX= 3994\r\n##NPOINTS= 7\r\n\
##XYDATA= (X++(Y..Y))\r\n4000 A00J00J00\r\n3997 C00%k00 $$ check\r\n3995 A00%J0\r\n##END=\r\n";

#[test]
fn sniffs() {
    assert!(sniff(IR.as_bytes()));
    assert!(sniff(b"\xEF\xBB\xBF\n$$ hi\n  ##title=x\n"));
    assert!(!sniff(b"##JCAMP-DX=5\n##TITLE=x"));
    assert!(!sniff(b"\x89PNG\r\n"));
}

#[test]
fn reads_an_asdf_ir_spectrum() {
    let s = parse(IR.as_bytes()).expect("parses");
    assert_eq!(s.page_count(), 1);
    let p = &s.pages[0];
    assert_eq!(p.x.title, "Wavenumber");
    assert!(p.x.reversed);
    assert_eq!(p.y.title, "Transmittance");
    let t = &p.traces[0];
    assert_eq!(
        t.x,
        vec![4000.0, 3999.0, 3998.0, 3997.0, 3996.0, 3995.0, 3994.0]
    );
    let want = [0.1, 0.2, 0.3, 0.3, 0.1, 0.1, 0.11];
    for (a, b) in t.y.iter().zip(want) {
        assert!((a - b).abs() < 1e-12, "{:?}", t.y);
    }
    assert_eq!(s.facts.technique, Technique::Infrared);
    assert_eq!(s.facts.origin.as_deref(), Some("lab"));
    assert_eq!(s.facts.points, 7);
    assert!(s.facts.warnings.is_empty(), "{:?}", s.facts.warnings);
}

#[test]
fn reads_link_blocks_and_peak_tables() {
    let f = "##TITLE= link\n##JCAMP-DX= 5.01\n##DATA TYPE= LINK\n##BLOCKS= 2\n\
##TITLE= ms\n##DATA TYPE= MASS SPECTRUM\n##XUNITS= M/Z\n##YUNITS= RELATIVE ABUNDANCE\n\
##PEAK TABLE= (XY..XY)\n15,100 27,50; 29,999\n##END=\n\
##TITLE= nmr\n##DATA TYPE= NMR SPECTRUM\n##.OBSERVE FREQUENCY= 100\n##XUNITS= HZ\n\
##FIRSTX= 1000\n##LASTX= 0\n##NPOINTS= 3\n##XYDATA= (X++(Y..Y))\n1000 1 2 3\n##END=\n##END=\n";
    let s = parse(f.as_bytes()).expect("parses");
    assert_eq!(s.pages.len(), 2);
    let ms = &s.pages[0];
    assert!(ms.traces[0].sticks);
    assert_eq!(ms.traces[0].x, vec![15.0, 27.0, 29.0]);
    assert_eq!(ms.peaks[0].x, 29.0);
    assert_eq!(ms.peaks[0].label, "29");
    let nmr = &s.pages[1];
    assert_eq!(nmr.x.unit, "ppm");
    assert_eq!(nmr.traces[0].x, vec![10.0, 5.0, 0.0]);
    assert_eq!(s.facts.title.as_deref(), Some("link"));
}

#[test]
fn reads_ntuples_real_part() {
    let f = "##TITLE= nt\n##JCAMP-DX= 5.0\n##DATA TYPE= NMR SPECTRUM\n##DATA CLASS= NTUPLES\n\
##.OBSERVE FREQUENCY= 10\n##NTUPLES= NMR SPECTRUM\n\
##VAR_NAME= FREQUENCY, SPECTRUM/REAL, SPECTRUM/IMAG, PAGE NUMBER\n##SYMBOL= X, R, I, N\n\
##VAR_DIM= 4, 4, 4, 2\n##UNITS= HZ, ARBITRARY UNITS, ARBITRARY UNITS,\n\
##FIRST= 30, 1, 1, 1\n##LAST= 0, 4, 4, 2\n##FACTOR= 1, 2, 1, 1\n\
##PAGE= N=1\n##DATA TABLE= (X++(R..R)), XYDATA\n30 1 2 3 4\n\
##PAGE= N=2\n##DATA TABLE= (X++(I..I)), XYDATA\n30 1 1 1 1\n##END NTUPLES= NMR SPECTRUM\n##END=\n";
    let s = parse(f.as_bytes()).expect("parses");
    assert_eq!(s.pages.len(), 1);
    let p = &s.pages[0];
    assert_eq!(p.traces[0].y, vec![2.0, 4.0, 6.0, 8.0]);
    assert_eq!(p.traces[0].x, vec![3.0, 2.0, 1.0, 0.0]);
    assert!(p.notes.iter().any(|n| n.contains("imaginary")));
}

#[test]
fn refuses_what_holds_no_spectrum() {
    let cs = "##TITLE= benzene\n##JCAMP-CS= 3.7\n##MOLFORM= C 6 H 6\n##END=\n";
    match parse(cs.as_bytes()) {
        Err(Error::Unsupported(m)) => assert!(m.contains("JCAMP-CS"), "{m}"),
        r => panic!("{r:?}"),
    }
    assert!(matches!(
        parse(b"##TITLE= empty\n##END=\n"),
        Err(Error::Unsupported(_))
    ));
    assert!(matches!(parse(b"hello"), Err(Error::NotThisFormat)));
}

#[test]
fn warns_about_damage() {
    let f = "##TITLE= t\n##FIRSTX= 0\n##LASTX= 4\n##NPOINTS= 6\n##XYDATA= (X++(Y..Y))\n\
0 A0J0J0\n2 C5J0\n";
    let s = parse(f.as_bytes()).expect("parses");
    let w = s.facts.warnings.join(" | ");
    assert!(w.contains("Y-check"), "{w}");
    assert!(w.contains("6 points but holds 4"), "{w}");
    assert!(w.contains("##END"), "{w}");
}
