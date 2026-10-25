use super::{Holder, marks::Click};

/// The process group that holds the terminal now, by `master`.
#[cfg(unix)]
pub(super) fn holder(master: Option<&std::fs::File>, pid: u32) -> Option<i32> {
    match crate::term::unix::foreground(master?) {
        Ok(group) => group,
        Err(error) => {
            log::warn!("pty {pid}: no foreground group for a report: {error}");
            None
        }
    }
}

/// Whether zsh, bash or fish, started interactive, holds `group` alone.
#[cfg(unix)]
pub(super) fn alone(group: i32, pid: u32) -> bool {
    use crate::term::{shell, unix};
    let exe = match unix::path(group) {
        Ok(exe) => exe,
        Err(error) => {
            log::debug!("pty {pid}: group {group}'s leader, gone or another user's: {error}");
            return false;
        }
    };
    let Some(marks) = shell::option_marks(&exe) else {
        return false;
    };
    let interactive = match unix::launch(group) {
        // Linux shows no arguments while an exec loads.
        Ok((args, _)) if args.first().is_none_or(String::is_empty) => false,
        // An operand names a script or `-c`'s command.
        Ok((args, _)) => args[1..].iter().all(|arg| arg.starts_with(marks)),
        Err(error) => {
            log::warn!("pty {pid}: group {group}'s arguments do not read: {error}");
            false
        }
    };
    interactive
        && unix::lone(group).unwrap_or_else(|error| {
            log::warn!("pty {pid}: group {group}'s processes do not read: {error}");
            false
        })
}

/// An unnamed prompt's holder, by its click and `group`.
pub(super) fn prompter(
    click: Click,
    group: Option<i32>,
    pid: u32,
    named: bool,
    alone: &dyn Fn(i32) -> bool,
) -> Holder {
    let ours = |group: i32| u32::try_from(group) == Ok(pid);
    match group {
        // A line that takes clicks is a shell's own.
        _ if click != Click::Arrows => Holder::Shell,
        Some(group) if !ours(group) && alone(group) => Holder::Shell,
        Some(group) if !ours(group) => Holder::Group(group),
        Some(_) if !named => Holder::Shell,
        _ => Holder::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unnamed_prompt_is_held_by_its_click_or_its_group() {
        let (shell, program, plain) = (|_: i32| true, |_: i32| false, Click::Arrows);
        // fish's or another unnamed prompt, held by the shell.
        assert_eq!(
            prompter(plain, Some(10), 10, false, &program),
            Holder::Shell
        );
        // A program's prompt under PowerShell, which shares its group.
        assert_eq!(prompter(plain, Some(10), 10, true, &shell), Holder::Unknown);
        // omp's, in a group of its own.
        assert_eq!(
            prompter(plain, Some(20), 10, true, &program),
            Holder::Group(20)
        );
        // A nested interactive shell's, alone in its own group.
        assert_eq!(prompter(plain, Some(20), 10, true, &shell), Holder::Shell);
        assert_eq!(prompter(plain, None, 10, false, &shell), Holder::Unknown);
        // A line that takes clicks, as fish's, from any group.
        assert_eq!(
            prompter(Click::Absolute, Some(20), 10, true, &program),
            Holder::Shell
        );
        assert_eq!(
            prompter(Click::Relative, None, 10, true, &program),
            Holder::Shell
        );
    }

    #[cfg(unix)]
    #[test]
    fn only_zsh_bash_or_fish_started_interactive_reads_alone_in_its_group() {
        use std::{
            io::{BufRead, BufReader, Write},
            os::unix::process::CommandExt,
            process::{Command, Stdio},
        };
        // Each says `ready` once it runs, past Linux's exec.
        let read = |program: &str, args: &[&str], line: &[u8]| {
            let mut child = Command::new(program)
                .args(args)
                .process_group(0)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .spawn()
                .expect("it starts");
            child
                .stdin
                .as_mut()
                .expect("its input")
                .write_all(line)
                .expect("it reads");
            let mut ready = String::new();
            let output = child.stdout.as_mut().expect("its output");
            BufReader::new(output)
                .read_line(&mut ready)
                .expect("it says");
            assert_eq!(ready, "ready\n", "{program} {args:?}");
            let read = alone(child.id() as i32, 0);
            // SAFETY: a signal to the test's own group.
            unsafe { libc::kill(-(child.id() as i32), libc::SIGKILL) };
            child.wait().expect("it ends");
            read
        };
        assert!(read("bash", &["-s"], b"echo ready\n"));
        assert!(read("bash", &["+H", "-s"], b"echo ready\n"));
        assert!(read("zsh", &["-f", "-s"], b"echo ready\n"));
        // A command's or a script's operand.
        assert!(!read("bash", &["-c", "echo ready; read -r line"], b""));
        assert!(!read("bash", &["-s", "operand"], b"echo ready\n"));
        // A job beside the shell, as a subshell runs one.
        assert!(!read("bash", &["-s"], b"sleep 30 & echo ready\n"));
        // A program alone in its group.
        assert!(!read("sh", &["-c", "echo ready; exec sleep 30"], b""));
    }
}
