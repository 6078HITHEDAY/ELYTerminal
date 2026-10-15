mod store;
pub mod window;

use std::{
    env,
    ffi::OsStr,
    ops::RangeInclusive,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::themes;

pub use store::{SettingsStore, load, save};

/// Where a new session starts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StartIn {
    #[default]
    Home,
    Session,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CursorStyle {
    #[default]
    Block,
    Bar,
    Underline,
    Hollow,
}

/// Which Option keys type Alt on macOS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OptionAsAlt {
    #[default]
    Off,
    Left,
    Right,
    Both,
}

/// What the right panel shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelTab {
    #[default]
    Git,
    Files,
}

/// Everything the settings window edits. A field the file leaves out takes its default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    /// A program path; empty runs the login shell.
    pub shell: String,
    pub start_in: StartIn,
    pub theme: String,
    /// The theme while the system is dark; none keeps `theme`.
    pub dark_theme: Option<String>,
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub ligatures: bool,
    pub cursor_style: CursorStyle,
    pub cursor_blink: bool,
    pub background_opacity: f32,
    pub background_blur: bool,
    pub padding_x: f32,
    pub padding_y: f32,
    pub scrollback: u32,
    pub option_as_alt: OptionAsAlt,
    pub copy_on_select: bool,
    pub show_sessions: bool,
    pub show_panel: bool,
    pub panel: PanelTab,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            shell: String::new(),
            start_in: StartIn::Home,
            theme: "catppuccin-mocha".into(),
            dark_theme: None,
            font_family: "JetBrains Mono".into(),
            font_size: 13.0,
            line_height: 1.2,
            ligatures: true,
            cursor_style: CursorStyle::Block,
            cursor_blink: true,
            background_opacity: 0.88,
            background_blur: true,
            padding_x: 10.0,
            padding_y: 8.0,
            scrollback: 10_000,
            option_as_alt: OptionAsAlt::Off,
            copy_on_select: false,
            show_sessions: true,
            show_panel: true,
            panel: PanelTab::Git,
        }
    }
}

pub const FONT_SIZE: RangeInclusive<f32> = 6.0..=72.0;
pub const LINE_HEIGHT: RangeInclusive<f32> = 1.0..=2.0;
pub const OPACITY: RangeInclusive<f32> = 0.3..=1.0;
pub const PADDING: RangeInclusive<f32> = 0.0..=64.0;
pub const SCROLLBACK: RangeInclusive<u32> = 0..=100_000;

impl Settings {
    /// Every value inside its range, every theme built in; the error names the field.
    pub fn validate(&self) -> anyhow::Result<()> {
        within("font_size", self.font_size, &FONT_SIZE)?;
        within("line_height", self.line_height, &LINE_HEIGHT)?;
        within("background_opacity", self.background_opacity, &OPACITY)?;
        within("padding_x", self.padding_x, &PADDING)?;
        within("padding_y", self.padding_y, &PADDING)?;
        within("scrollback", self.scrollback, &SCROLLBACK)?;
        for (field, id) in [
            ("theme", Some(&self.theme)),
            ("dark_theme", self.dark_theme.as_ref()),
        ] {
            if let Some(id) = id {
                anyhow::ensure!(
                    themes::find(id).is_some(),
                    "{field} is {id:?}, no built-in theme"
                );
            }
        }
        anyhow::ensure!(!self.font_family.is_empty(), "font_family is empty");
        anyhow::ensure!(
            self.shell.is_empty() || program_path(&self.shell).is_some(),
            "shell is {:?}, which names no program",
            self.shell
        );
        Ok(())
    }
}

/// The program `shell` names, as a session starts it: a file at a full path, or the first in `PATH`'s full folders.
pub fn program_path(shell: &str) -> Option<PathBuf> {
    find(shell, &env::var_os("PATH").unwrap_or_default())
}

/// `shell`'s program, or the first one named so in `folders`, listed as `PATH` lists them.
fn find(shell: &str, folders: &OsStr) -> Option<PathBuf> {
    let path = Path::new(shell);
    if path.is_absolute() {
        return program(path).then(|| path.to_path_buf());
    }
    // A relative path, or a relative folder, would hang on each session's own folder.
    if path.components().count() != 1 {
        return None;
    }
    let files = env::split_paths(folders)
        .filter(|folder| folder.is_absolute())
        .flat_map(|folder| {
            let file = folder.join(path);
            let exe = cfg!(windows).then(|| file.with_extension("exe"));
            [Some(file), exe]
        });
    files.flatten().find(|file| program(file))
}

