//! Which size of a feature's models suits this machine (DESIGN.md §10 "Local AI pack":
//! several models per feature, Specular picks one by the GPU; the user only turns a feature
//! on, and may name a size of their own).
//!
//! [`recommend`] is a pure function of the offer and a few [`MachineFacts`], so every rule
//! is a table row in the tests. How the facts map:
//!
//! 1. **Engine.** The GPU runs the models when there is a real one (integrated, discrete,
//!    or a class not known yet) and the inference self-test has not blocked it; otherwise
//!    the CPU does (no adapter, a software rasteriser, a blocked GPU path).
//! 2. **Memory budget** for the loaded models of one size ([`TierOffer::memory_mb`]):
//!    - discrete GPU: half its memory (read from the driver where it says, [`vram`]);
//!      [`ASSUMED_VRAM`] when unknown;
//!    - integrated GPU, or a class not known yet: the larger of half its carve-out and a
//!      quarter of RAM (it shares system memory);
//!    - CPU: a quarter of RAM.
//!
//!    Unknown RAM counts as [`ASSUMED_RAM`]. A size over the budget, or needing a GPU the
//!    engine is not, never **fits** ([`Unfit`] says why): the recommendation never picks it.
//! 3. **Size.**
//!    - A feature whose manifest names a default model ([`Offer::default`]): that model
//!      wherever it fits, else the largest fitting size below it reading the same languages
//!      (Search: SigLIP 2 B/32 before the English-only PE Core), else the largest fitting size
//!      below it, else the smallest that fits; the speed and power rules below do not apply
//!      (the user picks a faster model in the model manager).
//!    - On the CPU: the smallest size that fits (a large image encoder takes seconds per
//!      photo there, and indexing a library would take days).
//!    - On the GPU with the calibration's measured time for its ViT-Ti/16 (`ms`, idle,
//!      [`VIT_TI_MMACS`]): a size's time per photo is predicted as
//!      `ms × size.mmacs / VIT_TI_MMACS`; the largest fitting size within
//!      [`GPU_MS_PER_PHOTO`] is recommended, else the smallest that fits.
//!    - On the GPU without a measurement (calibration not run yet): large on a discrete GPU,
//!      standard otherwise.
//! 4. **Power.** In a low-power mode or on battery, one size smaller (when one fits). The
//!    recommendation is computed when the feature is turned on; it does not switch sizes
//!    later when the power state changes.
//! 5. **The user's choice** ([`SizeChoice`]) is absolute: Recommended follows the machine
//!    (the rules above); a named size is that size
//!    whatever the machine, even one that does not fit (the page warns and asks first). A
//!    named size the feature does not come in falls back to the recommendation.

use specular_core::paths::{Engine, GpuClass, Task, Trust, selector};

use crate::manifest::{Offer, Tier, TierOffer, VIT_TI_MMACS};

/// Time per photo the recommendation stays within on the GPU, in milliseconds: ~10 photos
/// a second, a 100,000-photo library in about three hours of idle time.
pub const GPU_MS_PER_PHOTO: f32 = 100.0;

/// Memory of a discrete GPU whose driver does not say (most current ones have at least this).
pub const ASSUMED_VRAM: u64 = 4 << 30;

/// RAM when the system does not say.
pub const ASSUMED_RAM: u64 = 8 << 30;

/// The user's size setting per feature: the machine's recommendation, or a size by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum SizeChoice {
    /// What [`recommend`] picks (and picks again when the machine changes, [`reapply`]).
    #[default]
    Recommended,
    /// This size, whatever the machine.
    Tier(Tier),
}

impl SizeChoice {
    pub const ALL: [SizeChoice; 4] = [
        SizeChoice::Recommended,
        SizeChoice::Tier(Tier::Small),
        SizeChoice::Tier(Tier::Standard),
        SizeChoice::Tier(Tier::Large),
    ];

    /// The settings value: `"recommended"` or the size's id.
    pub const fn id(self) -> &'static str {
        match self {
            SizeChoice::Recommended => "recommended",
            SizeChoice::Tier(t) => t.id(),
        }
    }

    pub fn from_id(s: &str) -> Option<SizeChoice> {
        SizeChoice::ALL.into_iter().find(|c| c.id() == s)
    }
}

