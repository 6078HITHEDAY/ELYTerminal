use std::{
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};

use anyhow::{Context as _, anyhow, bail, ensure};
use ely_gpui_component::{
    git::{ChangeSection, Commit, DiffLine, LineKind, Stretch, mark_words},
    lists::GitStatus,
};

/// Runs git in `folder`: English messages, literal paths, no lock the user's own git could meet; `input` feeds stdin.
fn git(folder: &Path, args: &[&str], input: &[u8]) -> anyhow::Result<Output> {
    let named = || format!("git: `git {}` in {}", args.join(" "), folder.display());
    let mut child = Command::new("git")
        .arg("-C")
        .arg(folder)
        .args(["--no-optional-locks", "--literal-pathspecs"])
        .args(args)
        .env("LC_ALL", "C")
        .stdin(if input.is_empty() {
            Stdio::null()
        } else {
            Stdio::piped()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("{} did not start", named()))?;
    let wrote = child
        .stdin
        .take()
        .map_or(Ok(()), |mut stdin| stdin.write_all(input));
    let output = child
        .wait_with_output()
        .with_context(|| format!("{} did not end", named()))?;
    if output.status.success() {
        wrote.with_context(|| format!("{} took no input", named()))?;
    }
    Ok(output)
}

/// What git said about a failure: its errors, else its output, else its exit status.
fn said(output: &Output) -> String {
    [&output.stderr, &output.stdout]
        .into_iter()
        .map(|text| String::from_utf8_lossy(text).trim().to_string())
        .find(|text| !text.is_empty())
        .unwrap_or_else(|| output.status.to_string())
}

/// Runs git and gives what it printed; a failure is an error carrying what git said.
fn run(folder: &Path, args: &[&str], input: &[u8]) -> anyhow::Result<Vec<u8>> {
    let output = git(folder, args, input)?;
    let (command, place) = (args.join(" "), folder.display());
    ensure!(
        output.status.success(),
        "git: `git {command}` failed in {place}: {}",
        said(&output)
    );
    Ok(output.stdout)
}

/// A record of `git status --porcelain=v2 -z`: index and work tree codes, its path from the top, a rename's old path.
#[derive(Debug, PartialEq)]
struct Record {
    x: u8,
    y: u8,
    path: String,
    from: Option<String>,
}

/// HEAD as status reads it: its branch, none when detached; its commit, none before the first.
#[derive(Debug, Default, PartialEq)]
struct Head {
    branch: Option<String>,
    commit: Option<String>,
}

/// Porcelain v2 records, each ended by NUL: headers name HEAD; a rename's old path follows it.
fn parse(out: &[u8]) -> anyhow::Result<(Head, Vec<Record>)> {
    let mut fields = out
        .split(|&byte| byte == 0)
        .filter(|field| !field.is_empty())
        .map(String::from_utf8_lossy);
    let (mut head, mut records) = (Head::default(), Vec::new());
    while let Some(field) = fields.next() {
        let malformed = || anyhow!("git: status record {field:?} is malformed");
        let (kind, rest) = field.split_once(' ').ok_or_else(malformed)?;
        // Fields before the path, which may hold spaces.
        let count = match kind {
            "#" => {
                match rest.split_once(' ') {
                    Some(("branch.oid", oid)) => {
                        head.commit = (oid != "(initial)").then(|| oid.into())
                    }
                    Some(("branch.head", name)) => {
                        head.branch = (name != "(detached)").then(|| name.into())
                    }
                    _ => {}
                }
                continue;
            }
            "?" => 0,
            "1" => 7,
            "2" => 8,
            "u" => 9,
            _ => return Err(malformed()),
        };
        let parts: Vec<&str> = rest.splitn(count + 1, ' ').collect();
        let (codes, path) = match (kind, parts.as_slice()) {
            ("?", [path]) => (b"??".as_slice(), *path),
            (_, [codes, .., path]) if parts.len() == count + 1 => (codes.as_bytes(), *path),
            _ => return Err(malformed()),
        };
        let (&[x, y], false) = (codes, path.is_empty()) else {
            return Err(malformed());
        };
        let from = match kind {
            "2" => Some(fields.next().ok_or_else(malformed)?.into_owned()),
            _ => None,
        };
        let path = path.to_string();
        records.push(Record { x, y, path, from });
    }
    Ok((head, records))
}

/// One mark for an index and a work tree code, `.` for none; a copy is a new file.
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

/// A record's mark from `x` and `y`; codes git never writes are an error.
fn mark(record: &Record, x: u8, y: u8) -> anyhow::Result<GitStatus> {
    let codes = format!("{}{}", x as char, y as char);
    kind(x, y).with_context(|| format!("git: {} has codes {codes}", record.path))
}

/// Each change at or under `folder`, by its full path; none outside a work tree.
pub fn changes(folder: &Path) -> anyhow::Result<Option<Vec<(PathBuf, GitStatus)>>> {
    let place = ["rev-parse", "--is-inside-work-tree", "--show-prefix"];
    let place = git(folder, &place, &[])?;
    if !place.status.success() {
        let error = said(&place);
        if error.contains("not a git repository") {
            return Ok(None);
        }
        bail!("git: rev-parse in {} failed: {error}", folder.display());
    }
    let place = String::from_utf8_lossy(&place.stdout);
    // The prefix may hold newlines of its own.
    let answer = place
        .strip_suffix('\n')
        .and_then(|place| place.split_once('\n'));
    let prefix = match answer {
        Some(("true", prefix)) => prefix,
        Some(("false", _)) => return Ok(None),
        _ => bail!("git: rev-parse in {} printed {place:?}", folder.display()),
    };
    let status = ["status", "--porcelain=v2", "-z", "--", "."];
    let (_, records) = parse(&run(folder, &status, &[])?)?;
    records
        .into_iter()
        .map(|record| {
            let status = mark(&record, record.x, record.y)?;
            let Some(rest) = record.path.strip_prefix(prefix) else {
                bail!("git: {} lies outside {prefix}", record.path);
            };
            let parts = rest.split('/').filter(|part| !part.is_empty());
            let path = parts.fold(folder.to_path_buf(), |path, part| path.join(part));
            Ok((path, status))
        })
        .collect::<anyhow::Result<_>>()
        .map(Some)
}

/// A changed file, by its path from the work tree's top; a staged rename keeps its old path.
#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    pub path: String,
    pub from: Option<String>,
    pub status: GitStatus,
}

