//! Reversible Windows ACL scope used by the AppContainer child-process policy.
//!
//! This module deliberately does not launch processes. Callers must keep the
//! AppContainer profile alive longer than the lease, and must finish/drop the
//! lease only after the Job Object has reaped every child.

use anyhow::{bail, Context, Result};
use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs::File;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::io::{FromRawHandle, RawHandle};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};
use windows::core::{PCWSTR, PWSTR};
use windows::Win32::Foundation::{
    CloseHandle, LocalFree, SetHandleInformation, HANDLE, HANDLE_FLAGS, HANDLE_FLAG_INHERIT,
    HLOCAL, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
#[cfg(test)]
use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SetNamedSecurityInfoW};
use windows::Win32::Security::Authorization::{
    GetSecurityInfo, SetEntriesInAclW, SetSecurityInfo, EXPLICIT_ACCESS_W, GRANT_ACCESS,
    REVOKE_ACCESS, SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
};
use windows::Win32::Security::Isolation::{CreateAppContainerProfile, DeleteAppContainerProfile};
use windows::Win32::Security::{
    ACE_FLAGS, ACL, CONTAINER_INHERIT_ACE, DACL_SECURITY_INFORMATION, OBJECT_INHERIT_ACE,
    OBJECT_SECURITY_INFORMATION, PROTECTED_DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
    SECURITY_ATTRIBUTES, SECURITY_CAPABILITIES, SE_DACL_PROTECTED,
    UNPROTECTED_DACL_SECURITY_INFORMATION,
};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FileAttributeTagInfo, FileIdBothDirectoryInfo, FileIdBothDirectoryRestartInfo,
    GetFileInformationByHandle, GetFileInformationByHandleEx, BY_HANDLE_FILE_INFORMATION,
    FILE_ALL_ACCESS, FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_ATTRIBUTE_TAG_INFO, FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
    FILE_GENERIC_EXECUTE, FILE_ID_BOTH_DIR_INFO, FILE_LIST_DIRECTORY, FILE_READ_ATTRIBUTES,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING, READ_CONTROL, WRITE_DAC,
};
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject, JOBOBJECT_BASIC_ACCOUNTING_INFORMATION,
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};
use windows::Win32::System::Pipes::CreatePipe;
use windows::Win32::System::Threading::{
    CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
    InitializeProcThreadAttributeList, ResumeThread, TerminateProcess, UpdateProcThreadAttribute,
    WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT,
    EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
    PROC_THREAD_ATTRIBUTE_HANDLE_LIST, PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES,
    STARTF_USESTDHANDLES, STARTUPINFOEXW,
};
use windows_sys::Wdk::Foundation::OBJECT_ATTRIBUTES as NT_OBJECT_ATTRIBUTES;
use windows_sys::Wdk::Storage::FileSystem::{NtCreateFile, FILE_OPEN, FILE_OPEN_REPARSE_POINT};
use windows_sys::Win32::Foundation::UNICODE_STRING as NT_UNICODE_STRING;
use windows_sys::Win32::System::IO::IO_STATUS_BLOCK as NT_IO_STATUS_BLOCK;

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

fn process_working_directory(path: &Path) -> Result<PathBuf> {
    let encoded = path.as_os_str().encode_wide().collect::<Vec<_>>();
    const EXTENDED_PREFIX: &[u16] = &[92, 92, 63, 92];
    const EXTENDED_UNC_PREFIX: &[u16] = &[92, 92, 63, 92, 85, 78, 67, 92];
    let disk_path = if encoded.starts_with(EXTENDED_UNC_PREFIX) {
        anyhow::bail!("UNC workspaces are not supported by the AppContainer command runner");
    } else if encoded.starts_with(EXTENDED_PREFIX) {
        &encoded[EXTENDED_PREFIX.len()..]
    } else {
        anyhow::ensure!(
            !encoded.starts_with(&[92, 92]),
            "UNC workspaces are not supported by the AppContainer command runner"
        );
        &encoded
    };
    anyhow::ensure!(
        disk_path.len() >= 3
            && ((b'A' as u16..=b'Z' as u16).contains(&disk_path[0])
                || (b'a' as u16..=b'z' as u16).contains(&disk_path[0]))
            && disk_path[1] == b':' as u16
            && disk_path[2] == b'\\' as u16,
        "AppContainer command working directory must resolve to a local drive path"
    );
    Ok(PathBuf::from(std::ffi::OsString::from_wide(disk_path)))
}

/// Result returned by the Windows restricted child-process runner.
#[derive(Debug)]
pub struct AppContainerOutput {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub timed_out: bool,
    pub cancelled: bool,
    /// Bytes actually read before EOF, including bytes omitted by the capture limit.
    pub stdout_total_bytes: u64,
    pub stderr_total_bytes: u64,
}

impl AppContainerOutput {
    /// Ok is returned only after root exit, zero active Job members, both EOF readers
    /// and workspace ACL cleanup. Early errors have no such confirmation.
    pub fn recovery_report(
        &self,
        operation_id: String,
    ) -> Result<crate::tool_recovery::RecoveryReport> {
        crate::tool_recovery::RecoveryReport::confirmed_result(
            operation_id,
            self.exit_code,
            self.timed_out,
            self.cancelled,
            self.stdout_total_bytes,
            self.stdout.len() as u64,
            self.stderr_total_bytes,
            self.stderr.len() as u64,
        )
        .map_err(anyhow::Error::msg)
    }
}

struct AppContainerProfile {
    name: Vec<u16>,
    sid: windows::Win32::Security::PSID,
}

impl AppContainerProfile {
    fn create() -> Result<Self> {
        let name = wide(OsStr::new(&format!("AgentZ-{}", uuid::Uuid::new_v4())));
        let display = wide(OsStr::new("AgentZ restricted command"));
        let description = wide(OsStr::new("No-network command with scoped workspace ACL"));
        let sid = unsafe {
            CreateAppContainerProfile(
                PCWSTR(name.as_ptr()),
                PCWSTR(display.as_ptr()),
                PCWSTR(description.as_ptr()),
                None,
            )?
        };
        Ok(Self { name, sid })
    }
}

impl Drop for AppContainerProfile {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Security::FreeSid(self.sid);
            let _ = DeleteAppContainerProfile(PCWSTR(self.name.as_ptr()));
        }
    }
}

struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn new(handle: HANDLE) -> Self {
        Self(handle)
    }

    fn raw(&self) -> HANDLE {
        self.0
    }

    fn into_file(mut self) -> File {
        let raw = self.0;
        self.0 = HANDLE(std::ptr::null_mut());
        unsafe { File::from_raw_handle(raw.0 as RawHandle) }
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_invalid() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/// Launch a Windows command in a fresh zero-capability AppContainer.
///
/// The package SID receives access only to `workspace_root` and the caller-supplied environment. The
/// process is assigned to a kill-on-close Job Object before its first instruction. Timeout or
/// cooperative cancellation terminates the job, drains both output streams and restores ACLs.
/// Any setup or cleanup error is returned; callers must not retry the same command unsandboxed.
pub fn run_appcontainer_command(
    program: &OsStr,
    args: &[std::ffi::OsString],
    workspace_root: &Path,
    cwd: &Path,
    timeout: Duration,
    cancel: Arc<AtomicBool>,
    env_overrides: &[(std::ffi::OsString, std::ffi::OsString)],
    max_output_bytes: usize,
) -> Result<AppContainerOutput> {
    anyhow::ensure!(timeout > Duration::ZERO, "sandbox timeout must be positive");
    let workspace_root = workspace_root
        .canonicalize()
        .context("resolve sandbox workspace root")?;
    anyhow::ensure!(
        workspace_root.is_dir(),
        "sandbox workspace root is not a directory"
    );
    let cwd = cwd
        .canonicalize()
        .context("resolve sandbox working directory")?;
    anyhow::ensure!(
        cwd.is_dir() && cwd.starts_with(&workspace_root),
        "sandbox working directory must be inside workspace_root"
    );
    let force_unprivileged_job =
        command_requires_unprivileged_toolchain_fallback(program, args, env_overrides);
    let mut profile = None;
    let mut workspace_lease = None;
    if !force_unprivileged_job {
        let created_profile = AppContainerProfile::create()?;
        match unsafe { WorkspaceAclLease::grant(&workspace_root, created_profile.sid) } {
            Ok(lease) => {
                workspace_lease = Some(lease);
                profile = Some(created_profile);
            }
            Err(error) if is_write_dac_denied_error(&error) => {
                drop(created_profile);
            }
            Err(error) => return Err(error),
        }
    }
    let use_appcontainer = workspace_lease.is_some();
    let _pinned_readonly_workspace = if !use_appcontainer {
        Some(verify_and_pin_readonly_workspace(&workspace_root)?)
    } else {
        None
    };

    let job = OwnedHandle::new(unsafe { CreateJobObjectW(None, PCWSTR::null())? });
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    unsafe {
        SetInformationJobObject(
            job.raw(),
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )?;
    }

    let security = SECURITY_ATTRIBUTES {
        nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
        lpSecurityDescriptor: std::ptr::null_mut(),
        bInheritHandle: true.into(),
    };
    let (stdin_read, stdin_write) = create_pipe(&security)?;
    let (stdout_read, stdout_write) = create_pipe(&security)?;
    let (stderr_read, stderr_write) = create_pipe(&security)?;
    unsafe {
        SetHandleInformation(stdin_write.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))?;
        SetHandleInformation(stdout_read.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))?;
        SetHandleInformation(stderr_read.raw(), HANDLE_FLAG_INHERIT.0, HANDLE_FLAGS(0))?;
    }

    let attribute_list = AttributeList::new(if use_appcontainer { 2 } else { 1 })?;
    let capabilities = SECURITY_CAPABILITIES {
        AppContainerSid: profile.as_ref().map(|p| p.sid).unwrap_or_default(),
        Capabilities: std::ptr::null_mut(),
        CapabilityCount: 0,
        Reserved: 0,
    };
    let inherited = [stdin_read.raw(), stdout_write.raw(), stderr_write.raw()];
    unsafe {
        if use_appcontainer {
            UpdateProcThreadAttribute(
                attribute_list.raw(),
                0,
                PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                Some((&capabilities as *const SECURITY_CAPABILITIES).cast()),
                std::mem::size_of::<SECURITY_CAPABILITIES>(),
                None,
                None,
            )?;
        }
        UpdateProcThreadAttribute(
            attribute_list.raw(),
            0,
            PROC_THREAD_ATTRIBUTE_HANDLE_LIST as usize,
            Some(inherited.as_ptr().cast()),
            std::mem::size_of_val(&inherited),
            None,
            None,
        )?;
    }

    let program_wide = wide(program);
    let mut command_line = make_command_line(program, args);
    let process_cwd = process_working_directory(&cwd)?;
    let cwd_wide = wide(process_cwd.as_os_str());
    let mut environment = build_environment(env_overrides)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = std::mem::size_of::<STARTUPINFOEXW>() as u32;
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = stdin_read.raw();
    startup.StartupInfo.hStdOutput = stdout_write.raw();
    startup.StartupInfo.hStdError = stderr_write.raw();
    startup.lpAttributeList = attribute_list.raw();
    let mut info = PROCESS_INFORMATION::default();
    unsafe {
        CreateProcessW(
            PCWSTR(program_wide.as_ptr()),
            PWSTR(command_line.as_mut_ptr()),
            None,
            None,
            true,
            EXTENDED_STARTUPINFO_PRESENT
                | CREATE_NO_WINDOW
                | CREATE_SUSPENDED
                | CREATE_UNICODE_ENVIRONMENT,
            Some(environment.as_mut_ptr().cast()),
            PCWSTR(cwd_wide.as_ptr()),
            &startup.StartupInfo,
            &mut info,
        )?;
    }
    let process = OwnedHandle::new(info.hProcess);
    let thread_handle = OwnedHandle::new(info.hThread);
    let assign_result = unsafe { AssignProcessToJobObject(job.raw(), process.raw()) };
    if let Err(error) = assign_result {
        unsafe {
            let _ = TerminateProcess(process.raw(), 125);
        }
        return Err(error.into());
    }
    if unsafe { ResumeThread(thread_handle.raw()) } == u32::MAX {
        unsafe {
            let _ = TerminateJobObject(job.raw(), 125);
        }
        bail!("ResumeThread failed for AppContainer command");
    }

    drop(thread_handle);
    drop(stdin_read);
    drop(stdin_write);
    drop(stdout_write);
    drop(stderr_write);
    let stdout_thread = spawn_capture(stdout_read.into_file(), max_output_bytes);
    let stderr_thread = spawn_capture(stderr_read.into_file(), max_output_bytes);
    let start = Instant::now();
    let mut timed_out = false;
    let mut cancelled = false;
    loop {
        if cancel.load(Ordering::Relaxed) {
            cancelled = true;
            unsafe {
                TerminateJobObject(job.raw(), 130)?;
            }
            break;
        }
        let elapsed = start.elapsed();
        if elapsed >= timeout {
            timed_out = true;
            unsafe {
                TerminateJobObject(job.raw(), 124)?;
            }
            break;
        }
        let wait = timeout
            .saturating_sub(elapsed)
            .min(Duration::from_millis(100));
        let millis = wait.as_millis().max(1).min(u32::MAX as u128) as u32;
        let state = unsafe { WaitForSingleObject(process.raw(), millis) };
        if state == WAIT_OBJECT_0 {
            // A command may leave background children holding the output pipes; the job must
            // not outlive the tool result, so terminate any remaining members before draining.
            unsafe {
                TerminateJobObject(job.raw(), 0)?;
            }
            break;
        }
        if state != WAIT_TIMEOUT {
            unsafe {
                TerminateJobObject(job.raw(), 125)?;
            }
            bail!("WaitForSingleObject failed for AppContainer command");
        }
    }
    let root_exit = unsafe { WaitForSingleObject(process.raw(), 5_000) };
    anyhow::ensure!(
        root_exit == WAIT_OBJECT_0,
        "AppContainer root process did not exit after Job Object termination"
    );
    let mut exit_code = if timed_out {
        124
    } else if cancelled {
        130
    } else {
        125
    };
    if !timed_out && !cancelled {
        unsafe {
            GetExitCodeProcess(process.raw(), &mut exit_code)?;
        }
    }
    drop(process);
    wait_for_job_empty(job.raw(), Duration::from_secs(5))?;
    drop(job);
    // Join both readers and attempt ACL cleanup even if one stage fails. A failed stage
    // must not skip the other reader or be converted into confirmed cleanup.
    let stdout = stdout_thread
        .join()
        .map_err(|_| anyhow::anyhow!("stdout capture thread panicked"))
        .and_then(|result| result);
    let stderr = stderr_thread
        .join()
        .map_err(|_| anyhow::anyhow!("stderr capture thread panicked"))
        .and_then(|result| result);
    let acl_cleanup = if let Some(ref mut workspace) = workspace_lease {
        workspace.finish()
    } else {
        Ok(())
    };
    let stdout = stdout?;
    let stderr = stderr?;
    acl_cleanup?;
    drop(workspace_lease);
    drop(_pinned_readonly_workspace);
    drop(profile);
    Ok(AppContainerOutput {
        exit_code: exit_code as i32,
        stdout: stdout.bytes,
        stderr: stderr.bytes,
        timed_out,
        cancelled,
        stdout_total_bytes: stdout.total_bytes,
        stderr_total_bytes: stderr.total_bytes,
    })
}

