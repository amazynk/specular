//! The About box: version, licence and the attributions the bundled data requires
//! (GeoNames place names are CC BY 4.0 and must be credited where users can see it; the map's
//! Natural Earth data is public domain and credited all the same).

use specular_gfx::Rect;
use specular_i18n::t;
use specular_theme::metrics;

use crate::Chrome;
use crate::ctx::{Ctx, HAlign, Target};

/// Year in the copyright line.
const COPYRIGHT_YEAR: i64 = 2026;

/// The GeoNames attribution in the UI language (the metadata catalogue's
/// `geonames-attribution`, whose English text is [`specular_metadata::GEONAMES_ATTRIBUTION`]).
pub(crate) fn geonames_attribution() -> String {
    let lang = specular_i18n::I18n::global().language().to_string();
    match lang.parse() {
        Ok(l) => specular_metadata::Localizer::new(l).text("geonames-attribution"),
        Err(_) => specular_metadata::GEONAMES_ATTRIBUTION.to_owned(),
    }
}

/// "© 2026 The Specular authors": the year without grouping ("2,026") in every language, in
/// its digits.
pub(crate) fn copyright_line() -> String {
    let year = specular_i18n::I18n::global()
        .localizer()
        .integer_plain(COPYRIGHT_YEAR);
    t!("about-copyright", year = year)
}

/// "Build 4a3c0658 2026-10-11 · release · x86_64-linux".
pub(crate) fn build_line() -> String {
    let b = specular_help::build_info();
    if b.commit.is_empty() {
        t!(
            "about-build-no-commit",
            profile = b.profile,
            target = b.target
        )
    } else {
        t!(
            "about-build",
            commit = b.commit,
            profile = b.profile,
            target = b.target
        )
    }
}

/// The third-party components, one paragraph per licence: "MIT OR Apache-2.0 (312)" over
/// the crates' names.
pub(crate) fn licence_lines() -> Vec<(String, String)> {
    specular_help::third_party_by_licence()
        .into_iter()
        .map(|(licence, crates)| {
            let head = t!(
                "about-licence-group",
                licence = licence,
                count = crates.len() as i64
            );
            let names: Vec<String> = crates
                .iter()
                .map(|c| format!("{} {}", c.name, c.version))
                .collect();
            (head, names.join(", "))
        })
        .collect()
}

