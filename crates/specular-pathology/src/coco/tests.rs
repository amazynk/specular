//! COCO round trips and RLE against the mask painter.

use super::*;
use crate::annot::Class;

pub(crate) fn sample() -> Vec<Annotation> {
    let class = |n: &str, c: [u8; 3]| {
        Some(Class {
            name: n.into(),
            colour: Some(c),
        })
    };
    let mut holed = Poly::new(vec![(10.0, 10.0), (50.0, 10.0), (50.0, 40.0), (10.0, 40.0)]);
    holed
        .holes
        .push(vec![(20.0, 20.0), (30.0, 20.0), (30.0, 30.0), (20.0, 30.0)]);
    vec![
        Annotation {
            name: "Region 1".into(),
            class: class("Tumor", [200, 0, 0]),
            geometry: Geometry::polygon(vec![(60.0, 5.0), (90.5, 12.25), (75.0, 45.0)]),
        },
        Annotation {
            name: String::new(),
            class: class("Stroma", [0, 150, 0]),
            geometry: Geometry::rect((5.0, 50.0), (35.0, 70.0)),
        },
        Annotation {
            name: "with a hole".into(),
            class: class("Tumor", [200, 0, 0]),
            geometry: Geometry::Area(vec![holed]),
        },
        Annotation {
            name: "nucleus".into(),
            class: None,
            geometry: Geometry::Point((80.5, 60.25)),
        },
        Annotation {
            name: String::new(),
            class: class("Immune cells", [0, 0, 255]),
            geometry: Geometry::Points(vec![(1.0, 1.0), (2.5, 3.5)]),
        },
        Annotation {
            name: "ruler".into(),
            class: None,
            geometry: Geometry::Line(vec![(0.0, 0.0), (10.0, 10.0)]),
        },
    ]
}

fn image() -> Image {
    Image {
        file_name: "slide.svs".into(),
        width: 100,
        height: 80,
    }
}

#[test]
fn round_trip_is_exact() {
    let items = sample();
    let (text, skipped) = write(&items, &image(), "Specular 0.1.0", "2026-10-25T12:00:00Z");
    assert_eq!(skipped.lines, 1);
    let back = read(&text).unwrap();
    assert_eq!(back.image, Some(image()));
    let want: Vec<Annotation> = items
        .into_iter()
        .filter(|a| !matches!(a.geometry, Geometry::Line(_)))
        .collect();
    assert_eq!(back.items, want);
    let v: Value = serde_json::from_str(&text).unwrap();
    assert_eq!(v["info"]["year"], 2026);
    assert_eq!(v["categories"][0]["name"], "Tumor");
    assert_eq!(v["categories"][2]["name"], crate::annot::UNCLASSIFIED);
    // The rectangle: polygon and box in level-0 pixels.
    let rect = &v["annotations"][1];
    assert_eq!(rect["bbox"], serde_json::json!([5, 50, 30, 20]));
    assert_eq!(
        rect["segmentation"][0],
        serde_json::json!([5, 50, 35, 50, 35, 70, 5, 70])
    );
    assert_eq!(rect["area"], 600);
    // The holed polygon is RLE.
    assert!(v["annotations"][2]["segmentation"]["counts"].is_array());
    assert_eq!(
        v["annotations"][3]["keypoints"],
        serde_json::json!([80.5, 60.25, 2])
    );
}

#[test]
fn foreign_files_without_specular_extras() {
    let text = r#"{"images":[{"id":7,"file_name":"a.tif","width":50,"height":40}],
        "categories":[{"id":3,"name":"cell"}],
        "annotations":[
          {"id":1,"image_id":7,"category_id":3,"segmentation":[[0,0,10,0,10,10,0,10]],"bbox":[0,0,10,10],"area":100,"iscrowd":0},
          {"id":2,"image_id":7,"category_id":3,"segmentation":{"size":[40,50],"counts":"abc"},"bbox":[1,2,3,4],"iscrowd":1},
          {"id":3,"image_id":8,"category_id":3,"bbox":[1,2,3,4]},
          {"id":4,"image_id":7,"category_id":3,"keypoints":[5,6,2,0,0,0]},
          {"id":5,"image_id":7,"category_id":3}
        ]}"#;
    let r = read(text).unwrap();
    assert_eq!(r.items.len(), 3);
    assert_eq!(r.rle_as_boxes, 1);
    assert_eq!(r.other_images, 1);
    assert_eq!(r.skipped, 1);
    assert_eq!(
        r.items[0].geometry,
        Geometry::polygon(vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])
    );
    assert_eq!(r.items[1].geometry, Geometry::rect((1.0, 2.0), (4.0, 6.0)));
    assert_eq!(r.items[2].geometry, Geometry::Point((5.0, 6.0)));
    assert_eq!(r.items[0].class_name(), Some("cell"));
    assert!(matches!(read("{}"), Err(Error::NotCoco)));
    assert!(matches!(read("{"), Err(Error::Json(_))));
}

#[test]
fn rle_matches_the_painted_mask() {
    let items = sample();
    let (w, h) = (100u32, 80u32);
    for a in items.iter().filter(|a| a.geometry.is_area()) {
        let polys = a.geometry.polys();
        let counts = rle(&polys, w, h);
        assert_eq!(counts.iter().sum::<u64>(), u64::from(w * h));
        // Decode column-major.
        let mut m = vec![0u8; (w * h) as usize];
        let (mut pos, mut on) = (0u64, false);
        for c in &counts {
            if on {
                for p in pos..pos + c {
                    let (x, y) = (p / u64::from(h), p % u64::from(h));
                    m[(y * u64::from(w) + x) as usize] = 1;
                }
            }
            pos += c;
            on = !on;
        }
        let cls = classes(std::slice::from_ref(a));
        let mut want = vec![0u16; (w * h) as usize];
        mask::paint(
            std::slice::from_ref(a),
            &cls,
            &mask::Region {
                x: 0.0,
                y: 0.0,
                w: 100.0,
                h: 80.0,
            },
            1.0,
            0,
            0,
            w,
            h,
            &mut want,
        );
        assert!(
            m.iter().zip(&want).all(|(a, b)| u16::from(*a) == *b),
            "{}",
            a.geometry.key()
        );
    }
    // Empty and full images.
    assert_eq!(rle(&[], 4, 3), vec![12]);
    let all = [Poly::new(vec![
        (0.0, 0.0),
        (4.0, 0.0),
        (4.0, 3.0),
        (0.0, 3.0),
    ])];
    assert_eq!(rle(&all, 4, 3), vec![0, 12]);
}
