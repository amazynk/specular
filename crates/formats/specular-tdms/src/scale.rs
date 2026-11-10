//! NI scaling (`NI_Number_Of_Scales`, `NI_Scale[n]_*`): a list of scales, each computing
//! from the raw data, a DAQmx scaler, or other scales; the last one gives the channel's
//! values.

use crate::{Prop, find};

/// Where a scale takes its input: the raw data, or scale `n`.
pub(crate) const RAW: i64 = 0xFFFF_FFFF;

#[derive(Clone, Debug)]
pub(crate) enum Scale {
    /// A DAQmx scaler's values, by scale id.
    Scaler(u32),
    NoOp(i64),
    Linear {
        slope: f64,
        intercept: f64,
        input: i64,
    },
    Polynomial {
        coefficients: Vec<f64>,
        input: i64,
    },
    /// Linear interpolation of increasing `xs` onto `ys`, clamped at the ends.
    Table {
        xs: Vec<f64>,
        ys: Vec<f64>,
        input: i64,
    },
    Add(i64, i64),
    /// Right minus left (as NI's Excel importer does).
    Subtract(i64, i64),
}

impl Scale {
    /// The scales (or [`RAW`]) this one reads.
    fn inputs(&self) -> [Option<i64>; 2] {
        match self {
            Scale::Scaler(_) => [None, None],
            Scale::NoOp(i)
            | Scale::Linear { input: i, .. }
            | Scale::Polynomial { input: i, .. }
            | Scale::Table { input: i, .. } => [Some(*i), None],
            Scale::Add(l, r) | Scale::Subtract(l, r) => [Some(*l), Some(*r)],
        }
    }
}

/// Reads scale inputs: `None` the raw data, `Some(id)` DAQmx scaler `id`.
pub(crate) type Input<'a> = dyn FnMut(Option<u32>, &mut [f64]) -> crate::Result<()> + 'a;

#[derive(Clone, Debug)]
pub(crate) struct Scaling {
    pub scales: Vec<Scale>,
}

fn num(p: &[(String, Prop)], k: &str) -> Result<f64, String> {
    find(p, k)
        .and_then(Prop::as_f64)
        .ok_or_else(|| format!("the TDMS scaling property {k} is missing"))
}

fn source(p: &[(String, Prop)], k: &str) -> i64 {
    find(p, k).and_then(Prop::as_f64).map_or(RAW, |v| v as i64)
}

fn count(p: &[(String, Prop)]) -> Option<i64> {
    if let Some(v) = find(p, "NI_Number_Of_Scales") {
        return v.as_f64().map(|v| v as i64);
    }
    p.iter()
        .filter_map(|(k, _)| {
            k.strip_prefix("NI_Scale[")?
                .strip_suffix("]_Scale_Type")?
                .parse::<i64>()
                .ok()
        })
        .max()
        .map(|m| m + 1)
}

