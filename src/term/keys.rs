/// Whether a key went down, repeated, or came up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Press,
    Repeat,
    Release,
}

/// A key as gpui names it, what it types, and the modifiers that count.
#[derive(Clone, Copy, Debug)]
pub struct Key<'a> {
    /// gpui's name: `a`, `!`, `space`, `enter`, `f5`.
    pub name: &'a str,
    /// What the key types; empty when it types nothing.
    pub text: &'a str,
    pub shift: bool,
    pub ctrl: bool,
    /// Alt as a modifier, not as the key that composes characters.
    pub alt: bool,
    pub action: Action,
}

/// The kitty keyboard flags a program set.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Kitty {
    pub disambiguate: bool,
    pub events: bool,
    pub alternates: bool,
    pub all: bool,
    pub text: bool,
}

/// The bytes a key sends; none when it sends nothing or its text is the input method's to type.
pub fn encode(key: &Key, app_cursor: bool, kitty: Kitty) -> Option<Vec<u8>> {
    let on = kitty.disambiguate || kitty.events || kitty.alternates || kitty.all || kitty.text;
    if on {
        encode_kitty(key, kitty)
    } else {
        legacy(key, app_cursor)
    }
}

/// xterm's modifier code: one plus shift 1, alt 2, ctrl 4.
fn code(key: &Key) -> u32 {
    1 + u32::from(key.shift) + 2 * u32::from(key.alt) + 4 * u32::from(key.ctrl)
}

/// The character a key name stands for, `space` included.
fn printable(name: &str) -> Option<char> {
    let mut chars = name.chars();
    match (chars.next(), chars.next()) {
        (Some(ch), None) => Some(ch),
        _ => (name == "space").then_some(' '),
    }
}

fn legacy(key: &Key, app_cursor: bool) -> Option<Vec<u8>> {
    if key.action == Action::Release {
        return None;
    }
    if let Some(sequence) = function(key, app_cursor) {
        return Some(sequence);
    }
    let ch = printable(key.name)?;
    if key.ctrl {
        if let Some(byte) = control(ch, key.shift) {
            return Some(if key.alt {
                vec![0x1B, byte]
            } else {
                vec![byte]
            });
        }
        return Some(format!("\x1b[{};{}u", ch as u32, code(key)).into_bytes());
    }
    if key.alt {
        let mut bytes = vec![0x1B];
        if key.text.is_empty() {
            let typed = if key.shift {
                ch.to_ascii_uppercase()
            } else {
                ch
            };
            bytes.extend(typed.to_string().as_bytes());
        } else {
            bytes.extend(key.text.as_bytes());
        }
        return Some(bytes);
    }
    None
}

/// xterm's bytes for a function, cursor or editing key.
fn function(key: &Key, app_cursor: bool) -> Option<Vec<u8>> {
    let mods = code(key);
    let modified = mods > 1;
    let cursor = |end: char| match (modified, app_cursor) {
        (true, _) => format!("\x1b[1;{mods}{end}"),
        (false, true) => format!("\x1bO{end}"),
        (false, false) => format!("\x1b[{end}"),
    };
    let tilde = |n: u32| {
        if modified {
            format!("\x1b[{n};{mods}~")
        } else {
            format!("\x1b[{n}~")
        }
    };
    let ss3 = |end: char| {
        if modified {
            format!("\x1b[1;{mods}{end}")
        } else {
            format!("\x1bO{end}")
        }
    };
    let only = (key.shift, key.alt, key.ctrl);
    let sequence = match key.name {
        "up" => cursor('A'),
        "down" => cursor('B'),
        "right" => cursor('C'),
        "left" => cursor('D'),
        "home" => cursor('H'),
        "end" => cursor('F'),
        "insert" => tilde(2),
        "delete" => tilde(3),
        "pageup" => tilde(5),
        "pagedown" => tilde(6),
        "f1" => ss3('P'),
        "f2" => ss3('Q'),
        "f3" if modified => tilde(13),
        "f3" => ss3('R'),
        "f4" => ss3('S'),
        "enter" => match only {
            (false, false, false) => "\r".into(),
            (false, true, false) => "\x1b\r".into(),
            _ => format!("\x1b[27;{mods};13~"),
        },
        "tab" => match only {
            (false, false, false) => "\t".into(),
            (true, false, false) => "\x1b[Z".into(),
            (false, true, false) => "\x1b\t".into(),
            _ => format!("\x1b[27;{mods};9~"),
        },
        "escape" => match only {
            (false, false, false) => "\x1b".into(),
            (false, true, false) => "\x1b\x1b".into(),
            _ => format!("\x1b[27;{mods};27~"),
        },
        "backspace" => {
            let byte = if key.ctrl { 0x08 } else { 0x7F };
            return Some(if key.alt {
                vec![0x1B, byte]
            } else {
                vec![byte]
            });
        }
        name => tilde(FUNCTION_TILDES.iter().find(|(f, _)| *f == name)?.1),
    };
    Some(sequence.into_bytes())
}

