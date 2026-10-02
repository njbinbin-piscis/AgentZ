//! `agentz-remote://<authority>/<absolute path>` addressing.
//!
//! Plain paths stay local; URIs with this scheme are served by the
//! agentz-server attached to the matching remote authority.

pub const SCHEME: &str = "agentz-remote";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteUri {
    pub authority: String,
    pub path: String,
}

pub fn parse(uri: &str) -> Option<RemoteUri> {
    // `PathBuf::join` on Windows inserts `\`; remote paths are always POSIX.
    let uri = uri.replace('\\', "/");
    let rest = uri.strip_prefix(SCHEME)?.strip_prefix("://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, "/"),
    };
    if authority.is_empty() {
        return None;
    }
    Some(RemoteUri {
        authority: percent_decode(authority),
        path: percent_decode(path),
    })
}

pub fn format(authority: &str, path: &str) -> String {
    let path = if path.starts_with('/') { path.to_string() } else { format!("/{path}") };
    format!("{SCHEME}://{authority}{path}")
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
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
}
