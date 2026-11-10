//! Conservative editing of Thunar's custom actions file (`~/.config/Thunar/uca.xml`).
//!
//! The file is not parsed into a tree and written back (that would reorder or drop what the
//! user has). Instead the top-level `<action>…</action>` blocks are located as text, comments
//! skipped; ours are recognised by a `<unique-id>` starting with [`ID_PREFIX`]; only those
//! blocks are removed or replaced, and new ones go right before `</actions>`. Everything else
//! stays byte for byte. A file we cannot make sense of is left alone with an error.

/// `<unique-id>` prefix of our actions.
pub const ID_PREFIX: &str = "specular-";

/// One custom action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Action {
    /// Suffix of the unique id (`specular-<id>`).
    pub id: &'static str,
    pub name: String,
    pub description: String,
    pub icon: String,
    /// Command line; `%f` is the file, `%F` the selected files.
    pub command: String,
    /// `;`-separated name patterns (`*.jpg;*.png`).
    pub patterns: String,
}

/// An empty file, as Thunar writes it.
const EMPTY: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n</actions>\n";

/// XML text escape.
pub fn escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => o.push_str("&amp;"),
            '<' => o.push_str("&lt;"),
            '>' => o.push_str("&gt;"),
            '"' => o.push_str("&quot;"),
            '\'' => o.push_str("&apos;"),
            '\n' | '\r' => o.push(' '),
            c => o.push(c),
        }
    }
    o
}

fn block(a: &Action) -> String {
    format!(
        "<action>\n\
         \t<icon>{}</icon>\n\
         \t<name>{}</name>\n\
         \t<submenu></submenu>\n\
         \t<unique-id>{ID_PREFIX}{}</unique-id>\n\
         \t<command>{}</command>\n\
         \t<description>{}</description>\n\
         \t<patterns>{}</patterns>\n\
         \t<image-files/>\n\
         </action>\n",
        escape(&a.icon),
        escape(&a.name),
        a.id,
        escape(&a.command),
        escape(&a.description),
        escape(&a.patterns),
    )
}

/// Byte ranges of the top-level `<action>` blocks (start of the tag to the end of
/// `</action>`), and the offset of the closing `</actions>`. Comments and CDATA are skipped.
fn scan(text: &str) -> Result<(Vec<(usize, usize)>, usize), String> {
    let mut blocks = Vec::new();
    let mut open: Option<usize> = None;
    let mut root_open = false;
    let mut end = None;
    let mut i = 0;
    let b = text.as_bytes();
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &text[i..];
        let skip_to = |pat: &str| rest.find(pat).map(|n| i + n + pat.len());
        if rest.starts_with("<!--") {
            i = skip_to("-->").ok_or("unterminated comment")?;
            continue;
        }
        if rest.starts_with("<![CDATA[") {
            i = skip_to("]]>").ok_or("unterminated CDATA")?;
            continue;
        }
        if rest.starts_with("<?") || rest.starts_with("<!") {
            i = skip_to(">").ok_or("unterminated declaration")?;
            continue;
        }
        let close = rest.find('>').ok_or("unterminated tag")?;
        let tag = &rest[1..close];
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("");
        let closing = tag.starts_with('/');
        let empty = tag.ends_with('/');
        match (name, closing) {
            ("actions", false) if !root_open && end.is_none() => root_open = !empty,
            ("actions", true) if root_open && open.is_none() => {
                end = Some(i);
                root_open = false;
            }
            ("action", false) if root_open && open.is_none() => {
                if empty {
                    blocks.push((i, i + close + 1));
                } else {
                    open = Some(i);
                }
            }
            ("action", true) if open.is_some() => {
                blocks.push((open.take().unwrap_or(i), i + close + 1));
            }
            ("actions", _) | ("action", _) => return Err(format!("unexpected <{tag}>")),
            _ if !root_open && end.is_none() && !closing => {
                return Err(format!("root element <{name}> is not <actions>"));
            }
            _ => {}
        }
        i += close + 1;
    }
    if open.is_some() {
        return Err("unterminated <action>".into());
    }
    let end = end.ok_or("no </actions>")?;
    Ok((blocks, end))
}

fn is_ours(block: &str) -> bool {
    block.contains(&format!("<unique-id>{ID_PREFIX}"))
}

/// Extends a block range to whole lines when the block sits alone on its lines (so removal
/// leaves no blank line behind).
fn whole_lines(text: &str, (s, e): (usize, usize)) -> (usize, usize) {
    let line_start = text[..s].rfind('\n').map_or(0, |n| n + 1);
    let before_blank = text[line_start..s].trim().is_empty();
    let line_end = text[e..].find('\n').map_or(text.len(), |n| e + n + 1);
    let after_blank = text[e..line_end].trim().is_empty();
    if before_blank && after_blank {
        (line_start, line_end)
    } else {
        (s, e)
    }
}

/// `text` without our actions.
pub fn strip(text: &str) -> Result<String, String> {
    let (blocks, _) = scan(text)?;
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    for r in blocks {
        if !is_ours(&text[r.0..r.1]) {
            continue;
        }
        let (s, e) = whole_lines(text, r);
        out.push_str(&text[at..s]);
        at = e;
    }
    out.push_str(&text[at..]);
    Ok(out)
}

