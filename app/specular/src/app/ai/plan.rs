//! What a Local AI setting change, or a change of the machine, means for a feature: install
//! a size, ask before switching, remember the machine, or nothing (DESIGN.md §10 "Choosing
//! models"). Pure, so the rules are tested without a pack or a GPU; the page's worker
//! ([`super::Pages`]) applies the result.
//!
//! - Switching a feature on installs the size the setting picks (Recommended, or the size it
//!   names), unless it is installed or on its way.
//! - Changing the size of an installed feature to one that means another download and a
//!   re-index asks first, with the download size, and with why the size may not run here
//!   when it does not fit. Naming such a size for a feature not installed yet asks too.
//! - The recommendation is looked at again when the machine it was made on changed (the
//!   first calibration, another GPU), and only while the size is left on Recommended; a new
//!   one is offered, never downloaded silently.
//! - The old relative settings (Smaller, Larger) become the size they mean on this machine
//!   ([`migrate`]).

use specular_ai::choose::{self, MachineFacts, Reapply, SizeChoice, Unfit};
use specular_ai::{FeatureState, FeatureStatus, Offer, Tier};
use specular_settings::ModelSize;

/// Why a feature is looked at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Why {
    /// Its switch was turned on.
    On,
    /// Its size setting changed.
    Size,
    /// The machine may have changed (startup, after the calibration, now and then).
    Check,
}

/// A size switch to ask about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Ask {
    pub(crate) tier: Tier,
    /// Bytes it downloads.
    pub(crate) download: u64,
    /// The recommendation moved (rather than the user's setting).
    pub(crate) suggested: bool,
    /// Why the size may not run on this machine (None: it fits).
    pub(crate) unfit: Option<Unfit>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Nothing,
    /// Install this size (recording the machine it was chosen on).
    Install(Tier),
    Ask(Ask),
    /// The size stays: remember the machine (no question).
    Record,
    /// A pending question no longer applies (the setting is back on the installed size).
    Forget,
}

/// The size setting as a choice; None for the old relative values ([`migrate`] first).
pub(crate) fn choice(m: ModelSize) -> Option<SizeChoice> {
    Some(match m {
        ModelSize::Recommended => SizeChoice::Recommended,
        ModelSize::Small => SizeChoice::Tier(Tier::Small),
        ModelSize::Standard => SizeChoice::Tier(Tier::Standard),
        ModelSize::Large => SizeChoice::Tier(Tier::Large),
        ModelSize::Smaller | ModelSize::Larger => return None,
    })
}

/// The size setting for a choice.
pub(crate) fn setting(c: SizeChoice) -> ModelSize {
    match c {
        SizeChoice::Recommended => ModelSize::Recommended,
        SizeChoice::Tier(Tier::Small) => ModelSize::Small,
        SizeChoice::Tier(Tier::Standard) => ModelSize::Standard,
        SizeChoice::Tier(Tier::Large) => ModelSize::Large,
    }
}

/// An old relative setting (one size either side of the recommendation) as the absolute
/// choice that keeps what it means on this machine: the size it resolves to now, or
/// Recommended when it resolves to none (no offer, nothing fits). None: not an old value.
pub(crate) fn migrate(
    m: ModelSize,
    offer: Option<&Offer>,
    facts: &MachineFacts,
) -> Option<SizeChoice> {
    let by = match m {
        ModelSize::Smaller => -1,
        ModelSize::Larger => 1,
        _ => return None,
    };
    Some(
        offer
            .and_then(|o| choose::step(o, facts, by))
            .map_or(SizeChoice::Recommended, SizeChoice::Tier),
    )
}

