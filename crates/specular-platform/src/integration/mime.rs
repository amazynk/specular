//! The file types Specular registers with the desktop: every MIME type the decoder registry
//! reads, with its file-name extensions. The table itself ([`IMAGE_TYPES`], in
//! `mime_table.rs`) is generated from `specular_decode::Registry::mime_groups` and a test in
//! the app crate (`mime_table_in_sync`) fails when it falls behind; run that test with
//! `SPECULAR_UPDATE_MIME=1` to rewrite it. This crate cannot depend on the decoders, hence
//! the generated copy.

include!("mime_table.rs");

/// Workspace-relative path of the generated table.
pub const TABLE_PATH: &str = "crates/specular-platform/src/integration/mime_table.rs";

/// Whether the default-viewer option claims `mime`. Only raster image types: SVG belongs to
/// editors and browsers, WMF and EMF to the office and drawing programs that edit them (and
/// Specular draws them without their text), DICOM to medical viewers, FLIC and animated
/// cursors to video players, OpenRaster, Krita, GIMP, Paint.NET and PaintShop Pro documents to the painting
/// programs that edit them, PDF, Illustrator and EPS (shown through its preview) to document and drawing
/// programs, DjVu, XPS and CGM to document viewers and drawing tools. Specular still opens those and is offered for them ("Open With", the thumbnailer,
/// file-manager actions), but does not take them over.
pub fn is_default_viewer_type(mime: &str) -> bool {
    mime.starts_with("image/")
        && !mime.starts_with("image/svg")
        && !matches!(
            mime,
            // EPS shows only its preview.
            "image/openraster"
                | "image/x-xcf"
                | "image/x-paintnet"
                | "image/x-paintshoppro"
                | "image/x-compressed-xcf"
                | "image/x-eps"
                // Documents and drawings: left to document viewers and CAD tools.
                | "image/vnd.djvu"
                | "image/cgm"
                // DWG opens only through the ODA File Converter, when the user allows it.
                | "image/vnd.dwg"
                | "image/wmf"
                | "image/emf"
        )
}

/// The groups the user may add to the default-viewer claim one by one (Settings → Integration
/// → "More file types"). Every type Specular reads but [`is_default_viewer_type`] leaves out
/// is in exactly one group ([`extra_group`]). Camera RAW has no group: every RAW type is
/// already claimed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ExtraGroup {
    /// Documents of painting and drawing programs (Krita, GIMP, Procreate, Affinity, Illustrator,
    /// Paint.NET, PaintShop Pro, CorelDRAW).
    Editors,
    /// Measurements and instrument files (thermal cameras, microscopes, HDF5, seismic, weather).
    Science,
    /// Medical and neuroimaging files (DICOM, NIfTI, tractography).
    Medical,
    /// Drawings, chip layouts and maps.
    CadGis,
    /// 3D models, point clouds and molecules.
    Models,
    /// Video files (none yet: kept so the list does not change shape when they come).
    Videos,
    /// Image types usually left to other programs: SVG, PDF, EPS, WMF, EMF, animations.
    OtherImages,
    /// Text, tables and subtitles: plain text and logs, Markdown, CSV, RTF, subtitle files.
    Text,
}

impl ExtraGroup {
    /// Display order.
    pub const ALL: [ExtraGroup; 8] = [
        ExtraGroup::Editors,
        ExtraGroup::Science,
        ExtraGroup::Medical,
        ExtraGroup::CadGis,
        ExtraGroup::Models,
        ExtraGroup::Videos,
        ExtraGroup::OtherImages,
        ExtraGroup::Text,
    ];

    /// Stable name (message ids, row ids): `editors`, `science`, …
    pub fn key(self) -> &'static str {
        match self {
            ExtraGroup::Editors => "editors",
            ExtraGroup::Science => "science",
            ExtraGroup::Medical => "medical",
            ExtraGroup::CadGis => "cad-gis",
            ExtraGroup::Models => "models",
            ExtraGroup::Videos => "videos",
            ExtraGroup::OtherImages => "other-images",
            ExtraGroup::Text => "text",
        }
    }

    /// The group's types with their extensions, in table order.
    pub fn types(self) -> impl Iterator<Item = (&'static str, &'static [&'static str])> {
        IMAGE_TYPES
            .iter()
            .filter(move |(m, _)| extra_group(m) == Some(self))
            .copied()
    }
}

