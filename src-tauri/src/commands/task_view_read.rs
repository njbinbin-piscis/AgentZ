//! Narrow task-scoped local reads. No renderer root, writes, processes or legacy IDE forwarding.
use crate::state::AppState;
use agentz_task_state::plan::PlanBinding;
use serde::Serialize;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
use tauri::{AppHandle, State, WebviewWindow};
const MAX_BYTES: u64 = 1_048_576;
const MAX_NODES: usize = 1000;
#[derive(Debug, Serialize)]
pub struct Entry {
    pub relative_path: String,
    pub is_dir: bool,
    pub size: u64,
}
#[derive(Debug, Serialize)]
pub struct Listing {
    pub scope: PlanBinding,
    pub workspace_root: String,
    pub entries: Vec<Entry>,
    pub truncated: bool,
}
#[derive(Debug, Serialize)]
pub struct TextFile {
    pub scope: PlanBinding,
    pub relative_path: String,
    pub content: String,
    pub bytes: u64,
}
fn relative(value: &str, allow_root: bool) -> Result<PathBuf, String> {
    if value.is_empty() && allow_root {
        return Ok(PathBuf::new());
    }
    if value.is_empty()
        || value.len() > 4096
        || value.chars().any(char::is_control)
        || value.contains(':')
        || value.contains('\\')
        || value.starts_with('/')
        || value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.trim() != s || s.ends_with('.'))
    {
        return Err("task read requires normal slash-separated relative path".into());
    }
    Ok(value.split('/').collect())
}
fn linked(meta: &std::fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        return meta.file_attributes() & 0x400 != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
}
fn checked(root: &Path, relative: &Path) -> Result<PathBuf, String> {
    let mut candidate = root.to_path_buf();
    if linked(&std::fs::symlink_metadata(root).map_err(|e| e.to_string())?) {
        return Err("task workspace link is unsupported".into());
    }
    for component in relative.components() {
        candidate.push(component);
        if linked(&std::fs::symlink_metadata(&candidate).map_err(|e| e.to_string())?) {
            return Err("task path links are unsupported".into());
        }
    }
    let canonical = std::fs::canonicalize(&candidate).map_err(|e| e.to_string())?;
    if !canonical.starts_with(root) {
        return Err("task path escaped workspace".into());
    }
    Ok(canonical)
}
#[cfg(windows)]
fn final_path(file: &std::fs::File) -> Result<PathBuf, String> {
    use std::os::windows::io::AsRawHandle;
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFinalPathNameByHandleW(
            handle: *mut std::ffi::c_void,
            path: *mut u16,
            length: u32,
            flags: u32,
        ) -> u32;
    }
    let mut buffer = vec![0u16; 32768];
    let n = unsafe {
        GetFinalPathNameByHandleW(
            file.as_raw_handle(),
            buffer.as_mut_ptr(),
            buffer.len() as u32,
            0,
        )
    };
    if n == 0 || n as usize >= buffer.len() {
        return Err("cannot verify task file handle final path".into());
    }
    use std::os::windows::ffi::OsStringExt;
    Ok(PathBuf::from(std::ffi::OsString::from_wide(
        &buffer[..n as usize],
    )))
}
#[cfg(windows)]
fn require_single_link(file: &std::fs::File) -> Result<(), String> {
    use std::os::windows::io::AsRawHandle;
    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }
    #[repr(C)]
    struct FileInformation {
        attributes: u32,
        creation: FileTime,
        last_access: FileTime,
        last_write: FileTime,
        volume_serial: u32,
        size_high: u32,
        size_low: u32,
        number_of_links: u32,
        index_high: u32,
        index_low: u32,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetFileInformationByHandle(
            handle: *mut std::ffi::c_void,
            information: *mut FileInformation,
        ) -> i32;
    }
    let mut information = std::mem::MaybeUninit::<FileInformation>::uninit();
    // SAFETY: repr(C) matches BY_HANDLE_FILE_INFORMATION (DWORD/FILETIME fields),
    // and the live File owns the handle for the full call. Inspect only on success.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), information.as_mut_ptr()) } == 0 {
        return Err("cannot verify task file hard-link count".into());
    }
    let information = unsafe { information.assume_init() };
    if information.number_of_links != 1 {
        return Err("task files with unknown or multiple hard links are unsupported".into());
    }
    Ok(())
}
#[cfg(windows)]
fn read_text(root: &Path, path: &Path) -> Result<(String, u64), String> {
    use std::os::windows::fs::OpenOptionsExt;
    // No share-delete or share-write: retain root/file identity while checking/reading.
    let root_handle = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x02000000 | 0x00200000)
        .open(root)
        .map_err(|e| e.to_string())?;
    let root_meta = root_handle.metadata().map_err(|e| e.to_string())?;
    if !root_meta.is_dir() || linked(&root_meta) {
        return Err("task root must be a normal directory".into());
    }
    let handle_root = final_path(&root_handle)?;
    if handle_root != std::fs::canonicalize(root).map_err(|e| e.to_string())? {
        return Err("task root handle changed".into());
    }
    let file = std::fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x00200000)
        .open(path)
        .map_err(|e| e.to_string())?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || linked(&metadata) || metadata.len() > MAX_BYTES {
        return Err("task text file is not regular or exceeds 1 MiB".into());
    }
    let actual = final_path(&file)?;
    if !actual.starts_with(&handle_root) || actual == handle_root {
        return Err("task file handle escaped workspace".into());
    }
    require_single_link(&file)?;
    let mut data = Vec::new();
    file.take(MAX_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|e| e.to_string())?;
    if data.len() as u64 > MAX_BYTES {
        return Err("task text file exceeds 1 MiB".into());
    }
    let n = data.len() as u64;
    let text = String::from_utf8(data).map_err(|_| "binary task preview is unsupported")?;
    if text.contains('\0') {
        return Err("binary task preview is unsupported".into());
    }
    Ok((text, n))
}
#[cfg(not(windows))]
fn read_text(_root: &Path, _path: &Path) -> Result<(String, u64), String> {
    Err("task handle-verified reads currently support Windows only".into())
}
fn list(root: &Path, start: &Path) -> Result<(Vec<Entry>, bool), String> {
    if !start.is_dir() {
        return Err("task list requires directory".into());
    }
    let mut queue = std::collections::VecDeque::from([(start.to_path_buf(), 0usize)]);
    let mut entries = Vec::new();
    let mut truncated = false;
    while let Some((directory, depth)) = queue.pop_front() {
        for item in std::fs::read_dir(&directory).map_err(|e| e.to_string())? {
            if entries.len() == MAX_NODES {
                truncated = true;
                return Ok((entries, truncated));
            }
            let item = item.map_err(|e| e.to_string())?;
            let path = item.path();
            let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            if linked(&meta) {
                return Err("task list links are unsupported".into());
            }
            let canonical = std::fs::canonicalize(&path).map_err(|e| e.to_string())?;
            if !canonical.starts_with(root) {
                return Err("task list escaped workspace".into());
            }
            let rel = canonical
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .to_str()
                .ok_or("task filename is not Unicode")?
                .replace('\\', "/");
            relative(&rel, false)?;
            let is_dir = meta.is_dir();
            entries.push(Entry {
                relative_path: rel,
                is_dir,
                size: if is_dir { 0 } else { meta.len() },
            });
            if is_dir {
                if depth < 2 {
                    queue.push_back((canonical, depth + 1));
                } else {
                    truncated = true;
                }
            }
        }
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok((entries, truncated))
}
/// Caller must retain a fresh verified task binding and its environment lease.
pub(crate) fn read_captured_text(
    root: &Path,
    relative_path: &str,
) -> Result<(String, u64), String> {
    let rel = relative(relative_path, false)?;
    let path = checked(root, &rel)?;
    read_text(root, &path)
}
#[tauri::command]
pub async fn task_view_read(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    relative_path: String,
) -> Result<TextFile, String> {
    relative(&relative_path, false)?;
    let _lease = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let captured =
        super::task_view::resolve_observation(&app, scope.project(), &session_id, scope.source())
            .await?;
    if captured.binding != expected_binding {
        return Err("stale task read binding".into());
    }
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("task read scope changed".into());
        }
        super::task_view::verify_observation(verified.project(), &session_id, &captured)?;
        let (content, bytes) = read_captured_text(&captured.root, &relative_path)?;
        super::task_view::verify_observation(verified.project(), &session_id, &captured)?;
        Ok(TextFile {
            scope: captured.binding.clone(),
            relative_path,
            content,
            bytes,
        })
    })
}
#[tauri::command]
pub async fn task_view_list(
    app: AppHandle,
    window: WebviewWindow,
    state: State<'_, AppState>,
    scope_handle: String,
    session_id: String,
    expected_binding: PlanBinding,
    relative_path: String,
) -> Result<Listing, String> {
    let rel = relative(&relative_path, true)?;
    let _lease = state.environment_access.execution().await;
    let scope = super::project_scope::lookup_for_main(&state, &window, &scope_handle)?;
    let captured =
        super::task_view::resolve_observation(&app, scope.project(), &session_id, scope.source())
            .await?;
    if captured.binding != expected_binding {
        return Err("stale task list binding".into());
    }
    super::project_scope::with_verified_scope(&state, &window, &scope_handle, |verified| {
        if verified.project() != scope.project() {
            return Err("task list scope changed".into());
        }
        super::task_view::verify_observation(verified.project(), &session_id, &captured)?;
        let path = checked(&captured.root, &rel)?;
        let (entries, truncated) = list(&captured.root, &path)?;
        super::task_view::verify_observation(verified.project(), &session_id, &captured)?;
        Ok(Listing {
            scope: captured.binding.clone(),
            workspace_root: captured.root.to_string_lossy().into_owned(),
            entries,
            truncated,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("task-read-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&p).unwrap();
            Self(std::fs::canonicalize(p).unwrap())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn path_input_rejects_escape_windows_and_remote_forms() {
        for path in [
            "",
            "../secret",
            "a/../b",
            "./a",
            "/abs",
            "C:/secret",
            r"C:\secret",
            r"\\server\share",
            "ssh://host/a",
            "a//b",
            "a\0b",
            "a/ trailing",
            "a/trailing.",
        ] {
            assert!(relative(path, false).is_err(), "accepted {path:?}");
        }
        assert!(relative("", true).unwrap().as_os_str().is_empty());
        assert_eq!(
            relative("src/main.rs", false).unwrap(),
            PathBuf::from("src").join("main.rs")
        );
    }
    #[test]
    fn list_caps_width_and_depth_and_returns_relative_names() {
        let f = Fixture::new();
        for i in 0..1001 {
            std::fs::write(f.0.join(format!("file-{i}")), b"x").unwrap();
        }
        let (entries, truncated) = list(&f.0, &f.0).unwrap();
        assert_eq!(entries.len(), 1000);
        assert!(truncated);
        assert!(entries
            .iter()
            .all(|e| !Path::new(&e.relative_path).is_absolute() && e.size == 1));
        let d = Fixture::new();
        std::fs::create_dir_all(d.0.join("a/b/c/d")).unwrap();
        std::fs::write(d.0.join("a/b/c/d/hidden"), b"x").unwrap();
        let (entries, truncated) = list(&d.0, &d.0).unwrap();
        assert!(truncated);
        assert_eq!(entries.len(), 3);
        assert!(!entries.iter().any(|e| e.relative_path.contains("hidden")));
    }
    #[cfg(windows)]
    #[test]
    fn same_handle_text_budget_binary_and_empty_are_checked() {
        let f = Fixture::new();
        let p = f.0.join("text");
        std::fs::write(&p, "你好").unwrap();
        assert_eq!(read_text(&f.0, &p).unwrap(), ("你好".into(), 6));
        std::fs::write(&p, b"").unwrap();
        assert_eq!(read_text(&f.0, &p).unwrap(), ("".into(), 0));
        std::fs::write(&p, [0xffu8]).unwrap();
        assert!(read_text(&f.0, &p).is_err());
        std::fs::write(&p, b"a\0b").unwrap();
        assert!(read_text(&f.0, &p).is_err());
        std::fs::write(&p, vec![b'x'; MAX_BYTES as usize]).unwrap();
        assert_eq!(read_text(&f.0, &p).unwrap().1, MAX_BYTES);
        std::fs::write(&p, vec![b'x'; MAX_BYTES as usize + 1]).unwrap();
        assert!(read_text(&f.0, &p).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn opened_file_outside_captured_root_is_rejected() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let p = outside.0.join("secret");
        std::fs::write(&p, b"synthetic outside").unwrap();
        assert!(read_text(&f.0, &p).is_err());
        assert!(checked(&f.0, Path::new("../secret")).is_err());
    }
    #[cfg(windows)]
    #[test]
    fn hard_link_alias_of_outside_file_is_rejected_without_removing_original() {
        let f = Fixture::new();
        let outside = Fixture::new();
        let original = outside.0.join("synthetic-secret");
        let alias = f.0.join("alias");
        std::fs::write(&original, b"outside synthetic content").unwrap();
        std::fs::hard_link(&original, &alias).unwrap();
        let handle = std::fs::File::open(&alias).unwrap();
        assert!(require_single_link(&handle).is_err());
        drop(handle);
        assert!(read_text(&f.0, &alias).is_err());
        std::fs::remove_file(&alias).unwrap();
        assert_eq!(
            std::fs::read(&original).unwrap(),
            b"outside synthetic content"
        );
        assert_eq!(
            read_text(&outside.0, &original).unwrap().0,
            "outside synthetic content"
        );
    }
    #[cfg(windows)]
    #[test]
    fn junction_escape_is_rejected_without_symlink_privilege() {
        let f = Fixture::new();
        let outside = Fixture::new();
        std::fs::write(outside.0.join("secret"), b"outside").unwrap();
        let link = f.0.join("junction");
        let output = piscis_kernel::proc::std_command("cmd")
            .args(["/D", "/C", "mklink", "/J"])
            .arg(&link)
            .arg(&outside.0)
            .output()
            .unwrap();
        assert!(output.status.success(), "fixture junction creation failed");
        assert!(checked(&f.0, Path::new("junction/secret")).is_err());
        assert!(read_text(&f.0, &link.join("secret")).is_err());
        assert!(list(&f.0, &f.0).is_err());
        // Remove only the junction itself before fixture Drop; never follow it recursively.
        std::fs::remove_dir(&link).unwrap();
        assert!(outside.0.join("secret").exists());
    }
}
