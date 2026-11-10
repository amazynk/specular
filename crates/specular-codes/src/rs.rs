//! Reed–Solomon error correction over GF(2^m): syndromes, Berlekamp–Massey for the error
//! locator, Chien search for the positions and Forney for the values. QR codes use the
//! field polynomial x⁸+x⁴+x³+x²+1 (0x11D) with generator roots α⁰…; Data Matrix 0x12D with
//! roots α¹…; Aztec GF(16), GF(64), GF(256), GF(1024) and GF(4096), roots α¹….

/// A field GF(2^m) (m up to 12) by its polynomial, with log and antilog tables.
pub(crate) struct Field {
    exp: Vec<u16>,
    log: Vec<u16>,
    /// The multiplicative group's order, 2^m - 1.
    order: usize,
    /// The power of α of the generator's first root.
    first_root: usize,
}

impl Field {
    /// The field of `poly` (its degree is m), generator roots from α^`first_root`.
    pub(crate) fn new(poly: u32, first_root: usize) -> Field {
        let m = 31 - poly.leading_zeros();
        let size = 1usize << m;
        let order = size - 1;
        let mut exp = vec![0u16; 2 * order + 2];
        let mut log = vec![0u16; size];
        let mut x: u32 = 1;
        for i in 0..order {
            exp[i] = x as u16;
            log[x as usize] = i as u16;
            x <<= 1;
            if x & size as u32 != 0 {
                x ^= poly;
            }
        }
        for i in order..exp.len() {
            exp[i] = exp[i - order];
        }
        Field {
            exp,
            log,
            order,
            first_root,
        }
    }

