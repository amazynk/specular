use super::*;

fn analysis() -> Analysis {
    let cols = 100;
    Analysis {
        cols,
        lanes: 2,
        channels: 2,
        duration_s: 10.0,
        wave: vec![[0.0; 3]; cols * 2],
        spec: vec![0; cols * BINS],
        bands: vec![0; cols * BANDS],
        meters: vec![[0.1, 0.005, 0.1]; cols * 2],
        stereo: vec![[0.0, 0.01, 0.01]; cols],
        f_lo: 20.0,
        f_hi: 24_000.0,
        exact: true,
        loudness: None,
    }
}

#[test]
fn live_feed_reads_the_tap_and_falls_back() {
    let tap = Arc::new(Tap::new());
    let feed = LiveFeed::new(
        Arc::clone(&tap),
        AnalysisFeed {
            analysis: Arc::new(analysis()),
            loudness: None,
        },
    );
    let mut s = Snapshot::default();
    // Nothing heard: the analysis (−20 dBFS peaks, correlation 0).
    assert!(feed.snapshot(2.0, &mut s));
    assert!(!s.live);
    assert!((s.peak[0] + 20.0).abs() < 0.01, "{:?}", s.peak);
    assert_eq!(s.correlation, Some(0.0));
    // 1 kHz at half scale in both channels, from 1.5 s to 2 s.
    let rate = 48_000u32;
    let buf: Vec<f32> = (0..24_000)
        .flat_map(|i| {
            let t = i as f64 / f64::from(rate);
            let v = (0.5 * (std::f64::consts::TAU * 1000.0 * t).sin()) as f32;
            [v, v]
        })
        .collect();
    tap.write(&buf, 2, rate, 1_500_000_000, 1.0);
    assert!(feed.snapshot(2.0, &mut s));
    assert!(s.live);
    for i in 0..2 {
        assert!((s.peak[i] + 6.02).abs() < 0.05, "{:?}", s.peak);
        assert!((s.rms[i] + 9.03).abs() < 0.05, "{:?}", s.rms);
        assert!(s.true_peak[i] >= s.peak[i] && s.true_peak[i] < -5.9);
    }
    assert!((s.correlation.unwrap() - 1.0).abs() < 1e-5);
    // The loudest band is 1 kHz (band 17), at about −6 dBFS.
    let (k, &v) = s
        .bands
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .unwrap();
    assert_eq!(k, 17);
    assert!((v + 6.0).abs() < 1.0, "{v}");
    // The goniometer's samples.
    let mut pcm = Vec::new();
    assert_eq!(feed.samples(2.0, 1024, &mut pcm), Some(2));
    assert_eq!(pcm.len(), 2048);
    // Paused: the analysis again.
    tap.idle();
    assert!(feed.snapshot(2.0, &mut s));
    assert!(!s.live);
    assert!((s.peak[0] + 20.0).abs() < 0.01);
    assert_eq!(feed.samples(2.0, 1024, &mut pcm), None);
}

#[test]
fn correlation_average_is_slow() {
    let r = Rates::new(false);
    let mut b = Ballistics::default();
    let mut s = Snapshot {
        channels: 2,
        correlation: Some(1.0),
        ..Snapshot::default()
    };
    b.update(&s, 0.016, &r);
    assert_eq!(b.correlation_avg, Some(1.0));
    s.correlation = Some(-1.0);
    b.update(&s, 0.1, &r);
    assert_eq!(b.correlation, Some(-1.0));
    let avg = b.correlation_avg.unwrap();
    assert!(avg > 0.9 && avg < 1.0, "{avg}");
    for _ in 0..300 {
        b.update(&s, 0.1, &r);
    }
    assert!((b.correlation_avg.unwrap() + 1.0).abs() < 1e-3);
    // Silence keeps the average.
    s.correlation = None;
    b.update(&s, 0.1, &r);
    assert!(b.correlation_avg.is_some());
}
