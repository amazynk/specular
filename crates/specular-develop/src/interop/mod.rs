//! Other tools' edits.
//!
//! - [`crs`]: Lightroom and Camera Raw settings (`crs:`), imported approximately on request.
//! - [`darktable`]: darktable sidecars, detected and listed by module name; never converted.
//!
//! Both read XMP text written by hand in the fixtures; mappings come from public
//! descriptions of what each setting means, not from either tool's code.

pub mod crs;
pub mod darktable;

#[cfg(test)]
pub(crate) mod tests;

/// Parses XMP text (a sidecar or an embedded packet) without DTDs.
pub(crate) fn parse(text: &str) -> Option<roxmltree::Document<'_>> {
    let opts = roxmltree::ParsingOptions {
        allow_dtd: false,
        ..Default::default()
    };
    // An embedded packet may carry padding or a NUL after the closing tag.
    let end = text.rfind('>').map_or(0, |i| i + 1);
    roxmltree::Document::parse_with_options(&text[..end], opts).ok()
}

pub(crate) const RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";

/// The values of `ns`'s simple properties in a document: attributes of any `rdf:Description`
/// and child elements holding text, in document order (`name`, `value`).
pub(crate) fn simple_props<'a>(doc: &'a roxmltree::Document, ns: &str) -> Vec<(&'a str, &'a str)> {
    let mut out = Vec::new();
    for d in doc
        .descendants()
        .filter(|n| n.has_tag_name((RDF, "Description")))
    {
        for a in d.attributes() {
            if a.namespace() == Some(ns) {
                out.push((a.name(), a.value()));
            }
        }
        for c in d.children().filter(|c| c.is_element()) {
            if c.tag_name().namespace() == Some(ns)
                && c.children().all(|t| t.is_text())
                && let Some(t) = c.text()
            {
                out.push((c.tag_name().name(), t.trim()));
            }
        }
    }
    out
}

/// The text items of `ns:name`'s `rdf:Seq` (or `rdf:Bag`), in order.
pub(crate) fn seq_items<'a>(doc: &'a roxmltree::Document, ns: &str, name: &str) -> Vec<&'a str> {
    doc.descendants()
        .filter(|n| n.has_tag_name((ns, name)))
        .flat_map(|n| n.descendants())
        .filter(|n| n.has_tag_name((RDF, "li")))
        .filter_map(|n| n.text())
        .map(str::trim)
        .collect()
}
