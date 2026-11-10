use super::*;

pub(crate) fn ai_offers() -> AiPage {
    let row = |feature, state, size, licences: &str, models: &str| AiFeatureRow {
        feature,
        state,
        size,
        licences: licences.into(),
        models: models.into(),
        ..Default::default()
    };
    AiPage {
        features: vec![
            row(
                "search",
                AiRowState::Downloading {
                    done: 61_000_000,
                    total: 152_000_000,
                },
                152_000_000,
                "Apache-2.0",
                "image-text 1",
            ),
            row(
                "text",
                AiRowState::Failed("checksum mismatch".into()),
                18_500_000,
                "Apache-2.0",
                "text-detect 1, text-read 1",
            ),
            row(
                "culling",
                AiRowState::Available,
                9_200_000,
                "MIT",
                "aesthetic 1",
            ),
            row(
                "people",
                AiRowState::Available,
                31_000_000,
                "MIT",
                "faces 1",
            ),
        ],
        people: Vec::new(),
        gpu: None,
        face_grouping: None,
        text_indexing: None,
    }
}

/// The Local AI page with features installed: search done on the GPU, people still indexing
/// on the CPU (its GPU result differed).
pub(crate) fn ai_installed() -> AiPage {
    let mut p = ai_offers();
    p.features[0].state = AiRowState::Installed {
        indexed: Some((12_480, 12_480)),
    };
    p.features[0].runs_on = Some(AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into())));
    p.features[3].runs_on = Some(AiEngine::Cpu("differed"));
    p.features[1].state = AiRowState::Available;
    p.features[3].state = AiRowState::Installed {
        indexed: Some((4_210, 12_480)),
    };
    p.people = vec![
        AiPerson {
            id: 1,
            name: "Ana".into(),
            faces: 412,
        },
        AiPerson {
            id: 2,
            name: String::new(),
            faces: 57,
        },
        AiPerson {
            id: 3,
            name: "Tomás".into(),
            faces: 9,
        },
    ];
    p
}

/// People and Culling as the built-in manifest offers them: People's two models
/// (SFace, the default, trained on research-only face sets; AuraFace) with grouping off,
/// Culling's Face Landmarker installed and checked on the GPU.
pub(crate) fn ai_faces() -> AiPage {
    let info = |name: &str, hint: &str, data: &str, terms, host: &str| AiModelInfo {
        name: name.into(),
        hint: hint.into(),
        data: data.into(),
        data_terms: terms,
        host: host.into(),
        ..Default::default()
    };
    let size = |tier, download: u64, memory_mb, info| AiSize {
        tier,
        download,
        size: download,
        on_disk: false,
        memory_mb,
        needs_gpu: false,
        unfit: None,
        info,
    };
    let people = AiFeatureRow {
        feature: "people",
        state: AiRowState::Installed {
            indexed: Some((1_204, 3_310)),
        },
        size: 39_780_139,
        licences: "Apache-2.0".into(),
        models: "blazeface-full-range 1, sface-2021dec 1".into(),
        sizes: vec![
            size(
                "standard",
                39_780_139,
                224,
                info(
                    "BlazeFace + SFace",
                    "",
                    "CASIA-WebFace, VGGFace2, MS1MV2",
                    "research",
                    "huggingface.co",
                ),
            ),
            size(
                "large",
                261_777_937,
                764,
                info("BlazeFace + AuraFace", "", "", "unstated", "huggingface.co"),
            ),
        ],
        tier: Some("standard"),
        recommended: Some("standard"),
        choice: "recommended",
        chosen: Some("standard"),
        runs_on: Some(AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into()))),
        ..Default::default()
    };
    let culling = AiFeatureRow {
        feature: "culling",
        state: AiRowState::Installed {
            indexed: Some((3_310, 3_310)),
        },
        size: 3_758_596,
        licences: "Apache-2.0".into(),
        models: "face-landmarker 1".into(),
        sizes: vec![size(
            "standard",
            3_758_596,
            64,
            info(
                "MediaPipe Face Landmarker",
                "closed-eyes",
                "Google, consented images",
                "stated",
                "storage.googleapis.com",
            ),
        )],
        tier: Some("standard"),
        recommended: Some("standard"),
        choice: "recommended",
        chosen: Some("standard"),
        runs_on: Some(AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into()))),
        ..Default::default()
    };
    AiPage {
        features: vec![culling, people],
        people: Vec::new(),
        gpu: None,
        face_grouping: Some(false),
        text_indexing: None,
    }
}