pub(crate) fn draw(ch: &mut Chrome, cx: &mut Ctx) {
    if ch.about_geonames.is_empty() {
        ch.about_geonames = geonames_attribution();
    }
    let w = (cx.w - 48.0).min(cx.u(560.0));
    let inner_w = w - 48.0;
    let title_s = cx.style(metrics::FONT_TITLE).weight(650.0);
    let body_s = cx.style(metrics::FONT_BODY);
    let small_s = cx.style(metrics::FONT_SMALL);
    let head_s = cx.style(metrics::FONT_SMALL).weight(650.0);

    // The title keeps clear of the Discord and Close buttons.
    let title = cx.layout(&t!("about-title"), title_s, Some(inner_w - 76.0));
    let version = cx.layout(
        &format!(
            "{}\n{}",
            t!("about-version", version = env!("CARGO_PKG_VERSION")),
            ch.about_build.clone().unwrap_or_else(build_line)
        ),
        small_s,
        Some(inner_w),
    );
    let storage = cx.layout(&ch.about_storage, small_s, Some(inner_w));
    let tagline = cx.layout(&t!("about-tagline"), body_s, Some(inner_w));
    let copyright = cx.layout(&copyright_line(), small_s, Some(inner_w));
    let license = cx.layout(&t!("about-license"), small_s, Some(inner_w));
    let head = cx.layout(&t!("about-third-party"), head_s, Some(inner_w));
    let geonames = cx.layout(&ch.about_geonames, small_s, Some(inner_w));
    let natural_earth = cx.layout(&t!("about-natural-earth"), small_s, Some(inner_w));
    let unicode = cx.layout(&t!("about-unicode"), small_s, Some(inner_w));
    let lensfun = cx.layout(&t!("about-lensfun"), small_s, Some(inner_w));
    let file = cx.layout(&t!("about-third-party-file"), small_s, Some(inner_w));

    let has_storage = !ch.about_storage.is_empty();
    let blocks = [
        (&title, 4.0),
        (&version, if has_storage { 2.0 } else { 14.0 }),
        (&storage, 14.0),
        (&tagline, 14.0),
        (&copyright, 2.0),
        (&license, 20.0),
        (&head, 8.0),
        (&geonames, 8.0),
        (&natural_earth, 8.0),
        (&unicode, 8.0),
        (&lensfun, 8.0),
        (&file, 0.0),
    ];
    // Without a storage line its block takes no room.
    let blocks = blocks.map(|(b, gap)| {
        if std::ptr::eq(b, &storage) && !has_storage {
            (b, 0.0, false)
        } else {
            (b, gap, true)
        }
    });
    // The licences of the third-party components, each group on its own paragraph, and
    // where to report problems.
    let count = specular_help::third_party().len() as i64;
    let mut extra = vec![(
        cx.layout(
            &t!("about-licences-count", count = count),
            head_s,
            Some(inner_w),
        ),
        8.0,
        cx.pal.text_faint,
    )];
    for (head, names) in licence_lines() {
        extra.push((
            cx.layout(&head, head_s, Some(inner_w)),
            2.0,
            cx.pal.text_muted,
        ));
        extra.push((
            cx.layout(&names, small_s, Some(inner_w)),
            10.0,
            cx.pal.text_faint,
        ));
    }
    if let Some((issues, email)) = &ch.contact {
        let c = t!(
            "about-contact",
            issues = issues.clone(),
            email = email.clone()
        );
        extra.push((
            cx.layout(&c, small_s, Some(inner_w)),
            0.0,
            cx.pal.text_muted,
        ));
    }
    let extra_h: f32 = extra.iter().map(|(b, gap, _)| b.height + gap).sum::<f32>() + 20.0;

    let content: f32 = blocks
        .iter()
        .filter(|(_, _, shown)| *shown)
        .map(|(b, gap, _)| b.height + gap)
        .sum::<f32>()
        + 20.0
        + extra_h;
    // Help and Save diagnostics bundle… under the scrolling text.
    let label_s = cx.style(metrics::FONT_LABEL);
    let help_b = cx.layout(&t!("about-help"), label_s, None);
    let diag_b = cx.layout(&t!("about-diagnostics"), label_s, None);
    let rend_b = cx.layout(&t!("about-renderer"), label_s, None);
    let bh = cx.u(32.0);
    let h = (content + 48.0 + bh + 16.0).min(cx.h - 32.0);
    let r = Rect::new(((cx.w - w) * 0.5).round(), ((cx.h - h) * 0.5).round(), w, h);
    cx.sheet(r);
    let close = cx.m_in(Rect::new(r.right() - 46.0, r.y + 14.0, 32.0, 32.0), r);
    cx.close_button(close);
    // The Discord button beside Close, as small as it.
    let discord = cx.m_in(
        Rect::new(r.right() - 46.0 - 36.0, r.y + 14.0, 32.0, 32.0),
        r,
    );
    cx.icon_button(
        discord,
        specular_gfx::Icon::Discord,
        Target::AboutDiscord,
        cx.pal.text_muted,
        false,
        false,
    );
    let by = r.bottom() - 20.0 - bh;
    let dw = diag_b.width + 24.0;
    let hw = help_b.width + 24.0;
    let rw = rend_b.width + 24.0;
    let dr = cx.m_in(Rect::new(r.right() - 24.0 - dw, by, dw, bh), r);
    let rr = cx.m_in(Rect::new(r.right() - 24.0 - dw - 8.0 - rw, by, rw, bh), r);
    let hr = cx.m_in(
        Rect::new(r.right() - 24.0 - dw - 8.0 - rw - 8.0 - hw, by, hw, bh),
        r,
    );
    for (b, br, t) in [
        (&help_b, hr, Target::AboutHelp),
        (&rend_b, rr, Target::AboutRenderer),
        (&diag_b, dr, Target::AboutDiagnostics),
    ] {
        crate::insights::button(cx, br, b, t.clone(), false);
        if cx.is_focused(&t) {
            cx.button_ring(br, metrics::RADIUS_SM + 2.0);
        }
    }

    let body = Rect::new(r.x + 24.0, r.y + 24.0, inner_w, by - 16.0 - (r.y + 24.0));
    let max = (content - body.h).max(0.0);
    ch.about_scroll = ch.about_scroll.clamp(0.0, max);
    cx.dl.push_clip(body);
    let mut y = body.y - ch.about_scroll;
    for (i, (b, gap, shown)) in blocks.iter().enumerate() {
        if !shown {
            continue;
        }
        let color = match i {
            0 => cx.pal.text,
            3 => cx.pal.text,
            6 => cx.pal.text_faint,
            _ => cx.pal.text_muted,
        };
        let br = Rect::new(
            body.x,
            y,
            if i == 0 { inner_w - 40.0 } else { inner_w },
            b.height,
        );
        let br = cx.m_in(br, body);
        cx.block_top(b, br, HAlign::Start, color);
        if i == 5 {
            cx.dl.rect(
                Rect::new(body.x, y + b.height + gap * 0.5, inner_w, 1.0),
                0.0,
                cx.pal.separator,
            );
        }
        y += b.height + gap;
    }
    y += 20.0;
    cx.dl.rect(
        Rect::new(body.x, y - 10.0, inner_w, 1.0),
        0.0,
        cx.pal.separator,
    );
    for (b, gap, color) in &extra {
        let br = cx.m_in(Rect::new(body.x, y, inner_w, b.height), body);
        cx.block_top(b, br, HAlign::Start, *color);
        y += b.height + gap;
    }
    cx.dl.pop_clip();
    crate::scrollbar::draw(
        ch,
        cx,
        crate::scrollbar::Area::About,
        body,
        Rect::new(r.x, body.y, r.w, body.h),
        max,
    );
}

#[cfg(test)]
mod tests {
    use specular_i18n::{I18n, langid};

    /// The year is never grouped ("© 2,026"), whatever the language's number format.
    #[test]
    fn the_copyright_year_has_no_grouping() {
        for lang in [
            langid!("en-US"),
            langid!("he"),
            langid!("de"),
            langid!("fr"),
        ] {
            let line = I18n::with_thread_language(lang.clone(), super::copyright_line);
            // Not "2,026", "2.026" or "2 026" (de and fr group with a dot, a narrow space).
            assert!(line.contains("2026"), "{lang}: {line}");
        }
    }
}