/// The scaling of one level's properties: `Ok(None)` when it has none (or says the data is
/// scaled already).
fn level(p: &[(String, Prop)]) -> Result<Option<Scaling>, String> {
    let Some(n) = count(p).filter(|&n| n > 0) else {
        return Ok(None);
    };
    if find(p, "NI_Scaling_Status").and_then(Prop::as_str) == Some("scaled") {
        return Ok(None);
    }
    if n > 4096 {
        return Err("a TDMS scaling with too many scales".into());
    }
    let mut scales = Vec::with_capacity(n as usize);
    for i in 0..n {
        let key = |s: &str| format!("NI_Scale[{i}]_{s}");
        let Some(t) = find(p, &key("Scale_Type")) else {
            scales.push(Scale::Scaler(i as u32));
            continue;
        };
        let t = t.as_str().unwrap_or("");
        scales.push(match t {
            "Linear" => Scale::Linear {
                slope: num(p, &key("Linear_Slope"))?,
                intercept: num(p, &key("Linear_Y_Intercept"))?,
                input: source(p, &key("Linear_Input_Source")),
            },
            "Polynomial" => {
                let k = num(p, &key("Polynomial_Coefficients_Size"))? as i64;
                if !(0..=4096).contains(&k) {
                    return Err("a TDMS polynomial scale with too many coefficients".into());
                }
                let coefficients = (0..k)
                    .map(|j| num(p, &key(&format!("Polynomial_Coefficients[{j}]"))))
                    .collect::<Result<Vec<_>, _>>()?;
                Scale::Polynomial {
                    coefficients,
                    input: source(p, &key("Polynomial_Input_Source")),
                }
            }
            "Table" => {
                let a = num(p, &key("Table_Pre_Scaled_Values_Size"))? as i64;
                let b = num(p, &key("Table_Scaled_Values_Size"))? as i64;
                if a != b || !(0..=65_536).contains(&a) {
                    return Err("a TDMS table scale whose two lists differ in length".into());
                }
                let mut xs = (0..a)
                    .map(|j| num(p, &key(&format!("Table_Pre_Scaled_Values[{j}]"))))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut ys = (0..a)
                    .map(|j| num(p, &key(&format!("Table_Scaled_Values[{j}]"))))
                    .collect::<Result<Vec<_>, _>>()?;
                let rising = |v: &[f64]| v.windows(2).all(|w| w[1] > w[0]);
                if !rising(&xs) {
                    xs.reverse();
                    ys.reverse();
                    if !rising(&xs) {
                        return Err("a TDMS table scale whose inputs are not in order".into());
                    }
                }
                Scale::Table {
                    xs,
                    ys,
                    input: source(p, &key("Table_Input_Source")),
                }
            }
            "Add" => Scale::Add(
                num(p, &key("Add_Left_Operand_Input_Source"))? as i64,
                num(p, &key("Add_Right_Operand_Input_Source"))? as i64,
            ),
            "Subtract" => Scale::Subtract(
                num(p, &key("Subtract_Left_Operand_Input_Source"))? as i64,
                num(p, &key("Subtract_Right_Operand_Input_Source"))? as i64,
            ),
            "AdvancedAPI" => Scale::NoOp(source(p, &key("AdvancedAPI_Input_Source"))),
            "RTD" | "Strain" | "Thermistor" | "Thermocouple" => {
                return Err(format!("TDMS {t} scaling is not supported"));
            }
            // Unknown scale types leave the data unscaled (as NI's own readers do).
            _ => return Ok(None),
        });
    }
    Ok(Some(Scaling { scales }))
}

/// The scaling of a channel: its own, else its group's, else the file's.
pub(crate) fn find_scaling(levels: [&[(String, Prop)]; 3]) -> Result<Option<Scaling>, String> {
    for p in levels {
        if let Some(s) = level(p)? {
            return Ok(Some(s));
        }
    }
    Ok(None)
}

impl Scaling {
    /// The values of scale `idx` (the raw data for [`RAW`]) for `n` samples, reading inputs
    /// through `input` (`None`: the raw data, `Some(id)`: DAQmx scaler `id`).
    ///
    /// Iterative: the scales `idx` needs are ordered inputs first (a scale that depends on
    /// itself is refused) and each is computed once, so neither a long chain (the stack) nor
    /// scales reading one input twice over and over (exponential work) can hurt.
    pub(crate) fn eval(&self, idx: i64, n: usize, input: &mut Input) -> crate::Result<Vec<f64>> {
        let unsupported = |m: &str| crate::Error::Unsupported(m.to_owned());
        let key = |i: i64| -> crate::Result<Option<usize>> {
            if i == RAW {
                return Ok(None);
            }
            usize::try_from(i)
                .ok()
                .filter(|&i| i < self.scales.len())
                .map(Some)
                .ok_or_else(|| unsupported("a TDMS scaling that refers to a scale it lacks"))
        };
        let Some(root) = key(idx)? else {
            let mut out = vec![0f64; n];
            input(None, &mut out)?;
            return Ok(out);
        };
        // Inputs first (depth-first, with an explicit stack). 1: on the current path.
        let len = self.scales.len();
        let mut state = vec![0u8; len];
        let mut order = Vec::new();
        let mut stack = vec![(root, false)];
        while let Some((i, done)) = stack.pop() {
            if done {
                state[i] = 2;
                order.push(i);
                continue;
            }
            match state[i] {
                1 => return Err(unsupported("a TDMS scaling that refers to itself")),
                2 => continue,
                _ => {}
            }
            state[i] = 1;
            stack.push((i, true));
            for c in self.scales[i].inputs().into_iter().flatten() {
                if let Some(c) = key(c)? {
                    stack.push((c, false));
                }
            }
        }
        // How often each value is still needed: the last use takes it, earlier ones copy.
        let mut uses = vec![0u32; len];
        let mut raw_uses = 0u32;
        for &i in &order {
            for c in self.scales[i].inputs().into_iter().flatten() {
                match key(c)? {
                    Some(c) => uses[c] += 1,
                    None => raw_uses += 1,
                }
            }
        }
        let mut memo: Vec<Option<Vec<f64>>> = vec![None; len];
        let mut raw: Option<Vec<f64>> = None;
        for &i in &order {
            let mut get = |c: i64| -> crate::Result<Vec<f64>> {
                let (slot, left) = match key(c)? {
                    Some(c) => (&mut memo[c], &mut uses[c]),
                    None => {
                        if raw.is_none() {
                            let mut out = vec![0f64; n];
                            input(None, &mut out)?;
                            raw = Some(out);
                        }
                        (&mut raw, &mut raw_uses)
                    }
                };
                *left = left.saturating_sub(1);
                let v = if *left == 0 {
                    slot.take()
                } else {
                    slot.clone()
                };
                v.ok_or_else(|| unsupported("a TDMS scaling that refers to itself"))
            };
            let v = match &self.scales[i] {
                Scale::Scaler(id) => {
                    let mut out = vec![0f64; n];
                    input(Some(*id), &mut out)?;
                    out
                }
                Scale::NoOp(c) => get(*c)?,
                Scale::Linear {
                    slope,
                    intercept,
                    input: c,
                } => {
                    let mut v = get(*c)?;
                    v.iter_mut().for_each(|x| *x = *x * slope + intercept);
                    v
                }
                Scale::Polynomial {
                    coefficients: k,
                    input: c,
                } => {
                    let mut v = get(*c)?;
                    match k.split_last() {
                        None => v.iter_mut().for_each(|x| *x = 0.0),
                        Some((&hi, rest)) => v.iter_mut().for_each(|x| {
                            let mut acc = hi + *x * 0.0;
                            for &k in rest.iter().rev() {
                                acc = k + acc * *x;
                            }
                            *x = acc;
                        }),
                    }
                    v
                }
                Scale::Table { xs, ys, input: c } => {
                    let mut v = get(*c)?;
                    v.iter_mut().for_each(|x| *x = interp(*x, xs, ys));
                    v
                }
                Scale::Add(l, r) => {
                    let a = get(*l)?;
                    let mut b = get(*r)?;
                    b.iter_mut().zip(a).for_each(|(y, x)| *y += x);
                    b
                }
                Scale::Subtract(l, r) => {
                    let a = get(*l)?;
                    let mut b = get(*r)?;
                    b.iter_mut().zip(a).for_each(|(y, x)| *y -= x);
                    b
                }
            };
            memo[i] = Some(v);
        }
        memo[root]
            .take()
            .ok_or_else(|| unsupported("a TDMS scaling that refers to itself"))
    }

