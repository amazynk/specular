//! Bruker parameter files (`acqus`, `acqu2s`, `procs`, `proc2s`): JCAMP-DX style
//! `##$NAME= value` records, arrays as `(0..n)` followed by their values, strings in `<>`.

use std::collections::HashMap;

/// A parameter file's records: name (without `##` and `$`) to its raw value.
#[derive(Clone, Debug, Default)]
pub struct Params {
    values: HashMap<String, String>,
    /// Names in file order (each once).
    order: Vec<String>,
}

impl Params {
    pub fn parse(text: &str) -> Params {
        let mut p = Params::default();
        let mut current: Option<(String, String)> = None;
        for raw in text.lines() {
            let line = raw.trim_end_matches('\r');
            if let Some(rest) = line.strip_prefix("##") {
                if let Some((k, v)) = current.take() {
                    p.insert(k, v);
                }
                let Some((name, value)) = rest.split_once('=') else {
                    continue;
                };
                let name = name.trim().trim_start_matches('$').to_owned();
                current = Some((name, value.trim().to_owned()));
            } else if line.starts_with("$$") {
                continue;
            } else if let Some((_, v)) = current.as_mut() {
                // Continuation: an array's values or a long string.
                if !v.is_empty() {
                    v.push(' ');
                }
                v.push_str(line.trim());
            }
        }
        if let Some((k, v)) = current.take() {
            p.insert(k, v);
        }
        p
    }

    fn insert(&mut self, k: String, v: String) {
        if !self.values.contains_key(&k) {
            self.order.push(k.clone());
        }
        self.values.insert(k, v);
    }

    /// The raw value as stored.
    pub fn raw(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    /// A string value without its `<>`; `None` when empty.
    pub fn text(&self, name: &str) -> Option<String> {
        let v = self.raw(name)?.trim();
        let v = v
            .strip_prefix('<')
            .and_then(|s| s.strip_suffix('>'))
            .unwrap_or(v)
            .trim();
        (!v.is_empty()).then(|| v.to_owned())
    }

    pub fn f64(&self, name: &str) -> Option<f64> {
        let v = self.raw(name)?.trim();
        if v.starts_with('(') {
            return None;
        }
        v.parse::<f64>().ok().filter(|x| x.is_finite())
    }

    pub fn i64(&self, name: &str) -> Option<i64> {
        let x = self.f64(name)?;
        (x.fract() == 0.0 && x.abs() < 9e15).then_some(x as i64)
    }

    /// Element `i` of an array value (`(0..63)` then the values).
    pub fn element(&self, name: &str, i: usize) -> Option<f64> {
        let v = self.raw(name)?.trim();
        let rest = v.strip_prefix('(')?;
        let (_, values) = rest.split_once(')')?;
        values.split_whitespace().nth(i)?.parse().ok()
    }

    /// Names in file order.
    pub fn names(&self) -> &[String] {
        &self.order
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_arrays_and_strings() {
        let p = Params::parse(
            "##TITLE= Parameter file, TOPSPIN\t\tVersion 3.2\n##$TD= 32768\n$$ comment\n\
             ##$NUC1= <1H>\n##$P= (0..3)\n10 12.5\n0 7\n##$SW_h= 8012.82\n##END=\n",
        );
        assert_eq!(p.i64("TD"), Some(32768));
        assert_eq!(p.text("NUC1").as_deref(), Some("1H"));
        assert_eq!(p.element("P", 1), Some(12.5));
        assert_eq!(p.element("P", 3), Some(7.0));
        assert_eq!(p.f64("P"), None);
        assert_eq!(p.f64("SW_h"), Some(8012.82));
        assert!(p.raw("TITLE").unwrap().contains("TOPSPIN"));
    }
}
