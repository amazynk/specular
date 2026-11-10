use crate::ctx::Target;
use crate::palette::{PaletteState, Run, rebuild};
use crate::{Chrome, HudStats, Overlays, Response, ViewerState};
use specular_input::{Action, ActionState, ImageState, Keymap};
use specular_settings::{Settings, SettingsStore, ToolbarItem as T};

const ITEMS: [T; 18] = [
    T::Open,
    T::Prev,
    T::Next,
    T::ZoomOut,
    T::ZoomLevel,
    T::ZoomIn,
    T::Fit,
    T::RotateLeft,
    T::RotateRight,
    T::Info,
    T::Copy,
    T::Delete,
    T::Palette,
    T::Settings,
    T::Fullscreen,
    T::Slideshow,
    T::ExportSlideshow,
    T::Mpr,
];

pub(super) const STATES: [ActionState; 6] = [
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::None,
        count: 0,
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
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Opening,
        count: 4,
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
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Decoding,
        count: 4,
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
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Ready,
        count: 4,
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
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Ready,
        count: 1,
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
    ActionState {
        comparing: false,
        compare_images: 0,
        volume: false,
        mpr: false,
        image: ImageState::Failed,
        count: 4,
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
];

pub(super) fn with_state<R>(actions: ActionState, f: impl FnOnce(&ViewerState) -> R) -> R {
    with_metadata(actions, None, f)
}

pub(super) fn with_metadata<R>(
    actions: ActionState,
    metadata: Option<&specular_metadata::MetadataReport>,
    f: impl FnOnce(&ViewerState) -> R,
) -> R {
    let hud = HudStats::default();
    let keymap = Keymap::default();
    let store = SettingsStore::in_memory();
    let settings = Settings::default();
    let st = ViewerState {
        actions,
        has_image: actions.image != ImageState::None,
        index: 0,
        count: actions.count,
        zoom: 1.0,
        at_fit: true,
        fullscreen: false,
        file_name: None,
        dims: None,
        loading: false,
        error: None,
        truncated: false,
        metadata,
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
    f(&st)
}

/// The toolbar, the edge arrows and the palette all give the same answer as
/// `Action::disabled`, in every state.
#[test]
fn toolbar_palette_and_edges_agree_with_the_rule() {
    for actions in STATES {
        with_state(actions, |st| {
            for item in ITEMS {
                let t = Target::Toolbar(item);
                let want = crate::toolbar::item_action(item, st).and_then(|a| a.disabled(&actions));
                assert_eq!(t.disabled(st), want, "{item:?} in {actions:?}");
                // The tooltip names the reason exactly when the control is disabled.
                let tip = t.tooltip(st).unwrap_or_default();
                assert_eq!(
                    tip.contains("unavailable"),
                    want.is_some(),
                    "{item:?} in {actions:?}: {tip}"
                );
            }
            assert_eq!(
                Target::EdgeNext.disabled(st),
                Action::NextImage.disabled(&actions)
            );
            assert_eq!(
                Target::EdgePrev.disabled(st),
                Action::PrevImage.disabled(&actions)
            );
            let mut p = PaletteState {
                dirty: true,
                ..Default::default()
            };
            rebuild(&mut p, st);
            assert!(!p.items.is_empty());
            for e in &p.items {
                if let Run::Action(a) = e.run {
                    assert_eq!(
                        e.disabled,
                        a.disabled(&actions),
                        "palette {a:?} in {actions:?}"
                    );
                }
            }
        });
    }
}

/// Clicking a disabled toolbar button or choosing a disabled palette entry does nothing.
#[test]
fn disabled_controls_do_nothing() {
    let empty = STATES[0];
    with_state(empty, |st| {
        let mut c = Chrome::new();
        let r: Response = c.activate(Target::Toolbar(T::Info), st);
        assert!(r.consumed && r.commands.is_empty());
        assert!(!c.info_open());
        let r = c.activate(Target::Toolbar(T::ZoomIn), st);
        assert!(r.commands.is_empty());
        let r = c.activate(Target::Toolbar(T::ZoomLevel), st);
        assert!(r.commands.is_empty() && !c.zoom_menu);
        let r = c.activate(Target::Toolbar(T::ExportSlideshow), st);
        assert!(r.commands.is_empty());
        // Open still works.
        let r = c.activate(Target::Toolbar(T::Open), st);
        assert_eq!(r.commands.len(), 1);
        // Palette: a disabled entry runs nothing; it opens the manual where it says why,
        // over the palette (Esc goes back to it).
        assert!(c.handle_action(Action::Palette));
        // Searched for: browsing lists at most 80 entries, greyed ones last.
        c.palette.query = "zoom in".into();
        c.palette.dirty = true;
        rebuild(&mut c.palette, st);
        let i = c
            .palette
            .items
            .iter()
            .position(|e| matches!(e.run, Run::Action(Action::ZoomIn)))
            .unwrap();
        let r = c.run_palette(i, st);
        assert!(r.commands.is_empty());
        assert_eq!(c.modal(), Some(crate::Modal::Help));
        let why = Action::ZoomIn.disabled(&st.actions).unwrap();
        assert_eq!(
            c.help_page().as_deref(),
            Some(specular_help::disabled_topic(why).page)
        );
        c.close_modal();
        assert_eq!(c.modal(), Some(crate::Modal::Palette));
    });
}

/// The bar keeps every item, zoom included, with an image open; the start screen (none open)
/// has no bar. `without_view_controls` drops the zoom controls and Fit without leaving
/// separators at an end or doubled.
#[test]
fn bar_keeps_its_view_controls_and_the_start_screen_has_none() {
    use crate::toolbar::without_view_controls;
    assert_eq!(
        without_view_controls(&[
            T::Separator,
            T::Open,
            T::Separator,
            T::ZoomOut,
            T::ZoomLevel,
            T::ZoomIn,
            T::Fit,
            T::Separator,
            T::Info,
            T::Separator,
            T::Fit,
        ]),
        [T::Open, T::Separator, T::Info]
    );
    assert_eq!(without_view_controls(&[T::Fit, T::ZoomIn]), []);
    let zoom = [T::ZoomOut, T::ZoomLevel, T::ZoomIn, T::Fit];
    let mut text = specular_gfx::Text::new();
    for (actions, open) in [(STATES[0], false), (STATES[3], true)] {
        with_state(actions, |st| {
            let mut c = Chrome::new();
            c.set_size(1000.0, 700.0);
            c.poke(1.0);
            let mut dl = specular_gfx::DrawList::new(1.0);
            c.draw(
                &mut dl,
                &mut text,
                st,
                &specular_theme::Palette::dark(),
                1.0,
            );
            let shown = |it: T| c.hits.iter().any(|(_, h)| *h == Target::Toolbar(it));
            if !open {
                assert!(c.welcome_drawn);
                assert!(!c.hits.iter().any(|(_, h)| matches!(h, Target::Toolbar(_))));
                return;
            }
            assert!(shown(T::Prev) && shown(T::Settings));
            for it in zoom {
                assert!(shown(it), "{it:?}");
            }
            assert!(c.bar_geom.is_some());
        });
    }
}
