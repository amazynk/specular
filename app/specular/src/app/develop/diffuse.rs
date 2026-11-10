//! The Soften and sharpen (diffuse) and Surface blur cards: their controls, the preset
//! choice (it writes all the module's fields; "Custom" when they match no preset) and the
//! "only at 100%" note for detail finer than a reduced view can show.

use specular_develop::model::{Fields, ModuleKind, Value};
use specular_develop::ops::detail::diffuse::{
    ANISOTROPY_FIELDS, Diffuse, FULL_VIEW_PX, MAX_ITERATIONS, MAX_RADIUS, Preset, SPEED_FIELDS,
};
use specular_develop::ops::detail::surface;
use specular_i18n::t;
use specular_widgets::choice::Choice;
use specular_widgets::develop_panel::{Body, Card, Control};
use specular_widgets::slider::{Slider, SliderSpec, Unit};

/// Control ids (260–279).
pub(super) mod id {
    pub const PRESET: u32 = 260;
    pub const ITERATIONS: u32 = 261;
    pub const RADIUS: u32 = 262;
    pub const SPAN: u32 = 263;
    /// Speeds 1–4 at `+ k`.
    pub const SPEED: u32 = 264;
    /// Anisotropies 1–4 at `+ k`.
    pub const ANISOTROPY: u32 = 268;
    pub const EDGES: u32 = 272;
    pub const THRESHOLD: u32 = 273;
    pub const HIGHLIGHTS: u32 = 274;
    pub const NOTE: u32 = 275;
    pub const SURFACE_AMOUNT: u32 = 276;
    pub const SURFACE_RADIUS: u32 = 277;
    pub const SURFACE_EDGES: u32 = 278;
}

/// Whether a control belongs to these cards.
pub(super) fn is_diffuse(control: u32) -> bool {
    (id::PRESET..=id::SURFACE_EDGES).contains(&control)
}

/// The module of a control of these cards.
pub(super) fn module_of(control: u32) -> Option<ModuleKind> {
    match control {
        id::SURFACE_AMOUNT..=id::SURFACE_EDGES => Some(ModuleKind::SURFACE_BLUR),
        c if is_diffuse(c) => Some(ModuleKind::DIFFUSE),
        _ => None,
    }
}

/// The float field a slider of these cards sets.
pub(super) fn float_field(control: u32) -> Option<(ModuleKind, &'static str)> {
    Some(match control {
        id::ITERATIONS => (ModuleKind::DIFFUSE, "iterations"),
        id::RADIUS => (ModuleKind::DIFFUSE, "radius"),
        id::SPAN => (ModuleKind::DIFFUSE, "span"),
        c @ id::SPEED..=267 => (ModuleKind::DIFFUSE, SPEED_FIELDS[(c - id::SPEED) as usize]),
        c @ id::ANISOTROPY..=271 => (
            ModuleKind::DIFFUSE,
            ANISOTROPY_FIELDS[(c - id::ANISOTROPY) as usize],
        ),
        id::EDGES => (ModuleKind::DIFFUSE, "edges"),
        id::THRESHOLD => (ModuleKind::DIFFUSE, "threshold"),
        id::HIGHLIGHTS => (ModuleKind::DIFFUSE, "highlights"),
        id::SURFACE_AMOUNT => (ModuleKind::SURFACE_BLUR, "amount"),
        id::SURFACE_RADIUS => (ModuleKind::SURFACE_BLUR, "radius"),
        id::SURFACE_EDGES => (ModuleKind::SURFACE_BLUR, "edges"),
        _ => return None,
    })
}

/// Iterations are stored as an integer.
pub(super) fn fix_written(control: u32, f: &mut Fields) {
    if control == id::ITERATIONS {
        let v = f
            .f32_or("iterations", 4.0)
            .round()
            .clamp(1.0, MAX_ITERATIONS as f32);
        f.set("iterations", Value::Int(v as i64));
    }
}

