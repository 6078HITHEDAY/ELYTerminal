use gpui::{Window, WindowBackgroundAppearance};

use crate::settings::Settings;

#[cfg(target_os = "linux")]
mod wayland;

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

/// What the main window shows of the desktop, as last set, and on Wayland the blur behind its surface.
pub struct Blur {
    shown: WindowBackgroundAppearance,
    #[cfg(target_os = "linux")]
    wayland: Option<wayland::Effect>,
}

impl Blur {
    /// The window as `WindowOptions` opened it from these settings.
    pub fn new(window: &Window, settings: &Settings) -> Self {
        let shown = appearance(settings);
        #[cfg(not(target_os = "linux"))]
        let _ = window;
        Self {
            shown,
            #[cfg(target_os = "linux")]
            wayland: wayland::Effect::on(window, shown == WindowBackgroundAppearance::Blurred),
        }
    }

    /// Sets the window to what the settings ask when it differs from what it shows; logs the way taken.
    pub fn follow(&mut self, window: &Window, settings: &Settings) {
        let wanted = appearance(settings);
        if wanted == self.shown {
            return;
        }
        self.shown = wanted;
        window.set_background_appearance(wanted);
        log::info!("blur: the window is {wanted:?} on {}", std::env::consts::OS);
        #[cfg(target_os = "linux")]
        if let Some(effect) = &self.wayland {
            effect.blur(wanted == WindowBackgroundAppearance::Blurred);
        }
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
