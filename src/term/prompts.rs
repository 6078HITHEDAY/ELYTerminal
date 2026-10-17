use alacritty_terminal::{
    grid::{Dimensions, Row, Scroll},
    index::Line,
    term::{Term, cell::Cell},
};
use gpui::Context;

use super::{tap::MOST, view::TerminalView};

/// The target of the OSC 8 links prompts become, each named by its prompt's number; no program names a link so.
pub(crate) const LINK: &str = "\u{7f}";

/// What ends a prompt's link.
const CLOSE: &[u8] = b"\x1b]8;;\x07";

const ESC: u8 = 0x1b;

/// Controls vte skips inside an OSC.
fn ignored(byte: u8) -> bool {
    matches!(byte, 0x00..=0x06 | 0x08..=0x17 | 0x19 | 0x1c..=0x1f)
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    /// After an ESC, held until the next byte says whether an OSC starts.
    Escape,
    /// An OSC held whole, since it may be a mark or a link to rewrite.
    Held,
    /// An OSC passed on as it comes.
    Passing,
}

/// Rewrites what goes to the parser so it stamps each prompt's cells with a link of its own, numbered per prompt.
#[derive(Default)]
pub(crate) struct Prompts {
    state: State,
    /// The escape or OSC held, from its ESC.
    held: Vec<u8>,
    /// The OSC's body as vte reads it, the controls it skips left out.
    body: Vec<u8>,
    /// The body passed the scanner's bound, so it counts for nothing.
    overflow: bool,
    /// The prompt open now, from its start mark to its end.
    prompt: Option<u32>,
    /// A program's own link is open.
    user: bool,
    /// The last prompt's number.
    count: u32,
    /// What goes to the parser from the last read.
    out: Vec<u8>,
}

impl Prompts {
    /// The bytes held from earlier reads.
    pub fn held(&self) -> usize {
        self.held.len()
    }

    /// What goes to the parser for `read`: marks become links, and a held start waits for its end.
    pub fn pass(&mut self, read: &[u8]) -> &[u8] {
        self.out.clear();
        for &byte in read {
            self.step(byte);
        }
        &self.out
    }

    fn step(&mut self, byte: u8) {
        self.state = match (self.state, byte) {
            (State::Ground, ESC) => {
                self.held.push(ESC);
                State::Escape
            }
            (State::Ground, _) => {
                self.out.push(byte);
                State::Ground
            }
            (State::Escape, b']') => {
                // Controls vte ran inside the escape go on first, as it ran them.
                self.out.extend(self.held.drain(1..));
                self.held.push(byte);
                self.body.clear();
                self.overflow = false;
                State::Held
            }
            (State::Escape, ESC) => {
                self.release();
                self.held.push(ESC);
                State::Escape
            }
            // vte runs these and stays in the escape.
            (State::Escape, _) if byte == 0x07 || ignored(byte) => {
                self.held.push(byte);
                State::Escape
            }
            (State::Escape, _) => {
                // A full reset clears the link the parser writes with.
                if byte == b'c' {
                    (self.prompt, self.user) = (None, false);
                }
                self.held.push(byte);
                self.release();
                State::Ground
            }
            (State::Held | State::Passing, 0x07 | 0x18 | 0x1a) => {
                self.end(Some(byte));
                State::Ground
            }
            (State::Held | State::Passing, ESC) => {
                self.end(None);
                self.held.push(ESC);
                State::Escape
            }
            (State::Held, _) => {
                self.held.push(byte);
                self.read(byte);
                if self.wanted() {
                    State::Held
                } else {
                    self.release();
                    State::Passing
                }
            }
            (State::Passing, _) => {
                self.out.push(byte);
                self.read(byte);
                State::Passing
            }
        };
    }

    /// Adds `byte` to the body as the tap's scanner does; skipped controls bound the bytes held too.
    fn read(&mut self, byte: u8) {
        if self.held.len() > 2 * MOST {
            self.overflow = true;
        }
        if ignored(byte) {
            return;
        }
        if self.body.len() < MOST {
            self.body.push(byte);
        } else {
            self.overflow = true;
        }
    }

