use alacritty_terminal::term::TermMode;

/// A button as reports name it; the wheel's two turns count as buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Middle,
    Right,
    WheelUp,
    WheelDown,
}

/// What the mouse did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gesture {
    Press,
    Release,
    Motion,
}

/// A mouse event a program may ask for.
#[derive(Clone, Copy, Debug)]
pub struct Report {
    /// None for motion with no button down.
    pub button: Option<Button>,
    pub gesture: Gesture,
    /// Column and row on screen, from 0.
    pub cell: (usize, usize),
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// The mouse events a program asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tracking {
    Off,
    /// Mode 9: presses alone, no modifiers.
    X10,
    /// Mode 1000: presses and releases.
    Normal,
    /// Mode 1002: and motion with a button down.
    Button,
    /// Mode 1003: and all motion.
    Any,
}

impl Tracking {
    /// The tracking `mode` sets; `x10` is mode 9, which the tap follows.
    pub fn new(mode: TermMode, x10: bool) -> Self {
        if x10 {
            Self::X10
        } else if mode.contains(TermMode::MOUSE_MOTION) {
            Self::Any
        } else if mode.contains(TermMode::MOUSE_DRAG) {
            Self::Button
        } else if mode.contains(TermMode::MOUSE_REPORT_CLICK) {
            Self::Normal
        } else {
            Self::Off
        }
    }
}

