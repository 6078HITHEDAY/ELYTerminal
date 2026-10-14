use std::{io, path::PathBuf, sync::Arc};

use alacritty_terminal::{
    event::{OnResize, WindowSize},
    tty::{ChildEvent, EventedPty, EventedReadWrite},
};
use futures::channel::mpsc::UnboundedSender;
use polling::{Event, PollMode, Poller};

use super::pty::PtyEvent;

/// The longest OSC body kept; longer ones are dropped whole.
const MOST: usize = 4096;

/// A pty whose reader hands each byte to an OSC 7 scanner before the parser sees it.
pub(crate) struct Tapped<P> {
    pub inner: P,
    scanner: Osc7,
    events: UnboundedSender<PtyEvent>,
}

impl<P> Tapped<P> {
    pub fn new(inner: P, events: UnboundedSender<PtyEvent>) -> Self {
        Self {
            inner,
            scanner: Osc7::default(),
            events,
        }
    }
}

impl<P: EventedReadWrite> io::Read for Tapped<P> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let count = self.inner.reader().read(buf)?;
        let events = &self.events;
        self.scanner.feed(&buf[..count], &mut |body| {
            if let Some(folder) = folder(body)
                && events.unbounded_send(PtyEvent::Folder(folder)).is_err()
            {
                log::debug!("pty: a folder report after its session closed");
            }
        });
        Ok(count)
    }
}

impl<P: EventedReadWrite> EventedReadWrite for Tapped<P> {
    type Reader = Self;
    type Writer = P::Writer;

    unsafe fn register(
        &mut self,
        poll: &Arc<Poller>,
        event: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        unsafe { self.inner.register(poll, event, mode) }
    }

    fn reregister(&mut self, poll: &Arc<Poller>, event: Event, mode: PollMode) -> io::Result<()> {
        self.inner.reregister(poll, event, mode)
    }

    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
        self.inner.deregister(poll)
    }

    fn reader(&mut self) -> &mut Self {
        self
    }

    fn writer(&mut self) -> &mut P::Writer {
        self.inner.writer()
    }
}

impl<P: EventedPty> EventedPty for Tapped<P> {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        self.inner.next_child_event()
    }
}

