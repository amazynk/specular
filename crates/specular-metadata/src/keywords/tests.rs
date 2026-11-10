use super::*;

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| (*x).to_owned()).collect()
}

fn lower(x: &str) -> String {
    x.to_lowercase()
}

#[test]
fn paths() {
    assert_eq!(
        normalise(" Places > France >  Paris "),
        Some("Places|France|Paris".into())
    );
    assert_eq!(normalise("a||b|"), Some("a|b".into()));
    assert_eq!(normalise("  New   York "), Some("New York".into()));
    assert_eq!(normalise(" | > "), None);
    assert_eq!(leaf("a|b|c"), "c");
    assert_eq!(parent("a|b|c"), Some("a|b"));
    assert_eq!(parent("a"), None);
    assert_eq!(depth("a|b|c"), 2);
    assert!(is_within("Places|France", "places"));
    assert!(!is_within("Placeses", "Places"));
    assert_eq!(display("a|b"), "a › b");
}

#[test]
fn reads_hierarchy_and_flat_keywords() {
    let x = crate::xmp::parse(
        br#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:lr="http://ns.adobe.com/lightroom/1.0/">
<dc:subject><rdf:Bag><rdf:li>Paris</rdf:li><rdf:li>France</rdf:li><rdf:li>Places</rdf:li><rdf:li>Lutetia</rdf:li><rdf:li>tram</rdf:li></rdf:Bag></dc:subject>
<lr:hierarchicalSubject><rdf:Bag><rdf:li>Places|France|Paris</rdf:li></rdf:Bag></lr:hierarchicalSubject>
</rdf:Description></rdf:RDF></x:xmpmeta>"#,
    );
    assert_eq!(
        from_xmp(&x).unwrap(),
        s(&["Places|France|Paris", "Lutetia", "tram"])
    );
    // A known synonym is not a keyword of its own.
    let syn = |n: &str| n == "Lutetia";
    assert_eq!(
        from_xmp_with(&x, &syn).unwrap(),
        s(&["Places|France|Paris", "tram"])
    );
    let none = crate::xmp::parse(b"<x:xmpmeta><rdf:RDF><rdf:Description/></rdf:RDF></x:xmpmeta>");
    assert_eq!(from_xmp(&none), None);
}

#[test]
fn writes_a_new_sidecar_and_reads_it_back() {
    let mut v = Vocabulary::new();
    v.add("Places|France|Paris");
    v.add("People|Ann");
    v.set_private("People|Ann", true);
    v.set_private("Places", true);
    v.set_synonyms("Places|France|Paris", &s(&["Lutetia"]));
    let kw = s(&["Places|France|Paris", "People|Ann", "tram & bus"]);
    let x = write_xmp(None, &kw, Some(&v));
    let back = crate::xmp::parse(x.as_bytes());
    // Private keywords and private levels stay out of dc:subject.
    assert_eq!(
        back.get_all("dc:subject").unwrap(),
        s(&["Paris", "France", "Lutetia", "tram & bus"])
    );
    assert_eq!(back.get_all("lr:hierarchicalSubject").unwrap(), &kw[..]);
    assert_eq!(from_xmp_text(&x, &|n| v.is_synonym(n)).unwrap(), kw);
    // Empty: both properties stay, saying "none".
    let e = write_xmp(Some(&x), &[], Some(&v));
    assert_eq!(from_xmp_text(&e, &|_| false), Some(Vec::new()));
    assert!(back.error.is_none());
}

#[test]
fn rewrites_only_the_keywords_of_another_tools_sidecar() {
    let lr = r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
 <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
  <rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:lr="http://ns.adobe.com/lightroom/1.0/" xmp:Rating="3">
   <dc:subject>
    <rdf:Bag>
     <rdf:li>old</rdf:li>
    </rdf:Bag>
   </dc:subject>
   <dc:title><rdf:Alt><rdf:li xml:lang="x-default">Title</rdf:li></rdf:Alt></dc:title>
   <lr:hierarchicalSubject><rdf:Bag><rdf:li>A|old</rdf:li></rdf:Bag></lr:hierarchicalSubject>
  </rdf:Description>
 </rdf:RDF>
</x:xmpmeta>"#;
    let out = write_xmp(Some(lr), &s(&["A|new"]), None);
    assert!(out.contains("xmp:Rating=\"3\""));
    assert!(out.contains(
        "<dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">Title</rdf:li></rdf:Alt></dc:title>"
    ));
    assert!(!out.contains("old"));
    assert_eq!(out.matches("xmlns:dc=").count(), 1);
    let x = crate::xmp::parse(out.as_bytes());
    assert_eq!(x.get_all("dc:subject").unwrap(), s(&["new", "A"]));
    assert_eq!(x.get("dc:title"), Some("Title"));
    assert_eq!(x.get("xmp:Rating"), Some("3"));
    // Writing the same again changes nothing.
    assert_eq!(write_xmp(Some(&out), &s(&["A|new"]), None), out);
}

