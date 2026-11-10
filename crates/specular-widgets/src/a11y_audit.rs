//! A screen-reader audit of an accessibility tree: what Orca, NVDA or VoiceOver would trip
//! over. Tests run it over every screen they draw (the chrome's screenshot cases, the app's
//! dialogs) and fail on any finding.
//!
//! - **Names:** every control (a role a user operates, or a node that can be clicked or
//!   focused) has a name (a label, or a node naming it).
//! - **Roles:** something that can be clicked is not plain text or a bare container.
//! - **State:** a disabled control offers no click; tick boxes, switches, radio buttons and toggle buttons say whether they are
//!   on; a control that opens a menu or list says whether it is open (one that opens a
//!   dialog need not); tabs, list options and leaf tree items say whether they are selected.
//! - **Order:** the controls of one container come in the order they are seen (top to
//!   bottom, then along the line in the reading direction), which is the order a screen
//!   reader walks them. Two allowances: a container's last controls may be its title bar's
//!   (on its top line, in order among themselves), as Tab reaches a dialog's
//!   Close after its content; and in a right-to-left interface a row kept left to right
//!   (media controls) reads left to right.
//! - **Structure:** ids are unique, every node but the root has one parent, and the focus
//!   is on a node in the tree.

use std::collections::{HashMap, HashSet};

use accesskit::{Action, HasPopup, Node, NodeId, Rect, Role, TreeUpdate};

/// Roles a user operates.
const CONTROLS: &[Role] = &[
    Role::Button,
    Role::DefaultButton,
    Role::CheckBox,
    Role::Switch,
    Role::RadioButton,
    Role::ComboBox,
    Role::EditableComboBox,
    Role::TextInput,
    Role::MultilineTextInput,
    Role::SearchInput,
    Role::NumberInput,
    Role::PasswordInput,
    Role::SpinButton,
    Role::Slider,
    Role::Link,
    Role::MenuItem,
    Role::MenuItemCheckBox,
    Role::MenuItemRadio,
    Role::MenuListOption,
    Role::Tab,
    Role::ListBoxOption,
    Role::TreeItem,
    Role::ColorWell,
    Role::DisclosureTriangle,
];

/// Roles that say whether they are on.
const TOGGLES: &[Role] = &[
    Role::CheckBox,
    Role::Switch,
    Role::RadioButton,
    Role::MenuItemCheckBox,
    Role::MenuItemRadio,
];

/// Roles that say whether they are selected.
const SELECTABLE: &[Role] = &[Role::Tab, Role::ListBoxOption, Role::TreeItem];

/// Roles that must not be what a user clicks: text, or a container with no meaning.
const NOT_CLICKABLE: &[Role] = &[
    Role::Unknown,
    Role::TextRun,
    Role::Label,
    Role::Paragraph,
    Role::GenericContainer,
];

/// How far (physical px) one box may reach into the next and still count as before it.
const SLACK: f64 = 2.0;

fn is_control(n: &Node) -> bool {
    CONTROLS.contains(&n.role())
        || n.supports_action(Action::Click)
        || n.supports_action(Action::Focus)
}

fn named(n: &Node) -> bool {
    n.label().is_some_and(|l| !l.trim().is_empty()) || !n.labelled_by().is_empty()
}

/// `b` is seen before `a`: on the same line and before it in the reading direction, or
/// above it in the same column, or above it in a column before its. (Above it in a later
/// column is not: a form in two columns reads one column, then the next.)
fn seen_before(b: &Rect, a: &Rect, rtl: bool) -> bool {
    let before_on_line = |b: &Rect, a: &Rect| {
        if rtl {
            b.x0 >= a.x1 - SLACK
        } else {
            b.x1 <= a.x0 + SLACK
        }
    };
    let above = b.y1 <= a.y0 + SLACK;
    let below = a.y1 <= b.y0 + SLACK;
    if !above && !below {
        // Overlapping lines: the one whose middle is clearly higher comes first.
        let (ma, mb) = ((a.y0 + a.y1) * 0.5, (b.y0 + b.y1) * 0.5);
        let line = (a.y1 - a.y0).min(b.y1 - b.y0) * 0.5;
        if mb + line < ma {
            return true;
        }
        if ma + line < mb {
            return false;
        }
        return before_on_line(b, a);
    }
    if below {
        return false;
    }
    let same_column = b.x0 < a.x1 - SLACK && a.x0 < b.x1 - SLACK;
    same_column || before_on_line(b, a)
}

