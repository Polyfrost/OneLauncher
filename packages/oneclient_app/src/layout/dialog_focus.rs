use std::{
    any::Any,
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use freya::{borderless::BorderlessPlugin, prelude::*};
use freya_core::{
    accessibility::tree::ACCESSIBILITY_ROOT_ID, element::EventHandlerType,
    elements::rect::RectElement, events::name::EventName, node_id::NodeId, tree::Tree,
};
use freya_winit::plugins::{FreyaPlugin, PluginEvent, PluginHandle};

use crate::components::{MountedDialog, mounted_dialogs};

#[derive(Default)]
pub struct DialogFocusPlugin {
    borderless: BorderlessPlugin,
    tabs: Rc<RefCell<HashMap<u64, Vec<bool>>>>,
    windows: HashMap<u64, WindowFocus>,
}

#[derive(Default)]
struct FocusPath {
    target: Option<AccessibilityId>,
    ancestors: Vec<AccessibilityId>,
}

thread_local! {
    static PATHS: RefCell<HashMap<u64, FocusPath>> = RefCell::new(HashMap::new());
}

/// The last mounted path for the focused control. Keep the walk out of the animation
/// frame handler and refresh it when the focused ID or active dialog changes.
pub(crate) fn focus_is_within(scope: AccessibilityId, focus: AccessibilityId) -> bool {
    let window = Platform::get().current_window().id;
    PATHS.with_borrow(|paths| {
        focus == scope
            || paths
                .get(&window)
                .is_some_and(|path| path.target == Some(focus) && path.ancestors.contains(&scope))
    })
}

struct WindowFocus {
    platform: PlatformWindow,
    active: MountedDialog,
    active_node: Option<NodeId>,
    dialogs: Vec<AccessibilityId>,
    origins: HashMap<AccessibilityId, FocusOrigin>,
    observed_focus: Option<AccessibilityId>,
    focus_request: FocusRequest,
}

#[derive(Default)]
struct FocusRequest {
    waiting: Option<AccessibilityId>,
}

impl FocusRequest {
    fn observe(&mut self, current: AccessibilityId, accessibility_updated: bool) {
        if accessibility_updated && self.waiting == Some(current) {
            self.waiting = None;
        }
    }

    fn next_request(
        &mut self,
        current: AccessibilityId,
        previous: Option<AccessibilityId>,
        target: AccessibilityId,
    ) -> Option<AccessibilityId> {
        let changed_request = self.waiting != Some(target);
        let supersedes = self.waiting.is_some() && changed_request;
        if current == target && !supersedes {
            return None;
        }
        self.waiting = Some(target);
        (changed_request || previous != Some(current)).then_some(target)
    }
}

#[derive(Clone, Copy)]
struct FocusOrigin {
    control: AccessibilityId,
    dialog: Option<AccessibilityId>,
}

impl FreyaPlugin for DialogFocusPlugin {
    fn plugin_id(&self) -> &'static str {
        "oneclient-dialog-focus"
    }

    fn root_component(&self, root: Element) -> Element {
        // Apply our handler directly to Freya's integration, then wrap it with
        // Borderless. Freya's plugin map has no registration order.
        self.borderless
            .root_component(install_tab_handler(root, self.tabs.clone()))
    }

    fn on_event(&mut self, event: &mut PluginEvent, handle: PluginHandle) {
        match event {
            PluginEvent::RunnerCreated { runner } => {
                runner.provide_root_context(MountedDialog::new);
            }
            PluginEvent::WindowCreated {
                window,
                runner,
                tree,
                ..
            } => {
                let id = u64::from(window.id());
                let (platform, active) = runner.run_in(|| {
                    (
                        Platform::get().current_window(),
                        consume_root_context::<MountedDialog>(),
                    )
                });
                self.windows.insert(
                    id,
                    WindowFocus {
                        platform,
                        active,
                        active_node: None,
                        dialogs: Vec::new(),
                        origins: HashMap::new(),
                        observed_focus: None,
                        focus_request: FocusRequest::default(),
                    },
                );
                self.refresh(id, tree, false);
            }
            PluginEvent::FinishedUpdatingTree { window, tree } => {
                self.refresh(u64::from(window.id()), tree, false);
            }
            PluginEvent::AfterAccessibility { window, tree, .. } => {
                self.refresh(u64::from(window.id()), tree, true);
            }
            PluginEvent::WindowClosed { window, .. } => {
                let id = u64::from(window.id());
                self.windows.remove(&id);
                self.tabs.borrow_mut().remove(&id);
                PATHS.with_borrow_mut(|paths| paths.remove(&id));
            }
            _ => {}
        }
        self.borderless.on_event(event, handle);
    }
}