fn create_pipe(security: &SECURITY_ATTRIBUTES) -> Result<(OwnedHandle, OwnedHandle)> {
    let mut read = HANDLE::default();
    let mut write = HANDLE::default();
    unsafe {
        CreatePipe(&mut read, &mut write, Some(security), 0)?;
    }
    Ok((OwnedHandle::new(read), OwnedHandle::new(write)))
}

fn wait_for_job_empty(job: HANDLE, timeout: Duration) -> Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        unsafe {
            QueryInformationJobObject(
                job,
                JobObjectBasicAccountingInformation,
                (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                std::mem::size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                None,
            )?;
        }
        if accounting.ActiveProcesses == 0 {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "AppContainer Job Object retained {} process(es) after termination",
            accounting.ActiveProcesses
        );
        thread::sleep(Duration::from_millis(20));
    }
}

struct AttributeList {
    memory: *mut u8,
    layout: std::alloc::Layout,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
    initialized: bool,
}

impl AttributeList {
    fn new(count: u32) -> Result<Self> {
        let mut size = 0usize;
        unsafe {
            let _ = InitializeProcThreadAttributeList(
                LPPROC_THREAD_ATTRIBUTE_LIST::default(),
                count,
                0,
                &mut size,
            );
        }
        anyhow::ensure!(
            size > 0,
            "Windows did not report process attribute-list size"
        );
        let layout = std::alloc::Layout::from_size_align(size, std::mem::align_of::<usize>())?;
        let memory = unsafe { std::alloc::alloc_zeroed(layout) };
        anyhow::ensure!(
            !memory.is_null(),
            "failed to allocate process attribute list"
        );
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(memory.cast());
        if let Err(error) = unsafe { InitializeProcThreadAttributeList(list, count, 0, &mut size) }
        {
            unsafe {
                std::alloc::dealloc(memory, layout);
            }
            return Err(error.into());
        }
        Ok(Self {
            memory,
            layout,
            list,
            initialized: true,
        })
    }

    fn raw(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.list
    }
}

impl Drop for AttributeList {
    fn drop(&mut self) {
        unsafe {
            if self.initialized {
                DeleteProcThreadAttributeList(self.list);
            }
            std::alloc::dealloc(self.memory, self.layout);
        }
    }
}

struct CapturedStream {
    bytes: Vec<u8>,
    total_bytes: u64,
}

fn spawn_capture(mut file: File, limit: usize) -> thread::JoinHandle<Result<CapturedStream>> {
    thread::spawn(move || {
        use std::io::Read;
        let mut captured = Vec::with_capacity(limit.min(64 * 1024));
        let mut buffer = [0u8; 8192];
        let mut total_bytes = 0u64;
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            total_bytes = total_bytes
                .checked_add(read as u64)
                .context("sandbox captured byte count overflow")?;
            let remaining = limit.saturating_sub(captured.len());
            captured.extend_from_slice(&buffer[..read.min(remaining)]);
        }
        Ok(CapturedStream {
            bytes: captured,
            total_bytes,
        })
    })
}

fn make_command_line(program: &OsStr, args: &[std::ffi::OsString]) -> Vec<u16> {
    let mut line = quote_windows_arg(program);
    for arg in args {
        line.push(b' ' as u16);
        line.extend(quote_windows_arg(arg));
    }
    line.push(0);
    line
}

fn quote_windows_arg(value: &OsStr) -> Vec<u16> {
    let input = value.encode_wide().collect::<Vec<_>>();
    if !input.iter().any(|ch| matches!(*ch, 32 | 9 | 34)) {
        return input;
    }
    let mut out = vec![34];
    let mut backslashes = 0usize;
    for ch in input {
        if ch == 92 {
            backslashes += 1;
        } else if ch == 34 {
            out.extend(std::iter::repeat(92).take(backslashes * 2 + 1));
            out.push(34);
            backslashes = 0;
        } else {
            out.extend(std::iter::repeat(92).take(backslashes));
            backslashes = 0;
            out.push(ch);
        }
    }
    out.extend(std::iter::repeat(92).take(backslashes * 2));
    out.push(34);
    out
}