/// What the choice looks at.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct MachineFacts {
    /// The kind of GPU the app renders with.
    pub gpu: GpuClass,
    /// The inference GPU path is not blocked (the self-test passed or has not run).
    pub gpu_usable: bool,
    /// Bytes of the GPU's own memory (a discrete card's VRAM, an integrated GPU's
    /// carve-out), when the driver says.
    pub gpu_memory: Option<u64>,
    /// Bytes of system RAM.
    pub ram: Option<u64>,
    /// The inference calibration's ViT-Ti/16 at 224², milliseconds per run, on the GPU and
    /// on the CPU (None: not measured yet).
    pub gpu_ms: Option<f32>,
    pub cpu_ms: Option<f32>,
    /// Low-power mode or on battery.
    pub low_power: bool,
}

impl MachineFacts {
    /// The facts now: the path selector's GPU class, trust and calibration timings, the
    /// system's RAM, the GPU's memory from the driver (`adapter`: the app's adapter, to find
    /// it), the power state.
    pub fn gather(adapter: Option<&wgpu::AdapterInfo>) -> MachineFacts {
        let sel = selector();
        let mut gpu = sel.gpu_class();
        if gpu == GpuClass::Unknown
            && let Some(a) = adapter
        {
            gpu = match a.device_type {
                wgpu::DeviceType::DiscreteGpu => GpuClass::Discrete,
                wgpu::DeviceType::IntegratedGpu => GpuClass::Integrated,
                wgpu::DeviceType::Cpu => GpuClass::Software,
                _ => GpuClass::Unknown,
            };
        }
        let gpu_path = Task::Inference.path_of(Engine::Gpu);
        let gpu_usable = gpu_path.is_some_and(|p| sel.trust(Task::Inference, p) != Trust::Blocked);
        let hw = specular_core::HardwareProfile::detect();
        let power = specular_platform::power_state();
        MachineFacts {
            gpu,
            gpu_usable,
            gpu_memory: adapter.and_then(|a| vram(a.vendor, a.device)),
            ram: (hw.ram_total > 0).then_some(hw.ram_total),
            gpu_ms: gpu_path.and_then(calibrated_ms),
            cpu_ms: calibrated_ms(0),
            low_power: power.saving(),
        }
    }

    /// Whether the GPU runs the models.
    pub fn on_gpu(&self) -> bool {
        self.gpu_usable
            && matches!(
                self.gpu,
                GpuClass::Integrated | GpuClass::Discrete | GpuClass::Unknown
            )
    }

    /// Memory the models of one size may take, in MB (rule 2).
    pub fn budget_mb(&self) -> u64 {
        let ram = self.ram.unwrap_or(ASSUMED_RAM);
        let bytes = if !self.on_gpu() {
            ram / 4
        } else if self.gpu == GpuClass::Discrete {
            self.gpu_memory.unwrap_or(ASSUMED_VRAM) / 2
        } else {
            (self.gpu_memory.unwrap_or(0) / 2).max(ram / 4)
        };
        bytes >> 20
    }

    /// The engine's calibration time, ms per ViT-Ti/16.
    fn ms(&self) -> Option<f32> {
        if self.on_gpu() {
            self.gpu_ms
        } else {
            self.cpu_ms
        }
        .filter(|m| m.is_finite() && *m > 0.0)
    }
}

/// Measured wall time of the calibration's ViT-Ti/16 on inference path `path`, in ms.
fn calibrated_ms(path: usize) -> Option<f32> {
    let sel = selector();
    let c = sel.calibration(Task::Inference, path, specular_infer::calibrate::PIXELS, 0);
    if sel.samples(Task::Inference, path, c.key) == 0 {
        return None;
    }
    let e = sel.estimate(Task::Inference, path, c.key, c.mpx);
    Some(e.wall_us / 1e3)
}

/// Bytes of the GPU's own memory, from the driver: Linux amdgpu's `mem_info_vram_total`
/// for the DRM card with the adapter's PCI vendor and device ids. None elsewhere (wgpu
/// does not report memory sizes).
pub fn vram(vendor: u32, device: u32) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let read = |p: &std::path::Path| std::fs::read_to_string(p).ok();
        let hex = |s: String| u32::from_str_radix(s.trim().trim_start_matches("0x"), 16).ok();
        for e in std::fs::read_dir("/sys/class/drm").ok()?.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            // card0, card1… (not the connectors, card1-DP-1).
            if !name
                .strip_prefix("card")
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            {
                continue;
            }
            let dev = e.path().join("device");
            if read(&dev.join("vendor")).and_then(hex) != Some(vendor)
                || read(&dev.join("device")).and_then(hex) != Some(device)
            {
                continue;
            }
            if let Some(n) = read(&dev.join("mem_info_vram_total"))
                .and_then(|s| s.trim().parse::<u64>().ok())
                .filter(|n| *n > 0)
            {
                return Some(n);
            }
        }
        None
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (vendor, device);
        None
    }
}

