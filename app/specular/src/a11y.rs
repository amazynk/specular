//! Screen readers (AccessKit). The adapter is created with the window, but the chrome's tree is
//! built only while an assistive technology is listening: activation, actions and deactivation
//! arrive as loader events on the UI thread, and every frame after activation sends the full
//! tree again if it changed.

use specular_gfx::winit::event::WindowEvent;
use specular_gfx::winit::event_loop::ActiveEventLoop;
use specular_gfx::winit::window::Window;
use specular_widgets::accesskit::{
    ActionHandler, ActionRequest, ActivationHandler, DeactivationHandler, Node, NodeId, TreeUpdate,
};

use crate::loader::{Event, Loader};

/// What the platform adapter asks for, delivered through the loader's event channel.
#[derive(Debug)]
pub enum A11yEvent {
    Activated,
    Action(ActionRequest),
    Deactivated,
}

struct Handler(Loader);

impl ActivationHandler for Handler {
    fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
        // The UI thread owns the chrome: it sends the full tree on its next frame.
        self.0.post(Event::Accessibility(A11yEvent::Activated));
        None
    }
}

impl ActionHandler for Handler {
    fn do_action(&mut self, request: ActionRequest) {
        self.0
            .post(Event::Accessibility(A11yEvent::Action(request)));
    }
}

impl DeactivationHandler for Handler {
    fn deactivate_accessibility(&mut self) {
        self.0.post(Event::Accessibility(A11yEvent::Deactivated));
    }
}

pub struct Accessibility {
    adapter: accesskit_winit::Adapter,
    active: bool,
    /// The last tree sent (nodes and focus), to skip unchanged frames.
    last: Option<(Vec<(NodeId, Node)>, NodeId)>,
}

impl Accessibility {
    /// Must run before the window is first shown.
    pub fn new(el: &ActiveEventLoop, window: &Window, loader: &Loader) -> Self {
        let adapter = accesskit_winit::Adapter::with_direct_handlers(
            el,
            window,
            Handler(loader.clone()),
            Handler(loader.clone()),
            Handler(loader.clone()),
        );
        Self {
            adapter,
            active: false,
            last: None,
        }
    }

    pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
        self.adapter.process_event(window, event);
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Activation or deactivation; the next update sends the full tree.
    pub fn set_active(&mut self, on: bool) {
        self.active = on;
        self.last = None;
    }

    /// The last tree sent and its focus (the `ui-state` test command, debug builds).
    #[cfg(debug_assertions)]
    pub fn last(&self) -> Option<(&[(NodeId, Node)], NodeId)> {
        self.last.as_ref().map(|(n, f)| (n.as_slice(), *f))
    }

    /// Sends `tree` unless it equals the last one sent.
    pub fn update(&mut self, mut tree: TreeUpdate) {
        if !self.active {
            return;
        }
        specular_widgets::strip_isolates(&mut tree);
        let focus = tree.focus;
        if self
            .last
            .as_ref()
            .is_some_and(|(n, f)| *f == focus && *n == tree.nodes)
        {
            return;
        }
        self.last = Some((tree.nodes.clone(), focus));
        self.adapter.update_if_active(|| tree);
    }
}
