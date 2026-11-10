//! The screen-reader audit ([`specular_widgets::a11y_audit`]) over the app's own dialogs,
//! sheets and panels, as their screenshot tests draw them: each module's accessibility nodes
//! under a window root, checked for unnamed controls, wrong roles, missing state and an
//! order that does not follow what is seen.

use specular_widgets::accesskit::{Node, NodeId, Rect, Role, TreeId, TreeInfo, TreeUpdate};

/// A tree with only the window (`size` in logical px, `scale` physical per logical), for a
/// module's `a11y` to add its nodes to.
pub(super) fn window(size: (f32, f32), scale: f64) -> TreeUpdate {
    let mut root = Node::new(Role::Window);
    root.set_label("Specular");
    root.set_bounds(Rect::new(
        0.0,
        0.0,
        f64::from(size.0) * scale,
        f64::from(size.1) * scale,
    ));
    TreeUpdate {
        nodes: vec![(NodeId(1), root)],
        tree: Some(TreeInfo::new(NodeId(1))),
        tree_id: TreeId::ROOT,
        focus: NodeId(1),
    }
}

/// Fails, listing every finding, unless `tree` passes the audit.
#[track_caller]
pub(super) fn expect_clean(name: &str, tree: &TreeUpdate, rtl: bool) {
    let f = specular_widgets::a11y_audit::audit(tree, rtl);
    assert!(
        f.is_empty(),
        "{name}: screen-reader audit, {} findings:\n{}",
        f.len(),
        f.join("\n")
    );
}

/// Audits what `add` puts under a window of `size` (logical px, at 1×): fails, listing
/// every finding, unless it passes, and when `add` adds nothing. With `A11Y_COLLECT` set it
/// prints the findings (`A11Y <name>: …`) instead of failing, to list them all in one run.
#[track_caller]
pub(super) fn audit_screen(
    name: &str,
    size: (f32, f32),
    rtl: bool,
    add: impl FnOnce(&mut TreeUpdate),
) {
    let mut tree = window(size, 1.0);
    add(&mut tree);
    assert!(tree.nodes.len() > 1, "{name}: no accessibility nodes");
    if std::env::var_os("A11Y_COLLECT").is_some() {
        for l in specular_widgets::a11y_audit::audit(&tree, rtl) {
            eprintln!("A11Y {name}: {l}");
        }
        return;
    }
    expect_clean(name, &tree, rtl);
}
