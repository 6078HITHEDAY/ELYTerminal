use std::path::Path;

use super::name;

/// How a shell leading its own prompt reads its options.
pub(crate) struct Options {
    /// Characters that start an option.
    marks: &'static [char],
    /// Short options may share one argument, as `-lo vi`.
    bundles: bool,
    /// Short options taking a value.
    short: &'static str,
    /// Of those, the ones whose value may be left out.
    optional: &'static str,
    /// Bundled values each take the next argument, as bash does.
    consume: bool,
    /// Short options that run a command, not a prompt.
    command_short: &'static str,
    /// Short options after whose argument a script follows.
    stop_short: &'static str,
    /// Arguments after which a script follows.
    stops: &'static [&'static str],
    /// Every long option, where a unique prefix names one.
    long: &'static [&'static str],
    /// Options taking the next argument, unless `=` holds one.
    valued: &'static [&'static str],
    /// Options that run a command or a file.
    command: &'static [&'static str],
    /// Names match ignoring case, as PowerShell's do.
    fold: bool,
}

impl Options {
    /// Whether `args`, past the program's name, start an interactive shell.
    pub(crate) fn interactive(&self, args: &[String]) -> bool {
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if self.stops.contains(&arg.as_str()) {
                return args.next().is_none();
            }
            let Some(rest) = arg.strip_prefix(self.marks) else {
                return false;
            };
            if !self.bundles || arg.starts_with("--") {
                let Some(name) = self.resolve(arg.split('=').next().unwrap_or(arg)) else {
                    return false;
                };
                if self.command.iter().any(|command| self.same(command, name)) {
                    return false;
                }
                let valued = self.valued.iter().any(|valued| self.same(valued, name));
                if valued && !arg.contains('=') && args.next().is_none() {
                    return false;
                }
                continue;
            }
            let (mut values, mut stop) = (Vec::new(), false);
            for (at, option) in rest.char_indices() {
                if self.command_short.contains(option) {
                    return false;
                }
                stop |= self.stop_short.contains(option);
                if self.short.contains(option) {
                    let last = at + option.len_utf8() == rest.len();
                    if self.consume || last {
                        values.push(option);
                    }
                    if !self.consume {
                        break;
                    }
                }
            }
            for option in values {
                if args.next().is_none() && !self.optional.contains(option) {
                    return false;
                }
            }
            if stop {
                return args.next().is_none();
            }
        }
        true
    }

    fn same(&self, name: &str, arg: &str) -> bool {
        if self.fold {
            name.eq_ignore_ascii_case(arg)
        } else {
            name == arg
        }
    }

    /// The long option `name` names, whole or by a unique prefix.
    fn resolve<'a>(&self, name: &'a str) -> Option<&'a str> {
        if self.long.is_empty() || self.long.contains(&name) {
            return Some(name);
        }
        let mut matches = self.long.iter().filter(|long| long.starts_with(name));
        match (matches.next(), matches.next()) {
            (Some(long), None) => Some(long),
            _ => None,
        }
    }
}

/// A shell's options where it leads its prompt; none elsewhere.
pub(crate) fn options(path: &Path) -> Option<Options> {
    let plain = Options {
        marks: &['-'],
        bundles: false,
        short: "",
        optional: "",
        consume: false,
        command_short: "",
        stop_short: "",
        stops: &["--", "-"],
        long: &[],
        valued: &[],
        command: &[],
        fold: false,
    };
    let options = match name(path).as_str() {
        "zsh" => Options {
            marks: &['-', '+'],
            bundles: true,
            short: "o",
            command_short: "c",
            stop_short: "b-",
            stops: &["--", "-", "+", "+-"],
            valued: &["--emulate"],
            ..plain
        },
        "bash" => Options {
            marks: &['-', '+'],
            bundles: true,
            short: "oO",
            optional: "oO",
            consume: true,
            command_short: "c",
            valued: &["--rcfile", "--init-file"],
            ..plain
        },
        "fish" => Options {
            bundles: true,
            short: "CdopfD",
            command_short: "c",
            stops: &["--"],
            long: &[
                "--command",
                "--init-command",
                "--features",
                "--debug",
                "--debug-output",
                "--debug-stack-frames",
                "--interactive",
                "--login",
                "--no-config",
                "--no-execute",
                "--print-rusage-self",
                "--print-debug-categories",
                "--install",
                "--profile",
                "--profile-startup",
                "--private",
                "--help",
                "--version",
            ],
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
            ..plain
        },
        "nu" => Options {
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
            ..plain
        },
        "pwsh" | "powershell" => Options {
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
            ..plain
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

/// Whether `path` relays another host's, user's or container's terminal.
pub(crate) fn relays(path: &Path) -> bool {
    matches!(
        name(path).as_str(),
        "ssh" | "mosh-client" | "sudo" | "su" | "doas" | "docker" | "podman" | "kubectl"
    )
}
