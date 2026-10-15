use std::ops::RangeInclusive;

use ely_gpui_component::{
    forms::{FontPicker, NumberInput, RadioCard, Slider, Switch},
    layout::SimpleGrid,
    settings::{SettingsRow, SettingsSection},
    theme::{ActiveTheme, ControlSize, Mix, Radius, TextSize},
};
use gpui::{
    App, Context, Div, Entity, Hsla, ParentElement, SharedString, Styled, Window, div, prelude::*,
    px, relative, rems,
};

use super::{SettingsWindow, choice, set, switch};
use crate::{
    settings::{CursorStyle, FONT_SIZE, LINE_HEIGHT, OPACITY, PADDING, Settings, SettingsStore},
    themes::{self, TerminalTheme},
};

/// Each cursor style, its key and its name.
const CURSORS: [(CursorStyle, &str, &str); 4] = [
    (CursorStyle::Block, "block", "Block"),
    (CursorStyle::Bar, "bar", "Bar"),
    (CursorStyle::Underline, "underline", "Underline"),
    (CursorStyle::Hollow, "hollow", "Hollow"),
];

impl SettingsWindow {
    /// The theme, font, cursor and window rows.
    pub(super) fn appearance(&self, window: &Window, cx: &Context<Self>) -> Div {
        let settings = SettingsStore::get(cx);
        let theme = cx.theme();
        let this = cx.entity();
        let separate = settings.dark_theme.is_some();
        let dark = Switch::new("dark_theme", separate).on_change({
            let this = this.clone();
            move |on, _, cx| {
                let theme = SettingsStore::get(cx).theme.clone();
                set(
                    &this,
                    format!("separate dark theme is {on}"),
                    cx,
                    |settings| settings.dark_theme = on.then_some(theme),
                );
            }
        });
        let light = separate.then_some("Light");
        let mut themes = SettingsSection::new("Theme")
            .row(grid(&this, light, false, settings, window, cx))
            .row(SettingsRow::new("Separate dark theme").control(dark));
        if separate {
            themes = themes.row(grid(&this, Some("Dark"), true, settings, window, cx));
        }
        let family = FontPicker::new("font_family", &self.font)
            .selected(settings.font_family.clone())
            .on_change({
                let this = this.clone();
                move |family, window, cx| {
                    let family = family.to_string();
                    let saved = set(
                        &this,
                        format!("font_family is {family:?}"),
                        cx,
                        |settings| settings.font_family = family,
                    );
                    // A refused pick shows the saved family again, the list closed.
                    if !saved {
                        let saved = SettingsStore::get(cx).font_family.clone();
                        let input = this.read(cx).font.clone();
                        input.update(cx, |input, cx| input.set_text(saved, cx));
                        window.blur(cx);
                    }
                }
            });
        let font = SettingsSection::new("Font")
            .row(SettingsRow::new("Family").control(div().w(theme.prose_width()).child(family)))
            .row(SettingsRow::new("Size").control(self.number(
                "font_size",
                settings.font_size,
                &FONT_SIZE,
                (1.0, 1),
                |settings, size| settings.font_size = size,
                cx,
            )))
            .row(SettingsRow::new("Line height").control(self.number(
                "line_height",
                settings.line_height,
                &LINE_HEIGHT,
                (0.05, 2),
                |settings, height| settings.line_height = height,
                cx,
            )))
            .row(SettingsRow::new("Ligatures").control(switch(
                &this,
                "ligatures",
                settings.ligatures,
                |settings, on| settings.ligatures = on,
            )));
        let style = choice(
            &this,
            "cursor_style",
            &CURSORS,
            settings.cursor_style,
            |settings, style| settings.cursor_style = style,
        );
        let active = themes::active(settings, window.appearance());
        let lit = self.lit || !settings.cursor_blink;
        let cursor = SettingsSection::new("Cursor")
            .row(SettingsRow::new("Style").control(style))
            .row(SettingsRow::new("Blink").control(switch(
                &this,
                "cursor_blink",
                settings.cursor_blink,
                |settings, on| settings.cursor_blink = on,
            )))
            .row(SettingsRow::new("Preview").control(prompt(settings, active, lit, cx)));
        let opacity = percent(settings.background_opacity);
        let slider = Slider::new("background_opacity", opacity)
            .range(percent(*OPACITY.start()), percent(*OPACITY.end()))
            .on_change({
                let this = this.clone();
                move |percent, _, cx| {
                    let value = fraction(percent);
                    set(
                        &this,
                        format!("background_opacity is {value}"),
                        cx,
                        |settings| settings.background_opacity = value,
                    );
                }
            });
        let opacity = div()
            .flex()
            .items_center()
            .gap_2()
            .child(div().w(theme.label_width()).child(slider))
            .child(
                div()
                    .text_size(theme.text_size(TextSize::Sm))
                    .child(format!("{opacity}%")),
            );
        let background = SettingsSection::new("Window")
            .row(SettingsRow::new("Background opacity").control(opacity))
            .row(SettingsRow::new("Background blur").control(switch(
                &this,
                "background_blur",
                settings.background_blur,
                |settings, on| settings.background_blur = on,
            )))
            .row(SettingsRow::new("Horizontal padding").control(self.number(
                "padding_x",
                settings.padding_x,
                &PADDING,
                (1.0, 0),
                |settings, padding| settings.padding_x = padding,
                cx,
            )))
            .row(SettingsRow::new("Vertical padding").control(self.number(
                "padding_y",
                settings.padding_y,
                &PADDING,
                (1.0, 0),
                |settings, padding| settings.padding_y = padding,
                cx,
            )));
        div()
            .flex()
            .flex_col()
            .gap_6()
            .child(themes)
            .child(font)
            .child(cursor)
            .child(background)
    }

