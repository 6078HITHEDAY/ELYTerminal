use std::ops::Range;

use alacritty_terminal::{grid::Scroll, term::TermMode};
use gpui::{
    App, Bounds, Context, EntityInputHandler, KeyBinding, KeyDownEvent, KeyUpEvent, Keystroke,
    Pixels, Point, UTF16Selection, Window, actions,
};

use super::{
    keys::{self, Action, Key, Kitty},
    view::TerminalView,
};

actions!(terminal, [SendTab, SendBackTab]);

/// The key context a terminal's root sets.
pub const CONTEXT: &str = "Terminal";

/// Tab and Shift-Tab reach the terminal before Ely moves focus with them.
pub fn bind_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", SendTab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", SendBackTab, Some(CONTEXT)),
    ]);
}

impl TerminalView {
    /// Bytes from the keyboard: back to the newest line, then to the program.
    pub(super) fn input(&mut self, bytes: Vec<u8>, cx: &mut Context<Self>) {
        self.pty.term.lock().scroll_display(Scroll::Bottom);
        self.pty.write(bytes);
        cx.notify();
    }

    pub(super) fn key_down(
        &mut self,
        event: &KeyDownEvent,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if event.prefer_character_input {
            return;
        }
        let action = if event.is_held {
            Action::Repeat
        } else {
            Action::Press
        };
        if self.send(&event.keystroke, action, cx) {
            cx.stop_propagation();
        }
    }

    pub(super) fn key_up(&mut self, event: &KeyUpEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.send(&event.keystroke, Action::Release, cx) {
            cx.stop_propagation();
        }
    }

    pub(super) fn tab(&mut self, shift: bool, cx: &mut Context<Self>) {
        let key = Key {
            name: "tab",
            text: if shift { "" } else { "\t" },
            shift,
            ctrl: false,
            alt: false,
            action: Action::Press,
        };
        self.encode(&key, cx);
    }

    /// Sends what `keystroke` means to the program; false when it is text the input method types.
    fn send(&mut self, keystroke: &Keystroke, action: Action, cx: &mut Context<Self>) -> bool {
        let modifiers = keystroke.modifiers;
        if modifiers.platform {
            return false;
        }
        let alt = modifiers.alt && self.alt_is_meta(cx);
        let text = match &keystroke.key_char {
            Some(_) if alt && cfg!(target_os = "macos") => "",
            Some(text) => text.as_str(),
            None => "",
        };
        let key = Key {
            name: &keystroke.key,
            text,
            shift: modifiers.shift,
            ctrl: modifiers.control,
            alt,
            action,
        };
        self.encode(&key, cx)
    }

    fn encode(&mut self, key: &Key, cx: &mut Context<Self>) -> bool {
        let mode = *self.pty.term.lock().mode();
        let kitty = Kitty {
            disambiguate: mode.contains(TermMode::DISAMBIGUATE_ESC_CODES),
            events: mode.contains(TermMode::REPORT_EVENT_TYPES),
            alternates: mode.contains(TermMode::REPORT_ALTERNATE_KEYS),
            all: mode.contains(TermMode::REPORT_ALL_KEYS_AS_ESC),
            text: mode.contains(TermMode::REPORT_ASSOCIATED_TEXT),
        };
        let Some(bytes) = keys::encode(key, mode.contains(TermMode::APP_CURSOR), kitty) else {
            return false;
        };
        log::trace!("terminal {}: {key:?} sends {bytes:?}", self.pty.pid);
        self.input(bytes, cx);
        true
    }

    /// Whether a held Alt is a modifier; on macOS only the Option keys the settings name.
    #[cfg(target_os = "macos")]
    fn alt_is_meta(&self, cx: &App) -> bool {
        use crate::settings::{OptionAsAlt, SettingsStore};
        let sides = || match super::mac::option_sides() {
            Ok(sides) => Some(sides),
            Err(error) => {
                log::error!("terminal {}: {error:#}; Option composes", self.pty.pid);
                None
            }
        };
        match SettingsStore::get(cx).option_as_alt {
            OptionAsAlt::Off => false,
            OptionAsAlt::Both => true,
            OptionAsAlt::Left => sides().is_some_and(|(left, _)| left),
            OptionAsAlt::Right => sides().is_some_and(|(_, right)| right),
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn alt_is_meta(&self, _: &App) -> bool {
        true
    }
}

impl EntityInputHandler for TerminalView {
    fn text_for_range(
        &mut self,
        _: Range<usize>,
        _: &mut Option<Range<usize>>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<String> {
        None
    }

    fn selected_text_range(
        &mut self,
        _: bool,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let end = self
            .marked
            .as_ref()
            .map_or(0, |marked| marked.encode_utf16().count());
        Some(UTF16Selection {
            range: end..end,
            reversed: false,
        })
    }

    fn marked_text_range(&self, _: &mut Window, _: &mut Context<Self>) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|marked| 0..marked.encode_utf16().count())
    }

    fn unmark_text(&mut self, _: &mut Window, cx: &mut Context<Self>) {
        log::debug!("terminal {}: composing ends", self.pty.pid);
        self.marked = None;
        cx.notify();
    }

    fn replace_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.marked = None;
        if !text.is_empty() {
            log::trace!("terminal {}: typed {text:?}", self.pty.pid);
            self.input(text.as_bytes().to_vec(), cx);
        }
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _: Option<Range<usize>>,
        text: &str,
        _: Option<Range<usize>>,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::debug!("terminal {}: composing {text:?}", self.pty.pid);
        self.marked = (!text.is_empty()).then(|| text.to_string());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _: Range<usize>,
        _: Bounds<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        Some(self.caret)
    }

    fn character_index_for_point(
        &mut self,
        _: Point<Pixels>,
        _: &mut Window,
        _: &mut Context<Self>,
    ) -> Option<usize> {
        None
    }
}
