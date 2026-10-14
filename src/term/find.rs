use std::ops::Range;

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line, Point},
    term::{
        Term,
        cell::{Cell, Flags},
        search::Match,
    },
};
use ely_gpui_component::{
    editor::{FindOptions, FindWidget, find_all},
    forms::{InputEvent, TextInput},
};
use gpui::{
    AppContext, Context, Entity, Focusable, Hsla, InteractiveElement, IntoElement, ParentElement,
    SharedString, Subscription, Window, div,
};

use super::{
    input::{FIND, FindNewer},
    view::TerminalView,
};

/// An open find: its query and options, the matches oldest first, the one stepped to.
pub struct Finder {
    query: Entity<TextInput>,
    options: FindOptions,
    found: Found,
    current: Option<usize>,
    error: Option<SharedString>,
    _input: Subscription,
}

impl Finder {
    /// The matches on screen, each with its wash; the current one's is stronger.
    pub fn lit<T>(&self, term: &Term<T>, color: Hsla) -> Vec<(Match, Hsla)> {
        let offset = term.grid().display_offset() as i32;
        let (top, bottom) = (-offset, term.screen_lines() as i32 - 1 - offset);
        self.found
            .hits
            .iter()
            .enumerate()
            .filter(|(_, hit)| hit.end().line.0 >= top && hit.start().line.0 <= bottom)
            .map(|(at, hit)| {
                let wash = if Some(at) == self.current { 0.55 } else { 0.25 };
                (hit.clone(), color.opacity(wash))
            })
            .collect()
    }
}

/// One search of the grid: its shown text, and each match as cells and as bytes of that text.
#[derive(Default)]
struct Found {
    text: String,
    hits: Vec<Match>,
    bytes: Vec<Range<usize>>,
    /// The grid's history then, to follow lines that scrolled since.
    history: usize,
}

const SPACERS: Flags = Flags::WIDE_CHAR_SPACER.union(Flags::LEADING_WIDE_CHAR_SPACER);

/// What a cell shows: its character and marks; a hidden cell shows a space.
fn shown(cell: &Cell) -> impl Iterator<Item = char> + '_ {
    let hidden = cell.flags.contains(Flags::HIDDEN);
    let marks = cell.zerowidth().filter(|_| !hidden);
    std::iter::once(if hidden { ' ' } else { cell.c }).chain(marks.into_iter().flatten().copied())
}

/// The grid's shown text, oldest row first, lines ended by newlines; and each row's first byte.
fn grid_text<T>(term: &Term<T>) -> (String, Vec<(usize, Line)>) {
    let (mut text, mut rows) = (String::new(), Vec::new());
    for line in term.topmost_line().0..=term.bottommost_line().0 {
        let row = &term.grid()[Line(line)];
        let cells = (0..term.columns()).map(|column| &row[Column(column)]);
        let wraps = row[term.last_column()].flags.contains(Flags::WRAPLINE);
        let end = if wraps {
            term.columns()
        } else {
            let blank = |cell: &Cell| shown(cell).eq([' ']);
            cells
                .clone()
                .rposition(|cell| !blank(cell))
                .map_or(0, |at| at + 1)
        };
        rows.push((text.len(), Line(line)));
        for cell in cells
            .take(end)
            .filter(|cell| !cell.flags.intersects(SPACERS))
        {
            text.extend(shown(cell));
        }
        if !wraps {
            text.push('\n');
        }
    }
    (text, rows)
}

/// The cell showing byte `offset` of the grid's text; a line's end maps to the cell after its text.
fn cell_at<T>(term: &Term<T>, rows: &[(usize, Line)], offset: usize) -> Point {
    let (mut end, line) = rows[rows.partition_point(|(start, _)| *start <= offset) - 1];
    let row = &term.grid()[line];
    for column in 0..term.columns() {
        let cell = &row[Column(column)];
        if !cell.flags.intersects(SPACERS) {
            end += shown(cell).map(char::len_utf8).sum::<usize>();
        }
        if offset < end {
            return Point::new(line, Column(column));
        }
    }
    Point::new(line, term.last_column())
}

impl Found {
    /// Every match of `query`, oldest first; a pattern that does not compile is an error.
    fn search<T>(term: &Term<T>, query: &str, options: FindOptions) -> Result<Self, String> {
        let history = term.grid().history_size();
        if query.is_empty() {
            return Ok(Self {
                history,
                ..Self::default()
            });
        }
        let (text, rows) = grid_text(term);
        let bytes = find_all(&text, query, options)?;
        let hits = bytes
            .iter()
            .map(|range| cell_at(term, &rows, range.start)..=cell_at(term, &rows, range.end - 1))
            .collect();
        Ok(Self {
            text,
            hits,
            bytes,
            history,
        })
    }

    /// The bytes of the line holding byte `offset`.
    fn line(&self, offset: usize) -> Range<usize> {
        let start = self.text[..offset].rfind('\n').map_or(0, |at| at + 1);
        let end = self.text[offset..]
            .find('\n')
            .map_or(self.text.len(), |at| offset + at);
        start..end
    }