/// Why a size does not fit this machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unfit {
    /// It needs a GPU and the models would run on the CPU.
    Gpu,
    /// It needs more memory than the budget (rule 2), both in MB.
    Memory { need_mb: u64, budget_mb: u64 },
}

/// How one size would do on this machine.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Fit {
    pub tier: Tier,
    /// Within the memory budget, and on an engine the size can run on.
    pub fits: bool,
    /// Why not, when it does not fit (the GPU first: no memory makes up for it).
    pub unfit: Option<Unfit>,
    /// Predicted time per photo, ms (None: no calibration, or the manifest gives no work).
    pub ms_per_photo: Option<f32>,
}

/// Every size of the offer, smallest first, as it would do here (what Settings shows next
/// to the size choice).
pub fn assess(offer: &Offer, facts: &MachineFacts) -> Vec<Fit> {
    offer.tiers.iter().map(|t| fit(t, facts)).collect()
}

fn fit(t: &TierOffer, facts: &MachineFacts) -> Fit {
    let (need_mb, budget_mb) = (u64::from(t.memory_mb()), facts.budget_mb());
    let unfit = if t.needs_gpu() && !facts.on_gpu() {
        Some(Unfit::Gpu)
    } else if need_mb > budget_mb {
        Some(Unfit::Memory { need_mb, budget_mb })
    } else {
        None
    };
    let ms_per_photo = match (facts.ms(), t.mmacs()) {
        (Some(ms), Some(work)) => Some(ms * work as f32 / VIT_TI_MMACS as f32),
        _ => None,
    };
    Fit {
        tier: t.tier,
        fits: unfit.is_none(),
        unfit,
        ms_per_photo,
    }
}

/// The size to install by default (rules 1 to 4); None when no size fits.
pub fn recommend(offer: &Offer, facts: &MachineFacts) -> Option<Tier> {
    let fits: Vec<Fit> = assess(offer, facts)
        .into_iter()
        .filter(|f| f.fits)
        .collect();
    if fits.is_empty() {
        return None;
    }
    // The manifest's default (defaults are fixed; the user picks another) wherever it can
    // run, whatever the speed or the power state; when it cannot, the largest fitting size
    // below it that reads the same languages (an English-only model is not chosen for a
    // multilingual default while another fits), else the largest fitting size below it,
    // else the smallest that fits.
    if let Some(d) = offer.default {
        let langs = |t: Tier| offer.choice(t).and_then(|c| c.languages.as_deref());
        let below = || fits.iter().rev().filter(|f| f.tier <= d);
        return Some(
            below()
                .find(|f| langs(f.tier) == langs(d))
                .or_else(|| below().next())
                .unwrap_or(&fits[0])
                .tier,
        );
    }
    let mut i = if !facts.on_gpu() {
        0
    } else if facts.ms().is_some() {
        fits.iter()
            .rposition(|f| f.ms_per_photo.is_some_and(|ms| ms <= GPU_MS_PER_PHOTO))
            .unwrap_or(0)
    } else {
        let want = if facts.gpu == GpuClass::Discrete {
            Tier::Large
        } else {
            Tier::Standard
        };
        // The wanted size, else the largest fitting one below it, else the smallest.
        fits.iter().rposition(|f| f.tier <= want).unwrap_or(0)
    };
    if facts.low_power {
        i = i.saturating_sub(1);
    }
    Some(fits[i].tier)
}

/// The size to install with the user's setting (rule 5): the recommendation, or the named
/// size when the feature comes in it, fitting or not. None: no named size of the feature,
/// and no size fits.
pub fn choose(offer: &Offer, facts: &MachineFacts, choice: SizeChoice) -> Option<Tier> {
    match choice {
        SizeChoice::Tier(t) if offer.tier_offer(t).is_some() => Some(t),
        _ => recommend(offer, facts),
    }
}

