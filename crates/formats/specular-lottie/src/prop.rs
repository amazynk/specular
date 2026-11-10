//! Animatable properties: a fixed value or keyframes with cubic-bezier easing (the out
//! tangent of a key and the in tangent of the next), hold keys, and spatial tangents for
//! positions; shape paths interpolate vertex by vertex.

use serde_json::Value;

/// A key of a numeric property.
#[derive(Clone, Debug)]
pub(crate) struct Key {
    pub t: f64,
    pub s: Vec<f64>,
    /// The end value (old exports; newer ones use the next key's start).
    pub e: Option<Vec<f64>>,
    /// Easing out of this key and into the next: (x, y) control points.
    pub out_ease: (f64, f64),
    pub in_ease: (f64, f64),
    pub hold: bool,
    /// Spatial tangents (positions), relative to the start and end values.
    pub to: Option<Vec<f64>>,
    pub ti: Option<Vec<f64>>,
}

#[derive(Clone, Debug)]
pub(crate) enum Prop {
    Fixed(Vec<f64>),
    Keys(Vec<Key>),
}

fn nums(v: &Value) -> Option<Vec<f64>> {
    match v {
        Value::Number(n) => Some(vec![n.as_f64()?]),
        Value::Array(a) => a.iter().map(Value::as_f64).collect(),
        _ => None,
    }
}

/// The first number of an easing component (a number or a list of them).
fn first(v: Option<&Value>, d: f64) -> f64 {
    match v {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(d),
        Some(Value::Array(a)) => a.first().and_then(Value::as_f64).unwrap_or(d),
        _ => d,
    }
}

impl Prop {
    pub fn parse(v: Option<&Value>, default: &[f64]) -> Prop {
        let Some(v) = v else {
            return Prop::Fixed(default.to_vec());
        };
        let k = v.get("k");
        let animated = v.get("a").and_then(Value::as_i64) == Some(1)
            || k.and_then(Value::as_array)
                .and_then(|a| a.first())
                .is_some_and(|f| f.get("t").is_some());
        if animated && let Some(Value::Array(keys)) = k {
            let mut out = Vec::new();
            for key in keys {
                let Some(t) = key.get("t").and_then(Value::as_f64) else {
                    continue;
                };
                let s = key.get("s").and_then(nums);
                let s = match s {
                    Some(s) => s,
                    // A last key of old exports carries only its time.
                    None => match out.last() {
                        Some(Key { e: Some(e), .. }) => e.clone(),
                        Some(Key { s, .. }) => s.clone(),
                        None => continue,
                    },
                };
                out.push(Key {
                    t,
                    s,
                    e: key.get("e").and_then(nums),
                    out_ease: (
                        first(key.pointer("/o/x"), 0.0),
                        first(key.pointer("/o/y"), 0.0),
                    ),
                    in_ease: (
                        first(key.pointer("/i/x"), 1.0),
                        first(key.pointer("/i/y"), 1.0),
                    ),
                    hold: key.get("h").and_then(Value::as_i64) == Some(1),
                    to: key.get("to").and_then(nums),
                    ti: key.get("ti").and_then(nums),
                });
            }
            if out.is_empty() {
                return Prop::Fixed(default.to_vec());
            }
            return Prop::Keys(out);
        }
        Prop::Fixed(k.and_then(nums).unwrap_or_else(|| default.to_vec()))
    }

    pub fn at(&self, t: f64) -> Vec<f64> {
        match self {
            Prop::Fixed(v) => v.clone(),
            Prop::Keys(keys) => {
                let (a, b, f) = match segment(keys, t, |k| k.t) {
                    Segment::Before => return keys[0].s.clone(),
                    Segment::After => {
                        let l = &keys[keys.len() - 1];
                        return if keys.len() > 1 && l.e.is_none() {
                            l.s.clone()
                        } else {
                            l.e.clone().unwrap_or_else(|| l.s.clone())
                        };
                    }
                    Segment::Between(i, f) => (&keys[i], &keys[i + 1], f),
                };
                let end = a.e.as_ref().unwrap_or(&b.s);
                if a.hold {
                    return a.s.clone();
                }
                let e = ease(a.out_ease, a.in_ease, f);
                match (&a.to, &a.ti) {
                    (Some(to), Some(ti))
                        if a.s.len() >= 2 && to.iter().chain(ti).any(|v| *v != 0.0) =>
                    {
                        // A position along its spatial bezier.
                        (0..a.s.len().min(end.len()))
                            .map(|d| {
                                let p0 = a.s[d];
                                let p3 = end[d];
                                let p1 = p0 + to.get(d).copied().unwrap_or(0.0);
                                let p2 = p3 + ti.get(d).copied().unwrap_or(0.0);
                                let u = 1.0 - e;
                                u * u * u * p0
                                    + 3.0 * u * u * e * p1
                                    + 3.0 * u * e * e * p2
                                    + e * e * e * p3
                            })
                            .collect()
                    }
                    _ => a.s.iter().zip(end).map(|(x, y)| x + (y - x) * e).collect(),
                }
            }
        }
    }

