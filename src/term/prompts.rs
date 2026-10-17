use alacritty_terminal::{
    grid::{Dimensions, Scroll},
    index::Line,
    term::{
        Term,
        cell::{Cell, Hyperlink},
    },
};
use gpui::Context;

use super::view::TerminalView;

/// The target of the OSC 8 link each prompt mark becomes; no program names a link so.
pub(crate) const LINK: &str = "\u{7f}";

/// The most bytes a mark's start may span, ignored controls in it too.
const MOST: usize = 64;

const ESC: u8 = 0x1b;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    /// An ESC at this index, which may start a mark.
    Escape(usize),
    /// Inside `ESC ]` from this index, with this much of `133;` matched.
    Body(usize, usize),
    /// Past a rewritten mark's letter, its options blanked to the end.
    Options,
}

/// Turns each OSC 133 mark into an OSC 8 link as it passes to the parser, so the parser marks the cells each prompt prints.
#[derive(Default)]
pub(crate) struct Prompts {
    state: State,
    /// A mark's start, held from the parser until its letter shows.
    held: Vec<u8>,
}

/// Controls vte skips inside a sequence.
fn ignored(byte: u8) -> bool {
    matches!(byte, 0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f)
}

impl Prompts {
    /// The bytes held from the last read, which go before the next.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// Rewrites the marks in `buf[..held + read]`, the held bytes put first; returns how many bytes go on.
    pub fn pass(&mut self, buf: &mut [u8], read: usize) -> usize {
        let from = self.held.len();
        buf[..from].copy_from_slice(&self.held);
        self.held.clear();
        let end = from + read;
        for at in from..end {
            let byte = buf[at];
            self.state = match self.state {
                State::Ground if byte == ESC => State::Escape(at),
                State::Ground => State::Ground,
                State::Escape(start) => match byte {
                    b']' => State::Body(start, 0),
                    ESC => State::Escape(at),
                    // vte runs these and stays in the escape.
                    byte if byte == 0x07 || ignored(byte) => State::Escape(start),
                    _ => State::Ground,
                },
                State::Body(start, seen) => match byte {
                    ESC => State::Escape(at),
                    0x07 | 0x18 | 0x1a => State::Ground,
                    byte if ignored(byte) => State::Body(start, seen),
                    byte if seen < 4 && byte == b"133;"[seen] => State::Body(start, seen + 1),
                    byte if seen == 4 => match byte {
                        b'A' | b'N' => link(&mut buf[start..=at], true),
                        b'B' | b'I' | b'C' | b'D' => link(&mut buf[start..=at], false),
                        _ => State::Ground,
                    },
                    _ => State::Ground,
                },
                State::Options => match byte {
                    ESC => State::Escape(at),
                    0x07 | 0x18 | 0x1a => State::Ground,
                    byte if ignored(byte) => State::Options,
                    _ => {
                        buf[at] = 0;
                        State::Options
                    }
                },
            };
        }
        match self.state {
            State::Escape(start) | State::Body(start, _) if end - start <= MOST => {
                self.held.extend_from_slice(&buf[start..end]);
                self.state = match self.state {
                    State::Body(_, seen) => State::Body(0, seen),
                    _ => State::Escape(0),
                };
                start
            }
            State::Escape(_) | State::Body(..) => {
                log::warn!("pty: a mark's start over {MOST} bytes passes as it came");
                self.state = State::Ground;
                end
            }
            State::Ground | State::Options => end,
        }
    }
}

/// Makes `ESC ] 133;X` an OSC 8 link of the terminal's own: opened at a prompt's start, else closed.
fn link(span: &mut [u8], open: bool) -> State {
    let bytes = [b'8', b';', b';', if open { 0x7f } else { 0 }, 0];
    let body = span
        .iter()
        .position(|&byte| byte == b']')
        .expect("a mark holds its ]")
        + 1;
    let mut next = bytes.into_iter();
    for byte in span[body..].iter_mut().filter(|byte| !ignored(**byte)) {
        *byte = next.next().expect("a mark's five bytes");
    }
    State::Options
}

/// The prompt link a cell carries.
fn prompt_link(cell: &Cell) -> Option<Hyperlink> {
    cell.hyperlink().filter(|link| link.uri() == LINK)
}

/// The prompt `line` shows.
fn prompt<T>(term: &Term<T>, line: Line) -> Option<Hyperlink> {
    term.grid()[line].into_iter().find_map(prompt_link)
}

/// Whether `line` starts a prompt: it shows one the line above does not.
fn starts<T>(term: &Term<T>, line: Line) -> bool {
    prompt(term, line).is_some_and(|link| {
        line == term.topmost_line() || prompt(term, line - 1).as_ref() != Some(&link)
    })
}

/// The display offset that puts the prompt before or after the view's top at the top, as Ghostty jumps.
pub fn jump<T>(term: &Term<T>, up: bool) -> Option<usize> {
    let top = Line(-(term.grid().display_offset() as i32));
    // ponytail: a scan of every row on the way; index prompt rows if long histories lag.
    let found = if up {
        let mut lines = (term.topmost_line().0..top.0).rev().map(Line);
        lines.find(|&line| starts(term, line))
    } else {
        let mut lines = (top.0 + 1..=term.bottommost_line().0).map(Line);
        lines.find(|&line| starts(term, line))
    }?;
    // A prompt on the screen means the bottom, which the view cannot pass.
    Some((-found.0).max(0) as usize)
}

impl TerminalView {
    /// Scrolls to the prompt before or after the view's top; with none there, nothing moves.
    pub(super) fn scroll_to_prompt(&mut self, up: bool, cx: &mut Context<Self>) {
        let mut term = self.pty.term.lock();
        let Some(offset) = jump(&term, up) else {
            log::debug!(
                "terminal {}: no prompt {}",
                self.pty.pid,
                if up { "above" } else { "below" }
            );
            return;
        };
        let delta = offset as i32 - term.grid().display_offset() as i32;
        term.scroll_display(Scroll::Delta(delta));
        drop(term);
        log::debug!("terminal {}: to the prompt {offset} lines up", self.pty.pid);
        cx.notify();
    }
}

#[cfg(test)]
mod tests;
