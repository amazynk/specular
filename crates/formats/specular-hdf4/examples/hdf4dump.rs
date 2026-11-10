//! `hdf4dump FILE... DIR`: writes what specular-hdf4 reads from each file to
//! `DIR/<basename>/manifest.tsv` for `gen/oracle.py compare` (lines
//! `kind\tname\ttype\tshape\tvalues`; long value lists go to raw little-endian f64 files named
//! `f:<file>`), and prints the open and first-dataset timings.

use std::io::Write;
use std::path::Path;
use std::time::Instant;

use specular_hdf4::{Attr, Ctx, File, Value};

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('\t', "\\t")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn attr_line(kind: &str, name: &str, a: &Attr) -> String {
    let (count, vals) = match &a.value {
        Value::Str(_) => {
            let t = a.value.text().unwrap_or_default();
            (t.chars().count(), format!("s:{}", esc(&t)))
        }
        v => {
            let n = v.as_f64s().unwrap_or_default();
            (n.len(), join(&n))
        }
    };
    format!("{kind}\t{name}\t?\t{count}\t{vals}\n")
}

fn join(v: &[f64]) -> String {
    v.iter()
        .map(|x| format!("{x:?}"))
        .collect::<Vec<_>>()
        .join(" ")
}

struct Out<'a> {
    dir: &'a Path,
    files: usize,
    man: String,
}

impl Out<'_> {
    fn values(&mut self, v: &[f64]) -> String {
        if v.len() <= 64 {
            return join(v);
        }
        self.files += 1;
        let name = format!("v{}.f64", self.files);
        let mut b = Vec::with_capacity(v.len() * 8);
        for x in v {
            b.extend_from_slice(&x.to_le_bytes());
        }
        std::fs::write(self.dir.join(&name), b).expect("write values");
        format!("f:{name}")
    }
}

fn dump(path: &Path, root: &Path) {
    let data = std::fs::read(path).expect("read file");
    let base = path.file_name().unwrap().to_string_lossy().into_owned();
    let dir = root.join(&base);
    std::fs::create_dir_all(&dir).expect("dump dir");
    let folder = path.parent().map(Path::to_path_buf).unwrap_or_default();
    let ext = move |name: &str, off: u64, len: u64| -> Option<Vec<u8>> {
        if name.contains('/') || name.contains('\\') || name.starts_with('.') {
            return None;
        }
        let b = std::fs::read(folder.join(name)).ok()?;
        let s = usize::try_from(off).ok()?;
        let e = s.checked_add(usize::try_from(len).ok()?)?;
        Some(b.get(s..e.min(b.len()))?.to_vec())
    };
    let t0 = Instant::now();
    let f = match File::open(&data) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("{base}: {e}");
            return;
        }
    };
    let t_open = t0.elapsed();
    let ctx = Ctx::with_external(&data, &ext);
    let mut out = Out {
        dir: &dir,
        files: 0,
        man: String::new(),
    };
    let mut first: Option<std::time::Duration> = None;
    for s in &f.sds {
        if s.shape.first() == Some(&0) {
            continue;
        }
        let t = Instant::now();
        match f.read_sds_all(ctx, s, 1 << 31) {
            Ok(v) => {
                if first.is_none() && s.shape.len() >= 2 {
                    first = Some(t.elapsed());
                }
                let shape: Vec<String> = s.shape.iter().map(u64::to_string).collect();
                let tn = format!("{}{}", s.nt.name(), if s.nt.little { "le" } else { "" });
                let vals = out.values(&v);
                out.man += &format!("sds\t{}\t{tn}\t{}\t{vals}\n", s.name, shape.join(","));
            }
            Err(e) => eprintln!("{base}: dataset {}: {e}", s.name),
        }
        for a in &s.attrs {
            out.man += &attr_line("sattr", &format!("{}/{}", s.name, a.name), a);
        }
    }
    for a in &f.attrs {
        out.man += &attr_line("gattr", &a.name, a);
    }
    for im in &f.images {
        match f.read_image(ctx, im) {
            Ok(d) => {
                let tn = format!("{}{}", im.nt.name(), if im.is_jpeg() { "jpeg" } else { "" });
                let vals = out.values(&d.values);
                out.man += &format!(
                    "ri\t{}\t{tn}\t{},{},{}\t{vals}\n",
                    im.name, d.height, d.width, d.ncomp
                );
            }
            Err(e) => eprintln!("{base}: image {}: {e}", im.name),
        }
    }
    for w in &f.warnings {
        eprintln!("{base}: warning: {w}");
    }
    let mut m = std::fs::File::create(dir.join("manifest.tsv")).expect("manifest");
    m.write_all(out.man.as_bytes()).expect("write manifest");
    println!(
        "{base}\t{} bytes\topen {:.2} ms\tfirst 2-D dataset {}",
        data.len(),
        t_open.as_secs_f64() * 1e3,
        first.map_or("-".into(), |d| format!("{:.2} ms", d.as_secs_f64() * 1e3))
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((dir, files)) = args.split_last() else {
        eprintln!("usage: hdf4dump FILE... DIR");
        std::process::exit(2);
    };
    for p in files {
        dump(Path::new(p), Path::new(dir));
    }
}
