use super::*;

/// What `paths` type for `quote`.
fn typed_as(quote: Quote, all: &[&str]) -> String {
    let paths: Vec<PathBuf> = all.iter().map(PathBuf::from).collect();
    typed(&paths, quote).unwrap()
}

#[test]
fn a_posix_path_escapes_what_a_shell_reads_specially() {
    let typed = |all: &[&str]| typed_as(Quote::Posix, all);
    assert_eq!(typed(&["/tmp/a.txt"]), "/tmp/a.txt ");
    assert_eq!(typed(&["/tmp/my file.png"]), "/tmp/my\\ file.png ");
    assert_eq!(typed(&["/a/it's (1)&$x"]), "/a/it\\'s\\ \\(1\\)\\&\\$x ");
    assert_eq!(
        typed(&["/a/a^b~c#d"]),
        "/a/a\\^b\\~c\\#d ",
        "zsh's extendedglob"
    );
    assert_eq!(typed(&["/a/中文 b"]), "/a/中文\\ b ");
    assert_eq!(
        typed(&["/a", "/b c"]),
        "/a /b\\ c ",
        "each followed by a space"
    );
}

#[test]
fn a_powershell_path_is_single_quoted_with_its_quotes_doubled() {
    let typed = |all: &[&str]| typed_as(Quote::PowerShell, all);
    assert_eq!(typed(&["C:\\a.txt"]), "'C:\\a.txt' ");
    assert_eq!(typed(&["C:\\tmp\\it's.txt"]), "'C:\\tmp\\it''s.txt' ");
    assert_eq!(typed(&["C:\\$HOME $(1+1);x"]), "'C:\\$HOME $(1+1);x' ");
    assert_eq!(
        typed(&["C:\\a\u{2019}b"]),
        "'C:\\a\u{2019}\u{2019}b' ",
        "a curly quote"
    );
    assert_eq!(typed(&["/a b", "/c"]), "'/a b' '/c' ");
}

#[test]
fn a_cmd_path_is_double_quoted_and_one_with_a_percent_types_nothing() {
    let typed = |all: &[&str]| typed_as(Quote::Cmd, all);
    assert_eq!(
        typed(&["C:\\My Files\\a&b^c.txt", "D:\\c"]),
        "\"C:\\My Files\\a&b^c.txt\" \"D:\\c\" "
    );
    let percent = super::typed(&[PathBuf::from("C:\\%PATH%.txt")], Quote::Cmd);
    assert!(percent.is_err(), "{percent:?}");
}

#[test]
fn a_path_with_a_control_character_types_nothing() {
    for quote in [Quote::Posix, Quote::PowerShell, Quote::Cmd] {
        let paths = [PathBuf::from("/ok"), PathBuf::from("/tmp/a\nls")];
        let error = typed(&paths, quote).unwrap_err();
        assert!(error.to_string().contains("control character"), "{error}");
    }
}