fn build_environment(overrides: &[(std::ffi::OsString, std::ffi::OsString)]) -> Result<Vec<u16>> {
    const ALLOWED: &[&str] = &[
        "COMSPEC",
        "PATH",
        "PATHEXT",
        "SYSTEMROOT",
        "WINDIR",
        "TEMP",
        "TMP",
        "USERPROFILE",
        "HOMEDRIVE",
        "HOMEPATH",
        "HOME",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "APPDATA",
        "LOCALAPPDATA",
        "PROGRAMFILES",
        "PROGRAMFILES(X86)",
        "PROCESSOR_ARCHITECTURE",
        "NUMBER_OF_PROCESSORS",
    ];
    let mut values =
        std::collections::BTreeMap::<String, (std::ffi::OsString, std::ffi::OsString)>::new();
    for (key, value) in std::env::vars_os() {
        let folded = key.to_string_lossy().to_ascii_uppercase();
        if ALLOWED.contains(&folded.as_str()) {
            values.insert(folded, (key, value));
        }
    }
    for (key, value) in overrides {
        let folded = key.to_string_lossy().to_ascii_uppercase();
        anyhow::ensure!(
            !folded.is_empty()
                && !folded.contains('=')
                && !folded.contains('\0')
                && !value.to_string_lossy().contains('\0'),
            "invalid sandbox environment variable"
        );
        values.insert(folded, (key.clone(), value.clone()));
    }
    let mut block = Vec::new();
    for (_, (key, value)) in values {
        block.extend(key.encode_wide());
        block.push(b'=' as u16);
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    Ok(block)
}

#[cfg(test)]
mod runner_tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::atomic::AtomicBool;

    fn cmd_exe() -> PathBuf {
        PathBuf::from(std::env::var_os("WINDIR").expect("WINDIR")).join("System32\\cmd.exe")
    }

    #[test]
    fn capture_reports_exact_limit_overflow_and_zero_limit_without_losing_eof() {
        use std::io::Write;
        for (input, limit) in [
            (b"abcd".as_slice(), 4),
            (b"abcde".as_slice(), 4),
            ("中文".as_bytes(), 4),
            (b"abc".as_slice(), 0),
        ] {
            let mut file = tempfile::tempfile().expect("create capture fixture");
            file.write_all(input).expect("write capture fixture");
            use std::io::{Seek, SeekFrom};
            file.seek(SeekFrom::Start(0)).expect("rewind fixture");
            let result = spawn_capture(file, limit).join().unwrap().unwrap();
            assert_eq!(result.total_bytes, input.len() as u64);
            assert_eq!(result.bytes, input[..input.len().min(limit)]);
        }
    }

    #[test]
    fn production_runner_reports_both_truncated_streams_after_cleanup() {
        let temp = tempfile::tempdir().expect("create truncation fixture");
        let work = temp.path().join("work");
        std::fs::create_dir_all(&work).expect("create truncation workspace");
        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(
                    "for /L %i in (1,1,100) do @(echo stdout-line & echo stderr-line 1>&2)",
                ),
            ],
            &work,
            &work,
            Duration::from_secs(10),
            Arc::new(AtomicBool::new(false)),
            &[],
            64,
        )
        .expect("run two-stream truncation fixture");
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.len(), 64);
        assert_eq!(result.stderr.len(), 64);
        assert!(result.stdout_total_bytes > 64);
        assert!(result.stderr_total_bytes > 64);
        assert!(!result.cancelled && !result.timed_out);
    }

    #[test]
    fn production_runner_captures_output_and_denies_sibling_reads() {
        let temp = tempfile::tempdir().expect("create AppContainer test root");
        let work = temp.path().join("workspace");
        let nested = work.join("nested");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&nested).expect("create scoped workspace");
        std::fs::create_dir_all(&outside).expect("create sibling directory");
        let workspace_file = work.join("workspace-source.txt");
        std::fs::write(&workspace_file, "workspace-visible").expect("write workspace canary");
        let secret = outside.join("private.txt");
        std::fs::write(&secret, "must stay outside").expect("write outside canary");

        let command = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("echo scoped-output"),
            ],
            &work,
            &nested,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("run bounded AppContainer command");
        assert_eq!(command.exit_code, 0);
        assert!(String::from_utf8_lossy(&command.stdout).contains("scoped-output"));
        assert!(!command.timed_out && !command.cancelled);

        let workspace_read = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("type ..\\workspace-source.txt"),
            ],
            &work,
            &nested,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("read workspace file from nested cwd");
        assert_eq!(
            workspace_read.exit_code,
            0,
            "stdout={} stderr={}",
            String::from_utf8_lossy(&workspace_read.stdout),
            String::from_utf8_lossy(&workspace_read.stderr)
        );
        assert!(String::from_utf8_lossy(&workspace_read.stdout).contains("workspace-visible"));

        let read = format!("type \"{}\"", secret.display());
        let denied = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(read),
            ],
            &work,
            &nested,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("run out-of-scope read probe");
        assert_ne!(
            denied.exit_code, 0,
            "AppContainer read an out-of-scope sibling"
        );

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind local-only probe");
        listener
            .set_nonblocking(true)
            .expect("set listener nonblocking");
        let port = listener.local_addr().expect("read listener address").port();
        let acceptor = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(3);
            loop {
                match listener.accept() {
                    Ok(_) => return true,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        if Instant::now() >= deadline {
                            return false;
                        }
                        thread::sleep(Duration::from_millis(20));
                    }
                    Err(_) => return false,
                }
            }
        });
        let curl =
            PathBuf::from(std::env::var_os("WINDIR").expect("WINDIR")).join("System32\\curl.exe");
        let network = run_appcontainer_command(
            curl.as_os_str(),
            &[
                OsString::from("--silent"),
                OsString::from("--max-time"),
                OsString::from("2"),
                OsString::from("--connect-timeout"),
                OsString::from("1"),
                OsString::from(format!("http://127.0.0.1:{port}/")),
            ],
            &work,
            &work,
            Duration::from_secs(4),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("run local-only network denial probe");
        assert_ne!(
            network.exit_code, 0,
            "AppContainer curl unexpectedly connected"
        );
        assert!(!acceptor.join().expect("join local listener"));
    }

    #[test]
    fn production_runner_rejects_hard_linked_workspace_files() {
        let temp = tempfile::tempdir().expect("create AppContainer hard-link test root");
        let work = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&work).expect("create scoped workspace");
        std::fs::create_dir_all(&outside).expect("create outside directory");
        let external_file = outside.join("private.txt");
        std::fs::write(&external_file, "outside hard-link canary").expect("write canary");
        std::fs::hard_link(&external_file, work.join("linked-private.txt"))
            .expect("create workspace hard link to outside file");

        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("echo must-not-start"),
            ],
            &work,
            &work,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        );
        let error = result.expect_err("hard-linked workspace must be rejected");
        assert!(
            error.to_string().contains("multiple hard links"),
            "{error:#}"
        );
    }

    struct RestoreDaclOnDrop {
        path: PathBuf,
        original: Vec<u8>,
        protected: bool,
    }

    struct RestoreHandleDaclOnDrop {
        path: PathBuf,
        handle: OwnedHandle,
        original: Vec<u8>,
        protected: bool,
    }

    impl Drop for RestoreHandleDaclOnDrop {
        fn drop(&mut self) {
            let _ = set_dacl_handle(&self.path, &self.handle, &self.original, self.protected);
        }
    }

    impl Drop for RestoreDaclOnDrop {
        fn drop(&mut self) {
            let _ = set_dacl(&self.path, &self.original, self.protected);
        }
    }

    fn dacl_bytes(path: &Path) -> Result<Vec<u8>> {
        let (acl, descriptor) = get_dacl(path)?;
        let bytes = acl.map_or_else(Vec::new, |acl| unsafe {
            std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec()
        });
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        Ok(bytes)
    }

    fn dacl_protected(path: &Path) -> Result<bool> {
        let (_, descriptor) = get_dacl(path)?;
        let protected = dacl_is_protected(descriptor);
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        protected
    }

    fn dacl_bytes_handle(path: &Path, handle: &OwnedHandle) -> Result<Vec<u8>> {
        let (acl, descriptor) = get_dacl_handle(path, handle)?;
        let bytes = acl.map_or_else(Vec::new, |acl| unsafe {
            std::slice::from_raw_parts(acl.cast::<u8>(), (*acl).AclSize as usize).to_vec()
        });
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        Ok(bytes)
    }

    fn dacl_protected_handle(path: &Path, handle: &OwnedHandle) -> Result<bool> {
        let (_, descriptor) = get_dacl_handle(path, handle)?;
        let protected = dacl_is_protected(descriptor);
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        protected
    }

    fn dacl_contains_sid(path: &Path, sid: windows::Win32::Security::PSID) -> Result<bool> {
        let acl = dacl_bytes(path)?;
        let sid_ptr = sid.0.cast::<u8>();
        let sid_length = unsafe { 8 + 4 * *sid_ptr.add(1) as usize };
        let sid_bytes = unsafe { std::slice::from_raw_parts(sid_ptr, sid_length) };
        Ok(acl.windows(sid_length).any(|window| window == sid_bytes))
    }

    fn set_null_dacl(path: &Path) -> Result<()> {
        let path = wide(path.as_os_str());
        let result = unsafe {
            SetNamedSecurityInfoW(
                PCWSTR(path.as_ptr()),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(std::ptr::null()),
                None,
            )
        };
        anyhow::ensure!(result.0 == 0, "set null DACL failed: {result:?}");
        Ok(())
    }

    #[test]
    fn appcontainer_cannot_move_workspace_file_to_sibling() {
        let temp = tempfile::tempdir().expect("create move-boundary test root");
        let work = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&work).expect("create workspace");
        std::fs::create_dir_all(&outside).expect("create sibling directory");
        let source = work.join("inside.txt");
        let destination = outside.join("escaped.txt");
        std::fs::write(&source, "workspace only").expect("create workspace file");
        let command = format!(
            "move \"{}\" \"{}\" >nul",
            source.display(),
            destination.display()
        );

        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(command),
            ],
            &work,
            &work,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("run workspace-to-sibling move attempt");

        assert_ne!(result.exit_code, 0, "out-of-workspace move must be denied");
        assert!(
            source.exists(),
            "denied move must leave the source in place"
        );
        assert!(
            !destination.exists(),
            "denied move must not create an outside file"
        );
    }

    #[test]
    fn host_move_of_new_workspace_object_drops_inherited_sid() {
        let temp = tempfile::tempdir().expect("create host-move cleanup test root");
        let work = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&work).expect("create workspace");
        std::fs::create_dir_all(&outside).expect("create sibling directory");
        let profile = AppContainerProfile::create().expect("create unique AppContainer profile");
        let mut lease = unsafe { WorkspaceAclLease::grant(&work, profile.sid) }
            .expect("grant temporary workspace ACL");

        let created_during_lease = work.join("created-during-lease.txt");
        std::fs::write(&created_during_lease, "created while SID is inherited")
            .expect("create inherited workspace object as host");
        let retained_during_lease = work.join("retained-during-lease.txt");
        std::fs::write(&retained_during_lease, "retained inside workspace")
            .expect("create retained workspace object as host");
        let nested_during_lease = work.join("nested-during-lease");
        std::fs::create_dir(&nested_during_lease).expect("create inherited workspace directory");
        let nested_file = nested_during_lease.join("child.txt");
        std::fs::write(&nested_file, "nested inherited object")
            .expect("create inherited nested workspace object");
        assert!(
            dacl_contains_sid(&created_during_lease, profile.sid)
                .expect("check inherited package SID"),
            "new workspace files must inherit the temporary AppContainer ACE"
        );
        assert!(
            dacl_contains_sid(&retained_during_lease, profile.sid)
                .expect("check retained file's inherited package SID"),
            "retained workspace files must inherit the temporary AppContainer ACE"
        );
        assert!(
            dacl_contains_sid(&nested_file, profile.sid)
                .expect("check nested file's inherited package SID"),
            "nested workspace files must inherit the temporary AppContainer ACE"
        );

        let moved_outside = outside.join("moved-during-lease.txt");
        std::fs::rename(&created_during_lease, &moved_outside)
            .expect("simulate host moving a newly-created object outside the workspace");
        let moved_directory = outside.join("moved-directory-during-lease");
        std::fs::rename(&nested_during_lease, &moved_directory)
            .expect("simulate host moving a new directory tree outside the workspace");
        assert!(
            dacl_contains_sid(&moved_outside, profile.sid)
                .expect("inspect moved object's DACL after rename"),
            "same-volume rename retains the inherited temporary SID until the original parent ACL is restored"
        );
        assert!(dacl_contains_sid(&moved_directory, profile.sid)
            .expect("inspect moved directory's DACL after rename"));
        assert!(
            dacl_contains_sid(&moved_directory.join("child.txt"), profile.sid)
                .expect("inspect moved descendant's DACL after rename")
        );
        lease.finish().expect("finish workspace ACL lease");

        assert!(
            !dacl_contains_sid(&moved_outside, profile.sid)
                .expect("inspect moved object's DACL after cleanup"),
            "an object moved out by the host must not retain the temporary SID after lease cleanup"
        );
        assert!(
            !dacl_contains_sid(&retained_during_lease, profile.sid)
                .expect("inspect retained object's DACL after cleanup"),
            "an object left in the workspace must not retain the temporary SID after lease cleanup"
        );
        assert!(
            !dacl_contains_sid(&moved_directory.join("child.txt"), profile.sid)
                .expect("inspect moved descendant's DACL after cleanup"),
            "a descendant moved outside with its new directory must not retain the temporary SID after lease cleanup"
        );
        assert!(!dacl_contains_sid(&moved_directory, profile.sid)
            .expect("inspect moved directory's DACL after cleanup"));
    }

    #[test]
    fn host_addition_after_cleanup_scan_does_not_retain_temporary_sid() {
        let temp = tempfile::tempdir().expect("create cleanup race test root");
        let work = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&work).expect("create workspace");
        std::fs::create_dir_all(&outside).expect("create sibling directory");
        let profile = AppContainerProfile::create().expect("create unique AppContainer profile");
        let mut lease = unsafe { WorkspaceAclLease::grant(&work, profile.sid) }
            .expect("grant temporary workspace ACL");

        let moved_after_scan = outside.join("created-after-scan.txt");
        lease
            .finish_with_after_scan(|| {
                let created = work.join("created-after-scan.txt");
                std::fs::write(&created, "created after cleanup enumeration")
                    .expect("create workspace object after cleanup scan");
                assert!(dacl_contains_sid(&created, profile.sid).expect("check inherited SID"));
                std::fs::rename(&created, &moved_after_scan)
                    .expect("move late-created object outside workspace");
                assert!(dacl_contains_sid(&moved_after_scan, profile.sid)
                    .expect("check SID retained after late move"));
            })
            .expect("finish lease after injected late addition");

        assert!(
            !dacl_contains_sid(&moved_after_scan, profile.sid)
                .expect("inspect late-moved object's DACL after cleanup"),
            "restoring the original parent ACL must not leave the SID on a same-volume object added after enumeration"
        );
    }

    #[test]
    fn production_runner_restores_prior_acl_changes_when_grant_fails_closed() {
        let temp = tempfile::tempdir().expect("create AppContainer ACL rollback test root");
        let work = temp.path().join("work");
        std::fs::create_dir_all(&work).expect("create scoped workspace");
        let file = work.join("null-dacl.txt");
        std::fs::write(&file, "no sensitive content").expect("write null DACL test file");
        let parent_before = dacl_bytes(temp.path()).expect("snapshot parent ACL");
        let root_before = dacl_bytes(&work).expect("snapshot workspace ACL");
        let file_before = dacl_bytes(&file).expect("snapshot file ACL");
        let file_protected = dacl_protected(&file).expect("snapshot file DACL inheritance");
        let _restore = RestoreDaclOnDrop {
            path: file.clone(),
            original: file_before.clone(),
            protected: file_protected,
        };
        set_null_dacl(&file).expect("install null DACL fault");

        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("echo must-not-start"),
            ],
            &work,
            &work,
            Duration::from_secs(5),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        );
        let error = result.expect_err("null DACL must reject AppContainer ACL lease");
        assert!(error.to_string().contains("null DACL"), "{error:#}");
        assert_eq!(
            dacl_bytes(temp.path()).expect("read parent DACL"),
            parent_before
        );
        assert_eq!(dacl_bytes(&work).expect("read workspace DACL"), root_before);
        assert!(dacl_bytes(&file).expect("read null DACL").is_empty());
        set_dacl(&file, &file_before, file_protected).expect("restore original file DACL");
        assert_eq!(
            dacl_bytes(&file).expect("verify original file DACL"),
            file_before
        );
        assert_eq!(
            dacl_protected(&file).expect("verify file DACL inheritance"),
            file_protected
        );
    }

    #[test]
    fn acl_cleanup_set_security_info_failure_is_reported_and_can_be_retried() {
        let temp = tempfile::tempdir().expect("create ACL cleanup failure test root");
        let work = temp.path().join("work");
        std::fs::create_dir_all(&work).expect("create empty workspace");
        let file = work.join("child.txt");
        std::fs::write(&file, "existing child").expect("create cleanup test child");
        let root_handle = open_directory_for_enumeration(&work);
        let file_handle = open_file_read_only_acl(&file);
        let root_acl = dacl_bytes_handle(&work, &root_handle).expect("snapshot workspace DACL");
        let root_protected =
            dacl_protected_handle(&work, &root_handle).expect("snapshot workspace DACL protection");
        let file_acl = dacl_bytes_handle(&file, &file_handle).expect("snapshot child DACL");
        let file_protected =
            dacl_protected_handle(&file, &file_handle).expect("snapshot child DACL protection");
        let mut lease = WorkspaceAclLease {
            root: work.clone(),
            _ancestor_handles: Vec::new(),
            sid: windows::Win32::Security::PSID(std::ptr::null_mut()),
            originals: vec![
                OriginalDacl {
                    path: work.clone(),
                    handle: root_handle,
                    acl: root_acl,
                    protected: root_protected,
                },
                OriginalDacl {
                    path: file.clone(),
                    handle: file_handle,
                    acl: file_acl,
                    protected: file_protected,
                },
            ],
            original_paths: HashSet::from([work.clone(), file.clone()]),
            active: true,
        };

        let error = lease
            .finish()
            .expect_err("failed DACL restore must be returned to the caller");
        assert!(
            format!("{error:#}").contains("SetSecurityInfo failed"),
            "unexpected cleanup error: {error:#}"
        );
        assert!(lease.active, "failed cleanup must remain retryable");

        lease.originals[1].handle = open_acl_handle(&file).expect("reopen child with WRITE_DAC");
        lease.finish().expect("retry real DACL restoration");
        assert!(!lease.active, "successful retry should release the lease");
    }

    #[test]
    fn acl_mutation_stays_bound_to_open_handle_after_path_replacement() {
        let temp = tempfile::tempdir().expect("create handle-binding test root");
        let work = temp.path().join("work");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&work).expect("create workspace");
        std::fs::create_dir_all(&outside).expect("create sibling directory");
        let target = work.join("target.txt");
        let moved = outside.join("moved.txt");
        let substitute = outside.join("substitute.txt");
        std::fs::write(&target, "checked workspace object").expect("write target");
        std::fs::write(&substitute, "replacement object").expect("write substitute");

        let handle = open_acl_handle(&target).expect("open target by no-follow handle");
        let original_acl = dacl_bytes_handle(&target, &handle).expect("snapshot target DACL");
        let original_protected =
            dacl_protected_handle(&target, &handle).expect("snapshot target inheritance");
        let substitute_acl = dacl_bytes(&substitute).expect("snapshot substitute DACL");
        let substitute_protected =
            dacl_protected(&substitute).expect("snapshot substitute inheritance");
        let restore = RestoreHandleDaclOnDrop {
            path: target.clone(),
            handle,
            original: original_acl.clone(),
            protected: original_protected,
        };

        std::fs::rename(&target, &moved).expect("move opened target out of workspace");
        std::fs::rename(&substitute, &target).expect("replace original path");
        set_dacl_handle(&target, &restore.handle, &original_acl, !original_protected)
            .expect("change DACL state through retained handle");

        assert_eq!(
            dacl_protected_handle(&target, &restore.handle)
                .expect("read moved target through handle"),
            !original_protected
        );
        assert_eq!(
            dacl_bytes(&target).expect("read replacement object DACL"),
            substitute_acl,
            "path replacement must not redirect a handle-based ACL update"
        );
        assert_eq!(
            dacl_protected(&target).expect("read replacement inheritance"),
            substitute_protected
        );
        set_dacl_handle(&target, &restore.handle, &original_acl, original_protected)
            .expect("restore moved target DACL through retained handle");
        assert_eq!(
            dacl_bytes(&moved).expect("verify restored moved target DACL"),
            original_acl
        );
    }

    #[test]
    fn relative_child_open_ignores_workspace_ancestor_path_replacement() {
        let temp = tempfile::tempdir().expect("create relative-open test root");
        let work = temp.path().join("work");
        let moved = temp.path().join("moved-work");
        std::fs::create_dir_all(&work).expect("create workspace");
        std::fs::write(work.join("inside.txt"), "original").expect("write original child");
        std::fs::write(work.join("only-original.txt"), "original only")
            .expect("write original-only child");

        let directory = open_directory_for_enumeration(&work);
        let before = directory_entry_names(directory.raw()).expect("enumerate original workspace");
        assert!(before.iter().any(|(name, _)| name == "inside.txt"));
        assert!(before.iter().any(|(name, _)| name == "only-original.txt"));
        let first_child = open_relative_acl_handle(
            directory.raw(),
            std::ffi::OsStr::new("inside.txt"),
            &work.join("inside.txt"),
            false,
            true,
        )
        .expect("open original child relative to retained directory");
        let original_info = file_information(&first_child).expect("query original child identity");
        drop(first_child);

        std::fs::rename(&work, &moved).expect("move workspace directory");
        std::fs::create_dir_all(&work).expect("replace workspace path with sibling directory");
        std::fs::write(work.join("inside.txt"), "replacement").expect("write replacement child");
        std::fs::write(work.join("only-replacement.txt"), "replacement only")
            .expect("write replacement-only child");

        let after = directory_entry_names(directory.raw())
            .expect("enumerate original workspace after ancestor path replacement");
        assert!(after.iter().any(|(name, _)| name == "only-original.txt"));
        assert!(!after.iter().any(|(name, _)| name == "only-replacement.txt"));

        let relative_child = open_relative_acl_handle(
            directory.raw(),
            std::ffi::OsStr::new("inside.txt"),
            &work.join("inside.txt"),
            false,
            true,
        )
        .expect("resolve child relative to original directory handle");
        let relative_info =
            file_information(&relative_child).expect("query relative child identity");
        let replacement =
            open_acl_handle(&work.join("inside.txt")).expect("open replacement by path");
        let replacement_info = file_information(&replacement).expect("query replacement identity");

        assert_eq!(
            relative_info.dwVolumeSerialNumber,
            original_info.dwVolumeSerialNumber
        );
        assert_eq!(relative_info.nFileIndexHigh, original_info.nFileIndexHigh);
        assert_eq!(relative_info.nFileIndexLow, original_info.nFileIndexLow);
        assert_ne!(relative_info.nFileIndexLow, replacement_info.nFileIndexLow);
    }

    #[test]
    fn workspace_root_open_is_anchored_from_volume_root() {
        let temp = tempfile::tempdir().expect("create volume-walk test root");
        let ancestor = temp.path().join("selected-ancestor");
        let parent = ancestor.join("selected-parent");
        let moved_parent = temp.path().join("moved-parent");
        let moved_ancestor = temp.path().join("moved-ancestor");
        let workspace = parent.join("work");
        std::fs::create_dir_all(&workspace).expect("create selected workspace");
        std::fs::write(workspace.join("only-original.txt"), "original")
            .expect("write original workspace entry");

        let canonical = workspace.canonicalize().expect("canonicalize workspace");
        let (opened_root, _ancestor_handles, parent_handle) =
            open_workspace_parent_chain(&canonical).expect("open canonical ancestor chain");
        assert_eq!(
            opened_root,
            process_working_directory(&canonical).expect("normalize local workspace path")
        );
        let original_root = open_relative_acl_handle(
            parent_handle.raw(),
            std::ffi::OsStr::new("work"),
            &workspace,
            true,
            true,
        )
        .expect("open workspace relative to retained parent handle");
        let original_info = file_information(&original_root).expect("query original workspace");

        let blocked_ancestor_move = std::fs::rename(&parent, &moved_parent);
        assert!(
            blocked_ancestor_move.is_err(),
            "an open root handle without delete sharing must block moving its parent"
        );
        let blocked_higher_ancestor_move = std::fs::rename(&ancestor, &moved_ancestor);
        assert!(
            blocked_higher_ancestor_move.is_err(),
            "an open root handle without delete sharing must block moving a higher ancestor"
        );
        drop(original_root);

        let still_blocked_higher_ancestor_move = std::fs::rename(&ancestor, &moved_ancestor);
        assert!(
            still_blocked_higher_ancestor_move.is_err(),
            "the retained ancestor chain must remain pinned for the lease lifetime"
        );

        let reopened_root = open_relative_acl_handle(
            parent_handle.raw(),
            std::ffi::OsStr::new("work"),
            &workspace,
            true,
            true,
        )
        .expect("reopen workspace under original retained parent");
        let reopened_info = file_information(&reopened_root).expect("query reopened workspace");
        let names = directory_entry_names(reopened_root.raw()).expect("enumerate original root");
        assert!(names.iter().any(|(name, _)| name == "only-original.txt"));
        assert_eq!(
            reopened_info.dwVolumeSerialNumber,
            original_info.dwVolumeSerialNumber
        );
        assert_eq!(reopened_info.nFileIndexHigh, original_info.nFileIndexHigh);
        assert_eq!(reopened_info.nFileIndexLow, original_info.nFileIndexLow);
    }

    #[test]
    fn workspace_root_cannot_be_renamed_while_lease_handle_is_open() {
        let temp = tempfile::tempdir().expect("create workspace rename test root");
        let work = temp.path().join("work");
        let moved = temp.path().join("moved-work");
        std::fs::create_dir_all(&work).expect("create workspace");
        let parent = open_acl_handle(temp.path()).expect("open workspace parent");
        let _root = open_workspace_root_handle(parent.raw(), std::ffi::OsStr::new("work"), &work)
            .expect("open root handle without delete sharing");

        let result = std::fs::rename(&work, &moved);
        assert!(result.is_err(), "leased workspace root must resist rename");
        assert!(work.is_dir());
        assert!(!moved.exists());
    }

    fn open_directory_for_enumeration(path: &Path) -> OwnedHandle {
        let path_wide = wide(path.as_os_str());
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                READ_CONTROL.0 | WRITE_DAC.0 | FILE_READ_ATTRIBUTES.0 | FILE_LIST_DIRECTORY.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .expect("open directory for handle-based enumeration");
        OwnedHandle::new(handle)
    }

    fn open_file_read_only_acl(path: &Path) -> OwnedHandle {
        let path_wide = wide(path.as_os_str());
        let handle = unsafe {
            CreateFileW(
                PCWSTR(path_wide.as_ptr()),
                READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0,
                FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                None,
                OPEN_EXISTING,
                FILE_FLAG_OPEN_REPARSE_POINT,
                None,
            )
        }
        .expect("open file without WRITE_DAC for failure injection");
        OwnedHandle::new(handle)
    }

    #[test]
    fn production_runner_cancels_and_reaps_the_job() {
        let temp = tempfile::tempdir().expect("create AppContainer test root");
        let work = temp.path().join("work");
        std::fs::create_dir_all(&work).expect("create scoped workspace");
        let cancel = Arc::new(AtomicBool::new(false));
        let signal = cancel.clone();
        let canceller = thread::spawn(move || {
            thread::sleep(Duration::from_millis(150));
            signal.store(true, Ordering::SeqCst);
        });
        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("for /L %i in (1,1,1000000) do @echo x>nul"),
            ],
            &work,
            &work,
            Duration::from_secs(8),
            cancel,
            &[],
            1024,
        )
        .expect("cancel AppContainer child process");
        canceller.join().expect("join cancel signal");
        assert!(result.cancelled);
        assert!(!result.timed_out);
        assert_eq!(result.exit_code, 130);
    }

    #[test]
    fn production_runner_times_out_and_reaps_the_job() {
        let temp = tempfile::tempdir().expect("create AppContainer test root");
        let work = temp.path().join("work");
        std::fs::create_dir_all(&work).expect("create scoped workspace");
        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from("for /L %i in (1,1,1000000) do @echo x>nul"),
            ],
            &work,
            &work,
            Duration::from_millis(150),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("time out AppContainer child process");
        assert!(result.timed_out);
        assert!(!result.cancelled);
        assert_eq!(result.exit_code, 124);
    }

    #[test]
    fn production_runner_supports_cloud_synced_workspace_and_write_dac_denial() {
        // 1. Verify benign cloud reparse tags vs redirection tags
        assert!(is_benign_cloud_reparse_tag(0x9000_001A)); // IO_REPARSE_TAG_CLOUD
        assert!(is_benign_cloud_reparse_tag(0x9000_F01A)); // IO_REPARSE_TAG_CLOUD_F
        assert!(is_benign_cloud_reparse_tag(0x9000_001C)); // IO_REPARSE_TAG_PROJFS
        assert!(!is_benign_cloud_reparse_tag(0xA000_000C)); // IO_REPARSE_TAG_SYMLINK
        assert!(!is_benign_cloud_reparse_tag(0xA000_0003)); // IO_REPARSE_TAG_MOUNT_POINT

        // 2. Simulate cloud sync daemon holding a DELETE-access handle on the workspace root
        let temp = tempfile::tempdir().expect("create cloud-sync test root");
        let work = temp.path().join("DimLab");
        std::fs::create_dir_all(work.join("node_modules\\exceljs"))
            .expect("create DimLab node_modules\\exceljs");
        let work_wide = wide(work.as_os_str());
        let cloud_daemon_handle = OwnedHandle::new(
            unsafe {
                CreateFileW(
                    PCWSTR(work_wide.as_ptr()),
                    windows::Win32::Storage::FileSystem::DELETE.0 | FILE_LIST_DIRECTORY.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    None,
                )
            }
            .expect("open workspace with DELETE access like cloud sync daemon"),
        );

        let result = run_appcontainer_command(
            cmd_exe().as_os_str(),
            &[
                OsString::from("/d"),
                OsString::from("/c"),
                OsString::from(
                    "if exist node_modules\\exceljs (echo exceljs OK) else (echo exceljs MISSING)",
                ),
            ],
            &work,
            &work,
            Duration::from_secs(8),
            Arc::new(AtomicBool::new(false)),
            &[],
            1024,
        )
        .expect("run command while cloud daemon holds DELETE handle on workspace root");
        assert_eq!(result.exit_code, 0);
        assert!(
            String::from_utf8_lossy(&result.stdout).contains("exceljs OK"),
            "stdout={}",
            String::from_utf8_lossy(&result.stdout)
        );

        // 3. Simulate cloud-mapped directory denying WRITE_DAC (NTSTATUS 0xC0000022)
        let pre_handle = OwnedHandle::new(
            unsafe {
                CreateFileW(
                    PCWSTR(work_wide.as_ptr()),
                    READ_CONTROL.0 | WRITE_DAC.0 | FILE_READ_ATTRIBUTES.0,
                    FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
                    None,
                    OPEN_EXISTING,
                    FILE_FLAG_BACKUP_SEMANTICS,
                    None,
                )
            }
            .expect("open pre-handle with WRITE_DAC before applying deny ACE"),
        );
        let (orig_acl_ptr, orig_descriptor) =
            get_dacl_handle(&work, &pre_handle).expect("read original DACL");
        let orig_acl_ptr = orig_acl_ptr.expect("workspace has non-null DACL");
        let orig_protected =
            dacl_is_protected(orig_descriptor).expect("read original DACL protection");
        let orig_bytes = unsafe {
            std::slice::from_raw_parts(
                orig_acl_ptr.cast::<u8>(),
                (*orig_acl_ptr).AclSize as usize,
            )
            .to_vec()
        };

        let mut world_sid = windows::Win32::Security::PSID::default();
        let mut owner_rights_sid = windows::Win32::Security::PSID::default();
        unsafe {
            windows::Win32::Security::Authorization::ConvertStringSidToSidW(
                windows::core::w!("S-1-1-0"),
                &mut world_sid,
            )
            .expect("allocate World SID");
            windows::Win32::Security::Authorization::ConvertStringSidToSidW(
                windows::core::w!("S-1-3-4"),
                &mut owner_rights_sid,
            )
            .expect("allocate Owner Rights SID");
        }
        let owner_rights_ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0,
            grfAccessMode: windows::Win32::Security::Authorization::SET_ACCESS,
            grfInheritance: ACE_FLAGS(0),
            Trustee: TRUSTEE_W {
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: PWSTR(owner_rights_sid.0.cast()),
                ..Default::default()
            },
        };
        let deny_ea = EXPLICIT_ACCESS_W {
            grfAccessPermissions: WRITE_DAC.0,
            grfAccessMode: windows::Win32::Security::Authorization::DENY_ACCESS,
            grfInheritance: ACE_FLAGS(0),
            Trustee: TRUSTEE_W {
                TrusteeForm: TRUSTEE_IS_SID,
                TrusteeType: TRUSTEE_IS_USER,
                ptstrName: PWSTR(world_sid.0.cast()),
                ..Default::default()
            },
        };
        let mut deny_dacl: *mut ACL = std::ptr::null_mut();
        let entries = [owner_rights_ea, deny_ea];
        let set_err =
            unsafe { SetEntriesInAclW(Some(&entries), Some(orig_acl_ptr), &mut deny_dacl) };
        assert_eq!(set_err.0, 0);
        let apply_err = unsafe {
            SetSecurityInfo(
                pre_handle.raw(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                None,
                None,
                Some(deny_dacl),
                None,
            )
        };
        assert_eq!(apply_err.0, 0);
        unsafe {
            let _ = LocalFree(HLOCAL(orig_descriptor.0));
        }

        // Verify that opening with WRITE_DAC now fails with 0xC0000022, matching the Nutstore DimLab error
        let parent_dir = open_directory_anchor_handle(temp.path()).expect("open temp parent dir");
        let rel_err = match open_relative_acl_handle(
            parent_dir.raw(),
            work.file_name().expect("DimLab file_name"),
            &work,
            true,
            true,
        ) {
            Ok(_) => panic!("WRITE_DAC open must fail with 0xC0000022"),
            Err(err) => err,
        };
        assert!(
            rel_err.to_string().contains("0xC0000022"),
            "unexpected error: {rel_err}"
        );

        // Verify that run_appcontainer_command succeeds via readonly-DACL workspace pinning + Job sandbox
        let powershell_exe = PathBuf::from(
            std::env::var_os("WINDIR").unwrap_or_else(|| OsString::from(r"C:\Windows")),
        )
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe");
        let ps_result = run_appcontainer_command(
            powershell_exe.as_os_str(),
            &[
                OsString::from("-NoProfile"),
                OsString::from("-NonInteractive"),
                OsString::from("-Command"),
                OsString::from(
                    "node -v; if (Test-Path node_modules\\exceljs) { echo \"exceljs OK\" } else { echo \"exceljs MISSING\" }",
                ),
            ],
            &work,
            &work,
            Duration::from_secs(15),
            Arc::new(AtomicBool::new(false)),
            &[],
            4096,
        );

        // Always restore original DACL before asserting so TempDir cleans up
        unsafe {
            let _ = LocalFree(HLOCAL(deny_dacl.cast()));
            let _ = LocalFree(HLOCAL(world_sid.0));
            let _ = LocalFree(HLOCAL(owner_rights_sid.0));
        }
        set_dacl_handle(&work, &pre_handle, &orig_bytes, orig_protected)
            .expect("restore original DACL via pre-opened handle");
        drop(pre_handle);
        drop(cloud_daemon_handle);

        let ps_result = ps_result.expect("run command on cloud-mapped workspace denying WRITE_DAC");
        assert_eq!(
            ps_result.exit_code,
            0,
            "stderr={}",
            String::from_utf8_lossy(&ps_result.stderr)
        );
        let stdout = String::from_utf8_lossy(&ps_result.stdout);
        assert!(stdout.contains("exceljs OK"), "stdout={stdout}");
    }

    #[test]
    fn windows_sandbox_large_node_modules_nutstore_workspace_does_not_overflow_or_rewrite_dacls() {
        let temp = tempfile::tempdir().expect("tempdir");
        let work = temp.path().join("DimLab");
        let nm = work.join("node_modules");
        std::fs::create_dir_all(&nm).expect("create node_modules");

        // Populate > MAX_APPCONTAINER_ACL_ENTRIES files and a deep directory chain plus a hardlink
        // inside node_modules (matching real npm/pnpm node_modules inside a Nutstore directory).
        for i in 0..300 {
            let pkg_dir = nm.join(format!("pkg-{i:03}"));
            std::fs::create_dir_all(&pkg_dir).expect("create pkg dir");
            std::fs::write(pkg_dir.join("index.js"), b"module.exports = 1;\n")
                .expect("write index.js");
        }
        let link_src = nm.join("pkg-000").join("index.js");
        let link_dst = nm.join("pkg-000").join("index.hardlink.js");
        let _ = std::fs::hard_link(&link_src, &link_dst);

        let work_for_thread = work.clone();
        let handle = std::thread::Builder::new()
            .name("nutstore-small-stack-test".to_string())
            .stack_size(512 * 1024)
            .spawn(move || {
                // 1) Exact command from user bug report: `node -v && echo OK` via cmd.exe
                let node_res = run_appcontainer_command(
                    cmd_exe().as_os_str(),
                    &[
                        OsString::from("/d"),
                        OsString::from("/c"),
                        OsString::from("chcp 65001 >nul 2>&1 & node -v && echo OK"),
                    ],
                    &work_for_thread,
                    &work_for_thread,
                    Duration::from_secs(15),
                    Arc::new(AtomicBool::new(false)),
                    &[],
                    4096,
                )
                .expect("run node -v && echo OK in large workspace");
                assert_eq!(
                    node_res.exit_code,
                    0,
                    "stderr={}",
                    String::from_utf8_lossy(&node_res.stderr)
                );
                let stdout = String::from_utf8_lossy(&node_res.stdout);
                assert!(stdout.contains("OK"), "stdout={stdout}");

                // 2) Non-toolchain command in the same >256-entry workspace with hardlinks
                let echo_res = run_appcontainer_command(
                    cmd_exe().as_os_str(),
                    &[
                        OsString::from("/d"),
                        OsString::from("/c"),
                        OsString::from("echo NON_TOOLCHAIN_OK"),
                    ],
                    &work_for_thread,
                    &work_for_thread,
                    Duration::from_secs(15),
                    Arc::new(AtomicBool::new(false)),
                    &[],
                    4096,
                )
                .expect("run non-toolchain command in large workspace");
                assert_eq!(
                    echo_res.exit_code,
                    0,
                    "stderr={}",
                    String::from_utf8_lossy(&echo_res.stderr)
                );
                let echo_stdout = String::from_utf8_lossy(&echo_res.stdout);
                assert!(echo_stdout.contains("NON_TOOLCHAIN_OK"), "stdout={echo_stdout}");
            })
            .expect("spawn small-stack test thread");
        handle.join().expect("small-stack thread must not overflow or panic");
    }
}

