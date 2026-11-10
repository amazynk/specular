//! Styles: named styles with parent chains and family defaults, the named drawing objects
//! (gradients, dashes, markers, hatches, fill images) and font face declarations, from
//! `styles.xml` and `content.xml`.

use std::collections::HashMap;

use specular_drawpage::xml::roxmltree::Node;

/// Properties of one kind, by attribute local name.
pub type Props = HashMap<String, String>;

/// Deepest parent chain followed.
const MAX_CHAIN: usize = 32;

#[derive(Debug, Clone, Default)]
pub struct Style {
    pub parent: Option<String>,
    /// `style:graphic-properties` (or `drawing-page-properties`).
    pub graphic: Props,
    pub paragraph: Props,
    pub text: Props,
    /// `style:page-layout-properties` (page layouts only).
    pub layout: Props,
    /// The list style for lists in the object's text (`style:list-style-name`, or one
    /// written inside the style).
    pub list: Option<String>,
}

/// One level of a list style.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ListLevel {
    /// A bullet character (`text:bullet-char`).
    pub bullet: Option<String>,
    /// Numbering: format ("1", "a", "A", "i", "I"), prefix, suffix, start.
    pub number: Option<(String, String, String, u32)>,
    /// Indent of the label from the paragraph's left margin, points.
    pub space_before: f64,
    /// Width of the label, points (text starts after it).
    pub label_width: f64,
    /// Label size relative to the text (`fo:font-size` percent), 0–n.
    pub size: f64,
}

fn list_levels(n: Node) -> Vec<ListLevel> {
    let mut levels: Vec<ListLevel> = Vec::new();
    for l in n.children().filter(Node::is_element) {
        let kind = l.tag_name().name();
        if !kind.starts_with("list-level-style-") {
            continue;
        }
        let level = attr(l, "level")
            .and_then(|v| v.parse::<usize>().ok())
            .unwrap_or(levels.len() + 1)
            .clamp(1, 10);
        let mut lv = ListLevel {
            size: 1.0,
            ..ListLevel::default()
        };
        match kind {
            "list-level-style-bullet" => {
                lv.bullet = Some(attr(l, "bullet-char").unwrap_or("\u{2022}").to_string())
            }
            "list-level-style-number" => {
                lv.number = Some((
                    attr(l, "num-format").unwrap_or("").to_string(),
                    attr(l, "num-prefix").unwrap_or("").to_string(),
                    attr(l, "num-suffix").unwrap_or("").to_string(),
                    attr(l, "start-value")
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(1),
                ))
            }
            _ => lv.bullet = Some("\u{2022}".into()),
        }
        for c in l.children().filter(Node::is_element) {
            match c.tag_name().name() {
                "list-level-properties" => {
                    let len = |k: &str| attr(c, k).and_then(crate::units::length);
                    lv.space_before = len("space-before").unwrap_or(0.0);
                    lv.label_width = len("min-label-width").unwrap_or(0.0);
                    if let Some(a) = c
                        .children()
                        .find(|a| a.tag_name().name() == "list-level-label-alignment")
                    {
                        let m = attr(a, "margin-left").and_then(crate::units::length);
                        let ti = attr(a, "text-indent").and_then(crate::units::length);
                        if let (Some(m), Some(ti)) = (m, ti) {
                            lv.space_before = m + ti;
                            lv.label_width = -ti;
                        }
                    }
                }
                "text-properties" => {
                    if let Some(p) = attr(c, "font-size").and_then(|v| v.strip_suffix('%')) {
                        lv.size = p.trim().parse::<f64>().map_or(1.0, |v| v / 100.0);
                    }
                }
                _ => {}
            }
        }
        if levels.len() < level {
            levels.resize(level, ListLevel::default());
        }
        levels[level - 1] = lv;
    }
    levels
}

/// Resolved properties of a style chain.
#[derive(Debug, Clone, Default)]
pub struct Resolved {
    pub graphic: Props,
    pub paragraph: Props,
    pub text: Props,
}

impl Resolved {
    pub fn g(&self, k: &str) -> Option<&str> {
        self.graphic.get(k).map(String::as_str)
    }
}

/// A named drawing object (`draw:gradient`, `draw:stroke-dash`, `draw:marker`, …): its
/// attributes and, for gradients, the `loext:gradient-stop` list.
#[derive(Debug, Clone, Default)]
pub struct Named {
    pub attrs: Props,
    /// (offset, colour) stops.
    pub stops: Vec<(f64, String)>,
}