    /// A field over `range`, by `step` to `places`, that saves each committed number through `put`; a refusal starts it again from the settings.
    fn number(
        &self,
        field: &'static str,
        value: f32,
        range: &RangeInclusive<f32>,
        (step, places): (f64, usize),
        put: fn(&mut Settings, f32),
        cx: &Context<Self>,
    ) -> NumberInput {
        let this = cx.entity();
        NumberInput::new((field, self.drafts), f64::from(value))
            .range(f64::from(*range.start()), f64::from(*range.end()))
            .step(step)
            .precision(places)
            .size(ControlSize::Sm)
            .on_commit(move |value, _, cx| {
                let value = value as f32;
                set(&this, format!("{field} is {value}"), cx, |settings| {
                    put(settings, value)
                });
            })
    }
}

/// `fraction` in whole percents; the slider moves by them, so a saved fraction carries no float noise.
fn percent(fraction: f32) -> f64 {
    (f64::from(fraction) * 100.).round()
}

fn fraction(percent: f64) -> f32 {
    (percent / 100.) as f32
}

/// Every built-in theme as a card over its preview; `dark` picks the one a dark system wears.
fn grid(
    this: &Entity<SettingsWindow>,
    title: Option<&'static str>,
    dark: bool,
    settings: &Settings,
    window: &Window,
    cx: &App,
) -> Div {
    let theme = cx.theme();
    let (field, chosen) = if dark {
        ("dark_theme", settings.dark_theme.as_deref())
    } else {
        ("theme", Some(settings.theme.as_str()))
    };
    let cards = themes::all().iter().map(|each| {
        let (id, this) = (each.id, this.clone());
        let key = SharedString::from(format!("{field}-{id}"));
        RadioCard::new(key, chosen == Some(id), each.name)
            .preview(preview(each, cx))
            .on_select(move |_, cx| {
                set(&this, format!("{field} is {id}"), cx, |settings| {
                    if dark {
                        settings.dark_theme = Some(id.into());
                    } else {
                        settings.theme = id.into();
                    }
                });
            })
    });
    let rem = window.rem_size();
    let (min, gap) = (
        theme.label_width().to_pixels(rem),
        rems(0.75).to_pixels(rem),
    );
    div()
        .flex()
        .flex_col()
        .gap_2()
        .py_3()
        .border_b_1()
        .border_color(theme.colors.border)
        .children(title.map(|title| div().text_size(theme.text_size(TextSize::Sm)).child(title)))
        .child(
            SimpleGrid::new(SharedString::from(format!("{field}-grid")), min, gap).children(cards),
        )
}

