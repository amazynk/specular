//! The metadata editor's fields against exiftool: what we write (an XMP sidecar, a JPEG's
//! XMP and IPTC-IIM, a TIFF's XMP) it reads the same, field by field; what it writes we read
//! the same; and where XMP and IIM disagree, which one each of us takes. Skipped (with a
//! note) when exiftool is not installed. Each test prints its mismatch count.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use specular_metadata::edit::{self, Field, Record, embed};
use specular_metadata::keywords as kw;

fn exiftool() -> bool {
    let ok = Command::new("exiftool").arg("-ver").output().is_ok();
    if !ok {
        eprintln!("exiftool not installed: the IPTC oracle is skipped");
    }
    ok
}

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "specular-iptc-oracle-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name),
    )
    .unwrap()
}

const SEP: &str = " ;; ";

/// Every tag exiftool reads from `p`, by `Group:Tag` (lists joined with [`SEP`]).
fn tags(p: &Path, extra: &[&str]) -> HashMap<String, String> {
    let out = Command::new("exiftool")
        .args(["-args", "-a", "-G1", "-charset", "UTF8", "-sep", SEP])
        .args(extra)
        .arg(p)
        .output()
        .unwrap();
    let mut m = HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let Some(l) = line.strip_prefix('-') else {
            continue;
        };
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        m.insert(k.to_owned(), v.to_owned());
    }
    m
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

/// A value for every field (no line breaks: exiftool's `-args` prints them as dots).
fn full_record() -> Record {
    let mut r = Record::new();
    for f in Field::all() {
        let v = match f {
            Field::Keywords => s(&["Places|Portugal|Lisbon", "tram & street"]),
            Field::Creator => s(&["Ana Silva", "Bo <B> Ek"]),
            Field::PersonsShown => s(&["Zoë", "José"]),
            Field::ShownCountryCode | Field::CreatedCountryCode => s(&["PRT"]),
            Field::ContactEmail => s(&["ana@example.org"]),
            f => vec![format!("{} «ação» & <b> שלום", f.key())],
        };
        r.set(f, v);
    }
    r
}

/// The exiftool tags holding each field in XMP.
fn xmp_tags(f: Field) -> &'static [&'static str] {
    match f {
        Field::Title => &["XMP-dc:Title"],
        Field::Headline => &["XMP-photoshop:Headline"],
        Field::Description => &["XMP-dc:Description"],
        Field::Keywords => &["XMP-lr:HierarchicalSubject"],
        Field::PersonsShown => &["XMP-iptcExt:PersonInImage"],
        Field::Event => &["XMP-iptcExt:Event"],
        Field::Creator => &["XMP-dc:Creator"],
        Field::CreatorJobTitle => &["XMP-photoshop:AuthorsPosition"],
        Field::ContactAddress => &["XMP-iptcCore:CreatorAddress"],
        Field::ContactCity => &["XMP-iptcCore:CreatorCity"],
        Field::ContactRegion => &["XMP-iptcCore:CreatorRegion"],
        Field::ContactPostalCode => &["XMP-iptcCore:CreatorPostalCode"],
        Field::ContactCountry => &["XMP-iptcCore:CreatorCountry"],
        Field::ContactPhone => &["XMP-iptcCore:CreatorWorkTelephone"],
        Field::ContactEmail => &["XMP-iptcCore:CreatorWorkEmail"],
        Field::ContactUrl => &["XMP-iptcCore:CreatorWorkURL"],
        Field::Copyright => &["XMP-dc:Rights"],
        Field::Credit => &["XMP-photoshop:Credit"],
        Field::Source => &["XMP-photoshop:Source"],
        Field::UsageTerms => &["XMP-xmpRights:UsageTerms"],
        Field::Instructions => &["XMP-photoshop:Instructions"],
        Field::JobId => &["XMP-photoshop:TransmissionReference"],
        Field::ShownSublocation => &[
            "XMP-iptcExt:LocationShownSublocation",
            "XMP-iptcCore:Location",
        ],
        Field::ShownCity => &["XMP-iptcExt:LocationShownCity", "XMP-photoshop:City"],
        Field::ShownState => &[
            "XMP-iptcExt:LocationShownProvinceState",
            "XMP-photoshop:State",
        ],
        Field::ShownCountry => &[
            "XMP-iptcExt:LocationShownCountryName",
            "XMP-photoshop:Country",
        ],
        Field::ShownCountryCode => &[
            "XMP-iptcExt:LocationShownCountryCode",
            "XMP-iptcCore:CountryCode",
        ],
        Field::CreatedSublocation => &["XMP-iptcExt:LocationCreatedSublocation"],
        Field::CreatedCity => &["XMP-iptcExt:LocationCreatedCity"],
        Field::CreatedState => &["XMP-iptcExt:LocationCreatedProvinceState"],
        Field::CreatedCountry => &["XMP-iptcExt:LocationCreatedCountryName"],
        Field::CreatedCountryCode => &["XMP-iptcExt:LocationCreatedCountryCode"],
    }
}

