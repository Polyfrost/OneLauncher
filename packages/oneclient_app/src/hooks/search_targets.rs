use std::cell::RefCell;
use std::rc::Rc;

use freya::prelude::*;

use super::{OverlayClaims, use_overlay_claims};

/// The search fields on screen, so Ctrl/Cmd+F and Ctrl/Cmd+L can focus one from
/// anywhere. The newest wins: a page's search mounts after the shell's, a
/// popup's after the page's.
#[derive(Clone, Default)]
pub struct SearchTargets(Rc<RefCell<Targets>>);

#[derive(Default)]
struct Targets {
    next: u64,
    entries: Vec<Target>,
}

struct Target {
    token: u64,
    id: AccessibilityId,
    /// Overlays already open when the field mounted. A field under a popup
    /// opened later is covered, so the shortcut skips it.
    depth: usize,
    active: bool,
}

impl SearchTargets {
    fn register(&self, id: AccessibilityId, depth: usize) -> u64 {
        let mut targets = self.0.borrow_mut();
        targets.next += 1;
        let token = targets.next;
        targets.entries.push(Target {
            token,
            id,
            depth,
            active: true,
        });
        token
    }

    fn set_active(&self, token: u64, active: bool) {
        if let Some(target) = self
            .0
            .borrow_mut()
            .entries
            .iter_mut()
            .find(|target| target.token == token)
        {
            target.active = active;
        }
    }

    fn unregister(&self, token: u64) {
        self.0
            .borrow_mut()
            .entries
            .retain(|target| target.token != token);
    }

    /// Focuses the field on top; false when no field on screen can take it.
    pub fn focus(&self, claims: &OverlayClaims) -> bool {
        let Some(id) = self.pick(claims.depth()) else {
            return false;
        };
        id.request_focus();
        true
    }

    /// The field the shortcut should focus while `open_overlays` overlays are open.
    pub fn pick(&self, open_overlays: usize) -> Option<AccessibilityId> {
        self.0
            .borrow()
            .entries
            .iter()
            .filter(|target| target.active && target.depth >= open_overlays)
            .max_by_key(|target| target.token)
            .map(|target| target.id)
    }
}

pub fn use_provide_search_targets() {
    use_provide_root_context(SearchTargets::default);
}

/// Falls back to a detached list outside `RootLayout`, where nothing reads it.
pub fn use_search_targets() -> SearchTargets {
    try_consume_root_context::<SearchTargets>().unwrap_or_default()
}

/// Offers the field `id` to the search shortcut while it is mounted and `active`.
pub fn use_search_target(id: AccessibilityId, active: bool) {
    let targets = use_hook(use_search_targets);
    let token = use_hook({
        let targets = targets.clone();
        move || targets.register(id, use_overlay_claims().depth())
    });
    targets.set_active(token, active);
    use_drop(move || targets.unregister(token));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(n: u64) -> Vec<AccessibilityId> {
        (1000..1000 + n).map(AccessibilityId).collect()
    }

    #[test]
    fn nothing_registered_picks_nothing() {
        assert_eq!(SearchTargets::default().pick(0), None);
    }

    #[test]
    fn the_newest_field_wins() {
        let targets = SearchTargets::default();
        let id = ids(2);
        targets.register(id[0], 0);
        targets.register(id[1], 0);
        assert_eq!(targets.pick(0), Some(id[1]));
    }

    #[test]
    fn an_unmounted_field_hands_back_to_the_one_below() {
        let targets = SearchTargets::default();
        let id = ids(2);
        targets.register(id[0], 0);
        let top = targets.register(id[1], 0);
        targets.unregister(top);
        assert_eq!(targets.pick(0), Some(id[0]));
        targets.unregister(top);
        assert_eq!(targets.pick(0), Some(id[0]));
    }

    #[test]
    fn a_disabled_field_is_skipped_until_enabled() {
        let targets = SearchTargets::default();
        let id = ids(2);
        targets.register(id[0], 0);
        let logs = targets.register(id[1], 0);
        targets.set_active(logs, false);
        assert_eq!(targets.pick(0), Some(id[0]));
        targets.set_active(logs, true);
        assert_eq!(targets.pick(0), Some(id[1]));
    }

    #[test]
    fn a_popup_without_search_covers_the_page_search() {
        let targets = SearchTargets::default();
        let id = ids(1);
        targets.register(id[0], 0);
        assert_eq!(targets.pick(1), None);
        assert_eq!(targets.pick(0), Some(id[0]));
    }

    #[test]
    fn a_popup_search_wins_and_hands_back_when_it_closes() {
        let targets = SearchTargets::default();
        let id = ids(2);
        targets.register(id[0], 0);
        let popup = targets.register(id[1], 1);
        assert_eq!(targets.pick(1), Some(id[1]));
        targets.unregister(popup);
        assert_eq!(targets.pick(0), Some(id[0]));
    }

    #[test]
    fn a_search_opened_inside_a_popup_stays_reachable_under_a_nested_one_with_search() {
        let targets = SearchTargets::default();
        let id = ids(3);
        targets.register(id[0], 0);
        targets.register(id[1], 1);
        targets.register(id[2], 2);
        assert_eq!(targets.pick(2), Some(id[2]));
        assert_eq!(targets.pick(3), None);
    }
}

