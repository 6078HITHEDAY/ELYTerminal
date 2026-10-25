use super::Holder;

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

/// Whether `group`'s leader is zsh, bash or fish, started interactive.
#[cfg(unix)]
pub(super) fn interactive(group: i32, pid: u32) -> bool {
    use crate::term::{shell, unix};
    match unix::path(group) {
        Ok(exe) if !shell::job_control(&exe) => false,
        Ok(_) => match unix::launch(group) {
            // An operand names a script or `-c`'s command.
            Ok((args, _)) => args.iter().skip(1).all(|arg| arg.starts_with('-')),
            Err(error) => {
                log::warn!("pty {pid}: group {group}'s arguments do not read: {error}");
                false
            }
        },
        Err(error) => {
            log::debug!("pty {pid}: group {group}'s leader, gone or another user's: {error}");
            false
        }
    }
}

/// The holder of an unnamed prompt, by `group` and `interactive`.
pub(super) fn prompter(
    group: Option<i32>,
    pid: u32,
    named: bool,
    interactive: &dyn Fn(i32) -> bool,
) -> Holder {
    match group {
        // Its jobs hold groups of their own, so it prompts.
        Some(group) if u32::try_from(group) != Ok(pid) && interactive(group) => Holder::Shell,
        Some(group) if u32::try_from(group) != Ok(pid) => Holder::Group(group),
        Some(_) if !named => Holder::Shell,
        _ => Holder::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unnamed_prompt_is_held_by_its_group_the_shell_or_no_one_known() {
        let (shell, program) = (|_: i32| true, |_: i32| false);
        // fish's or another unnamed prompt, held by the shell.
        assert_eq!(prompter(Some(10), 10, false, &program), Holder::Shell);
        // A program's prompt under PowerShell, which shares its group.
        assert_eq!(prompter(Some(10), 10, true, &shell), Holder::Unknown);
        // omp's, in a group of its own.
        assert_eq!(prompter(Some(20), 10, true, &program), Holder::Group(20));
        // A nested interactive shell's, in a group of its own.
        assert_eq!(prompter(Some(20), 10, true, &shell), Holder::Shell);
        assert_eq!(prompter(None, 10, false, &shell), Holder::Unknown);
    }

    #[cfg(unix)]
    #[test]
    fn only_zsh_bash_or_fish_with_options_alone_reads_interactive() {
        use std::{
            os::unix::process::CommandExt,
            process::{Command, Stdio},
        };
        let read = |program: &str, args: &[&str]| {
            let mut child = Command::new(program)
                .args(args)
                .process_group(0)
                .stdin(Stdio::piped())
                .spawn()
                .expect("it starts");
            let read = interactive(child.id() as i32, 0);
            child.kill().expect("it stops");
            child.wait().expect("it ends");
            read
        };
        assert!(read("bash", &["-s"]));
        assert!(read("zsh", &["-s"]));
        assert!(!read("bash", &["-c", "read -r line"]));
        assert!(!read("bash", &["-s", "script"]));
        assert!(!read("sleep", &["30"]));
    }
}
