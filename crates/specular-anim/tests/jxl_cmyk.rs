//! An animated CMYK JPEG XL plays through the player on its real session: every frame is
//! shown in turn, not just the first (the file is specular-decode's test writer).

#[path = "../../specular-decode/tests/common/jxl_cmyk.rs"]
mod jxl_cmyk;

use specular_anim::{DelayPolicy, Player, PlayerConfig};
use specular_core::Source;
use specular_decode::Registry;
use std::sync::Arc;
use std::time::{Duration, Instant};

#[test]
fn animated_cmyk_jxl_plays_in_the_player() {
    let file = jxl_cmyk::animated_cmyk(&[64, 192, 128]);
    let s = Registry::default()
        .open(Arc::new(Source::from_vec("anim.jxl", file)))
        .unwrap();
    for scale in [1, 2] {
        let cfg = PlayerConfig {
            delay_policy: DelayPolicy::AS_STORED,
            ..PlayerConfig::default()
        };
        let mut p = Player::from_session(s.as_ref(), scale, cfg, None)
            .unwrap()
            .expect("animated");
        let t0 = Instant::now();
        let mut shown = Vec::new();
        for k in 0..4u64 {
            assert!(p.wait_idle(Duration::from_secs(10)));
            let r = p.tick(t0 + Duration::from_millis(100 * k));
            if let Some(f) = r.show {
                shown.push(f.index);
            }
        }
        assert_eq!(shown, [0, 1, 2, 0], "1/{scale}");
    }
}
