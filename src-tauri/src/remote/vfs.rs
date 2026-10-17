//! `agentz-remote://<authority>/<absolute path>` addressing.
//!
//! Plain paths stay local; URIs with this scheme are served by the
//! agentz-server attached to the matching remote authority.

pub const SCHEME: &str = "agentz-remote";
pub fn has_scheme(uri: &str) -> bool {
    uri.split_once(':')
        .is_some_and(|(scheme, _)| scheme.eq_ignore_ascii_case(SCHEME))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUri {
    pub authority: String,
    pub path: String,
}

pub fn parse(uri: &str) -> Option<RemoteUri> {
    // `PathBuf::join` on Windows inserts `\`; remote paths are always POSIX.
    let uri = uri.replace('\\', "/");
    let (scheme, rest) = uri.split_once(':')?;
    if !scheme.eq_ignore_ascii_case(SCHEME) {
        return None;
    }
    let rest = rest.strip_prefix("//")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    let authority = percent_decode(authority);
    let path = percent_decode(path);
    if authority
        .chars()
        .any(|c| c.is_control() || c == '/' || c == '\\')
        || path.contains('\0')
    {
        return None;
    }
    Some(RemoteUri { authority, path })
}

pub fn format(authority: &str, path: &str) -> String {
    let path = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    format!("{SCHEME}://{authority}{path}")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = |b: u8| match b {
                b'0'..=b'9' => Some(b - b'0'),
                b'a'..=b'f' => Some(b - b'a' + 10),
                b'A'..=b'F' => Some(b - b'A' + 10),
                _ => None,
            };
            if let (Some(high), Some(low)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                let b = high * 16 + low;
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_authority_and_path() {
        let u = parse("agentz-remote://ssh-remote%2Bbox/home/me/a%20b.rs").unwrap();
        assert_eq!(u.authority, "ssh-remote+box");
        assert_eq!(u.path, "/home/me/a b.rs");
        assert_eq!(parse("C:/x"), None);
    }

    #[test]
    fn round_trips() {
        let s = format("wsl+Ubuntu", "/etc/hosts");
        assert_eq!(parse(&s).unwrap().path, "/etc/hosts");
    }

    #[test]
    fn malformed_remote_remains_remote() {
        for uri in [
            "agentz-remote:/home",
            "AGENTZ-REMOTE:///home",
            "agentz-remote://box%2Fother/home",
            "agentz-remote://box%00/home",
            "agentz-remote://box/home%00",
        ] {
            assert!(has_scheme(uri));
            assert_eq!(parse(uri), None, "{uri}");
        }
        assert!(!has_scheme("C:\\home"));
        assert!(!has_scheme("/home/agentz-remote:x"));
    }

    #[test]
    fn unicode_and_incomplete_escapes_do_not_panic() {
        let uri = parse("AGENTZ-REMOTE://box/中文/%中/%A中/%/a%20b").unwrap();
        assert_eq!(uri.path, "/中文/%中/%A中/%/a b");
        assert_eq!(
            parse("agentz-remote://box\\home\\中文").unwrap().path,
            "/home/中文"
        );
    }
}