/// A work tree as the Git panel shows it.
#[derive(Debug, PartialEq)]
pub struct Repo {
    /// The work tree's top folder.
    pub top: PathBuf,
    /// The branch checked out; none when HEAD is detached.
    pub branch: Option<String>,
    /// HEAD's commit; none before the first.
    pub commit: Option<String>,
    pub conflicted: Vec<Change>,
    pub staged: Vec<Change>,
    pub unstaged: Vec<Change>,
    pub untracked: Vec<Change>,
}

/// The work tree holding `folder`, read whole; none outside one.
pub fn repo(folder: &Path) -> anyhow::Result<Option<Repo>> {
    let top = git(folder, &["rev-parse", "--show-toplevel"], &[])?;
    if !top.status.success() {
        let error = said(&top);
        if error.contains("not a git repository") || error.contains("must be run in a work tree") {
            return Ok(None);
        }
        bail!("git: rev-parse in {} failed: {error}", folder.display());
    }
    let top = String::from_utf8_lossy(&top.stdout);
    let top = top
        .strip_suffix('\n')
        .with_context(|| format!("git: rev-parse in {} printed {top:?}", folder.display()))?;
    let top = PathBuf::from(top);
    let status = ["status", "--porcelain=v2", "--branch", "-z"];
    let (head, records) = parse(&run(&top, &status, &[])?)?;
    let mut repo = Repo {
        top,
        branch: head.branch,
        commit: head.commit,
        conflicted: Vec::new(),
        staged: Vec::new(),
        unstaged: Vec::new(),
        untracked: Vec::new(),
    };
    for record in records {
        let change = |status, from| Change {
            path: record.path.clone(),
            from,
            status,
        };
        match mark(&record, record.x, record.y)? {
            GitStatus::Untracked => repo.untracked.push(change(GitStatus::Untracked, None)),
            GitStatus::Conflicted => repo.conflicted.push(change(GitStatus::Conflicted, None)),
            _ => {
                if record.x != b'.' {
                    let staged = mark(&record, record.x, b'.')?;
                    repo.staged.push(change(staged, record.from.clone()));
                }
                if record.y != b'.' {
                    let unstaged = mark(&record, b'.', record.y)?;
                    repo.unstaged.push(change(unstaged, None));
                }
            }
        }
    }
    Ok(Some(repo))
}

/// Paths for git's `--pathspec-from-file`: each change's and a rename's old one, NUL-ended.
fn pathspecs(changes: &[Change]) -> anyhow::Result<Vec<u8>> {
    // No path would take the whole tree.
    ensure!(!changes.is_empty(), "git: no file named");
    let mut out = Vec::new();
    for change in changes {
        for path in std::iter::once(&change.path).chain(&change.from) {
            out.extend_from_slice(path.as_bytes());
            out.push(0);
        }
    }
    Ok(out)
}

fn names(changes: &[Change]) -> String {
    let names: Vec<&str> = changes.iter().map(|change| change.path.as_str()).collect();
    names.join(", ")
}

/// Stages `changes`: new, changed and gone files alike.
pub fn stage(top: &Path, changes: &[Change]) -> anyhow::Result<()> {
    let args = ["add", "-A", "--pathspec-from-file=-", "--pathspec-file-nul"];
    run(top, &args, &pathspecs(changes)?)?;
    log::info!("git: staged {} in {}", names(changes), top.display());
    Ok(())
}

/// Unstages `changes`, before the first commit too.
pub fn unstage(top: &Path, changes: &[Change]) -> anyhow::Result<()> {
    let args = [
        "reset",
        "-q",
        "--pathspec-from-file=-",
        "--pathspec-file-nul",
    ];
    run(top, &args, &pathspecs(changes)?)?;
    log::info!("git: unstaged {} in {}", names(changes), top.display());
    Ok(())
}

