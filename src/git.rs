use std::{
    path::{Path, PathBuf},
    process::{Command, Output},
};

use anyhow::{Context as _, bail};
use ely_gpui_component::lists::GitStatus;

/// Runs git in `folder` with English messages, taking no lock the user's own git could meet.
fn git(folder: &Path, args: &[&str]) -> anyhow::Result<Output> {
    Command::new("git")
        .arg("-C")
        .arg(folder)
        .arg("--no-optional-locks")
        .args(args)
        .env("LC_ALL", "C")
        .output()
        .with_context(|| format!("git: `git {}` did not start", args.join(" ")))
}

/// Each change at or under `folder`, by its full path; none outside a work tree.
pub fn changes(folder: &Path) -> anyhow::Result<Option<Vec<(PathBuf, GitStatus)>>> {
    let place = git(
        folder,
        &["rev-parse", "--is-inside-work-tree", "--show-prefix"],
    )?;
    if !place.status.success() {
        let error = String::from_utf8_lossy(&place.stderr);
        if error.contains("not a git repository") {
            return Ok(None);
        }
        bail!(
            "git: rev-parse in {} failed: {}",
            folder.display(),
            error.trim()
        );
    }
    let place = String::from_utf8_lossy(&place.stdout);
    let mut lines = place.lines();
    if lines.next() != Some("true") {
        return Ok(None);
    }
    let prefix = lines.next().context("git: rev-parse printed no prefix")?;
    let status = git(folder, &["status", "--porcelain=v1", "-z", "--", "."])?;
    if !status.status.success() {
        let error = String::from_utf8_lossy(&status.stderr);
        bail!(
            "git: status in {} failed: {}",
            folder.display(),
            error.trim()
        );
    }
    parse(&status.stdout)?
        .into_iter()
        .map(|(path, status)| {
            let Some(rest) = path.strip_prefix(prefix) else {
                bail!("git: {path} lies outside {prefix}");
            };
            let parts = rest.split('/').filter(|part| !part.is_empty());
            Ok((
                parts.fold(folder.to_path_buf(), |path, part| path.join(part)),
                status,
            ))
        })
        .collect::<anyhow::Result<_>>()
        .map(Some)
}

/// Porcelain v1 records, each ended by NUL: `XY path`, a rename's or copy's old path after it.
fn parse(out: &[u8]) -> anyhow::Result<Vec<(String, GitStatus)>> {
    let mut fields = out
        .split(|&byte| byte == 0)
        .filter(|field| !field.is_empty());
    let mut changes = Vec::new();
    while let Some(field) = fields.next() {
        let record = || String::from_utf8_lossy(field);
        let (x, y, path) = match field {
            [x, y, b' ', path @ ..] if !path.is_empty() => (*x, *y, path),
            _ => bail!("git: status record {:?} is malformed", record()),
        };
        let Some(status) = kind(x, y) else {
            bail!("git: status record {:?} has an unknown code", record());
        };
        if [x, y].iter().any(|code| matches!(code, b'R' | b'C')) {
            fields
                .next()
                .with_context(|| format!("git: {:?} lacks its old path", record()))?;
        }
        changes.push((String::from_utf8_lossy(path).into_owned(), status));
    }
    Ok(changes)
}