/// Whether `path` is a file the running user may run, as the system answers, ACLs too.
#[cfg(unix)]
fn program(path: &Path) -> bool {
    use std::{ffi::CString, os::unix::ffi::OsStrExt};
    let Ok(name) = CString::new(path.as_os_str().as_bytes()) else {
        return false;
    };
    path.is_file() && unsafe { libc::access(name.as_ptr(), libc::X_OK) } == 0
}

#[cfg(not(unix))]
fn program(path: &Path) -> bool {
    path.is_file()
}

fn within<T: PartialOrd + std::fmt::Display>(
    field: &str,
    value: T,
    range: &RangeInclusive<T>,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        range.contains(&value),
        "{field} is {value}, outside {} to {}",
        range.start(),
        range.end()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        Settings::default().validate().expect("defaults pass");
    }

    #[test]
    fn a_value_out_of_range_names_its_field() {
        let settings = Settings {
            line_height: 2.5,
            ..Settings::default()
        };
        let error = settings.validate().expect_err("2.5 is too tall");
        assert_eq!(error.to_string(), "line_height is 2.5, outside 1 to 2");
        let settings = Settings {
            scrollback: 100_001,
            ..Settings::default()
        };
        let error = settings.validate().expect_err("too many lines");
        assert_eq!(
            error.to_string(),
            "scrollback is 100001, outside 0 to 100000"
        );
    }

    #[test]
    fn a_shell_names_a_program_or_none() {
        let shell = |shell: &str| Settings {
            shell: shell.into(),
            ..Settings::default()
        };
        assert!(shell("").validate().is_ok(), "the login shell");
        let named = if cfg!(windows) { "cmd" } else { "sh" };
        assert!(shell(named).validate().is_ok(), "a name on PATH");
        let error = shell("/no/such/shell").validate().expect_err("no file");
        let said = "shell is \"/no/such/shell\", which names no program";
        assert_eq!(error.to_string(), said);
        assert!(shell("bin/sh").validate().is_err(), "a relative path");
        #[cfg(unix)]
        {
            let sh = Some(PathBuf::from("/bin/sh"));
            assert_eq!(find("sh", OsStr::new("/bin")), sh);
            let root = OsStr::new("/");
            assert_eq!(
                find("bin/sh", root),
                None,
                "a relative path, though / holds bin/sh"
            );
            assert!(shell("/bin/sh").validate().is_ok());
            assert!(shell("/bin").validate().is_err(), "a folder");
            assert!(
                shell("/etc/hosts").validate().is_err(),
                "a file that does not run"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_shell_runs_for_this_user_from_a_full_folder() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let folder = std::env::temp_dir().join(format!("elyterminal-shell-{}", std::process::id()));
        fs::create_dir_all(&folder).expect("a test folder");
        let file = folder.join("review-shell");
        fs::write(&file, "#!/bin/sh\n").expect("a script");
        // Root may run what any execute bit allows.
        let root = unsafe { libc::geteuid() } == 0;
        for (mode, runs) in [(0o644, false), (0o641, root), (0o755, true)] {
            fs::set_permissions(&file, fs::Permissions::from_mode(mode)).expect("a mode");
            assert_eq!(program(&file), runs, "mode {mode:o}");
        }
        assert_eq!(find("review-shell", folder.as_os_str()), Some(file.clone()));
    }

    #[test]
    fn a_relative_folder_counts_for_none() {
        // Tests run in the crate, whose scripts folder holds a program.
        let scripts = OsStr::new("scripts");
        assert!(program(Path::new("scripts/check.sh")));
        assert_eq!(find("check.sh", scripts), None);
    }

    #[test]
    fn a_nan_is_out_of_range() {
        let settings = Settings {
            font_size: f32::NAN,
            ..Settings::default()
        };
        assert!(settings.validate().is_err());
    }

    #[test]
    fn an_unknown_theme_fails() {
        let settings = Settings {
            dark_theme: Some(String::new()),
            ..Settings::default()
        };
        assert_eq!(
            settings.validate().expect_err("empty").to_string(),
            "dark_theme is \"\", no built-in theme"
        );
        let settings = Settings {
            theme: "mocha".into(),
            ..Settings::default()
        };
        assert_eq!(
            settings.validate().expect_err("unknown").to_string(),
            "theme is \"mocha\", no built-in theme"
        );
    }
}