/// A field's name in a history row.
pub(super) fn field_label(kind: ModuleKind, field: &str) -> Option<String> {
    let k = |names: [&str; 4]| names.iter().position(|n| *n == field);
    Some(match kind {
        ModuleKind::DIFFUSE => match field {
            "iterations" => t!("develop-diffuse-iterations"),
            "radius" => t!("develop-diffuse-radius"),
            "span" => t!("develop-diffuse-span"),
            "edges" => t!("develop-diffuse-edges"),
            "threshold" => t!("develop-diffuse-threshold"),
            "highlights" => t!("develop-diffuse-highlights"),
            _ => match (k(SPEED_FIELDS), k(ANISOTROPY_FIELDS)) {
                (Some(0), _) => t!("develop-diffuse-speed-1"),
                (Some(1), _) => t!("develop-diffuse-speed-2"),
                (Some(2), _) => t!("develop-diffuse-speed-3"),
                (Some(_), _) => t!("develop-diffuse-speed-4"),
                (_, Some(0)) => t!("develop-diffuse-anisotropy-1"),
                (_, Some(1)) => t!("develop-diffuse-anisotropy-2"),
                (_, Some(2)) => t!("develop-diffuse-anisotropy-3"),
                (_, Some(_)) => t!("develop-diffuse-anisotropy-4"),
                _ => return None,
            },
        },
        ModuleKind::SURFACE_BLUR => match field {
            "amount" => t!("develop-surface-amount"),
            "radius" => t!("develop-surface-radius"),
            "edges" => t!("develop-surface-edges"),
            _ => return None,
        },
        _ => return None,
    })
}

fn preset_name(p: Preset) -> String {
    match p {
        Preset::Sharpen => t!("develop-diffuse-preset-sharpen"),
        Preset::Deblur => t!("develop-diffuse-preset-deblur"),
        Preset::LocalContrast => t!("develop-diffuse-preset-local-contrast"),
        Preset::Bloom => t!("develop-diffuse-preset-bloom"),
        Preset::InpaintHighlights => t!("develop-diffuse-preset-inpaint"),
        Preset::Smooth => t!("develop-diffuse-preset-smooth"),
    }
}

/// The preset choice: the matching preset, else "Custom" (first).
pub(super) fn read_choice(c: &mut Choice, f: &Fields) {
    let i = Preset::matching(f)
        .and_then(|p| Preset::ALL.iter().position(|q| *q == p))
        .map_or(0, |i| i + 1);
    c.set(i);
}

/// Writes the chosen preset's fields; "Custom" changes nothing.
pub(super) fn write_choice(c: &Choice, f: &mut Fields) -> Option<()> {
    let p = Preset::ALL.get(c.selected.checked_sub(1)?)?;
    f.overlay(&p.params().to_fields());
    Some(())
}

/// The note under the controls: empty unless the detail is too fine for a reduced view.
pub(super) fn note(f: &Fields, frame: Option<(f64, f64)>) -> String {
    let long = frame.map_or(6000.0, |(w, h)| w.max(h) as f32);
    let d = Diffuse::from_fields(f);
    if specular_develop::ops::detail::diffuse::needs_full_view(&d, long, FULL_VIEW_PX) {
        t!("develop-diffuse-check-full")
    } else {
        String::new()
    }
}

fn slider(id: u32, spec: SliderSpec) -> Control {
    Control {
        id,
        body: Body::Slider(Slider::new(spec)),
    }
}

fn signed(id: u32, label: String) -> Control {
    slider(
        id,
        SliderSpec::new(label, -1.0, 1.0, 0.0)
            .steps(0.01, 0.1)
            .unit(Unit::Percent, 0)
            .signed(),
    )
}