/// The controls of one container out of visual order (see the module's allowances).
fn order_findings(controls: &[(NodeId, &Node, Rect)], rtl: bool) -> Vec<String> {
    // A title bar's controls last: a final run on the container's top line.
    let top = controls
        .iter()
        .map(|c| c.2)
        .min_by(|a, b| a.y0.total_cmp(&b.y0));
    let in_bar = |r: &Rect| top.is_some_and(|t| r.y0 < t.y1 && t.y0 < r.y1);
    let mut body = controls.len();
    while body > 1 && in_bar(&controls[body - 1].2) {
        body -= 1;
    }
    // All on one line: a row, no title bar.
    if body == 1 && in_bar(&controls[0].2) {
        body = controls.len();
    }
    let check = |run: &[(NodeId, &Node, Rect)], rtl: bool| -> Vec<String> {
        run.windows(2)
            .filter(|w| seen_before(&w[1].2, &w[0].2, rtl))
            .map(|w| {
                format!(
                    "{} comes after {} but is seen before it",
                    describe(w[1].0, w[1].1),
                    describe(w[0].0, w[0].1)
                )
            })
            .collect()
    };
    let mut out = Vec::new();
    for run in [&controls[..body], &controls[body..]] {
        let f = check(run, rtl);
        // A row kept left to right in a right-to-left interface.
        if rtl && !f.is_empty() && check(run, false).is_empty() {
            continue;
        }
        out.extend(f);
    }
    out
}

fn describe(id: NodeId, n: &Node) -> String {
    let at = n.bounds().map_or_else(String::new, |b| {
        format!(" at {:.0},{:.0}–{:.0},{:.0}", b.x0, b.y0, b.x1, b.y1)
    });
    match n.label() {
        Some(l) if !l.is_empty() => format!("{:?} {id:?} \"{l}\"{at}", n.role()),
        _ => format!("{:?} {id:?}{at}", n.role()),
    }
}

