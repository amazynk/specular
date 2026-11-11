use super::*;

fn req(s: &str) -> Request {
    parse_request(s.as_bytes()).unwrap().req
}

fn code(s: &[u8]) -> Code {
    parse_request(s).unwrap_err().2.code
}

/// The test commands (`idle`, `ui-state`): debug builds parse them from version 3 on, with
/// their waits capped; release builds do not know them.
#[test]
fn test_commands() {
    if cfg!(debug_assertions) {
        assert_eq!(
            req(r#"{"v":3,"cmd":"idle"}"#),
            Request::Idle {
                quiet_ms: 1000,
                timeout_ms: 30_000
            }
        );
        #[cfg(debug_assertions)]
        assert_eq!(
            req(r#"{"v":3,"cmd":"idle","quiet_ms":999999,"timeout_ms":99999999999}"#),
            Request::Idle {
                quiet_ms: 60_000,
                timeout_ms: crate::remote::server::IDLE_MAX_MS
            }
        );
        assert_eq!(req(r#"{"v":3,"cmd":"ui-state"}"#), Request::UiState);
        let r = Request::Idle {
            quiet_ms: 500,
            timeout_ms: 2000,
        };
        assert_eq!(req(&request_line(1, &r)), r);
    } else {
        assert_eq!(code(br#"{"v":3,"cmd":"idle"}"#), Code::UnknownCommand);
    }
    assert_eq!(code(br#"{"v":2,"cmd":"ui-state"}"#), Code::Version);
}

#[test]
fn requests() {
    assert_eq!(req(r#"{"v":1,"cmd":"hello"}"#), Request::Hello);
    let e = parse_request(br#"{"v":1,"id":"a7","cmd":"state","extra":[1]}"#).unwrap();
    assert_eq!((e.id, e.req), (json!("a7"), Request::State));
    #[cfg(unix)]
    {
        assert_eq!(
            req(r#"{"v":1,"cmd":"open","files":["/a b.jpg","file:///c%20d.png"]}"#),
            Request::Open {
                files: vec!["/a b.jpg".into(), "/c d.png".into()],
                window: Window::Current,
                view: None,
                compare: None,
            }
        );
        assert!(matches!(
            req(r#"{"v":1,"cmd":"open","files":["/a"],"window":"new"}"#),
            Request::Open {
                window: Window::New,
                ..
            }
        ));
    }
    assert_eq!(
        req(r#"{"v":1,"cmd":"navigate","to":"last"}"#),
        Request::Navigate(Nav::Last)
    );
    assert_eq!(
        req(r#"{"v":1,"cmd":"navigate","index":12}"#),
        Request::Navigate(Nav::Index(12))
    );
    assert_eq!(
        req(r#"{"v":1,"cmd":"navigate","by":-3}"#),
        Request::Navigate(Nav::By(-3))
    );
    assert_eq!(
        req(r#"{"v":1,"cmd":"action","name":"zoom-to:200"}"#),
        Request::Action {
            name: "zoom-to:200".into()
        }
    );
    assert_eq!(
        req(r#"{"v":1,"cmd":"open-url","url":"specular:///x"}"#),
        Request::OpenUrl {
            url: "specular:///x".into()
        }
    );
}

#[test]
fn errors() {
    assert_eq!(code(b"not json"), Code::Parse);
    assert_eq!(code(b"[1,2]"), Code::Parse);
    assert_eq!(code(b"\xff\xfe"), Code::Parse);
    assert_eq!(code(br#"{"cmd":"hello"}"#), Code::BadArguments);
    assert_eq!(code(br#"{"v":4,"cmd":"hello"}"#), Code::Version);
    assert_eq!(code(br#"{"v":1,"cmd":"view-get"}"#), Code::Version);
    assert_eq!(code(br#"{"v":"1","cmd":"hello"}"#), Code::BadArguments);
    assert_eq!(code(br#"{"v":1,"cmd":"rm -rf"}"#), Code::UnknownCommand);
    assert_eq!(code(br#"{"v":1}"#), Code::BadArguments);
    assert_eq!(
        code(br#"{"v":1,"cmd":"open","files":[]}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"open","files":["rel.jpg"]}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"open","files":[1]}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"open","files":["file://host/x"]}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"open","files":["/a"],"window":"other"}"#),
        Code::BadArguments
    );
    assert_eq!(code(br#"{"v":1,"cmd":"navigate"}"#), Code::BadArguments);
    assert_eq!(
        code(br#"{"v":1,"cmd":"navigate","to":"next","index":1}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"navigate","index":-1}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"navigate","index":1.5}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"action","name":""}"#),
        Code::BadArguments
    );
    assert_eq!(
        code(br#"{"v":1,"cmd":"action","name":"a\u0000b"}"#),
        Code::BadArguments
    );
    assert_eq!(code(br#"{"v":1,"cmd":"open-url"}"#), Code::BadArguments);
    // The id comes back on errors once it could be read.
    let (v, id, _) = parse_request(br#"{"v":1,"id":5,"cmd":"nope"}"#).unwrap_err();
    assert_eq!((v, id), (1, json!(5)));
    assert_eq!(
        code(br#"{"v":1,"id":{"x":1},"cmd":"hello"}"#),
        Code::BadArguments
    );
    // Deep nesting is refused by the parser's recursion limit, not by the stack.
    let deep = format!(
        "{{\"v\":1,\"cmd\":\"hello\",\"x\":{}{}}}",
        "[".repeat(100_000),
        "]".repeat(100_000)
    );
    assert_eq!(code(deep.as_bytes()), Code::Parse);
    let many: Vec<String> = (0..=MAX_FILES).map(|i| format!("\"/{i}\"")).collect();
    let big = format!(r#"{{"v":1,"cmd":"open","files":[{}]}}"#, many.join(","));
    assert_eq!(code(big.as_bytes()), Code::BadArguments);
    assert_eq!(code(&vec![b' '; MAX_LINE + 1]), Code::TooLong);
}

#[test]
fn responses_and_round_trip() {
    let ok = response(1, &json!(3), &Ok(json!({"index": 2})));
    let v: Value = serde_json::from_str(&ok).unwrap();
    assert_eq!(
        v,
        json!({"v": 1, "id": 3, "ok": true, "result": {"index": 2}})
    );
    let err = response(2, &Value::Null, &Err(Fail::new(Code::Disabled, "no-image")));
    let v: Value = serde_json::from_str(&err).unwrap();
    assert_eq!(v["error"]["code"], "disabled");
    assert_eq!(v["ok"], false);
    let huge = response(2, &Value::Null, &Ok(json!({"s": "x".repeat(MAX_LINE)})));
    assert!(huge.contains("too-long") && huge.len() < 200);
    for r in [
        Request::Hello,
        Request::Navigate(Nav::Index(4)),
        Request::Navigate(Nav::By(-2)),
        Request::Navigate(Nav::Prev),
        Request::State,
        Request::Marked,
        Request::Activate,
        Request::Quit,
        Request::OpenUrl {
            url: "specular:///a".into(),
        },
        Request::Action {
            name: "rotate-left".into(),
        },
    ] {
        assert_eq!(req(&request_line(1, &r)), r);
        assert_eq!(req(&request_line_v(1, 1, &r)), r, "v1 commands speak v1");
    }
}

/// Random bytes and mutated requests never panic and always get a well-formed response.
#[test]
fn random_lines() {
    let seeds: &[&[u8]] = &[
        br#"{"v":1,"id":1,"cmd":"open","files":["/a","file:///b%20c"],"window":"new"}"#,
        br#"{"v":1,"cmd":"navigate","by":-1}"#,
        br#"{"v":1,"cmd":"action","name":"zoom-to:50"}"#,
    ];
    let mut x: u64 = 0x2545_F491_4F6C_DD1D;
    let mut next = || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..30_000 {
        let mut b = seeds[i % seeds.len()].to_vec();
        for _ in 0..(next() % 6) {
            let pos = (next() as usize) % (b.len() + 1);
            let alphabet = b"{}[]\":,0-9e.\\u\x00\xffabc/";
            let c = alphabet[(next() as usize) % alphabet.len()];
            if next() % 2 == 0 {
                b.insert(pos, c);
            } else if pos < b.len() {
                b.remove(pos);
            }
        }
        let (ver, id, reply) = match parse_request(&b) {
            Ok(e) => (e.v, e.id, Ok(json!({}))),
            Err((v, id, f)) => (v, id, Err(f)),
        };
        let line = response(ver, &id, &reply);
        let v: Value = serde_json::from_str(&line).unwrap();
        assert!(v["v"] == 1 || v["v"] == 2 || v["v"] == 3);
    }
}

/// Every version 2 command survives the client's encoding and the viewer's parsing.
#[test]
fn v2_round_trips() {
    let mut text = Shape::new(ShapeKind::Text, vec![[3.0, 4.5]]);
    text.text = "cell 1".into();
    text.color = [10, 20, 30, 40];
    text.width = 1.5;
    let all = [
        Request::ViewGet,
        Request::ViewSet(ViewSet {
            zoom: Some(Zoom::Percent(250.0)),
            center: Some([10.5, -2.0]),
            levels: Some(Levels::Set {
                center: 40.0,
                width: 400.0,
            }),
            colormap: Some("viridis".into()),
            channel: Some(Channel::Index(2)),
        }),
        Request::ViewSet(ViewSet {
            zoom: Some(Zoom::Fit),
            levels: Some(Levels::Auto),
            channel: Some(Channel::All),
            ..ViewSet::default()
        }),
        Request::Metadata,
        Request::BlobNew { bytes: 1 << 20 },
        Request::BlobWrite {
            blob: 3,
            offset: 512,
            data: (0..=255).collect(),
        },
        Request::BlobRead {
            blob: 3,
            offset: 0,
            length: 100,
        },
        Request::BlobFree { blob: 3 },
        Request::ArrayShow {
            source: ArraySource::Blob(1),
            shape: vec![2, 3, 4],
            dtype: "<f4".into(),
            strides: Some(vec![48, 16, 4]),
            name: Some("my array.npy".into()),
        },
        Request::ArrayShow {
            source: ArraySource::File("frame-1.npy".into()),
            shape: vec![5],
            dtype: "|u1".into(),
            strides: None,
            name: None,
        },
        Request::OverlayAdd {
            layer: "cells".into(),
            shapes: vec![
                Shape::new(ShapeKind::Point, vec![[1.0, 2.0]]),
                Shape::new(ShapeKind::Line, vec![[0.0, 0.0], [5.0, 5.0]]),
                Shape::new(ShapeKind::Polygon, vec![[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]]),
                Shape::new(ShapeKind::Rect, vec![[0.0, 0.0], [5.0, 5.0]]),
                text,
            ],
        },
        Request::OverlayClear { layer: None },
        Request::OverlayClear {
            layer: Some("cells".into()),
        },
        Request::OverlayList,
        Request::Rois {
            format: RoiFormat::Json,
        },
        Request::Rois {
            format: RoiFormat::GeoJson,
        },
        Request::Screenshot,
        Request::ViewCommand { lang: Lang::Python },
        Request::ViewCommand { lang: Lang::Shell },
    ];
    for r in all {
        // Unchanged in version 2, and the same in version 3.
        let e = parse_request(request_line_v(2, 9, &r).as_bytes()).unwrap();
        assert_eq!((e.v, e.req), (2, r.clone()));
        let e = parse_request(request_line(9, &r).as_bytes()).unwrap();
        assert_eq!((e.v, e.req), (3, r.clone()));
        // Sent as version 1, a version 2 command is refused.
        let (v, id, f) = parse_request(request_line_v(1, 4, &r).as_bytes()).unwrap_err();
        assert_eq!((v, id, f.code), (1, json!(4), Code::Version), "{r:?}");
    }
    let names: Vec<&str> = COMMANDS_V1.iter().chain(COMMANDS_V2).copied().collect();
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), names.len());
}

/// Every version 3 command survives the client's encoding and the viewer's parsing, and is
/// refused in versions 1 and 2.
#[test]
fn v3_round_trips() {
    let files = vec![
        crate::remote::test_abs("/p/a.cr3"),
        crate::remote::test_abs("/p/b c;d.jpg"),
    ];
    let all = [
        Request::Subscribe {
            events: vec!["marked".into(), "exported".into()],
        },
        Request::Subscribe {
            events: vec!["*".into()],
        },
        Request::Unsubscribe { events: None },
        Request::Unsubscribe {
            events: Some(vec!["opened".into()]),
        },
        Request::Select {
            files: files.clone(),
            add: false,
        },
        Request::Select {
            files: files.clone(),
            add: true,
        },
        Request::Mark {
            files: files.clone(),
            mark: MarkSet {
                flag: Some(Flag::Pick),
                stars: Some(4),
                label: None,
            },
        },
        Request::Mark {
            files: files.clone(),
            mark: MarkSet {
                flag: Some(Flag::None),
                stars: Some(0),
                label: Some(3),
            },
        },
        Request::ApplyStyle {
            files: files.clone(),
            style: "Warm".into(),
        },
        Request::Export {
            files,
            recipe: "Web".into(),
        },
    ];
    for r in all {
        let e = parse_request(request_line(9, &r).as_bytes()).unwrap();
        assert_eq!((e.v, e.req), (3, r.clone()));
        for v in [1, 2] {
            let (v2, id, f) = parse_request(request_line_v(v, 4, &r).as_bytes()).unwrap_err();
            assert_eq!((v2, id, f.code), (v, json!(4), Code::Version), "{r:?}");
        }
    }
    let names: Vec<&str> = COMMANDS_V1
        .iter()
        .chain(COMMANDS_V2)
        .chain(COMMANDS_V3)
        .copied()
        .collect();
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), names.len());
    // `null` clears, as `none` does.
    let line = json!({"v": 3, "cmd": "mark", "files": [crate::remote::test_abs("/a")], "flag": null, "label": null});
    let r = req(&line.to_string());
    let Request::Mark { mark, .. } = r else {
        panic!()
    };
    assert_eq!(
        (mark.flag, mark.label, mark.stars),
        (Some(Flag::None), Some(0), None)
    );
    // A name given twice counts once.
    assert_eq!(
        req(r#"{"v":3,"cmd":"subscribe","events":["marked","marked"]}"#),
        Request::Subscribe {
            events: vec!["marked".into()]
        }
    );
    assert_eq!(
        hello_extras(br#"{"v":3,"cmd":"hello","key":"abc","hook":"Up"}"#),
        (Some("abc".into()), Some("Up".into()))
    );
    assert_eq!(
        hello_extras(br#"{"v":3,"cmd":"hello","hook":"a\nb"}"#),
        (None, None)
    );
}

#[test]
fn v3_limits() {
    let c = |s: &str| code(s.as_bytes());
    for bad in [
        r#"{"v":3,"cmd":"subscribe"}"#,
        r#"{"v":3,"cmd":"subscribe","events":[]}"#,
        r#"{"v":3,"cmd":"subscribe","events":"marked"}"#,
        r#"{"v":3,"cmd":"subscribe","events":[1]}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"]}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"],"stars":6}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"],"flag":"maybe"}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"],"label":"purple"}"#,
        r#"{"v":3,"cmd":"mark","files":["a"],"stars":1}"#,
        r#"{"v":3,"cmd":"mark","files":[],"stars":1}"#,
        r#"{"v":3,"cmd":"select","files":["/a"],"add":"yes"}"#,
        r#"{"v":3,"cmd":"apply-style","files":["/a"]}"#,
        r#"{"v":3,"cmd":"export","files":["/a"],"recipe":""}"#,
    ] {
        assert_eq!(c(bad), Code::BadArguments, "{bad}");
    }
    assert_eq!(
        c(r#"{"v":3,"cmd":"subscribe","events":["clicked"]}"#),
        Code::UnknownEvent
    );
    let many: Vec<String> = (0..=MAX_EVENT_NAMES)
        .map(|_| "\"marked\"".to_owned())
        .collect();
    let line = format!(
        r#"{{"v":3,"cmd":"subscribe","events":[{}]}}"#,
        many.join(",")
    );
    assert_eq!(c(&line), Code::BadArguments);
    assert_eq!(Code::UnknownRecipe.as_str(), "unknown-recipe");
    assert_eq!(Code::UnknownStyle.as_str(), "unknown-style");
}

#[test]
fn v2_limits() {
    let c = |s: &str| code(s.as_bytes());
    for bad in [
        r#"{"v":2,"cmd":"view-set"}"#,
        r#"{"v":2,"cmd":"view-set","zoom":0}"#,
        r#"{"v":2,"cmd":"view-set","zoom":"huge"}"#,
        r#"{"v":2,"cmd":"view-set","center":[1]}"#,
        r#"{"v":2,"cmd":"view-set","center":[1e300,1]}"#,
        r#"{"v":2,"cmd":"view-set","window":{"center":1,"width":0}}"#,
        r#"{"v":2,"cmd":"view-set","window":7}"#,
        r#"{"v":2,"cmd":"view-set","channel":-1}"#,
        r#"{"v":2,"cmd":"blob-new","bytes":0}"#,
        r#"{"v":2,"cmd":"blob-new","bytes":99999999999999}"#,
        r#"{"v":2,"cmd":"blob-write","blob":1,"offset":0,"data":"***"}"#,
        r#"{"v":2,"cmd":"blob-write","blob":1,"data":"AA=="}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"file":"a.npy","shape":[1],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","file":"../etc/passwd","shape":[1],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","file":"/etc/passwd","shape":[1],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","file":".hidden","shape":[1],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[0],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[1,1,1,1,1,1,1,1,1],"dtype":"u1"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[2],"dtype":"O"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[2],"dtype":"<c16"}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[2],"dtype":"u1","strides":[1,1]}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[2],"dtype":"u1","name":"a/b"}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"blob","points":[[0,0]]}]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"line","points":[[0,0]]}]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"polygon","points":[[0,0],[1,1]]}]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"text","points":[[0,0]]}]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"point","points":[[0,0]],"color":"red"}]}"#,
        r#"{"v":2,"cmd":"overlay-add","shapes":[{"kind":"point","points":[[0,0]],"width":99}]}"#,
        r#"{"v":2,"cmd":"overlay-add","layer":"","shapes":[{"kind":"point","points":[[0,0]]}]}"#,
        r#"{"v":2,"cmd":"rois","format":"shapefile"}"#,
        r#"{"v":2,"cmd":"view-command","lang":"perl"}"#,
    ] {
        assert_eq!(c(bad), Code::BadArguments, "{bad}");
    }
    // Too many shapes, or too many points in one shape.
    let many = vec![r#"{"kind":"point","points":[[0,0]]}"#; MAX_SHAPES + 1].join(",");
    assert_eq!(
        c(&format!(
            r#"{{"v":2,"cmd":"overlay-add","shapes":[{many}]}}"#
        )),
        Code::BadArguments
    );
    let long = vec!["[1,1]"; MAX_POINTS + 1].join(",");
    let line = format!(
        r#"{{"v":2,"cmd":"overlay-add","shapes":[{{"kind":"polyline","points":[{long}]}}]}}"#
    );
    assert!(line.len() < MAX_LINE);
    assert_eq!(c(&line), Code::BadArguments);
    // A chunk over the limit.
    let big = specular_client::base64::encode(&vec![0; MAX_CHUNK + 3]);
    let line = format!(r#"{{"v":2,"cmd":"blob-write","blob":1,"offset":0,"data":"{big}"}}"#);
    assert_eq!(c(&line), Code::BadArguments);
    assert!(dtype_ok("<f8") && dtype_ok("|b1") && dtype_ok("u2") && !dtype_ok("<<f8"));
}

/// Version 2 and 3 requests under 20 000 seeded random mutations (bytes, and JSON values
/// swapped for extreme or wrongly typed ones): parsing never panics, every outcome gets a
/// well-formed response, and whatever parses survives the client's encoding unchanged.
#[test]
fn v3_mutated_lines() {
    let seeds: &[&str] = &[
        r#"{"v":3,"id":1,"cmd":"subscribe","events":["marked","exported","*"]}"#,
        r#"{"v":3,"id":"a","cmd":"unsubscribe","events":["opened"]}"#,
        r#"{"v":3,"cmd":"select","files":["/p/a.cr3","/p/b c;d.jpg"],"add":true}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"],"flag":"pick","stars":4,"label":3}"#,
        r#"{"v":3,"cmd":"mark","files":["/a"],"flag":null,"label":null}"#,
        r#"{"v":3,"cmd":"apply-style","files":["/a"],"style":"Warm"}"#,
        r#"{"v":3,"cmd":"export","files":["/a","/b"],"recipe":"Web"}"#,
        r#"{"v":3,"cmd":"hello","key":"abc","hook":"Up"}"#,
        r#"{"v":3,"cmd":"open","files":["/a","file:///b%20c"],"window":"new"}"#,
        r#"{"v":3,"cmd":"navigate","by":-1}"#,
        r#"{"v":2,"cmd":"view-set","zoom":250,"center":[10.5,-2],"window":{"center":40,"width":400},"colormap":"viridis","channel":2}"#,
        r#"{"v":2,"cmd":"blob-new","bytes":1048576}"#,
        r#"{"v":2,"cmd":"blob-write","blob":3,"offset":512,"data":"AAEC"}"#,
        r#"{"v":2,"cmd":"blob-read","blob":3,"offset":0,"length":100}"#,
        r#"{"v":2,"cmd":"array-show","blob":1,"shape":[2,3,4],"dtype":"<f4","strides":[48,16,4],"name":"my array.npy"}"#,
        r#"{"v":2,"cmd":"overlay-add","layer":"cells","shapes":[{"kind":"text","points":[[3,4.5]],"text":"cell 1","color":[10,20,30,40],"width":1.5},{"kind":"polygon","points":[[0,0],[5,0],[5,5]]}]}"#,
        r#"{"v":2,"cmd":"rois","format":"geojson"}"#,
        r#"{"v":2,"cmd":"view-command","lang":"python"}"#,
    ];
    let values: &[&str] = &[
        "null",
        "true",
        "0",
        "-1",
        "1e308",
        "-1e308",
        "1e-320",
        "18446744073709551615",
        "18446744073709551616",
        "-9223372036854775808",
        "0.5",
        "\"\"",
        "\"*\"",
        "\"/\"",
        "\"\\u0000\"",
        "\"\\ud800\"",
        "[]",
        "[[]]",
        "[1]",
        "[1,2,3,4,5,6,7,8,9,10]",
        "{}",
        "[\"/a\"]",
        "[[0,0],[1e308,-1e308]]",
        "{\"center\":0,\"width\":-0}",
    ];
    let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut next = move || {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x
    };
    for i in 0..20_000 {
        let mut b = seeds[i % seeds.len()].as_bytes().to_vec();
        for _ in 0..1 + next() % 4 {
            match next() % 4 {
                // A value after a colon or comma swapped for another.
                0 | 1 => {
                    let at: Vec<usize> = b
                        .iter()
                        .enumerate()
                        .filter(|(_, c)| matches!(c, b':' | b','))
                        .map(|(k, _)| k + 1)
                        .collect();
                    if at.is_empty() {
                        continue;
                    }
                    let from = at[next() as usize % at.len()];
                    let end = (from..b.len())
                        .find(|&k| matches!(b[k], b',' | b'}' | b']'))
                        .unwrap_or(b.len());
                    let v = values[next() as usize % values.len()].as_bytes();
                    b.splice(from..end, v.iter().copied());
                }
                2 => {
                    let pos = next() as usize % (b.len() + 1);
                    let alphabet = b"{}[]\":,0-9e.\\u\x00\xffabc/";
                    b.insert(pos, alphabet[next() as usize % alphabet.len()]);
                }
                _ => {
                    if !b.is_empty() {
                        let pos = next() as usize % b.len();
                        b.remove(pos);
                    }
                }
            }
        }
        let _ = hello_extras(&b);
        let (ver, id, reply) = match parse_request(&b) {
            Ok(e) => {
                let line = String::from_utf8_lossy(&b);
                match parse_request(request_line_v(e.v, 1, &e.req).as_bytes()) {
                    Ok(a) => assert_eq!(a.req, e.req, "{line}"),
                    Err((_, _, f)) => panic!("{line}: re-encoded request refused: {}", f.message),
                }
                (e.v, e.id, Ok(json!({})))
            }
            Err((v, id, f)) => (v, id, Err(f)),
        };
        let line = response(ver, &id, &reply);
        assert!(line.len() <= MAX_LINE);
        let v: Value = serde_json::from_str(&line).unwrap();
        assert!(v["v"] == 1 || v["v"] == 2 || v["v"] == 3);
    }
}