    pub fn num(&self, t: f64) -> f64 {
        self.at(t).first().copied().unwrap_or(0.0)
    }
}

pub(crate) enum Segment {
    Before,
    After,
    Between(usize, f64),
}

pub(crate) fn segment<K>(keys: &[K], t: f64, time: impl Fn(&K) -> f64) -> Segment {
    if keys.is_empty() || t < time(&keys[0]) {
        return Segment::Before;
    }
    for i in 0..keys.len() - 1 {
        let (t0, t1) = (time(&keys[i]), time(&keys[i + 1]));
        if t < t1 {
            let f = if t1 > t0 { (t - t0) / (t1 - t0) } else { 1.0 };
            return Segment::Between(i, f.clamp(0.0, 1.0));
        }
    }
    Segment::After
}

/// The eased fraction for `x` on the curve (0,0), `o`, `i`, (1,1).
pub(crate) fn ease(o: (f64, f64), i: (f64, f64), x: f64) -> f64 {
    let (x1, y1, x2, y2) = (o.0.clamp(0.0, 1.0), o.1, i.0.clamp(0.0, 1.0), i.1);
    if (x1 - y1).abs() < 1e-9 && (x2 - y2).abs() < 1e-9 {
        return x;
    }
    let bez = |a: f64, b: f64, s: f64| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // Solve x(s) = x by bisection (monotonic in s for x control points in [0, 1]).
    let (mut lo, mut hi) = (0.0, 1.0);
    let mut s = x;
    for _ in 0..40 {
        let v = bez(x1, x2, s);
        if (v - x).abs() < 1e-7 {
            break;
        }
        if v < x {
            lo = s;
        } else {
            hi = s;
        }
        s = (lo + hi) / 2.0;
    }
    bez(y1, y2, s)
}

/// A bezier path: vertices with in and out tangents (relative to the vertex).
#[derive(Clone, Debug, Default)]
pub(crate) struct Bez {
    pub closed: bool,
    pub v: Vec<[f64; 2]>,
    pub i: Vec<[f64; 2]>,
    pub o: Vec<[f64; 2]>,
}

fn pairs(v: Option<&Value>) -> Vec<[f64; 2]> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| {
                    let p = nums(p).unwrap_or_default();
                    [
                        p.first().copied().unwrap_or(0.0),
                        p.get(1).copied().unwrap_or(0.0),
                    ]
                })
                .collect()
        })
        .unwrap_or_default()
}

impl Bez {
    fn parse(v: &Value) -> Option<Bez> {
        let v = match v {
            Value::Array(a) => a.first()?,
            v => v,
        };
        let verts = pairs(v.get("v"));
        let n = verts.len();
        let mut i = pairs(v.get("i"));
        let mut o = pairs(v.get("o"));
        i.resize(n, [0.0, 0.0]);
        o.resize(n, [0.0, 0.0]);
        Some(Bez {
            closed: v.get("c").and_then(Value::as_bool).unwrap_or(false),
            v: verts,
            i,
            o,
        })
    }

    fn lerp(&self, b: &Bez, f: f64) -> Bez {
        if self.v.len() != b.v.len() {
            return if f < 1.0 { self.clone() } else { b.clone() };
        }
        let mix = |x: &[[f64; 2]], y: &[[f64; 2]]| {
            x.iter()
                .zip(y)
                .map(|(p, q)| [p[0] + (q[0] - p[0]) * f, p[1] + (q[1] - p[1]) * f])
                .collect()
        };
        Bez {
            closed: self.closed,
            v: mix(&self.v, &b.v),
            i: mix(&self.i, &b.i),
            o: mix(&self.o, &b.o),
        }
    }

