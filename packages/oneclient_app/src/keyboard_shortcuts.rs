//! App-wide keyboard shortcuts, read the same way in every window.

use freya::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppShortcut {
    /// Ctrl/Cmd+,
    OpenSettings,
    /// Ctrl/Cmd+F or Ctrl/Cmd+L
    FocusSearch,
}

/// `primary` is Cmd on macOS and Ctrl elsewhere, held alone. The typed
/// character decides so the shortcut follows the keyboard layout; the physical
/// key stands in only when that character is not plain ASCII (a Cyrillic
/// layout, or the control character some platforms report while Ctrl is down).
pub fn app_shortcut(
    key: &Key,
    code: Code,
    modifiers: Modifiers,
    primary: Modifiers,
) -> Option<AppShortcut> {
    let held =
        modifiers & (Modifiers::CONTROL | Modifiers::META | Modifiers::ALT | Modifiers::SHIFT);
    if held != primary {
        return None;
    }
    let typed = match key {
        Key::Character(c) => {
            let mut chars = c.chars();
            match (chars.next(), chars.next()) {
                (Some(ch), None) if ch.is_ascii_graphic() => Some(ch.to_ascii_lowercase()),
                _ => None,
            }
        }
        _ => return None,
    };
    let typed = typed.or(match code {
        Code::Comma => Some(','),
        Code::KeyF => Some('f'),
        Code::KeyL => Some('l'),
        _ => None,
    })?;
    match typed {
        ',' => Some(AppShortcut::OpenSettings),
        'f' | 'l' => Some(AppShortcut::FocusSearch),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CMD: Modifiers = Modifiers::META;
    const CTRL: Modifiers = Modifiers::CONTROL;

    fn ch(c: &str) -> Key {
        Key::Character(c.into())
    }

    fn mac(key: Key, code: Code, modifiers: Modifiers) -> Option<AppShortcut> {
        app_shortcut(&key, code, modifiers, CMD)
    }

    fn pc(key: Key, code: Code, modifiers: Modifiers) -> Option<AppShortcut> {
        app_shortcut(&key, code, modifiers, CTRL)
    }

    #[test]
    fn cmd_comma_opens_settings_on_macos() {
        assert_eq!(
            mac(ch(","), Code::Comma, CMD),
            Some(AppShortcut::OpenSettings)
        );
    }

    #[test]
    fn ctrl_comma_opens_settings_elsewhere() {
        assert_eq!(
            pc(ch(","), Code::Comma, CTRL),
            Some(AppShortcut::OpenSettings)
        );
    }

    #[test]
    fn f_and_l_focus_search() {
        for (key, code) in [("f", Code::KeyF), ("l", Code::KeyL)] {
            assert_eq!(
                mac(ch(key), code, CMD),
                Some(AppShortcut::FocusSearch),
                "cmd+{key}"
            );
            assert_eq!(
                pc(ch(key), code, CTRL),
                Some(AppShortcut::FocusSearch),
                "ctrl+{key}"
            );
        }
    }

    #[test]
    fn caps_lock_still_counts() {
        assert_eq!(
            mac(ch("F"), Code::KeyF, CMD),
            Some(AppShortcut::FocusSearch)
        );
        assert_eq!(
            pc(ch("L"), Code::KeyL, CTRL),
            Some(AppShortcut::FocusSearch)
        );
    }

    #[test]
    fn the_other_platforms_modifier_does_nothing() {
        // Ctrl+F on macOS moves the caret in a text field; Win/Super+F belongs to the OS
        assert_eq!(mac(ch("f"), Code::KeyF, CTRL), None);
        assert_eq!(mac(ch(","), Code::Comma, CTRL), None);
        assert_eq!(pc(ch("f"), Code::KeyF, CMD), None);
    }

    #[test]
    fn extra_modifiers_leave_the_key_alone() {
        for extra in [Modifiers::SHIFT, Modifiers::ALT, CTRL] {
            assert_eq!(mac(ch("f"), Code::KeyF, CMD | extra), None, "{extra:?}");
            assert_eq!(mac(ch(","), Code::Comma, CMD | extra), None, "{extra:?}");
        }
        // AltGr on Windows arrives as Ctrl+Alt
        assert_eq!(pc(ch("l"), Code::KeyL, CTRL | Modifiers::ALT), None);
        // Cmd+Shift+O stays the logs folder shortcut
        assert_eq!(mac(ch("o"), Code::KeyO, CMD | Modifiers::SHIFT), None);
    }

    #[test]
    fn bare_keys_are_typing() {
        assert_eq!(mac(ch("f"), Code::KeyF, Modifiers::empty()), None);
        assert_eq!(mac(ch(","), Code::Comma, Modifiers::SHIFT), None);
    }

    #[test]
    fn caps_and_num_lock_are_not_modifiers() {
        let locks = Modifiers::CAPS_LOCK | Modifiers::NUM_LOCK;
        assert_eq!(
            mac(ch("f"), Code::KeyF, CMD | locks),
            Some(AppShortcut::FocusSearch)
        );
    }

    #[test]
    fn other_letters_are_not_shortcuts() {
        for (key, code) in [
            ("k", Code::KeyK),
            ("p", Code::KeyP),
            (".", Code::Period),
            ("o", Code::KeyO),
        ] {
            assert_eq!(mac(ch(key), code, CMD), None, "{key}");
        }
        assert_eq!(mac(Key::Named(NamedKey::Enter), Code::Enter, CMD), None);
    }

    #[test]
    fn the_layout_decides_not_the_physical_key() {
        // AZERTY: the key that types ',' sits where QWERTY has M
        assert_eq!(
            mac(ch(","), Code::KeyM, CMD),
            Some(AppShortcut::OpenSettings)
        );
        // and QWERTY's comma position types ';' there
        assert_eq!(mac(ch(";"), Code::Comma, CMD), None);
        // Dvorak: the physical F key types 'u'
        assert_eq!(mac(ch("u"), Code::KeyF, CMD), None);
        assert_eq!(
            mac(ch("f"), Code::KeyY, CMD),
            Some(AppShortcut::FocusSearch)
        );
    }

    #[test]
    fn non_latin_layouts_fall_back_to_the_physical_key() {
        // Russian: F types 'а', L types 'д', comma types 'б'
        assert_eq!(
            mac(ch("а"), Code::KeyF, CMD),
            Some(AppShortcut::FocusSearch)
        );
        assert_eq!(
            pc(ch("д"), Code::KeyL, CTRL),
            Some(AppShortcut::FocusSearch)
        );
        assert_eq!(
            mac(ch("б"), Code::Comma, CMD),
            Some(AppShortcut::OpenSettings)
        );
        assert_eq!(mac(ch("ж"), Code::Semicolon, CMD), None);
    }

    #[test]
    fn control_characters_fall_back_to_the_physical_key() {
        // Some platforms report Ctrl+F as the control character U+0006
        assert_eq!(
            pc(ch("\u{6}"), Code::KeyF, CTRL),
            Some(AppShortcut::FocusSearch)
        );
        assert_eq!(
            pc(ch("\u{c}"), Code::KeyL, CTRL),
            Some(AppShortcut::FocusSearch)
        );
    }
}