#[cfg(test)]
mod ui_tests {
    use freya_testing::prelude::*;

    use super::*;
    use crate::components::TextInput;
    use crate::hooks::{use_overlay_claim, use_provide_overlay_claims};
    use crate::keyboard_shortcuts::{AppShortcut, app_shortcut};

    const SHELL: AccessibilityId = AccessibilityId(900_001);
    const PAGE: AccessibilityId = AccessibilityId(900_002);
    const POPUP: AccessibilityId = AccessibilityId(900_003);
    const OTHER: AccessibilityId = AccessibilityId(900_004);

    #[derive(Clone, Copy, PartialEq)]
    enum Popup {
        Closed,
        Plain,
        WithSearch,
    }

    #[derive(Clone, Copy)]
    struct Scene {
        page: State<bool>,
        page_enabled: State<bool>,
        popup: State<Popup>,
        settings_opened: State<u32>,
    }

    fn field(id: AccessibilityId, search: bool) -> TextInput {
        let input = TextInput::new(use_state(String::new)).a11y_id(id);
        if search { input.search() } else { input }
    }

    #[derive(PartialEq)]
    struct Page;
    impl Component for Page {
        fn render(&self) -> impl IntoElement {
            let scene = use_consume::<Scene>();
            rect()
                .child(field(PAGE, true).enabled(*scene.page_enabled.read()))
                .child(field(OTHER, false))
        }
    }

    #[derive(PartialEq)]
    struct PopupView(bool);
    impl Component for PopupView {
        fn render(&self) -> impl IntoElement {
            use_overlay_claim();
            rect().maybe_child(self.0.then(|| field(POPUP, true)))
        }
    }

    fn app() -> impl IntoElement {
        use_provide_overlay_claims();
        use_provide_search_targets();
        let scene = use_consume::<Scene>();
        let targets = use_search_targets();
        let claims = use_overlay_claims();
        let popup = *scene.popup.read();
        let mut settings_opened = scene.settings_opened;
        rect()
            .on_global_key_down(move |e: Event<KeyboardEventData>| {
                match app_shortcut(&e.key, e.code, e.modifiers, Modifiers::ctrl_or_meta()) {
                    Some(AppShortcut::FocusSearch) => {
                        if targets.focus(&claims) {
                            e.prevent_default();
                        }
                    }
                    Some(AppShortcut::OpenSettings) => *settings_opened.write() += 1,
                    None => {}
                }
            })
            .child(field(SHELL, true))
            .maybe_child(scene.page.read().then(|| Page.into_element()))
            .maybe_child(match popup {
                Popup::Closed => None,
                Popup::Plain => Some(PopupView(false).into_element()),
                Popup::WithSearch => Some(PopupView(true).into_element()),
            })
    }

    fn launch() -> (TestingRunner, Scene) {
        let (mut test, scene) = TestingRunner::new(
            app,
            Size2D::new(400., 400.),
            |runner| {
                runner.provide_root_context(|| Scene {
                    page: State::create(true),
                    page_enabled: State::create(true),
                    popup: State::create(Popup::Closed),
                    settings_opened: State::create(0),
                })
            },
            1.,
        );
        test.sync_and_update();
        (test, scene)
    }

    fn chord(test: &mut TestingRunner, key: &str, code: Code) {
        test.send_event(PlatformEvent::Keyboard {
            name: KeyboardEventName::KeyDown,
            key: Key::Character(key.into()),
            code,
            modifiers: Modifiers::ctrl_or_meta(),
        });
        settle(test);
    }

