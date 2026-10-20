//! Windows gives a new session the environment the registry holds now, not the one this process
//! started with, as Windows Terminal's `compatibility.reloadEnvironmentVariables` does: a folder
//! added to `PATH` after ELYTerminal opened reaches the next session, and `%SCOOP_PATH%` is
//! expanded rather than passed as text.
//!
//! A path variable takes the registry's value, since that is the fresh one, then what this process
//! holds that it lacks, so a folder the app was started with survives without repeating.

use std::collections::{HashMap, HashSet};

use windows_sys::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows_sys::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, REG_EXPAND_SZ, REG_SZ, RegCloseKey,
    RegEnumValueW, RegOpenKeyExW,
};

/// The variables Windows reads again for each session, in the order `til::env::regenerate` reads them.
const MACHINE: &str = "SYSTEM\\CurrentControlSet\\Control\\Session Manager\\Environment";
const USER: &str = "Environment";
const VOLATILE: &str = "Volatile Environment";

/// The environment a session starts with: this process's, brought up to date from the registry.
pub fn fresh() -> HashMap<String, String> {
    let held: HashMap<String, String> = std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect();
    let mut env = held.clone();
    let mut raw: HashMap<String, String> = HashMap::new();
    for (root, path) in [
        (HKEY_LOCAL_MACHINE, MACHINE),
        (HKEY_CURRENT_USER, USER),
        (HKEY_CURRENT_USER, VOLATILE),
    ] {
        match read(root, path) {
            // A path variable joins what the earlier key listed, as Windows Terminal's `concat_var` does.
            Ok(found) => {
                for (key, value) in found {
                    match raw.get_mut(&key) {
                        Some(joined) if is_path(&key) => {
                            if !joined.ends_with(';') {
                                joined.push(';');
                            }
                            joined.push_str(&value);
                        }
                        _ => {
                            raw.insert(key, value);
                        }
                    }
                }
            }
            Err(error) => {
                log::error!("pty: cannot read {path}, whose variables stay as they were: {error}")
            }
        }
    }
    for (key, value) in &raw {
        let value = expand(value, &raw, &held);
        if is_path(key) {
            // The registry lists the folders a fresh logon has; this process may hold others.
            let mut joined = value;
            let mut seen: HashSet<String> = joined
                .split(';')
                .filter(|entry| !entry.is_empty())
                .map(|entry| entry.trim_end_matches('\\').to_lowercase())
                .collect();
            if let Some(previous) = lookup(key, &held) {
                for entry in previous.split(';').filter(|entry| !entry.is_empty()) {
                    // A `%NAME%` this process was started with is stale, since the registry's expansion replaced it.
                    if entry.contains('%') {
                        continue;
                    }
                    if seen.insert(entry.trim_end_matches('\\').to_lowercase()) {
                        if !joined.ends_with(';') {
                            joined.push(';');
                        }
                        joined.push_str(entry);
                    }
                }
            }
            env.insert(key.clone(), joined);
        } else {
            env.insert(key.clone(), value);
        }
    }
    env
}

/// Whether `name` holds a list of folders, which joins rather than replaces.
fn is_path(name: &str) -> bool {
    ["Path", "LibPath", "Os2LibPath"]
        .iter()
        .any(|known| known.eq_ignore_ascii_case(name))
}

/// `value` with each `%NAME%` replaced from the registry, else from what this process holds.
fn expand(value: &str, raw: &HashMap<String, String>, held: &HashMap<String, String>) -> String {
    let find = |name: &str| lookup(name, raw).or_else(|| lookup(name, held));
    let mut text = value.to_string();
    for _ in 0..8 {
        let mut changed = false;
        let mut out = String::with_capacity(text.len());
        let mut rest = text.as_str();
        while let Some(open) = rest.find('%') {
            let after = &rest[open + 1..];
            let Some(close) = after.find('%') else { break };
            let name = &after[..close];
            out.push_str(&rest[..open]);
            match find(name) {
                Some(found) => {
                    out.push_str(&found);
                    changed = true;
                }
                // A name nothing holds stays as it is, as Windows leaves one it cannot resolve.
                None => {
                    out.push('%');
                    out.push_str(name);
                    out.push('%');
                }
            }
            rest = &after[close + 1..];
        }
        out.push_str(rest);
        text = out;
        if !changed {
            break;
        }
    }
    text
}

/// What `name` holds, matched as Windows matches a variable's name.
fn lookup(name: &str, map: &HashMap<String, String>) -> Option<String> {
    map.get(name)
        .or_else(|| {
            map.iter()
                .find(|(key, _)| key.eq_ignore_ascii_case(name))
                .map(|(_, found)| found)
        })
        .cloned()
}

/// Each string value `root\path` holds, as written, so `%NAME%` stays for `expand`.
fn read(root: HKEY, path: &str) -> Result<HashMap<String, String>, String> {
    let mut key: HKEY = std::ptr::null_mut();
    let wide = wide(path);
    // SAFETY: `wide` outlives the call, and `key` is closed once no longer read.
    let status = unsafe { RegOpenKeyExW(root, wide.as_ptr(), 0, KEY_READ, &mut key) };
    if status == ERROR_FILE_NOT_FOUND {
        return Ok(HashMap::new());
    }
    if status != ERROR_SUCCESS {
        return Err(format!("RegOpenKeyExW({path}) failed, {status}"));
    }
    let mut found = HashMap::new();
    let mut name = vec![0u16; 1024];
    let mut data = vec![0u8; 128 * 1024];
    let mut index = 0u32;
    loop {
        // Both counts are the room each buffer offers on this pass.
        let mut name_len = name.len() as u32;
        let mut kind = 0u32;
        let mut data_len = data.len() as u32;
        // SAFETY: each buffer is as long as the length passed beside it.
        let status = unsafe {
            RegEnumValueW(
                key,
                index,
                name.as_mut_ptr(),
                &mut name_len,
                std::ptr::null_mut(),
                &mut kind,
                data.as_mut_ptr(),
                &mut data_len,
            )
        };
        if status == ERROR_NO_MORE_ITEMS || status == ERROR_FILE_NOT_FOUND {
            break;
        }
        if status != ERROR_SUCCESS {
            // SAFETY: `key` came from RegOpenKeyExW and is closed once.
            unsafe { RegCloseKey(key) };
            return Err(format!("RegEnumValueW({path}, {index}) failed, {status}"));
        }
        index += 1;
        let name = String::from_utf16_lossy(&name[..name_len as usize]);
        // Only the two text kinds hold a value; a number or a binary block names no variable.
        if name.is_empty() || !matches!(kind, REG_SZ | REG_EXPAND_SZ) {
            continue;
        }
        let text = String::from_utf16_lossy(&utf16(&data, data_len as usize));
        found.insert(name, text);
    }
    // SAFETY: `key` came from RegOpenKeyExW and is closed once.
    unsafe { RegCloseKey(key) };
    Ok(found)
}

/// The UTF-16 text `bytes` holds, cut at its first null.
fn utf16(bytes: &[u8], len: usize) -> Vec<u16> {
    bytes[..len.min(bytes.len())]
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

#[cfg(test)]
mod tests;