/// F5 to F25 and the numbers xterm sends them with.
const FUNCTION_TILDES: [(&str, u32); 21] = [
    ("f5", 15),
    ("f6", 17),
    ("f7", 18),
    ("f8", 19),
    ("f9", 20),
    ("f10", 21),
    ("f11", 23),
    ("f12", 24),
    ("f13", 25),
    ("f14", 26),
    ("f15", 28),
    ("f16", 29),
    ("f17", 31),
    ("f18", 32),
    ("f19", 33),
    ("f20", 34),
    ("f21", 42),
    ("f22", 43),
    ("f23", 44),
    ("f24", 45),
    ("f25", 46),
];

/// The C0 byte Ctrl turns a key into, as kitty and Ghostty do; `[`, `i` and `m` go as CSI u.
fn control(ch: char, shift: bool) -> Option<u8> {
    if shift && ch != '@' {
        return None;
    }
    Some(match ch {
        ' ' | '2' | '@' => 0,
        'a'..='h' | 'j' | 'k' | 'l' | 'n'..='z' => ch as u8 - b'a' + 1,
        '3' => 27,
        '4' | '\\' => 28,
        '5' | ']' => 29,
        '6' | '^' | '~' => 30,
        '7' | '/' | '_' => 31,
        '8' | '?' => 127,
        '0' | '1' | '9' => ch as u8,
        _ => return None,
    })
}

/// kitty's functional keys: the number and the final byte.
fn functional(name: &str) -> Option<(u32, char)> {
    Some(match name {
        "escape" => (27, 'u'),
        "enter" => (13, 'u'),
        "tab" => (9, 'u'),
        "backspace" => (127, 'u'),
        "insert" => (2, '~'),
        "delete" => (3, '~'),
        "left" => (1, 'D'),
        "right" => (1, 'C'),
        "up" => (1, 'A'),
        "down" => (1, 'B'),
        "pageup" => (5, '~'),
        "pagedown" => (6, '~'),
        "home" => (1, 'H'),
        "end" => (1, 'F'),
        "f1" => (1, 'P'),
        "f2" => (1, 'Q'),
        "f3" => (13, '~'),
        "f4" => (1, 'S'),
        name => match FUNCTION_TILDES.iter().position(|(f, _)| *f == name)? {
            index @ 0..=7 => (FUNCTION_TILDES[index].1, '~'),
            index => (57376 + index as u32 - 8, 'u'),
        },
    })
}

fn encode_kitty(key: &Key, kitty: Kitty) -> Option<Vec<u8>> {
    let control_text = key.text.chars().any(char::is_control);
    if key.action == Action::Release {
        let legacy = matches!(key.name, "enter" | "tab" | "backspace");
        if !kitty.events || (legacy && !kitty.all) {
            return None;
        }
    }
    let shift_typed = key.shift && !key.text.is_empty() && !control_text;
    let bound = key.ctrl || key.alt || (key.shift && !shift_typed);
    if !kitty.all && !bound && key.action != Action::Release {
        match key.name {
            "enter" => return Some(b"\r".to_vec()),
            "tab" => return Some(b"\t".to_vec()),
            "backspace" => return Some(vec![0x7F]),
            _ if !key.text.is_empty() && !control_text => return None,
            _ => {}
        }
    }
    let (number, end) = functional(key.name)
        .or_else(|| printable(key.name).map(|ch| (ch.to_ascii_lowercase() as u32, 'u')))?;
    let mods = code(key);
    let event = match key.action {
        Action::Press => 1,
        Action::Repeat => 2,
        Action::Release => 3,
    };
    let event = if kitty.events { event } else { 1 };
    let mut out = String::from("\x1b[");
    if end != 'u' && end != '~' {
        if event > 1 {
            out.push_str(&format!("1;{mods}:{event}"));
        } else if mods > 1 {
            out.push_str(&format!("1;{mods}"));
        }
        out.push(end);
        return Some(out.into_bytes());
    }
    out.push_str(&number.to_string());
    let shifted = key.text.chars().next().filter(|&typed| {
        kitty.alternates && key.shift && typed as u32 != number && !typed.is_control()
    });
    if let Some(shifted) = shifted {
        out.push_str(&format!(":{}", shifted as u32));
    }
    let wrote_mods = event > 1 || mods > 1;
    if event > 1 {
        out.push_str(&format!(";{mods}:{event}"));
    } else if mods > 1 {
        out.push_str(&format!(";{mods}"));
    }
    let text: Vec<String> = key
        .text
        .chars()
        .filter(|ch| !ch.is_control())
        .map(|ch| (ch as u32).to_string())
        .collect();
    if kitty.text && key.action != Action::Release && !key.ctrl && !key.alt && !text.is_empty() {
        out.push_str(if wrote_mods { ";" } else { ";;" });
        out.push_str(&text.join(":"));
    }
    out.push(end);
    Some(out.into_bytes())
}

#[cfg(test)]
mod tests;
