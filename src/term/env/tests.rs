use super::*;

fn vars(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
    pairs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

#[test]
fn an_expanded_name_takes_what_the_registry_holds() {
    let raw = vars(&[("SCOOP_PATH", "D:\\Scoop\\shims")]);
    let held = HashMap::new();
    assert_eq!(
        expand("%SCOOP_PATH%\\bin", &raw, &held),
        "D:\\Scoop\\shims\\bin"
    );
}

#[test]
fn an_unheld_name_stays_as_it_is() {
    let raw = Vec::new();
    let held = HashMap::new();
    assert_eq!(expand("%NOPE%\\x", &raw, &held), "%NOPE%\\x");
}

#[test]
fn this_process_names_a_variable_the_registry_lacks() {
    let raw = Vec::new();
    let held = HashMap::from([("SystemRoot".to_string(), "C:\\WINDOWS".to_string())]);
    assert_eq!(
        expand("%SystemRoot%\\system32", &raw, &held),
        "C:\\WINDOWS\\system32"
    );
}

#[test]
fn a_name_matches_without_its_case() {
    let raw = vars(&[("SCOOP_PATH", "D:\\S")]);
    let held = HashMap::new();
    assert_eq!(expand("%scoop_path%", &raw, &held), "D:\\S");
}

#[test]
fn a_path_variable_is_the_one_that_joins() {
    assert!(is_path("Path"));
    assert!(is_path("PATH"));
    assert!(is_path("LibPath"));
    assert!(is_path("OS2LIBPATH"));
    assert!(!is_path("PATHEXT"));
    assert!(!is_path("SCOOP_PATH"));
}

#[test]
fn a_name_expands_through_another_name() {
    let raw = vars(&[("A", "%B%\\a"), ("B", "C:\\b")]);
    assert_eq!(expand("%A%", &raw, &HashMap::new()), "C:\\b\\a");
}

#[test]
fn a_path_variable_joins_what_the_earlier_key_listed() {
    let raw = join(Vec::new(), vars(&[("Path", "C:\\machine")]));
    let raw = join(raw, vars(&[("PATH", "C:\\user")]));
    assert_eq!(raw.len(), 1, "one name holds it: {raw:?}");
    assert_eq!(raw[0].0, "Path");
    assert_eq!(raw[0].1, "C:\\machine;C:\\user");
}

#[test]
fn another_key_replaces_what_the_earlier_one_held() {
    let raw = join(Vec::new(), vars(&[("HOME", "C:\\one")]));
    let raw = join(raw, vars(&[("home", "C:\\two")]));
    assert_eq!(raw.len(), 1, "one name holds it: {raw:?}");
    assert_eq!(raw[0].1, "C:\\two");
}

#[test]
fn a_name_of_another_case_is_one_variable() {
    let held = HashMap::from([("Path".to_string(), "C:\\stale".to_string())]);
    let env = merge(&held, vars(&[("PATH", "C:\\fresh")]));
    assert_eq!(env.len(), 1, "one name holds it: {env:?}");
    assert_eq!(
        env.get("PATH").map(String::as_str),
        Some("C:\\fresh;C:\\stale")
    );
}

#[test]
fn a_folder_is_listed_once() {
    let held = HashMap::new();
    assert_eq!(dedupe("C:\\a;C:\\b;C:\\a\\", &held, "Path"), "C:\\a;C:\\b");
}

#[test]
fn a_folder_only_this_process_holds_comes_after() {
    let held = HashMap::from([("Path".to_string(), "C:\\old;C:\\a".to_string())]);
    assert_eq!(dedupe("C:\\a", &held, "Path"), "C:\\a;C:\\old");
}

#[test]
fn a_stale_name_of_this_process_is_dropped() {
    let held = HashMap::from([("Path".to_string(), "C:\\%OLD%".to_string())]);
    assert_eq!(dedupe("C:\\a", &held, "Path"), "C:\\a");
}

#[test]
fn a_session_gets_the_registry_path() {
    let env = super::fresh();
    let path = env
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("Path"))
        .map(|(_, value)| value.clone())
        .expect("PATH");
    let mut seen = HashSet::new();
    for entry in path.split(';').filter(|entry| !entry.is_empty()) {
        let key = entry.trim_end_matches('\\').to_lowercase();
        assert!(seen.insert(key), "{entry} is listed twice");
    }
    assert!(
        path.to_lowercase().contains("system32"),
        "the system folders are there: {path}"
    );
    // Every name the session holds is expanded, so none of them waits in the path as text.
    for name in env.keys() {
        assert!(!path.contains(&format!("%{name}%")), "{name} waits: {path}");
    }
}