/// The exiftool tag holding each field in IIM.
fn iim_tag(f: Field) -> Option<&'static str> {
    Some(match f {
        Field::Title => "IPTC:ObjectName",
        Field::Headline => "IPTC:Headline",
        Field::Description => "IPTC:Caption-Abstract",
        Field::Keywords => "IPTC:Keywords",
        Field::Creator => "IPTC:By-line",
        Field::CreatorJobTitle => "IPTC:By-lineTitle",
        Field::Copyright => "IPTC:CopyrightNotice",
        Field::Credit => "IPTC:Credit",
        Field::Source => "IPTC:Source",
        Field::Instructions => "IPTC:SpecialInstructions",
        Field::JobId => "IPTC:OriginalTransmissionReference",
        Field::ShownSublocation => "IPTC:Sub-location",
        Field::ShownCity => "IPTC:City",
        Field::ShownState => "IPTC:Province-State",
        Field::ShownCountry => "IPTC:Country-PrimaryLocationName",
        Field::ShownCountryCode => "IPTC:Country-PrimaryLocationCode",
        _ => return None,
    })
}

/// Compares every field's XMP tags (and IIM tags when `iim`) with `r`; returns (checked,
/// mismatches) and prints each mismatch.
fn compare(what: &str, t: &HashMap<String, String>, r: &Record, iim: bool) -> (usize, usize) {
    let (mut n, mut bad) = (0, 0);
    let mut check = |tag: &str, want: String| {
        n += 1;
        let got = t.get(tag).cloned().unwrap_or_default();
        if got != want {
            bad += 1;
            eprintln!("{what}: {tag}: exiftool {got:?}, ours {want:?}");
        }
    };
    for f in Field::all() {
        let want = r.values(f).join(SEP);
        for tag in xmp_tags(f) {
            check(tag, want.clone());
        }
        if f == Field::Keywords {
            check("XMP-dc:Subject", kw::subject(r.values(f), None).join(SEP));
        }
        if iim && let Some(tag) = iim_tag(f) {
            let max = edit::iim_limit(f).unwrap_or(usize::MAX);
            let vals: Vec<String> = if f == Field::Keywords {
                kw::subject(r.values(f), None)
            } else {
                r.values(f).to_vec()
            };
            let cut: Vec<String> = vals
                .iter()
                .map(|v| {
                    let mut e = max.min(v.len());
                    while !v.is_char_boundary(e) {
                        e -= 1;
                    }
                    v[..e].trim().to_owned()
                })
                .collect();
            check(tag, cut.join(SEP));
        }
    }
    eprintln!("{what}: {n} values against exiftool, {bad} mismatches");
    (n, bad)
}

fn all_fields() -> Vec<Field> {
    Field::all().collect()
}