/// Search with three model sizes (made-up figures in the range of the real ones), on a machine
/// whose graphics card suits Standard, in the state the shot names.
pub(crate) fn ai_sized(state: State) -> AiPage {
    let size = |tier, download, memory_mb, needs_gpu| AiSize {
        tier,
        download,
        size: download,
        on_disk: false,
        memory_mb,
        needs_gpu,
        unfit: None,
        info: Default::default(),
    };
    let mut search = AiFeatureRow {
        feature: "search",
        size: 600_000_000,
        licences: "MIT".into(),
        models: "clip-b32-256 1".into(),
        sizes: vec![
            size("small", 150_000_000, 600, false),
            size("standard", 600_000_000, 1_500, false),
            size("large", 1_700_000_000, 4_000, true),
        ],
        licence_links: vec![AiLicence {
            name: "MIT".into(),
            url: "https://github.com/mlfoundations/open_clip/blob/main/LICENSE".into(),
            models: "clip-b32-256 1".into(),
        }],
        recommended: Some("standard"),
        choice: "recommended",
        chosen: Some("standard"),
        ..Default::default()
    };
    let installed = AiRowState::Installed {
        indexed: Some((12_480, 12_480)),
    };
    // On a machine with 3 GB for models, Large does not fit (shown with why).
    let too_big = |search: &mut AiFeatureRow| {
        search.sizes[2].unfit = Some(AiUnfit::Memory {
            need_mb: 4_000,
            have_mb: 3_000,
        });
    };
    match state {
        State::AiSizeOff => too_big(&mut search),
        State::AiSizeNamed => {
            too_big(&mut search);
            search.state = installed;
            search.tier = Some("small");
            search.choice = "small";
            search.chosen = Some("small");
            search.size = 150_000_000;
        }
        State::AiSizeRecommended => {
            search.state = installed;
            search.tier = Some("standard");
            search.question = Some(AiQuestion {
                tier: "large",
                download: 1_700_000_000,
                suggested: true,
            });
        }
        State::AiSizeDownloading => {
            search.state = AiRowState::Downloading {
                done: 228_000_000,
                total: 600_000_000,
            };
            search.target = Some("standard");
        }
        State::AiSizeSwitching => {
            search.state = AiRowState::Installed {
                indexed: Some((3_120, 12_480)),
            };
            search.tier = Some("large");
            search.previous = Some("standard");
            search.choice = "large";
            search.chosen = Some("large");
            search.size = 1_700_000_000;
        }
        _ => {
            search.state = AiRowState::Failed("connection reset".into());
            search.tier = Some("standard");
            search.target = Some("large");
            search.choice = "large";
            search.chosen = Some("large");
            search.size = 1_700_000_000;
        }
    }
    // The sizes installed are on disk: nothing to download for them.
    for z in &mut search.sizes {
        if search.tier == Some(z.tier) || search.previous == Some(z.tier) {
            z.on_disk = true;
            z.download = 0;
        }
    }
    AiPage {
        features: vec![search],
        people: Vec::new(),
        gpu: Some("NVIDIA GeForce RTX 3060".into()),
        face_grouping: None,
        text_indexing: None,
    }
}