/// One mark for a record's index and work tree codes; a copy is a new file.
fn kind(x: u8, y: u8) -> Option<GitStatus> {
    Some(match (x, y) {
        (b'?', b'?') => GitStatus::Untracked,
        (b'U', _) | (_, b'U') | (b'A', b'A') | (b'D', b'D') => GitStatus::Conflicted,
        (b'D', _) | (_, b'D') => GitStatus::Deleted,
        (b'R', _) | (_, b'R') => GitStatus::Renamed,
        (b'A' | b'C', _) | (_, b'A' | b'C') => GitStatus::Added,
        (b'M' | b'T', _) | (_, b'M' | b'T') => GitStatus::Modified,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use super::*;

    /// A fresh folder under the temp dir.
    fn folder(name: &str) -> PathBuf {
        let folder = env::temp_dir().join(format!("elyterminal-{name}-{}", std::process::id()));
        if folder.exists() {
            fs::remove_dir_all(&folder).expect("a stale test folder goes");
        }
        fs::create_dir_all(&folder).expect("a test folder");
        folder
    }

    /// Runs git in `folder` as a fixed author, unsigned.
    fn run(folder: &Path, args: &[&str]) {
        let done = Command::new("git")
            .arg("-C")
            .arg(folder)
            .args(["-c", "user.name=Ely", "-c", "user.email=ely@example.com"])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .output()
            .expect("git runs");
        let error = String::from_utf8_lossy(&done.stderr);
        assert!(done.status.success(), "git {args:?}: {error}");
    }

    #[test]
    fn records_read_as_marks_skipping_old_paths() {
        let out = b"RM new.rs\0old.rs\0 M src/a.rs\0?? notes/\0AA both.txt\0UU clash.rs\0\
            A  added.rs\0 D gone.rs\0C  copy.rs\0from.rs\0 T link\0MM twice.rs\0DU kept.rs\0";
        let marks = parse(out).expect("records");
        let expected = [
            ("new.rs", GitStatus::Renamed),
            ("src/a.rs", GitStatus::Modified),
            ("notes/", GitStatus::Untracked),
            ("both.txt", GitStatus::Conflicted),
            ("clash.rs", GitStatus::Conflicted),
            ("added.rs", GitStatus::Added),
            ("gone.rs", GitStatus::Deleted),
            ("copy.rs", GitStatus::Added),
            ("link", GitStatus::Modified),
            ("twice.rs", GitStatus::Modified),
            ("kept.rs", GitStatus::Conflicted),
        ]
        .map(|(path, status)| (path.to_string(), status));
        assert_eq!(marks, expected);
    }

    #[test]
    fn records_git_never_writes_are_errors() {
        for out in [
            &b"M\0"[..],
            b"MM \0",
            b"!! ignored\0",
            b"R  new.rs\0",
            b"   x\0",
        ] {
            assert!(parse(out).is_err(), "{:?}", String::from_utf8_lossy(out));
        }
    }

    #[test]
    fn a_work_tree_reports_changes_under_the_folder() {
        let root = folder("git");
        run(&root, &["init", "-q"]);
        run(&root, &["config", "status.showUntrackedFiles", "normal"]);
        fs::create_dir_all(root.join("src/inner")).expect("folders");
        for (path, text) in [
            ("kept.txt", "kept"),
            ("gone.txt", "gone"),
            ("src/main.rs", "fn main() {}"),
            ("src/inner/deep.rs", "deep"),
        ] {
            fs::write(root.join(path), text).expect("a file");
        }
        run(&root, &["add", "."]);
        run(&root, &["commit", "-qm", "first"]);
        fs::write(root.join("src/main.rs"), "fn main() { }").expect("an edit");
        fs::remove_file(root.join("gone.txt")).expect("a removal");
        fs::write(root.join("added.txt"), "new").expect("a new file");
        run(&root, &["add", "added.txt"]);
        fs::create_dir_all(root.join("notes/sub")).expect("an untracked folder");
        fs::write(root.join("notes/sub/todo.txt"), "todo").expect("an untracked file");

        let mut all = changes(&root).expect("status").expect("a work tree");
        all.sort_by(|a, b| a.0.cmp(&b.0));
        let expected = [
            ("added.txt", GitStatus::Added),
            ("gone.txt", GitStatus::Deleted),
            ("notes", GitStatus::Untracked),
            ("src/main.rs", GitStatus::Modified),
        ]
        .map(|(path, status)| (root.join(path), status));
        assert_eq!(all, expected);
        let src = root.join("src");
        let main = (src.join("main.rs"), GitStatus::Modified);
        assert_eq!(changes(&src).expect("status"), Some(vec![main]));
        let sub = root.join("notes").join("sub");
        let whole = (sub.clone(), GitStatus::Untracked);
        assert_eq!(
            changes(&sub).expect("status"),
            Some(vec![whole]),
            "a folder inside an untracked one is untracked whole"
        );
        assert_eq!(changes(&root.join(".git")).expect("rev-parse"), None);
        assert_eq!(changes(&folder("git-outside")).expect("rev-parse"), None);
    }
}
