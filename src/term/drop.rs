//! Files dropped on the terminal type their paths, as macOS Terminal does.

use std::path::PathBuf;

use anyhow::Context as _;
use gpui::{Context, ExternalPaths};

use super::view::TerminalView;

/// What a shell reads specially in a path, which Ghostty escapes in one dropped.
const SPECIAL: &str = "\\ ()[]{}<>\"'`!#$&;|*?";

/// The text `paths` type, each followed by a space: escaped for a Unix shell, or quoted when it holds a space, as Windows Terminal quotes it.
pub fn typed(paths: &[PathBuf], windows: bool) -> anyhow::Result<String> {
    paths
        .iter()
        .map(|path| {
            let text = (path.to_str())
                .with_context(|| format!("{} is no UTF-8 a session can type", path.display()))?;
            anyhow::ensure!(
                !text.chars().any(char::is_control),
                "{text:?} holds a control character, which would act as typed"
            );
            Ok(if windows && text.contains(' ') {
                format!("\"{text}\" ")
            } else if windows {
                format!("{text} ")
            } else {
                let mut escaped = String::with_capacity(text.len() + 1);
                for ch in text.chars() {
                    if SPECIAL.contains(ch) {
                        escaped.push('\\');
                    }
                    escaped.push(ch);
                }
                escaped.push(' ');
                escaped
            })
        })
        .collect()
}

impl TerminalView {
    /// Types the dropped paths at the cursor as a paste; a path it cannot type stops the drop.
    pub(super) fn drop_paths(&mut self, dropped: &ExternalPaths, cx: &mut Context<Self>) {
        let paths = dropped.paths();
        match typed(paths, cfg!(windows)) {
            Ok(text) => {
                log::info!("terminal {}: {} paths dropped", self.pty.pid, paths.len());
                self.paste_text(&text, cx);
            }
            Err(error) => log::error!(
                "terminal {}: the drop types nothing: {error:#}",
                self.pty.pid
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(all: &[&str]) -> Vec<PathBuf> {
        all.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn a_unix_path_escapes_what_a_shell_reads_specially() {
        let typed = |all: &[&str]| typed(&paths(all), false).unwrap();
        assert_eq!(typed(&["/tmp/a.txt"]), "/tmp/a.txt ");
        assert_eq!(typed(&["/tmp/my file.png"]), "/tmp/my\\ file.png ");
        assert_eq!(typed(&["/a/it's (1)&$x"]), "/a/it\\'s\\ \\(1\\)\\&\\$x ");
        assert_eq!(typed(&["/a/中文 b"]), "/a/中文\\ b ");
        assert_eq!(
            typed(&["/a", "/b c"]),
            "/a /b\\ c ",
            "each followed by a space"
        );
    }

    #[test]
    fn a_windows_path_is_quoted_when_it_holds_a_space() {
        let typed = |all: &[&str]| typed(&paths(all), true).unwrap();
        assert_eq!(typed(&["C:\\a.txt"]), "C:\\a.txt ");
        assert_eq!(
            typed(&["C:\\My Files\\b.txt", "D:\\c"]),
            "\"C:\\My Files\\b.txt\" D:\\c "
        );
    }

    #[test]
    fn a_path_with_a_control_character_types_nothing() {
        let error = typed(&paths(&["/ok", "/tmp/a\nls"]), false).unwrap_err();
        assert!(error.to_string().contains("control character"), "{error}");
    }
}