    /// A focus request lands on the sync after the one that ran the handler.
    fn settle(test: &mut TestingRunner) {
        test.sync_and_update();
        test.sync_and_update();
    }

    fn focused(test: &TestingRunner) -> AccessibilityId {
        test.run_in(|| {
            *Platform::get()
                .current_window()
                .focused_accessibility_id
                .peek()
        })
    }

    fn set<T: 'static>(test: &mut TestingRunner, mut state: State<T>, value: T) {
        test.run_in(move || state.set(value));
        settle(test);
    }

    #[test]
    fn cmd_f_focuses_the_page_search_over_the_shell_search() {
        let (mut test, _) = launch();
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), PAGE);
    }

    #[test]
    fn cmd_l_does_the_same() {
        let (mut test, _) = launch();
        chord(&mut test, "l", Code::KeyL);
        assert_eq!(focused(&test), PAGE);
    }

    #[test]
    fn leaving_the_page_hands_the_shortcut_to_the_shell() {
        let (mut test, scene) = launch();
        set(&mut test, scene.page, false);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), SHELL);
        set(&mut test, scene.page, true);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), PAGE);
    }

    #[test]
    fn a_disabled_page_search_is_skipped() {
        let (mut test, scene) = launch();
        set(&mut test, scene.page_enabled, false);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), SHELL);
        set(&mut test, scene.page_enabled, true);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), PAGE);
    }

    #[test]
    fn a_popup_without_search_keeps_focus_off_the_page_behind_it() {
        let (mut test, scene) = launch();
        set(&mut test, scene.popup, Popup::Plain);
        let before = focused(&test);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), before);
        assert_ne!(focused(&test), PAGE);
    }

    #[test]
    fn a_popup_search_wins_then_hands_back_when_it_closes() {
        let (mut test, scene) = launch();
        set(&mut test, scene.popup, Popup::WithSearch);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), POPUP);
        set(&mut test, scene.popup, Popup::Closed);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), PAGE);
    }

    #[test]
    fn the_shortcut_moves_focus_out_of_another_field_without_typing() {
        let (mut test, _) = launch();
        test.run_in(|| OTHER.request_focus());
        settle(&mut test);
        assert_eq!(focused(&test), OTHER);
        chord(&mut test, "f", Code::KeyF);
        assert_eq!(focused(&test), PAGE);
        test.write_text("x");
        let typed = test.find(|_, element| {
            Paragraph::try_downcast(element).and_then(|p| {
                let text: String = p.spans.iter().map(|s| s.text.to_string()).collect();
                (!text.is_empty()).then_some(text)
            })
        });
        assert_eq!(typed.as_deref(), Some("x"));
    }

    #[test]
    fn the_shortcut_works_again_from_the_search_it_focused() {
        let (mut test, scene) = launch();
        chord(&mut test, "f", Code::KeyF);
        set(&mut test, scene.page, false);
        chord(&mut test, "l", Code::KeyL);
        assert_eq!(focused(&test), SHELL);
    }

    fn typed(test: &TestingRunner) -> String {
        test.find_many(|_, element| {
            Paragraph::try_downcast(element).map(|p| {
                p.spans
                    .iter()
                    .map(|s| s.text.to_string())
                    .collect::<String>()
            })
        })
        .into_iter()
        .collect()
    }

    #[test]
    fn cmd_comma_reaches_the_app_while_typing_and_types_nothing() {
        let (mut test, scene) = launch();
        test.run_in(|| OTHER.request_focus());
        settle(&mut test);
        chord(&mut test, ",", Code::Comma);
        assert_eq!(test.run_in(|| *scene.settings_opened.peek()), 1);
        assert_eq!(typed(&test), "");
    }

    #[test]
    fn other_editing_chords_still_reach_the_field() {
        let (mut test, scene) = launch();
        test.run_in(|| OTHER.request_focus());
        settle(&mut test);
        test.write_text("ab");
        // Cmd/Ctrl+A selects, then typing replaces the selection
        chord(&mut test, "a", Code::KeyA);
        test.write_text("z");
        assert_eq!(typed(&test), "z");
        assert_eq!(focused(&test), OTHER);
        assert_eq!(test.run_in(|| *scene.settings_opened.peek()), 0);
    }

    #[test]
    fn plain_f_is_typing_not_the_shortcut() {
        let (mut test, _) = launch();
        test.run_in(|| OTHER.request_focus());
        settle(&mut test);
        test.write_text("f");
        assert_eq!(focused(&test), OTHER);
    }
}