    /// Whether the OSC held may be a mark, or a link that ends inside a prompt, within the scanner's bounds.
    fn wanted(&self) -> bool {
        let fits = |prefix: &[u8]| {
            let length = self.body.len().min(prefix.len());
            self.body[..length] == prefix[..length]
        };
        !self.overflow && (fits(b"133;") || (self.prompt.is_some() && fits(b"8;")))
    }

    fn release(&mut self) {
        self.out.append(&mut self.held);
    }

    /// The OSC ends at `end`, BEL, CAN or SUB, or at an ESC when none.
    fn end(&mut self, end: Option<u8>) {
        let rewrite = if self.overflow {
            None
        } else if let Some(opens) = self.body.strip_prefix(b"133;").and_then(mark) {
            if opens {
                self.count = self.count.wrapping_add(1);
                self.prompt = Some(self.count);
                (!self.user).then(|| link(self.count))
            } else {
                let closes = self.prompt.take().is_some() && !self.user;
                closes.then(|| CLOSE.to_vec())
            }
        } else if let Some(opens) = user(&self.body) {
            self.user = opens;
            // The program's link ends inside a prompt: the prompt's link takes up again.
            self.prompt.filter(|_| !opens).map(link)
        } else {
            None
        };
        match (rewrite, self.state) {
            (Some(bytes), State::Held) => {
                self.held.clear();
                self.out.extend_from_slice(&bytes);
                // CAN and SUB end the OSC and run as well.
                self.out.extend(end.filter(|&byte| byte != 0x07));
            }
            (_, State::Held) => {
                self.held.extend(end);
                self.release();
            }
            _ => self.out.extend(end),
        }
    }
}

/// Whether a mark's body opens a prompt or ends one, as `marks::read` reads its letter; none for neither.
fn mark(body: &[u8]) -> Option<bool> {
    let (&letter, rest) = body.split_first()?;
    if !rest.is_empty() && rest[0] != b';' {
        return None;
    }
    match letter {
        b'A' | b'N' => Some(true),
        b'B' | b'I' | b'C' | b'D' => Some(false),
        _ => None,
    }
}

/// Whether an OSC 8 body opens a program's link, as alacritty reads its target; none for no link at all.
fn user(body: &[u8]) -> Option<bool> {
    let rest = body.strip_prefix(b"8;")?;
    let target = &rest[rest.iter().position(|&byte| byte == b';')? + 1..];
    // alacritty joins the target's parts with `;`, each one empty when it is not UTF-8.
    Some(
        target.contains(&b';')
            || std::str::from_utf8(target).is_ok_and(|target| !target.is_empty()),
    )
}

/// The OSC 8 link that marks prompt `number`'s cells.
fn link(number: u32) -> Vec<u8> {
    format!("\x1b]8;id={number};{LINK}\x07").into_bytes()
}

/// The number of the prompt `row` shows, as its link names it.
fn prompt(row: &Row<Cell>) -> Option<u32> {
    row.into_iter().find_map(|cell| {
        let link = cell.hyperlink().filter(|link| link.uri() == LINK)?;
        link.id().parse().ok()
    })
}

/// The display offset that puts the prompt before or after the view's top at the top, as Ghostty jumps.
pub fn jump<T>(term: &Term<T>, up: bool) -> Option<usize> {
    let top = -(term.grid().display_offset() as i32);
    let (first, last) = (term.topmost_line().0, term.bottommost_line().0);
    let number = |line: i32| prompt(&term.grid()[Line(line)]);
    // ponytail: a scan of the rows on the way; index prompt rows if long histories lag.
    let found = if up {
        // The nearest prompt above the view's top, then its first row, blank rows within it passed.
        let (line, own) = (first..top)
            .rev()
            .find_map(|line| number(line).map(|own| (line, own)))?;
        let mut start = line;
        for above in (first..line).rev() {
            match number(above) {
                Some(other) if other != own => break,
                Some(_) => start = above,
                None => {}
            }
        }
        start
    } else {
        // The prompt at or above the view's top may go on below it; the next one starts.
        let here = (first..=top).rev().find_map(number);
        (top + 1..=last).find(|&line| number(line).is_some_and(|other| Some(other) != here))?
    };
    // A prompt on the screen means the bottom, which the view cannot pass.
    Some((-found).max(0) as usize)
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
