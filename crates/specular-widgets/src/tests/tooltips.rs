use crate::Chrome;
use crate::ctx::Target;
use specular_gfx::Rect;
use specular_settings::ToolbarItem;

/// No tooltip for the control whose popover is open, none over the popover, and none for
/// that control after the popover closes until the pointer has left it and come back.
#[test]
fn popovers_hide_their_owners_tooltip() {
    let owner = Target::Toolbar(ToolbarItem::ZoomLevel);
    let menu = Rect::new(100.0, 100.0, 120.0, 180.0);
    let mut c = Chrome::new();
    c.pointer = Some((150.0, 400.0));
    c.hover = Some(owner.clone());
    assert!(
        c.tooltip_allowed(&[]),
        "tooltip shows before the popover opens"
    );

    c.zoom_menu = true;
    assert!(
        !c.tooltip_allowed(&[menu]),
        "owner while its popover is open"
    );
    // Over the popover itself.
    c.pointer = Some((150.0, 150.0));
    c.hover = Some(Target::ZoomPreset(specular_input::Action::ZoomFit));
    assert!(!c.tooltip_allowed(&[menu]), "over the popover");
    // Another control while the popover is open still gets its tooltip.
    c.pointer = Some((300.0, 400.0));
    c.hover = Some(Target::Toolbar(ToolbarItem::Info));
    assert!(c.tooltip_allowed(&[menu]));

    // Closed by clicking the owner: still hidden while the pointer stays on it.
    c.zoom_menu = false;
    c.pointer = Some((150.0, 400.0));
    c.hover = Some(owner.clone());
    assert!(
        !c.tooltip_allowed(&[]),
        "just closed, pointer still on the owner"
    );
    assert!(!c.tooltip_allowed(&[]));
    // Leave and come back: the tooltip is allowed again.
    c.hover = None;
    assert!(c.tooltip_allowed(&[]));
    c.hover = Some(owner);
    assert!(c.tooltip_allowed(&[]), "after leaving and coming back");
}