#[test]
fn opens_an_empty_description_of_a_marks_sidecar() {
    // What the gallery writes for marks alone.
    let marks = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description rdf:about=\"\"\n   xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n   xmp:Rating=\"4\"/></rdf:RDF></x:xmpmeta>";
    let out = write_xmp(Some(marks), &s(&["sea"]), None);
    let x = crate::xmp::parse(out.as_bytes());
    assert!(x.error.is_none(), "{out}");
    assert_eq!(x.get("xmp:Rating"), Some("4"));
    assert_eq!(x.get_all("dc:subject").unwrap(), s(&["sea"]));
    assert_eq!(x.get_all("lr:hierarchicalSubject").unwrap(), s(&["sea"]));
}

#[test]
fn self_closed_and_attribute_properties_are_replaced() {
    let a = "<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\" dc:subject=\"x\"><lr:hierarchicalSubject/></rdf:Description></rdf:RDF></x:xmpmeta>";
    let out = write_xmp(Some(a), &s(&["y"]), None);
    assert!(!out.contains("dc:subject=\"x\""), "{out}");
    assert_eq!(out.matches("hierarchicalSubject>").count(), 2);
    let x = crate::xmp::parse(out.as_bytes());
    assert_eq!(x.get_all("dc:subject").unwrap(), s(&["y"]));
}

#[test]
fn embedded_keywords_from_xmp_then_iptc() {
    fn dataset(record: u8, number: u8, v: &[u8]) -> Vec<u8> {
        let mut b = vec![0x1C, record, number];
        b.extend((v.len() as u16).to_be_bytes());
        b.extend(v);
        b
    }
    let iim = [dataset(2, 25, b"one"), dataset(2, 25, b"two")].concat();
    let mut res = b"8BIM\x04\x04\0\0".to_vec();
    res.extend((iim.len() as u32).to_be_bytes());
    res.extend(&iim);
    let mut app13 = b"Photoshop 3.0\0".to_vec();
    app13.extend(&res);
    let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xED];
    jpeg.extend(((app13.len() + 2) as u16).to_be_bytes());
    jpeg.extend(&app13);
    jpeg.extend([0xFF, 0xD9]);
    assert_eq!(from_file_head(&jpeg), s(&["one", "two"]));
    let png_like = b"\x89PNG....<x:xmpmeta><rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\"><rdf:Description xmlns:dc=\"http://purl.org/dc/elements/1.1/\"><dc:subject><rdf:Bag><rdf:li>k</rdf:li></rdf:Bag></dc:subject></rdf:Description></rdf:RDF></x:xmpmeta>";
    assert_eq!(from_file_head(png_like), s(&["k"]));
    assert!(from_file_head(b"nothing").is_empty());
}

const LR_LIST: &str = "\u{feff}[Places]\r\n\tFrance\r\n\t\tParis\r\n\t\t\t{Lutetia}\r\n\t\t\t{Paname}\r\nPeople\r\n\t[Ann]\r\n\r\nSubjects\r\n\t\t\tDeep\r\n";

#[test]
fn lightroom_list_round_trip() {
    let v = Vocabulary::from_lightroom(LR_LIST);
    let paths: Vec<&str> = v.entries().iter().map(|e| e.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            "Places",
            "Places|France",
            "Places|France|Paris",
            "People",
            "People|Ann",
            "Subjects",
            "Subjects|Deep"
        ]
    );
    assert!(v.is_private("places") && v.is_private("People|Ann"));
    assert!(!v.is_private("People"));
    assert_eq!(
        v.get("Places|France|Paris").unwrap().synonyms,
        s(&["Lutetia", "Paname"])
    );
    assert_eq!(v.synonym_of("paname"), ["Places|France|Paris"]);
    let text = v.to_lightroom();
    assert_eq!(
        text,
        "People\r\n\t[Ann]\r\n[Places]\r\n\tFrance\r\n\t\tParis\r\n\t\t\t{Lutetia}\r\n\t\t\t{Paname}\r\nSubjects\r\n\tDeep\r\n"
    );
    let again = Vocabulary::from_lightroom(&text);
    assert_eq!(again.entries().len(), v.entries().len());
    assert_eq!(again.to_lightroom(), text);
}

#[test]
fn vocabulary_edits() {
    let mut v = Vocabulary::new();
    assert_eq!(v.add("places|France"), "places|France");
    // The known level keeps its spelling.
    assert_eq!(v.add("PLACES|Spain"), "places|Spain");
    v.set_synonyms("places|Spain", &s(&[" España ", "spain", "", "España"]));
    assert_eq!(v.get("places|spain").unwrap().synonyms, s(&["España"]));
    assert!(v.is_synonym("españa"));
    assert_eq!(v.remove("Places"), 3);
    assert!(v.is_empty() && !v.is_synonym("España"));
    let mut a = Vocabulary::from_lightroom("A\n\tB\n");
    let b = Vocabulary::from_lightroom("[A]\n\tC\n\t\t{c2}\n");
    assert_eq!(a.merge(&b), 1);
    assert!(a.is_private("A"));
    assert_eq!(a.get("A|C").unwrap().synonyms, s(&["c2"]));
}