    pub fn path(&self) -> specular_drawpage::BezPath {
        let mut p = specular_drawpage::BezPath::new();
        let n = self.v.len();
        if n == 0 {
            return p;
        }
        p.move_to((self.v[0][0], self.v[0][1]));
        let seg = |p: &mut specular_drawpage::BezPath, a: usize, b: usize| {
            let (va, vb) = (self.v[a], self.v[b]);
            p.curve_to(
                (va[0] + self.o[a][0], va[1] + self.o[a][1]),
                (vb[0] + self.i[b][0], vb[1] + self.i[b][1]),
                (vb[0], vb[1]),
            );
        };
        for k in 1..n {
            seg(&mut p, k - 1, k);
        }
        if self.closed {
            seg(&mut p, n - 1, 0);
            p.close_path();
        }
        p
    }
}

/// A path key: time, start, end (old exports), easing out and in, hold.
pub(crate) type PathKey = (f64, Bez, Option<Bez>, (f64, f64), (f64, f64), bool);

/// A shape path property.
#[derive(Clone, Debug)]
pub(crate) enum PathProp {
    Fixed(Bez),
    Keys(Vec<PathKey>),
}

impl PathProp {
    pub fn parse(v: Option<&Value>) -> PathProp {
        let Some(v) = v else {
            return PathProp::Fixed(Bez::default());
        };
        let k = v.get("k");
        if let Some(Value::Array(keys)) = k
            && keys.first().is_some_and(|f| f.get("t").is_some())
        {
            let mut out: Vec<PathKey> = Vec::new();
            for key in keys {
                let Some(t) = key.get("t").and_then(Value::as_f64) else {
                    continue;
                };
                let s = match key.get("s").and_then(Bez::parse) {
                    Some(s) => s,
                    None => match out.last() {
                        Some((_, s, e, ..)) => e.clone().unwrap_or_else(|| s.clone()),
                        None => continue,
                    },
                };
                out.push((
                    t,
                    s,
                    key.get("e").and_then(Bez::parse),
                    (
                        first(key.pointer("/o/x"), 0.0),
                        first(key.pointer("/o/y"), 0.0),
                    ),
                    (
                        first(key.pointer("/i/x"), 1.0),
                        first(key.pointer("/i/y"), 1.0),
                    ),
                    key.get("h").and_then(Value::as_i64) == Some(1),
                ));
            }
            if !out.is_empty() {
                return PathProp::Keys(out);
            }
        }
        PathProp::Fixed(k.and_then(Bez::parse).unwrap_or_default())
    }

    pub fn at(&self, t: f64) -> Bez {
        match self {
            PathProp::Fixed(b) => b.clone(),
            PathProp::Keys(keys) => match segment(keys, t, |k| k.0) {
                Segment::Before => keys[0].1.clone(),
                Segment::After => {
                    let l = &keys[keys.len() - 1];
                    l.2.clone().unwrap_or_else(|| l.1.clone())
                }
                Segment::Between(i, f) => {
                    let (a, b) = (&keys[i], &keys[i + 1]);
                    if a.5 {
                        return a.1.clone();
                    }
                    let end = a.2.as_ref().unwrap_or(&b.1);
                    a.1.lerp(end, ease(a.3, a.4, f))
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_interpolate() {
        let v: Value = serde_json::from_str(
            r#"{"a":1,"k":[{"t":0,"s":[0,0],"o":{"x":0,"y":0},"i":{"x":1,"y":1}},{"t":10,"s":[10,20]}]}"#,
        )
        .unwrap();
        let p = Prop::parse(Some(&v), &[0.0]);
        assert_eq!(p.at(5.0), vec![5.0, 10.0]);
        assert_eq!(p.at(-1.0), vec![0.0, 0.0]);
        assert_eq!(p.at(20.0), vec![10.0, 20.0]);
        // Ease in-out: slow at the ends.
        assert!(ease((0.42, 0.0), (0.58, 1.0), 0.1) < 0.05);
        assert!((ease((0.42, 0.0), (0.58, 1.0), 0.5) - 0.5).abs() < 1e-6);
    }
}
