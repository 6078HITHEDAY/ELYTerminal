use std::{
    collections::HashMap,
    ffi::{CStr, CString, OsString},
    fs::File,
    io::{self, ErrorKind, Read},
    mem::{ManuallyDrop, MaybeUninit},
    os::{
        fd::AsRawFd,
        unix::{
            ffi::{OsStrExt, OsStringExt},
            net::UnixStream,
            process::CommandExt,
        },
    },
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::Arc,
};

use alacritty_terminal::{
    event::{OnResize, WindowSize},
    tty::{ChildEvent, EventedPty, EventedReadWrite, ToWinsize},
};
use polling::{Event, PollMode, Poller};
use rustix_openpty::{
    openpty,
    rustix::termios::{self, InputModes, OptionalActions},
};
use signal_hook::{
    SigId,
    consts::SIGCHLD,
    low_level::{pipe, unregister},
};

/// The keys alacritty's event loop reads pty readiness by.
const READ_WRITE: usize = 0;
const CHILD: usize = 1;

/// A program on a pseudo-terminal, started here so a failed `chdir` fails the start.
pub(crate) struct Pty {
    child: Child,
    file: ManuallyDrop<File>,
    signals: UnixStream,
    sig: SigId,
}

impl Pty {
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// A copy of the master, which reads the foreground and, in tests, the modes.
    pub fn master(&self) -> io::Result<File> {
        self.file.try_clone()
    }
}

/// `master`'s foreground group; none once none holds the terminal.
pub(super) fn foreground(master: &File) -> io::Result<Option<i32>> {
    match unsafe { libc::tcgetpgrp(master.as_raw_fd()) } {
        -1 => Err(io::Error::last_os_error()),
        0 => Ok(None),
        group => Ok(Some(group)),
    }
}

/// A program's arguments and variables, as the system holds them.
type Invocation = (Vec<String>, HashMap<String, OsString>);

/// The executable process `pid` runs.
#[cfg(target_os = "linux")]
pub(super) fn path(pid: i32) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).map(installed)
}

/// `path` without the ` (deleted)` an update leaves.
#[cfg(target_os = "linux")]
pub(super) fn installed(path: PathBuf) -> PathBuf {
    match path.as_os_str().as_bytes().strip_suffix(b" (deleted)") {
        Some(kept) => PathBuf::from(std::ffi::OsStr::from_bytes(kept)),
        None => path,
    }
}

#[cfg(target_os = "macos")]
pub(super) fn path(pid: i32) -> io::Result<PathBuf> {
    let mut buffer = vec![0u8; libc::PROC_PIDPATHINFO_MAXSIZE as usize];
    // SAFETY: the buffer holds the size given.
    let length =
        unsafe { libc::proc_pidpath(pid, buffer.as_mut_ptr().cast(), buffer.len() as u32) };
    if length <= 0 {
        return Err(io::Error::last_os_error());
    }
    buffer.truncate(length as usize);
    Ok(PathBuf::from(OsString::from_vec(buffer)))
}

/// Process `pid`'s arguments and variables.
#[cfg(target_os = "linux")]
pub(super) fn launch(pid: i32) -> io::Result<Invocation> {
    let read = |name: &str| std::fs::read(format!("/proc/{pid}/{name}"));
    let (args, env) = (read("cmdline")?, read("environ")?);
    let args = strings(&args).map(lossy).collect();
    let env = strings(&env).filter_map(variable).collect();
    Ok((args, env))
}

/// Each NUL-ended string in `bytes`.
#[cfg(target_os = "linux")]
fn strings(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .strip_suffix(b"\0")
        .unwrap_or(bytes)
        .split(|byte| *byte == 0)
}

#[cfg(target_os = "macos")]
pub(super) fn launch(pid: i32) -> io::Result<Invocation> {
    let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut ask = |buffer: &mut Vec<u8>| {
        let mut size = buffer.len();
        let into = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        // SAFETY: `into` holds `size` bytes, or null asks the size.
        let done = unsafe {
            libc::sysctl(
                name.as_mut_ptr(),
                3,
                into,
                &mut size,
                std::ptr::null_mut(),
                0,
            )
        };
        if done != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(size)
    };
    let mut buffer = Vec::new();
    let size = ask(&mut buffer)?;
    buffer.resize(size, 0);
    let size = ask(&mut buffer)?;
    buffer.truncate(size);
    procargs(&buffer)
}

