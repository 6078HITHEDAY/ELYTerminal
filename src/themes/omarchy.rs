use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use gpui::App;

use super::{OMARCHY, Omarchy, TerminalTheme, apply, parse};
use crate::settings::SettingsStore;

/// How often the theme folder is checked for a switch.
const CHECK: Duration = Duration::from_millis(500);

/// A Ghostty theme file's keys; Omarchy's other lines pass by.
const COLORS: [&str; 7] = [
    "palette",
    "background",
    "foreground",
    "cursor-color",
    "cursor-text",
    "selection-background",
    "selection-foreground",
];

/// Omarchy 4's current theme for Ghostty, under $HOME whatever XDG_STATE_HOME says.
pub fn path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".local/state/omarchy/current/theme/ghostty.conf"))
}

/// Follows Omarchy's theme; a failed read stops the start only if worn.
pub fn start(cx: &mut App) -> anyhow::Result<()> {
    let settings = SettingsStore::get(cx);
    let worn = settings.theme == OMARCHY || settings.dark_theme.as_deref() == Some(OMARCHY);
    let path = path().context("omarchy: no home folder")?;
    match read(&path) {
        Ok(theme) => {
            log::info!("omarchy: the theme follows {}", path.display());
            cx.set_global(Omarchy(theme));
            watch(path, cx);
            Ok(())
        }
        Err(error) if worn => Err(error.context("settings wear the Omarchy theme")),
        Err(error) if path.exists() => {
            log::error!("{error:#}; no Omarchy theme to follow");
            Ok(())
        }
        Err(_) => {
            log::info!("omarchy: none at {}", path.display());
            Ok(())
        }
    }
}

/// A switch replaces the folder, so the file's identity changes.
fn stamp(path: &Path) -> Option<(u64, u64, i64, i64, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((
        meta.dev(),
        meta.ino(),
        meta.mtime(),
        meta.mtime_nsec(),
        meta.len(),
    ))
}

/// Omarchy signals only Alacritty, kitty and Ghostty, so this checks.
fn watch(path: PathBuf, cx: &mut App) {
    let mut seen = stamp(&path);
    cx.spawn(async move |cx| {
        loop {
            cx.background_executor().timer(CHECK).await;
            let now = stamp(&path);
            // Gone mid-switch: the next check finds the new folder.
            if now.is_none() || now == seen {
                continue;
            }
            seen = now;
            match read(&path) {
                Ok(theme) => cx.update(|cx| {
                    log::info!("omarchy: the theme changed");
                    cx.set_global(Omarchy(theme));
                    apply(cx);
                }),
                Err(error) => log::error!("{error:#}; the last Omarchy theme stays"),
            }
        }
    })
    .detach();
}

fn read(path: &Path) -> anyhow::Result<TerminalTheme> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("omarchy: reading {}", path.display()))?;
    colors(&text).with_context(|| format!("omarchy: {}", path.display()))
}

/// Omarchy writes no cursor-text; the background stands in, as Ghostty's default.
pub(super) fn colors(text: &str) -> anyhow::Result<TerminalTheme> {
    let key = |line: &str| {
        line.split_once('=')
            .map_or("", |(key, _)| key.trim())
            .to_owned()
    };
    let mut lines: Vec<&str> = text
        .lines()
        .filter(|line| COLORS.contains(&key(line).as_str()))
        .collect();
    let background = lines
        .iter()
        .find(|line| key(line) == "background")
        .and_then(|line| line.split_once('='))
        .map(|(_, value)| format!("cursor-text = {}", value.trim()));
    if !lines.iter().any(|line| key(line) == "cursor-text") {
        lines.extend(background.as_deref());
    }
    parse(OMARCHY, "Omarchy", &lines.join("\n"))
}