fn install_tab_handler(root: Element, tabs: Rc<RefCell<HashMap<u64, Vec<bool>>>>) -> Element {
    let Element::Element {
        key,
        element,
        elements,
    } = root
    else {
        return root;
    };
    let Some(mut integration) = (element.as_ref() as &dyn Any)
        .downcast_ref::<RectElement>()
        .cloned()
    else {
        return Element::Element {
            key,
            element,
            elements,
        };
    };
    let Some(EventHandlerType::Keyboard(default_handler)) = integration
        .event_handlers
        .get(&EventName::GlobalKeyDown)
        .cloned()
    else {
        return Element::Element {
            key,
            element,
            elements,
        };
    };
    integration.event_handlers.insert(
        EventName::GlobalKeyDown,
        EventHandlerType::Keyboard(EventHandler::new(move |event: Event<KeyboardEventData>| {
            if event.key != Key::Named(NamedKey::Tab)
                || !(event.modifiers.is_empty() || event.modifiers == Modifiers::SHIFT)
            {
                default_handler.call(event);
                return;
            }

            let mut window = Platform::get().current_window();
            window
                .navigation_mode
                .set_if_modified(NavigationMode::Keyboard);
            tabs.borrow_mut()
                .entry(window.id)
                .or_default()
                .push(event.modifiers.contains(Modifiers::SHIFT));
        })),
    );
    Element::Element {
        key,
        element: Rc::new(integration),
        elements,
    }
}

impl DialogFocusPlugin {
    fn refresh(&mut self, id: u64, tree: &Tree, accessibility_updated: bool) {
        let Some(window) = self.windows.get_mut(&id) else {
            return;
        };
        let current = *window.platform.focused_accessibility_id.peek();
        window.focus_request.observe(current, accessibility_updated);
        let previous = window.observed_focus.replace(current);
        let registered = mounted_dialogs(id);
        let dialogs: Vec<_> = registered.iter().map(|(dialog, _)| *dialog).collect();
        let old_top = *window.active.active.peek();
        let registrations_changed = dialogs != window.dialogs;
        let stable_top = window.active_node.filter(|node| {
            !registrations_changed
                && old_top == dialogs.last().copied()
                && tree
                    .accessibility_state
                    .get(node)
                    .map(|state| state.a11y_id)
                    == old_top
                && visible_dialog(tree, *node)
        });
        let (active, active_node) = if let Some(node) = stable_top {
            (old_top, Some(node))
        } else {
            top_dialog(tree, &registered).map_or((None, None), |(id, node)| (Some(id), Some(node)))
        };
        let changed = registrations_changed || active != old_top;
        window.active_node = active_node;

        // The platform acknowledges focus asynchronously. Keep the latest intent
        // as the cursor so Tabs in adjacent event batches cannot start from stale focus.
        let pending = window
            .focus_request
            .waiting
            .filter(|&id| is_focus_in_scope(tree, id, active));
        let mut target = pending.unwrap_or(current);
        if changed {
            window.active.active.set_if_modified(active);
            target = reconcile_focus(window, tree, target, old_top, active, &registered);
            window.dialogs = dialogs;
        } else if pending.is_none()
            && active.is_some()
            && (previous != Some(current) || window.focus_request.waiting.is_some())
            && !is_focus_in_scope(tree, current, active)
        {
            // Native accessibility can directly focus an older dialog or the
            // retained page. Redirect it as soon as Freya reports the change.
            target = previous
                .filter(|&id| is_focus_in_scope(tree, id, active))
                .or(active)
                .unwrap_or(ACCESSIBILITY_ROOT_ID);
        }

        if let Some(tabs) = self.tabs.borrow_mut().remove(&id) {
            let order = tab_order(tree, active);
            let mut origin = tab_anchor(tree, target, &order);
            for backward in tabs {
                if let Some(next) = next_focus(&order, origin, backward) {
                    target = next;
                    origin = next;
                }
            }
        }

        PATHS.with_borrow_mut(|paths| {
            let path = paths.entry(id).or_default();
            if changed || path.target != Some(current) {
                *path = FocusPath {
                    target: Some(current),
                    ancestors: node_for(tree, current)
                        .into_iter()
                        .flat_map(|node| ancestors(tree, node))
                        .filter_map(|node| tree.accessibility_state.get(&node))
                        .map(|state| state.a11y_id)
                        .collect(),
                };
            }
        });
        if let Some(target) = window.focus_request.next_request(current, previous, target) {
            window.platform.send(UserEvent::FocusAccessibilityNode(
                AccessibilityFocusStrategy::Node(target),
            ));
        }
    }
}

