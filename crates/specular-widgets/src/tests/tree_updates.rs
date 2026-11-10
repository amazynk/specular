use crate::accesskit::{Live, Role, TreeUpdate};
use crate::{Chrome, HudStats, Modal, Overlays, ViewerState};
use accesskit_consumer::{NodeRef, Tree, TreeChangeHandler};
use specular_gfx::{DrawList, Text};
use specular_input::{ActionState, ImageState, Keymap};
use specular_metadata::{Field, FieldGroup, MetadataReport, SummaryGroup, SummaryItem};
use specular_settings::{Settings, SettingsStore};
use specular_theme::Palette;

struct Ignore;
impl TreeChangeHandler for Ignore {
    fn node_added(&mut self, _: &NodeRef) {}
    fn node_updated(&mut self, _: &NodeRef, _: &NodeRef) {}
    fn focus_moved(&mut self, _: Option<&NodeRef>, _: Option<&NodeRef>) {}
    fn node_removed(&mut self, _: &NodeRef) {}
}

fn report() -> MetadataReport {
    MetadataReport {
        summary: vec![SummaryGroup {
            key: "camera",
            title: "Camera".into(),
            items: vec![SummaryItem {
                key: "model",
                label: "Model".into(),
                value: "X100".into(),
                detail: None,
            }],
        }],
        all_fields: vec![FieldGroup {
            title: "Exif".into(),
            fields: vec![Field {
                tag: "Model".into(),
                raw: "X100".into(),
                display: "X100".into(),
                empty: false,
                full: None,
            }],
        }],
        language: "en".into(),
        ..Default::default()
    }
}

