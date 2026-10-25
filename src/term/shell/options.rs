use std::path::Path;

use super::name;

/// How a shell leading its own prompt reads its options.
pub(crate) struct Options {
    /// Characters that start an option.
    marks: &'static [char],
    /// Short options may share one argument, as `-lo vi`.
    bundles: bool,
    /// Short options taking a value, attached or the next argument.
    short: &'static str,
    /// Of those, the ones whose value may be left out.
    optional: &'static str,
    /// Short options that run a command, not a prompt.
    command_short: &'static str,
    /// Other options taking the next argument, unless `=` holds one.
    valued: &'static [&'static str],
    /// Other options that run a command or a file.
    command: &'static [&'static str],
    /// Names match ignoring case, as PowerShell's do.
    fold: bool,
}

impl Options {
    /// Whether `args`, past the program's name, start an interactive shell.
    pub(crate) fn interactive(&self, args: &[String]) -> bool {
        let same = |name: &str, arg: &str| {
            if self.fold {
                name.eq_ignore_ascii_case(arg)
            } else {
                name == arg
            }
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            // What follows the end of options is a script.
            if arg == "--" || arg == "-" {
                return args.next().is_none();
            }
            let Some(rest) = arg.strip_prefix(self.marks) else {
                return false;
            };
            if !self.bundles || arg.starts_with("--") {
                let name = arg.split('=').next().unwrap_or(arg);
                if self.command.iter().any(|command| same(command, name)) {
                    return false;
                }
                let valued = self.valued.iter().any(|valued| same(valued, name));
                if valued && !arg.contains('=') && args.next().is_none() {
                    return false;
                }
                continue;
            }
            for (at, option) in rest.char_indices() {
                if self.command_short.contains(option) {
                    return false;
                }
                if self.short.contains(option) {
                    let last = at + option.len_utf8() == rest.len();
                    if last && args.next().is_none() && !self.optional.contains(option) {
                        return false;
                    }
                    break;
                }
            }
        }
        true
    }
}

/// A shell's options where it leads its prompt; none elsewhere.
pub(crate) fn options(path: &Path) -> Option<Options> {
    let shell = name(path);
    let options = match shell.as_str() {
        "zsh" => Options {
            marks: &['-', '+'],
            bundles: true,
            short: "o",
            optional: "",
            command_short: "c",
            valued: &["--emulate"],
            command: &[],
            fold: false,
        },
        "bash" => Options {
            marks: &['-', '+'],
            bundles: true,
            short: "oO",
            optional: "oO",
            command_short: "c",
            valued: &["--rcfile", "--init-file"],
            command: &[],
            fold: false,
        },
        "fish" => Options {
            marks: &['-'],
            bundles: true,
            short: "CdopfD",
            optional: "",
            command_short: "c",
            valued: &[
                "--init-command",
                "--debug",
                "--debug-output",
                "--profile",
                "--profile-startup",
                "--features",
                "--debug-stack-frames",
            ],
            command: &["--command"],
            fold: false,
        },
        "nu" => Options {
            marks: &['-'],
            bundles: false,
            short: "",
            optional: "",
            command_short: "",
            valued: &[
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
            command: &["-c", "--commands"],
            fold: false,
        },
        "pwsh" | "powershell" => Options {
            marks: &['-'],
            bundles: false,
            short: "",
            optional: "",
            command_short: "",
            valued: &[
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
            command: &[
                "-Command",
                "-c",
                "-File",
                "-f",
                "-EncodedCommand",
                "-e",
                "-ec",
                "-CommandWithArgs",
                "-cwa",
            ],
            fold: true,
        },
        _ => return None,
    };
    Some(options)
}

/// Whether `path` runs fish or nu, whose own lines mark clicks.
#[cfg(unix)]
pub(crate) fn clicks(path: &Path) -> bool {
    matches!(name(path).as_str(), "fish" | "nu")
}