fn reconcile_focus(
    window: &mut WindowFocus,
    tree: &Tree,
    current: AccessibilityId,
    old_top: Option<AccessibilityId>,
    active: Option<AccessibilityId>,
    registered: &[(AccessibilityId, AccessibilityId)],
) -> AccessibilityId {
    let closed_top = old_top.filter(|old| !registered.iter().any(|(id, _)| id == old));
    for &(dialog, captured) in registered {
        window.origins.entry(dialog).or_insert_with(|| {
            let parent = node_for(tree, captured)
                .and_then(|node| containing_dialog(tree, node))
                .or_else(|| {
                    (!is_mounted(tree, captured) && Some(dialog) == active)
                        .then_some(closed_top)
                        .flatten()
                });
            FocusOrigin {
                control: captured,
                dialog: parent,
            }
        });
    }

    let restore = closed_top.and_then(|closed| {
        find_focus_origin(&window.origins, closed, |id| {
            is_focus_in_scope(tree, id, active)
        })
    });
    collapse_closed_origins(&mut window.origins, registered);

    if active.is_some_and(|id| !window.dialogs.contains(&id)) {
        active.unwrap_or(current)
    } else if let Some(restore) = restore {
        restore
    } else if is_focus_in_scope(tree, current, active) {
        current
    } else {
        active.unwrap_or(ACCESSIBILITY_ROOT_ID)
    }
}

/// The opener can be inside a dialog that closes in the same tree update.
fn find_focus_origin(
    origins: &HashMap<AccessibilityId, FocusOrigin>,
    mut dialog: AccessibilityId,
    mut eligible: impl FnMut(AccessibilityId) -> bool,
) -> Option<AccessibilityId> {
    for _ in 0..origins.len() {
        let origin = origins.get(&dialog)?;
        if eligible(origin.control) {
            return Some(origin.control);
        }
        dialog = origin.dialog?;
    }
    None
}