#[derive(Debug, Default)]
pub struct Styles {
    /// By family, then name.
    styles: HashMap<(String, String), Style>,
    defaults: HashMap<String, Style>,
    pub gradients: HashMap<String, Named>,
    pub dashes: HashMap<String, Named>,
    pub markers: HashMap<String, Named>,
    pub hatches: HashMap<String, Named>,
    pub fill_images: HashMap<String, Named>,
    pub opacities: HashMap<String, Named>,
    /// Table templates: name → (part, cell style name) pairs.
    pub templates: HashMap<String, Vec<(String, String)>>,
    /// List styles by name.
    pub lists: HashMap<String, Vec<ListLevel>>,
    /// Font face name → family.
    pub fonts: HashMap<String, String>,
    pub page_layouts: HashMap<String, Style>,
    /// Master page name → (page layout, drawing-page style).
    pub masters: HashMap<String, (Option<String>, Option<String>)>,
}

fn props(n: Node) -> Props {
    n.attributes()
        .filter(|a| {
            // LibreOffice extensions are left out (they repeat ODF values or are unknown).
            !a.namespace()
                .is_some_and(|ns| ns.contains("libreoffice.org"))
        })
        .map(|a| (a.name().to_string(), a.value().to_string()))
        .collect()
}

fn style_of(n: Node) -> Style {
    let mut s = Style {
        parent: n
            .attributes()
            .find(|a| a.name() == "parent-style-name")
            .map(|a| a.value().to_string()),
        ..Style::default()
    };
    for c in n.children().filter(Node::is_element) {
        match c.tag_name().name() {
            // LibreOffice's own graphic properties repeat the shape's on paragraph styles,
            // but are the only fill of a table cell style.
            "graphic-properties" | "drawing-page-properties"
                if attr(n, "family") == Some("table-cell")
                    || !c
                        .tag_name()
                        .namespace()
                        .is_some_and(|ns| ns.contains("libreoffice.org")) =>
            {
                s.graphic.extend(props(c));
                // Text columns (`style:columns` inside): count and gap as properties.
                if let Some(cols) = c.children().find(|k| k.tag_name().name() == "columns") {
                    for key in ["column-count", "column-gap"] {
                        if let Some(v) = attr(cols, key) {
                            s.graphic.insert(key.to_string(), v.to_string());
                        }
                    }
                }
            }
            "paragraph-properties" => s.paragraph.extend(props(c)),
            "text-properties" => s.text.extend(props(c)),
            "page-layout-properties" | "table-column-properties" | "table-row-properties" => {
                s.layout.extend(props(c))
            }
            // Cell borders, padding, background and vertical alignment, beside the cell's
            // graphic properties.
            "table-cell-properties" => s.graphic.extend(props(c)),
            _ => {}
        }
    }
    s
}

fn attr<'a>(n: Node<'a, '_>, name: &str) -> Option<&'a str> {
    specular_drawpage::xml::attr(n, name)
}