struct OriginalDacl {
    path: PathBuf,
    handle: OwnedHandle,
    acl: Vec<u8>,
    protected: bool,
}

struct PreparedAcl {
    path: PathBuf,
    handle: OwnedHandle,
    acl: Vec<u8>,
    protected: bool,
    permissions: u32,
    inheritance: ACE_FLAGS,
}

/// Temporarily grants one package SID traversal to `root` and full access only
/// inside that tree. Existing DACLs are restored exactly; newly created objects
/// have only this unique package SID revoked. Reparse points and multiply-linked
/// files fail closed to avoid extending access through aliases outside the tree.
pub struct WorkspaceAclLease {
    root: PathBuf,
    _ancestor_handles: Vec<OwnedHandle>,
    sid: windows::Win32::Security::PSID,
    originals: Vec<OriginalDacl>,
    original_paths: HashSet<PathBuf>,
    active: bool,
}

impl WorkspaceAclLease {
    /// Temporarily adds access for one AppContainer package SID to an existing workspace tree.
    ///
    /// # Safety
    /// `sid` must point to a valid AppContainer SID and remain allocated until this lease has
    /// been finished or dropped. The caller must stop and reap every process using the SID before
    /// restoring the ACLs.
    pub unsafe fn grant(root: &Path, sid: windows::Win32::Security::PSID) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("resolve sandbox workspace {}", root.display()))?;
        let (root, ancestor_handles, parent_handle) = open_workspace_parent_chain(&root)?;
        let parent = root
            .parent()
            .context("sandbox workspace has no parent directory")?
            .to_path_buf();
        // Resolve every canonical path component from the volume root, then
        // retain the chain while the workspace ACL lease is active. Only the
        // immediate workspace parent receives temporary traversal permission.
        let parent_entry =
            prepare_acl_handle(&parent, parent_handle, FILE_GENERIC_EXECUTE.0, ACE_FLAGS(0))?;
        let mut prepared = vec![parent_entry];
        let parent_handle = prepared[0].handle.raw();
        collect_prepared_tree(&root, parent_handle, &mut prepared)?;

        let mut lease = Self {
            root,
            _ancestor_handles: ancestor_handles,
            sid,
            originals: Vec::new(),
            original_paths: HashSet::new(),
            active: true,
        };
        for prepared in prepared {
            let path = prepared.path.clone();
            if let Err(error) = grant_prepared(&prepared, sid) {
                let current_restore = set_dacl_handle(
                    &prepared.path,
                    &prepared.handle,
                    &prepared.acl,
                    prepared.protected,
                );
                let rollback = lease.finish();
                return match (current_restore, rollback) {
                    (Ok(()), Ok(())) => Err(error),
                    (Err(current), Ok(())) => Err(error.context(format!(
                        "additionally failed to restore current DACL: {current:#}"
                    ))),
                    (Ok(()), Err(cleanup)) => Err(error.context(format!(
                        "additionally failed to roll back sandbox ACLs: {cleanup:#}"
                    ))),
                    (Err(current), Err(cleanup)) => Err(error.context(format!(
                        "additionally failed to restore current DACL: {current:#}; sandbox ACL rollback: {cleanup:#}"
                    ))),
                };
            }
            lease.original_paths.insert(path.clone());
            lease.originals.push(OriginalDacl {
                path: path.clone(),
                handle: prepared.handle,
                acl: prepared.acl.clone(),
                protected: prepared.protected,
            });
        }
        Ok(lease)
    }

    /// Restores original DACLs and revokes this SID from new workspace objects.
    /// Returns errors so callers can fail the tool result if OS cleanup did not complete.
    pub fn finish(&mut self) -> Result<()> {
        self.finish_inner(|| {})
    }

    #[cfg(test)]
    fn finish_with_after_scan(&mut self, after_scan: impl FnOnce()) -> Result<()> {
        self.finish_inner(after_scan)
    }

    fn finish_inner(&mut self, after_scan: impl FnOnce()) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        let mut errors = Vec::new();
        let mut current = Vec::new();
        let root_handle = self
            .originals
            .iter()
            .find(|original| original.path == self.root)
            .map(|original| &original.handle)
            .context("workspace ACL lease lost its root handle");
        if let Err(error) = root_handle
            .and_then(|handle| collect_current_handles(&self.root, handle.raw(), &mut current))
        {
            errors.push(format!(
                "enumerate sandbox workspace for ACL cleanup: {error:#}"
            ));
        }
        after_scan();
        for (path, handle) in current.into_iter().rev() {
            if !self.original_paths.contains(&path) {
                if let Err(error) = revoke_sid_handle(&path, &handle, self.sid) {
                    errors.push(format!(
                        "revoke temporary SID from {}: {error:#}",
                        path.display()
                    ));
                }
            }
        }
        for original in self.originals.iter().rev() {
            if let Err(error) = set_dacl_handle(
                &original.path,
                &original.handle,
                &original.acl,
                original.protected,
            ) {
                errors.push(format!(
                    "restore DACL for {}: {error:#}",
                    original.path.display()
                ));
            }
        }
        self.active = !errors.is_empty();
        if errors.is_empty() {
            Ok(())
        } else {
            bail!("sandbox ACL cleanup incomplete: {}", errors.join("; "))
        }
    }
}

