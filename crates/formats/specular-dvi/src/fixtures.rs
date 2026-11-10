//! A generated DVI file for tests.

fn u32b(v: u32) -> [u8; 4] {
    v.to_be_bytes()
}

/// A two-page DVI: page 1 sets "AB" in font 0 and a rule; page 2 puts one character.
pub fn sample() -> Vec<u8> {
    let mut d = vec![247, 2];
    d.extend_from_slice(&u32b(25_400_000));
    d.extend_from_slice(&u32b(473_628_672));
    d.extend_from_slice(&u32b(1000));
    let c = b" TeX output 2026.11.07";
    d.push(c.len() as u8);
    d.extend_from_slice(c);
    let fnt_def = |d: &mut Vec<u8>| {
        d.extend_from_slice(&[243, 0]);
        d.extend_from_slice(&u32b(0x1234));
        d.extend_from_slice(&u32b(655_360));
        d.extend_from_slice(&u32b(655_360));
        d.extend_from_slice(&[0, 5]);
        d.extend_from_slice(b"cmr10");
    };
    let bop = |d: &mut Vec<u8>, n: i32, prev: i32| {
        d.push(139);
        d.extend_from_slice(&n.to_be_bytes());
        d.extend_from_slice(&[0; 36]);
        d.extend_from_slice(&prev.to_be_bytes());
    };
    let p1 = d.len() as i32;
    bop(&mut d, 1, -1);
    fnt_def(&mut d);
    d.push(171); // fnt_num_0
    d.push(141); // push
    d.extend_from_slice(&[157, 100]); // down1 100
    d.extend_from_slice(b"AB");
    d.push(142); // pop
    d.extend_from_slice(&[132]);
    d.extend_from_slice(&u32b(1000));
    d.extend_from_slice(&u32b(50_000));
    d.extend_from_slice(&[149, 0, 10]); // w2 = 10
    d.push(147); // w0
    d.push(140);
    let p2 = d.len() as i32;
    bop(&mut d, 2, p1);
    d.push(171);
    d.extend_from_slice(&[133, b'C']); // put1
    d.push(140);
    let post = d.len() as u32;
    d.push(248);
    d.extend_from_slice(&p2.to_be_bytes());
    d.extend_from_slice(&u32b(25_400_000));
    d.extend_from_slice(&u32b(473_628_672));
    d.extend_from_slice(&u32b(1000));
    d.extend_from_slice(&u32b(1_000_000));
    d.extend_from_slice(&u32b(2_000_000));
    d.extend_from_slice(&[0, 3, 0, 2]);
    fnt_def(&mut d);
    d.push(249);
    d.extend_from_slice(&post.to_be_bytes());
    d.push(2);
    d.extend_from_slice(&[223; 4]);
    d
}
