use std::env;

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

#[test]
fn folders_come_first_then_files_by_name_ignoring_case() {
    let root = folder("files-list");
    for name in ["Zeta", "alpha"] {
        fs::create_dir(root.join(name)).expect("a folder");
    }
    for name in ["c.txt", "B.txt", "a.txt"] {
        fs::write(root.join(name), "").expect("a file");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        symlink(root.join("Zeta"), root.join("link")).expect("a link to a folder");
        symlink(root.join("nowhere"), root.join("broken")).expect("a broken link");
    }
    let expected: &[(&str, bool)] = if cfg!(unix) {
        &[
            ("alpha", true),
            ("link", true),
            ("Zeta", true),
            ("a.txt", false),
            ("B.txt", false),
            ("broken", false),
            ("c.txt", false),
        ]
    } else {
        &[
            ("alpha", true),
            ("Zeta", true),
            ("a.txt", false),
            ("B.txt", false),
            ("c.txt", false),
        ]
    };
    let listed: Vec<_> = list(&root).expect("a listing");
    let listed: Vec<_> = listed
        .iter()
        .map(|entry| (entry.name.as_str(), entry.folder))
        .collect();
    assert_eq!(listed, expected);
    assert!(list(&root.join("missing")).is_err());
}

#[test]
fn a_mark_dots_each_folder_above_it_up_to_the_root() {
    let root = Path::new("/r");
    let changes = vec![
        (
            root.join("src").join("ui").join("a.rs"),
            GitStatus::Modified,
        ),
        (root.join("gone.txt"), GitStatus::Deleted),
    ];
    let (marks, dirty) = marks(root, changes);
    assert_eq!(marks.len(), 2);
    let expected = HashSet::from([
        root.to_path_buf(),
        root.join("src"),
        root.join("src").join("ui"),
    ]);
    assert_eq!(dirty, expected);
}

#[test]
fn the_tree_wears_marks_and_an_untracked_folder_marks_what_it_holds() {
    let root = PathBuf::from("/r");
    let (notes, src, locked) = (root.join("notes"), root.join("src"), root.join("locked"));
    let entry = |name: &str, folder| Entry {
        name: name.into(),
        folder,
    };
    let mut files = Files::new(root.clone());
    let top = vec![
        entry("notes", true),
        entry("src", true),
        entry("locked", true),
        entry("a.txt", false),
    ];
    files.listings.insert(root.clone(), Ok(top));
    let inside = vec![entry("deep", true), entry("todo.txt", false)];
    files.listings.insert(notes.clone(), Ok(inside));
    files
        .listings
        .insert(src.clone(), Ok(vec![entry("main.rs", false)]));
    files.listings.insert(locked.clone(), Err("denied".into()));
    let changes = vec![
        (notes.clone(), GitStatus::Untracked),
        (src.join("main.rs"), GitStatus::Modified),
    ];
    (files.marks, files.dirty) = marks(&root, changes);

    let node = |path: &Path, icon| {
        let label = path
            .file_name()
            .expect("a name")
            .to_string_lossy()
            .to_string();
        TreeNode::new(key(path), label).icon(icon)
    };
    let untracked = |node: TreeNode| node.note("U").tone(Tone::Success);
    let dotted = |node: TreeNode| node.note("•").tone(Tone::Warning);
    let expected = dotted(
        node(&root, IconName::Folder).children([
            untracked(node(&notes, IconName::Folder).children([
                untracked(node(&notes.join("deep"), IconName::Folder).pending()),
                untracked(node(&notes.join("todo.txt"), IconName::File)),
            ])),
            dotted(
                node(&src, IconName::Folder).children([node(&src.join("main.rs"), IconName::File)
                    .note("M")
                    .tone(Tone::Warning)]),
            ),
            node(&locked, IconName::Folder)
                .children([])
                .note("unreadable")
                .tone(Tone::Danger),
            node(&root.join("a.txt"), IconName::File),
        ]),
    );
    assert_eq!(files.folder(&root, "r".into(), false), expected);
}