impl Drop for WorkspaceAclLease {
    fn drop(&mut self) {
        let _ = self.finish();
    }
}

const MAX_APPCONTAINER_ACL_ENTRIES: usize = 256;

fn collect_prepared_tree(
    root: &Path,
    parent: HANDLE,
    prepared: &mut Vec<PreparedAcl>,
) -> Result<()> {
    let inheritance = ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0);
    let component = root
        .file_name()
        .context("workspace root has no final path component")?;
    let handle = open_workspace_root_handle(parent, component, root)?;
    let entry = prepare_acl_handle(root, handle, FILE_ALL_ACCESS.0, inheritance)?;
    collect_prepared_node(entry, prepared)
}

fn collect_prepared_node(root_entry: PreparedAcl, prepared: &mut Vec<PreparedAcl>) -> Result<()> {
    let root_info = file_information(&root_entry.handle)?;
    let root_is_dir = root_info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
    let root_idx = prepared.len();
    prepared.push(root_entry);
    if !root_is_dir {
        return Ok(());
    }

    let mut worklist = vec![root_idx];
    while let Some(dir_idx) = worklist.pop() {
        let dir_path = prepared[dir_idx].path.clone();
        let parent_handle = prepared[dir_idx].handle.raw();
        let mut children = directory_entry_names(parent_handle)?;
        children.sort_by(|left, right| left.0.cmp(&right.0));
        anyhow::ensure!(
            prepared.len().saturating_add(children.len()) <= MAX_APPCONTAINER_ACL_ENTRIES,
            "sandbox workspace exceeds per-file AppContainer ACL lease limit ({MAX_APPCONTAINER_ACL_ENTRIES} entries)"
        );
        let mut child_dir_indices = Vec::new();
        for (component, is_directory) in children {
            let child = dir_path.join(&component);
            match open_relative_acl_handle(parent_handle, &component, &child, is_directory, true) {
                Ok(handle) => {
                    let info = file_information(&handle)?;
                    let child_is_directory =
                        info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
                    let child_inheritance = if child_is_directory {
                        ACE_FLAGS(OBJECT_INHERIT_ACE.0 | CONTAINER_INHERIT_ACE.0)
                    } else {
                        ACE_FLAGS(0)
                    };
                    let entry =
                        prepare_acl_handle(&child, handle, FILE_ALL_ACCESS.0, child_inheritance)?;
                    let idx = prepared.len();
                    prepared.push(entry);
                    if child_is_directory {
                        child_dir_indices.push(idx);
                    }
                }
                Err(error) if is_write_dac_denied_error(&error) => {
                    let readonly = open_relative_acl_handle(
                        parent_handle,
                        &component,
                        &child,
                        is_directory,
                        false,
                    )?;
                    validate_readonly_tree(&child, readonly)?;
                }
                Err(error) => return Err(error),
            }
        }
        for idx in child_dir_indices.into_iter().rev() {
            worklist.push(idx);
        }
    }
    Ok(())
}

