//! Size, quality and speed of the encoder alone on 1080p slideshows rendered by the export's
//! own path (the app's `dump_slideshows_for_encoder_bench`): per show and QP the bytes, mean
//! luma PSNR (from the reconstruction, which is the decoder's output bit for bit), fps and
//! picture and macroblock decisions, for the fading pictures and the rest; then the mean
//! chroma PSNR of all pictures (for judging luma/chroma trade-offs); last the bytes and count
//! of the IDR pictures (for the export's size estimate) and a hash of the bitstream (for
//! checking that speed work leaves the output alone).
//!
//! `SPECULAR_FADE_BENCH_DIR=<dir>`; `SPECULAR_FADE_BENCH_SHOWS` (default
//! crossfade,kenburns,black), `SPECULAR_FADE_BENCH_QPS` (default 18,24,28),
//! `SPECULAR_FADE_BENCH_FRAMES` (first that many pictures of each show),
//! `SPECULAR_FADE_BENCH_PROFILE` (baseline, main or high: the default). CSV on stderr.
//! `cargo test --release -p specular-h264enc --test slideshow -- --ignored --nocapture`.

use std::hash::{Hash, Hasher};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

use specular_h264enc::{Config, Encoder, Frame, Hints, MotionHint, Profile};

struct Cue {
    idr: bool,
    fading: bool,
    hint: Option<MotionHint>,
}

fn psnr(a: &[u8], b: &[u8]) -> f64 {
    let se: u64 = a
        .iter()
        .zip(b)
        .map(|(&x, &y)| {
            let d = i64::from(x) - i64::from(y);
            (d * d) as u64
        })
        .sum();
    if se == 0 {
        return 99.0;
    }
    10.0 * (255.0f64 * 255.0 * a.len() as f64 / se as f64).log10()
}

#[test]
#[ignore = "benchmark"]
fn bench_slideshows() {
    let Ok(dir) = std::env::var("SPECULAR_FADE_BENCH_DIR") else {
        eprintln!("SPECULAR_FADE_BENCH_DIR not set; skipped");
        return;
    };
    let env = |k: &str, d: &str| std::env::var(k).unwrap_or_else(|_| d.to_owned());
    let qps: Vec<u8> = env("SPECULAR_FADE_BENCH_QPS", "18,24,28")
        .split(',')
        .map(|q| q.trim().parse().unwrap())
        .collect();
    let limit: usize = env("SPECULAR_FADE_BENCH_FRAMES", "0").parse().unwrap();
    let shows = env("SPECULAR_FADE_BENCH_SHOWS", "crossfade,kenburns,black");
    let profile = match env("SPECULAR_FADE_BENCH_PROFILE", "high").as_str() {
        "baseline" => Profile::Baseline,
        "main" => Profile::Main,
        _ => Profile::High,
    };
    eprintln!(
        "show,qp,kB,fade_kB,rest_kB,dB,fade_dB,rest_dB,fps,p_pics,b_pics,weighted,intra,inter,skip,cdB,idr_kB,idrs,hash"
    );
    for name in shows.split(',') {
        let dir = Path::new(&dir);
        let Ok(meta) = std::fs::read_to_string(dir.join(format!("{name}.txt"))) else {
            eprintln!("{name}: not dumped; skipped");
            continue;
        };
        let file = std::fs::File::open(dir.join(format!("{name}.yuv"))).unwrap();
        let mut lines = meta.lines();
        let head: Vec<u32> = lines
            .next()
            .unwrap()
            .split(' ')
            .map(|v| v.parse().unwrap())
            .collect();
        let (w, h, fps) = (head[0] as usize, head[1] as usize, head[2]);
        let mut cues: Vec<Cue> = lines
            .map(|l| {
                let v: Vec<&str> = l.split(' ').collect();
                Cue {
                    idr: v[0] == "1",
                    fading: v[1] == "1",
                    hint: (v[2] != "-").then(|| MotionHint {
                        scale: v[2].parse().unwrap(),
                        dx: v[3].parse().unwrap(),
                        dy: v[4].parse().unwrap(),
                    }),
                }
            })
            .collect();
        if limit > 0 {
            cues.truncate(limit);
        }
        let n = cues.len();
        let (ys, cs) = (w * h, (w / 2) * (h / 2));
        let read = |i: usize| -> Vec<u8> {
            let mut buf = vec![0u8; ys + 2 * cs];
            let at = (i * buf.len()) as u64;
            let mut f = &file;
            f.seek(SeekFrom::Start(at)).unwrap();
            f.read_exact(&mut buf).unwrap();
            buf
        };
        for &qp in &qps {
            let mut cfg = Config::new(w as u32, h as u32, fps, 1, qp);
            cfg.keyint = fps * 5;
            cfg.profile = profile;
            let mut enc = Encoder::new(cfg).unwrap();
            let mut secs = 0.0;
            // (bytes, dB, count) for fading pictures and the rest.
            let mut acc = [(0usize, 0f64, 0usize); 2];
            let mut cdb_sum = 0f64;
            let (mut pp, mut bp, mut wp) = (0, 0, 0);
            let (mut ni, mut nn, mut ns) = (0u64, 0u64, 0u64);
            let (mut idr_bytes, mut idrs) = (0usize, 0usize);
            let mut hash = std::hash::DefaultHasher::new();
            for (i, cue) in cues.iter().enumerate() {
                let p = read(i);
                let f = Frame {
                    y: &p[..ys],
                    u: &p[ys..ys + cs],
                    v: &p[ys + cs..],
                    y_stride: w,
                    c_stride: w / 2,
                };
                let t0 = Instant::now();
                let hints = Hints {
                    motion: cue.hint,
                    fading: cue.fading.then_some(true),
                };
                let pk = enc.encode_with(&f, cue.idr, hints);
                secs += t0.elapsed().as_secs_f64();
                let rec = enc.reconstruction();
                let db = psnr(&p[..ys], &rec[0]);
                cdb_sum += (psnr(&p[ys..ys + cs], &rec[1]) + psnr(&p[ys + cs..], &rec[2])) / 2.0;
                let a = &mut acc[usize::from(cue.fading)];
                a.0 += pk.data.len();
                a.1 += db;
                a.2 += 1;
                pk.data.hash(&mut hash);
                if pk.key {
                    idr_bytes += pk.data.len();
                    idrs += 1;
                } else if pk.stats.b {
                    bp += 1;
                } else {
                    pp += 1;
                }
                wp += u32::from(pk.stats.weighted);
                ni += u64::from(pk.stats.intra);
                nn += u64::from(pk.stats.inter);
                ns += u64::from(pk.stats.skip);
            }
            let kb = |a: (usize, f64, usize)| a.0 as f64 / 1000.0;
            let db = |a: (usize, f64, usize)| a.1 / a.2.max(1) as f64;
            let all = (
                acc[0].0 + acc[1].0,
                acc[0].1 + acc[1].1,
                acc[0].2 + acc[1].2,
            );
            eprintln!(
                "{name},{qp},{:.0},{:.0},{:.0},{:.3},{:.3},{:.3},{:.1},{pp},{bp},{wp},{ni},{nn},{ns},{:.3},{:.0},{idrs},{:016x}",
                kb(all),
                kb(acc[1]),
                kb(acc[0]),
                db(all),
                db(acc[1]),
                db(acc[0]),
                n as f64 / secs,
                cdb_sum / all.2.max(1) as f64,
                idr_bytes as f64 / 1000.0,
                hash.finish(),
            );
        }
    }
}