/// What the old relative settings ("smaller", "larger": `by` −1 or +1) mean on this machine:
/// `by` steps from the recommendation among the sizes that fit, clamped at the ends. For
/// moving such a setting to the size it names; None when no size fits.
pub fn step(offer: &Offer, facts: &MachineFacts, by: i8) -> Option<Tier> {
    let rec = recommend(offer, facts)?;
    let fits: Vec<Tier> = assess(offer, facts)
        .into_iter()
        .filter(|f| f.fits)
        .map(|f| f.tier)
        .collect();
    let i = fits.iter().position(|t| *t == rec)?;
    let i = i.saturating_add_signed(isize::from(by)).min(fits.len() - 1);
    Some(fits[i])
}

/// What a size was chosen on ([`crate::Pack::install_for`]'s `basis`): the GPU's name and
/// whether the inference calibration had run. A later difference is a reason to look at the
/// recommendation again ([`reapply`]); battery and power state are not part of it (they
/// come and go).
pub fn basis(gpu_name: &str, facts: &MachineFacts) -> String {
    let measured = if facts.ms().is_some() {
        "calibrated"
    } else {
        "uncalibrated"
    };
    format!("{gpu_name}|{measured}")
}

/// What to do about an installed size once the machine may have changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reapply {
    /// Nothing changed.
    Nothing,
    /// The machine changed but the size stays: remember the new basis (no question).
    Record,
    /// The recommendation moved to this size: ask the user (never switch silently, as it
    /// downloads and re-indexes).
    Suggest(Tier),
}