fn validate_readonly_tree(root_path: &Path, root_handle: OwnedHandle) -> Result<()> {
    let mut worklist = vec![(root_path.to_path_buf(), root_handle)];
    let mut visited = 0usize;
    while let Some((path, handle)) = worklist.pop() {
        visited += 1;
        anyhow::ensure!(
            visited <= MAX_APPCONTAINER_ACL_ENTRIES,
            "sandbox workspace exceeds per-file AppContainer ACL lease limit ({MAX_APPCONTAINER_ACL_ENTRIES} entries)"
        );
        let info = file_information(&handle)?;
        ensure_not_redirection_reparse_point(
            &handle,
            &info,
            "sandbox workspace contains a reparse point",
            &path,
        )?;
        let is_directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
        if !is_directory {
            anyhow::ensure!(
                info.nNumberOfLinks == 1,
                "sandbox workspace contains a file with multiple hard links: {}",
                path.display()
            );
        }
        let (old_acl, descriptor) = get_dacl_handle(&path, &handle)?;
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        if old_acl.is_none() {
            bail!("refusing sandbox scope on a null DACL: {}", path.display());
        }
        if !is_directory {
            continue;
        }
        let mut children = directory_entry_names(handle.raw())?;
        children.sort_by(|left, right| right.0.cmp(&left.0));
        for (component, child_is_dir) in children {
            let child = path.join(&component);
            let child_handle =
                open_relative_acl_handle(handle.raw(), &component, &child, child_is_dir, false)?;
            worklist.push((child, child_handle));
        }
    }
    Ok(())
}

fn prepare_acl_handle(
    path: &Path,
    handle: OwnedHandle,
    permissions: u32,
    inheritance: ACE_FLAGS,
) -> Result<PreparedAcl> {
    let info = file_information(&handle)?;
    ensure_not_redirection_reparse_point(&handle, &info, "sandbox workspace contains a reparse point", path)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 == 0 {
        anyhow::ensure!(
            info.nNumberOfLinks == 1,
            "sandbox workspace contains a file with multiple hard links: {}",
            path.display()
        );
    }
    let (old_acl, descriptor) = get_dacl_handle(path, &handle)?;
    let Some(old_acl) = old_acl else {
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        bail!("refusing sandbox scope on a null DACL: {}", path.display());
    };
    let protected = dacl_is_protected(descriptor)?;
    let acl = unsafe {
        std::slice::from_raw_parts(old_acl.cast::<u8>(), (*old_acl).AclSize as usize).to_vec()
    };
    unsafe {
        let _ = LocalFree(HLOCAL(descriptor.0));
    }
    Ok(PreparedAcl {
        path: path.to_path_buf(),
        handle,
        acl,
        protected,
        permissions,
        inheritance,
    })
}

fn collect_current_handles(
    root: &Path,
    root_handle: HANDLE,
    objects: &mut Vec<(PathBuf, OwnedHandle)>,
) -> Result<()> {
    let mut worklist = vec![(root.to_path_buf(), root_handle)];
    while let Some((dir_path, dir_handle)) = worklist.pop() {
        let mut children = directory_entry_names(dir_handle)?;
        children.sort_by(|left, right| left.0.cmp(&right.0));
        let mut subdirs = Vec::new();
        for (component, enumerated_directory) in children {
            let path = dir_path.join(&component);
            let handle = match open_relative_acl_handle(
                dir_handle,
                &component,
                &path,
                enumerated_directory,
                true,
            ) {
                Ok(handle) => handle,
                Err(error) if is_write_dac_denied_error(&error) => {
                    let readonly = open_relative_acl_handle(
                        dir_handle,
                        &component,
                        &path,
                        enumerated_directory,
                        false,
                    )?;
                    let info = file_information(&readonly)?;
                    ensure_not_redirection_reparse_point(
                        &readonly,
                        &info,
                        "sandbox workspace contains a reparse point during ACL cleanup",
                        &path,
                    )?;
                    continue;
                }
                Err(error) => return Err(error),
            };
            let info = file_information(&handle)?;
            ensure_not_redirection_reparse_point(
                &handle,
                &info,
                "sandbox workspace contains a reparse point during ACL cleanup",
                &path,
            )?;
            let is_directory = info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0;
            let child_handle = handle.raw();
            let child_path = path.clone();
            objects.push((path, handle));
            if is_directory {
                subdirs.push((child_path, child_handle));
            }
        }
        for subdir in subdirs.into_iter().rev() {
            worklist.push(subdir);
        }
    }
    Ok(())
}

