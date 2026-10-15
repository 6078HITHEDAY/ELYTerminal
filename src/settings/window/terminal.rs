use ely_gpui_component::{
    forms::NumberInput,
    settings::{SettingsRow, SettingsSection},
    theme::ControlSize,
};
use gpui::Context;

use super::{SettingsWindow, choice, set, switch};
use crate::settings::{OptionAsAlt, SCROLLBACK, SettingsStore};

/// Each way Option types Alt, its key and its name.
const OPTIONS: [(OptionAsAlt, &str, &str); 4] = [
    (OptionAsAlt::Off, "off", "Off"),
    (OptionAsAlt::Left, "left", "Left"),
    (OptionAsAlt::Right, "right", "Right"),
    (OptionAsAlt::Both, "both", "Both"),
];

impl SettingsWindow {
    /// Scrollback, Option as Alt on macOS, and copy on select.
    pub(super) fn terminal(&self, cx: &Context<Self>) -> SettingsSection {
        let settings = SettingsStore::get(cx);
        let this = cx.entity();
        let lines = NumberInput::new(("scrollback", self.drafts), f64::from(settings.scrollback))
            .range(f64::from(*SCROLLBACK.start()), f64::from(*SCROLLBACK.end()))
            .step(1000.)
            .size(ControlSize::Sm)
            .on_commit({
                let this = this.clone();
                move |lines, _, cx| {
                    let lines = lines as u32;
                    set(&this, format!("scrollback is {lines}"), cx, |settings| {
                        settings.scrollback = lines
                    });
                }
            });
        let mut section = SettingsSection::new("Terminal")
            .row(SettingsRow::new("Scrollback lines").control(lines));
        // Off macOS, Alt is always Alt.
        if cfg!(target_os = "macos") {
            let option = choice(
                &this,
                "option_as_alt",
                &OPTIONS,
                settings.option_as_alt,
                |settings, option| settings.option_as_alt = option,
            );
            section = section.row(SettingsRow::new("Option as Alt").control(option));
        }
        let copy = switch(
            &this,
            "copy_on_select",
            settings.copy_on_select,
            |settings, on| settings.copy_on_select = on,
        );
        section.row(SettingsRow::new("Copy on select").control(copy))
    }
}
