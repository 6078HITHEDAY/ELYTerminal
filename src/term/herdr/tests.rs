use std::{fs, os::unix::net::UnixListener};

use super::*;

fn args(words: &str) -> Vec<String> {
    words.split(' ').map(str::to_owned).collect()
}

fn env(pairs: &[(&str, &str)]) -> HashMap<String, OsString> {
    pairs
        .iter()
        .map(|(name, value)| ((*name).to_owned(), OsString::from(value)))
        .collect()
}

fn found(words: &str, env: &HashMap<String, OsString>) -> PathBuf {
    follows(&args(words), env)
        .expect("a local client")
        .expect("herdr's client")
}

#[test]
fn a_client_finds_its_server_as_herdr_does() {
    let home = env(&[("HOME", "/h")]);
    let default = PathBuf::from("/h/.config/herdr/herdr.sock");
    let work = PathBuf::from("/h/.config/herdr/sessions/work/herdr.sock");
    assert_eq!(found("herdr", &home), default);
    assert_eq!(found("herdr client", &home), default);
    assert_eq!(found("herdr --session work", &home), work);
    assert_eq!(found("herdr --session=work", &home), work);
    assert_eq!(found("herdr session attach work", &home), work);
    assert_eq!(found("herdr --session default", &home), default);
    let config = env(&[("HOME", "/h"), ("XDG_CONFIG_HOME", "/x")]);
    assert_eq!(
        found("herdr", &config),
        PathBuf::from("/x/herdr/herdr.sock")
    );
    let socket = env(&[("HOME", "/h"), ("HERDR_SOCKET_PATH", "/s.sock")]);
    assert_eq!(found("herdr", &socket), PathBuf::from("/s.sock"));
    assert_eq!(found("herdr --session work", &socket), work);
    let named = env(&[("HOME", "/h"), ("HERDR_SESSION", "work")]);
    assert_eq!(found("herdr", &named), work);
    assert!(follows(&args("herdr"), &env(&[])).is_err());
}

#[test]
fn only_herdr_s_terminal_client_is_followed() {
    let home = env(&[("HOME", "/h")]);
    for words in [
        "herdr pane list",
        "herdr api snapshot",
        "herdr server",
        "herdr session list",
        "herdr --version",
        "herdr --session work status",
    ] {
        assert_eq!(follows(&args(words), &home).expect(words), None, "{words}");
    }
    for words in ["herdr --remote box", "herdr --remote=box --session work"] {
        let error = follows(&args(words), &home).expect_err(words);
        assert!(error.to_string().contains("another machine"), "{error}");
    }
}

fn reply(focused_pane: &str, cwd: &str) -> String {
    format!(
        r#"{{"id":"elyterminal","result":{{"type":"session_snapshot","snapshot":{{"version":"0.9.5","protocol":22,"focused_pane_id":{focused_pane},"workspaces":[],"tabs":[],"panes":[{{"pane_id":"w1:p1","cwd":"/a","focused":false}},{{"pane_id":"w1:p2","cwd":{cwd},"focused":true}}],"layouts":[],"agents":[]}}}}}}"#
    )
}

#[test]
fn the_focused_pane_s_folder_comes_from_herdr_s_snapshot() {
    let read = |focused_pane: &str, cwd: &str| focused(&reply(focused_pane, cwd));
    assert_eq!(
        read(r#""w1:p2""#, r#""/srv/b""#).expect("a folder"),
        Some("/srv/b".into())
    );
    assert_eq!(read("null", r#""/srv/b""#).expect("no focus"), None);
    assert_eq!(read(r#""w1:p2""#, "null").expect("no folder"), None);
    let error = read(r#""w1:p9""#, r#""/b""#).expect_err("a lost pane");
    assert!(error.to_string().contains("w1:p9"), "{error}");
    let error = read(r#""w1:p2""#, r#""b""#).expect_err("a relative folder");
    assert!(error.to_string().contains("relative"), "{error}");
    let failed =
        r#"{"id":"elyterminal","error":{"code":"invalid_request","message":"no such method"}}"#;
    let error = focused(failed).expect_err("an error");
    assert_eq!(error.to_string(), "invalid_request: no such method");
    assert!(focused("").is_err());
}

#[test]
fn herdr_answers_over_its_socket() {
    let dir = std::env::temp_dir().join(format!("elyterminal-herdr-{}", std::process::id()));
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("a stale test folder goes");
    }
    fs::create_dir_all(&dir).expect("a test folder");
    let socket = dir.join("herdr.sock");
    let listener = UnixListener::bind(&socket).expect("a socket");
    let answer = reply(r#""w1:p2""#, r#""/srv""#);
    let server = std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("a client");
        let mut request = String::new();
        BufReader::new(&stream)
            .read_line(&mut request)
            .expect("a request");
        (&stream)
            .write_all(format!("{answer}\n").as_bytes())
            .expect("a reply");
        request
    });
    assert_eq!(folder(&socket).expect("an answer"), Some("/srv".into()));
    assert_eq!(server.join().expect("the server").as_bytes(), SNAPSHOT);
    fs::remove_dir_all(&dir).expect("the test folder goes");
    let error = folder(&socket).expect_err("no server");
    assert!(format!("{error:#}").contains("asking"), "{error:#}");
    assert!(starting(&error), "{error:#}");
    assert!(!starting(&focused("").expect_err("no reply")));
}

/// macOS hides the variables of its own programs, such as `/bin/sleep`, so the test reads itself.
#[test]
fn a_process_reads_with_its_path_arguments_and_variables() {
    let pid = std::process::id() as i32;
    let read = path(pid).expect("its path");
    let (args, env) = launch(pid).expect("its arguments");
    let exe = std::env::current_exe().expect("this test's path");
    assert_eq!(fs::canonicalize(read).ok(), fs::canonicalize(exe).ok());
    assert_eq!(args, std::env::args().collect::<Vec<_>>());
    assert_eq!(env.get("HOME"), std::env::var_os("HOME").as_ref());
}

#[cfg(target_os = "macos")]
#[test]
fn macos_lays_out_the_arguments_after_the_program_s_path() {
    let mut buffer = 2i32.to_ne_bytes().to_vec();
    buffer.extend_from_slice(b"/usr/local/bin/herdr\0\0\0\0herdr\0\0HOME=/h\0X=a=b\0\0\0");
    let (args, env) = procargs(&buffer).expect("it parses");
    assert_eq!(args, ["herdr", ""]);
    assert_eq!(env.get("HOME"), Some(&OsString::from("/h")));
    assert_eq!(env.get("X"), Some(&OsString::from("a=b")));
    assert!(procargs(&[1, 0]).is_err());
}