    fn mul(&self, a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[usize::from(self.log[usize::from(a)]) + usize::from(self.log[usize::from(b)])]
        }
    }

    fn div(&self, a: u16, b: u16) -> u16 {
        debug_assert!(b != 0);
        if a == 0 {
            0
        } else {
            self.exp[usize::from(self.log[usize::from(a)]) + self.order
                - usize::from(self.log[usize::from(b)])]
        }
    }

    /// α^i.
    fn pow(&self, i: usize) -> u16 {
        self.exp[i % self.order]
    }

    /// Evaluates a polynomial (highest power first) at x.
    fn eval(&self, p: &[u16], x: u16) -> u16 {
        p.iter().fold(0u16, |acc, &c| self.mul(acc, x) ^ c)
    }

    /// As [`Field::correct_words`], for bytes (fields up to GF(256)).
    pub(crate) fn correct(&self, block: &mut [u8], ecc: usize) -> Option<usize> {
        let mut w: Vec<u16> = block.iter().map(|&b| u16::from(b)).collect();
        let n = self.correct_words(&mut w, ecc)?;
        for (b, &v) in block.iter_mut().zip(&w) {
            *b = v as u8;
        }
        Some(n)
    }

    /// Corrects `block` (data then check words, the first word the highest power) in place
    /// with `ecc` check words. Returns the number of corrected words, or `None` when there
    /// are more errors than the code can fix.
    pub(crate) fn correct_words(&self, block: &mut [u16], ecc: usize) -> Option<usize> {
        let n = block.len();
        let order = self.order;
        if ecc == 0 || n > order {
            return Some(0);
        }
        // syndromes S_j = r(α^(first_root + j))
        let synd: Vec<u16> = (0..ecc)
            .map(|j| self.eval(block, self.pow(self.first_root + j)))
            .collect();
        if synd.iter().all(|&s| s == 0) {
            return Some(0);
        }
        // Berlekamp–Massey: the error locator Λ(x), lowest power first
        let mut lambda = vec![1u16];
        let mut prev = vec![1u16];
        let mut l = 0usize;
        let mut m = 1usize;
        let mut b = 1u16;
        for k in 0..ecc {
            let mut d = synd[k];
            for i in 1..=l.min(lambda.len() - 1) {
                d ^= self.mul(lambda[i], synd[k - i]);
            }
            if d == 0 {
                m += 1;
                continue;
            }
            let coef = self.div(d, b);
            let mut next = lambda.clone();
            if next.len() < prev.len() + m {
                next.resize(prev.len() + m, 0);
            }
            for (i, &p) in prev.iter().enumerate() {
                next[i + m] ^= self.mul(coef, p);
            }
            if 2 * l <= k {
                l = k + 1 - l;
                prev = lambda;
                b = d;
                m = 1;
            } else {
                m += 1;
            }
            lambda = next;
        }
        while lambda.len() > 1 && *lambda.last().unwrap() == 0 {
            lambda.pop();
        }
        let errors = lambda.len() - 1;
        if errors == 0 || errors > ecc / 2 || errors != l {
            return None;
        }
        // Chien search: position p (power of x, from the end) is an error when Λ(α^-p) = 0
        let mut positions = Vec::with_capacity(errors);
        for p in 0..n {
            let x_inv = self.pow(order - (p % order));
            let v = lambda
                .iter()
                .rev()
                .fold(0u16, |acc, &c| self.mul(acc, x_inv) ^ c);
            if v == 0 {
                positions.push(p);
            }
        }
        if positions.len() != errors {
            return None;
        }
        // Ω(x) = S(x)Λ(x) mod x^ecc, lowest power first
        let mut omega = vec![0u16; ecc];
        for (i, &s) in synd.iter().enumerate() {
            for (j, &c) in lambda.iter().enumerate() {
                if i + j < ecc {
                    omega[i + j] ^= self.mul(s, c);
                }
            }
        }
        // Forney: e = X^(1-first_root) Ω(X⁻¹) / Λ'(X⁻¹)
        for &p in &positions {
            let x = self.pow(p);
            let x_inv = self.pow(order - (p % order));
            let om = omega
                .iter()
                .rev()
                .fold(0u16, |acc, &c| self.mul(acc, x_inv) ^ c);
            // formal derivative: odd terms
            let mut dl = 0u16;
            let mut xp = 1u16;
            let x_inv2 = self.mul(x_inv, x_inv);
            for i in (1..lambda.len()).step_by(2) {
                dl ^= self.mul(lambda[i], xp);
                xp = self.mul(xp, x_inv2);
            }
            if dl == 0 {
                return None;
            }
            let mut e = self.div(om, dl);
            // X^(1 - first_root)
            let k = (1 + order - self.first_root % order) % order;
            for _ in 0..k {
                e = self.mul(e, x);
            }
            block[n - 1 - p] ^= e;
        }
        // check
        if (0..ecc).any(|j| self.eval(block, self.pow(self.first_root + j)) != 0) {
            return None;
        }
        Some(errors)
    }

    /// The check bytes for `data` (to make test symbols).
    #[cfg(test)]
    pub(crate) fn encode(&self, data: &[u8], ecc: usize) -> Vec<u8> {
        let d: Vec<u16> = data.iter().map(|&b| u16::from(b)).collect();
        self.encode_words(&d, ecc)
            .iter()
            .map(|&v| v as u8)
            .collect()
    }

    /// The check words for `data` (to make test symbols).
    #[cfg(test)]
    pub(crate) fn encode_words(&self, data: &[u16], ecc: usize) -> Vec<u16> {
        // generator Π (x - α^(first_root + i)), highest power first
        let mut g = vec![1u16];
        for i in 0..ecc {
            let r = self.pow(self.first_root + i);
            let mut ng = vec![0u16; g.len() + 1];
            for (j, &c) in g.iter().enumerate() {
                ng[j] ^= c;
                ng[j + 1] ^= self.mul(c, r);
            }
            g = ng;
        }
        let mut rem = vec![0u16; ecc];
        for &d in data {
            let f = d ^ rem[0];
            rem.remove(0);
            rem.push(0);
            for i in 0..ecc {
                rem[i] ^= self.mul(g[i + 1], f);
            }
        }
        rem
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roundtrip(f: &Field) {
        let data: Vec<u8> = (0..40u8)
            .map(|i| i.wrapping_mul(37).wrapping_add(11))
            .collect();
        for ecc in [10usize, 18, 30] {
            let mut block = data.clone();
            block.extend(f.encode(&data, ecc));
            let clean = block.clone();
            assert_eq!(f.correct(&mut block.clone(), ecc), Some(0));
            for errs in 1..=ecc / 2 {
                let mut b = clean.clone();
                for e in 0..errs {
                    let at = (e * 3 + 1) % b.len();
                    b[at] ^= 0x5A ^ e as u8;
                }
                assert_eq!(f.correct(&mut b, ecc), Some(errs), "ecc {ecc} errs {errs}");
                assert_eq!(b, clean);
            }
            let mut b = clean.clone();
            let len = b.len();
            for e in 0..ecc / 2 + 3 {
                b[(e * 2 + 1) % len] ^= 0xFF;
            }
            let fixed = f.correct(&mut b, ecc).is_some();
            assert!(!(fixed && b == clean));
        }
    }

    #[test]
    fn qr_field_corrects_up_to_half_the_check_bytes() {
        roundtrip(&Field::new(0x11D, 0));
    }

    #[test]
    fn data_matrix_field_corrects_too() {
        roundtrip(&Field::new(0x12D, 1));
    }

    #[test]
    fn wide_and_narrow_fields_correct() {
        for (poly, n) in [(0x13u32, 15usize), (0x43, 60), (0x409, 300), (0x1069, 900)] {
            let f = Field::new(poly, 1);
            let max = (f.order + 1) as u16;
            let data: Vec<u16> = (0..n * 2 / 3).map(|i| (i as u16 * 37 + 11) % max).collect();
            let ecc = n - data.len();
            let mut block = data.clone();
            block.extend(f.encode_words(&data, ecc));
            let clean = block.clone();
            for e in 0..ecc / 2 {
                block[(e * 7 + 2) % n] ^= 1 + (e as u16 % (max - 1));
            }
            assert_eq!(f.correct_words(&mut block, ecc), Some(ecc / 2), "{poly:x}");
            assert_eq!(block, clean);
        }
    }
}