/// `text` (`None`: no file yet) with exactly `actions` as our actions, after the user's.
pub fn merge(text: Option<&str>, actions: &[Action]) -> Result<String, String> {
    let base = strip(text.unwrap_or(EMPTY))?;
    let (_, end) = scan(&base)?;
    let line_start = base[..end].rfind('\n').map_or(0, |n| n + 1);
    let mut ins: String = actions.iter().map(block).collect();
    // `</actions>` not at the start of its line: start ours on a new line.
    let at = if base[line_start..end].trim().is_empty() {
        line_start
    } else {
        ins.insert(0, '\n');
        end
    };
    let mut out = String::with_capacity(base.len() + ins.len());
    out.push_str(&base[..at]);
    out.push_str(&ins);
    out.push_str(&base[at..]);
    Ok(out)
}

/// The unique ids (without the prefix) of our actions in `text`.
pub fn ours(text: &str) -> Vec<String> {
    let Ok((blocks, _)) = scan(text) else {
        return Vec::new();
    };
    let tag = format!("<unique-id>{ID_PREFIX}");
    blocks
        .into_iter()
        .filter_map(|(s, e)| {
            let b = &text[s..e];
            let i = b.find(&tag)? + tag.len();
            let n = b[i..].find('<')?;
            Some(b[i..i + n].to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const USER: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<actions>\n\
<!-- <action><unique-id>specular-fake</unique-id></action> -->\n\
<action>\n\
\t<icon>utilities-terminal</icon>\n\
\t<name>Open Terminal Here</name>\n\
\t<submenu></submenu>\n\
\t<unique-id>1527510935520125-1</unique-id>\n\
\t<command>exo-open --working-directory %f --launch TerminalEmulator</command>\n\
\t<description>Example for a custom action</description>\n\
\t<range></range>\n\
\t<patterns>*</patterns>\n\
\t<startup-notify/>\n\
\t<directories/>\n\
</action>\n\
<action><icon></icon><name>Mine &amp; yours</name><unique-id>x-2</unique-id><command>a %F</command><description></description><patterns>*.png</patterns><image-files/></action>\n\
</actions>\n";

    fn actions() -> Vec<Action> {
        vec![
            Action {
                id: "open",
                name: "Open in Specular".into(),
                description: "Open & flip".into(),
                icon: "app.specular.Specular".into(),
                command: "'/opt/my apps/specular' %F".into(),
                patterns: "*.jpg;*.png".into(),
            },
            Action {
                id: "wallpaper",
                name: "Set as <Wallpaper>".into(),
                description: String::new(),
                icon: "preferences-desktop-wallpaper".into(),
                command: "specular --set-wallpaper %f".into(),
                patterns: "*.jpg".into(),
            },
        ]
    }

    #[test]
    fn merge_preserves_user_actions_and_is_idempotent() {
        let once = merge(Some(USER), &actions()).unwrap();
        // The user's text is untouched, ours go right before </actions>.
        let at = USER.rfind("</actions>").unwrap();
        assert!(once.starts_with(&USER[..at]));
        assert!(once.ends_with("</actions>\n"));
        assert!(once.contains("<command>&apos;/opt/my apps/specular&apos; %F</command>"));
        assert!(once.contains("<name>Set as &lt;Wallpaper&gt;</name>"));
        assert_eq!(ours(&once), ["open", "wallpaper"]);
        // The commented-out "action" is not a block.
        assert_eq!(scan(USER).unwrap().0.len(), 2);

        let twice = merge(Some(&once), &actions()).unwrap();
        assert_eq!(twice, once);
        // Changed labels replace ours in place of the old ones.
        let mut a = actions();
        a[0].name = "In Specular öffnen".into();
        let relabelled = merge(Some(&once), &a).unwrap();
        assert!(
            relabelled.contains("In Specular öffnen") && !relabelled.contains(">Open in Specular<")
        );
        assert_eq!(ours(&relabelled).len(), 2);

        assert_eq!(strip(&once).unwrap(), USER);
        assert_eq!(strip(USER).unwrap(), USER);
    }

    #[test]
    fn new_file_and_odd_layouts() {
        let fresh = merge(None, &actions()).unwrap();
        assert!(fresh.starts_with("<?xml"));
        assert_eq!(ours(&fresh).len(), 2);
        assert_eq!(strip(&fresh).unwrap(), EMPTY);

        // Everything on one line.
        let one = "<actions><action><unique-id>u-1</unique-id></action></actions>";
        let m = merge(Some(one), &actions()).unwrap();
        assert_eq!(
            strip(&m).unwrap(),
            one.replace("</actions>", "\n</actions>")
        );
        assert!(m.starts_with("<actions><action><unique-id>u-1</unique-id></action>\n<action>"));
        assert_eq!(ours(&m).len(), 2);

        // An empty root element.
        let m = merge(Some("<?xml version=\"1.0\"?>\n<actions/>\n"), &actions());
        assert!(m.is_err());
    }

    #[test]
    fn refuses_what_it_does_not_understand() {
        for bad in [
            "",
            "<actions>\n<action>\n",
            "<config><actions></actions></config>",
            "<actions><!-- never closed </actions>",
            "<actions><action><action></action></action></actions>",
        ] {
            assert!(merge(Some(bad), &actions()).is_err(), "{bad:?}");
            assert!(strip(bad).is_err(), "{bad:?}");
        }
    }
}
