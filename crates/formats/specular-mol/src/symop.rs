//! Crystallographic symmetry operators in the `x,y,z` notation (`-x+1/2,y,-z`,
//! `x-y,x,z+1/3`, `1/2+X`, `0.5-y`), acting on fractional coordinates.

/// `f' = rot · f + tr`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymOp {
    pub rot: [[f64; 3]; 3],
    pub tr: [f64; 3],
}

impl SymOp {
    pub const IDENTITY: SymOp = SymOp {
        rot: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        tr: [0.0; 3],
    };

    pub fn apply(&self, f: [f64; 3]) -> [f64; 3] {
        [0, 1, 2].map(|r| {
            self.rot[r][0] * f[0] + self.rot[r][1] * f[1] + self.rot[r][2] * f[2] + self.tr[r]
        })
    }

    /// Parses `x,y,z` notation (an optional leading operator number is skipped).
    pub fn parse(s: &str) -> Option<SymOp> {
        let s = s.trim();
        // "1 x,y,z" or "1 'x, y, z'" forms put a number first.
        let s = match s.split_once(|c: char| c.is_whitespace()) {
            Some((n, rest)) if n.chars().all(|c| c.is_ascii_digit()) => rest.trim(),
            _ => s,
        };
        let s = s.trim_matches(|c| c == '\'' || c == '"');
        let parts: Vec<&str> = s.split(',').collect();
        if parts.len() != 3 {
            return None;
        }
        let mut op = SymOp {
            rot: [[0.0; 3]; 3],
            tr: [0.0; 3],
        };
        for (r, p) in parts.iter().enumerate() {
            let (row, t) = parse_row(p)?;
            op.rot[r] = row;
            op.tr[r] = t;
        }
        Some(op)
    }
}

fn parse_row(s: &str) -> Option<([f64; 3], f64)> {
    let b: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace())
        .map(|c| c.to_ascii_lowercase())
        .collect();
    if b.is_empty() {
        return None;
    }
    let mut row = [0.0; 3];
    let mut t = 0.0;
    let mut i = 0;
    while i < b.len() {
        let mut sign = 1.0;
        if b[i] == b'+' || b[i] == b'-' {
            if b[i] == b'-' {
                sign = -1.0;
            }
            i += 1;
        }
        // A number (with an optional fraction), possibly multiplying an axis: "2x", "1/2".
        let start = i;
        while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
            i += 1;
        }
        let mut val: Option<f64> = if i > start {
            Some(std::str::from_utf8(&b[start..i]).ok()?.parse().ok()?)
        } else {
            None
        };
        if i < b.len() && b[i] == b'/' {
            i += 1;
            let s2 = i;
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            let den: f64 = std::str::from_utf8(&b[s2..i]).ok()?.parse().ok()?;
            if den == 0.0 {
                return None;
            }
            val = Some(val? / den);
        }
        if i < b.len() && b[i] == b'*' {
            i += 1;
        }
        match b.get(i) {
            Some(c @ (b'x' | b'y' | b'z')) => {
                row[usize::from(c - b'x')] += sign * val.unwrap_or(1.0);
                i += 1;
            }
            Some(b'+' | b'-') | None => t += sign * val?,
            _ => return None,
        }
    }
    Some((row, t))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notations() {
        let op = SymOp::parse("-x+1/2, y+1/2, -z").unwrap();
        let p = op.apply([0.1, 0.2, 0.3]);
        assert!(
            p.iter()
                .zip([0.4, 0.7, -0.3])
                .all(|(a, b)| (a - b).abs() < 1e-12)
        );
        let op = SymOp::parse("x-y,x,z+1/3").unwrap();
        let p = op.apply([0.5, 0.25, 0.0]);
        assert!((p[0] - 0.25).abs() < 1e-12 && (p[1] - 0.5).abs() < 1e-12);
        assert!((p[2] - 1.0 / 3.0).abs() < 1e-12);
        let op = SymOp::parse("'1/2+X, 0.5-Y, Z'").unwrap();
        assert_eq!(op.apply([0.0, 0.0, 0.0]), [0.5, 0.5, 0.0]);
        assert_eq!(SymOp::parse("2 x,y,z"), Some(SymOp::IDENTITY));
        assert!(SymOp::parse("x,y").is_none());
        assert!(SymOp::parse("x,y,q").is_none());
        assert!(SymOp::parse("x,y,z+1/0").is_none());
    }
}
