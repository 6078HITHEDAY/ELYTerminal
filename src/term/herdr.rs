use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::{self, BufRead, BufReader, Write},
    os::unix::{ffi::OsStringExt, net::UnixStream},
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context as _;
use gpui::{Context, Task};
use serde::Deserialize;

use super::view::{TerminalEvent, TerminalView};

/// How often the session's foreground program is checked.
const CHECK: Duration = Duration::from_millis(500);

/// How long herdr may take to answer.
const WAIT: Duration = Duration::from_secs(1);

/// The socket request for herdr's session: its focus and its panes' folders.
const SNAPSHOT: &[u8] = b"{\"id\":\"elyterminal\",\"method\":\"session.snapshot\",\"params\":{}}\n";

/// A program's arguments, and its variables as the kernel holds them.
type Invocation = (Vec<String>, HashMap<String, OsString>);

/// Follows the pane herdr's server has focused while herdr's client runs in the foreground.
pub fn follow(cx: &mut Context<TerminalView>) -> Task<()> {
    cx.spawn(async move |view, cx| {
        let mut seen: Option<(i32, Option<PathBuf>)> = None;
        let (mut socket, mut shown, mut retry, mut failed) = (None, None, false, String::new());
        loop {
            cx.background_executor().timer(CHECK).await;
            let Ok((pid, group)) = view.update(cx, |view, _| (view.pty.pid, view.pty.foreground()))
            else {
                break;
            };
            let group = match group {
                Ok(group) => Some(group),
                Err(error) => {
                    if seen.is_some() {
                        log::debug!("terminal {pid}: its foreground does not read: {error}");
                    }
                    None
                }
            };
            // An exec keeps the group and swaps the program, so the two name the foreground.
            let program = match group {
                Some(group) => {
                    let exe = cx
                        .background_executor()
                        .spawn(async move { path(group).ok() })
                        .await;
                    Some((group, exe))
                }
                None => None,
            };
            if program != seen {
                if let Some((group, None)) = &program {
                    log::debug!(
                        "terminal {pid}: process {group}, another user's or gone, does not read"
                    );
                }
                seen = program.clone();
                shown = None;
                retry = true;
                failed.clear();
                if socket.take().is_some() {
                    log::info!("terminal {pid}: herdr left the foreground");
                }
            }
            if retry {
                let found = match program {
                    Some((group, Some(exe)))
                        if exe.file_name().is_some_and(|name| name == "herdr") =>
                    {
                        cx.background_executor()
                            .spawn(async move { client(group) })
                            .await
                    }
                    _ => Ok(None),
                };
                retry = found.is_err();
                socket = found.unwrap_or_else(|error| {
                    failure(&mut failed, pid, &error);
                    None
                });
                if let Some(socket) = &socket {
                    log::info!(
                        "terminal {pid}: herdr runs here, its server at {}",
                        socket.display()
                    );
                }
            }
            let Some(path) = socket.clone() else {
                continue;
            };
            match cx
                .background_executor()
                .spawn(async move { folder(&path) })
                .await
            {
                Ok(Some(folder)) if shown.as_ref() != Some(&folder) => {
                    failed.clear();
                    let report = folder.clone();
                    // herdr may have left while it answered; the shell's own report then stands.
                    let Ok(still) = view.update(cx, |view, cx| {
                        let still = view.pty.foreground().ok() == group;
                        if still {
                            cx.emit(TerminalEvent::Folder(report));
                        }
                        still
                    }) else {
                        break;
                    };
                    if still {
                        log::info!(
                            "terminal {pid}: herdr's focused pane is in {}",
                            folder.display()
                        );
                        shown = Some(folder);
                    }
                }
                Ok(_) => failed.clear(),
                Err(error) => failure(&mut failed, pid, &error),
            }
        }
    })
}

/// Logs `error` once while it lasts; a server not up yet is no error.
fn failure(failed: &mut String, pid: u32, error: &anyhow::Error) {
    let text = format!("{error:#}");
    if text != *failed {
        let level = if starting(error) {
            log::Level::Info
        } else {
            log::Level::Error
        };
        log::log!(level, "terminal {pid}: {text}; the panel keeps its folder");
        *failed = text;
    }
}

