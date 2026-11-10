use super::*;

#[test]
fn relight_actions_need_a_relightable_image() {
    let plain = ActionState {
        image: ImageState::Ready,
        count: 1,
        ..Default::default()
    };
    let rti = ActionState {
        relight: true,
        import_sources: false,
        ..plain
    };
    for a in [
        Action::RelightLeft,
        Action::RelightRight,
        Action::RelightUp,
        Action::RelightDown,
        Action::RelightMode,
        Action::RelightReset,
        Action::RelightWidget,
    ] {
        assert_eq!(a.disabled(&plain), Some(Disabled::NotRelightable), "{a:?}");
        assert_eq!(a.disabled(&rti), None, "{a:?}");
    }
}