impl<P: OnResize> OnResize for Tapped<P> {
    fn on_resize(&mut self, size: WindowSize) {
        self.inner.on_resize(size);
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum State {
    #[default]
    Ground,
    Escape,
    Osc,
    OscEscape,
}

/// Finds `ESC ] 7 ; body` ended by BEL or `ESC \`, across reads.
#[derive(Default)]
pub(crate) struct Osc7 {
    state: State,
    body: Vec<u8>,
    overflow: bool,
}

impl Osc7 {
    pub fn feed(&mut self, bytes: &[u8], found: &mut impl FnMut(&[u8])) {
        let mut rest = bytes;
        while !rest.is_empty() {
            if self.state == State::Ground {
                match rest.iter().position(|&byte| byte == 0x1b) {
                    Some(at) => {
                        self.state = State::Escape;
                        rest = &rest[at + 1..];
                    }
                    None => return,
                }
                continue;
            }
            let byte = rest[0];
            rest = &rest[1..];
            self.state = match (self.state, byte) {
                (State::Escape, b']') => {
                    self.body.clear();
                    self.overflow = false;
                    State::Osc
                }
                (State::Escape, 0x1b) => State::Escape,
                (State::Escape, _) => State::Ground,
                (State::Osc, 0x07) | (State::OscEscape, b'\\') => {
                    self.finish(found);
                    State::Ground
                }
                (State::Osc | State::OscEscape, 0x18 | 0x1a) => State::Ground,
                (State::Osc, 0x1b) => State::OscEscape,
                (State::OscEscape, b']') => {
                    self.body.clear();
                    self.overflow = false;
                    State::Osc
                }
                (State::OscEscape, 0x1b) => State::Escape,
                (State::OscEscape, _) => State::Ground,
                (State::Osc, byte) => {
                    if self.body.len() < MOST {
                        self.body.push(byte);
                    } else {
                        self.overflow = true;
                    }
                    State::Osc
                }
                (State::Ground, _) => unreachable!("ground scans by position"),
            };
        }
    }

    fn finish(&mut self, found: &mut impl FnMut(&[u8])) {
        if self.overflow {
            log::warn!("pty: an OSC over {MOST} bytes dropped");
        } else if let Some(body) = self.body.strip_prefix(b"7;") {
            found(body);
        }
        self.body.clear();
    }
}

/// The local folder an OSC 7 `file://host/path` names; another host's folder is none.
pub(crate) fn folder(body: &[u8]) -> Option<PathBuf> {
    let Some(rest) = body.strip_prefix(b"file://") else {
        log::warn!(
            "pty: OSC 7 without file:// ({})",
            String::from_utf8_lossy(body)
        );
        return None;
    };
    let slash = rest.iter().position(|&byte| byte == b'/')?;
    let host = String::from_utf8_lossy(&rest[..slash]);
    let local = gethostname::gethostname();
    if !(host.is_empty()
        || host.eq_ignore_ascii_case("localhost")
        || local
            .to_str()
            .is_some_and(|name| host.eq_ignore_ascii_case(name)))
    {
        log::debug!("pty: OSC 7 from host {host}, not this one");
        return None;
    }
    let path = match decode(&rest[slash..]) {
        Some(path) => path,
        None => {
            log::warn!("pty: OSC 7 path is not UTF-8 after decoding");
            return None;
        }
    };
    let drive = path.as_bytes().get(1..3).is_some_and(|two| two[1] == b':')
        && path.as_bytes()[1].is_ascii_alphabetic();
    Some(PathBuf::from(if cfg!(windows) && drive {
        &path[1..]
    } else {
        &path[..]
    }))
}

fn decode(bytes: &[u8]) -> Option<String> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        let hex = bytes
            .get(at + 1..at + 3)
            .and_then(|two| std::str::from_utf8(two).ok())
            .and_then(|two| u8::from_str_radix(two, 16).ok());
        match (bytes[at], hex) {
            (b'%', Some(value)) => {
                out.push(value);
                at += 3;
            }
            (byte, _) => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(chunks: &[&[u8]]) -> Vec<String> {
        let mut scanner = Osc7::default();
        let mut found = Vec::new();
        for chunk in chunks {
            scanner.feed(chunk, &mut |body| {
                found.push(String::from_utf8(body.to_vec()).expect("utf-8"))
            });
        }
        found
    }

    #[test]
    fn finds_osc_7_ended_by_bel_or_st_across_reads() {
        assert_eq!(scan(&[b"ab\x1b]7;file:///a\x07cd"]), ["file:///a"]);
        assert_eq!(scan(&[b"\x1b]7;file:", b"///b\x1b", b"\\"]), ["file:///b"]);
        assert_eq!(scan(&[b"\x1b", b"]7;x\x07\x1b]7;y\x1b\\"]), ["x", "y"]);
    }

    #[test]
    fn other_sequences_and_cancels_pass() {
        assert!(scan(&[b"\x1b]0;title\x07\x1b[31m"]).is_empty());
        assert!(scan(&[b"\x1b]7;file:///a\x18\x07"]).is_empty());
        assert_eq!(scan(&[b"\x1b]7;lost\x1b]7;kept\x07"]), ["kept"]);
    }

    #[test]
    fn an_overlong_body_is_dropped() {
        let mut long = b"\x1b]7;".to_vec();
        long.extend(std::iter::repeat_n(b'a', MOST + 1));
        long.push(0x07);
        assert!(scan(&[&long]).is_empty());
        assert_eq!(scan(&[&long, b"\x1b]7;b\x07"]), ["b"]);
    }

    #[test]
    fn a_local_uri_gives_its_decoded_folder() {
        assert_eq!(
            folder(b"file:///tmp/ely%20dir"),
            Some(PathBuf::from("/tmp/ely dir"))
        );
        assert_eq!(folder(b"file://localhost/x"), Some(PathBuf::from("/x")));
        let host = gethostname::gethostname().into_string().expect("host");
        let named = format!("file://{host}/y");
        assert_eq!(folder(named.as_bytes()), Some(PathBuf::from("/y")));
        assert_eq!(
            folder("file:///%E4%B8%AD".as_bytes()),
            Some(PathBuf::from("/中"))
        );
    }

    #[test]
    fn other_hosts_and_bad_uris_give_none() {
        assert_eq!(folder(b"file://elsewhere.invalid/tmp"), None);
        assert_eq!(folder(b"http://x/y"), None);
        assert_eq!(folder(b"file:///%FF"), None);
    }
}
