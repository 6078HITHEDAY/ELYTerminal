use alacritty_terminal::{
    event::EventListener,
    grid::Dimensions,
    index::{Column, Line},
    term::{
        Term,
        cell::{Cell, Flags},
    },
};

/// Clears each wide character on the screen an erase, insert or delete split, as xterm clears the whole character; alacritty clears one half and leaves the other drawing; returns how many went.
pub(super) fn mend<T: EventListener>(term: &mut Term<T>) -> usize {
    let (lines, columns) = (term.screen_lines(), term.columns());
    let mut mended = 0;
    for line in 0..lines {
        let row = &mut term.grid_mut()[Line(line as i32)];
        for column in 0..columns {
            let flags = row[Column(column)].flags;
            let paired =
                |at: usize, flag: Flags| at < columns && row[Column(at)].flags.contains(flag);
            if flags.contains(Flags::WIDE_CHAR) && !paired(column + 1, Flags::WIDE_CHAR_SPACER) {
                let bg = row[Column((column + 1).min(columns - 1))].bg;
                row[Column(column)] = Cell {
                    bg,
                    ..Cell::default()
                };
                mended += 1;
            } else if flags.contains(Flags::WIDE_CHAR_SPACER)
                && (column == 0 || !paired(column - 1, Flags::WIDE_CHAR))
            {
                row[Column(column)].flags.remove(Flags::WIDE_CHAR_SPACER);
                mended += 1;
            }
        }
    }
    mended
}

#[cfg(test)]
mod tests {
    use alacritty_terminal::{
        event::VoidListener,
        term::{Config, test::TermSize},
        vte::ansi::Processor,
    };

    use super::*;

    /// The first row after `bytes` and a mend, a wide character's spacer as `_`.
    fn mended(bytes: &str) -> String {
        let mut term = Term::new(Config::default(), &TermSize::new(10, 2), VoidListener);
        let mut parser: Processor = Processor::new();
        parser.advance(&mut term, bytes.as_bytes());
        mend(&mut term);
        let row = &term.grid()[Line(0)];
        let text: String = (0..10)
            .map(|column| {
                let cell = &row[Column(column)];
                if cell.flags.contains(Flags::WIDE_CHAR_SPACER) {
                    '_'
                } else {
                    cell.c
                }
            })
            .collect();
        text.trim_end().to_string()
    }

    #[test]
    fn a_wide_character_split_by_an_erase_goes_whole_as_xterm_clears_it() {
        // `单` takes the second and third cells; each sequence starts on one of its halves.
        for (sequence, xterm) in [
            ("\x1b[1;3H\x1b[1X", "A  BC"),
            ("\x1b[1;2H\x1b[1X", "A  BC"),
            ("\x1b[1;3H\x1b[K", "A"),
            ("\x1b[1;3H\x1b[J", "A"),
            ("\x1b[1;3H\x1b[P", "A BC"),
            ("\x1b[1;3H\x1b[@", "A   BC"),
        ] {
            assert_eq!(mended(&format!("A单BC{sequence}")), xterm, "{sequence:?}");
        }
    }

    #[test]
    fn whole_wide_characters_stay() {
        assert_eq!(mended("A单BC"), "A单_BC");
        assert_eq!(mended("A单BC\x1b[1;3HX"), "A XBC");
    }
}
