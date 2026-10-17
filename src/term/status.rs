use base64::{
    Engine,
    alphabet::STANDARD,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};

/// What a program says it is doing, OSC 7501.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Idle,
    Working,
    Done,
    Blocked,
    Error,
}

/// What a blocked program waits for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Permission,
    Question,
    Auth,
}

/// A record, as the last report for its id left it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub state: State,
    pub kind: Option<Kind>,
    pub progress: Option<u8>,
    pub app: Option<String>,
    pub title: Option<String>,
    pub msg: Option<String>,
}

/// An OSC 7501 sequence, checked whole; an id is empty for the root record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Report(String, Record),
    /// Drops the record at the id and those under it; with no id, all.
    Clear(Option<String>),
    /// Asks which states and kinds the terminal takes.
    Query,
}

/// The answer to the query: every state and kind this terminal takes.
pub const REPLY: &str =
    "\x1b]7501;?:states=idle,working,done,blocked,error:kinds=permission,question,auth\x1b\\";

/// The spec's limits: a whole sequence, a key, an app, an id, its segments and levels.
const SEQUENCE: usize = 4096;
const KEY: usize = 16;
const APP: usize = 32;
const ID: usize = 128;
const SEGMENT: usize = 32;
const LEVELS: usize = 8;

/// Records a terminal keeps; past them, the one updated longest ago goes.
pub const RECORDS: usize = 256;

/// Standard base64, its padding optional.
const BASE64: GeneralPurpose = GeneralPurpose::new(
    &STANDARD,
    GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// The sequence in an OSC 7501 body past `7501;`; none for one the spec drops, logged with why.
pub fn read(body: &[u8]) -> Option<Status> {
    parse(body)
        .inspect_err(|why| log::warn!("pty: OSC 7501 dropped: {why}"))
        .ok()
}

/// A value's bytes, as the spec allows them.
fn valued(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_.,+/=-".contains(&byte)
}

/// An id's or an app's bytes.
fn named(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_.+-".contains(&byte)
}

fn parse(body: &[u8]) -> Result<Status, String> {
    // ESC ] 7501 ; and an ST of two bytes frame the body.
    if body.len() + 9 > SEQUENCE {
        return Err(format!("over {SEQUENCE} bytes"));
    }
    if body.trim_ascii() == b"?" {
        return Ok(Status::Query);
    }
    let mut pairs = Vec::new();
    for pair in body.split(|&byte| byte == b':') {
        // A malformed pair is skipped: no `=`, no key, or a byte outside the set.
        let Some(at) = pair.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        let (key, value) = (pair[..at].trim_ascii(), pair[at + 1..].trim_ascii());
        if key.is_empty()
            || !key.iter().all(u8::is_ascii_lowercase)
            || !value.iter().all(|&byte| valued(byte))
        {
            continue;
        }
        if key.len() > KEY {
            return Err(format!("a key over {KEY} bytes"));
        }
        let ascii = |bytes| String::from_utf8(Vec::from(bytes)).expect("ASCII");
        pairs.push((ascii(key), ascii(value)));
    }
    // The last of a key wins.
    let get = |key: &str| {
        pairs
            .iter()
            .rev()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    };
    let id = get("id").map(id).transpose()?;
    let state = match get("state").ok_or("no state")? {
        "idle" => State::Idle,
        "working" => State::Working,
        "done" => State::Done,
        "blocked" => State::Blocked,
        "error" => State::Error,
        "clear" => return Ok(Status::Clear(id)),
        other => return Err(format!("state {other} is not the spec's")),
    };
    let kind = match (state, get("kind")) {
        (State::Blocked, Some("permission")) => Some(Kind::Permission),
        (State::Blocked, Some("question")) => Some(Kind::Question),
        (State::Blocked, Some("auth")) => Some(Kind::Auth),
        _ => None,
    };
    let progress = match state {
        State::Working | State::Blocked => get("progress")
            .filter(|value| value.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|value| value.parse::<u8>().ok())
            .filter(|&value| value <= 100),
        _ => None,
    };
    let app = match get("app") {
        Some(app) if app.len() > APP => return Err(format!("an app over {APP} bytes")),
        Some(app) if !app.is_empty() && app.bytes().all(named) => Some(app.to_string()),
        _ => None,
    };
    let title = get("title")
        .map(|value| text(value, "title", 256, 192))
        .transpose()?;
    let msg = get("msg")
        .map(|value| text(value, "msg", 2732, 2048))
        .transpose()?;
    let record = Record {
        state,
        kind,
        progress,
        app,
        title,
        msg,
    };
    Ok(Status::Report(id.unwrap_or_default(), record))
}

/// An id that keeps to the spec's grammar and limits.
fn id(value: &str) -> Result<String, String> {
    let segments: Vec<&str> = value.split('/').collect();
    let fits = value.len() <= ID
        && segments.len() <= LEVELS
        && segments
            .iter()
            .all(|segment| (1..=SEGMENT).contains(&segment.len()) && segment.bytes().all(named));
    if fits {
        Ok(value.to_string())
    } else {
        Err(format!("id {value} is not the spec's"))
    }
}

/// The text a base64 value names, one line, within `encoded` and `decoded` bytes.
fn text(value: &str, key: &str, encoded: usize, decoded: usize) -> Result<String, String> {
    if value.len() > encoded {
        return Err(format!("a {key} over {encoded} bytes"));
    }
    let bytes = BASE64
        .decode(value)
        .map_err(|error| format!("{key} is no base64: {error}"))?;
    if bytes.len() > decoded {
        return Err(format!("a {key} over {decoded} bytes decoded"));
    }
    let text = String::from_utf8(bytes).map_err(|_| format!("{key} is not UTF-8"))?;
    if text.chars().any(char::is_control) {
        return Err(format!("{key} holds a control character"));
    }
    Ok(text)
}

/// The records one terminal keeps, by id, each with when it was last reported.
#[derive(Default)]
pub struct Records {
    list: Vec<(String, Record, u64)>,
    clock: u64,
}

impl Records {
    /// Replaces the record at `id` whole.
    pub fn report(&mut self, id: String, record: Record) {
        self.clock += 1;
        if let Some(entry) = self.list.iter_mut().find(|(at, ..)| *at == id) {
            *entry = (id, record, self.clock);
            return;
        }
        if self.list.len() == RECORDS {
            let oldest = (0..self.list.len())
                .min_by_key(|&at| self.list[at].2)
                .expect("records");
            let (gone, ..) = self.list.remove(oldest);
            log::debug!("status: record {gone:?} goes, the one updated longest ago");
        }
        self.list.push((id, record, self.clock));
    }

    /// Drops the record at `id` and every one under it; with none, all.
    pub fn clear(&mut self, id: Option<&str>) {
        match id {
            None => self.list.clear(),
            Some(id) => {
                let under = format!("{id}/");
                self.list
                    .retain(|(at, ..)| at != id && !at.starts_with(&under));
            }
        }
    }

    /// A new prompt: working, blocked and idle records go, done and error stay.
    pub fn prompt(&mut self) {
        self.list
            .retain(|(_, record, _)| matches!(record.state, State::Done | State::Error));
    }
}

#[cfg(test)]
mod tests;
