//! Lists the "Open with…" applications for a file: `cargo run --example open_with -- FILE [LANG]`.

fn main() {
    let mut args = std::env::args_os().skip(1);
    let Some(file) = args.next() else {
        eprintln!("usage: open_with FILE [LANG]");
        std::process::exit(2);
    };
    let lang = args
        .next()
        .and_then(|l| l.into_string().ok())
        .unwrap_or_else(|| "en".into());
    let t = std::time::Instant::now();
    let hs = specular_platform::open_with::handlers_for(std::path::Path::new(&file), &lang);
    let took = t.elapsed();
    for h in &hs {
        println!(
            "{} {:<30} {}",
            if h.is_default { "*" } else { " " },
            h.name,
            h.id
        );
    }
    println!("{} handlers in {took:.1?}", hs.len());
}
