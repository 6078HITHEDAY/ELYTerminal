//! The lines a wheel event scrolls, from the units gpui gives on each system.

use gpui::{Pixels, ScrollDelta, ScrollWheelEvent};

/// Lines a wheel line scrolls: gpui gives a notch as the lines Windows sets and as 3 on Linux; macOS gives one, which scrolls 3, as Ghostty's discrete wheel.
const NOTCH: f32 = if cfg!(target_os = "macos") { 3. } else { 1. };

/// Whole lines `event` moves, a screen of `rows` at most, as Windows' one screen at a time asks with u32::MAX lines; `rest` keeps a line's part.
pub fn lines(
    rest: &mut f32,
    event: &ScrollWheelEvent,
    height: Pixels,
    rows: usize,
    sideways: impl FnOnce() -> Option<(f32, f32)>,
) -> Option<i32> {
    let travel = match event.delta {
        ScrollDelta::Pixels(delta) => delta.y / height,
        // gpui gives Shift's notch sideways on Windows and Linux.
        ScrollDelta::Lines(delta) if event.modifiers.shift && delta.y == 0. && delta.x != 0. => {
            let (lines, chars) = sideways()?;
            delta.x * lines / chars * NOTCH
        }
        ScrollDelta::Lines(delta) => delta.y * NOTCH,
    };
    let page = rows as f32;
    *rest = (*rest + travel).clamp(-page, page);
    let lines = rest.trunc();
    *rest -= lines;
    Some(lines as i32)
}

/// The lines Windows sets for a notch and the characters for a sideways one, which gpui gives Shift's notch in.
#[cfg(windows)]
pub fn sideways() -> Option<(f32, f32)> {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SPI_GETWHEELSCROLLCHARS, SPI_GETWHEELSCROLLLINES, SystemParametersInfoW,
    };
    let setting = |action, name| {
        let mut value = 0u32;
        if unsafe { SystemParametersInfoW(action, 0, (&raw mut value).cast(), 0) } == 0 {
            log::error!("windows: the wheel's {name} did not read");
            return None;
        }
        Some(value as f32)
    };
    Some((
        setting(SPI_GETWHEELSCROLLLINES, "lines")?,
        setting(SPI_GETWHEELSCROLLCHARS, "characters")?,
    ))
}

/// Off Windows a sideways notch counts the wheel's own lines.
#[cfg(not(windows))]
pub fn sideways() -> Option<(f32, f32)> {
    Some((1., 1.))
}

#[cfg(test)]
mod tests {
    use gpui::{Modifiers, point, px};

    use super::*;

    fn wheel(delta: ScrollDelta, shift: bool) -> ScrollWheelEvent {
        let modifiers = Modifiers {
            shift,
            ..Modifiers::default()
        };
        ScrollWheelEvent {
            delta,
            modifiers,
            ..ScrollWheelEvent::default()
        }
    }

    /// gpui's lines a notch: Windows' setting there, 3 by default, 3 on Linux, one on macOS.
    #[test]
    fn a_notch_scrolls_as_the_system_asks_and_an_event_a_screen_at_most() {
        let mut rest = 0.;
        let mut scroll = |delta, shift, sideways: (f32, f32)| {
            lines(&mut rest, &wheel(delta, shift), px(14.4), 24, || {
                Some(sideways)
            })
        };
        let notch = |y: f32| ScrollDelta::Lines(point(0., y));
        let one = if cfg!(target_os = "macos") { 1. } else { 3. };
        assert_eq!(
            scroll(notch(one), false, (1., 1.)),
            Some(3),
            "at 125%, 14.4 points a line"
        );
        assert_eq!(scroll(notch(1.), false, (1., 1.)), Some(NOTCH as i32));
        let page = notch(-(u32::MAX as f32));
        assert_eq!(
            scroll(page, false, (1., 1.)),
            Some(-24),
            "Windows' one screen at a time"
        );
        assert_eq!(
            scroll(notch(-1.), false, (1., 1.)),
            Some(-NOTCH as i32),
            "no part stays"
        );
        let shifted = ScrollDelta::Lines(point(3., 0.));
        let at_seven = Some((7. * NOTCH) as i32);
        assert_eq!(
            scroll(shifted, true, (7., 3.)),
            at_seven,
            "Shift's 3 characters, 7 lines"
        );
        assert_eq!(
            scroll(shifted, false, (7., 3.)),
            Some(0),
            "a sideways wheel alone"
        );
        let part = ScrollDelta::Pixels(point(px(0.), px(10.)));
        assert_eq!(scroll(part, false, (1., 1.)), Some(0));
        assert_eq!(scroll(part, false, (1., 1.)), Some(1));
    }
}