#[test]
fn exiftool_reads_what_we_write() {
    if !exiftool() {
        return;
    }
    let d = dir("write");
    let r = full_record();
    let mut total = (0, 0);
    let mut add = |x: (usize, usize)| {
        total.0 += x.0;
        total.1 += x.1;
    };
    // A sidecar.
    let side = d.join("a.xmp");
    std::fs::write(&side, edit::write_xmp(None, &r, &all_fields(), None, true)).unwrap();
    add(compare("sidecar", &tags(&side, &[]), &r, false));
    // A JPEG with XMP already (lisbon.jpg: title, creator, rights, subject, rating): XMP and
    // IIM written, the rating and label kept.
    for name in ["lisbon.jpg", "ocean.jpg"] {
        let j = fixture(name);
        let own = edit::read_embedded(&mut std::io::Cursor::new(&j)).unwrap();
        let x = edit::write_xmp(own.xmp.as_deref(), &r, &all_fields(), None, false);
        let subject = kw::subject(r.values(Field::Keywords), None);
        let iim = edit::write_iim(own.iim.as_deref(), &r, &all_fields(), &subject);
        let out = embed::jpeg(&j, &x, Some(iim.as_deref())).unwrap();
        let p = d.join(name);
        std::fs::write(&p, &out).unwrap();
        let t = tags(&p, &[]);
        add(compare(name, &t, &r, true));
        if name == "lisbon.jpg" {
            assert_eq!(t.get("XMP-xmp:Rating").map(String::as_str), Some("4"));
            assert_eq!(t.get("XMP-xmp:Label").map(String::as_str), Some("Red"));
        }
        // exiftool finds nothing wrong that was not wrong before, and the image is the same.
        let orig = d.join(format!("orig-{name}"));
        std::fs::write(&orig, &j).unwrap();
        let problems = |p: &Path| -> Vec<String> {
            let out = Command::new("exiftool")
                .args(["-args", "-a", "-G1", "-validate", "-warning", "-error"])
                .arg(p)
                .output()
                .unwrap();
            let mut w: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter(|l| l.contains("Warning=") || l.contains("Error="))
                .map(str::to_owned)
                .collect();
            w.sort();
            w
        };
        let before = problems(&orig);
        let new: Vec<String> = problems(&p)
            .into_iter()
            .filter(|w| !before.contains(w))
            .collect();
        assert!(new.is_empty(), "{name}: {new:?}");
        let hash = |p: &Path| {
            tags(p, &["-ImageDataHash", "-api", "ImageHashType=SHA256"])
                .get("File:ImageDataHash")
                .cloned()
        };
        assert!(hash(&orig).is_some());
        assert_eq!(hash(&orig), hash(&p), "{name}: image data changed");
    }
    // A TIFF: XMP only.
    let tif = tiny_tiff();
    let x = edit::write_xmp(None, &r, &all_fields(), None, false);
    let out = embed::tiff(&tif, &x).unwrap();
    let p = d.join("a.tif");
    std::fs::write(&p, &out).unwrap();
    add(compare("tiff", &tags(&p, &[]), &r, false));
    let v = tags(&p, &["-validate", "-warning", "-error"]);
    let problems: Vec<_> = v.iter().filter(|(k, _)| k.contains("Error")).collect();
    assert!(problems.is_empty(), "tiff: {problems:?}");
    eprintln!("written by us: {} values, {} mismatches", total.0, total.1);
    assert_eq!(total.1, 0);
    let _ = std::fs::remove_dir_all(&d);
}

/// A little-endian 2×1 grey TIFF.
fn tiny_tiff() -> Vec<u8> {
    let mut t = b"II*\0\x0a\0\0\0".to_vec();
    t.extend([10u8, 200]);
    let entries: [(u16, u16, u32, u32); 8] = [
        (256, 3, 1, 2),
        (257, 3, 1, 1),
        (258, 3, 1, 8),
        (259, 3, 1, 1),
        (262, 3, 1, 1),
        (273, 4, 1, 8),
        (278, 3, 1, 1),
        (279, 4, 1, 2),
    ];
    t.extend((entries.len() as u16).to_le_bytes());
    for (tag, ty, n, v) in entries {
        t.extend(tag.to_le_bytes());
        t.extend(ty.to_le_bytes());
        t.extend(n.to_le_bytes());
        t.extend(v.to_le_bytes());
    }
    t.extend(0u32.to_le_bytes());
    t
}

