use gpui::{Window, WindowBackgroundAppearance};

use crate::settings::Settings;

/// What the window shows of the desktop: blurred or clear while the background lets it through.
pub fn appearance(settings: &Settings) -> WindowBackgroundAppearance {
    if settings.background_opacity >= 1.0 {
        WindowBackgroundAppearance::Opaque
    } else if settings.background_blur {
        WindowBackgroundAppearance::Blurred
    } else {
        WindowBackgroundAppearance::Transparent
    }
}

/// Sets `window` to what the settings ask when it differs from `shown`; logs the way taken.
pub fn follow(window: &Window, shown: &mut WindowBackgroundAppearance, settings: &Settings) {
    let wanted = appearance(settings);
    if wanted != *shown {
        *shown = wanted;
        window.set_background_appearance(wanted);
        log::info!("blur: the window is {wanted:?} on {}", std::env::consts::OS);
    }
}

#[cfg(test)]
mod tests {
    use gpui::WindowBackgroundAppearance::{Blurred, Opaque, Transparent};

    use super::*;

    #[test]
    fn the_desktop_shows_only_through_a_background_it_can_pass() {
        let at = |opacity, blur| {
            appearance(&Settings {
                background_opacity: opacity,
                background_blur: blur,
                ..Settings::default()
            })
        };
        assert_eq!(at(1.0, true), Opaque);
        assert_eq!(at(1.0, false), Opaque);
        assert_eq!(at(0.88, true), Blurred);
        assert_eq!(at(0.3, false), Transparent);
    }
}
