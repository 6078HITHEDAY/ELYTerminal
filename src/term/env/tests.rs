use super::*;

#[test]
fn an_expanded_name_takes_what_the_registry_holds() {
    let raw = HashMap::from([("SCOOP_PATH".to_string(), "D:\\Scoop\\shims".to_string())]);
    let held = HashMap::new();
    assert_eq!(
        expand("%SCOOP_PATH%\\bin", &raw, &held),
        "D:\\Scoop\\shims\\bin"
    );
}

#[test]
fn an_unheld_name_stays_as_it_is() {
    let raw = HashMap::new();
    let held = HashMap::new();
    assert_eq!(expand("%NOPE%\\x", &raw, &held), "%NOPE%\\x");
}

#[test]
fn this_process_names_a_variable_the_registry_lacks() {
    let raw = HashMap::new();
    let held = HashMap::from([("SystemRoot".to_string(), "C:\\WINDOWS".to_string())]);
    assert_eq!(
        expand("%SystemRoot%\\system32", &raw, &held),
        "C:\\WINDOWS\\system32"
    );
}

#[test]
fn a_name_matches_without_its_case() {
    let raw = HashMap::from([("SCOOP_PATH".to_string(), "D:\\S".to_string())]);
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
    let raw = HashMap::from([
        ("A".to_string(), "%B%\\a".to_string()),
        ("B".to_string(), "C:\\b".to_string()),
    ]);
    assert_eq!(expand("%A%", &raw, &HashMap::new()), "C:\\b\\a");
}

#[test]
fn a_session_gets_the_registry_path() {
    let env = super::fresh();
    let path = env.get("Path").or_else(|| env.get("PATH")).expect("PATH");
    assert!(!path.contains('%'), "no %NAME% survives: {path}");
    assert!(
        path.to_lowercase().contains("system32"),
        "the system folders are there: {path}"
    );
    // cmd.exe reads 8191 characters of PATH at most, so a folder may appear once and no more.
    assert!(
        path.len() <= 8191,
        "PATH is {} characters, past what cmd.exe reads: {path}",
        path.len()
    );
    let mut seen = std::collections::HashSet::new();
    for entry in path.split(';').filter(|entry| !entry.is_empty()) {
        let key = entry.trim_end_matches('\\').to_lowercase();
        assert!(seen.insert(key), "{entry} is listed twice");
    }
}