impl Styles {
    /// Reads the styles of one part (`office:document-styles` or `office:document-content`
    /// or a flat `office:document`). Later parts override earlier ones by name.
    pub fn add_part(&mut self, root: Node) {
        for sect in root.children().filter(Node::is_element) {
            match sect.tag_name().name() {
                "font-face-decls" => {
                    for f in sect.children().filter(Node::is_element) {
                        if let (Some(n), Some(fam)) = (attr(f, "name"), attr(f, "font-family")) {
                            let fam = fam.trim().trim_matches(['\'', '"']).to_string();
                            self.fonts.insert(n.to_string(), fam);
                        }
                    }
                }
                "styles" | "automatic-styles" => self.add_styles(sect),
                "master-styles" => {
                    for m in sect.children().filter(Node::is_element) {
                        if m.tag_name().name() == "master-page"
                            && let Some(n) = attr(m, "name")
                        {
                            self.masters.insert(
                                n.to_string(),
                                (
                                    attr(m, "page-layout-name").map(str::to_string),
                                    attr(m, "style-name").map(str::to_string),
                                ),
                            );
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn add_styles(&mut self, sect: Node) {
        for s in sect.children().filter(Node::is_element) {
            let name = attr(s, "name").map(str::to_string);
            match s.tag_name().name() {
                "style" => {
                    if let (Some(n), Some(f)) = (name, attr(s, "family")) {
                        let mut st = style_of(s);
                        st.list = attr(s, "list-style-name").map(str::to_string);
                        if let Some(l) = s
                            .descendants()
                            .find(|d| d.tag_name().name() == "list-style")
                        {
                            let key = format!("\u{1}{f}/{n}");
                            self.lists.insert(key.clone(), list_levels(l));
                            st.list = Some(key);
                        }
                        self.styles.insert((f.to_string(), n), st);
                    }
                }
                "list-style" => {
                    if let Some(n) = name {
                        self.lists.insert(n, list_levels(s));
                    }
                }
                "table-template" => {
                    // ODF 1.3 names it `table:name`; LibreOffice wrote `text:style-name`.
                    if let Some(n) = name.or_else(|| attr(s, "style-name").map(str::to_string)) {
                        let parts = s
                            .children()
                            .filter(Node::is_element)
                            .filter_map(|c| {
                                Some((
                                    c.tag_name().name().to_string(),
                                    attr(c, "style-name")?.to_string(),
                                ))
                            })
                            .collect();
                        self.templates.insert(n, parts);
                    }
                }
                "default-style" => {
                    if let Some(f) = attr(s, "family") {
                        self.defaults.insert(f.to_string(), style_of(s));
                    }
                }
                "page-layout" => {
                    if let Some(n) = name {
                        self.page_layouts.insert(n, style_of(s));
                    }
                }
                kind @ ("gradient" | "linear-gradient" | "radial-gradient" | "stroke-dash"
                | "marker" | "hatch" | "fill-image" | "opacity") => {
                    let Some(n) = name else { continue };
                    let named = Named {
                        attrs: props(s),
                        stops: s
                            .children()
                            .filter(|c| c.tag_name().name() == "gradient-stop")
                            .filter_map(|c| {
                                let off = attr(c, "offset")?.trim();
                                let off = off
                                    .strip_suffix('%')
                                    .and_then(|p| p.parse::<f64>().ok().map(|v| v / 100.0))
                                    .or_else(|| off.parse().ok())?;
                                Some((off, attr(c, "color-value")?.to_string()))
                            })
                            .collect(),
                    };
                    let map = match kind {
                        "stroke-dash" => &mut self.dashes,
                        "marker" => &mut self.markers,
                        "hatch" => &mut self.hatches,
                        "fill-image" => &mut self.fill_images,
                        "opacity" => &mut self.opacities,
                        _ => &mut self.gradients,
                    };
                    map.insert(n, named);
                }
                _ => {}
            }
        }
    }

    /// The chain of a style, most general first: the family default, then ancestors.
    fn chain(&self, family: &str, name: Option<&str>) -> Vec<&Style> {
        let mut out: Vec<&Style> = Vec::new();
        let mut cur = name.map(str::to_string);
        while let Some(n) = cur {
            if out.len() >= MAX_CHAIN {
                break;
            }
            let Some(s) = self.styles.get(&(family.to_string(), n)) else {
                break;
            };
            out.push(s);
            cur = s.parent.clone();
        }
        if let Some(d) = self.defaults.get(family) {
            out.push(d);
        }
        out.reverse();
        out
    }

    /// Resolves `name` of `family` into `into`, over what is there.
    pub fn apply(&self, into: &mut Resolved, family: &str, name: Option<&str>) {
        for s in self.chain(family, name) {
            into.graphic.extend(s.graphic.clone());
            into.paragraph.extend(s.paragraph.clone());
            into.text.extend(s.text.clone());
        }
    }

    /// Paragraph and text properties of a paragraph or text style (no family default: the
    /// shape's own text properties lie below).
    pub fn apply_text(&self, into: &mut Resolved, family: &str, name: Option<&str>) {
        if name.is_none() {
            return;
        }
        let chain = self.chain(family, name);
        let skip = usize::from(self.defaults.contains_key(family));
        for s in chain.iter().skip(skip) {
            into.paragraph.extend(s.paragraph.clone());
            into.text.extend(s.text.clone());
        }
    }

    /// The list style of a graphic or presentation style chain (nearest first).
    pub fn list_of(&self, family: &str, name: Option<&str>) -> Option<&Vec<ListLevel>> {
        self.chain(family, name)
            .iter()
            .rev()
            .find_map(|s| s.list.as_ref())
            .and_then(|l| self.lists.get(l))
    }

    /// The layout properties (table column and row) of `name`'s chain.
    pub fn apply_layout(&self, family: &str, name: &str) -> Props {
        let mut p = Props::new();
        for s in self.chain(family, Some(name)) {
            p.extend(s.layout.clone());
        }
        p
    }

    /// A table cell style chain over `into`: its graphic (fill, borders, padding) and text.
    pub fn apply_cell(&self, into: &mut Resolved, name: &str) {
        for s in self.chain("table-cell", Some(name)) {
            into.graphic.extend(s.graphic.clone());
            into.paragraph.extend(s.paragraph.clone());
            into.text.extend(s.text.clone());
        }
    }

    pub fn get(&self, family: &str, name: &str) -> Option<&Style> {
        self.styles.get(&(family.to_string(), name.to_string()))
    }

    /// The drawing-page properties for a page: master page style below the page's own.
    pub fn page_props(&self, master: Option<&str>, own: Option<&str>) -> Props {
        let mut p = Props::new();
        if let Some(ms) = master
            .and_then(|m| self.masters.get(m))
            .and_then(|m| m.1.as_deref())
            .and_then(|s| self.get("drawing-page", s))
        {
            p.extend(ms.graphic.clone());
        }
        if let Some(s) = own.and_then(|s| self.get("drawing-page", s)) {
            p.extend(s.graphic.clone());
        }
        p
    }
}