fn collapse_closed_origins(
    origins: &mut HashMap<AccessibilityId, FocusOrigin>,
    registered: &[(AccessibilityId, AccessibilityId)],
) {
    let mounted = |id| registered.iter().any(|&(dialog, _)| dialog == id);
    if origins.keys().all(|&id| mounted(id)) {
        return;
    }
    let previous = origins.clone();
    origins.retain(|&id, origin| {
        if !mounted(id) {
            return false;
        }
        for _ in 0..previous.len() {
            let Some(parent) = origin.dialog.filter(|&parent| !mounted(parent)) else {
                break;
            };
            *origin = previous.get(&parent).copied().unwrap_or(FocusOrigin {
                control: ACCESSIBILITY_ROOT_ID,
                dialog: None,
            });
        }
        true
    });
}
fn node_for(tree: &Tree, id: AccessibilityId) -> Option<NodeId> {
    tree.accessibility_state
        .iter()
        .find_map(|(&node, state)| (state.a11y_id == id).then_some(node))
}

fn ancestors(tree: &Tree, node: NodeId) -> impl Iterator<Item = NodeId> + '_ {
    std::iter::successors(Some(node), |node| tree.parents.get(node).copied())
}

fn is_mounted(tree: &Tree, id: AccessibilityId) -> bool {
    id == ACCESSIBILITY_ROOT_ID || node_for(tree, id).is_some()
}

fn is_focus_in_scope(tree: &Tree, id: AccessibilityId, active: Option<AccessibilityId>) -> bool {
    if id == ACCESSIBILITY_ROOT_ID {
        return active.is_none();
    }
    let Some(node) = node_for(tree, id) else {
        return false;
    };
    tree.accessibility_state
        .get(&node)
        .is_some_and(|state| state.a11y_focusable == Focusable::Enabled)
        && tree
            .elements
            .get(&node)
            .is_some_and(|element| !element.accessibility().builder.is_disabled())
        && is_visible(tree, node)
        && containing_dialog(tree, node) == active
}

fn containing_dialog(tree: &Tree, node: NodeId) -> Option<AccessibilityId> {
    ancestors(tree, node).find_map(|node| {
        tree.elements
            .get(&node)
            .filter(|element| element.accessibility().builder.is_modal())?;
        tree.accessibility_state
            .get(&node)
            .map(|state| state.a11y_id)
    })
}

fn is_visible(tree: &Tree, node: NodeId) -> bool {
    ancestors(tree, node).all(|node| {
        tree.elements
            .get(&node)
            .is_some_and(|element| !element.accessibility().builder.is_hidden())
    })
}

fn visible_dialog(tree: &Tree, node: NodeId) -> bool {
    tree.elements
        .get(&node)
        .is_some_and(|element| element.accessibility().builder.is_modal())
        && is_visible(tree, node)
}

fn visible_tree(tree: &Tree, mut visit: impl FnMut(NodeId, Option<AccessibilityId>)) {
    let mut pending = vec![(NodeId::ROOT, None)];
    while let Some((id, parent_dialog)) = pending.pop() {
        let Some(element) = tree.elements.get(&id) else {
            continue;
        };
        let access = element.accessibility();
        if access.builder.is_hidden() {
            continue;
        }
        let modal = access.builder.is_modal();
        let scope = if modal {
            tree.accessibility_state.get(&id).map(|state| state.a11y_id)
        } else {
            parent_dialog
        };
        visit(id, scope);
        if let Some(children) = tree.children.get(&id) {
            pending.extend(children.iter().rev().map(|&id| (id, scope)));
        }
    }
}

fn top_dialog(
    tree: &Tree,
    registered: &[(AccessibilityId, AccessibilityId)],
) -> Option<(AccessibilityId, NodeId)> {
    // Registration order is the same order used to paint each dialog band.
    registered.iter().rev().find_map(|&(id, _)| {
        let node = node_for(tree, id)?;
        visible_dialog(tree, node).then_some((id, node))
    })
}