    /// The match that was `then`'s `at`: same line and place, nearest its scrolled spot; else the nearest.
    fn follow(&self, then: &Found, at: usize) -> Option<usize> {
        let was = then.bytes[at].start;
        let line = then.line(was);
        let (text, offset) = (&then.text[line.clone()], was - line.start);
        let scrolled = self.history as i32 - then.history as i32;
        let expected = then.hits[at].start().line.0 - scrolled;
        let distance = |index: &usize| (self.hits[*index].start().line.0 - expected).abs();
        let all = 0..self.hits.len();
        all.clone()
            .filter(|&index| {
                let start = self.bytes[index].start;
                let line = self.line(start);
                start - line.start == offset && self.text[line] == *text
            })
            .min_by_key(distance)
            .or_else(|| all.min_by_key(distance))
    }
}

/// The match one newer than `current`, or one older, round the ends.
fn stepped(current: Option<usize>, count: usize, newer: bool) -> Option<usize> {
    let at = current?;
    Some(if newer {
        (at + 1) % count
    } else {
        (at + count - 1) % count
    })
}

impl TerminalView {
    /// Opens the find box, or takes the focus back to it.
    pub(super) fn find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = match &self.finder {
            Some(finder) => finder.query.clone(),
            None => {
                let query = cx.new(|cx| TextInput::new(window, cx));
                let input = cx.subscribe_in(&query, window, |view, _, event, _, cx| match event {
                    InputEvent::Changed => view.search(true, cx),
                    InputEvent::Submit => view.step(false, cx),
                    InputEvent::Focus | InputEvent::Blur => {}
                });
                log::debug!("terminal {}: find opens", self.pty.pid);
                self.finder = Some(Finder {
                    query: query.clone(),
                    options: FindOptions::default(),
                    found: Found::default(),
                    current: None,
                    error: None,
                    _input: input,
                });
                query
            }
        };
        query.update(cx, |input, cx| {
            let all = 0..input.text().len();
            input.select(all, cx);
        });
        window.focus(&query.focus_handle(cx), cx);
        cx.notify();
    }

    fn close_find(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.finder.take().is_some() {
            log::debug!("terminal {}: find closes", self.pty.pid);
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// Finds the query anew: fresh, from the newest match; else following the current one.
    pub(super) fn search(&mut self, fresh: bool, cx: &mut Context<Self>) {
        let Some(finder) = self.finder.as_mut() else {
            return;
        };
        let query = finder.query.read(cx).text().to_string();
        let found = Found::search(&self.pty.term.lock(), &query, finder.options);
        let (found, error) = match found {
            Ok(found) => (found, None),
            Err(error) => {
                log::debug!(
                    "terminal {}: {query:?} is no pattern: {error}",
                    self.pty.pid
                );
                (Found::default(), Some(SharedString::from(error)))
            }
        };
        finder.current = match finder.current {
            Some(at) if !fresh => found.follow(&finder.found, at),
            _ => found.hits.len().checked_sub(1),
        };
        finder.error = error;
        finder.found = found;
        if fresh {
            log::debug!(
                "terminal {}: {} matches of {query:?}",
                self.pty.pid,
                finder.found.hits.len()
            );
            self.reveal(cx);
        } else {
            let count = finder.found.hits.len();
            log::trace!(
                "terminal {}: {count} matches as the grid changed",
                self.pty.pid
            );
            cx.notify();
        }
    }

    /// Steps to a newer match, or an older one, and scrolls it into view.
    fn step(&mut self, newer: bool, cx: &mut Context<Self>) {
        let Some(finder) = self.finder.as_mut() else {
            return;
        };
        finder.current = stepped(finder.current, finder.found.hits.len(), newer);
        self.reveal(cx);
        cx.notify();
    }

    fn reveal(&mut self, cx: &mut Context<Self>) {
        let start = self.finder.as_ref().and_then(|finder| {
            let at = finder.current?;
            Some(*finder.found.hits[at].start())
        });
        if let Some(start) = start {
            log::trace!("terminal {}: match at {start:?}", self.pty.pid);
            self.pty.term.lock().scroll_to_point(start);
        }
        cx.notify();
    }

    fn set_options(&mut self, options: FindOptions, cx: &mut Context<Self>) {
        if let Some(finder) = self.finder.as_mut() {
            finder.options = options;
            self.search(true, cx);
        }
    }

    /// The find box over the grid; Shift-Enter steps to a newer match.
    pub(super) fn find_box(&self, cx: &mut Context<Self>) -> Option<impl IntoElement + use<>> {
        let finder = self.finder.as_ref()?;
        let (flip, step, close, back) = (cx.entity(), cx.entity(), cx.entity(), cx.entity());
        let total = finder.found.hits.len();
        let widget = FindWidget::new(
            ("find", cx.entity_id()),
            &finder.query,
            finder.current,
            total,
        )
        .options(finder.options)
        .on_options(move |options, _, cx| flip.update(cx, |view, cx| view.set_options(options, cx)))
        .on_step(move |newer, _, cx| step.update(cx, |view, cx| view.step(newer, cx)))
        .on_close(move |window, cx| close.update(cx, |view, cx| view.close_find(window, cx)));
        let widget = match &finder.error {
            Some(error) => widget.error(error.clone()),
            None => widget,
        };
        Some(
            div()
                .key_context(FIND)
                .on_action(move |_: &FindNewer, _, cx| {
                    back.update(cx, |view, cx| view.step(true, cx))
                })
                .child(widget),
        )
    }
}

#[cfg(test)]
mod tests;