    /// The last scale's index: the channel's values.
    pub(crate) fn last(&self) -> i64 {
        self.scales.len() as i64 - 1
    }
}

/// Piecewise-linear interpolation as numpy's `interp`.
fn interp(x: f64, xs: &[f64], ys: &[f64]) -> f64 {
    let n = xs.len();
    if n == 0 || x.is_nan() {
        return f64::NAN;
    }
    if x <= xs[0] {
        return ys[0];
    }
    if x >= xs[n - 1] {
        return ys[n - 1];
    }
    let j = xs.partition_point(|&v| v <= x) - 1;
    let slope = (ys[j + 1] - ys[j]) / (xs[j + 1] - xs[j]);
    let y = slope * (x - xs[j]) + ys[j];
    if y.is_nan() {
        let y2 = slope * (x - xs[j + 1]) + ys[j + 1];
        if y2.is_nan() && ys[j] == ys[j + 1] {
            return ys[j];
        }
        return y2;
    }
    y
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &Scaling, n: usize) -> crate::Result<Vec<f64>> {
        let mut input = |_: Option<u32>, out: &mut [f64]| {
            out.fill(1.0);
            Ok(())
        };
        s.eval(s.last(), n, &mut input)
    }

    /// A 4096-scale chain used to recurse once per scale; scales adding one input to
    /// itself used to cost 2^depth evaluations.
    #[test]
    fn hostile_scalings() {
        specular_thread::on_stack(256 * 1024, || {
            let mut scales = vec![Scale::NoOp(RAW)];
            scales.extend((1..4096).map(|i| Scale::Linear {
                slope: 1.0,
                intercept: 1.0,
                input: i - 1,
            }));
            let v = run(&Scaling { scales }, 4).unwrap();
            assert_eq!(v, vec![4096.0; 4]);

            let mut scales = vec![Scale::NoOp(RAW)];
            scales.extend((1..64).map(|i| Scale::Add(i - 1, i - 1)));
            let v = run(&Scaling { scales }, 2).unwrap();
            assert_eq!(v, vec![2f64.powi(63); 2]);

            let scales = vec![Scale::NoOp(RAW), Scale::Add(0, 2), Scale::NoOp(1)];
            assert!(run(&Scaling { scales }, 2).is_err());
        });
    }
}
