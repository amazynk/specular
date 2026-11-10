//! The gallery's page: `index.html` with the photos in it, one stylesheet and one small
//! script, all in the folder. The markup is polyglot XHTML (every element closed, every
//! attribute quoted), so it parses as HTML and as XML alike. Nothing points outside the
//! folder: no fonts, no CDN, no analytics; links are relative, so the page works from
//! `file://` and from any static host.
//!
//! Without the script the grid still shows and each tile opens its photo; the script adds the
//! lightbox (←/→, Home/End, Esc, P to pick; swipe on touch screens) and the picks: a heart per
//! photo, kept in the browser's local storage under the gallery's id, sent back as a copied
//! list or a downloaded `picks.txt` / `picks.json` ([`crate::picks`] reads both).

use std::fmt::Write as _;
use std::io;
use std::path::Path;

use crate::{Captions, Theme, Words};

/// One photo on the page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    /// The original file name (shown).
    pub name: String,
    /// What the client's picks call it: the file name, or `folder/name` when another photo
    /// of the gallery has that name too.
    pub key: String,
    /// The file in `images/` and `thumbs/` ("img_0042.jpg").
    pub file: String,
    pub width: u32,
    pub height: u32,
    pub thumb_width: u32,
    pub thumb_height: u32,
    pub title: Option<String>,
    pub caption: Option<String>,
    /// EXIF summary lines (empty: none shown).
    pub exif: Vec<String>,
}

/// Everything the page shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Site {
    pub title: String,
    /// Keys the client's picks in the browser, so two galleries never share them.
    pub id: String,
    pub theme: Theme,
    pub captions: Captions,
    pub entries: Vec<Entry>,
    pub words: Words,
}

/// The stylesheet's and script's paths in the folder.
pub const CSS_PATH: &str = "assets/gallery.css";
pub const JS_PATH: &str = "assets/gallery.js";

/// `s` with `&`, `<`, `>`, `"` and `'` escaped (text and attribute values).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            // Control characters are not allowed in XML; tabs and newlines are.
            c if c.is_control() && !matches!(c, '\t' | '\n') => out.push(' '),
            c => out.push(c),
        }
    }
    out
}

/// A file name or path segment as a relative URL (UTF-8 percent-encoded but for the
/// unreserved characters and `/`).
pub fn url_path(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~' | b'/') {
            out.push(b as char);
        } else {
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}

/// What shows under a photo, as lines (the first one bold when there are several).
fn caption_lines(e: &Entry, c: Captions) -> Vec<String> {
    let clean = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    match c {
        Captions::Off => Vec::new(),
        Captions::FileName => vec![e.name.clone()],
        Captions::Title => clean(&e.title).into_iter().collect(),
        Captions::All => std::iter::once(e.name.clone())
            .chain(clean(&e.title))
            .chain(clean(&e.caption))
            .collect(),
    }
}

/// The heart, drawn by the stylesheet in the text colour (filled when picked).
const HEART: &str = "<svg viewBox=\"0 0 24 24\" aria-hidden=\"true\" focusable=\"false\"><path d=\"M12 20.5s-7.4-4.5-9.4-9C1.2 8.2 3.2 4.5 6.9 4.5c2 0 3.6 1.1 5.1 2.9 1.5-1.8 3.1-2.9 5.1-2.9 3.7 0 5.7 3.7 4.3 7-2 4.5-9.4 9-9.4 9z\"/></svg>";