/// herdr's client runs before its server listens: no socket yet, or none answering.
fn starting(error: &anyhow::Error) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| {
        matches!(
            error.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::ConnectionRefused
        )
    })
}

/// The socket of the herdr server whose terminal client leads process group `group`, if it is one.
fn client(group: i32) -> anyhow::Result<Option<PathBuf>> {
    let (args, env) =
        launch(group).with_context(|| format!("herdr: process {group} does not read"))?;
    let Some(socket) = follows(&args, &env)? else {
        return Ok(None);
    };
    if socket.is_absolute() {
        return Ok(Some(socket));
    }
    // herdr reads a relative socket from its client's folder.
    let folder =
        cwd(group).with_context(|| format!("herdr: process {group}'s folder does not read"))?;
    Ok(Some(folder.join(socket)))
}

/// The socket a herdr process with `args` and `env` shows, if it is herdr's terminal client.
pub(super) fn follows(
    args: &[String],
    env: &HashMap<String, OsString>,
) -> anyhow::Result<Option<PathBuf>> {
    if !attaches(args) {
        return Ok(None);
    }
    let remote = |arg: &String| {
        ["--remote", "--machine"]
            .iter()
            .any(|flag| arg == flag || arg.starts_with(&format!("{flag}=")))
    };
    anyhow::ensure!(
        !args.iter().any(remote),
        "herdr: its client shows another machine's server, whose folders cannot show here"
    );
    socket(args, env).map(Some)
}

/// Whether `args` start herdr's terminal client, not one of its commands.
fn attaches(args: &[String]) -> bool {
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--session" | "--remote" | "--machine" | "--remote-keybindings" => {
                rest.next();
            }
            "--version" | "-V" | "--help" | "-h" | "--default-config" | "--skill" => return false,
            "client" => return true,
            "session" => return rest.next().is_some_and(|next| next == "attach"),
            flag if flag.starts_with('-') => {}
            _ => return false,
        }
    }
    true
}

/// The socket herdr 0.9's client finds: a named session, `HERDR_SOCKET_PATH`, `HERDR_SESSION`, the default.
fn socket(args: &[String], env: &HashMap<String, OsString>) -> anyhow::Result<PathBuf> {
    let dir = match (env.get("XDG_CONFIG_HOME"), env.get("HOME")) {
        (Some(config), _) => PathBuf::from(config).join("herdr"),
        (None, Some(home)) => PathBuf::from(home).join(".config/herdr"),
        (None, None) => anyhow::bail!("herdr: its client has no HOME"),
    };
    let named = |name: &OsStr| {
        if name == "default" {
            dir.join("herdr.sock")
        } else {
            dir.join("sessions").join(name).join("herdr.sock")
        }
    };
    if let Some(name) = session(args) {
        return Ok(named(OsStr::new(&name)));
    }
    if let Some(path) = env.get("HERDR_SOCKET_PATH") {
        return Ok(PathBuf::from(path));
    }
    Ok(env
        .get("HERDR_SESSION")
        .map_or_else(|| dir.join("herdr.sock"), |name| named(name)))
}

/// The session `session attach NAME`, `--session NAME` or `--session=NAME` names, as herdr reads it.
fn session(args: &[String]) -> Option<String> {
    if let [_, noun, verb, name] = args
        && noun == "session"
        && verb == "attach"
    {
        return Some(name.clone());
    }
    let mut named = None;
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        if arg == "--" {
            break;
        }
        if arg == "--session" {
            named = rest.next().cloned();
        } else if let Some(name) = arg.strip_prefix("--session=") {
            named = Some(name.to_owned());
        }
    }
    named
}

/// The folder of the pane herdr's server has focused; none while it has none or knows none.
fn folder(socket: &Path) -> anyhow::Result<Option<PathBuf>> {
    let ask = || -> io::Result<String> {
        let mut stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(WAIT))?;
        stream.set_write_timeout(Some(WAIT))?;
        stream.write_all(SNAPSHOT)?;
        let mut reply = String::new();
        BufReader::new(stream).read_line(&mut reply)?;
        Ok(reply)
    };
    let reply = ask().with_context(|| format!("herdr: asking {}", socket.display()))?;
    focused(&reply).with_context(|| format!("herdr: {}", socket.display()))
}

