//! A folder in a WSL distribution, `\\wsl.localhost\DISTRO\...`, whose git runs inside it: Windows' git reads that share slowly and refuses it as another's.

use std::path::{Path, PathBuf};

/// The share Windows names `folder`'s distribution by, `\\HOST\DISTRO`, and the folder's path inside it.
pub fn split(folder: &Path) -> Option<(&str, String)> {
    let text = folder.to_str()?;
    let host = ["\\\\wsl.localhost\\", "\\\\wsl$\\"]
        .into_iter()
        .find(|host| {
            text.get(..host.len())
                .is_some_and(|head| head.eq_ignore_ascii_case(host))
        })?;
    let end = text[host.len()..]
        .find('\\')
        .map_or(text.len(), |at| host.len() + at);
    if end == host.len() {
        return None;
    }
    let inside = text[end..].replace('\\', "/");
    Some((
        &text[..end],
        if inside.is_empty() {
            "/".into()
        } else {
            inside
        },
    ))
}

/// The distribution `share` names.
pub fn distro(share: &str) -> &str {
    share.rsplit_once('\\').map_or(share, |(_, name)| name)
}

/// A path git printed inside `share`'s distribution, as Windows names it.
pub fn join(share: &str, inside: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{share}{}",
        inside.display().to_string().replace('/', "\\")
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wsl_share_names_its_distribution_and_the_path_inside() {
        let folder = Path::new(r"\\wsl.localhost\Ubuntu-24.04\home\me\my repo");
        let (share, inside) = split(folder).expect("a WSL folder");
        assert_eq!(
            (share, inside.as_str()),
            (r"\\wsl.localhost\Ubuntu-24.04", "/home/me/my repo")
        );
        assert_eq!(distro(share), "Ubuntu-24.04");
        assert_eq!(
            join(share, Path::new("/home/me")),
            PathBuf::from(r"\\wsl.localhost\Ubuntu-24.04\home\me")
        );
        assert_eq!(
            split(Path::new(r"\\WSL$\Debian")),
            Some((r"\\WSL$\Debian", "/".into()))
        );
        assert_eq!(split(Path::new(r"\\wsl.localhost\")), None);
        assert_eq!(split(Path::new(r"\\server\share\x")), None);
        assert_eq!(split(Path::new(r"C:\Users\me")), None);
    }
}
