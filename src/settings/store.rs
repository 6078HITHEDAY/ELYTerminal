use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use gpui::{App, Global};

use super::Settings;

/// The settings in force and the file they live in; the only writer of that file.
pub struct SettingsStore {
    settings: Settings,
    path: PathBuf,
}

impl Global for SettingsStore {}

impl SettingsStore {
    /// Loads `settings.json` from the platform config folder into the app.
    pub fn init(cx: &mut App) -> anyhow::Result<()> {
        let folder = dirs::config_dir().context("settings: the platform names no config folder")?;
        let path = folder.join("ELYTerminal").join("settings.json");
        let settings = load(&path)?;
        cx.set_global(Self { settings, path });
        Ok(())
    }

    pub fn get(cx: &App) -> &Settings {
        &cx.global::<Self>().settings
    }

    /// Applies `edit`, checks the result and saves it; a bad edit changes nothing.
    pub fn update(cx: &mut App, edit: impl FnOnce(&mut Settings)) -> anyhow::Result<()> {
        let store = cx.global::<Self>();
        let mut next = store.settings.clone();
        edit(&mut next);
        if next == store.settings {
            return Ok(());
        }
        next.validate()?;
        save(&store.path, &next)?;
        cx.global_mut::<Self>().settings = next;
        Ok(())
    }
}

/// The file's settings; defaults when there is no file. A file that fails to read or check is an error.
pub fn load(path: &Path) -> anyhow::Result<Settings> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            log::info!("settings: no file at {}, defaults", path.display());
            return Ok(Settings::default());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("settings: cannot read {}", path.display()));
        }
    };
    let settings: Settings = serde_json::from_str(&text)
        .with_context(|| format!("settings: {} does not parse", path.display()))?;
    settings
        .validate()
        .with_context(|| format!("settings: {} holds a bad value", path.display()))?;
    log::info!("settings: loaded {}", path.display());
    Ok(settings)
}

/// Writes a temp file beside `path`, then renames it over `path`.
pub fn save(path: &Path, settings: &Settings) -> anyhow::Result<()> {
    let folder = path
        .parent()
        .with_context(|| format!("settings: {} has no folder", path.display()))?;
    fs::create_dir_all(folder)
        .with_context(|| format!("settings: cannot create {}", folder.display()))?;
    let temp = path.with_extension("json.tmp");
    let mut text = serde_json::to_string_pretty(settings).context("settings: cannot encode")?;
    text.push('\n');
    let mut file = fs::File::create(&temp)
        .with_context(|| format!("settings: cannot create {}", temp.display()))?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.sync_all())
        .with_context(|| format!("settings: cannot write {}", temp.display()))?;
    fs::rename(&temp, path).with_context(|| {
        format!(
            "settings: cannot move {} to {}",
            temp.display(),
            path.display()
        )
    })?;
    log::info!("settings: saved {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::CursorStyle;

    fn folder(name: &str) -> PathBuf {
        let folder =
            std::env::temp_dir().join(format!("elyterminal-{name}-{}", std::process::id()));
        if folder.exists() {
            fs::remove_dir_all(&folder).expect("a stale test folder goes");
        }
        folder
    }

    #[test]
    fn no_file_gives_defaults() {
        let path = folder("none").join("settings.json");
        assert_eq!(load(&path).expect("defaults"), Settings::default());
    }

    #[test]
    fn a_saved_file_loads_back_and_leaves_no_temp() {
        let folder = folder("round");
        let path = folder.join("settings.json");
        let settings = Settings {
            font_size: 15.5,
            cursor_style: CursorStyle::Bar,
            dark_theme: Some("catppuccin-latte".into()),
            ..Settings::default()
        };
        save(&path, &settings).expect("saves");
        assert_eq!(load(&path).expect("loads"), settings);
        assert!(!path.with_extension("json.tmp").exists());
        fs::remove_dir_all(folder).expect("cleanup");
    }

    #[test]
    fn a_missing_field_takes_its_default() {
        let folder = folder("partial");
        let path = folder.join("settings.json");
        fs::create_dir_all(&folder).expect("folder");
        fs::write(&path, r#"{ "font_size": 16.0 }"#).expect("write");
        let settings = load(&path).expect("loads");
        assert_eq!(settings.font_size, 16.0);
        assert_eq!(settings.theme, Settings::default().theme);
        fs::remove_dir_all(folder).expect("cleanup");
    }

    #[test]
    fn a_bad_file_is_an_error_naming_it() {
        let folder = folder("bad");
        let path = folder.join("settings.json");
        fs::create_dir_all(&folder).expect("folder");
        fs::write(&path, "{ font_size: ").expect("write");
        let error = load(&path).expect_err("does not parse");
        assert!(format!("{error:#}").contains("does not parse"));
        assert!(error.to_string().contains("settings.json"));
        fs::write(&path, r#"{ "cursor_style": "beam" }"#).expect("write");
        assert!(format!("{:#}", load(&path).expect_err("unknown style")).contains("beam"));
        fs::write(&path, r#"{ "font_sise": 12 }"#).expect("write");
        assert!(format!("{:#}", load(&path).expect_err("unknown field")).contains("font_sise"));
        fs::remove_dir_all(folder).expect("cleanup");
    }

    #[test]
    fn a_bad_value_is_an_error_naming_the_field() {
        let folder = folder("range");
        let path = folder.join("settings.json");
        fs::create_dir_all(&folder).expect("folder");
        fs::write(&path, r#"{ "background_opacity": 0.1 }"#).expect("write");
        let error = load(&path).expect_err("too clear");
        assert!(format!("{error:#}").contains("background_opacity is 0.1"));
        fs::remove_dir_all(folder).expect("cleanup");
    }
}
