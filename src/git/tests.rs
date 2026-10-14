use std::{env, fs, ops::Range};

use gpui::SharedString;

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

/// Runs git in `root` to set a test up.
fn prepare(root: &Path, args: &[&str]) {
    let done = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs");
    let error = String::from_utf8_lossy(&done.stderr);
    assert!(done.status.success(), "git {args:?}: {error}");
}

/// A new repository on `main` whose own config keeps the user's away.
fn repository(name: &str) -> PathBuf {
    let root = folder(name);
    prepare(&root, &["init", "-q", "-b", "main"]);
    for [key, value] in [
        ["user.name", "Ely"],
        ["user.email", "ely@example.com"],
        ["commit.gpgsign", "false"],
        ["core.hooksPath", "no-hooks"],
        ["core.autocrlf", "false"],
        ["merge.conflictStyle", "merge"],
        ["status.showUntrackedFiles", "normal"],
    ] {
        prepare(&root, &["config", key, value]);
    }
    root
}

/// Writes `text` to `path` under `root`, making its folders.
fn write(root: &Path, path: &str, text: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().expect("a parent")).expect("folders");
    fs::write(path, text).expect("a file");
}

fn change(path: &str, status: GitStatus) -> Change {
    Change {
        path: path.into(),
        from: None,
        status,
    }
}

fn read(root: &Path) -> Repo {
    repo(root).expect("status").expect("a work tree")
}

#[test]
fn status_records_read_with_head() {
    let out = b"# branch.oid 1a2b\0# branch.head main\0# branch.upstream origin/main\0\
        # branch.ab +1 -0\0\
        1 .M N... 100644 100644 100644 aaa bbb src/a b.rs\0\
        2 RM N... 100644 100644 100644 aaa bbb R100 new name.rs\0old name.rs\0\
        u UU N... 100644 100644 100644 100644 aaa bbb ccc clash.rs\0? notes/\0";
    let (head, records) = parse(out).expect("records");
    let main = Head {
        branch: Some("main".into()),
        commit: Some("1a2b".into()),
    };
    assert_eq!(head, main);
    let record = |x, y, path: &str, from: Option<&str>| Record {
        x,
        y,
        path: path.into(),
        from: from.map(Into::into),
    };
    let expected = [
        record(b'.', b'M', "src/a b.rs", None),
        record(b'R', b'M', "new name.rs", Some("old name.rs")),
        record(b'U', b'U', "clash.rs", None),
        record(b'?', b'?', "notes/", None),
    ];
    assert_eq!(records, expected);
    let unborn = b"# branch.oid (initial)\0# branch.head (detached)\0";
    let (head, _) = parse(unborn).expect("headers");
    assert_eq!(head, Head::default(), "no commit yet, no branch");
}

#[test]
fn records_git_never_writes_are_errors() {
    for out in [
        &b"1 .M N... 100644\0"[..],
        b"x junk\0",
        b"2 R. N... 1 2 3 a b R100 new\0",
        b"1 M N... 1 2 3 a b p\0",
        b"?\0",
        b"1 .M N... 1 2 3 a b \0",
    ] {
        assert!(parse(out).is_err(), "{:?}", String::from_utf8_lossy(out));
    }
}

#[test]
fn both_codes_combine_into_one_mark() {
    use GitStatus::*;
    for (codes, status) in [
        (b"??", Untracked),
        (b"UU", Conflicted),
        (b"AA", Conflicted),
        (b"DU", Conflicted),
        (b".D", Deleted),
        (b"RM", Renamed),
        (b"AM", Added),
        (b"C.", Added),
        (b".T", Modified),
        (b"MM", Modified),
    ] {
        let named = String::from_utf8_lossy(codes);
        assert_eq!(kind(codes[0], codes[1]), Some(status), "{named}");
    }
    assert_eq!(kind(b'.', b'.'), None);
    assert_eq!(kind(b'!', b'!'), None);
}

#[test]
fn a_work_tree_reports_changes_under_the_folder() {
    let root = repository("git-changes");
    for (path, text) in [
        ("kept.txt", "kept"),
        ("gone.txt", "gone"),
        ("src/main.rs", "fn main() {}"),
        ("src/inner/deep.rs", "deep"),
    ] {
        write(&root, path, text);
    }
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "src/main.rs", "fn main() { }");
    fs::remove_file(root.join("gone.txt")).expect("a removal");
    write(&root, "added.txt", "new");
    prepare(&root, &["add", "added.txt"]);
    write(&root, "notes/sub/todo.txt", "todo");

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