/// The two cards, closed and off-until-used (each is the identity at its defaults).
pub(super) fn cards() -> Vec<Card> {
    let mut presets = vec![t!("develop-diffuse-preset-custom")];
    presets.extend(Preset::ALL.iter().map(|p| preset_name(*p)));
    let pct = |id: u32, label: String, max: f32, default: f32| {
        slider(
            id,
            SliderSpec::new(label, 0.0, max, default)
                .steps(0.0001, 0.001)
                .unit(Unit::Percent, 2),
        )
    };
    let speeds = [
        t!("develop-diffuse-speed-1"),
        t!("develop-diffuse-speed-2"),
        t!("develop-diffuse-speed-3"),
        t!("develop-diffuse-speed-4"),
    ];
    let dirs = [
        t!("develop-diffuse-anisotropy-1"),
        t!("develop-diffuse-anisotropy-2"),
        t!("develop-diffuse-anisotropy-3"),
        t!("develop-diffuse-anisotropy-4"),
    ];
    let d = Diffuse::default();
    let mut controls = vec![
        Control {
            id: id::PRESET,
            body: Body::Choice(Choice::new(t!("develop-diffuse-preset"), presets)),
        },
        slider(
            id::ITERATIONS,
            SliderSpec::new(
                t!("develop-diffuse-iterations"),
                1.0,
                MAX_ITERATIONS as f32,
                d.iterations as f32,
            )
            .steps(1.0, 4.0)
            .unit(Unit::Plain, 0),
        ),
        pct(
            id::RADIUS,
            t!("develop-diffuse-radius"),
            MAX_RADIUS,
            d.radius,
        ),
        pct(id::SPAN, t!("develop-diffuse-span"), MAX_RADIUS, d.span),
    ];
    for (k, (s, a)) in speeds.into_iter().zip(dirs).enumerate() {
        controls.push(signed(id::SPEED + k as u32, s));
        controls.push(signed(id::ANISOTROPY + k as u32, a));
    }
    controls.extend([
        slider(
            id::EDGES,
            SliderSpec::new(t!("develop-diffuse-edges"), 0.0, 1.0, d.edges)
                .steps(0.01, 0.1)
                .unit(Unit::Percent, 0),
        ),
        slider(
            id::THRESHOLD,
            SliderSpec::new(t!("develop-diffuse-threshold"), 0.0, 1.0, d.threshold)
                .steps(0.01, 0.1)
                .unit(Unit::Percent, 0),
        ),
        slider(
            id::HIGHLIGHTS,
            SliderSpec::new(t!("develop-diffuse-highlights"), 0.0, 4.0, d.highlights)
                .steps(0.01, 0.1)
                .unit(Unit::Plain, 2),
        ),
        Control {
            id: id::NOTE,
            body: Body::Note(String::new()),
        },
    ]);
    let s = surface::SurfaceBlur::default();
    let card = |kind: ModuleKind, title: String, controls: Vec<Control>| Card {
        id: u32::from(kind.0),
        title,
        enabled: Some(true),
        expanded: false,
        controls,
    };
    vec![
        card(
            ModuleKind::SURFACE_BLUR,
            t!("develop-module-surface-blur"),
            vec![
                slider(
                    id::SURFACE_AMOUNT,
                    SliderSpec::new(t!("develop-surface-amount"), 0.0, 1.0, s.amount)
                        .steps(0.01, 0.1)
                        .unit(Unit::Percent, 0),
                ),
                pct(
                    id::SURFACE_RADIUS,
                    t!("develop-surface-radius"),
                    surface::MAX_RADIUS,
                    s.radius,
                ),
                slider(
                    id::SURFACE_EDGES,
                    SliderSpec::new(
                        t!("develop-surface-edges"),
                        surface::MIN_EDGES,
                        surface::MAX_EDGES,
                        s.edges,
                    )
                    .steps(0.01, 0.05)
                    .unit(Unit::Plain, 2),
                ),
            ],
        ),
        card(ModuleKind::DIFFUSE, t!("develop-module-diffuse"), controls),
    ]
}