/// The rule for re-applying the recommendation: after the first calibration and when the
/// GPU changed (`basis` against `now`), and only while the size is left on Recommended.
/// `chosen` is [`choose`]'s pick now; `installed` the size in use.
pub fn reapply(
    installed: Tier,
    basis: Option<&str>,
    now: &str,
    choice: SizeChoice,
    chosen: Option<Tier>,
) -> Reapply {
    if basis == Some(now) {
        return Reapply::Nothing;
    }
    match (basis, choice, chosen) {
        // Installed before sizes, or a size the user named: only the basis is new.
        (None, ..) | (_, SizeChoice::Tier(_), _) => Reapply::Record,
        (Some(_), SizeChoice::Recommended, Some(t)) if t != installed => Reapply::Suggest(t),
        _ => Reapply::Record,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Manifest;

    const GB: u64 = 1 << 30;

    /// Three sizes like the built-in search models: memory (MB), work (MMACs).
    fn offer(sizes: &[(Tier, u32, u32)]) -> Offer {
        let mut t = String::from("schema = 2\n");
        for (tier, mb, mmacs) in sizes {
            t += &format!(
                r#"
[[model]]
feature = "search"
tier = "{tier}"
id = "m-{tier}"
version = "1"
role = "image-text-encoder"
urls = ["https://models.invalid/{tier}"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 1000
licence = "MIT"
licence_url = "https://models.invalid/LICENSE"
placeholder = true

[model.runtime]
format = "safetensors"
min_memory_mb = {mb}
input = 224
mmacs = {mmacs}
"#
            );
        }
        Manifest::parse(&t)
            .unwrap()
            .offer(crate::Feature::Search, true)
            .unwrap()
    }

    fn search() -> Offer {
        offer(&[
            (Tier::Small, 400, 2_000),
            (Tier::Standard, 700, 5_800),
            (Tier::Large, 1_400, 81_000),
        ])
    }

    fn facts(gpu: GpuClass) -> MachineFacts {
        MachineFacts {
            gpu,
            gpu_usable: true,
            ram: Some(16 * GB),
            ..MachineFacts::default()
        }
    }

    #[test]
    fn table() {
        use GpuClass::*;
        use Tier::*;
        let o = search();
        let d = facts;
        #[rustfmt::skip]
        let rows: [(&str, MachineFacts, Option<Tier>); 18] = [
            // CPU only: the smallest size.
            ("no gpu", d(None), Some(Small)),
            ("software", d(Software), Some(Small)),
            ("blocked gpu", MachineFacts { gpu_usable: false, ..d(Discrete) }, Some(Small)),
            ("fast cpu still small", MachineFacts { cpu_ms: Some(5.0), ..d(None) }, Some(Small)),
            // No calibration yet: by class.
            ("unknown", d(Unknown), Some(Standard)),
            ("integrated", d(Integrated), Some(Standard)),
            ("discrete, vram unknown", d(Discrete), Some(Large)),
            ("big discrete", MachineFacts { gpu_memory: Some(16 * GB), ..d(Discrete) }, Some(Large)),
            // Measured: the largest within the time budget.
            ("fast gpu", MachineFacts { gpu_ms: Some(1.0), ..d(Discrete) }, Some(Large)),
            ("medium gpu", MachineFacts { gpu_ms: Some(4.0), ..d(Integrated) }, Some(Standard)),
            ("slow gpu", MachineFacts { gpu_ms: Some(30.0), ..d(Discrete) }, Some(Small)),
            ("very slow gpu", MachineFacts { gpu_ms: Some(500.0), ..d(Discrete) }, Some(Small)),
            // Memory: never a size that does not fit.
            ("small vram", MachineFacts { gpu_memory: Some(2 * GB), ..d(Discrete) }, Some(Standard)),
            ("small vram, fast", MachineFacts { gpu_memory: Some(2 * GB), gpu_ms: Some(0.5), ..d(Discrete) }, Some(Standard)),
            ("low ram, integrated", MachineFacts { ram: Some(2 * GB), ..d(Integrated) }, Some(Small)),
            ("tiny ram, cpu", MachineFacts { ram: Some(GB), ..d(None) }, Option::None),
            // Power: one size down.
            ("discrete on battery", MachineFacts { low_power: true, ..d(Discrete) }, Some(Standard)),
            ("cpu on battery", MachineFacts { low_power: true, ..d(None) }, Some(Small)),
        ];
        for (what, f, want) in rows {
            assert_eq!(recommend(&o, &f), want, "{what}: {f:?}");
            if let Some(t) = recommend(&o, &f) {
                let fit = assess(&o, &f).into_iter().find(|x| x.tier == t).unwrap();
                assert!(fit.fits, "{what}");
            }
        }
    }

    /// The old relative settings, for migrating them: one step among the sizes that fit.
    #[test]
    fn legacy_steps_and_clamp() {
        use Tier::*;
        let o = search();
        let cases = [
            // (facts, recommended, smaller, larger)
            (facts(GpuClass::Integrated), Standard, Small, Large),
            (facts(GpuClass::Discrete), Large, Standard, Large),
            (facts(GpuClass::None), Small, Small, Standard),
            // 2 GB of VRAM: large does not fit, so Larger stays at standard.
            (
                MachineFacts {
                    gpu_memory: Some(2 * GB),
                    ..facts(GpuClass::Discrete)
                },
                Standard,
                Small,
                Standard,
            ),
        ];
        for (f, rec, smaller, larger) in cases {
            assert_eq!(step(&o, &f, 0), Some(rec), "{f:?}");
            assert_eq!(step(&o, &f, -1), Some(smaller), "{f:?}");
            assert_eq!(step(&o, &f, 1), Some(larger), "{f:?}");
        }
        let starved = MachineFacts {
            ram: Some(GB),
            ..facts(GpuClass::None)
        };
        assert_eq!(step(&o, &starved, 1), None);
    }

    /// A named size is that size on any machine, every size of the offer is reachable, and
    /// only Recommended follows the machine.
    #[test]
    fn named_sizes_are_absolute() {
        use SizeChoice::Recommended;
        let o = search();
        let machines = [
            facts(GpuClass::None),
            facts(GpuClass::Integrated),
            facts(GpuClass::Discrete),
            MachineFacts {
                gpu_memory: Some(2 * GB),
                ..facts(GpuClass::Discrete)
            },
            // Nothing fits at all: a named size is still that size.
            MachineFacts {
                ram: Some(GB),
                ..facts(GpuClass::None)
            },
        ];
        for f in machines {
            for t in Tier::ALL {
                assert_eq!(choose(&o, &f, SizeChoice::Tier(t)), Some(t), "{f:?}");
            }
            assert_eq!(choose(&o, &f, Recommended), recommend(&o, &f), "{f:?}");
        }
        // Small recommended (no GPU), Large still reachable: the old relative choice could
        // only get to Standard.
        let cpu = facts(GpuClass::None);
        assert_eq!(recommend(&o, &cpu), Some(Tier::Small));
        assert_eq!(step(&o, &cpu, 1), Some(Tier::Standard));
        assert_eq!(
            choose(&o, &cpu, SizeChoice::Tier(Tier::Large)),
            Some(Tier::Large)
        );
    }

    #[test]
    fn only_the_sizes_that_exist() {
        let o = offer(&[(Tier::Small, 400, 2_000), (Tier::Large, 1_400, 81_000)]);
        // Integrated without a measurement wants standard: the largest below is small.
        assert_eq!(
            recommend(&o, &facts(GpuClass::Integrated)),
            Some(Tier::Small)
        );
        assert_eq!(step(&o, &facts(GpuClass::Integrated), 1), Some(Tier::Large));
        // A size the feature does not come in: the recommendation.
        assert_eq!(
            choose(
                &o,
                &facts(GpuClass::Integrated),
                SizeChoice::Tier(Tier::Standard)
            ),
            Some(Tier::Small)
        );
        let o = offer(&[(Tier::Large, 1_400, 81_000)]);
        assert_eq!(recommend(&o, &facts(GpuClass::None)), Some(Tier::Large));
        assert_eq!(step(&o, &facts(GpuClass::None), -1), Some(Tier::Large));
    }

    /// What a size that does not fit lacks, for the page's warning.
    #[test]
    fn unfit_says_why() {
        let o = search();
        // 2 GB of VRAM on a discrete card: a 1 GB budget, Large needs 1,400 MB.
        let f = MachineFacts {
            gpu_memory: Some(2 * GB),
            ..facts(GpuClass::Discrete)
        };
        let a = assess(&o, &f);
        assert_eq!(a[0].unfit, None);
        assert!(a[0].fits && a[1].fits && !a[2].fits);
        assert_eq!(
            a[2].unfit,
            Some(Unfit::Memory {
                need_mb: 1_400,
                budget_mb: 1_024
            })
        );
        let t = r#"
schema = 2
[[model]]
feature = "search"
tier = "large"
id = "g"
version = "1"
role = "x"
urls = ["https://m.invalid/g"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 1
licence = "MIT"
licence_url = "https://m.invalid/L"
placeholder = true
[model.runtime]
format = "safetensors"
gpu = "required"
min_memory_mb = 999999
"#;
        let o = Manifest::parse(t)
            .unwrap()
            .offer(crate::Feature::Search, true)
            .unwrap();
        // On the CPU the GPU is what is missing, whatever the memory.
        let a = assess(&o, &facts(GpuClass::None));
        assert_eq!(a[0].unfit, Some(Unfit::Gpu));
        assert!(matches!(
            assess(&o, &facts(GpuClass::Discrete))[0].unfit,
            Some(Unfit::Memory { .. })
        ));
    }

    #[test]
    fn default_falls_back_to_a_model_of_its_languages() {
        // The built-in Search: B/16 (multilingual, the default) does not fit, B/32
        // (multilingual) and PE Core (English, the larger tier) do: B/32 is chosen.
        let o = Manifest::parse(crate::manifest::BUILTIN)
            .unwrap()
            .offer(crate::Feature::Search, false)
            .unwrap();
        let mb = |t| o.tier_offer(t).unwrap().memory_mb();
        let (small, standard, large) = (mb(Tier::Small), mb(Tier::Standard), mb(Tier::Large));
        assert!(standard < small && small < large);
        // On the CPU the budget is a quarter of the RAM.
        let cpu = |budget_mb: u32| MachineFacts {
            ram: Some((u64::from(budget_mb) * 4) << 20),
            ..facts(GpuClass::None)
        };
        assert_eq!(recommend(&o, &cpu(large)), Some(Tier::Large));
        assert_eq!(recommend(&o, &cpu(large - 1)), Some(Tier::Small));
        // Only PE Core fits: it is the one that runs.
        assert_eq!(recommend(&o, &cpu(small - 1)), Some(Tier::Standard));
        assert_eq!(recommend(&o, &cpu(standard - 1)), None);
    }

    #[test]
    fn gpu_only_sizes_need_a_gpu() {
        let t = r#"
schema = 2
[[model]]
feature = "search"
tier = "large"
id = "g"
version = "1"
role = "x"
urls = ["https://m.invalid/g"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 1
licence = "MIT"
licence_url = "https://m.invalid/L"
placeholder = true
[model.runtime]
format = "safetensors"
gpu = "required"
"#;
        let o = Manifest::parse(t)
            .unwrap()
            .offer(crate::Feature::Search, true)
            .unwrap();
        assert_eq!(recommend(&o, &facts(GpuClass::None)), None);
        assert_eq!(recommend(&o, &facts(GpuClass::Discrete)), Some(Tier::Large));
    }

    #[test]
    fn budgets() {
        let f = |gpu, gpu_memory, ram| MachineFacts {
            gpu,
            gpu_usable: true,
            gpu_memory,
            ram,
            ..MachineFacts::default()
        };
        assert_eq!(
            f(GpuClass::Discrete, Some(8 * GB), Some(GB)).budget_mb(),
            4096
        );
        assert_eq!(f(GpuClass::Discrete, None, None).budget_mb(), 2048);
        // An integrated GPU's carve-out or a quarter of the RAM it shares, the larger.
        assert_eq!(
            f(GpuClass::Integrated, Some(8 * GB), Some(64 * GB)).budget_mb(),
            16384
        );
        assert_eq!(
            f(GpuClass::Integrated, Some(8 * GB), Some(8 * GB)).budget_mb(),
            4096
        );
        assert_eq!(f(GpuClass::None, Some(8 * GB), None).budget_mb(), 2048);
    }

    #[test]
    fn predicted_time_scales_with_work() {
        let o = search();
        let f = MachineFacts {
            gpu_ms: Some(2.0),
            ..facts(GpuClass::Discrete)
        };
        let a = assess(&o, &f);
        let ms: Vec<f32> = a.iter().map(|x| x.ms_per_photo.unwrap()).collect();
        assert!((ms[0] - 2.0 * 2_000.0 / VIT_TI_MMACS as f32).abs() < 1e-3);
        assert!(ms[0] < ms[1] && ms[1] < ms[2]);
    }

    #[test]
    fn size_choice_ids() {
        for c in SizeChoice::ALL {
            assert_eq!(SizeChoice::from_id(c.id()), Some(c));
        }
        // The named sizes are the tiers by their manifest ids; the old relative ones are gone.
        for t in Tier::ALL {
            assert_eq!(SizeChoice::from_id(t.id()), Some(SizeChoice::Tier(t)));
        }
        assert_eq!(SizeChoice::from_id("larger"), None);
        assert_eq!(SizeChoice::default(), SizeChoice::Recommended);
    }

    #[test]
    fn gather_runs() {
        // Whatever this machine is, the facts are coherent and the vram read is harmless.
        let f = MachineFacts::gather(None);
        assert!(f.budget_mb() > 0);
        let _ = vram(0x1002, 0xffff);
    }

    #[test]
    fn reapply_only_on_a_new_basis_and_only_when_recommended() {
        let calibrated = "GPU A|calibrated";
        let before = "GPU A|uncalibrated";
        use SizeChoice::Recommended;
        // Same machine: nothing, whatever the recommendation says now (battery comes and goes).
        assert_eq!(
            reapply(
                Tier::Standard,
                Some(calibrated),
                calibrated,
                Recommended,
                Some(Tier::Large)
            ),
            Reapply::Nothing
        );
        // The first calibration moved the recommendation: ask.
        assert_eq!(
            reapply(
                Tier::Standard,
                Some(before),
                calibrated,
                Recommended,
                Some(Tier::Large)
            ),
            Reapply::Suggest(Tier::Large)
        );
        // It did not: remember the basis, no question.
        assert_eq!(
            reapply(
                Tier::Standard,
                Some(before),
                calibrated,
                Recommended,
                Some(Tier::Standard)
            ),
            Reapply::Record
        );
        // Another GPU, smaller recommendation.
        assert_eq!(
            reapply(
                Tier::Large,
                Some(calibrated),
                "GPU B|calibrated",
                Recommended,
                Some(Tier::Small)
            ),
            Reapply::Suggest(Tier::Small)
        );
        // The user named a size: never re-applied, whichever it is.
        for c in [SizeChoice::Tier(Tier::Small), SizeChoice::Tier(Tier::Large)] {
            assert_eq!(
                reapply(Tier::Small, Some(before), calibrated, c, Some(Tier::Large)),
                Reapply::Record
            );
        }
        // Installed before sizes: the basis is recorded, no question.
        assert_eq!(
            reapply(
                Tier::Standard,
                None,
                calibrated,
                Recommended,
                Some(Tier::Large)
            ),
            Reapply::Record
        );
        // Nothing fits any more: keep what is there.
        assert_eq!(
            reapply(Tier::Standard, Some(before), calibrated, Recommended, None),
            Reapply::Record
        );
        let mut f = facts(GpuClass::Discrete);
        f.gpu_ms = None;
        f.cpu_ms = None;
        assert_eq!(basis("GPU A", &f), before);
        f.gpu_ms = Some(3.0);
        assert_eq!(basis("GPU A", &f), calibrated);
    }
}
