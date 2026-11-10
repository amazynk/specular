use super::*;

#[test]
fn known_is_sorted() {
    let k = interp::KNOWN;
    assert!(k.windows(2).all(|w| w[0] < w[1]));
}

#[test]
fn probe() {
    assert!(is_hpgl(b"IN;SP1;PU0,0;PD100,100;"));
    assert!(is_hpgl(b"in;sp1;pa0,0;"));
    assert!(is_hpgl(b"\x1b.(;\x1b.I81;;17:\x1b.N;19:IN;SP1;"));
    assert!(is_hpgl(
        b"\x1b%-12345X@PJL\r\n@PJL ENTER LANGUAGE=HPGL2\r\nBPIN;"
    ));
    assert!(is_hpgl(b"\x1bE\x1b&l1O\x1b%0BINSP1"));
    assert!(is_hpgl(b"BP;IN;PS10000,5000;"));
    assert!(is_hpgl(b"IN"));
    assert!(is_hpgl(b"PU;PA 10 20;PD 30 40;"));
    // From the hp2xx tests: a stray ETX first, run-together commands; an odd command
    // after good ones; an unquoted comment.
    assert!(is_hpgl(
        b"\x03INCA2IP0,0,10300,7650DI0,1SP2VSSP2PAPU6545,6440"
    ));
    assert!(is_hpgl(
        b"IN;\nPS5000,7000;\nSP1;\nPA1016,5080;\nPD:\nBZ1016,8128;"
    ));
    assert!(is_hpgl(
        b"CO Concentric circles;\nPU 500,500;\nCI50,1;\nPW0.4\n"
    ));
    assert!(!is_hpgl(b"CO Concentric circles;"));
    assert!(!is_hpgl(b"CO hello; and more prose;"));
    // Gnuplot scripts, prose, other printer jobs.
    assert!(!is_hpgl(b"set terminal png\nplot sin(x)\n"));
    assert!(!is_hpgl(b"# gnuplot\nplot 'data.dat' using 1:2\n"));
    assert!(!is_hpgl(b"plot \"x\" with lines"));
    assert!(!is_hpgl(b"Input file;"));
    assert!(!is_hpgl(b"\x1bE\x1b&l0OHello printer\x0c"));
    assert!(!is_hpgl(b"\x89PNG\r\n\x1a\n"));
    assert!(!is_hpgl(b""));
    assert!(!is_hpgl(b"IN;SPx;"));
    assert!(!is_hpgl(b"PA1..2;PD3;"));
}

#[test]
fn polyline_encoded() {
    // PE: pen 1, then (+100, +50) relative and an absolute (0, 0) pen up.
    fn enc(v: i64, out: &mut Vec<u8>) {
        let mut n = if v < 0 { ((-v) << 1) | 1 } else { v << 1 } as u64;
        loop {
            let d = (n & 63) as u8;
            n >>= 6;
            if n == 0 {
                out.push(191 + d);
                break;
            }
            out.push(63 + d);
        }
    }
    let mut b = b"IN;PA0,0;PE:".to_vec();
    enc(1, &mut b);
    enc(100, &mut b);
    enc(-50, &mut b);
    b.push(b'<');
    b.push(b'=');
    enc(7, &mut b);
    enc(9, &mut b);
    enc(10, &mut b);
    enc(0, &mut b);
    b.push(b';');
    let s = parse(&b, &Limits::default()).unwrap();
    let items = &s.sheets[0].items;
    assert_eq!(items.len(), 2, "{items:?}");
    let specular_vector::Shape::Stroke { path, .. } = &items[0].shape else {
        panic!()
    };
    let pts: Vec<_> = path.elements().to_vec();
    assert_eq!(
        pts,
        vec![
            kurbo::PathEl::MoveTo((0.0, 0.0).into()),
            kurbo::PathEl::LineTo((2.5, -1.25).into()),
        ]
    );
}