/// The group of a type the default viewer does not claim by itself; `None` for the types it
/// claims (and for types Specular does not read).
pub fn extra_group(mime: &str) -> Option<ExtraGroup> {
    if is_default_viewer_type(mime) || !IMAGE_TYPES.iter().any(|(m, _)| *m == mime) {
        return None;
    }
    let (top, sub) = mime.split_once('/')?;
    let sub = sub.strip_prefix("x-").unwrap_or(sub);
    Some(match (top, sub) {
        // Text, tables, Markdown, RTF and subtitle files.
        ("text", _) | ("application", "rtf" | "subrip" | "pgs" | "ttml+xml") => ExtraGroup::Text,
        (
            "application",
            "procreate" | "affinity-photo" | "affinity-designer" | "affinity-publisher" | "krita"
            | "illustrator" | "vnd.corel-draw" | "clip-studio" | "sketch" | "figma",
        )
        | ("image", "openraster" | "xcf" | "compressed-xcf" | "paintnet" | "paintshoppro") => {
            ExtraGroup::Editors
        }
        (
            "application",
            "dicom" | "cifti" | "gifti" | "trackvis" | "mrtrix-tracks" | "mrtrix-image" | "nifti"
            | "nrrd" | "metaimage" | "freesurfer-mgh" | "minc",
        ) => ExtraGroup::Medical,
        (
            "application",
            "vnd.hp-hpgl"
            | "vnd.gds"
            | "shapefile"
            | "geopackage+sqlite3"
            | "gpx+xml"
            | "geo+json"
            | "srtm-hgt"
            | "dted"
            | "mbtiles"
            | "vnd.pmtiles"
            | "flatgeobuf"
            | "vnd.apache.parquet"
            | "vnd.apache.arrow.file"
            | "vnd.apache.arrow.stream"
            | "gml+xml"
            | "vnd.openstreetmap.data+xml"
            | "osm-pbf"
            | "vnd.ant.fit"
            | "vnd.garmin.tcx+xml"
            | "mapinfo-tab"
            | "vnd.nitf",
        ) => ExtraGroup::CadGis,
        ("application", s) if s.starts_with("vnd.google-earth.") => ExtraGroup::CadGis,
        // CGM: technical illustrations (S1000D, WebCGM). DWG through its converter.
        ("image", "cgm" | "vnd.dwg") => ExtraGroup::CadGis,
        ("model" | "chemical", _)
        | ("application", "vnd.las" | "vnd.laszip" | "blender" | "vnd.sketchup.skp") => {
            ExtraGroup::Models
        }
        // Office drawings and documents: Visio, OpenDocument (pages or thumbnails), WPG, and
        // Word, Excel, PowerPoint and iWork documents (their stored previews).
        ("application", s)
            if s.starts_with("vnd.ms-visio.")
                || s.starts_with("vnd.oasis.opendocument.")
                || s.starts_with("vnd.openxmlformats-officedocument.")
                || s.starts_with("vnd.ms-word.")
                || s.starts_with("vnd.ms-excel")
                || s.starts_with("vnd.ms-powerpoint")
                || s == "msword"
                || matches!(
                    s,
                    "vnd.apple.pages" | "vnd.apple.numbers" | "vnd.apple.keynote"
                ) =>
        {
            ExtraGroup::OtherImages
        }
        // Fonts and books: specimens and pages, usually left to font managers and readers.
        ("font", _)
        | ("image", _)
        | ("application", "pdf" | "vnd.ms-xpsdocument" | "oxps" | "navi-animation" | "epub+zip")
        | ("application", "vnd.visio" | "wpg")
        | ("video", "mng" | "flic" | "anim") => ExtraGroup::OtherImages,
        // Astronomy capture runs (SER) and FLIR thermal sequences are frames from a camera,
        // read as science data.
        ("video", "ser" | "flir-seq") => ExtraGroup::Science,
        // MXF: broadcast, post-production and camera movies; ASF: Windows Media.
        ("video", _) | ("application", "mxf" | "vnd.ms-asf") => ExtraGroup::Videos,
        _ => ExtraGroup::Science,
    })
}