/// `KERN_PROCARGS2`: argc, path, NUL padding, arguments, then variables.
#[cfg(target_os = "macos")]
pub(super) fn procargs(buffer: &[u8]) -> io::Result<Invocation> {
    let invalid =
        |what: &str| io::Error::new(io::ErrorKind::InvalidData, format!("KERN_PROCARGS2 {what}"));
    let (count, rest) = buffer
        .split_first_chunk::<4>()
        .ok_or_else(|| invalid("holds no argument count"))?;
    let count = usize::try_from(i32::from_ne_bytes(*count))
        .map_err(|_| invalid("holds a negative argument count"))?;
    let mut parts = rest
        .split(|byte| *byte == 0)
        .skip(1)
        .skip_while(|part| part.is_empty());
    let args = parts.by_ref().take(count).map(lossy).collect();
    let env = parts
        .take_while(|part| !part.is_empty())
        .filter_map(variable)
        .collect();
    Ok((args, env))
}

fn lossy(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// A `NAME=value` variable; its value keeps its bytes.
fn variable(pair: &[u8]) -> Option<(String, OsString)> {
    let at = pair.iter().position(|byte| *byte == b'=')?;
    Some((
        lossy(&pair[..at]),
        OsString::from_vec(pair[at + 1..].to_vec()),
    ))
}

/// Starts `program`, or the login shell, in `cwd` on a new pty of `size`.
pub(crate) fn spawn(
    program: Option<&(String, Vec<String>)>,
    cwd: &Path,
    env: &HashMap<String, String>,
    size: WindowSize,
) -> io::Result<Pty> {
    let pair = openpty(None, Some(&size.to_winsize()))?;
    let (master, slave) = (pair.controller, pair.user);
    let mut modes = termios::tcgetattr(&master)?;
    modes.input_modes.set(InputModes::IUTF8, true);
    termios::tcsetattr(&master, OptionalActions::Now, &modes)?;
    let user = User::find()?;
    let mut command = match program {
        Some((program, args)) => {
            let mut command = Command::new(program);
            command.args(args);
            command
        }
        None => login_shell(&user),
    };
    command.stdin(slave.try_clone()?);
    command.stderr(slave.try_clone()?);
    command.stdout(slave.try_clone()?);
    command.env("USER", &user.name).env("HOME", &user.home);
    command.envs(env);
    command.env_remove("XDG_ACTIVATION_TOKEN");
    command.env_remove("DESKTOP_STARTUP_ID");
    let folder = CString::new(cwd.as_os_str().as_bytes())?;
    let (master_fd, slave_fd) = (master.as_raw_fd(), slave.as_raw_fd());
    // SAFETY: the hook calls only async-signal-safe libc functions.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() == -1 || libc::chdir(folder.as_ptr()) == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(slave_fd, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            libc::close(slave_fd);
            libc::close(master_fd);
            for signal in [
                libc::SIGCHLD,
                libc::SIGHUP,
                libc::SIGINT,
                libc::SIGQUIT,
                libc::SIGTERM,
                libc::SIGALRM,
            ] {
                libc::signal(signal, libc::SIG_DFL);
            }
            Ok(())
        });
    }
    let (sender, signals) = UnixStream::pair()?;
    let sig = pipe::register(SIGCHLD, sender)?;
    signals.set_nonblocking(true)?;
    let child = match command.spawn() {
        Ok(child) => child,
        Err(error) => {
            unregister(sig);
            return Err(error);
        }
    };
    drop(slave);
    let flags = unsafe { libc::fcntl(master_fd, libc::F_GETFL, 0) };
    if flags == -1
        || unsafe { libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(Pty {
        child,
        file: ManuallyDrop::new(File::from(master)),
        signals,
        sig,
    })
}

/// The user's login shell: SHELL, else the password entry's.
pub(crate) fn login_shell_path() -> io::Result<String> {
    Ok(User::find()?.shell)
}

/// macOS runs the shell through `login` so it is a login shell on a tty, as Terminal does.
#[cfg(target_os = "macos")]
fn login_shell(user: &User) -> Command {
    let name = user.shell.rsplit('/').next().unwrap_or(&user.shell);
    let hush = Path::new(&user.home).join(".hushlogin").exists();
    let mut command = Command::new("/usr/bin/login");
    command.args([
        if hush { "-qflp" } else { "-flp" },
        &user.name,
        "/bin/zsh",
        "-fc",
        &format!("exec -a -{name} {}", user.shell),
    ]);
    command
}