#[cfg(unix)]
#[test]
fn a_folder_named_with_a_newline_keeps_its_whole_prefix() {
    let root = repository("git-newline");
    write(&root, "line\nbreak/tracked.txt", "first");
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "line\nbreak/tracked.txt", "second");
    let inner = root.join("line\nbreak");
    let edit = (inner.join("tracked.txt"), GitStatus::Modified);
    assert_eq!(changes(&inner).expect("status"), Some(vec![edit]));
}

#[test]
fn a_repository_lists_its_branch_and_each_section() {
    let root = repository("git-repo");
    for path in ["kept.txt", "gone.txt", "old.txt", "src/main.rs"] {
        write(&root, path, "first\n");
    }
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "src/main.rs", "second\n");
    fs::remove_file(root.join("gone.txt")).expect("a removal");
    write(&root, "added.txt", "new\n");
    write(&root, "kept.txt", "staged\n");
    prepare(&root, &["add", "added.txt", "kept.txt"]);
    write(&root, "kept.txt", "staged, then changed\n");
    prepare(&root, &["mv", "old.txt", "new.txt"]);
    write(&root, "notes/todo.txt", "todo\n");

    let state = read(&root.join("src"));
    let real = |path: &Path| fs::canonicalize(path).expect("a real path");
    assert_eq!(
        real(&state.top),
        real(&root),
        "the top, from a folder under it"
    );
    assert_eq!(state.branch.as_deref(), Some("main"));
    assert!(state.commit.is_some());
    let renamed = Change {
        path: "new.txt".into(),
        from: Some("old.txt".into()),
        status: GitStatus::Renamed,
    };
    let staged = [
        change("added.txt", GitStatus::Added),
        change("kept.txt", GitStatus::Modified),
        renamed,
    ];
    assert_eq!(state.staged, staged);
    let unstaged = [
        change("gone.txt", GitStatus::Deleted),
        change("kept.txt", GitStatus::Modified),
        change("src/main.rs", GitStatus::Modified),
    ];
    assert_eq!(state.unstaged, unstaged);
    assert_eq!(state.untracked, [change("notes/", GitStatus::Untracked)]);
    assert!(state.conflicted.is_empty());

    prepare(&root, &["commit", "-qm", "second"]);
    prepare(&root, &["checkout", "-q", "--detach"]);
    assert_eq!(read(&root).branch, None, "a detached HEAD has no branch");
    assert_eq!(repo(&root.join(".git")).expect("rev-parse"), None);
    assert_eq!(repo(&folder("git-repo-outside")).expect("rev-parse"), None);
}

#[test]
fn files_stage_and_unstage_by_their_literal_paths() {
    let root = repository("git-stage");
    write(&root, "star[1].txt", "a\n");
    write(&root, "star1.txt", "b\n");
    let star = change("star[1].txt", GitStatus::Untracked);
    stage(&root, &[star]).expect("a stage before the first commit");
    let state = read(&root);
    assert_eq!(state.commit, None);
    let added = change("star[1].txt", GitStatus::Added);
    assert_eq!(state.staged, [added], "the brackets match nothing else");
    assert_eq!(state.untracked, [change("star1.txt", GitStatus::Untracked)]);
    unstage(&root, &state.staged).expect("an unstage before the first commit");
    let state = read(&root);
    assert!(state.staged.is_empty(), "{:?}", state.staged);
    assert_eq!(state.untracked.len(), 2);

    stage(&root, &state.untracked).expect("a stage");
    commit(&root, "first").expect("a commit");
    prepare(&root, &["mv", "star1.txt", "moved.txt"]);
    unstage(&root, &read(&root).staged).expect("an unstaged rename");
    let state = read(&root);
    assert!(
        state.staged.is_empty(),
        "both sides of the rename: {state:?}"
    );
    assert_eq!(state.unstaged, [change("star1.txt", GitStatus::Deleted)]);
    assert_eq!(state.untracked, [change("moved.txt", GitStatus::Untracked)]);
    assert!(
        stage(&root, &[]).is_err(),
        "no path would stage the whole tree"
    );
    assert!(unstage(&root, &[]).is_err());
}

