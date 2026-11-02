//! Cross-process exclusive file locks. The OS releases a lock when its handle
//! closes or the owning process dies, so a held lock is live-owner evidence.

use std::{
    fs::{self, File, OpenOptions},
    path::Path,
    time::{Duration, Instant},
};

/// `Ok(true)` when the exclusive lock was taken, `Ok(false)` when another live handle holds it.
#[cfg(windows)]
pub fn try_exclusive(file: &File) -> Result<bool, String> {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Foundation::ERROR_LOCK_VIOLATION;
    use windows_sys::Win32::Storage::FileSystem::{
        LockFileEx, LOCKFILE_EXCLUSIVE_LOCK, LOCKFILE_FAIL_IMMEDIATELY,
    };
    use windows_sys::Win32::System::IO::OVERLAPPED;
    // SAFETY: OVERLAPPED is plain data; an all-zero value is a valid offset-0 request.
    let mut overlapped: OVERLAPPED = unsafe { std::mem::zeroed() };
    // SAFETY: the handle is owned by `file` for the duration of the call; the byte-range lock
    // covers byte 0 only and is released when the handle closes.
    let ok = unsafe {
        LockFileEx(
            file.as_raw_handle() as _,
            LOCKFILE_EXCLUSIVE_LOCK | LOCKFILE_FAIL_IMMEDIATELY,
            0,
            1,
            0,
            &mut overlapped,
        )
    };
    if ok != 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(ERROR_LOCK_VIOLATION as i32) {
        Ok(false)
    } else {
        Err(format!("file lock failed: {error}"))
    }
}

/// `Ok(true)` when the exclusive lock was taken, `Ok(false)` when another live handle holds it.
#[cfg(unix)]
pub fn try_exclusive(file: &File) -> Result<bool, String> {
    use std::os::unix::io::AsRawFd;
    // SAFETY: the descriptor is valid for the lifetime of `file`; flock has no memory effects.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } == 0 {
        return Ok(true);
    }
    let error = std::io::Error::last_os_error();
    if error.kind() == std::io::ErrorKind::WouldBlock {
        Ok(false)
    } else {
        Err(format!("file lock failed: {error}"))
    }
}

fn open_lock_file(path: &Path) -> Result<File, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("lock directory: {e}"))?;
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|e| format!("lock open: {e}"))
}

/// Takes the lock without waiting. `Ok(None)` means a live handle holds it.
pub fn try_lock(path: &Path) -> Result<Option<File>, String> {
    let file = open_lock_file(path)?;
    Ok(try_exclusive(&file)?.then_some(file))
}

/// Polls for the lock until `timeout` elapses.
pub fn lock_with_timeout(path: &Path, timeout: Duration) -> Result<File, String> {
    let file = open_lock_file(path)?;
    let deadline = Instant::now() + timeout;
    loop {
        if try_exclusive(&file)? {
            return Ok(file);
        }
        if Instant::now() >= deadline {
            return Err(format!("timed out waiting for lock {}", path.display()));
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// True when a live handle holds the lock at `path`. Missing files count as free.
pub fn is_held(path: &Path) -> Result<bool, String> {
    let file = match OpenOptions::new().read(true).write(true).open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(format!("lock probe: {error}")),
    };
    Ok(!try_exclusive(&file)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn second_handle_is_refused_until_the_first_closes() {
        let dir = std::env::temp_dir().join(format!("agentz-os-lock-{}", uuid::Uuid::new_v4()));
        let path = dir.join("a.lock");
        let held = try_lock(&path).unwrap().expect("first lock");
        assert!(try_lock(&path).unwrap().is_none());
        assert!(is_held(&path).unwrap());
        assert!(lock_with_timeout(&path, Duration::from_millis(80)).is_err());
        drop(held);
        assert!(!is_held(&path).unwrap());
        assert!(lock_with_timeout(&path, Duration::from_millis(80)).is_ok());
        let _ = fs::remove_dir_all(&dir);
    }
}