/// Text installed (PP-OCRv6 small), at its model setting: the library's text switch, both
/// sizes, and the add-ons installed, downloading, failed, available and removing.
pub(crate) fn ai_text_addons() -> AiPage {
    let info = |name: &str, languages: &str, data: &str| AiModelInfo {
        name: name.into(),
        languages: languages.into(),
        hint: String::new(),
        data: data.into(),
        data_terms: "unstated",
        host: "huggingface.co".into(),
    };
    let size = |tier, bytes, name: &str, languages: &str| AiSize {
        tier,
        download: if tier == "standard" { 0 } else { bytes },
        size: bytes,
        on_disk: tier == "standard",
        memory_mb: 520,
        needs_gpu: false,
        unfit: None,
        info: info(name, languages, ""),
    };
    let addon = |id: &str, name: &str, languages: &str, bytes, state| AiAddon {
        id: id.into(),
        info: AiModelInfo {
            host: if id == "hebrew" {
                "raw.githubusercontent.com".into()
            } else {
                "huggingface.co".into()
            },
            ..info(
                name,
                languages,
                if id == "hebrew" {
                    "Synthetic text lines rendered by Google"
                } else {
                    ""
                },
            )
        },
        size: bytes,
        licence: "Apache-2.0".into(),
        state,
    };
    let text = AiFeatureRow {
        feature: "text",
        state: AiRowState::Installed {
            indexed: Some((380, 12_480)),
        },
        size: 31_400_000,
        licences: "Apache-2.0".into(),
        models: "ppocrv6-small-det 1, ppocrv6-small-rec 1".into(),
        sizes: vec![
            size("small", 14_500_000, "PP-OCRv6 tiny", "zh-latin"),
            size("standard", 31_400_000, "PP-OCRv6 small", "cjk-latin"),
        ],
        tier: Some("standard"),
        recommended: Some("standard"),
        choice: "recommended",
        chosen: Some("standard"),
        runs_on: Some(AiEngine::Gpu(Some("AMD Radeon 860M Graphics".into()))),
        addons: vec![
            addon(
                "arabic",
                "PP-OCRv5 Arabic",
                "arabic",
                22_625_000,
                AiRowState::Installed { indexed: None },
            ),
            addon(
                "hebrew",
                "Tesseract Hebrew",
                "hebrew",
                3_704_077,
                AiRowState::Downloading {
                    done: 1_200_000,
                    total: 3_704_077,
                },
            ),
            addon(
                "korean",
                "PP-OCRv5 Korean",
                "korean",
                28_170_000,
                AiRowState::Failed("connection reset".into()),
            ),
            addon(
                "greek",
                "PP-OCRv5 Greek",
                "greek",
                22_430_000,
                AiRowState::Available,
            ),
            addon(
                "thai",
                "PP-OCRv5 Thai",
                "thai",
                22_510_000,
                AiRowState::Removing,
            ),
        ],
        licence_links: vec![AiLicence {
            name: "Apache-2.0".into(),
            url: "https://huggingface.co/PaddlePaddle/PP-OCRv6_small_rec_safetensors".into(),
            models: "ppocrv6-small-det 1, ppocrv6-small-rec 1".into(),
        }],
        ..Default::default()
    };
    AiPage {
        features: vec![text],
        people: Vec::new(),
        gpu: Some("AMD Radeon 860M Graphics".into()),
        face_grouping: None,
        text_indexing: Some(true),
    }
}

/// This feature's goldens ([`super::cases`]).
pub(crate) fn ai_shot_cases() -> Vec<Case> {
    let c = |name, state, dark, lang| Case {
        name,
        state,
        dark,
        lang,
    };
    vec![
        c("ai-none-dark", State::AiNone, true, langid!("en-US")),
        c("ai-offers-light", State::AiOffers, false, langid!("en-US")),
        c("ai-offers-he-dark", State::AiOffers, true, langid!("he")),
        c(
            "ai-installed-dark",
            State::AiInstalled,
            true,
            langid!("en-US"),
        ),
        c("ai-people-light", State::AiPeople, false, langid!("en-US")),
        c("ai-faces-dark", State::AiFaces, true, langid!("en-US")),
        c("ai-faces-he-light", State::AiFaces, false, langid!("he")),
        c("ai-text-2-dark", State::AiLargeText, true, langid!("en-US")),
        c(
            "ai-size-off-light",
            State::AiSizeOff,
            false,
            langid!("en-US"),
        ),
        c(
            "ai-size-recommended-dark",
            State::AiSizeRecommended,
            true,
            langid!("en-US"),
        ),
        c(
            "ai-size-recommended-he-light",
            State::AiSizeRecommended,
            false,
            langid!("he"),
        ),
        c(
            "ai-size-downloading-light",
            State::AiSizeDownloading,
            false,
            langid!("en-US"),
        ),
        c(
            "ai-size-switching-dark",
            State::AiSizeSwitching,
            true,
            langid!("en-US"),
        ),
        c(
            "ai-size-failed-light",
            State::AiSizeFailed,
            false,
            langid!("en-US"),
        ),
        c(
            "ai-size-named-light",
            State::AiSizeNamed,
            false,
            langid!("en-US"),
        ),
        c(
            "ai-size-named-he-dark",
            State::AiSizeNamed,
            true,
            langid!("he"),
        ),
        c(
            "ai-text-addons-dark",
            State::AiTextAddons,
            true,
            langid!("en-US"),
        ),
        c(
            "ai-text-addons-he-light",
            State::AiTextAddons,
            false,
            langid!("he"),
        ),
    ]
}
