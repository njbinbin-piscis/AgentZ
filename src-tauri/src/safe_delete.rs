//! Deleting user files without losing them.
//!
//! Anything the app removes on the user's behalf is first moved into the
//! project's own recycle folder, `<project>/.agentz/trash/<stamp>/...`, so a
//! mistaken delete or discard can be undone by moving the file back.

use std::path::{Path, PathBuf};

/// The folder that owns `path`'s trash: the nearest ancestor that is a project
/// root (has `.agentz` or `.git`), else the file's own directory.
pub fn trash_owner(path: &Path) -> PathBuf {
    let start = if path.is_dir() { Some(path) } else { path.parent() };
    let mut cur = start;
    while let Some(dir) = cur {
        if dir.join(".agentz").is_dir() || dir.join(".git").exists() {
            return dir.to_path_buf();
        }
        cur = dir.parent();
    }
    start.map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."))
}

fn copy_dir_all(from: &Path, to: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(to)?;
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// Move `path` (file or folder) into the project's trash and return where it
/// went. A path that does not exist is not an error and returns `None`. If the
/// move fails the original is left exactly where it was.
pub fn move_to_trash(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let owner = trash_owner(path);
    let rel = path
        .strip_prefix(&owner)
        .map(Path::to_path_buf)
        .unwrap_or_else(|_| PathBuf::from(path.file_name().unwrap_or_default()));
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let base = owner.join(".agentz").join("trash").join(stamp);
    let mut dest = base.join(&rel);
    let mut n = 1;
    while dest.exists() {
        let name = rel
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        dest = base.join(rel.parent().unwrap_or(Path::new(""))).join(format!("{name}.{n}"));
        n += 1;
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create the trash folder, nothing was deleted: {e}"))?;
    }
    if std::fs::rename(path, &dest).is_ok() {
        return Ok(Some(dest));
    }
    // Rename can fail across volumes: copy, then remove only after the copy is whole.
    let copied = if path.is_dir() {
        copy_dir_all(path, &dest)
    } else {
        std::fs::copy(path, &dest).map(|_| ())
    };
    copied.map_err(|e| format!("could not copy to the trash, nothing was deleted: {e}"))?;
    let removed = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    removed.map_err(|e| format!("copied to the trash but could not remove the original: {e}"))?;
    Ok(Some(dest))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_project() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("agentz-trash-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(dir.join(".agentz")).unwrap();
        dir
    }

    #[test]
    fn a_deleted_file_is_recoverable_from_the_project_trash() {
        let root = temp_project();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::write(root.join("src").join("a.rs"), b"hours of work").unwrap();
        let moved = move_to_trash(&root.join("src").join("a.rs")).unwrap().unwrap();
        assert!(!root.join("src").join("a.rs").exists());
        assert_eq!(std::fs::read(&moved).unwrap(), b"hours of work");
        assert!(moved.starts_with(root.join(".agentz").join("trash")));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_deleted_folder_keeps_its_whole_tree() {
        let root = temp_project();
        std::fs::create_dir_all(root.join("pkg").join("deep")).unwrap();
        std::fs::write(root.join("pkg").join("deep").join("b.txt"), b"b").unwrap();
        std::fs::write(root.join("pkg").join("a.txt"), b"a").unwrap();
        let moved = move_to_trash(&root.join("pkg")).unwrap().unwrap();
        assert!(!root.join("pkg").exists());
        assert_eq!(std::fs::read(moved.join("deep").join("b.txt")).unwrap(), b"b");
        assert_eq!(std::fs::read(moved.join("a.txt")).unwrap(), b"a");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn two_deletes_of_the_same_name_do_not_overwrite_each_other() {
        let root = temp_project();
        std::fs::write(root.join("n.txt"), b"first").unwrap();
        let one = move_to_trash(&root.join("n.txt")).unwrap().unwrap();
        std::fs::write(root.join("n.txt"), b"second").unwrap();
        let two = move_to_trash(&root.join("n.txt")).unwrap().unwrap();
        assert_ne!(one, two);
        assert_eq!(std::fs::read(one).unwrap(), b"first");
        assert_eq!(std::fs::read(two).unwrap(), b"second");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn a_missing_path_is_not_an_error() {
        let root = temp_project();
        assert!(move_to_trash(&root.join("nope")).unwrap().is_none());
        let _ = std::fs::remove_dir_all(&root);
    }
}