#[derive(Deserialize)]
struct Reply {
    result: Option<Snapshot>,
    error: Option<Failure>,
}

#[derive(Deserialize)]
struct Snapshot {
    snapshot: Session,
}

#[derive(Deserialize)]
struct Session {
    focused_pane_id: Option<String>,
    panes: Vec<Pane>,
}

#[derive(Deserialize)]
struct Pane {
    pane_id: String,
    cwd: Option<PathBuf>,
}

#[derive(Deserialize)]
struct Failure {
    code: String,
    message: String,
}

/// The focused pane's folder in herdr's `session.snapshot` reply.
pub(super) fn focused(reply: &str) -> anyhow::Result<Option<PathBuf>> {
    let reply: Reply = serde_json::from_str(reply).context("its reply does not parse")?;
    if let Some(Failure { code, message }) = reply.error {
        anyhow::bail!("{code}: {message}");
    }
    let session = reply.result.context("its reply holds no result")?.snapshot;
    let Some(id) = session.focused_pane_id else {
        return Ok(None);
    };
    let pane = session
        .panes
        .into_iter()
        .find(|pane| pane.pane_id == id)
        .with_context(|| format!("its focused pane {id} is not among its panes"))?;
    if let Some(cwd) = &pane.cwd {
        anyhow::ensure!(
            cwd.is_absolute(),
            "pane {id}'s folder {} is relative",
            cwd.display()
        );
    }
    Ok(pane.cwd)
}

#[cfg(target_os = "linux")]
fn path(pid: i32) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/exe")).map(installed)
}

/// The path of an executable the kernel marks ` (deleted)`, as an update that replaced it leaves it.
#[cfg(target_os = "linux")]
pub(super) fn installed(path: PathBuf) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    match path.as_os_str().as_bytes().strip_suffix(b" (deleted)") {
        Some(kept) => PathBuf::from(OsStr::from_bytes(kept)),
        None => path,
    }
}

#[cfg(target_os = "linux")]
fn launch(pid: i32) -> io::Result<Invocation> {
    let read = |name: &str| std::fs::read(format!("/proc/{pid}/{name}"));
    let (args, env) = (read("cmdline")?, read("environ")?);
    let args = strings(&args).map(lossy).collect();
    let env = strings(&env).filter_map(variable).collect();
    Ok((args, env))
}

#[cfg(target_os = "linux")]
fn cwd(pid: i32) -> io::Result<PathBuf> {
    std::fs::read_link(format!("/proc/{pid}/cwd"))
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
fn path(pid: i32) -> io::Result<PathBuf> {
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

#[cfg(target_os = "macos")]
fn launch(pid: i32) -> io::Result<Invocation> {
    let mut name = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut ask = |buffer: &mut Vec<u8>| {
        let mut size = buffer.len();
        let into = if buffer.is_empty() {
            std::ptr::null_mut()
        } else {
            buffer.as_mut_ptr().cast()
        };
        // SAFETY: `into` holds `size` bytes, or is null to ask for the size.
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

#[cfg(target_os = "macos")]
fn cwd(pid: i32) -> io::Result<PathBuf> {
    // SAFETY: the struct is plain C data, valid zeroed.
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_vnodepathinfo>() as libc::c_int;
    // SAFETY: `info` holds the size given.
    let read = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    if read <= 0 {
        return Err(io::Error::last_os_error());
    }
    if read != size {
        return Err(io::Error::other(format!(
            "proc_pidinfo read {read} of {size} bytes"
        )));
    }
    let path = info
        .pvi_cdir
        .vip_path
        .iter()
        .flatten()
        .map(|byte| *byte as u8);
    Ok(PathBuf::from(OsString::from_vec(
        path.take_while(|byte| *byte != 0).collect(),
    )))
}

/// `KERN_PROCARGS2`'s layout: argc, the program's path, NUL padding, the arguments, the variables.
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

#[cfg(test)]
mod tests;