/// The page.
pub fn index_html(site: &Site) -> String {
    let w = &site.words;
    let e = escape;
    let mut h = String::with_capacity(4096 + site.entries.len() * 900);
    let dir = if w.rtl { "rtl" } else { "ltr" };
    let _ = write!(
        h,
        "<!DOCTYPE html>\n<html lang=\"{}\" dir=\"{dir}\">\n<head>\n\
         <meta charset=\"utf-8\"/>\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\n\
         <meta name=\"referrer\" content=\"no-referrer\"/>\n\
         <meta name=\"robots\" content=\"noindex, nofollow\"/>\n\
         <title>{}</title>\n\
         <link rel=\"stylesheet\" href=\"{CSS_PATH}\"/>\n\
         </head>\n",
        e(&w.lang),
        e(&site.title),
    );
    let _ = writeln!(
        h,
        "<body class=\"theme-{}\" data-gallery=\"{}\" data-title=\"{}\" data-l-picked=\"{}\" \
         data-l-pick=\"{}\" data-l-copied=\"{}\" data-l-copy-hand=\"{}\" data-l-clear-ask=\"{}\" \
         data-l-no-storage=\"{}\" data-l-position=\"{}\">",
        site.theme.id(),
        e(&site.id),
        e(&site.title),
        e(&w.picked),
        e(&w.pick),
        e(&w.copied),
        e(&w.copy_by_hand),
        e(&w.clear_ask),
        e(&w.no_storage),
        e(&w.position),
    );
    // The header: the title, the count, the picks bar.
    let _ = write!(
        h,
        "<header>\n<h1>{}</h1>\n<p class=\"count\">{}</p>\n\
         <p class=\"hint\">{}</p>\n\
         <div class=\"picks-bar\" role=\"region\" aria-label=\"{}\">\n\
         <span class=\"picks-count\" aria-live=\"polite\"></span>\n\
         <button type=\"button\" data-act=\"copy\" data-needs-picks=\"1\">{}</button>\n\
         <button type=\"button\" data-act=\"txt\" data-needs-picks=\"1\">{}</button>\n\
         <button type=\"button\" data-act=\"json\" data-needs-picks=\"1\">{}</button>\n\
         <button type=\"button\" data-act=\"clear\" data-needs-picks=\"1\">{}</button>\n\
         </div>\n\
         <p class=\"picks-note\" aria-live=\"polite\"></p>\n\
         <textarea class=\"picks-list\" readonly=\"readonly\" rows=\"6\" hidden=\"hidden\" aria-label=\"{}\"></textarea>\n\
         <noscript><p class=\"hint\">{}</p></noscript>\n\
         </header>\n",
        e(&site.title),
        e(&w.count),
        e(&w.hint),
        e(&w.picks),
        e(&w.copy),
        e(&w.download_txt),
        e(&w.download_json),
        e(&w.clear),
        e(&w.picks),
        e(&w.noscript),
    );
    h.push_str("<main>\n<ul class=\"grid\">\n");
    for en in &site.entries {
        let lines = caption_lines(en, site.captions);
        let alt = en
            .title
            .as_deref()
            .or(en.caption.as_deref())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(&en.name);
        let src = url_path(&format!("images/{}", en.file));
        let thumb = url_path(&format!("thumbs/{}", en.file));
        let _ = write!(
            h,
            "<li class=\"tile\" data-name=\"{}\" data-src=\"{src}\" data-w=\"{}\" data-h=\"{}\" \
             data-label=\"{}\" data-exif=\"{}\">\n\
             <a class=\"open\" href=\"{src}\"><img src=\"{thumb}\" width=\"{}\" height=\"{}\" \
             alt=\"{}\" loading=\"lazy\" decoding=\"async\"/></a>\n\
             <button type=\"button\" class=\"pick\" aria-pressed=\"false\" aria-label=\"{}\" \
             title=\"{}\">{HEART}</button>\n",
            e(&en.key),
            en.width,
            en.height,
            e(&lines.join(" — ")),
            e(&en.exif.join(" · ")),
            en.thumb_width,
            en.thumb_height,
            e(alt),
            e(&format!("{}: {}", w.pick, en.name)),
            e(&w.pick),
        );
        if !lines.is_empty() {
            h.push_str("<p class=\"cap\">");
            for (i, l) in lines.iter().enumerate() {
                if i == 0 && lines.len() > 1 {
                    let _ = write!(h, "<b>{}</b>", e(l));
                } else {
                    let _ = write!(h, "<span>{}</span>", e(l));
                }
            }
            h.push_str("</p>\n");
        }
        h.push_str("</li>\n");
    }
    h.push_str("</ul>\n</main>\n");
    // The lightbox: previous and next point the reading way.
    let (prev_glyph, next_glyph) = if w.rtl {
        ("›", "‹")
    } else {
        ("‹", "›")
    };
    let _ = write!(
        h,
        "<div class=\"lightbox\" role=\"dialog\" aria-modal=\"true\" aria-label=\"{}\" hidden=\"hidden\">\n\
         <div class=\"lb-stage\"><img class=\"lb-img\" alt=\"\"/></div>\n\
         <div class=\"lb-bar\">\n\
         <button type=\"button\" class=\"lb-prev\" aria-label=\"{}\" title=\"{}\">{prev_glyph}</button>\n\
         <button type=\"button\" class=\"lb-next\" aria-label=\"{}\" title=\"{}\">{next_glyph}</button>\n\
         <div class=\"lb-text\"><p class=\"lb-cap\"></p><p class=\"lb-exif\"></p><p class=\"lb-pos\"></p></div>\n\
         <button type=\"button\" class=\"lb-pick pick\" aria-pressed=\"false\" aria-label=\"{}\" title=\"{} (P)\">{HEART}</button>\n\
         <button type=\"button\" class=\"lb-close\" aria-label=\"{}\" title=\"{} (Esc)\">×</button>\n\
         </div>\n</div>\n\
         <script src=\"{JS_PATH}\"></script>\n</body>\n</html>\n",
        e(&w.viewer),
        e(&w.previous),
        e(&w.previous),
        e(&w.next),
        e(&w.next),
        e(&w.pick),
        e(&w.pick),
        e(&w.close),
        e(&w.close),
    );
    h
}