#[test]
fn suggestions_rank_and_recents() {
    let mut v = Vocabulary::new();
    for p in [
        "Places|France|Paris",
        "People|Paris Hilton",
        "Animals|Parrot",
        "Events|Comparison",
        "Places|Spain",
    ] {
        v.add(p);
    }
    v.set_synonyms("Places|Spain", &s(&["Pa-espana"]));
    let mut r = Recents::default();
    let got = |r: &Recents, typed: &str| -> Vec<String> {
        suggest(&v, &s(&["Catalogue|pa"]), r, typed, &lower, 10)
            .into_iter()
            .map(|x| x.path)
            .collect()
    };
    assert_eq!(
        got(&r, "pa"),
        s(&[
            "Catalogue|pa",
            "Places|France|Paris",
            "Animals|Parrot",
            "People|Paris Hilton",
            "Places|Spain",
            "Events|Comparison",
        ])
    );
    r.used("Animals|Parrot");
    assert_eq!(got(&r, "pa")[0], "Animals|Parrot");
    // Upper levels typed narrow it.
    assert_eq!(
        got(&r, "places > pa"),
        s(&["Places|France|Paris", "Places|Spain"])
    );
    assert_eq!(got(&r, "fr>pa"), s(&["Places|France|Paris"]));
    assert!(got(&r, "   ").is_empty());
    let syn = suggest(&v, &[], &r, "pa-e", &lower, 10);
    assert_eq!(syn[0].synonym.as_deref(), Some("Pa-espana"));
    // A level above matches last.
    assert_eq!(got(&r, "anim"), s(&["Animals", "Animals|Parrot"]));
}

#[test]
fn recents_list() {
    let mut r = Recents::default();
    for i in 0..30 {
        r.used(&format!("k{i}"));
    }
    r.used("K29");
    assert_eq!(r.list().len(), RECENTS);
    assert_eq!(r.list()[0], "K29");
    assert_eq!(r.rank("k28"), Some(1));
    let back = Recents::from_text(&r.to_text());
    assert_eq!(back, r);
    r.forget("k28");
    assert_eq!(r.rank("k28"), None);
}

#[test]
fn changes() {
    let cur = s(&["A|B", "A|B|C", "x"]);
    let out = apply(
        &cur,
        &[
            Change::Add("y".into()),
            Change::Add("X".into()),
            Change::Remove("a|b".into()),
        ],
    );
    assert_eq!(out, s(&["A|B|C", "x", "y"]));
}

#[test]
fn exports_leave_private_keywords_out() {
    let mut v = Vocabulary::new();
    v.add("People|Ann");
    v.set_private("People|Ann", true);
    let side = write_xmp(None, &s(&["People|Ann", "Places|Paris"]), Some(&v));
    let (subject, hier) = for_export(&side).unwrap();
    assert_eq!(subject, s(&["Paris", "Places"]));
    assert_eq!(hier, s(&["Places|Paris"]));
    let out = write_lists(None, &subject, &hier);
    assert_eq!(
        from_xmp_text(&out, &|_| false).unwrap(),
        s(&["Places|Paris"])
    );
    assert_eq!(for_export("<x:xmpmeta xmlns:x=\"adobe:ns:meta/\"/>"), None);
}

#[test]
fn rename_move_and_merge() {
    assert_eq!(
        renamed("Places|Pari|Louvre", "places|pari", "Places|Paris"),
        Some("Places|Paris|Louvre".into())
    );
    assert_eq!(renamed("Places|Pariss", "Places|Pari", "X"), None);
    assert_eq!(
        renamed("Pari", "Pari", "Places|Paris"),
        Some("Places|Paris".into())
    );
    // Merged onto a keyword the photo has: kept once.
    assert_eq!(
        rename_in(
            &s(&["Paris", "Places|Paris", "sea"]),
            "Paris",
            "Places|Paris"
        ),
        s(&["Places|Paris", "sea"])
    );
    let mut v = Vocabulary::new();
    v.add("Places|Pari|Louvre");
    v.add("Places|Paris");
    v.set_synonyms("Places|Pari", &s(&["Lutetia"]));
    v.set_private("Places|Pari|Louvre", true);
    v.set_synonyms("Places|Paris", &s(&["City of Light"]));
    // Into itself: refused.
    assert_eq!(v.rename("Places|Pari", "Places|Pari|x"), 0);
    assert_eq!(v.rename("Nowhere", "x"), 0);
    // Onto Places|Paris: merged, synonyms joined, the private mark kept below.
    assert_eq!(v.rename("Places|Pari", "Places|Paris"), 2);
    assert!(!v.contains("Places|Pari"));
    assert_eq!(
        v.get("Places|Paris").unwrap().synonyms,
        s(&["City of Light", "Lutetia"])
    );
    assert!(v.is_private("Places|Paris|Louvre"));
    assert!(v.is_synonym("lutetia"));
    // A move to the top level makes its new parents.
    assert_eq!(v.rename("Places|Paris", "Cities|Paris"), 2);
    assert!(v.contains("Cities") && v.contains("Cities|Paris|Louvre"));
    assert!(v.contains("Places"), "the old parent stays");
}