/// The extra types a user picked (`integration.extra_default_types`, comma-separated), kept to
/// the ones [`extra_group`] knows, without duplicates, in table order.
pub fn parse_extra_types(setting: &str) -> Vec<&'static str> {
    let picked: Vec<&str> = setting.split(',').map(str::trim).collect();
    IMAGE_TYPES
        .iter()
        .map(|(m, _)| *m)
        .filter(|m| picked.contains(m) && extra_group(m).is_some())
        .collect()
}

/// The setting value for `types` (the inverse of [`parse_extra_types`]).
pub fn format_extra_types(types: &[&str]) -> String {
    types.join(",")
}

/// The MIME type of `path` by its extension, `application/octet-stream` when the table does
/// not know it.
pub fn of_path(path: &std::path::Path) -> &'static str {
    path.extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .and_then(|e| {
            IMAGE_TYPES
                .iter()
                .find(|(_, exts)| exts.contains(&e.as_str()))
                .map(|(m, _)| *m)
        })
        .unwrap_or("application/octet-stream")
}

/// Workspace-relative path of the Windows installer's generated file associations
/// ([`render_wix`]), included by `packaging/windows/specular.wxs`.
pub const WIX_PATH: &str = "packaging/windows/associations.wxi";

/// Workspace-relative path of the winget installer manifest template, whose `FileExtensions`
/// block is generated ([`render_winget`]).
pub const WINGET_PATH: &str = "packaging/windows/winget/Specular.Specular.installer.yaml";

/// Every extension of `groups` Windows can register, once (`.xyz` is two types), in registry
/// order, with whether the default-viewer option may claim it ([`is_default_viewer_type`]).
/// Windows goes by the last extension only, so `ome.tif` (covered by `tif`) and `cbf.gz` (which
/// would claim every `.gz`) are left out.
pub fn extensions<'a>(groups: &[(&'a str, Vec<&'a str>)]) -> Vec<(&'a str, bool)> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for (mime, exts) in groups {
        for e in exts {
            if !e.contains('.') && seen.insert(*e) {
                out.push((*e, is_default_viewer_type(mime)));
            }
        }
    }
    out
}

/// The text of `associations.wxi` for `groups`: the Windows counterpart of the `.desktop`
/// file's `MimeType=`. Every type Specular reads is offered in "Open with" (the extension's
/// `OpenWithProgids` and the `Applications\specular.exe` `SupportedTypes`), which takes
/// nothing over; Default apps lists only the types the default-viewer option claims on Linux.
pub fn render_wix(groups: &[(&str, Vec<&str>)]) -> String {
    let exts = extensions(groups);
    let mut s = String::from(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!-- @generated from specular_decode::Registry::mime_groups by the app crate's\n     \
         `mime_table_in_sync` test (SPECULAR_UPDATE_MIME=1 rewrites it). Do not edit.\n     \
         Included by specular.wxs inside the main component. -->\n\
         <Include xmlns=\"http://wixtoolset.org/schemas/v4/wxs\">\n\
         \x20 <!-- \"Open with\": every type Specular reads, without taking over the default. -->\n",
    );
    let open_with = r"SOFTWARE\Classes\Applications\specular.exe\SupportedTypes";
    for (e, _) in &exts {
        s.push_str(&format!(
            "  <RegistryValue Root=\"HKLM\" Key=\"SOFTWARE\\Classes\\.{e}\\OpenWithProgids\" \
             Name=\"Specular.Image\" Value=\"\" Type=\"string\" />\n"
        ));
        s.push_str(&format!(
            "  <RegistryValue Root=\"HKLM\" Key=\"{open_with}\" Name=\".{e}\" Value=\"\" \
             Type=\"string\" />\n"
        ));
    }
    s.push_str(
        "  <!-- Default apps (Settings > Apps > Default apps > Specular): the image types the\n       \
         default-viewer option claims (is_default_viewer_type). -->\n",
    );
    for (e, _) in exts.iter().filter(|(_, claim)| *claim) {
        s.push_str(&format!(
            "  <RegistryValue Root=\"HKLM\" Key=\"SOFTWARE\\Specular\\Capabilities\\FileAssociations\" \
             Name=\".{e}\" Value=\"Specular.Image\" Type=\"string\" />\n"
        ));
    }
    s.push_str("</Include>\n");
    s
}