fn tab_order(tree: &Tree, active: Option<AccessibilityId>) -> Vec<AccessibilityId> {
    let mut controls = Vec::new();
    visible_tree(tree, |id, scope| {
        if scope == active
            && let Some(state) = tree.accessibility_state.get(&id)
            && state.a11y_focusable == Focusable::Enabled
            && tree
                .elements
                .get(&id)
                .is_some_and(|element| !element.accessibility().builder.is_disabled())
        {
            controls.push((state.a11y_id, state.a11y_member_of));
        }
    });
    let enabled: HashSet<_> = controls.iter().map(|&(id, _)| id).collect();
    let mut visited = HashSet::new();
    controls
        .into_iter()
        .filter_map(|(id, group)| {
            let anchor = group.filter(|group| enabled.contains(group)).unwrap_or(id);
            visited.insert(anchor).then_some(anchor)
        })
        .collect()
}

fn tab_anchor(tree: &Tree, current: AccessibilityId, order: &[AccessibilityId]) -> AccessibilityId {
    node_for(tree, current)
        .and_then(|node| tree.accessibility_state.get(&node))
        .and_then(|state| state.a11y_member_of)
        .filter(|group| order.contains(group))
        .unwrap_or(current)
}

fn next_focus(
    order: &[AccessibilityId],
    current: AccessibilityId,
    backward: bool,
) -> Option<AccessibilityId> {
    let index = order.iter().position(|&id| id == current);
    if backward {
        index
            .and_then(|index| index.checked_sub(1))
            .and_then(|index| order.get(index))
            .or_else(|| order.last())
            .copied()
    } else {
        index
            .and_then(|index| order.get(index + 1))
            .or_else(|| order.first())
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn borderless_integration_dispatches_tab_in_keyboard_mode() {
        let plugin = DialogFocusPlugin::default();
        let tabs = plugin.tabs.clone();
        let mut plugins = freya_winit::plugins::PluginsManager::default();
        plugins.add_plugin(plugin);
        plugins.add_plugin(freya::metrics::MetricsPlugin::default());
        let platform_tab_calls = Rc::new(RefCell::new(0));
        let original_calls = platform_tab_calls.clone();
        let (mut runner, ()) = freya_testing::TestingRunner::new(
            move || {
                let original_calls = original_calls.clone();
                plugins.wrap_root(
                    rect()
                        .on_global_key_down(move |event: Event<KeyboardEventData>| {
                            if event.key == Key::Named(NamedKey::Tab) {
                                *original_calls.borrow_mut() += 1;
                            }
                        })
                        .into_element(),
                )
            },
            (360., 260.).into(),
            |_| (),
            1.,
        );
        runner.poll_n(Duration::ZERO, 3);
        runner.press_key(Key::Named(NamedKey::Tab));
        runner.poll_n(Duration::ZERO, 3);
        runner.run_in(|| {
            let window = Platform::get().current_window();
            assert_eq!(tabs.borrow().get(&window.id), Some(&vec![false]));
            assert_eq!(*window.navigation_mode.peek(), NavigationMode::Keyboard);
        });
        assert_eq!(*platform_tab_calls.borrow(), 0);
    }

    #[test]
    fn tab_order_uses_only_the_top_dialog_and_skips_hidden_subtrees() {
        use freya_core::runner::Runner;

        let mut runner = Runner::new(|| {
            let item = |id| rect().a11y_id(AccessibilityId(id)).a11y_focusable(true);
            let dialog = |id, level| {
                item(id)
                    .a11y_role(AccessibilityRole::Dialog)
                    .a11y_builder(|node| node.set_modal())
                    .layer(Layer::OverlayLevel(level))
            };
            rect()
                .child(item(90))
                .child(dialog(91, 7).child(item(92)))
                .child(
                    dialog(93, 12)
                        .child(item(94))
                        .child(
                            rect()
                                .a11y_builder(|node| node.set_hidden())
                                .child(item(95)),
                        )
                        .child(item(96)),
                )
                .into_element()
        });
        let mut tree = Tree::default();
        let changes = runner.sync_and_update();
        runner.run_in(|| tree.apply_mutations(changes, 1.));

        let registered = [
            (AccessibilityId(91), ACCESSIBILITY_ROOT_ID),
            (AccessibilityId(93), AccessibilityId(92)),
        ];
        assert_eq!(
            tab_order(&tree, top_dialog(&tree, &registered).map(|(id, _)| id)),
            [
                AccessibilityId(93),
                AccessibilityId(94),
                AccessibilityId(96)
            ]
        );
    }

    #[test]
    fn focus_restores_past_a_closed_parent_to_an_eligible_opener() {
        let root_trigger = AccessibilityId(10);
        let parent = AccessibilityId(11);
        let disabled_trigger = AccessibilityId(12);
        let child = AccessibilityId(13);
        let mut origins = HashMap::from([
            (
                parent,
                FocusOrigin {
                    control: root_trigger,
                    dialog: None,
                },
            ),
            (
                child,
                FocusOrigin {
                    control: disabled_trigger,
                    dialog: Some(parent),
                },
            ),
        ]);
        assert_eq!(
            find_focus_origin(&origins, child, |id| id == root_trigger),
            Some(root_trigger)
        );
        collapse_closed_origins(&mut origins, &[(child, disabled_trigger)]);
        assert!(!origins.contains_key(&parent));
        assert_eq!(
            find_focus_origin(&origins, child, |_| true),
            Some(root_trigger)
        );
    }

    #[test]
    fn group_stays_before_its_interleaved_actions_for_either_selection() {
        use freya_core::runner::Runner;

        let first = AccessibilityId(20);
        let first_action = AccessibilityId(21);
        let second = AccessibilityId(22);
        let second_action = AccessibilityId(23);
        for selected in [first, second] {
            let mut runner = Runner::new(move || {
                let item = |id| rect().a11y_id(id).a11y_focusable(true);
                rect()
                    .child(item(first).a11y_member_of(selected))
                    .child(item(first_action))
                    .child(item(second).a11y_member_of(selected))
                    .child(item(second_action))
                    .into_element()
            });
            let mut tree = Tree::default();
            let changes = runner.sync_and_update();
            runner.run_in(|| tree.apply_mutations(changes, 1.));
            let order = tab_order(&tree, None);
            assert_eq!(order, [selected, first_action, second_action]);
            let unselected = if selected == first { second } else { first };
            let anchor = tab_anchor(&tree, unselected, &order);
            assert_eq!(next_focus(&order, anchor, false), Some(first_action));
            assert_eq!(next_focus(&order, anchor, true), Some(second_action));
        }
    }

    #[test]
    fn tab_wraps_in_both_directions_and_recovers_from_external_focus() {
        let order = [AccessibilityId(2), AccessibilityId(3), AccessibilityId(4)];
        assert_eq!(
            next_focus(&order, AccessibilityId(4), false),
            Some(AccessibilityId(2))
        );
        assert_eq!(
            next_focus(&order, AccessibilityId(2), true),
            Some(AccessibilityId(4))
        );
        assert_eq!(
            next_focus(&order, AccessibilityId(9), false),
            Some(AccessibilityId(2))
        );
        assert_eq!(
            next_focus(&order, AccessibilityId(9), true),
            Some(AccessibilityId(4))
        );
    }

    #[test]
    fn reversing_tab_replaces_an_unacknowledged_request() {
        let first = AccessibilityId(101);
        let second = AccessibilityId(102);
        let order = [first, second];
        let mut request = FocusRequest::default();

        let forward = next_focus(&order, first, false).unwrap();
        assert_eq!(
            request.next_request(first, Some(first), forward),
            Some(second)
        );
        request.observe(first, false);
        let backward = next_focus(&order, request.waiting.unwrap(), true).unwrap();
        assert_eq!(
            request.next_request(first, Some(first), backward),
            Some(first)
        );
        request.observe(first, false);
        assert_eq!(request.waiting, Some(first));

        request.observe(second, true);
        assert_eq!(
            request.next_request(second, Some(first), backward),
            Some(first)
        );
        request.observe(first, true);
        assert_eq!(request.waiting, None);
    }
}