#[cfg(not(target_os = "macos"))]
fn login_shell(user: &User) -> Command {
    Command::new(&user.shell)
}

struct User {
    name: String,
    home: String,
    shell: String,
}

impl User {
    /// USER, HOME and SHELL, each from the password entry when the variable is unset.
    fn find() -> io::Result<Self> {
        let entry = || -> io::Result<(String, String, String)> {
            let mut buffer = vec![0 as libc::c_char; 4096];
            let mut entry = MaybeUninit::<libc::passwd>::uninit();
            let mut found = std::ptr::null_mut();
            let status = unsafe {
                libc::getpwuid_r(
                    libc::getuid(),
                    entry.as_mut_ptr(),
                    buffer.as_mut_ptr(),
                    buffer.len(),
                    &mut found,
                )
            };
            if status != 0 || found.is_null() {
                return Err(io::Error::other("pty: no password entry for this user"));
            }
            let entry = unsafe { entry.assume_init() };
            let text = |field: *const libc::c_char| {
                unsafe { CStr::from_ptr(field) }
                    .to_string_lossy()
                    .into_owned()
            };
            Ok((
                text(entry.pw_name),
                text(entry.pw_dir),
                text(entry.pw_shell),
            ))
        };
        let (name, home, shell) = match (
            std::env::var("USER"),
            std::env::var("HOME"),
            std::env::var("SHELL"),
        ) {
            (Ok(name), Ok(home), Ok(shell)) => (name, home, shell),
            (name, home, shell) => {
                let (pw_name, pw_home, pw_shell) = entry()?;
                (
                    name.unwrap_or(pw_name),
                    home.unwrap_or(pw_home),
                    shell.unwrap_or(pw_shell),
                )
            }
        };
        Ok(Self { name, home, shell })
    }
}

impl EventedReadWrite for Pty {
    type Reader = File;
    type Writer = File;

    unsafe fn register(
        &mut self,
        poll: &Arc<Poller>,
        mut interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        interest.key = READ_WRITE;
        unsafe {
            poll.add_with_mode(&*self.file, interest, mode)?;
            poll.add_with_mode(&self.signals, Event::readable(CHILD), PollMode::Level)
        }
    }

    fn reregister(
        &mut self,
        poll: &Arc<Poller>,
        mut interest: Event,
        mode: PollMode,
    ) -> io::Result<()> {
        interest.key = READ_WRITE;
        poll.modify_with_mode(&*self.file, interest, mode)?;
        poll.modify_with_mode(&self.signals, Event::readable(CHILD), PollMode::Level)
    }

    fn deregister(&mut self, poll: &Arc<Poller>) -> io::Result<()> {
        poll.delete(&*self.file)?;
        poll.delete(&self.signals)
    }

    fn reader(&mut self) -> &mut File {
        &mut self.file
    }

    fn writer(&mut self) -> &mut File {
        &mut self.file
    }
}

impl EventedPty for Pty {
    fn next_child_event(&mut self) -> Option<ChildEvent> {
        let mut byte = [0u8; 1];
        if let Err(error) = self.signals.read(&mut byte) {
            if error.kind() != ErrorKind::WouldBlock {
                log::error!("pty {}: the SIGCHLD pipe failed: {error}", self.id());
            }
            return None;
        }
        match self.child.try_wait() {
            Ok(None) => None,
            Ok(status) => Some(ChildEvent::Exited(status)),
            Err(error) => {
                log::error!(
                    "pty {}: cannot read the program's state: {error}",
                    self.id()
                );
                None
            }
        }
    }
}

impl OnResize for Pty {
    fn on_resize(&mut self, size: WindowSize) {
        let winsize = size.to_winsize();
        if unsafe { libc::ioctl(self.file.as_raw_fd(), libc::TIOCSWINSZ, &winsize) } == -1 {
            log::error!(
                "pty {}: resize failed: {}",
                self.id(),
                io::Error::last_os_error()
            );
        }
    }
}

impl Drop for Pty {
    fn drop(&mut self) {
        unsafe { libc::kill(self.child.id() as libc::pid_t, libc::SIGHUP) };
        unregister(self.sig);
        // Closed before the wait, so a program that ignores SIGHUP finds the terminal gone.
        unsafe { ManuallyDrop::drop(&mut self.file) };
        match self.child.wait() {
            Ok(status) => log::info!("pty {}: hung up, {status}", self.id()),
            Err(error) => log::error!("pty {}: cannot reap the program: {error}", self.id()),
        }
    }
}