/// The winget installer template `template` with its `FileExtensions` block (and the comment
/// line above it) replaced by every extension of `groups`.
pub fn render_winget(template: &str, groups: &[(&str, Vec<&str>)]) -> String {
    const NOTE: &str = "# FileExtensions: generated from the decoder registry by the app crate's \
                        `mime_table_in_sync` test (SPECULAR_UPDATE_MIME=1).";
    let mut out = String::new();
    let mut lines = template.lines().peekable();
    while let Some(line) = lines.next() {
        if line == NOTE {
            continue;
        }
        if line == "FileExtensions:" {
            while lines.peek().is_some_and(|l| l.starts_with("  - ")) {
                lines.next();
            }
            out.push_str(NOTE);
            out.push('\n');
            out.push_str("FileExtensions:\n");
            for (e, _) in extensions(groups) {
                out.push_str(&format!("  - {e}\n"));
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// The text of `mime_table.rs` for `groups` (MIME type, extensions).
pub fn render(groups: &[(&str, Vec<&str>)]) -> String {
    let mut s = String::from(
        "// @generated from specular_decode::Registry::mime_groups by the app crate's\n\
         // `mime_table_in_sync` test (SPECULAR_UPDATE_MIME=1 rewrites it). Do not edit.\n\n\
         /// (MIME type, file extensions) of every format the decoder registry reads, in\n\
         /// registry order.\n\
         pub const IMAGE_TYPES: &[(&str, &[&str])] = &[\n",
    );
    for (mime, exts) in groups {
        let exts: Vec<String> = exts.iter().map(|e| format!("{e:?}")).collect();
        s.push_str(&format!("    ({mime:?}, &[{}]),\n", exts.join(", ")));
    }
    s.push_str("];\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Extensions that two registered types share (specular_decode's SHARED_EXTENSION_MIME).
    const SHARED: &[&str] = &[
        "xyz", "mts", "m2ts", "fit", "vob", "f4v", "pcd", "gbr", "ase",
    ];

    fn groups() -> Vec<(&'static str, Vec<&'static str>)> {
        vec![
            ("image/jpeg", vec!["jpg", "jpeg"]),
            ("image/tiff", vec!["tif", "ome.tif"]),
            ("model/x-xyz", vec!["xyz"]),
            ("chemical/x-xyz", vec!["xyz"]),
            ("image/svg+xml", vec!["svg"]),
            ("image/x-cbf", vec!["cbf", "cbf.gz"]),
        ]
    }

    /// Windows lists: each extension once, none with two parts, Default apps only for the
    /// types the default-viewer option claims.
    #[test]
    fn windows_extensions() {
        let e = extensions(&groups());
        assert_eq!(
            e,
            [
                ("jpg", true),
                ("jpeg", true),
                ("tif", true),
                ("xyz", false),
                ("svg", false),
                ("cbf", true)
            ]
        );
        let wix = render_wix(&groups());
        assert_eq!(wix.matches("OpenWithProgids").count(), 6);
        assert_eq!(wix.matches("FileAssociations").count(), 4);
        assert!(!wix.contains(".ome.tif") && !wix.contains(".gz"));
        // An XML comment may not contain "--".
        assert!(!wix.replace("<!--", "").replace("-->", "").contains("--"));
    }

    /// The winget block is replaced in place, idempotently, and nothing else changes.
    #[test]
    fn winget_block() {
        let template = "PackageVersion: 1\nFileExtensions:\n  - png\nCommands:\n  - specular\n";
        let once = render_winget(template, &groups());
        assert!(once.starts_with("PackageVersion: 1\n# FileExtensions: generated"));
        assert!(once.contains("FileExtensions:\n  - jpg\n  - jpeg\n  - tif\n"));
        assert!(!once.contains("  - png\n") && once.ends_with("Commands:\n  - specular\n"));
        assert_eq!(render_winget(&once, &groups()), once);
    }

    #[test]
    fn table_is_well_formed() {
        let mut mimes = std::collections::HashSet::new();
        let mut exts_seen = std::collections::HashSet::new();
        for (mime, exts) in IMAGE_TYPES {
            assert!(mimes.insert(*mime), "duplicate {mime}");
            // Types read by content only (SMV, ESRF EDF, Gaussian cube) have no extension.
            assert!(mime.contains('/'));
            for e in *exts {
                // `.xyz` is a point cloud or a molecule: registered under both types.
                assert!(
                    exts_seen.insert(*e) || SHARED.contains(e),
                    "extension {e} twice"
                );
                assert_eq!(e.to_ascii_lowercase(), *e);
            }
        }
        assert!(is_default_viewer_type("image/jxl"));
        assert!(!is_default_viewer_type("image/svg+xml"));
        assert!(!is_default_viewer_type("application/dicom"));
        assert!(!is_default_viewer_type("image/x-xcf"));
        assert!(!is_default_viewer_type("image/x-eps"));
        assert!(!is_default_viewer_type("application/pdf"));
        assert!(!is_default_viewer_type("image/emf"));
        assert!(!is_default_viewer_type("application/x-procreate"));
        assert!(!is_default_viewer_type("image/x-paintnet"));
        assert!(!is_default_viewer_type("image/x-paintshoppro"));
        assert!(is_default_viewer_type("image/flif"));
        assert!(is_default_viewer_type("image/bpg"));
        // Movies are offered for and thumbnailed, never claimed: the claim is for images.
        for m in [
            "video/mp4",
            "video/quicktime",
            "video/x-m4v",
            "video/3gpp",
            "video/3gpp2",
            "video/x-matroska",
            "video/webm",
            "video/mp2t",
            "video/vnd.dlna.mpeg-tts",
        ] {
            assert!(!is_default_viewer_type(m), "{m}");
        }
        assert_eq!(of_path(std::path::Path::new("/a/B.JPG")), "image/jpeg");
        // A shared extension resolves to its first (registry) type.
        assert_eq!(of_path(std::path::Path::new("/a/b.xyz")), "model/x-xyz");
        assert_eq!(of_path(std::path::Path::new("/a/b.txt")), "text/plain");
        assert_eq!(
            of_path(std::path::Path::new("/a/b.unknown")),
            "application/octet-stream"
        );
    }

    /// Every type outside the default claim is offered in exactly one group, none inside it;
    /// camera RAW needs no group because it is all claimed already.
    #[test]
    fn extra_groups_cover_the_rest() {
        for (mime, _) in IMAGE_TYPES {
            let g = extra_group(mime);
            assert_eq!(g.is_none(), is_default_viewer_type(mime), "{mime}");
            if let Some(g) = g {
                assert_eq!(g.types().filter(|(m, _)| m == mime).count(), 1);
            }
        }
        let count = |g: ExtraGroup| g.types().count();
        let total: usize = ExtraGroup::ALL.into_iter().map(count).sum();
        let defaults = IMAGE_TYPES
            .iter()
            .filter(|(m, _)| is_default_viewer_type(m))
            .count();
        assert_eq!(total + defaults, IMAGE_TYPES.len());
        for (mime, g) in [
            ("application/x-krita", ExtraGroup::Editors),
            ("image/x-xcf", ExtraGroup::Editors),
            ("application/illustrator", ExtraGroup::Editors),
            ("application/dicom", ExtraGroup::Medical),
            ("application/x-nifti", ExtraGroup::Medical),
            ("application/x-hdf", ExtraGroup::Science),
            ("application/x-flir-fff", ExtraGroup::Science),
            ("video/x-ser", ExtraGroup::Science),
            ("video/x-flir-seq", ExtraGroup::Science),
            ("application/vnd.hp-hpgl", ExtraGroup::CadGis),
            ("application/vnd.google-earth.kmz", ExtraGroup::CadGis),
            ("model/stl", ExtraGroup::Models),
            ("chemical/x-pdb", ExtraGroup::Models),
            ("application/vnd.laszip", ExtraGroup::Models),
            ("image/svg+xml", ExtraGroup::OtherImages),
            ("application/pdf", ExtraGroup::OtherImages),
            ("application/vnd.ms-xpsdocument", ExtraGroup::OtherImages),
            ("application/oxps", ExtraGroup::OtherImages),
            ("image/vnd.djvu", ExtraGroup::OtherImages),
            ("image/cgm", ExtraGroup::CadGis),
            ("font/ttf", ExtraGroup::OtherImages),
            ("font/woff2", ExtraGroup::OtherImages),
            ("application/epub+zip", ExtraGroup::OtherImages),
            ("image/vnd.dwg", ExtraGroup::CadGis),
            ("model/vnd.usdz+zip", ExtraGroup::Models),
            ("model/x-gaussian-splat", ExtraGroup::Models),
            ("application/x-blender", ExtraGroup::Models),
            ("application/vnd.sketchup.skp", ExtraGroup::Models),
            ("video/x-mng", ExtraGroup::OtherImages),
        ] {
            assert_eq!(extra_group(mime), Some(g), "{mime}");
        }
        assert_eq!(
            count(ExtraGroup::Videos),
            33,
            "MP4, M4V, QuickTime, 3GPP, 3GPP2, Matroska, WebM, MPEG-TS (twice), MPEG, DVD VOB, AVI, Ogg, OGM, IVF, MXF, Blackmagic RAW, Cinema RAW Light, FLV, WMV, ASF, DV, CD+G, YUV4MPEG, F4V, DVR-MS, raw H.264, HEVC, AV1 and Motion JPEG, APV, WTV, PlayStation STR"
        );
        assert_eq!(extra_group("video/x-braw"), Some(ExtraGroup::Videos));
        assert_eq!(extra_group("video/x-canon-crm"), Some(ExtraGroup::Videos));
        // Claimed already: camera RAW, Photoshop, DXF.
        for m in [
            "image/x-canon-cr3",
            "image/x-nikon-nef",
            "image/vnd.adobe.photoshop",
        ] {
            assert!(IMAGE_TYPES.iter().any(|(t, _)| *t == m), "{m}");
            assert_eq!(extra_group(m), None, "{m}");
        }
        // Text, tables and subtitles: never claimed by default, offered as their own group.
        for m in [
            "text/plain",
            "text/markdown",
            "text/csv",
            "application/rtf",
            "application/x-subrip",
            "text/vtt",
            "application/ttml+xml",
            "application/x-pgs",
        ] {
            assert_eq!(extra_group(m), Some(ExtraGroup::Text), "{m}");
        }
        assert_eq!(count(ExtraGroup::Text), 15);
        assert_eq!(extra_group("text/x-unknown"), None);
    }

    #[test]
    fn extra_types_round_trip() {
        let v = parse_extra_types(" model/stl,image/jpeg,application/dicom,nonsense,model/stl ,");
        assert_eq!(v, ["application/dicom", "model/stl"]);
        assert_eq!(format_extra_types(&v), "application/dicom,model/stl");
        assert_eq!(parse_extra_types(&format_extra_types(&v)), v);
        assert!(parse_extra_types("").is_empty());
    }

    #[test]
    fn render_round_trips() {
        let groups: Vec<(&str, Vec<&str>)> =
            IMAGE_TYPES.iter().map(|(m, e)| (*m, e.to_vec())).collect();
        let file = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/integration/mime_table.rs"),
        )
        .unwrap();
        assert_eq!(render(&groups), file);
    }
}
