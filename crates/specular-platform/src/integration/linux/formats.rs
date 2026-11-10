//! The image types Specular registers for, with their file-name patterns.

/// (MIME type, file extensions) of every format the decoder registry reads (generated, see
/// [`crate::integration::mime`]).
pub use crate::integration::mime::IMAGE_TYPES;

/// All MIME types, in table order.
pub fn mime_types() -> impl Iterator<Item = &'static str> {
    IMAGE_TYPES.iter().map(|(m, _)| *m)
}

/// The types the default-viewer option claims (raster images; see
/// [`crate::integration::mime::is_default_viewer_type`]).
pub fn default_types() -> impl Iterator<Item = &'static str> {
    mime_types().filter(|m| crate::integration::mime::is_default_viewer_type(m))
}

/// `image/jpeg;image/png;…;` (the `MimeType=` value of desktop entries).
pub fn mime_list(types: &[&str]) -> String {
    types.iter().map(|m| format!("{m};")).collect()
}

/// Whether `name` has one of our extensions (case-insensitive).
pub fn is_image_name(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    IMAGE_TYPES
        .iter()
        .any(|(_, exts)| exts.iter().any(|e| e.eq_ignore_ascii_case(ext)))
}

/// Shared-MIME-info package adding our extensions to the types (merged with the system
/// database; harmless where the types exist already).
pub fn mime_package() -> String {
    let mut s = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <!-- {}: installed by Specular (Settings > Integration > Default viewer); removed with it. -->\n\
         <mime-info xmlns=\"http://www.freedesktop.org/standards/shared-mime-info\">\n",
        super::MARKER
    );
    // Types read by content only have no glob to add (the system package's magic, if any,
    // names them).
    for (mime, exts) in IMAGE_TYPES.iter().filter(|(_, e)| !e.is_empty()) {
        s.push_str(&format!("  <mime-type type=\"{mime}\">\n"));
        for e in *exts {
            s.push_str(&format!("    <glob pattern=\"*.{e}\"/>\n"));
        }
        s.push_str("  </mime-type>\n");
    }
    s.push_str("</mime-info>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table() {
        assert!(is_image_name("a.JPG"));
        assert!(is_image_name("x.y.jxl"));
        assert!(!is_image_name("notes.doc1"));
        assert!(is_image_name("notes.txt"));
        assert!(!is_image_name("jpg"));
        assert_eq!(
            mime_list(&["image/png", "image/gif"]),
            "image/png;image/gif;"
        );
        let pkg = mime_package();
        assert!(pkg.contains("<glob pattern=\"*.cr3\"/>"));
        assert!(default_types().any(|m| m == "image/jxl"));
        assert!(!default_types().any(|m| m == "image/svg+xml" || m == "application/dicom"));
        assert!(mime_types().any(|m| m == "image/svg+xml"));
        let mut seen = std::collections::HashSet::new();
        assert!(mime_types().all(|m| seen.insert(m)), "duplicate MIME type");
    }

    /// The packaged `.desktop` file and thumbnailer (system packages) agree with the
    /// generated table: every type the decoders read is offered, none that they do not.
    #[test]
    fn packaging_in_sync() {
        use crate::integration::linux::keyfile::{KeyFile, split_list};
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/linux");
        let desktop = std::fs::read_to_string(dir.join("app.specular.Specular.desktop")).unwrap();
        // A second MimeType= line would silently shadow the first (a merge once added one).
        assert_eq!(
            desktop
                .lines()
                .filter(|l| l.starts_with("MimeType="))
                .count(),
            1,
            "packaging/linux/app.specular.Specular.desktop: one MimeType= line"
        );
        let kf = KeyFile::parse(&desktop);
        let all: Vec<&str> = mime_types().collect();
        assert_eq!(
            kf.get("Desktop Entry", "MimeType"),
            Some(mime_list(&all).as_str()),
            "packaging/linux/app.specular.Specular.desktop: MimeType= must be the generated list"
        );
        let actions = super::super::DESKTOP_ACTIONS
            .iter()
            .map(|a| format!("{};", a.id()))
            .collect::<String>();
        assert_eq!(kf.get("Desktop Entry", "Actions"), Some(actions.as_str()));
        for a in super::super::DESKTOP_ACTIONS {
            let group = format!("Desktop Action {}", a.id());
            assert_eq!(
                kf.get(&group, "Exec").map(str::to_owned),
                Some(format!("specular {}", a.flag()))
            );
        }
        // The thumbnailer: table types, plus older names some systems still use.
        const ALIASES: &[&str] = &[
            "image/x-hdr",
            "image/x-psd",
            "image/x-qoi",
            "image/vnd.ms-photo",
            "image/x-djvu",
        ];
        let thumb = std::fs::read_to_string(dir.join("specular.thumbnailer")).unwrap();
        // A second MimeType= line would silently shadow the first.
        assert_eq!(
            thumb.lines().filter(|l| l.starts_with("MimeType=")).count(),
            1,
            "specular.thumbnailer: one MimeType= line"
        );
        let kf = KeyFile::parse(&thumb);
        let types = split_list(kf.get("Thumbnailer Entry", "MimeType").unwrap());
        assert!(types.len() > 20);
        for t in types {
            assert!(
                all.contains(&t) || ALIASES.contains(&t),
                "specular.thumbnailer lists {t}, which no decoder reads"
            );
        }
    }
}
