use gpui::{Rgba, WindowAppearance};

use super::*;

fn rgb(hex: u32) -> Hsla {
    let channel = |shift: u32| ((hex >> shift) & 0xff) as f32 / 255.0;
    Rgba {
        r: channel(16),
        g: channel(8),
        b: channel(0),
        a: 1.0,
    }
    .into()
}

#[test]
fn every_bundled_file_parses_with_unique_ids() {
    assert_eq!(all().len(), 14);
    let mut ids: Vec<_> = all().iter().map(|theme| theme.id).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), 14);
    let folder = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/themes");
    for entry in std::fs::read_dir(folder).expect("themes folder") {
        let name = entry
            .expect("entry")
            .file_name()
            .into_string()
            .expect("utf-8");
        if name != "LICENSE" {
            assert!(find(&name).is_some(), "{name} is bundled but not listed");
        }
    }
}

#[test]
fn catppuccin_mocha_reads_its_upstream_colors() {
    let mocha = find("catppuccin-mocha").expect("mocha");
    assert_eq!(mocha.background, rgb(0x1e1e2e));
    assert_eq!(mocha.foreground, rgb(0xcdd6f4));
    assert_eq!(mocha.cursor, rgb(0xf5e0dc));
    assert_eq!(mocha.ansi[1], rgb(0xf38ba8));
    assert_eq!(mocha.ansi[15], rgb(0xa6adc8));
    assert_eq!(mocha.selection_foreground, Some(rgb(0x1e1e2e)));
}

#[test]
fn light_and_dark_follow_the_background() {
    let light: Vec<_> = all()
        .iter()
        .filter(|theme| !theme.is_dark())
        .map(|theme| theme.id)
        .collect();
    assert_eq!(
        light,
        [
            "ely-light",
            "catppuccin-latte",
            "solarized-light",
            "one-light"
        ]
    );
}

#[test]
fn a_missing_key_or_bad_color_names_it() {
    let text = include_str!("../../assets/themes/nord");
    let cut: String = text
        .lines()
        .filter(|line| !line.starts_with("cursor-text"))
        .collect::<Vec<_>>()
        .join("\n");
    let error = parse("x", "X", &cut).expect_err("no cursor-text");
    assert_eq!(error.to_string(), "no `cursor-text`");
    let cut: String = text
        .lines()
        .filter(|line| !line.starts_with("palette = 7="))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        parse("x", "X", &cut).expect_err("no 7").to_string(),
        "no palette 7"
    );
    let bad = text.replace("background = #", "background = ");
    assert!(parse("x", "X", &bad).is_err());
    assert!(parse("x", "X", "palette = 16=#000000").is_err());
}

#[test]
fn the_dark_theme_wins_only_in_dark_mode() {
    let settings = Settings {
        theme: "catppuccin-latte".into(),
        dark_theme: Some("catppuccin-mocha".into()),
        ..Settings::default()
    };
    assert_eq!(
        active(&settings, WindowAppearance::Light).id,
        "catppuccin-latte"
    );
    assert_eq!(
        active(&settings, WindowAppearance::VibrantDark).id,
        "catppuccin-mocha"
    );
    let single = Settings {
        dark_theme: None,
        ..settings
    };
    assert_eq!(
        active(&single, WindowAppearance::Dark).id,
        "catppuccin-latte"
    );
}

#[test]
fn the_chrome_wears_the_theme() {
    let dracula = find("dracula").expect("dracula");
    let chrome = palette(dracula, 1.0);
    assert_eq!(chrome.bg, dracula.background);
    assert_eq!(chrome.fg, dracula.foreground);
    assert_eq!(chrome.ansi, dracula.ansi);
    assert_eq!(chrome.danger, dracula.ansi[1]);
    let ely = palette(find("ely-dark").expect("ely"), 1.0);
    assert_eq!(ely.bg, Palette::dark(false).bg, "Ely's own colors");
}

#[test]
fn the_glass_lets_through_what_the_opacity_leaves() {
    for id in ["dracula", "ely-light"] {
        let theme = find(id).expect("a theme");
        let chrome = palette(theme, 0.4);
        assert_eq!(chrome.glass, chrome.bg.alpha(0.4), "{id}");
        assert_eq!(
            chrome.bg.a, 1.0,
            "{id}: the ground under dialogs stays solid"
        );
        assert_eq!(palette(theme, 1.0).glass, chrome.bg, "{id}");
    }
}