#[cfg(test)]
fn open_acl_handle(path: &Path) -> Result<OwnedHandle> {
    let path_wide = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path_wide.as_ptr()),
            READ_CONTROL.0 | WRITE_DAC.0 | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .with_context(|| format!("open no-follow ACL handle for {}", path.display()))?;
    Ok(OwnedHandle::new(handle))
}

fn open_relative_acl_handle(
    parent: HANDLE,
    component: &std::ffi::OsStr,
    diagnostic_path: &Path,
    enumerate_directory: bool,
    modify_acl: bool,
) -> Result<OwnedHandle> {
    open_relative_acl_handle_with_sharing(
        parent,
        component,
        diagnostic_path,
        enumerate_directory,
        modify_acl,
        true,
    )
}

fn open_workspace_root_handle(
    parent: HANDLE,
    component: &std::ffi::OsStr,
    diagnostic_path: &Path,
) -> Result<OwnedHandle> {
    // Keeping delete sharing off prevents a concurrent rename of the leased
    // workspace root while still allowing files inside it to use atomic replace.
    // If a cloud sync daemon (e.g. Nutstore / OneDrive) already holds a
    // delete-shared handle on the directory, this returns 0xC0000022 / 0xC0000043
    // so the caller cleanly falls back to the read-only pinned Job Object runner
    // without mutating DACLs inside an active cloud-synced folder.
    open_relative_acl_handle_with_sharing(
        parent,
        component,
        diagnostic_path,
        true,
        true,
        false,
    )
}

fn open_relative_acl_handle_with_sharing(
    parent: HANDLE,
    component: &std::ffi::OsStr,
    diagnostic_path: &Path,
    enumerate_directory: bool,
    modify_acl: bool,
    allow_delete_share: bool,
) -> Result<OwnedHandle> {
    const FILE_OPEN_FOR_BACKUP_INTENT: u32 = 0x0000_4000;
    const STATUS_ACCESS_DENIED_NT: u32 = 0xC000_0022;

    let name = component.encode_wide().collect::<Vec<_>>();
    anyhow::ensure!(
        !name.is_empty() && name.len() * std::mem::size_of::<u16>() <= u16::MAX as usize,
        "invalid workspace path component: {}",
        diagnostic_path.display()
    );
    let byte_length = (name.len() * std::mem::size_of::<u16>()) as u16;
    let unicode_name = NT_UNICODE_STRING {
        Length: byte_length,
        MaximumLength: byte_length,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = NT_OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<NT_OBJECT_ATTRIBUTES>() as u32,
        RootDirectory: parent.0,
        ObjectName: &unicode_name,
        Attributes: 0x40, // OBJ_CASE_INSENSITIVE
        SecurityDescriptor: std::ptr::null(),
        SecurityQualityOfService: std::ptr::null(),
    };
    let mut io_status = unsafe { std::mem::zeroed::<NT_IO_STATUS_BLOCK>() };
    let mut handle = std::ptr::null_mut();
    let desired_access = READ_CONTROL.0
        | FILE_READ_ATTRIBUTES.0
        | if modify_acl { WRITE_DAC.0 } else { 0 }
        | if enumerate_directory {
            FILE_LIST_DIRECTORY.0
        } else {
            0
        };
    let share_mode = FILE_SHARE_READ | FILE_SHARE_WRITE;
    let share_mode = if allow_delete_share {
        share_mode | FILE_SHARE_DELETE
    } else {
        share_mode
    };
    let mut status = unsafe {
        NtCreateFile(
            &mut handle,
            desired_access,
            &attributes,
            &mut io_status,
            std::ptr::null(),
            0,
            share_mode.0,
            FILE_OPEN,
            FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT,
            std::ptr::null(),
            0,
        )
    };
    if (status as u32) == STATUS_ACCESS_DENIED_NT {
        // Cloud sync filters (e.g. cldflt.sys / Nutstore) can reject raw
        // FILE_OPEN_REPARSE_POINT opens with WRITE_DAC on cloud placeholders.
        // Probe with read-only attributes first to verify it is not a name-surrogate
        // symlink/junction before retrying through the cloud filter.
        let mut probe_handle = std::ptr::null_mut();
        let mut probe_io = unsafe { std::mem::zeroed::<NT_IO_STATUS_BLOCK>() };
        let probe_status = unsafe {
            NtCreateFile(
                &mut probe_handle,
                READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0,
                &attributes,
                &mut probe_io,
                std::ptr::null(),
                0,
                (FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE).0,
                FILE_OPEN,
                FILE_OPEN_FOR_BACKUP_INTENT | FILE_OPEN_REPARSE_POINT,
                std::ptr::null(),
                0,
            )
        };
        if probe_status >= 0 && !probe_handle.is_null() {
            let probe = OwnedHandle::new(HANDLE(probe_handle));
            if let Ok(info) = file_information(&probe) {
                if ensure_not_redirection_reparse_point(
                    &probe,
                    &info,
                    "sandbox workspace contains a reparse point",
                    diagnostic_path,
                )
                .is_ok()
                {
                    drop(probe);
                    status = unsafe {
                        NtCreateFile(
                            &mut handle,
                            desired_access,
                            &attributes,
                            &mut io_status,
                            std::ptr::null(),
                            0,
                            share_mode.0,
                            FILE_OPEN,
                            FILE_OPEN_FOR_BACKUP_INTENT,
                            std::ptr::null(),
                            0,
                        )
                    };
                }
            }
        }
    }
    anyhow::ensure!(
        status >= 0 && !handle.is_null(),
        "NtCreateFile relative no-follow open failed for {}: NTSTATUS 0x{:08X}",
        diagnostic_path.display(),
        status as u32
    );
    Ok(OwnedHandle::new(HANDLE(handle)))
}

/// Opens each component from the local volume root, rejecting reparse points
/// at every level. The returned ancestor handles keep the selected parent chain
/// alive while the temporary workspace ACL is active.
fn open_workspace_parent_chain(root: &Path) -> Result<(PathBuf, Vec<OwnedHandle>, OwnedHandle)> {
    open_workspace_parent_chain_with_mode(root, true)
}

fn open_workspace_parent_chain_with_mode(
    root: &Path,
    modify_parent_acl: bool,
) -> Result<(PathBuf, Vec<OwnedHandle>, OwnedHandle)> {
    let root = process_working_directory(root)?;
    let encoded = root.as_os_str().encode_wide().collect::<Vec<_>>();
    anyhow::ensure!(
        encoded.len() >= 3 && encoded[1] == b':' as u16 && encoded[2] == b'\\' as u16,
        "sandbox workspace must resolve to a local drive path"
    );
    let volume_root = PathBuf::from(std::ffi::OsString::from_wide(&encoded[..3]));
    anyhow::ensure!(
        root != volume_root,
        "a drive root cannot be used as an AppContainer workspace"
    );
    let parent = root
        .parent()
        .context("sandbox workspace has no parent directory")?;
    let remaining = parent
        .strip_prefix(&volume_root)
        .context("sandbox workspace parent is outside its local volume root")?;

    let mut ancestors = Vec::new();
    let mut current_path = volume_root.clone();
    let mut current_handle = open_directory_anchor_handle(&volume_root)?;
    ensure_directory_handle(&current_handle, &current_path)?;
    let components = remaining.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let std::path::Component::Normal(component) = component else {
            bail!("invalid component in canonical workspace parent path");
        };
        current_path.push(component);
        let want_modify = modify_parent_acl && index + 1 == components.len();
        let next = open_relative_acl_handle(
            current_handle.raw(),
            component,
            &current_path,
            false,
            want_modify,
        )?;
        ensure_directory_handle(&next, &current_path)?;
        ancestors.push(current_handle);
        current_handle = next;
    }
    Ok((root, ancestors, current_handle))
}

fn ensure_directory_handle(handle: &OwnedHandle, path: &Path) -> Result<()> {
    let info = file_information(handle)?;
    ensure_not_redirection_reparse_point(
        handle,
        &info,
        "sandbox workspace ancestor is a reparse point",
        path,
    )?;
    anyhow::ensure!(
        info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
        "sandbox workspace ancestor is not a directory: {}",
        path.display()
    );
    Ok(())
}

/// Returns true only for in-place cloud sync / system filter reparse tags
/// (Windows Cloud Files API `IO_REPARSE_TAG_CLOUD*`, `IO_REPARSE_TAG_PROJFS`,
/// `IO_REPARSE_TAG_DEDUP`, `IO_REPARSE_TAG_WOF`) that are NOT name-surrogate
/// path redirection links (`IO_REPARSE_TAG_SYMLINK`, `IO_REPARSE_TAG_MOUNT_POINT`).
fn is_benign_cloud_reparse_tag(tag: u32) -> bool {
    const NAME_SURROGATE_BIT: u32 = 0x2000_0000;
    const IO_REPARSE_TAG_CLOUD_BASE: u32 = 0x9000_001A;
    const IO_REPARSE_TAG_CLOUD_MASK: u32 = 0xFFFF_0FFF;
    const IO_REPARSE_TAG_PROJFS: u32 = 0x9000_001C;
    const IO_REPARSE_TAG_DEDUP: u32 = 0x8000_0013;
    const IO_REPARSE_TAG_WOF: u32 = 0x8000_0017;

    if tag & NAME_SURROGATE_BIT != 0 {
        return false;
    }
    (tag & IO_REPARSE_TAG_CLOUD_MASK) == IO_REPARSE_TAG_CLOUD_BASE
        || tag == IO_REPARSE_TAG_PROJFS
        || tag == IO_REPARSE_TAG_DEDUP
        || tag == IO_REPARSE_TAG_WOF
}

fn ensure_not_redirection_reparse_point(
    handle: &OwnedHandle,
    info: &BY_HANDLE_FILE_INFORMATION,
    message_prefix: &str,
    path: &Path,
) -> Result<()> {
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 == 0 {
        return Ok(());
    }
    let mut tag_info = FILE_ATTRIBUTE_TAG_INFO::default();
    let tag_ok = unsafe {
        GetFileInformationByHandleEx(
            handle.raw(),
            FileAttributeTagInfo,
            (&mut tag_info as *mut FILE_ATTRIBUTE_TAG_INFO).cast(),
            std::mem::size_of::<FILE_ATTRIBUTE_TAG_INFO>() as u32,
        )
    };
    if tag_ok.is_ok() && is_benign_cloud_reparse_tag(tag_info.ReparseTag) {
        return Ok(());
    }
    bail!("{message_prefix}: {}", path.display())
}

fn is_write_dac_denied_error(error: &anyhow::Error) -> bool {
    let text = format!("{error:#}");
    text.contains("NTSTATUS 0xC0000022")
        || text.contains("NTSTATUS 0xC0000043")
        || text.contains("SetSecurityInfo failed")
        || text.contains("exceeds per-file AppContainer ACL lease limit")
}

struct PinnedReadonlyWorkspace {
    _ancestor_handles: Vec<OwnedHandle>,
    _parent_handle: OwnedHandle,
    _root_handle: OwnedHandle,
}