#[derive(Clone, Copy, Debug)]
enum Step {
    Info,
    Modal(Modal),
    Close,
    Toast,
    Next,
    Error,
    /// The user zooms in (same image).
    Zoom,
    /// The next image, at its own (different) zoom.
    NextZoomed,
    Type(&'static str),
}

const STEPS: [Step; 14] = [
    Step::Info,
    Step::Modal(Modal::Palette),
    Step::Modal(Modal::CheatSheet),
    Step::Modal(Modal::Settings),
    Step::Modal(Modal::Language),
    Step::Modal(Modal::ConfirmTrash),
    Step::Modal(Modal::About),
    Step::Modal(Modal::HwReport),
    Step::Modal(Modal::Insights),
    Step::Close,
    Step::Toast,
    Step::Next,
    Step::Error,
    Step::Info,
];

/// Runs `steps` from a fresh chrome, feeding every frame's tree to AccessKit's consumer;
/// returns the last tree.
fn replay(steps: &[Step]) -> TreeUpdate {
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let meta = report();
    let names = ["beach.jpg", "harbour.jpg", "snow.jpg", "zz-broken.jpg"];
    let mut text = Text::new();
    let mut c = Chrome::new();
    c.set_size(1000.0, 700.0);
    c.set_accessibility(true);
    c.poke(1.0);
    let (mut index, mut error, mut now, mut zoom) = (0usize, false, 1.0, 1.0);
    let mut tree: Option<Tree> = None;
    let mut last = None;
    for step in std::iter::once(None).chain(steps.iter().copied().map(Some)) {
        match step {
            None => {}
            Some(Step::Info) => c.toggle_info(),
            Some(Step::Modal(m)) => c.open_modal(m),
            Some(Step::Close) => c.close_modal(),
            Some(Step::Toast) => {
                c.toast("Zoom 200%".into(), None, now);
            }
            Some(Step::Next) => index = (index + 1) % names.len(),
            Some(Step::Error) => error = !error,
            Some(Step::Zoom) => zoom *= 2.0,
            Some(Step::NextZoomed) => {
                index = (index + 1) % names.len();
                zoom *= 0.5;
            }
            Some(Step::Type(_)) => {}
        }
        let st = ViewerState {
            actions: ActionState {
                comparing: false,
                compare_images: 0,
                volume: false,
                mpr: false,
                image: if error {
                    ImageState::Failed
                } else {
                    ImageState::Ready
                },
                count: names.len(),
                gallery_files: 0,
                animated: false,
                video: false,
                sound: false,
                sound_file: false,
                in_archive: false,
                has_alpha: false,
                gallery_views: None,
                medical: None,
                dicom: false,
                series: 0,
                presentation_states: 0,
                science: None,
                debayer: false,
                annotations: false,
                stain: None,
                stain_region: false,
                slide_annotations: false,
                thermal_visible: false,
                drawing: false,
                mesh: false,
                points: false,
                molecule: false,
                tracts: false,
                relight: false,
                map: None,
                import_sources: false,
                pages: 0,
                rotatable: false,
                wallpaper: false,
                share: false,
                email: false,
                profile: specular_input::Profile::UNRESTRICTED,
            },
            has_image: !error,
            index,
            count: names.len(),
            zoom,
            at_fit: zoom == 1.0,
            fullscreen: false,
            file_name: Some(names[index]),
            dims: (!error).then_some((7, 5)),
            loading: false,
            error: error.then_some("unsupported format"),
            truncated: false,
            metadata: (!error).then_some(&meta),
            hud: &hud,
            keymap: &keymap,
            store: &store,
            settings: &settings,
            overlays: Overlays::default(),
            playback: None,
            motion: None,
            spread: None,
            pages: None,
        };
        if let Some(Step::Type(t)) = step {
            c.input(&crate::Input::Text(t.into()), &st, now);
        }
        let mut dl = DrawList::new(1.0);
        for _ in 0..4 {
            now += 0.016;
            dl.clear(1.0);
            c.draw(&mut dl, &mut text, &st, &Palette::dark(), now);
            let update = c.accessibility_tree(&st, names[index], 1.0);
            // Every node but the root hangs off another one.
            let children: std::collections::HashSet<_> = update
                .nodes
                .iter()
                .flat_map(|(_, n)| n.children().to_vec())
                .collect();
            for (id, n) in &update.nodes {
                assert!(
                    *id == update.tree.as_ref().unwrap().root || children.contains(id),
                    "after {step:?}: orphan {id:?} {:?}",
                    n.role()
                );
            }
            last = Some(update.clone());
            match &mut tree {
                None => tree = Some(Tree::new(update, true)),
                Some(t) => t.update_and_process_changes(update, &mut Ignore),
            }
        }
    }
    last.unwrap()
}

fn statuses(t: &TreeUpdate) -> Vec<String> {
    t.nodes
        .iter()
        .filter(|(_, n)| n.role() == Role::Status)
        .filter_map(|(_, n)| n.label().map(str::to_owned))
        .collect()
}

/// Zooming is announced; a new image opening at its own zoom is not.
#[test]
fn zoom_changes_are_announced() {
    let t = replay(&[Step::Zoom]);
    assert!(
        statuses(&t).iter().any(|s| s.contains("200")),
        "{:?}",
        statuses(&t)
    );
    let t = replay(&[Step::NextZoomed]);
    assert!(
        !statuses(&t).iter().any(|s| s.contains('%')),
        "{:?}",
        statuses(&t)
    );
}

/// Names reach the screen reader without Fluent's bidi isolation marks; typed text is kept.
#[test]
fn names_without_isolation_marks() {
    let mut t = replay(&[Step::Modal(Modal::Palette), Step::Type("a\u{2068}b")]);
    let isolated = |t: &TreeUpdate| {
        t.nodes
            .iter()
            .filter(|(_, n)| {
                [n.label(), n.description()]
                    .iter()
                    .flatten()
                    .any(|s| s.contains(['\u{2068}', '\u{2069}']))
            })
            .count()
    };
    assert!(isolated(&t) > 0, "the fixture has isolated placeables");
    crate::strip_isolates(&mut t);
    assert_eq!(isolated(&t), 0);
    let (_, field) = t.nodes.iter().find(|(id, _)| *id == t.focus).unwrap();
    assert_eq!(field.value(), Some("a\u{2068}b"));
}

/// So are the marks of right-to-left languages: FSI resolved to LRI / RLI, the
/// paragraph's RLM (unbalanced once FSI / PDI alone were dropped).
#[test]
fn names_without_rtl_marks() {
    let mut n = crate::accesskit::Node::new(Role::Button);
    n.set_label("\u{200F}\u{2067}\u{05E7}\u{2069} (\u{2066}Ctrl+G\u{2069})");
    n.set_description("\u{2066}7 × \u{2066}5\u{2069}\u{2069} \u{202B}x\u{202C}\u{200E}");
    let mut t = TreeUpdate {
        nodes: vec![(crate::accesskit::NodeId(1), n)],
        tree: None,
        tree_id: crate::accesskit::TreeId::ROOT,
        focus: crate::accesskit::NodeId(1),
    };
    crate::strip_isolates(&mut t);
    let n = &t.nodes[0].1;
    assert_eq!(n.label(), Some("\u{05E7} (Ctrl+G)"));
    assert_eq!(n.description(), Some("7 × 5 x"));
}

/// AT-SPI reads a field's text and caret from its text run, and names, not values.
#[test]
fn what_atspi_can_read() {
    let t = replay(&[Step::Modal(Modal::Palette), Step::Type("zoom")]);
    let (_, field) = t.nodes.iter().find(|(id, _)| *id == t.focus).unwrap();
    assert_eq!(field.role(), Role::SearchInput);
    assert_eq!(field.value(), Some("zoom"));
    let run = field.children()[0];
    let (_, r) = t.nodes.iter().find(|(id, _)| *id == run).unwrap();
    assert_eq!((r.role(), r.value()), (Role::TextRun, Some("zoom")));
    assert_eq!(r.character_lengths(), &[1, 1, 1, 1]);
    let sel = field.text_selection().unwrap();
    assert_eq!((sel.focus.node, sel.focus.character_index), (run, 4));

    let t = replay(&[Step::Error]);
    let alert = t
        .nodes
        .iter()
        .find(|(_, n)| n.role() == Role::Alert)
        .unwrap();
    assert_eq!(alert.1.label(), Some("unsupported format"));
    assert_eq!(alert.1.live(), Some(Live::Assertive));
    let t = replay(&[]);
    assert!(
        t.nodes.iter().any(|(_, n)| n
            .label()
            .is_some_and(|l| l.contains("1") && l.contains("4"))
            && n.role() == Role::Button),
        "the position is a button's name"
    );
}

#[test]
fn the_scripted_session() {
    use Step::*;
    replay(&[Next, Info, Info, Modal(crate::Modal::Settings), Close]);
    replay(&[
        Modal(crate::Modal::Palette),
        Type("rot"),
        Close,
        Zoom,
        Error,
        Info,
    ]);
}

#[test]
fn every_pair_of_steps() {
    for a in STEPS {
        for b in STEPS {
            let r = std::panic::catch_unwind(|| replay(&[a, b, Step::Close, b, a]));
            assert!(r.is_ok(), "{a:?} then {b:?}");
        }
    }
}
