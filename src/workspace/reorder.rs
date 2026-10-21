use gpui::Context;

use super::Workspace;

/// The session shown once session `from` moves to place `to`.
pub fn after_move(active: usize, from: usize, to: usize) -> usize {
    if active == from {
        to
    } else if from < active && active <= to {
        active - 1
    } else if to <= active && active < from {
        active + 1
    } else {
        active
    }
}

impl Workspace {
    /// Moves session `from` to place `to`, both inside the list.
    pub(super) fn move_session(&mut self, from: usize, to: usize) {
        let moved = self.sessions.remove(from);
        self.sessions.insert(to, moved);
        self.active = after_move(self.active, from, to);
    }

    /// Moves session `from` to place `to`, as a drag asks.
    pub fn reorder(&mut self, from: usize, to: usize, cx: &mut Context<Self>) {
        if from == to || from >= self.sessions.len() || to >= self.sessions.len() {
            return;
        }
        log::info!("app: session {} moves to {}", from + 1, to + 1);
        self.move_session(from, to);
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::after_move;

    #[test]
    fn the_shown_session_follows_its_row() {
        assert_eq!(after_move(2, 2, 0), 0, "the shown row moved");
        assert_eq!(after_move(2, 0, 3), 1, "a row below it went up");
        assert_eq!(after_move(2, 3, 0), 3, "a row above it went down");
        assert_eq!(after_move(3, 1, 0), 3, "a move below it leaves it");
        assert_eq!(after_move(0, 2, 3), 0, "a move below the shown one");
    }
}