/// The bytes `report` sends; none when the program did not ask for it or its format cannot hold the cell.
pub fn encode(report: &Report, tracking: Tracking, mode: TermMode) -> Option<Vec<u8>> {
    let clicked = matches!(
        report.button,
        Some(Button::Left | Button::Middle | Button::Right)
    );
    let wanted = match tracking {
        Tracking::Off => false,
        Tracking::X10 => report.gesture == Gesture::Press && clicked,
        Tracking::Normal => report.gesture != Gesture::Motion,
        Tracking::Button => report.gesture != Gesture::Motion || report.button.is_some(),
        Tracking::Any => true,
    };
    if !wanted {
        return None;
    }
    let sgr = mode.contains(TermMode::SGR_MOUSE);
    let mut code = match report.button {
        None => 3,
        Some(_) if report.gesture == Gesture::Release && !sgr => 3,
        Some(Button::Left) => 0,
        Some(Button::Middle) => 1,
        Some(Button::Right) => 2,
        Some(Button::WheelUp) => 64,
        Some(Button::WheelDown) => 65,
    };
    if tracking != Tracking::X10 {
        code +=
            4 * u32::from(report.shift) + 8 * u32::from(report.alt) + 16 * u32::from(report.ctrl);
    }
    if report.gesture == Gesture::Motion {
        code += 32;
    }
    let (column, row) = (report.cell.0 + 1, report.cell.1 + 1);
    if sgr {
        let end = if report.gesture == Gesture::Release {
            'm'
        } else {
            'M'
        };
        return Some(format!("\x1b[<{code};{column};{row}{end}").into_bytes());
    }
    let utf8 = mode.contains(TermMode::UTF8_MOUSE);
    let most = if utf8 { 0x7FF - 32 } else { 0xFF - 32 };
    if column > most || row > most {
        log::debug!("mouse: cell {column},{row} is past what the format holds");
        return None;
    }
    let mut bytes = b"\x1b[M".to_vec();
    bytes.push(32 + code as u8);
    for value in [column, row] {
        let value = 32 + value as u32;
        if utf8 {
            let ch = char::from_u32(value).expect("a code point under U+0800");
            bytes.extend(ch.to_string().as_bytes());
        } else {
            bytes.push(value as u8);
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(button: Option<Button>, gesture: Gesture, cell: (usize, usize)) -> Report {
        Report {
            button,
            gesture,
            cell,
            shift: false,
            alt: false,
            ctrl: false,
        }
    }

    fn sent(report: Report, tracking: Tracking, mode: TermMode) -> Option<Vec<u8>> {
        encode(&report, tracking, mode)
    }

    const LEFT: Option<Button> = Some(Button::Left);

    #[test]
    fn normal_tracking_sends_presses_and_releases_as_x10_bytes() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let tracking = Tracking::new(mode, false);
        assert_eq!(tracking, Tracking::Normal);
        let press = report(LEFT, Gesture::Press, (0, 0));
        assert_eq!(sent(press, tracking, mode), Some(b"\x1b[M !!".to_vec()));
        let release = report(LEFT, Gesture::Release, (3, 2));
        assert_eq!(sent(release, tracking, mode), Some(b"\x1b[M#$#".to_vec()));
        let motion = report(LEFT, Gesture::Motion, (1, 1));
        assert_eq!(sent(motion, tracking, mode), None);
    }

    #[test]
    fn sgr_names_the_released_button_and_the_modifiers() {
        let mode = TermMode::MOUSE_REPORT_CLICK | TermMode::SGR_MOUSE;
        let press = Report {
            shift: true,
            ctrl: true,
            ..report(Some(Button::Right), Gesture::Press, (4, 2))
        };
        assert_eq!(
            sent(press, Tracking::Normal, mode),
            Some(b"\x1b[<22;5;3M".to_vec())
        );
        let release = report(Some(Button::Middle), Gesture::Release, (300, 9));
        assert_eq!(
            sent(release, Tracking::Normal, mode),
            Some(b"\x1b[<1;301;10m".to_vec())
        );
        let alt = Report {
            alt: true,
            ..report(Some(Button::WheelDown), Gesture::Press, (0, 0))
        };
        assert_eq!(
            sent(alt, Tracking::Normal, mode),
            Some(b"\x1b[<73;1;1M".to_vec())
        );
    }

    #[test]
    fn motion_needs_button_or_any_tracking() {
        let mode = TermMode::SGR_MOUSE;
        let held = report(LEFT, Gesture::Motion, (1, 0));
        let free = report(None, Gesture::Motion, (1, 0));
        assert_eq!(Tracking::new(TermMode::MOUSE_DRAG, false), Tracking::Button);
        assert_eq!(
            sent(held, Tracking::Button, mode),
            Some(b"\x1b[<32;2;1M".to_vec())
        );
        assert_eq!(sent(free, Tracking::Button, mode), None);
        assert_eq!(Tracking::new(TermMode::MOUSE_MOTION, false), Tracking::Any);
        assert_eq!(
            sent(free, Tracking::Any, mode),
            Some(b"\x1b[<35;2;1M".to_vec())
        );
    }

    #[test]
    fn x10_tracking_sends_presses_alone_without_modifiers() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let tracking = Tracking::new(mode, true);
        assert_eq!(tracking, Tracking::X10);
        let press = Report {
            shift: true,
            ..report(LEFT, Gesture::Press, (0, 0))
        };
        assert_eq!(sent(press, tracking, mode), Some(b"\x1b[M !!".to_vec()));
        let release = report(LEFT, Gesture::Release, (0, 0));
        assert_eq!(sent(release, tracking, mode), None);
        let wheel = report(Some(Button::WheelUp), Gesture::Press, (0, 0));
        assert_eq!(sent(wheel, tracking, mode), None);
        assert_eq!(Tracking::new(TermMode::empty(), false), Tracking::Off);
    }

    #[test]
    fn cells_past_the_format_go_utf8_or_nowhere() {
        let mode = TermMode::MOUSE_REPORT_CLICK;
        let edge = report(LEFT, Gesture::Press, (222, 0));
        assert_eq!(
            sent(edge, Tracking::Normal, mode),
            Some(vec![0x1b, b'[', b'M', 32, 255, 33])
        );
        let past = report(LEFT, Gesture::Press, (223, 0));
        assert_eq!(sent(past, Tracking::Normal, mode), None);
        let utf8 = mode | TermMode::UTF8_MOUSE;
        let mut wide = b"\x1b[M ".to_vec();
        wide.extend("\u{100}!".as_bytes());
        assert_eq!(sent(past, Tracking::Normal, utf8), Some(wide));
        let far = report(LEFT, Gesture::Press, (2015, 0));
        assert_eq!(sent(far, Tracking::Normal, utf8), None);
    }
}