fn verify_and_pin_readonly_workspace(root: &Path) -> Result<PinnedReadonlyWorkspace> {
    let (root, ancestor_handles, parent_handle) = open_workspace_parent_chain_with_mode(root, false)?;
    let parent = root
        .parent()
        .context("sandbox workspace has no parent directory")?;
    let (parent_acl, parent_desc) = get_dacl_handle(parent, &parent_handle)?;
    unsafe {
        let _ = LocalFree(HLOCAL(parent_desc.0));
    }
    if parent_acl.is_none() {
        bail!("refusing sandbox scope on a null DACL: {}", parent.display());
    }
    let component = root
        .file_name()
        .context("workspace root has no final path component")?;
    let root_handle =
        open_relative_acl_handle(parent_handle.raw(), component, &root, true, false)?;
    ensure_directory_handle(&root_handle, &root)?;
    let (root_acl, root_desc) = get_dacl_handle(&root, &root_handle)?;
    unsafe {
        let _ = LocalFree(HLOCAL(root_desc.0));
    }
    if root_acl.is_none() {
        bail!("refusing sandbox scope on a null DACL: {}", root.display());
    }
    Ok(PinnedReadonlyWorkspace {
        _ancestor_handles: ancestor_handles,
        _parent_handle: parent_handle,
        _root_handle: root_handle,
    })
}

/// Checks whether the command invokes a developer toolchain binary
/// (such as `node`, `npm`, `npx`, `python`, `cargo`, `rustc`, `uv`, `pnpm`, `yarn`, `bun`, `git`)
/// that requires standard user token access to toolchain caches/runtimes and must run inside
/// the Job Object + pinned workspace root runner rather than a zero-capability LowBox AppContainer.
fn command_requires_unprivileged_toolchain_fallback(
    program: &OsStr,
    args: &[std::ffi::OsString],
    _env_overrides: &[(std::ffi::OsString, std::ffi::OsString)],
) -> bool {
    const TOOLCHAIN_BINARIES: &[&str] = &[
        "node", "node.exe", "npm", "npm.cmd", "npx", "npx.cmd", "pnpm", "pnpm.cmd", "yarn",
        "yarn.cmd", "bun", "bun.exe", "python", "python.exe", "python3", "python3.exe", "pip",
        "pip.exe", "pip3", "pip3.exe", "uv", "uv.exe", "pytest", "pytest.exe", "cargo",
        "cargo.exe", "rustc", "rustc.exe", "rustup", "rustup.exe", "tsc", "tsc.cmd", "vitest",
        "vitest.cmd", "dotnet", "dotnet.exe", "go", "go.exe", "git", "git.exe",
    ];
    let combined = std::iter::once(program)
        .chain(args.iter().map(|s| s.as_os_str()))
        .map(|s| s.to_string_lossy())
        .collect::<Vec<_>>()
        .join(" ");
    let lower = combined.to_ascii_lowercase();

    for &tool in TOOLCHAIN_BINARIES {
        let base = tool.trim_end_matches(".exe").trim_end_matches(".cmd");
        if contains_command_token(&lower, base) {
            return true;
        }
    }
    false
}

fn contains_command_token(haystack: &str, token: &str) -> bool {
    for part in haystack.split(|c: char| {
        c.is_whitespace() || matches!(c, ';' | '|' | '&' | '(' | ')' | '"' | '\'' | '\\' | '/')
    }) {
        let trimmed = part.trim_end_matches(".exe").trim_end_matches(".cmd");
        if trimmed == token {
            return true;
        }
    }
    false
}

fn open_directory_anchor_handle(path: &Path) -> Result<OwnedHandle> {
    let path_wide = wide(path.as_os_str());
    let handle = unsafe {
        CreateFileW(
            PCWSTR(path_wide.as_ptr()),
            READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .with_context(|| format!("open local volume root handle for {}", path.display()))?;
    Ok(OwnedHandle::new(handle))
}

/// Enumerates names from the directory object itself, so a renamed/replaced
/// ancestor path cannot redirect the walk to a different directory tree.
fn directory_entry_names(handle: HANDLE) -> Result<Vec<(std::ffi::OsString, bool)>> {
    const BUFFER_BYTES: usize = 64 * 1024;
    const ERROR_NO_MORE_FILES_HRESULT: u32 = 0x8007_0012;
    let mut storage = vec![0u64; BUFFER_BYTES / std::mem::size_of::<u64>()];
    let buffer = storage.as_mut_ptr().cast::<u8>();
    let mut class = FileIdBothDirectoryRestartInfo;
    let mut entries = Vec::new();

    loop {
        let result = unsafe {
            GetFileInformationByHandleEx(handle, class, buffer.cast(), BUFFER_BYTES as u32)
        };
        if let Err(error) = result {
            if error.code().0 as u32 == ERROR_NO_MORE_FILES_HRESULT {
                break;
            }
            return Err(error).context("enumerate sandbox directory by retained handle");
        }
        class = FileIdBothDirectoryInfo;

        let mut offset = 0usize;
        loop {
            let fixed = std::mem::offset_of!(FILE_ID_BOTH_DIR_INFO, FileName);
            anyhow::ensure!(
                offset <= BUFFER_BYTES.saturating_sub(fixed),
                "invalid directory information record offset"
            );
            let record_ptr = unsafe { buffer.add(offset).cast::<FILE_ID_BOTH_DIR_INFO>() };
            let name_bytes =
                unsafe { std::ptr::addr_of!((*record_ptr).FileNameLength).read_unaligned() }
                    as usize;
            let file_attributes =
                unsafe { std::ptr::addr_of!((*record_ptr).FileAttributes).read_unaligned() };
            let next_entry_offset =
                unsafe { std::ptr::addr_of!((*record_ptr).NextEntryOffset).read_unaligned() }
                    as usize;
            anyhow::ensure!(
                name_bytes % std::mem::size_of::<u16>() == 0
                    && name_bytes <= BUFFER_BYTES - offset - fixed,
                "invalid directory entry name length"
            );
            let name_len = name_bytes / std::mem::size_of::<u16>();
            let name_ptr = unsafe { buffer.add(offset + fixed).cast::<u16>() };
            let mut name_buf = Vec::with_capacity(name_len);
            for i in 0..name_len {
                name_buf.push(unsafe { name_ptr.add(i).read_unaligned() });
            }
            let name = std::ffi::OsString::from_wide(&name_buf);
            if name != "." && name != ".." {
                entries.push((
                    name,
                    file_attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0,
                ));
            }
            if next_entry_offset == 0 {
                break;
            }
            anyhow::ensure!(
                next_entry_offset >= fixed
                    && offset + next_entry_offset <= BUFFER_BYTES - fixed,
                "invalid directory information record chain"
            );
            offset += next_entry_offset;
        }
    }
    Ok(entries)
}

fn file_information(handle: &OwnedHandle) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe {
        GetFileInformationByHandle(handle.raw(), &mut info)?;
    }
    Ok(info)
}

fn grant_prepared(prepared: &PreparedAcl, sid: windows::Win32::Security::PSID) -> Result<()> {
    let aligned_acl = aligned_acl(&prepared.acl);
    let old_acl = aligned_acl.as_ptr().cast::<ACL>();
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: prepared.permissions,
        grfAccessMode: GRANT_ACCESS,
        grfInheritance: prepared.inheritance,
        Trustee: TRUSTEE_W {
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: PWSTR(sid.0.cast()),
            ..Default::default()
        },
    };
    let mut new_acl = std::ptr::null_mut();
    let result = unsafe { SetEntriesInAclW(Some(&[entry]), Some(old_acl), &mut new_acl) };
    anyhow::ensure!(
        result.0 == 0,
        "SetEntriesInAclW failed for {}: {result:?}",
        prepared.path.display()
    );
    let status = unsafe {
        SetSecurityInfo(
            prepared.handle.raw(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    unsafe {
        let _ = LocalFree(HLOCAL(new_acl.cast()));
    }
    anyhow::ensure!(
        status.0 == 0,
        "SetSecurityInfo failed for {}: {status:?}",
        prepared.path.display()
    );
    Ok(())
}

fn aligned_acl(bytes: &[u8]) -> Vec<u32> {
    let mut words = vec![0u32; bytes.len().div_ceil(std::mem::size_of::<u32>())];
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), words.as_mut_ptr().cast(), bytes.len());
    }
    words
}

#[cfg(test)]
fn get_dacl(path: &Path) -> Result<(Option<*mut ACL>, PSECURITY_DESCRIPTOR)> {
    let path_wide = wide(path.as_os_str());
    let mut old_dacl = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let result = unsafe {
        GetNamedSecurityInfoW(
            PCWSTR(path_wide.as_ptr()),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            &mut descriptor,
        )
    };
    anyhow::ensure!(
        result.0 == 0,
        "GetNamedSecurityInfoW failed for {}: {result:?}",
        path.display()
    );
    Ok(((!old_dacl.is_null()).then_some(old_dacl), descriptor))
}

fn get_dacl_handle(
    path: &Path,
    handle: &OwnedHandle,
) -> Result<(Option<*mut ACL>, PSECURITY_DESCRIPTOR)> {
    let mut old_dacl = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    let result = unsafe {
        GetSecurityInfo(
            handle.raw(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(&mut old_dacl),
            None,
            Some(&mut descriptor),
        )
    };
    anyhow::ensure!(
        result.0 == 0,
        "GetSecurityInfo failed for {}: {result:?}",
        path.display()
    );
    Ok(((!old_dacl.is_null()).then_some(old_dacl), descriptor))
}

fn dacl_is_protected(descriptor: PSECURITY_DESCRIPTOR) -> Result<bool> {
    let mut control = 0u16;
    let mut revision = 0u32;
    unsafe {
        windows::Win32::Security::GetSecurityDescriptorControl(
            descriptor,
            &mut control,
            &mut revision,
        )?;
    }
    Ok(control & SE_DACL_PROTECTED.0 != 0)
}

#[cfg(test)]
fn set_dacl(path: &Path, acl: &[u8], protected: bool) -> Result<()> {
    let handle = open_acl_handle(path)?;
    set_dacl_handle(path, &handle, acl, protected)
}

fn set_dacl_handle(path: &Path, handle: &OwnedHandle, acl: &[u8], protected: bool) -> Result<()> {
    let protection = if protected {
        PROTECTED_DACL_SECURITY_INFORMATION
    } else {
        UNPROTECTED_DACL_SECURITY_INFORMATION
    };
    let aligned = aligned_acl(acl);
    let status = unsafe {
        SetSecurityInfo(
            handle.raw(),
            SE_FILE_OBJECT,
            OBJECT_SECURITY_INFORMATION(DACL_SECURITY_INFORMATION.0 | protection.0),
            None,
            None,
            Some(aligned.as_ptr().cast()),
            None,
        )
    };
    anyhow::ensure!(
        status.0 == 0,
        "SetSecurityInfo failed for {}: {status:?}",
        path.display()
    );
    Ok(())
}

fn revoke_sid_handle(
    path: &Path,
    handle: &OwnedHandle,
    sid: windows::Win32::Security::PSID,
) -> Result<()> {
    let (old_acl, descriptor) = get_dacl_handle(path, handle)?;
    let Some(old_acl) = old_acl else {
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        return Ok(());
    };
    let entry = EXPLICIT_ACCESS_W {
        grfAccessPermissions: 0,
        grfAccessMode: REVOKE_ACCESS,
        grfInheritance: ACE_FLAGS(0),
        Trustee: TRUSTEE_W {
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: PWSTR(sid.0.cast()),
            ..Default::default()
        },
    };
    let mut new_acl = std::ptr::null_mut();
    let result = unsafe { SetEntriesInAclW(Some(&[entry]), Some(old_acl), &mut new_acl) };
    if result.0 != 0 {
        unsafe {
            let _ = LocalFree(HLOCAL(descriptor.0));
        }
        bail!(
            "SetEntriesInAclW revoke failed for {}: {result:?}",
            path.display()
        );
    }
    let status = unsafe {
        SetSecurityInfo(
            handle.raw(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            None,
            None,
            Some(new_acl),
            None,
        )
    };
    unsafe {
        let _ = LocalFree(HLOCAL(new_acl.cast()));
        let _ = LocalFree(HLOCAL(descriptor.0));
    }
    anyhow::ensure!(
        status.0 == 0,
        "SetSecurityInfo revoke failed for {}: {status:?}",
        path.display()
    );
    Ok(())
}
