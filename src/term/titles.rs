/// The titles alacritty saves before it drops the oldest.
const SAVED: usize = 4096;

/// A program's title as alacritty keeps it, read in the output's order: OSC 0 and 2 set it, CSI 22 t saves it, CSI 23 t brings it back, a full reset empties both.
#[derive(Default)]
pub(crate) struct Titles {
    /// Empty while the program names none.
    title: String,
    saved: Vec<String>,
}

impl Titles {
    /// Sets the title an OSC 0 or 2 body past its number names: its UTF-8 parameters joined by `;` and trimmed, as alacritty joins them.
    pub fn set(&mut self, rest: &[u8]) -> String {
        let parts: Vec<&str> = rest
            .split(|&byte| byte == b';')
            .filter_map(|part| std::str::from_utf8(part).ok())
            .collect();
        self.title = parts.join(";").trim().to_owned();
        self.title.clone()
    }

    /// Saves the title: CSI 22 t.
    pub fn push(&mut self) {
        if self.saved.len() == SAVED {
            self.saved.remove(0);
        }
        self.saved.push(self.title.clone());
    }

    /// Brings the last title saved back: CSI 23 t; none when none was saved.
    pub fn pop(&mut self) -> Option<String> {
        let title = self.saved.pop()?;
        self.title.clone_from(&title);
        Some(title)
    }

    /// A full reset empties the title and those saved.
    pub fn reset(&mut self) {
        self.title.clear();
        self.saved.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_title_reads_as_alacritty_joins_it() {
        let mut titles = Titles::default();
        assert_eq!(titles.set(b"  a;b  "), "a;b");
        assert_eq!(
            titles.set(b"a;\xff;b"),
            "a;b",
            "a part not in UTF-8 passes by"
        );
        assert_eq!(titles.set(b""), "");
    }

    #[test]
    fn a_title_saved_comes_back_and_a_reset_empties_both() {
        let mut titles = Titles::default();
        assert_eq!(titles.pop(), None, "none saved");
        titles.set(b"shell");
        titles.push();
        titles.set(b"vim");
        assert_eq!(titles.pop().as_deref(), Some("shell"));
        assert_eq!(titles.pop(), None);
        titles.push();
        titles.reset();
        assert_eq!(titles.pop(), None, "a reset empties those saved");
        for at in 0..=SAVED {
            titles.set(at.to_string().as_bytes());
            titles.push();
        }
        assert_eq!(titles.saved.len(), SAVED);
        assert_eq!(titles.saved[0], "1", "the oldest went");
    }
}
