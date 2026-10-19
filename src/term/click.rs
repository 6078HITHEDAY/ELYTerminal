//! A click on the line the shell reads moves its cursor there with arrows, as Ghostty's `cursor-click-to-move`.

use alacritty_terminal::{
    grid::Dimensions,
    index::{Column, Line, Point},
    term::{
        Term, TermMode,
        cell::{Cell, Flags},
    },
};
use gpui::Context;

use super::{prompts::prompted, view::TerminalView};

/// Arrows from the cursor to `click`, right positive; none when `click` lies off the input after the cursor's prompt, soft wraps joined.
pub fn arrows<T>(term: &Term<T>, click: Point) -> Option<i32> {
    if term.mode().contains(TermMode::ALT_SCREEN) {
        return None;
    }
    let grid = term.grid();
    let columns = grid.columns();
    let cursor = grid.cursor.point;
    let wraps = |line: i32| {
        grid[Line(line)][Column(columns - 1)]
            .flags
            .contains(Flags::WRAPLINE)
    };
    let mut first = cursor.line.0;
    while first > grid.topmost_line().0 && wraps(first - 1) {
        first -= 1;
    }
    let mut last = cursor.line.0;
    while last < grid.bottommost_line().0 && wraps(last) {
        last += 1;
    }
    if !(first..=last).contains(&click.line.0) {
        return None;
    }
    let cells: Vec<&Cell> = (first..=last)
        .flat_map(|line| (0..columns).map(move |column| &grid[Line(line)][Column(column)]))
        .collect();
    let at = |point: Point| (point.line.0 - first) as usize * columns + point.column.0;
    let (here, there) = (at(cursor), at(click));
    // Input starts past the prompt the cursor's line shows before it.
    let start = cells[..here].iter().rposition(|cell| prompted(cell))? + 1;
    if there < start {
        return None;
    }
    let inked = |cell: &&Cell| !prompted(cell) && !matches!(cell.c, ' ' | '\t');
    let end = cells[start..]
        .iter()
        .rposition(inked)
        .map_or(here, |last| here.max(start + last + 1));
    let mut target = there.min(end);
    if cells[target].flags.contains(Flags::WIDE_CHAR_SPACER) {
        target -= 1;
    }
    let spacer = Flags::WIDE_CHAR_SPACER | Flags::LEADING_WIDE_CHAR_SPACER;
    let typed = |cell: &&&Cell| !prompted(cell) && !cell.flags.intersects(spacer);
    let count = |from: usize, to: usize| cells[from..to].iter().filter(typed).count() as i32;
    Some(if target >= here {
        count(here, target)
    } else {
        -count(target, here)
    })
}

impl TerminalView {
    /// Moves the shell's cursor to `click`, a press and release of an arrow per character, while the shell reads a line.
    pub(super) fn click_to_move(&mut self, click: Point, cx: &mut Context<Self>) {
        let pid = self.pty.pid;
        if !self.reading {
            log::debug!("terminal {pid}: a click at {click:?}, no line read");
            return;
        }
        let Some(count) = arrows(&self.pty.term.lock(), click) else {
            log::debug!("terminal {pid}: a click at {click:?}, off the input");
            return;
        };
        log::debug!("terminal {pid}: a click at {click:?} moves the cursor {count}");
        let arrow = if count < 0 { "left" } else { "right" };
        for _ in 0..count.unsigned_abs() {
            self.tap_key(arrow, cx);
        }
    }
}

#[cfg(test)]
mod tests;