#[cfg(unix)]
#[test]
fn a_name_with_a_newline_stages_whole() {
    let root = repository("git-stage-newline");
    write(&root, "line\nfeed.txt", "a\n");
    let new = change("line\nfeed.txt", GitStatus::Untracked);
    stage(&root, &[new]).expect("a stage");
    let added = change("line\nfeed.txt", GitStatus::Added);
    assert_eq!(read(&root).staged, [added]);
}

#[test]
fn a_commit_takes_the_staged_files_and_heads_the_log() {
    let root = repository("git-commit");
    write(&root, "a.txt", "a\n");
    stage(&root, &[change("a.txt", GitStatus::Untracked)]).expect("a stage");
    commit(&root, "first subject\n\nIts body.").expect("a commit");
    write(&root, "b.txt", "b\n");
    stage(&root, &[change("b.txt", GitStatus::Untracked)]).expect("a stage");
    commit(&root, "second").expect("a commit");

    let commits = log(&root, 10).expect("a log");
    let [newest, first] = commits.as_slice() else {
        panic!("two commits: {commits:?}");
    };
    assert_eq!(newest.subject.as_ref(), "second");
    assert_eq!(first.subject.as_ref(), "first subject");
    assert_eq!(newest.parents, std::slice::from_ref(&first.id));
    assert!(first.parents.is_empty());
    assert!(newest.id.chars().all(|c| c.is_ascii_hexdigit()) && !newest.id.is_empty());
    assert_eq!(newest.author.as_ref(), "Ely");
    assert!(newest.when.ends_with("ago"), "{}", newest.when);
    assert_eq!(newest.refs, [SharedString::from("HEAD -> main")]);
    assert!(first.refs.is_empty());
    assert_eq!(log(&root, 1).expect("a log").len(), 1);
    let error = commit(&root, "nothing").expect_err("nothing is staged");
    assert!(format!("{error:#}").contains("nothing"), "{error:#}");
}

fn line(kind: LineKind, at: (Option<usize>, Option<usize>), text: &str) -> DiffLine {
    lined(kind, at, text, Vec::new())
}

fn lined(
    kind: LineKind,
    (old, new): (Option<usize>, Option<usize>),
    text: &str,
    words: Vec<Range<usize>>,
) -> DiffLine {
    DiffLine {
        kind,
        old,
        new,
        text: text.to_string().into(),
        words,
    }
}

fn hunk(header: &str, lines: Vec<DiffLine>) -> Stretch {
    Stretch::Hunk {
        header: header.to_string().into(),
        lines,
    }
}

#[test]
fn diffs_read_as_ely_stretches() {
    use LineKind::*;
    let root = repository("git-diff");
    write(&root, "a.txt", "one\nthe old line\nthree\n");
    write(&root, "old.txt", "kept\n");
    fs::write(root.join("bin.dat"), b"x\0y").expect("a binary file");
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "first"]);
    write(&root, "a.txt", "one\nthe new line\nthree\n");
    prepare(&root, &["add", "a.txt"]);
    write(&root, "a.txt", "one\nthe new line\nthree\nfour");
    fs::write(root.join("bin.dat"), b"x\0z").expect("a binary edit");
    prepare(&root, &["mv", "old.txt", "new.txt"]);
    write(&root, "fresh.txt", "fresh\nlines\n");
    write(&root, "notes/todo.txt", "todo\n");

    let a = change("a.txt", GitStatus::Modified);
    let staged = diff(&root, &a, ChangeSection::Staged).expect("a diff");
    let words = |kind, at, text| lined(kind, at, text, vec![Range { start: 4, end: 7 }]);
    let expected = hunk(
        "@@ -1,3 +1,3 @@",
        vec![
            line(Same, (Some(1), Some(1)), "one"),
            words(Removed, (Some(2), None), "the old line"),
            words(Added, (None, Some(2)), "the new line"),
            line(Same, (Some(3), Some(3)), "three"),
        ],
    );
    assert_eq!(staged, Some(vec![expected]));
    let unstaged = diff(&root, &a, ChangeSection::Unstaged).expect("a diff");
    let expected = hunk(
        "@@ -1,3 +1,4 @@",
        vec![
            line(Same, (Some(1), Some(1)), "one"),
            line(Same, (Some(2), Some(2)), "the new line"),
            line(Same, (Some(3), Some(3)), "three"),
            line(Added, (None, Some(4)), "four"),
        ],
    );
    assert_eq!(unstaged, Some(vec![expected]));
    let fresh = change("fresh.txt", GitStatus::Untracked);
    let untracked = diff(&root, &fresh, ChangeSection::Untracked).expect("a diff");
    let expected = hunk(
        "@@ -0,0 +1,2 @@",
        vec![
            line(Added, (None, Some(1)), "fresh"),
            line(Added, (None, Some(2)), "lines"),
        ],
    );
    assert_eq!(untracked, Some(vec![expected]));
    let binary = change("bin.dat", GitStatus::Modified);
    let binary = diff(&root, &binary, ChangeSection::Unstaged).expect("a diff");
    assert_eq!(binary, None, "a binary file has no lines");
    let moved = read(&root)
        .staged
        .into_iter()
        .find(|change| change.from.is_some());
    let moved = moved.expect("a staged rename");
    let renamed = diff(&root, &moved, ChangeSection::Staged).expect("a diff");
    assert_eq!(renamed, Some(Vec::new()), "a pure rename changes no line");
    let notes = change("notes/", GitStatus::Untracked);
    assert!(diff(&root, &notes, ChangeSection::Untracked).is_err());
}

