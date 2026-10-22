use std::{
    env, fs,
    io::ErrorKind,
    path::{Path, PathBuf},
};

use anyhow::Context as _;
use gpui::{App, Global};

use super::{drop::Quote, pty::Launch};

/// Each script, by its path under the integration folder.
const SCRIPTS: [(&str, &str); 4] = [
    (
        "zsh/.zshenv",
        include_str!("../../assets/shell-integration/zsh/.zshenv"),
    ),
    (
        "bash/elyterminal.bash",
        include_str!("../../assets/shell-integration/bash/elyterminal.bash"),
    ),
    (
        "fish/vendor_conf.d/elyterminal.fish",
        include_str!("../../assets/shell-integration/fish/vendor_conf.d/elyterminal.fish"),
    ),
    (
        "pwsh/elyterminal.ps1",
        include_str!("../../assets/shell-integration/pwsh/elyterminal.ps1"),
    ),
];

/// The folder the scripts live in, written at start.
pub struct ShellIntegration(pub PathBuf);

impl Global for ShellIntegration {}

impl ShellIntegration {
    /// Writes the scripts into the platform's data folder and keeps its path.
    pub fn init(cx: &mut App) -> anyhow::Result<()> {
        let dir = dirs::data_local_dir()
            .context("shell integration: the platform names no data folder")?
            .join("ELYTerminal")
            .join("shell-integration");
        install(&dir)?;
        cx.set_global(Self(dir));
        Ok(())
    }
}

/// Writes the scripts into `dir`, replacing any that differ.
pub fn install(dir: &Path) -> anyhow::Result<()> {
    for (name, text) in SCRIPTS {
        let path = dir.join(name);
        match fs::read_to_string(&path) {
            Ok(kept) if kept == text => continue,
            Ok(_) => {}
            Err(error) if matches!(error.kind(), ErrorKind::NotFound | ErrorKind::InvalidData) => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("shell integration: cannot read {}", path.display()));
            }
        }
        let folder = path
            .parent()
            .context("shell integration: a script has no folder")?;
        fs::create_dir_all(folder)
            .with_context(|| format!("shell integration: cannot create {}", folder.display()))?;
        fs::write(&path, text)
            .with_context(|| format!("shell integration: cannot write {}", path.display()))?;
        log::info!("shell integration: wrote {}", path.display());
    }
    Ok(())
}

/// How to start `shell`, or the login shell when empty, in `cwd` so it reports its folder.
pub fn launch(shell: &str, cwd: PathBuf, dir: &Path) -> anyhow::Result<Launch> {
    let path = if shell.is_empty() {
        default_shell()?
    } else {
        // The settings checked this path; the session runs no other.
        crate::settings::program_path(shell).context("shell integration")?
    };
    let name = Path::new(&path)
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let mut launch = Launch {
        program: (!shell.is_empty()).then(|| (path.clone(), Vec::new())),
        cwd,
        env: Vec::new(),
        reports: true,
        quote: match name.as_str() {
            "pwsh" | "powershell" => Quote::PowerShell,
            "cmd" => Quote::Cmd,
            _ => Quote::Posix,
        },
    };
    let script = |name: &str| dir.join(name).display().to_string();
    match name.as_str() {
        "zsh" => {
            if let Ok(zdotdir) = env::var("ZDOTDIR") {
                launch.env.push(("ELY_ZSH_ZDOTDIR".into(), zdotdir));
            }
            launch.env.push(("ZDOTDIR".into(), script("zsh")));
        }
        "bash" => {
            let rcfile = script("bash/elyterminal.bash");
            launch.program = Some((path.clone(), vec!["--rcfile".into(), rcfile]));
            if cfg!(target_os = "macos") {
                launch.env.push(("ELY_BASH_LOGIN".into(), "1".into()));
            }
        }
        "fish" => {
            let previous = env::var("XDG_DATA_DIRS").unwrap_or_default();
            let rest = if previous.is_empty() {
                "/usr/local/share:/usr/share"
            } else {
                &previous
            };
            let data = format!("{}:{rest}", dir.display());
            launch.env.push(("XDG_DATA_DIRS".into(), data));
            launch.env.push(("ELY_FISH_XDG_DATA_DIRS".into(), previous));
        }
        "pwsh" | "powershell" => {
            let file = script("pwsh/elyterminal.ps1").replace('\'', "''");
            launch.program = Some((
                path.clone(),
                vec![
                    "-NoLogo".into(),
                    "-NoExit".into(),
                    "-ExecutionPolicy".into(),
                    "Bypass".into(),
                    "-Command".into(),
                    format!(
                        "try {{ . '{file}' }} catch {{ [Console]::Error.WriteLine(\"ELYTerminal: shell integration failed: $_\"); exit 1 }}"
                    ),
                ],
            ));
        }
        _ => {
            launch.reports = false;
            // Windows has no login shell of its own, so the shell that was chosen runs as it is.
            #[cfg(windows)]
            {
                launch.program = Some((path.clone(), Vec::new()));
            }
            log::info!("shell integration: none for {path}; its folder goes unreported")
        }
    }
    log::info!("shell integration: {path} as {name}");
    Ok(launch)
}

