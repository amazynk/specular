//! A line repeated in a tight loop is written a few times, not thousands; other lines go on,
//! the same text from another place in the code is its own line, and nothing is lost: what
//! was left out is counted in a summary line.

fn spam(n: usize) {
    for _ in 0..n {
        specular_log::warn!(target: specular_log::TILES, "tile 3,4: damaged data");
    }
}

#[test]
fn repeated_identical_lines_are_rate_limited() {
    let buf = specular_log::capture("").expect("first logger in this process");
    spam(1000);
    specular_log::warn!(target: specular_log::TILES, "tile 3,5: damaged data");
    // The same words from another call site: its own first occurrence is written.
    specular_log::warn!(target: specular_log::TILES, "tile 3,4: damaged data");
    let text = String::from_utf8_lossy(&buf.lock()).into_owned();
    assert_eq!(text.matches("tile 3,4: damaged data").count(), 6, "{text}");
    assert_eq!(text.matches("tile 3,5: damaged data").count(), 1);
    // Nothing summed up yet: the window (10 s) is not over.
    assert!(!text.contains("more times"), "{text}");
}