/// A small window in `theme`'s colors: the sessions strip, then a prompt, its output and the cursor.
fn preview(theme: &TerminalTheme, cx: &App) -> Div {
    let (bg, fg) = (theme.background, theme.foreground);
    let bar = |color: Hsla, width: f32| div().h_1().w(relative(width)).rounded_full().bg(color);
    let prompt = |end: Div| {
        let dot = div()
            .flex_none()
            .size_1p5()
            .rounded_full()
            .bg(theme.ansi[2]);
        div().flex().items_center().gap_1().child(dot).child(end)
    };
    let strip = div()
        .w(relative(0.2))
        .h_full()
        .flex()
        .flex_col()
        .items_center()
        .gap_1()
        .pt_2()
        .bg(bg.mix(&fg, 0.06))
        .children([0.6, 0.3, 0.3].map(|alpha| bar(fg.opacity(alpha), 0.6)));
    let lines = div()
        .flex_1()
        .flex()
        .flex_col()
        .justify_center()
        .gap_1()
        .px_2()
        .child(prompt(bar(fg, 0.5)))
        .child(bar(theme.ansi[4], 0.75))
        .child(bar(theme.ansi[5], 0.55))
        .child(prompt(div().flex_none().w_1p5().h_2p5().bg(theme.cursor)));
    let chrome = cx.theme();
    div()
        .w_full()
        .aspect_ratio(2.)
        .flex()
        .overflow_hidden()
        .rounded(chrome.radius(Radius::Sm))
        .border_1()
        .border_color(chrome.colors.border)
        .bg(bg)
        .child(strip)
        .child(lines)
}

/// A prompt in the terminal's theme and font, its cursor drawn as the terminal draws it.
fn prompt(settings: &Settings, theme: &TerminalTheme, lit: bool, cx: &App) -> Div {
    let rule = (px(settings.font_size) / 14.).max(px(1.));
    let mark = div().absolute().bg(theme.cursor);
    let cursor = match settings.cursor_style {
        CursorStyle::Block => mark.inset_0(),
        CursorStyle::Bar => mark.top_0().bottom_0().left_0().w(rule * 2.),
        CursorStyle::Underline => mark.left_0().right_0().bottom_0().h(rule * 2.),
        CursorStyle::Hollow => div()
            .absolute()
            .inset_0()
            .border_1()
            .border_color(theme.cursor),
    };
    let chrome = cx.theme();
    div()
        .flex()
        .p_2()
        .rounded(chrome.radius(Radius::Sm))
        .border_1()
        .border_color(chrome.colors.border)
        .bg(theme.background)
        .text_color(theme.foreground)
        .font_family(settings.font_family.clone())
        .text_size(px(settings.font_size))
        .line_height(relative(settings.line_height))
        .child(div().text_color(theme.ansi[2]).child("$"))
        .child(" ls")
        .child(
            div()
                .relative()
                .child(" ")
                .when(lit, |cell| cell.child(cursor)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_percent_saves_as_its_own_short_fraction() {
        let (low, high) = (percent(*OPACITY.start()), percent(*OPACITY.end()));
        assert_eq!((low, high), (30., 100.));
        for whole in 30..=100 {
            let saved = fraction(f64::from(whole));
            assert_eq!(percent(saved), f64::from(whole));
            let text = serde_json::to_string(&saved).expect("a number");
            assert!(text.len() <= 4, "{whole}% saves as {text}");
        }
    }
}