/// The stylesheet: three themes on custom properties, a responsive grid, the lightbox.
pub const CSS: &str = r##"/* Specular client gallery. System fonts only; nothing is loaded from elsewhere. */
:root{--bg:#fff;--fg:#1b1b1f;--muted:#5f6068;--line:#dcdce2;--tile:#f2f2f5;--accent:#c8325a;--on-accent:#fff;--btn:#fff;--btn-fg:#1b1b1f;--gap:12px;--radius:8px}
body.theme-dark{--bg:#131316;--fg:#ececf1;--muted:#a6a6b0;--line:#33333b;--tile:#1d1d22;--accent:#ff6d8f;--on-accent:#1b0d12;--btn:#24242a;--btn-fg:#ececf1}
body.theme-minimal{--bg:#fff;--fg:#111;--muted:#6b6b6b;--line:#cfcfcf;--tile:transparent;--accent:#111;--on-accent:#fff;--btn:transparent;--btn-fg:#111;--gap:32px;--radius:0}
*{box-sizing:border-box}
html{-webkit-text-size-adjust:100%;text-size-adjust:100%}
body{margin:0;background:var(--bg);color:var(--fg);font:16px/1.45 system-ui,-apple-system,"Segoe UI",Roboto,"Noto Sans",Arial,sans-serif}
header,main{max-width:1600px;margin:0 auto;padding:0 16px}
header{padding-top:24px}
h1{font-size:1.6rem;line-height:1.2;margin:0 0 4px;font-weight:650;overflow-wrap:anywhere}
.theme-minimal h1{font-weight:300;letter-spacing:.02em}
.count,.hint,.picks-note{color:var(--muted);margin:0 0 8px}
.picks-note:empty{display:none}
.picks-bar{display:flex;flex-wrap:wrap;gap:8px;align-items:center;position:sticky;top:0;z-index:2;background:var(--bg);padding:8px 0}
.picks-count{font-weight:600;margin-inline-end:8px}
button{font:inherit;color:var(--btn-fg);background:var(--btn);border:1px solid var(--line);border-radius:6px;padding:6px 12px;cursor:pointer;min-height:36px}
button:disabled{opacity:.45;cursor:default}
a:focus-visible,button:focus-visible,textarea:focus-visible{outline:2px solid var(--accent);outline-offset:2px}
.picks-list{display:block;width:100%;margin:0 0 12px;font:inherit;color:var(--fg);background:var(--tile);border:1px solid var(--line);border-radius:6px;padding:8px}
.picks-list[hidden]{display:none}
main{padding-top:8px;padding-bottom:48px}
.grid{list-style:none;margin:0;padding:0;display:grid;gap:var(--gap);grid-template-columns:repeat(auto-fill,minmax(min(220px,100%),1fr))}
.tile{position:relative;background:var(--tile);border-radius:var(--radius);overflow:hidden}
.tile .open{display:block;aspect-ratio:1/1}
.tile img{display:block;width:100%;height:100%;object-fit:contain}
.pick{position:absolute;top:8px;inset-inline-end:8px;width:44px;height:44px;min-height:0;padding:9px;border:0;border-radius:50%;background:rgba(0,0,0,.45);color:#fff}
.pick svg{display:block;width:100%;height:100%;fill:none;stroke:currentColor;stroke-width:2;stroke-linejoin:round}
.pick[aria-pressed="true"]{background:var(--accent);color:var(--on-accent)}
.pick[aria-pressed="true"] svg{fill:currentColor}
.tile.picked{box-shadow:inset 0 0 0 3px var(--accent)}
.cap{margin:0;padding:6px 10px 10px;font-size:.9rem;color:var(--muted);overflow-wrap:anywhere}
.cap b,.cap span{display:block}
.cap b{color:var(--fg);font-weight:600}
.theme-minimal .cap{padding-inline:0}
body.lb-open{overflow:hidden}
.lightbox{position:fixed;inset:0;z-index:10;display:flex;flex-direction:column;background:rgba(8,8,10,.94);color:#f2f2f2}
.lightbox[hidden]{display:none}
.lb-stage{flex:1;min-height:0;display:flex;align-items:center;justify-content:center;padding:12px}
.lb-img{max-width:100%;max-height:100%;width:auto;height:auto;object-fit:contain}
.lb-bar{display:flex;flex-wrap:wrap;gap:8px;align-items:center;padding:8px 16px 16px}
.lb-text{flex:1 1 220px;min-width:0}
.lb-text p{margin:0;overflow-wrap:anywhere}
.lb-exif,.lb-pos{color:#b8b8c0;font-size:.85rem}
.lb-exif:empty{display:none}
.lightbox button{color:#f2f2f2;background:rgba(255,255,255,.14);border-color:transparent;min-width:44px;min-height:44px;font-size:1.2rem}
.lightbox .lb-pick{position:static}
.lightbox .lb-pick[aria-pressed="true"]{background:var(--accent);color:var(--on-accent)}
@media (max-width:600px){h1{font-size:1.3rem}.grid{grid-template-columns:repeat(2,minmax(0,1fr));gap:6px}.theme-minimal .grid{gap:16px}}
@media print{.picks-bar,.pick,.hint,.picks-note,.picks-list{display:none}}
"##;

/// The script: picks in local storage, sending them back, the lightbox. Plain ES5, no
/// libraries, nothing fetched.
pub const JS: &str = r##"/* Specular client gallery: picks stay in this browser; nothing is sent anywhere. */
(function () {
  "use strict";
  var body = document.body, d = body.dataset;
  var key = "specular-picks:" + (d.gallery || "");
  var store = null;
  try { store = window.localStorage; store.getItem(key); } catch (e) { store = null; }
  var tiles = Array.prototype.slice.call(document.querySelectorAll(".grid .tile"));
  var names = tiles.map(function (t) { return t.getAttribute("data-name"); });
  var picks = {};
  var rtl = document.documentElement.getAttribute("dir") === "rtl";

  function fill(t, map) {
    return String(t || "").replace(/\{(\w+)\}/g, function (m, k) { return k in map ? String(map[k]) : m; });
  }
  function list() { return names.filter(function (n) { return picks[n]; }); }
  function load() {
    if (!store) { return; }
    try {
      var v = JSON.parse(store.getItem(key) || "[]");
      if (Array.isArray(v)) { v.forEach(function (n) { if (typeof n === "string") { picks[n] = true; } }); }
    } catch (e) { /* a damaged entry: start empty */ }
  }
  function save() {
    if (!store) { return; }
    try { store.setItem(key, JSON.stringify(list())); } catch (e) { /* full or blocked */ }
  }

  var countEl = document.querySelector(".picks-count");
  var noteEl = document.querySelector(".picks-note");
  var listEl = document.querySelector(".picks-list");
  var needs = Array.prototype.slice.call(document.querySelectorAll("[data-needs-picks]"));
  function say(s) { noteEl.textContent = s || ""; }

  var lb = document.querySelector(".lightbox");
  var lbImg = lb.querySelector(".lb-img"), lbCap = lb.querySelector(".lb-cap");
  var lbExif = lb.querySelector(".lb-exif"), lbPos = lb.querySelector(".lb-pos");
  var lbPick = lb.querySelector(".lb-pick");
  var cur = -1;

  function render() {
    tiles.forEach(function (t, i) {
      var on = !!picks[names[i]];
      t.classList.toggle("picked", on);
      t.querySelector(".pick").setAttribute("aria-pressed", on ? "true" : "false");
    });
    var n = list().length;
    countEl.textContent = fill(d.lPicked, { n: n });
    needs.forEach(function (b) { b.disabled = n === 0; });
    if (cur >= 0) { lbPick.setAttribute("aria-pressed", picks[names[cur]] ? "true" : "false"); }
  }
  function toggle(i) {
    var n = names[i];
    if (picks[n]) { delete picks[n]; } else { picks[n] = true; }
    save();
    render();
  }

  function asText() {
    return "# Specular client picks\n# gallery: " + (d.title || "") + "\n# id: " + (d.gallery || "") +
      "\n" + list().join("\n") + "\n";
  }
  function asJson() {
    return JSON.stringify({ format: "specular-picks", version: 1, gallery: d.title || "",
      id: d.gallery || "", picks: list() }, null, 2) + "\n";
  }
  function download(name, text, type) {
    var a = document.createElement("a");
    a.href = URL.createObjectURL(new Blob([text], { type: type }));
    a.download = name;
    body.appendChild(a);
    a.click();
    setTimeout(function () { URL.revokeObjectURL(a.href); body.removeChild(a); }, 1000);
  }
  function byHand() {
    listEl.hidden = false;
    listEl.focus();
    listEl.select();
    var ok = false;
    try { ok = document.execCommand("copy"); } catch (e) { ok = false; }
    say(ok ? d.lCopied : d.lCopyHand);
  }
  function copy() {
    listEl.value = list().join("\n");
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(listEl.value).then(function () {
        listEl.hidden = false;
        say(d.lCopied);
      }, byHand);
    } else {
      byHand();
    }
  }
  document.querySelector(".picks-bar").addEventListener("click", function (e) {
    var b = e.target.closest ? e.target.closest("button") : null;
    if (!b || b.disabled) { return; }
    var act = b.getAttribute("data-act");
    if (act === "copy") { copy(); }
    else if (act === "txt") { download("picks.txt", asText(), "text/plain;charset=utf-8"); }
    else if (act === "json") { download("picks.json", asJson(), "application/json"); }
    else if (act === "clear" && window.confirm(d.lClearAsk)) {
      picks = {};
      save();
      listEl.hidden = true;
      say("");
      render();
    }
  });

  function show(i) {
    cur = (i + tiles.length) % tiles.length;
    var t = tiles[cur];
    lbImg.src = t.getAttribute("data-src");
    lbImg.alt = t.querySelector("img").getAttribute("alt") || "";
    lbImg.setAttribute("width", t.getAttribute("data-w"));
    lbImg.setAttribute("height", t.getAttribute("data-h"));
    lbCap.textContent = t.getAttribute("data-label") || "";
    lbExif.textContent = t.getAttribute("data-exif") || "";
    lbPos.textContent = fill(d.lPosition, { i: cur + 1, n: tiles.length });
    lbPick.setAttribute("aria-label", d.lPick + ": " + names[cur]);
    render();
    [cur + 1, cur - 1].forEach(function (k) {
      var n = tiles[(k + tiles.length) % tiles.length];
      if (n) { (new Image()).src = n.getAttribute("data-src"); }
    });
  }
  function open(i) {
    lb.hidden = false;
    body.classList.add("lb-open");
    show(i);
    lb.querySelector(".lb-close").focus();
  }
  function close() {
    var t = tiles[cur];
    lb.hidden = true;
    body.classList.remove("lb-open");
    lbImg.removeAttribute("src");
    cur = -1;
    if (t) { t.querySelector(".open").focus(); }
  }

  tiles.forEach(function (t, i) {
    t.querySelector(".pick").addEventListener("click", function (e) { e.preventDefault(); toggle(i); });
    t.querySelector(".open").addEventListener("click", function (e) {
      if (e.ctrlKey || e.metaKey || e.shiftKey || e.altKey || e.button) { return; }
      e.preventDefault();
      open(i);
    });
  });
  lb.querySelector(".lb-prev").addEventListener("click", function () { show(cur - 1); });
  lb.querySelector(".lb-next").addEventListener("click", function () { show(cur + 1); });
  lb.querySelector(".lb-close").addEventListener("click", close);
  lbPick.addEventListener("click", function () { toggle(cur); });
  lb.querySelector(".lb-stage").addEventListener("click", function (e) { if (e.target !== lbImg) { close(); } });

  document.addEventListener("keydown", function (e) {
    if (lb.hidden || e.ctrlKey || e.metaKey || e.altKey) { return; }
    var k = e.key;
    if (k === "Escape") { close(); }
    else if (k === "ArrowLeft") { show(rtl ? cur + 1 : cur - 1); }
    else if (k === "ArrowRight") { show(rtl ? cur - 1 : cur + 1); }
    else if (k === "Home") { show(0); }
    else if (k === "End") { show(tiles.length - 1); }
    else if (k === "p" || k === "P") { toggle(cur); }
    else if (k === "Tab") {
      // Focus stays in the viewer while it is open.
      var f = Array.prototype.slice.call(lb.querySelectorAll("button"));
      var at = f.indexOf(document.activeElement);
      var next = e.shiftKey ? (at <= 0 ? f.length - 1 : at - 1) : (at + 1) % f.length;
      f[next].focus();
    } else { return; }
    e.preventDefault();
  });

  var x0 = null;
  lb.addEventListener("touchstart", function (e) { x0 = e.touches.length === 1 ? e.touches[0].clientX : null; }, { passive: true });
  lb.addEventListener("touchend", function (e) {
    if (x0 === null || !e.changedTouches.length) { return; }
    var dx = e.changedTouches[0].clientX - x0;
    x0 = null;
    if (Math.abs(dx) > 50) { show((dx < 0) !== rtl ? cur + 1 : cur - 1); }
  });

  load();
  if (!store) { say(d.lNoStorage); }
  render();
})();
"##;

/// Writes the page, its stylesheet and script into `dir` (made when missing).
pub fn write(dir: &Path, site: &Site) -> io::Result<()> {
    std::fs::create_dir_all(dir.join("assets"))?;
    std::fs::write(dir.join(CSS_PATH), CSS)?;
    std::fs::write(dir.join(JS_PATH), JS)?;
    std::fs::write(dir.join("index.html"), index_html(site))
}

/// `s` as a safe file name stem: letters, digits, `-` and `_` kept (non-ASCII letters too),
/// anything else `_`, at most 80 characters, never empty or a dot name.
pub fn file_stem(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(80)
        .collect();
    while out.ends_with('_') && out.len() > 1 {
        out.pop();
    }
    if out.is_empty() || out.chars().all(|c| c == '_') {
        out = "photo".into();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_text_and_urls() {
        assert_eq!(
            escape(r#"a<b & "c" 'd'>"#),
            "a&lt;b &amp; &quot;c&quot; &#39;d&#39;&gt;"
        );
        assert_eq!(escape("a\u{1}b"), "a b");
        assert_eq!(url_path("images/a b#1.jpg"), "images/a%20b%231.jpg");
        assert_eq!(
            url_path("images/שלום.jpg"),
            "images/%D7%A9%D7%9C%D7%95%D7%9D.jpg"
        );
    }

    #[test]
    fn stems_are_safe() {
        assert_eq!(file_stem("IMG_0042"), "IMG_0042");
        assert_eq!(file_stem("../etc/passwd"), "___etc_passwd");
        assert_eq!(file_stem("a b.c"), "a_b_c");
        assert_eq!(file_stem("..."), "photo");
        assert_eq!(file_stem(""), "photo");
        assert_eq!(file_stem("קיץ 2026"), "קיץ_2026");
    }

    #[test]
    fn captions_follow_the_choice() {
        let e = Entry {
            name: "IMG_1.CR3".into(),
            key: "IMG_1.CR3".into(),
            file: "IMG_1.jpg".into(),
            width: 10,
            height: 10,
            thumb_width: 5,
            thumb_height: 5,
            title: Some("Dunes".into()),
            caption: Some(" ".into()),
            exif: Vec::new(),
        };
        assert!(caption_lines(&e, Captions::Off).is_empty());
        assert_eq!(caption_lines(&e, Captions::FileName), ["IMG_1.CR3"]);
        assert_eq!(caption_lines(&e, Captions::Title), ["Dunes"]);
        assert_eq!(caption_lines(&e, Captions::All), ["IMG_1.CR3", "Dunes"]);
    }
}
