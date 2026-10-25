use std::path::Path;

use super::name;

/// How zsh, bash, fish, nu or PowerShell read their options.
pub(crate) struct Options {
    /// Characters that start an option.
    marks: &'static [char],
    /// Options whose value is the next argument.
    valued: &'static [&'static str],
    /// Names match ignoring case, as PowerShell's do.
    fold: bool,
}

impl Options {
    /// Whether `args`, past the program's name, hold options alone.
    pub(crate) fn interactive(&self, args: &[String]) -> bool {
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if !arg.starts_with(self.marks) {
                return false;
            }
            let valued = self.valued.iter().any(|name| {
                if self.fold {
                    name.eq_ignore_ascii_case(arg)
                } else {
                    name == arg
                }
            });
            if valued && args.next().is_none() {
                return false;
            }
        }
        true
    }
}

/// A shell's options, where it leads its own prompt; none elsewhere.
pub(crate) fn options(path: &Path) -> Option<Options> {
    let (marks, valued): (&'static [char], &'static [&'static str]) = match name(path).as_str() {
        "zsh" => (&['-', '+'], &["-o", "+o"]),
        "bash" => (
            &['-', '+'],
            &["-o", "+o", "-O", "+O", "--rcfile", "--init-file"],
        ),
        "fish" => (
            &['-'],
            &[
                "-C",
                "--init-command",
                "-d",
                "--debug",
                "-o",
                "--debug-output",
                "-p",
                "--profile",
                "--profile-startup",
                "-f",
                "--features",
            ],
        ),
        "nu" => (
            &['-'],
            &[
                "-e",
                "--execute",
                "-m",
                "--table-mode",
                "--error-style",
                "-I",
                "--include-path",
                "--config",
                "--env-config",
                "--config-home",
                "--log-level",
                "--log-target",
                "--log-file",
                "--log-include",
                "--log-exclude",
                "--experimental-options",
                "--plugin-config",
                "--plugins",
                "--mcp-transport",
                "--mcp-port",
                "--mcp-host",
            ],
        ),
        "pwsh" => (
            &['-'],
            &[
                "-ExecutionPolicy",
                "-ex",
                "-ep",
                "-WorkingDirectory",
                "-wd",
                "-ConfigurationName",
                "-config",
                "-ConfigurationFile",
                "-CustomPipeName",
                "-SettingsFile",
                "-settings",
                "-OutputFormat",
                "-o",
                "-of",
                "-InputFormat",
                "-inp",
                "-if",
                "-WindowStyle",
                "-w",
            ],
        ),
        _ => return None,
    };
    let fold = name(path) == "pwsh";
    Some(Options {
        marks,
        valued,
        fold,
    })
}

/// Whether `path` runs fish or nu, whose own lines mark clicks.
pub(crate) fn clicks(path: &Path) -> bool {
    matches!(name(path).as_str(), "fish" | "nu")
}