/// The findings for `tree` (a full tree), each a line naming the node; empty when it is
/// clean. `rtl`: the interface reads right to left.
pub fn audit(tree: &TreeUpdate, rtl: bool) -> Vec<String> {
    let mut out = Vec::new();
    let nodes: HashMap<NodeId, &Node> = tree.nodes.iter().map(|(id, n)| (*id, n)).collect();
    if nodes.len() != tree.nodes.len() {
        out.push("duplicate node ids".to_owned());
    }
    let mut parents: HashMap<NodeId, NodeId> = HashMap::new();
    for (id, n) in &tree.nodes {
        for c in n.children() {
            if !nodes.contains_key(c) {
                out.push(format!("{} has an undefined child {c:?}", describe(*id, n)));
            }
            if parents.insert(*c, *id).is_some() {
                out.push(format!("{c:?} has two parents"));
            }
        }
    }
    let Some(root) = tree.tree.as_ref().map(|t| t.root) else {
        out.push("not a full tree".to_owned());
        return out;
    };
    for (id, n) in &tree.nodes {
        if *id != root && !parents.contains_key(id) {
            out.push(format!("{} is an orphan", describe(*id, n)));
        }
    }
    if !nodes.contains_key(&tree.focus) {
        out.push(format!("the focus {:?} is not in the tree", tree.focus));
    }

    // Walk from the root, as a screen reader does; hidden subtrees are skipped. Each node
    // carries the box its ancestors show it in (controls scrolled partly out of it are not
    // ordered: they are not where they would be seen).
    let mut seen = HashSet::new();
    let mut stack: Vec<(NodeId, Option<Rect>)> = vec![(root, None)];
    while let Some((id, clip)) = stack.pop() {
        if !seen.insert(id) {
            continue;
        }
        let Some(n) = nodes.get(&id).copied() else {
            continue;
        };
        if n.is_hidden() {
            continue;
        }
        let role = n.role();
        let what = || describe(id, n);
        if is_control(n) && id != root && !named(n) {
            out.push(format!("{} has no name", what()));
        }
        if n.supports_action(Action::Click) && NOT_CLICKABLE.contains(&role) {
            out.push(format!("{} can be clicked but is not a control", what()));
        }
        if n.is_disabled() && n.supports_action(Action::Click) {
            out.push(format!("{} is disabled but offers a click", what()));
        }
        if TOGGLES.contains(&role) && n.toggled().is_none() {
            out.push(format!("{} does not say whether it is on", what()));
        }
        let opens =
            n.has_popup().is_some_and(|p| p != HasPopup::Dialog) || matches!(role, Role::ComboBox);
        if opens && n.is_expanded().is_none() {
            out.push(format!("{} does not say whether it is open", what()));
        }
        let branch = role == Role::TreeItem && n.is_expanded().is_some();
        if SELECTABLE.contains(&role) && !branch && n.is_selected().is_none() {
            out.push(format!("{} does not say whether it is selected", what()));
        }
        // The controls among the children, in tree order, against where they are drawn.
        let clip = match (clip, n.bounds()) {
            (Some(c), Some(b)) => Some(c.intersect(b)),
            (c, b) => c.or(b),
        };
        let inside = |b: &Rect| {
            clip.is_none_or(|c| {
                b.x0 >= c.x0 - SLACK
                    && b.y0 >= c.y0 - SLACK
                    && b.x1 <= c.x1 + SLACK
                    && b.y1 <= c.y1 + SLACK
            })
        };
        let controls: Vec<(NodeId, &Node, Rect)> = n
            .children()
            .iter()
            .filter_map(|c| nodes.get(c).map(|m| (*c, *m)))
            .filter(|(_, m)| !m.is_hidden() && is_control(m))
            .filter_map(|(c, m)| m.bounds().map(|b| (c, m, b)))
            .filter(|(_, _, b)| b.x1 > b.x0 && b.y1 > b.y0 && inside(b))
            .collect();
        out.extend(order_findings(&controls, rtl));
        // Children in reverse, so they are visited in order.
        stack.extend(n.children().iter().rev().map(|c| (*c, clip)));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use accesskit::{Toggled, TreeId, TreeInfo};

    fn tree(nodes: Vec<(NodeId, Node)>) -> TreeUpdate {
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(NodeId(1))),
            tree_id: TreeId::ROOT,
            focus: NodeId(1),
        }
    }

    fn at(mut n: Node, x: f64, y: f64) -> Node {
        n.set_bounds(Rect::new(x, y, x + 40.0, y + 20.0));
        n
    }

    fn button(label: &str) -> Node {
        let mut n = Node::new(Role::Button);
        n.set_label(label);
        n.add_action(Action::Click);
        n
    }

    #[test]
    fn a_clean_tree_has_no_findings() {
        let mut root = Node::new(Role::Window);
        root.set_label("w");
        root.set_children(vec![NodeId(2), NodeId(3), NodeId(4)]);
        let mut tick = at(Node::new(Role::CheckBox), 0.0, 40.0);
        tick.set_label("Loop");
        tick.set_toggled(Toggled::False);
        tick.add_action(Action::Click);
        let t = tree(vec![
            (NodeId(1), root),
            (NodeId(2), at(button("Back"), 0.0, 0.0)),
            (NodeId(3), at(button("Next"), 50.0, 0.0)),
            (NodeId(4), tick),
        ]);
        assert_eq!(audit(&t, false), Vec::<String>::new());
        // A row kept left to right reads left to right in a right-to-left interface too.
        assert_eq!(audit(&t, true), Vec::<String>::new());
        // A close button last, in the title bar above the rest: allowed.
        let mut close = t.clone();
        close.nodes[0].1.push_child(NodeId(5));
        close
            .nodes
            .push((NodeId(5), at(button("Close"), 200.0, -30.0)));
        assert_eq!(audit(&close, false), Vec::<String>::new());
        // Not when it is below the title bar, above the content it follows.
        close.nodes[4]
            .1
            .set_bounds(Rect::new(0.0, 25.0, 40.0, 35.0));
        assert_eq!(audit(&close, false).len(), 1, "{:?}", audit(&close, false));
    }

    #[test]
    fn finds_names_roles_state_and_order() {
        let mut root = Node::new(Role::Window);
        root.set_children(vec![NodeId(2), NodeId(3), NodeId(4), NodeId(5), NodeId(6)]);
        let mut text = at(Node::new(Role::Label), 0.0, 100.0);
        text.set_label("Click me");
        text.add_action(Action::Click);
        let mut tick = at(Node::new(Role::CheckBox), 0.0, 130.0);
        tick.set_label("Loop");
        let mut menu = at(button("More"), 0.0, 160.0);
        menu.set_has_popup(HasPopup::Menu);
        let mut opt = at(Node::new(Role::ListBoxOption), 0.0, 190.0);
        opt.set_label("One");
        menu.set_disabled();
        let t = tree(vec![
            (NodeId(1), root),
            // Unnamed, and below the next one.
            (NodeId(2), at(button(""), 0.0, 50.0)),
            (NodeId(3), at(button("Top"), 0.0, 0.0)),
            (NodeId(4), text),
            (NodeId(5), tick),
            (NodeId(6), menu),
            (NodeId(7), opt),
        ]);
        let f = audit(&t, false);
        let has = |s: &str| f.iter().any(|l| l.contains(s));
        assert!(has("Button #2 at 0,50–40,70 has no name"), "{f:#?}");
        assert!(has("seen before"), "{f:#?}");
        assert!(has("can be clicked but is not a control"), "{f:#?}");
        assert!(has("whether it is on"), "{f:#?}");
        assert!(has("whether it is open"), "{f:#?}");
        assert!(has("disabled but offers a click"), "{f:#?}");
        // The option is an orphan (never reached by the walk).
        assert!(has("orphan"), "{f:#?}");
    }
}