#[cfg(unix)]
fn default_shell() -> anyhow::Result<String> {
    Ok(super::unix::login_shell_path()?)
}

/// `pwsh`, as Windows Terminal takes it, else the Windows PowerShell the system ships, else `cmd`.
#[cfg(windows)]
fn default_shell() -> anyhow::Result<String> {
    // The registry's PATH, so a shell in a folder added since this process opened is found.
    first_shell(&super::env::path()?)
}

/// The first of `pwsh`, `powershell` and `cmd` that `folders` hold outside `WindowsApps`.
#[cfg(windows)]
fn first_shell(folders: &str) -> anyhow::Result<String> {
    for name in ["pwsh", "powershell", "cmd"] {
        for folder in env::split_paths(folders) {
            match crate::settings::program_path_in(name, &folder) {
                // An App Execution Alias is a stub that starts only while its package is installed.
                Ok(found) if alias(&found) => {
                    log::info!("shell integration: {found} is an alias, so passed by")
                }
                Ok(found) => return Ok(found),
                Err(_) => {}
            }
        }
        log::info!("shell integration: no {name} on PATH");
    }
    anyhow::bail!("shell integration: neither pwsh, powershell nor cmd lies on PATH")
}

/// Whether `path` lies in the folder Windows puts its App Execution Aliases in.
#[cfg(windows)]
fn alias(path: &str) -> bool {
    Path::new(path)
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name.eq_ignore_ascii_case("WindowsApps"))
}

#[cfg(all(test, unix))]
mod tests;

#[cfg(all(test, windows))]
mod windows_tests {
    use super::{alias, first_shell};

    #[test]
    fn an_alias_hides_no_pwsh_later_on_path() {
        let root = std::env::temp_dir().join(format!("elyterminal-shells-{}", std::process::id()));
        let folders = ["WindowsApps", "PS7", "System32"].map(|name| root.join(name));
        for (folder, file) in folders
            .iter()
            .zip(["pwsh.exe", "pwsh.exe", "powershell.exe"])
        {
            std::fs::create_dir_all(folder).expect("a folder");
            std::fs::write(folder.join(file), "").expect("a program");
        }
        let path = std::env::join_paths(&folders).expect("a PATH");
        let found = first_shell(path.to_str().expect("text"));
        std::fs::remove_dir_all(&root).expect("cleanup");
        assert_eq!(
            found.expect("a shell"),
            folders[1].join("pwsh.exe").display().to_string()
        );
    }

    #[test]
    fn an_app_execution_alias_is_recognised_by_its_folder() {
        assert!(alias(
            "C:\\Users\\x\\AppData\\Local\\Microsoft\\WindowsApps\\pwsh.exe"
        ));
        assert!(alias(
            "C:\\Users\\x\\AppData\\Local\\Microsoft\\WINDOWSAPPS\\pwsh.exe"
        ));
        assert!(!alias("C:\\Program Files\\PowerShell\\7\\pwsh.exe"));
        assert!(!alias(
            "C:\\WINDOWS\\System32\\WindowsPowerShell\\v1.0\\powershell.exe"
        ));
        assert!(!alias("pwsh.exe"));
    }
}
