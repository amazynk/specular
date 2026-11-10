//! Small in-memory XPS and OpenXPS packages for tests elsewhere in the workspace.

const XPS_NS: &str = "http://schemas.microsoft.com/xps/2005/06";
const XPS_REL: &str = "http://schemas.microsoft.com/xps/2005/06/fixedrepresentation";
const OXPS_NS: &str = "http://schemas.openxps.org/oxps/v1.0";
const OXPS_REL: &str = "http://schemas.openxps.org/oxps/v1.0/fixedrepresentation";

/// Core properties every fixture carries.
pub const TITLE: &str = "Fixture title";
pub const CREATOR: &str = "Fixture author";

fn crc32(d: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in d {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                0xEDB8_8320 ^ (c >> 1)
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// A stored (uncompressed) ZIP of `files`.
pub fn zip(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let off = out.len() as u32;
        let crc = crc32(data);
        let header = |v: &mut Vec<u8>, central: bool| {
            v.extend_from_slice(if central {
                b"PK\x01\x02"
            } else {
                b"PK\x03\x04"
            });
            if central {
                v.extend_from_slice(&20u16.to_le_bytes());
            }
            v.extend_from_slice(&20u16.to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes()); // flags
            v.extend_from_slice(&0u16.to_le_bytes()); // stored
            v.extend_from_slice(&0u32.to_le_bytes()); // time, date
            v.extend_from_slice(&crc.to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(data.len() as u32).to_le_bytes());
            v.extend_from_slice(&(name.len() as u16).to_le_bytes());
            v.extend_from_slice(&0u16.to_le_bytes());
            if central {
                v.extend_from_slice(&[0; 6]); // comment length, disk, internal attributes
                v.extend_from_slice(&0u32.to_le_bytes());
                v.extend_from_slice(&off.to_le_bytes());
            }
            v.extend_from_slice(name.as_bytes());
        };
        header(&mut out, false);
        out.extend_from_slice(data);
        header(&mut central, true);
    }
    let cd = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(b"PK\x05\x06\0\0\0\0");
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(files.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&cd.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

/// A one-document package (XPS 1.0, or OpenXPS when `openxps`) with one fixed page per
/// `(width, height, markup)`: the size in 1/96 inch and the FixedPage's inner XAML, e.g.
/// `<Path Data="M 0,0 H 96 V 96 H 0 Z" Fill="#FF0000"/>`. The package has core properties
/// with [`TITLE`] and [`CREATOR`].
pub fn package(openxps: bool, pages: &[(f64, f64, &str)]) -> Vec<u8> {
    let (ns, rel) = if openxps {
        (OXPS_NS, OXPS_REL)
    } else {
        (XPS_NS, XPS_REL)
    };
    let rels = format!(
        r#"<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="R1" Type="{rel}" Target="/FixedDocumentSequence.fdseq"/><Relationship Id="R2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="/docProps/core.xml"/></Relationships>"#
    );
    let core = format!(
        r#"<cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>{TITLE}</dc:title><dc:creator>{CREATOR}</dc:creator></cp:coreProperties>"#
    );
    let types = r#"<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="fdseq" ContentType="application/vnd.ms-package.xps-fixeddocumentsequence+xml"/><Default Extension="fdoc" ContentType="application/vnd.ms-package.xps-fixeddocument+xml"/><Default Extension="fpage" ContentType="application/vnd.ms-package.xps-fixedpage+xml"/><Default Extension="xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/></Types>"#;
    let fds = format!(
        r#"<FixedDocumentSequence xmlns="{ns}"><DocumentReference Source="Documents/1/FixedDocument.fdoc"/></FixedDocumentSequence>"#
    );
    let mut fdoc = format!(r#"<FixedDocument xmlns="{ns}">"#);
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    for (i, (w, h, body)) in pages.iter().enumerate() {
        fdoc.push_str(&format!(
            r#"<PageContent Source="Pages/{}.fpage" Width="{w}" Height="{h}"/>"#,
            i + 1
        ));
        parts.push((
            format!("Documents/1/Pages/{}.fpage", i + 1),
            format!(r#"<FixedPage xmlns="{ns}" Width="{w}" Height="{h}" xml:lang="en">{body}</FixedPage>"#)
                .into_bytes(),
        ));
    }
    fdoc.push_str("</FixedDocument>");
    let mut files: Vec<(String, Vec<u8>)> = vec![
        ("[Content_Types].xml".into(), types.as_bytes().to_vec()),
        ("_rels/.rels".into(), rels.into_bytes()),
        ("docProps/core.xml".into(), core.into_bytes()),
        ("FixedDocumentSequence.fdseq".into(), fds.into_bytes()),
        ("Documents/1/FixedDocument.fdoc".into(), fdoc.into_bytes()),
    ];
    files.extend(parts);
    let refs: Vec<(&str, &[u8])> = files
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    zip(&refs)
}