#[test]
fn a_conflict_diffs_against_our_side() {
    use LineKind::*;
    let root = repository("git-conflict");
    write(&root, "c.txt", "base\n");
    prepare(&root, &["add", "."]);
    prepare(&root, &["commit", "-qm", "base"]);
    prepare(&root, &["checkout", "-qb", "other"]);
    write(&root, "c.txt", "theirs\n");
    prepare(&root, &["commit", "-qam", "theirs"]);
    prepare(&root, &["checkout", "-q", "main"]);
    write(&root, "c.txt", "ours\n");
    prepare(&root, &["commit", "-qam", "ours"]);
    let merged = Command::new("git")
        .arg("-C")
        .arg(&root)
        .args(["merge", "-q", "other"])
        .output()
        .expect("git runs");
    assert!(!merged.status.success(), "the merge stops");

    let state = read(&root);
    assert_eq!(state.conflicted, [change("c.txt", GitStatus::Conflicted)]);
    let ours = diff(&root, &state.conflicted[0], ChangeSection::Conflicted).expect("a diff");
    let Some([Stretch::Hunk { lines, .. }]) = ours.as_deref() else {
        panic!("one hunk: {ours:?}");
    };
    let shown: Vec<_> = lines
        .iter()
        .map(|line| (line.kind, line.text.as_ref()))
        .collect();
    let expected = [
        (Added, "<<<<<<< HEAD"),
        (Same, "ours"),
        (Added, "======="),
        (Added, "theirs"),
        (Added, ">>>>>>> other"),
    ];
    assert_eq!(shown, expected);
}

#[test]
fn hunks_number_lines_skip_markers_and_stop_at_the_next_file() {
    use LineKind::*;
    let text = "* Unmerged path a\ndiff --git a/a b/a\n--- a/a\n+++ b/a\n\
        @@ -3 +3,2 @@ fn main\n-x\r\n+y\n+z\n\\ No newline at end of file\n\
        diff --git a/b b/b\nnew file mode 100644\n--- /dev/null\n+++ b/b\n@@ -0,0 +1 @@\n+b\n";
    let first = hunk(
        "@@ -3 +3,2 @@ fn main",
        vec![
            line(Removed, (Some(3), None), "x"),
            line(Added, (None, Some(3)), "y"),
            line(Added, (None, Some(4)), "z"),
        ],
    );
    let second = hunk("@@ -0,0 +1 @@", vec![line(Added, (None, Some(1)), "b")]);
    assert_eq!(hunks(text).expect("hunks"), Some(vec![first, second]));
    let blank = hunk(
        "@@ -1,3 +1,3 @@",
        vec![
            line(Same, (Some(1), Some(1)), "a"),
            line(Same, (Some(2), Some(2)), ""),
            line(Removed, (Some(3), None), "b"),
            line(Added, (None, Some(3)), "c"),
        ],
    );
    let suppressed = "@@ -1,3 +1,3 @@\n a\n\n-b\n+c\n";
    assert_eq!(hunks(suppressed).expect("hunks"), Some(vec![blank]));
    let binary = "diff --git a/x b/x\nBinary files a/x and b/x differ\n";
    assert_eq!(hunks(binary).expect("a diff"), None);
    assert!(hunks("@@ -a +b @@\n").is_err(), "a header without numbers");
    assert!(
        hunks("@@ -1 +1 @@\n?x\n").is_err(),
        "a line git never writes"
    );
}
