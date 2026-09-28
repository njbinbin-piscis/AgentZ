//! Bound allocation even if a file grows between metadata and read.
use std::io::{self, Read};
use std::path::Path;

pub fn read(path: &Path, limit: u64) -> io::Result<Vec<u8>> {
    let file = std::fs::File::open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file is not regular or exceeds size limit",
        ));
    }
    let mut bytes = Vec::new();
    file.take(limit.saturating_add(1)).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "file exceeds size limit",
        ));
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_large_files_and_accepts_boundary() {
        let path = std::env::temp_dir().join(format!("agentz-read-limit-{}", std::process::id()));
        std::fs::write(&path, b"12345").unwrap();
        assert!(read(&path, 4).is_err());
        assert_eq!(read(&path, 5).unwrap(), b"12345");
        std::fs::remove_file(path).unwrap();
    }
}