/// What `why` means for the feature in `s`, its switch `on` and size setting `size`, on the
/// machine `facts` (whose [`choose::basis`] is `now`). `download` gives a size's download.
pub(crate) fn decide(
    s: &FeatureStatus,
    facts: &MachineFacts,
    now: &str,
    on: bool,
    size: SizeChoice,
    why: Why,
    download: impl Fn(Tier) -> Option<u64>,
) -> Step {
    let busy = matches!(
        s.state,
        FeatureState::Downloading { .. } | FeatureState::Removing
    );
    if busy || s.state == FeatureState::Unavailable {
        return Step::Nothing;
    }
    let offer = s.offer.as_ref();
    let chosen = offer.and_then(|o| choose::choose(o, facts, size));
    let unfit = |tier: Tier| {
        offer.and_then(|o| {
            choose::assess(o, facts)
                .into_iter()
                .find(|f| f.tier == tier)
                .and_then(|f| f.unfit)
        })
    };
    let ask = |tier: Tier, suggested: bool| {
        Step::Ask(Ask {
            tier,
            download: download(tier).unwrap_or(0),
            suggested,
            unfit: unfit(tier),
        })
    };
    match (why, s.tier) {
        (Why::On, None) if on && s.state != FeatureState::Installed => {
            chosen.map_or(Step::Nothing, Step::Install)
        }
        (Why::On, _) => Step::Nothing,
        (Why::Size, Some(cur)) => match chosen {
            Some(t) if t != cur => ask(t, false),
            _ => Step::Forget,
        },
        // Not installed: the size applies when it is switched on; one that may not run here
        // is asked about now, while the choice is fresh.
        (Why::Size, None) => match chosen {
            Some(t) if unfit(t).is_some() && matches!(size, SizeChoice::Tier(_)) => ask(t, false),
            _ => Step::Forget,
        },
        // Mid-switch: the new size is indexing; look again once it is done.
        (Why::Check, Some(_)) if s.previous.is_some() => Step::Nothing,
        (Why::Check, Some(cur)) => {
            match choose::reapply(cur, s.basis.as_deref(), now, size, chosen) {
                Reapply::Nothing => Step::Nothing,
                Reapply::Suggest(t) if on => ask(t, true),
                Reapply::Record | Reapply::Suggest(_) => Step::Record,
            }
        }
        (Why::Check, None) => Step::Nothing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use specular_ai::Feature;
    use specular_ai::Manifest;
    use specular_core::paths::GpuClass;

    const GB: u64 = 1 << 30;

    /// Search in three sizes (memory MB, work MMACs) like the built-in manifest.
    fn offer() -> specular_ai::Offer {
        let mut t = String::from("schema = 2\n");
        for (tier, mb, mmacs) in [
            ("small", 300, 1_000),
            ("standard", 900, 4_400),
            ("large", 3_000, 81_000),
        ] {
            t += &format!(
                r#"
[[model]]
feature = "search"
tier = "{tier}"
id = "m-{tier}"
version = "1"
role = "image-text-encoder"
urls = ["https://models.example/{tier}"]
sha256 = "0000000000000000000000000000000000000000000000000000000000000000"
size = 1000
licence = "MIT"
licence_url = "https://models.example/licence"
[model.runtime]
format = "safetensors"
min_memory_mb = {mb}
gpu = "optional"
mmacs = {mmacs}
"#
            );
        }
        Manifest::parse(&t)
            .unwrap()
            .offer(Feature::Search, false)
            .unwrap()
    }

    fn facts(gpu: GpuClass, vram: u64, gpu_ms: Option<f32>) -> MachineFacts {
        MachineFacts {
            gpu,
            gpu_usable: true,
            gpu_memory: Some(vram),
            ram: Some(16 * GB),
            gpu_ms,
            cpu_ms: None,
            low_power: false,
        }
    }

    fn status(state: FeatureState, tier: Option<Tier>, basis: Option<&str>) -> FeatureStatus {
        FeatureStatus {
            feature: Feature::Search,
            state,
            offer: Some(offer()),
            tier,
            target: None,
            previous: None,
            basis: basis.map(Into::into),
        }
    }

    fn dl(t: Tier) -> Option<u64> {
        Some(match t {
            Tier::Small => 10,
            Tier::Standard => 20,
            Tier::Large => 30,
        })
    }

    #[test]
    fn old_relative_sizes_become_the_size_they_mean_here() {
        let f = facts(GpuClass::Discrete, 8 * GB, None);
        let rec = choose::recommend(&offer(), &f).unwrap();
        let up = choose::step(&offer(), &f, 1).unwrap();
        let down = choose::step(&offer(), &f, -1).unwrap();
        // One size either side, clamped to the sizes that fit.
        assert!(
            up >= rec && down <= rec && up > down,
            "{down:?} {rec:?} {up:?}"
        );
        assert_eq!(
            migrate(ModelSize::Larger, Some(&offer()), &f),
            Some(SizeChoice::Tier(up))
        );
        assert_eq!(
            migrate(ModelSize::Smaller, Some(&offer()), &f),
            Some(SizeChoice::Tier(down))
        );
        // No offer (no models in this build): Recommended.
        assert_eq!(
            migrate(ModelSize::Larger, None, &f),
            Some(SizeChoice::Recommended)
        );
        // Not an old value: nothing to migrate.
        for m in [
            ModelSize::Recommended,
            ModelSize::Small,
            ModelSize::Standard,
            ModelSize::Large,
        ] {
            assert_eq!(migrate(m, Some(&offer()), &f), None);
            assert_eq!(choice(m).map(setting), Some(m));
        }
        assert_eq!(choice(ModelSize::Smaller), None);
        assert_eq!(choice(ModelSize::Larger), None);
        for c in SizeChoice::ALL {
            assert_eq!(choice(setting(c)), Some(c));
        }
    }

    #[test]
    fn switching_on_installs_the_chosen_size_once() {
        let f = facts(GpuClass::Discrete, 8 * GB, None);
        let rec = choose::recommend(&offer(), &f).unwrap();
        let avail = status(FeatureState::Available, None, None);
        let go =
            |s: &FeatureStatus, size| decide(s, &f, "gpu|uncalibrated", true, size, Why::On, dl);
        assert_eq!(go(&avail, SizeChoice::Recommended), Step::Install(rec));
        assert_eq!(
            go(&avail, SizeChoice::Tier(Tier::Small)),
            Step::Install(Tier::Small)
        );
        // A failed install is offered again: on installs again.
        let failed = status(FeatureState::Failed("x".into()), None, None);
        assert_eq!(go(&failed, SizeChoice::Recommended), Step::Install(rec));
        // Installed or on its way: nothing more.
        let inst = status(FeatureState::Installed, Some(rec), Some("b"));
        assert_eq!(go(&inst, SizeChoice::Recommended), Step::Nothing);
        let dling = status(FeatureState::Downloading { done: 1, total: 2 }, None, None);
        assert_eq!(go(&dling, SizeChoice::Recommended), Step::Nothing);
        // Switched off again before the turn was seen: nothing.
        assert_eq!(
            decide(&avail, &f, "b", false, SizeChoice::Recommended, Why::On, dl),
            Step::Nothing
        );
        // No offer: nothing.
        let mut none = avail.clone();
        none.offer = None;
        assert_eq!(go(&none, SizeChoice::Recommended), Step::Nothing);
    }

    #[test]
    fn a_size_change_asks_with_the_download_and_forgets_when_changed_back() {
        let f = facts(GpuClass::Discrete, 8 * GB, None);
        let rec = choose::recommend(&offer(), &f).unwrap();
        assert_ne!(rec, Tier::Small);
        let inst = status(FeatureState::Installed, Some(rec), Some("b"));
        let small = SizeChoice::Tier(Tier::Small);
        assert_eq!(
            decide(&inst, &f, "b", true, small, Why::Size, dl),
            Step::Ask(Ask {
                tier: Tier::Small,
                download: dl(Tier::Small).unwrap(),
                suggested: false,
                unfit: None,
            })
        );
        assert_eq!(
            decide(&inst, &f, "b", true, SizeChoice::Recommended, Why::Size, dl),
            Step::Forget
        );
        // Naming the size in use: no question.
        assert_eq!(
            decide(&inst, &f, "b", true, SizeChoice::Tier(rec), Why::Size, dl),
            Step::Forget
        );
        // Not installed: the size applies when it is switched on.
        let avail = status(FeatureState::Available, None, None);
        assert_eq!(
            decide(&avail, &f, "b", true, small, Why::Size, dl),
            Step::Forget
        );
    }

    #[test]
    fn a_size_that_does_not_fit_is_asked_about_with_why() {
        // 2 GB of VRAM: a 1 GB budget; Large needs 3,000 MB.
        let f = facts(GpuClass::Discrete, 2 * GB, None);
        let large = SizeChoice::Tier(Tier::Large);
        let unfit = Some(Unfit::Memory {
            need_mb: 3_000,
            budget_mb: 1_024,
        });
        let want = Step::Ask(Ask {
            tier: Tier::Large,
            download: dl(Tier::Large).unwrap(),
            suggested: false,
            unfit,
        });
        // Installed at another size: the switch question carries the warning.
        let inst = status(FeatureState::Installed, Some(Tier::Small), Some("b"));
        assert_eq!(decide(&inst, &f, "b", true, large, Why::Size, dl), want);
        // Not installed yet: asked now, while the choice is fresh.
        let avail = status(FeatureState::Available, None, None);
        assert_eq!(decide(&avail, &f, "b", true, large, Why::Size, dl), want);
        // A size that fits, not installed: nothing to ask.
        assert_eq!(
            decide(
                &avail,
                &f,
                "b",
                true,
                SizeChoice::Tier(Tier::Small),
                Why::Size,
                dl
            ),
            Step::Forget
        );
        // Kept anyway and switched on: installed as chosen.
        assert_eq!(
            decide(&avail, &f, "b", true, large, Why::On, dl),
            Step::Install(Tier::Large)
        );
    }

    #[test]
    fn the_recommendation_is_offered_again_only_on_a_new_machine_and_on_recommended() {
        // Chosen before calibration on an integrated GPU: small.
        let before = facts(GpuClass::Integrated, 0, None);
        let rec = choose::recommend(&offer(), &before).unwrap();
        let b0 = choose::basis("iGPU", &before);
        let inst = status(FeatureState::Installed, Some(rec), Some(&b0));
        let check = |s: &FeatureStatus, f: &MachineFacts, now: &str, on, size| {
            decide(s, f, now, on, size, Why::Check, dl)
        };
        // Same machine: nothing.
        assert_eq!(
            check(&inst, &before, &b0, true, SizeChoice::Recommended),
            Step::Nothing
        );
        // A discrete GPU with room: a larger size is suggested, never installed.
        let after = facts(GpuClass::Discrete, 16 * GB, Some(0.5));
        let now = choose::basis("dGPU", &after);
        let rec2 = choose::recommend(&offer(), &after).unwrap();
        assert_ne!(rec2, rec);
        assert_eq!(
            check(&inst, &after, &now, true, SizeChoice::Recommended),
            Step::Ask(Ask {
                tier: rec2,
                download: dl(rec2).unwrap(),
                suggested: true,
                unfit: None,
            })
        );
        // A size chosen by name stays, whatever it is: only the machine is remembered.
        for t in [Tier::Small, Tier::Standard, Tier::Large] {
            assert_eq!(
                check(&inst, &after, &now, true, SizeChoice::Tier(t)),
                Step::Record
            );
        }
        // Switched off: remembered, not asked.
        assert_eq!(
            check(&inst, &after, &now, false, SizeChoice::Recommended),
            Step::Record
        );
        // Mid-switch: looked at again later.
        let mut switching = inst.clone();
        switching.previous = Some(Tier::Large);
        assert_eq!(
            check(&switching, &after, &now, true, SizeChoice::Recommended),
            Step::Nothing
        );
        // Installed before sizes (no basis): remembered.
        let old = status(FeatureState::Installed, Some(rec), None);
        assert_eq!(
            check(&old, &after, &now, true, SizeChoice::Recommended),
            Step::Record
        );
    }
}