/// Commits what is staged with `message`.
pub fn commit(top: &Path, message: &str) -> anyhow::Result<()> {
    run(top, &["commit", "-q", "-F", "-"], message.as_bytes())?;
    log::info!("git: committed in {}", top.display());
    Ok(())
}

/// The newest `count` commits from HEAD, which must have one.
pub fn log(top: &Path, count: usize) -> anyhow::Result<Vec<Commit>> {
    let count = format!("-n{count}");
    let format = "--format=%H%x00%P%x00%s%x00%an%x00%ar%x00%D";
    let args = ["log", &count, "-z", "--no-show-signature", format];
    let out = run(top, &args, &[])?;
    let out = String::from_utf8_lossy(&out);
    let fields: Vec<&str> = out.split_terminator('\0').collect();
    ensure!(
        fields.len().is_multiple_of(6),
        "git: log in {} printed {} fields, not six for each commit",
        top.display(),
        fields.len()
    );
    let listed = |text: &str, by: &str| -> Vec<_> {
        let parts = text.split(by).filter(|part| !part.is_empty());
        parts.map(|part| part.to_string().into()).collect()
    };
    let commits = fields.chunks(6).map(|commit| Commit {
        id: commit[0].to_string().into(),
        parents: listed(commit[1], " "),
        subject: commit[2].to_string().into(),
        author: commit[3].to_string().into(),
        when: commit[4].to_string().into(),
        refs: listed(commit[5], ", "),
    });
    Ok(commits.collect())
}

/// `change`'s diff as `section` holds it, in Ely's stretches; none for a binary file.
pub fn diff(
    top: &Path,
    change: &Change,
    section: ChangeSection,
) -> anyhow::Result<Option<Vec<Stretch>>> {
    let path = change.path.as_str();
    let mut args = vec!["diff", "--no-color", "--no-ext-diff"];
    match section {
        ChangeSection::Staged => args.extend(["--cached", "-M", "--", path]),
        ChangeSection::Unstaged => args.extend(["--", path]),
        ChangeSection::Conflicted => args.extend(["--ours", "--", path]),
        ChangeSection::Untracked if path.ends_with('/') => bail!("git: {path} is a folder"),
        ChangeSection::Untracked => args.extend(["--no-index", "--", "/dev/null", path]),
        ChangeSection::Ignored => bail!("git: {path} is ignored"),
    }
    args.extend(change.from.as_deref());
    let output = git(top, &args, &[])?;
    // Without an index, differing files exit 1.
    let differ = section == ChangeSection::Untracked
        && output.status.code() == Some(1)
        && output.stderr.is_empty();
    ensure!(
        output.status.success() || differ,
        "git: `git {}` failed in {}: {}",
        args.join(" "),
        top.display(),
        said(&output)
    );
    hunks(&String::from_utf8_lossy(&output.stdout))
}

/// A hunk header's first old and new line numbers.
fn starts(header: &str) -> Option<(usize, usize)> {
    let mut ranges = header.strip_prefix("@@ -")?.split(' ');
    let first = |range: &str| range.split(',').next()?.parse().ok();
    let old = first(ranges.next()?)?;
    let new = first(ranges.next()?.strip_prefix('+')?)?;
    Some((old, new))
}

/// Git's unified diff as Ely's stretches, changed words marked; none for a binary file.
fn hunks(text: &str) -> anyhow::Result<Option<Vec<Stretch>>> {
    let (mut stretches, mut at, mut inside) = (Vec::new(), (0, 0), false);
    for line in text.lines() {
        if line.starts_with("diff ") {
            inside = false;
        } else if line.starts_with("@@") {
            at = starts(line).with_context(|| format!("git: hunk header {line:?}"))?;
            let header = line.to_string().into();
            stretches.push(Stretch::Hunk {
                header,
                lines: Vec::new(),
            });
            inside = true;
        } else if !inside {
            if line.starts_with("Binary files ") {
                return Ok(None);
            }
        } else if let Some(Stretch::Hunk { lines, .. }) = stretches.last_mut() {
            let (kind, text) = match line.as_bytes().first() {
                // `diff.suppressBlankEmpty` drops a blank line's space.
                None => (LineKind::Same, ""),
                Some(b' ') => (LineKind::Same, &line[1..]),
                Some(b'-') => (LineKind::Removed, &line[1..]),
                Some(b'+') => (LineKind::Added, &line[1..]),
                Some(b'\\') => continue,
                Some(_) => bail!("git: diff line {line:?} is malformed"),
            };
            let old = (kind != LineKind::Added).then_some(at.0);
            let new = (kind != LineKind::Removed).then_some(at.1);
            at = (
                at.0 + usize::from(old.is_some()),
                at.1 + usize::from(new.is_some()),
            );
            lines.push(DiffLine {
                kind,
                old,
                new,
                text: text.to_string().into(),
                words: Vec::new(),
            });
        }
    }
    for stretch in &mut stretches {
        if let Stretch::Hunk { lines, .. } = stretch {
            mark_words(lines);
        }
    }
    Ok(Some(stretches))
}

#[cfg(test)]
mod tests;
