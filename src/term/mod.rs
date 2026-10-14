pub mod frame;
pub mod ink;
mod paint;
mod pty;
pub mod shell;
mod tap;
#[cfg(unix)]
mod unix;
mod view;

pub use pty::{Events, Launch, Listener, Pty, PtyEvent, config, fit, spawn};
pub use view::{ICONS, MONO, TerminalEvent, TerminalView};