#[test]
fn we_read_what_exiftool_writes() {
    if !exiftool() {
        return;
    }
    let d = dir("read");
    let r = full_record();
    // Every field through exiftool's own writer: XMP and IIM (UTF-8).
    let p = d.join("a.jpg");
    std::fs::write(&p, fixture("ocean.jpg")).unwrap();
    let mut args: Vec<String> = s(&[
        "-overwrite_original",
        "-charset",
        "UTF8",
        "-codedcharacterset=utf8",
    ]);
    for f in Field::all() {
        for tag in xmp_tags(f).iter().take(1) {
            for v in r.values(f) {
                args.push(format!("-{tag}={v}"));
            }
        }
        if f == Field::Keywords {
            for v in kw::subject(r.values(f), None) {
                args.push(format!("-XMP-dc:Subject={v}"));
            }
        }
    }
    let out = Command::new("exiftool")
        .args(&args)
        .arg(&p)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let own = edit::read_embedded(&mut std::fs::File::open(&p).unwrap()).unwrap();
    let got = own.record();
    let mut bad = 0;
    for f in Field::all() {
        if got.values(f) != r.values(f) {
            bad += 1;
            eprintln!("{f:?}: ours {:?}, written {:?}", got.values(f), r.values(f));
        }
    }
    eprintln!(
        "written by exiftool (XMP): {} fields, {bad} mismatches",
        Field::all().count()
    );
    // IIM alone (exiftool's), each field that has a dataset.
    let q = d.join("b.jpg");
    std::fs::write(&q, fixture("ocean.jpg")).unwrap();
    let mut args: Vec<String> = s(&[
        "-overwrite_original",
        "-charset",
        "UTF8",
        "-codedcharacterset=utf8",
    ]);
    let mut iim_fields = 0;
    for f in Field::all() {
        if let Some(tag) = iim_tag(f) {
            iim_fields += 1;
            let vals = if f == Field::Keywords {
                s(&["tram", "lisbon"])
            } else {
                vec![format!("{}-é", &f.key()[..f.key().len().min(2)])]
            };
            for v in vals {
                args.push(format!("-{tag}={v}"));
            }
        }
    }
    let out = Command::new("exiftool")
        .args(&args)
        .arg(&q)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let t = tags(&q, &[]);
    let own = edit::read_embedded(&mut std::fs::File::open(&q).unwrap()).unwrap();
    let got = own.record();
    let mut bad_iim = 0;
    for f in Field::all() {
        if let Some(tag) = iim_tag(f) {
            let want = t.get(tag).cloned().unwrap_or_default();
            if got.values(f).join(SEP) != want {
                bad_iim += 1;
                eprintln!("{tag}: ours {:?}, exiftool {want:?}", got.values(f));
            }
        }
    }
    eprintln!("written by exiftool (IIM): {iim_fields} fields, {bad_iim} mismatches");
    assert_eq!((bad, bad_iim), (0, 0));
    let _ = std::fs::remove_dir_all(&d);
}

/// XMP and IIM disagreeing: we take XMP (the Metadata Working Group's rule); exiftool's MWG
/// composite tags are compared with ours.
#[test]
fn xmp_wins_as_in_exiftools_mwg_tags() {
    if !exiftool() {
        return;
    }
    let d = dir("mwg");
    let p = d.join("a.jpg");
    std::fs::write(&p, fixture("ocean.jpg")).unwrap();
    let out = Command::new("exiftool")
        .args([
            "-overwrite_original",
            "-charset",
            "UTF8",
            "-codedcharacterset=utf8",
            "-XMP-dc:Description=XMP caption",
            "-IPTC:Caption-Abstract=IIM caption",
            "-XMP-dc:Creator=XMP creator",
            "-IPTC:By-line=IIM creator",
            "-XMP-dc:Rights=XMP rights",
            "-IPTC:CopyrightNotice=IIM rights",
            "-IPTC:City=IIM city",
            "-IPTC:Keywords=iim",
            "-XMP-dc:Subject=xmp",
        ])
        .arg(&p)
        .output()
        .unwrap();
    assert!(out.status.success());
    let own = edit::read_embedded(&mut std::fs::File::open(&p).unwrap()).unwrap();
    let r = own.record();
    let t = tags(
        &p,
        &[
            "-use",
            "MWG",
            "-MWG:Description",
            "-MWG:Creator",
            "-MWG:Copyright",
            "-MWG:City",
            "-MWG:Keywords",
        ],
    );
    let mut bad = 0;
    for (field, tag) in [
        (Field::Description, "MWG:Description"),
        (Field::Creator, "MWG:Creator"),
        (Field::Copyright, "MWG:Copyright"),
        (Field::ShownCity, "MWG:City"),
        (Field::Keywords, "MWG:Keywords"),
    ] {
        let ours = r.values(field).join(SEP);
        let theirs = t.get(tag).cloned().unwrap_or_default();
        if ours != theirs {
            bad += 1;
            eprintln!("{tag}: exiftool MWG {theirs:?}, ours {ours:?}");
        }
    }
    eprintln!("XMP against IIM: 5 fields against exiftool's MWG tags, {bad} mismatches");
    assert_eq!(r.values(Field::Description), s(&["XMP caption"]));
    assert_eq!(
        r.values(Field::ShownCity),
        s(&["IIM city"]),
        "IIM fills what XMP lacks"
    );
    assert_eq!(bad, 0);
    let _ = std::fs::remove_dir_all(&d);
}
