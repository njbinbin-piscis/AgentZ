//! Locale-independent path text.
//!
//! Windows paths are UTF-16. Turning them into text with [`std::ffi::OsStr::to_string_lossy`]
//! uses that Unicode data directly and does not consult the ANSI or OEM code page, so a
//! Chinese path stays Chinese on a Japanese or Western system locale. The `\\?\` prefix
//! added by `canonicalize` is removed when what remains is an ordinary drive or UNC path.
//! In verbatim form `/` is not a separator, so leaving the prefix in tool output makes
//! later joins fail.

use std::ffi::OsStr;
use std::path::Path;

/// Unicode text for an OS string. Unpaired surrogates become U+FFFD; valid text,
/// including CJK, is preserved on every system locale.
pub fn os_text(value: &OsStr) -> String {
    value.to_string_lossy().into_owned()
}

/// Drop a Windows verbatim prefix when the remainder is a normal path.
pub fn strip_verbatim(text: &str) -> String {
    if let Some(rest) = text.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{rest}");
    }
    if let Some(rest) = text.strip_prefix(r"\\?\") {
        let bytes = rest.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && &bytes[1..3] == br":\" {
            return rest.to_string();
        }
    }
    text.to_string()
}

/// Absolute path for people and models, without a verbatim prefix.
pub fn display_path(path: &Path) -> String {
    strip_verbatim(&os_text(path.as_os_str()))
}

/// Path relative to `root` with `/` separators when `path` is inside `root`.
/// Otherwise the absolute path without a verbatim prefix.
pub fn display_path_under(path: &Path, root: &Path) -> String {
    match relative_under(path, root) {
        Some(rel) if rel.is_empty() => ".".to_string(),
        Some(rel) => rel,
        None => display_path(path),
    }
}

/// Like [`display_path_under`], but a path equal to `root` stays absolute.
/// Search roots and working directories read better that way than as `.`.
pub fn display_path_for_tool(path: &Path, root: &Path) -> String {
    let rel = display_path_under(path, root);
    if rel == "." {
        display_path(path)
    } else {
        rel
    }
}

/// Split a path on either separator. Comparison does not use the host's path
/// parser, so a Windows `\\?\` path is handled the same on every OS.
fn parts(path: &Path) -> Vec<String> {
    strip_verbatim(&os_text(path.as_os_str()))
        .split(['/', '\\'])
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn relative_under(path: &Path, root: &Path) -> Option<String> {
    let path = parts(path);
    let root = parts(root);
    if root.is_empty() || path.len() < root.len() {
        return None;
    }
    let inside = path
        .iter()
        .zip(root.iter())
        .all(|(path_part, root_part)| path_part.eq_ignore_ascii_case(root_part));
    if !inside {
        return None;
    }
    Some(path[root.len()..].join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbatim_drive_prefix_drops_and_cjk_stays_unicode() {
        let path = Path::new(r"\\?\D:\项目 空格\源码\中文.txt");
        assert_eq!(display_path(path), r"D:\项目 空格\源码\中文.txt");
        assert!(!display_path(path).contains(r"\\?\"));
        assert!(!display_path(path).contains('\u{FFFD}'));
    }

    #[test]
    fn verbatim_unc_and_volume_prefixes() {
        assert_eq!(
            display_path(Path::new(r"\\?\UNC\server\共享\a.txt")),
            r"\\server\共享\a.txt"
        );
        let volume = r"\\?\Volume{12345678-1234-1234-1234-123456789abc}\data";
        assert_eq!(display_path(Path::new(volume)), volume);
    }

    #[test]
    fn relative_display_ignores_verbatim_prefix_and_drive_case() {
        let root = Path::new(r"\\?\D:\Nutstore\项目\DimLab");
        let file = Path::new(r"d:\nutstore\项目\DimLab\public\execution.html");
        assert_eq!(
            display_path_under(file, root),
            "public/execution.html"
        );
        assert_eq!(display_path_for_tool(root, root), r"D:\Nutstore\项目\DimLab");
    }

    #[test]
    fn characters_outside_one_code_page_round_trip() {
        let path = Path::new(r"\\?\C:\work\中文\日本語\한글\café.txt");
        assert_eq!(
            display_path(path),
            r"C:\work\中文\日本語\한글\café.txt"
        );
    }
}
