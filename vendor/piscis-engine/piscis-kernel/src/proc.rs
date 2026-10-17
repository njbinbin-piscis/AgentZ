//! Cross-platform process spawning that never flashes a console window on Windows.
//!
//! Every `Command::new` invocation across the workspace must go through one of
//! the helpers in this module so the `CREATE_NO_WINDOW` flag is applied
//! uniformly on Windows. A `clippy.toml` `disallowed-methods` rule enforces
//! this at lint time — the only place `tokio::process::Command::new` and
//! `std::process::Command::new` are allowed is right here.
//!
//! Why this matters: missing the `CREATE_NO_WINDOW` flag causes a brief blue
//! console window to flash on screen for every short-lived child process
//! (ripgrep, npx, git, powershell, …). When a feature triggers many spawns
//! in sequence (file watcher → git status, search panel → ripgrep on every
//! keystroke, etc.) those flashes look like a popup storm to the user.
//!
//! The helpers are no-ops on non-Windows platforms.

#[cfg(windows)]
#[path = "proc_windows_sandbox.rs"]
pub mod windows_sandbox;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a [`tokio::process::Command`] that hides any child console window on
/// Windows. Use this everywhere instead of `tokio::process::Command::new`.
#[allow(clippy::disallowed_methods)]
pub fn tokio_command<S: AsRef<std::ffi::OsStr>>(program: S) -> tokio::process::Command {
    #[cfg(windows)]
    {
        let mut cmd = tokio::process::Command::new(program);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        tokio::process::Command::new(program)
    }
}

/// Build a [`std::process::Command`] that hides any child console window on
/// Windows. Use this everywhere instead of `std::process::Command::new`.
#[allow(clippy::disallowed_methods)]
pub fn std_command<S: AsRef<std::ffi::OsStr>>(program: S) -> std::process::Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let mut cmd = std::process::Command::new(program);
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd
    }
    #[cfg(not(windows))]
    {
        std::process::Command::new(program)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokio_command_returns_runnable_command() {
        // Just exercise the constructor — actually spawning would require a
        // platform-specific binary, which CI doesn't always have.
        let _ = tokio_command("echo");
    }

    #[test]
    fn std_command_returns_runnable_command() {
        let _ = std_command("echo");
    }

    #[cfg(windows)]
    #[test]
    fn windows_appcontainer_child_is_confined_to_explicit_directory() {
        windows_appcontainer_probe::run();
    }

    #[cfg(windows)]
    mod windows_appcontainer_probe {
        use std::alloc::{alloc_zeroed, dealloc, Layout};
        use std::ffi::OsStr;
        use std::mem::{align_of, size_of, MaybeUninit};
        use std::os::windows::ffi::OsStrExt;
        use std::path::Path;
        use uuid::Uuid;
        use windows::core::{PCWSTR, PWSTR};
        use windows::Win32::Foundation::{CloseHandle, HLOCAL, WAIT_OBJECT_0};
        use windows::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
        use windows::Win32::Security::Isolation::{
            CreateAppContainerProfile, DeleteAppContainerProfile,
        };
        use windows::Win32::Security::{
            FreeSid, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, SECURITY_CAPABILITIES,
        };
        use windows::Win32::System::JobObjects::{
            AssignProcessToJobObject, CreateJobObjectW, JobObjectBasicAccountingInformation,
            JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
            JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
            JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        };
        use windows::Win32::System::Threading::{
            CreateProcessW, DeleteProcThreadAttributeList, GetExitCodeProcess,
            InitializeProcThreadAttributeList, ResumeThread, TerminateProcess,
            UpdateProcThreadAttribute, WaitForSingleObject, CREATE_NO_WINDOW, CREATE_SUSPENDED,
            EXTENDED_STARTUPINFO_PRESENT, LPPROC_THREAD_ATTRIBUTE_LIST, PROCESS_INFORMATION,
            PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES, STARTUPINFOEXW,
        };

        fn wide(value: &OsStr) -> Vec<u16> {
            value.encode_wide().chain(std::iter::once(0)).collect()
        }

        fn dacl_bytes(path: &Path) -> anyhow::Result<Vec<u8>> {
            let path_wide = wide(path.as_os_str());
            let mut dacl = std::ptr::null_mut();
            let mut descriptor = PSECURITY_DESCRIPTOR::default();
            let result = unsafe {
                GetNamedSecurityInfoW(
                    PCWSTR(path_wide.as_ptr()),
                    SE_FILE_OBJECT,
                    DACL_SECURITY_INFORMATION,
                    None,
                    None,
                    Some(&mut dacl),
                    None,
                    &mut descriptor,
                )
            };
            anyhow::ensure!(result.0 == 0, "GetNamedSecurityInfoW failed: {result:?}");
            let bytes = if dacl.is_null() {
                Vec::new()
            } else {
                unsafe {
                    std::slice::from_raw_parts(dacl.cast::<u8>(), (*dacl).AclSize as usize).to_vec()
                }
            };
            unsafe {
                let _ = windows::Win32::Foundation::LocalFree(HLOCAL(descriptor.0));
            }
            Ok(bytes)
        }

        fn launch_in_empty_appcontainer(
            app: &Path,
            command: &str,
            allow_root: Option<&Path>,
            descendant_ready: Option<&Path>,
        ) -> anyhow::Result<(u32, u32)> {
            let profile = format!("AgentZProbe-{}", Uuid::new_v4());
            let profile_wide = wide(OsStr::new(&profile));
            let display = wide(OsStr::new("AgentZ disposable test"));
            let description = wide(OsStr::new("Temporary AppContainer isolation probe"));
            let app_wide = wide(app.as_os_str());
            let mut command_wide = wide(OsStr::new(command));
            let cwd = allow_root
                .map(Path::to_path_buf)
                .map(Ok)
                .unwrap_or_else(std::env::current_dir)?;
            let cwd_wide = wide(cwd.as_os_str());

            // This probe deliberately grants no capabilities and no workspace ACL. It validates
            // that Windows starts the child in an AppContainer and denies a normal user path.
            let sid = unsafe {
                CreateAppContainerProfile(
                    PCWSTR(profile_wide.as_ptr()),
                    PCWSTR(display.as_ptr()),
                    PCWSTR(description.as_ptr()),
                    None,
                )?
            };
            let mut workspace_access = if let Some(root) = allow_root {
                match unsafe { super::super::windows_sandbox::WorkspaceAclLease::grant(root, sid) }
                {
                    Ok(lease) => Some(lease),
                    Err(error) => {
                        unsafe {
                            let _ = FreeSid(sid);
                            let _ = DeleteAppContainerProfile(PCWSTR(profile_wide.as_ptr()));
                        }
                        return Err(error);
                    }
                }
            } else {
                None
            };

            let mut attr_size = MaybeUninit::<usize>::uninit();
            let _ = unsafe {
                InitializeProcThreadAttributeList(
                    LPPROC_THREAD_ATTRIBUTE_LIST::default(),
                    1,
                    0,
                    attr_size.as_mut_ptr(),
                )
            };
            let attr_size = unsafe { attr_size.assume_init() };
            anyhow::ensure!(
                attr_size > 0,
                "Windows did not return AppContainer attribute size"
            );
            let layout = Layout::from_size_align(attr_size, align_of::<usize>())?;
            let attr_mem = unsafe { alloc_zeroed(layout) };
            anyhow::ensure!(
                !attr_mem.is_null(),
                "failed to allocate process attribute list"
            );
            let attr_list = LPPROC_THREAD_ATTRIBUTE_LIST(attr_mem.cast());
            let mut initialized = false;
            let mut process_info = PROCESS_INFORMATION::default();
            let job = unsafe { CreateJobObjectW(None, PCWSTR::null())? };
            let mut job_limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            job_limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;

            let result = (|| -> anyhow::Result<(u32, u32)> {
                unsafe {
                    SetInformationJobObject(
                        job,
                        JobObjectExtendedLimitInformation,
                        (&job_limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                        size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                    )?;
                }
                unsafe {
                    InitializeProcThreadAttributeList(attr_list, 1, 0, &mut (attr_size.clone()))?
                };
                initialized = true;
                let capabilities = SECURITY_CAPABILITIES {
                    AppContainerSid: sid,
                    Capabilities: std::ptr::null_mut(),
                    CapabilityCount: 0,
                    Reserved: 0,
                };
                unsafe {
                    UpdateProcThreadAttribute(
                        attr_list,
                        0,
                        PROC_THREAD_ATTRIBUTE_SECURITY_CAPABILITIES as usize,
                        Some((&capabilities as *const SECURITY_CAPABILITIES).cast()),
                        size_of::<SECURITY_CAPABILITIES>(),
                        None,
                        None,
                    )?;
                }
                let mut startup = STARTUPINFOEXW::default();
                startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
                startup.lpAttributeList = attr_list;
                unsafe {
                    CreateProcessW(
                        PCWSTR(app_wide.as_ptr()),
                        PWSTR(command_wide.as_mut_ptr()),
                        None,
                        None,
                        false,
                        EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW | CREATE_SUSPENDED,
                        None,
                        PCWSTR(cwd_wide.as_ptr()),
                        &startup.StartupInfo,
                        &mut process_info,
                    )?;
                    AssignProcessToJobObject(job, process_info.hProcess)?;
                    anyhow::ensure!(
                        ResumeThread(process_info.hThread) != u32::MAX,
                        "ResumeThread failed for AppContainer child"
                    );
                }
                let wait = unsafe { WaitForSingleObject(process_info.hProcess, 10_000) };
                if wait != WAIT_OBJECT_0 {
                    unsafe { TerminateProcess(process_info.hProcess, 124)? };
                    anyhow::bail!("AppContainer child did not exit within 10 seconds");
                }
                let mut exit_code = 0;
                unsafe { GetExitCodeProcess(process_info.hProcess, &mut exit_code)? };
                if let Some(ready) = descendant_ready {
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
                    while !ready.exists() && std::time::Instant::now() < deadline {
                        std::thread::sleep(std::time::Duration::from_millis(10));
                    }
                    anyhow::ensure!(ready.exists(), "descendant did not signal readiness");
                }
                let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
                unsafe {
                    QueryInformationJobObject(
                        job,
                        JobObjectBasicAccountingInformation,
                        (&mut accounting as *mut JOBOBJECT_BASIC_ACCOUNTING_INFORMATION).cast(),
                        size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                        None,
                    )?;
                }
                Ok((exit_code, accounting.ActiveProcesses))
            })();

            let acl_cleanup = workspace_access
                .as_mut()
                .map(|lease| lease.finish())
                .transpose();
            drop(workspace_access);
            unsafe {
                if !process_info.hThread.is_invalid() {
                    let _ = CloseHandle(process_info.hThread);
                }
                if !process_info.hProcess.is_invalid() {
                    let _ = CloseHandle(process_info.hProcess);
                }
                // The job is deliberately closed after the root process handle. With
                // KILL_ON_JOB_CLOSE this also reaps descendants the command spawned.
                let _ = CloseHandle(job);
                if initialized {
                    DeleteProcThreadAttributeList(attr_list);
                }
                dealloc(attr_mem, layout);
                let _ = FreeSid(sid);
                let _ = DeleteAppContainerProfile(PCWSTR(profile_wide.as_ptr()));
            }
            match (result, acl_cleanup) {
                (Ok(value), Ok(_)) => Ok(value),
                (Err(error), Ok(_)) => Err(error),
                (Ok(_), Err(error)) => Err(error),
                (Err(error), Err(cleanup)) => Err(error.context(format!(
                    "additionally failed to restore workspace ACLs: {cleanup:#}"
                ))),
            }
        }

        pub(super) fn run() {
            let temp = tempfile::tempdir().expect("create canary directory");
            let work = temp.path().join("work");
            let outside = temp.path().join("outside");
            std::fs::create_dir_all(&work).expect("create allowed directory");
            std::fs::create_dir_all(&outside).expect("create out-of-scope directory");
            let allowed_file = work.join("existing-source.txt");
            let outside_file = outside.join("private.txt");
            std::fs::write(&allowed_file, b"allowed").expect("write in-scope canary");
            std::fs::write(&outside_file, b"must remain private").expect("write canary");
            let parent_acl_before = dacl_bytes(temp.path()).expect("snapshot parent DACL");
            let root_acl_before = dacl_bytes(&work).expect("snapshot workspace DACL");
            let source_acl_before = dacl_bytes(&allowed_file).expect("snapshot source DACL");
            let cmd_path = std::path::PathBuf::from(
                std::env::var("WINDIR").expect("WINDIR") + "\\System32\\cmd.exe",
            );
            let cmd = cmd_path.as_path();

            let allowed_command = format!(
                "\"{}\" /d /c type \"{}\"",
                cmd.display(),
                allowed_file.display()
            );
            let (allowed_exit, allowed_active_processes) =
                launch_in_empty_appcontainer(cmd, &allowed_command, Some(&work), None)
                    .expect("launch AppContainer process with scoped root");
            assert_eq!(
                allowed_exit, 0,
                "AppContainer could not read an existing file in its granted directory"
            );
            assert_eq!(
                allowed_active_processes, 1,
                "the exited root process remains accounted for until its handle closes"
            );

            let denied_command = format!(
                "\"{}\" /d /c type \"{}\"",
                cmd.display(),
                outside_file.display()
            );
            let (denied_exit, _) =
                launch_in_empty_appcontainer(cmd, &denied_command, Some(&work), None)
                    .expect("launch AppContainer process with scoped root");
            assert_ne!(
                denied_exit, 0,
                "AppContainer unexpectedly read an out-of-scope file"
            );

            let created_file = work.join("created-by-container.txt");
            let write_command = format!(
                "\"{}\" /d /c echo written>\"{}\"",
                cmd.display(),
                created_file.display()
            );
            let (write_exit, _) =
                launch_in_empty_appcontainer(cmd, &write_command, Some(&work), None)
                    .expect("launch AppContainer process with scoped write root");
            assert_eq!(
                write_exit, 0,
                "AppContainer could not write in its scoped directory"
            );
            assert_eq!(
                std::fs::read_to_string(&created_file)
                    .expect("read scoped write result")
                    .trim(),
                "written"
            );

            // timeout.exe needs console input and may exit immediately in CREATE_NO_WINDOW.
            // Use a file handshake instead: the child signals readiness, then cannot write
            // the survival marker until the parent releases it after closing the Job.
            let descendant_marker = work.join("descendant-survived-job-close.txt");
            let descendant_ready = work.join("descendant-ready.txt");
            let descendant_release = work.join("descendant-release.txt");
            let descendant_script = work.join("descendant-probe.cmd");
            std::fs::write(
                &descendant_script,
                "@echo off\r\necho ready>descendant-ready.txt\r\n:wait\r\nif not exist descendant-release.txt goto wait\r\necho escaped>descendant-survived-job-close.txt\r\n",
            )
            .expect("write noninteractive descendant probe");
            // Positive control: prove this exact child can write the marker when released.
            std::fs::write(&descendant_release, "release").unwrap();
            let control_command = format!(
                "\"{}\" /d /c call \"{}\"",
                cmd.display(),
                descendant_script.display()
            );
            let (control_exit, _) =
                launch_in_empty_appcontainer(cmd, &control_command, Some(&work), None)
                    .expect("launch released descendant positive control");
            assert_eq!(control_exit, 0);
            assert!(descendant_marker.exists());
            std::fs::remove_file(&descendant_release).unwrap();
            std::fs::remove_file(&descendant_ready).unwrap();
            std::fs::remove_file(&descendant_marker).unwrap();
            let descendant_command = format!(
                "\"{}\" /d /c start \"\" /b \"{}\" /d /c call \"{}\"",
                cmd.display(),
                cmd.display(),
                descendant_script.display()
            );
            let (root_exit, active_after_root) = launch_in_empty_appcontainer(
                cmd,
                &descendant_command,
                Some(&work),
                Some(&descendant_ready),
            )
            .expect("launch AppContainer child with descendant");
            assert_eq!(root_exit, 0, "root process failed to start descendant");
            assert!(
                active_after_root > 1,
                "the Job Object should still contain the delayed descendant after root exit"
            );
            assert!(
                !descendant_marker.exists(),
                "unreleased child wrote survival marker"
            );
            std::fs::write(&descendant_release, "release").unwrap();
            std::thread::sleep(std::time::Duration::from_secs(5));
            assert!(
                !descendant_marker.exists(),
                "descendant outlived kill-on-close Job Object"
            );

            // A loopback listener is created only on this host. No connection leaves the
            // machine; without AppContainer network capabilities the child must fail to connect.
            let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind local probe");
            listener
                .set_nonblocking(true)
                .expect("set nonblocking local listener");
            let port = listener.local_addr().expect("listener address").port();
            let acceptor = std::thread::spawn(move || {
                let deadline = std::time::Instant::now() + std::time::Duration::from_secs(4);
                loop {
                    match listener.accept() {
                        Ok((mut stream, _)) => {
                            use std::io::{Read, Write};
                            let mut request = [0u8; 1024];
                            let _ = stream.read(&mut request);
                            let _ = stream.write_all(
                                b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok",
                            );
                            return true;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                            if std::time::Instant::now() >= deadline {
                                return false;
                            }
                            std::thread::sleep(std::time::Duration::from_millis(20));
                        }
                        Err(_) => return false,
                    }
                }
            });
            let curl =
                Path::new(&(std::env::var("WINDIR").expect("WINDIR") + "\\System32\\curl.exe"))
                    .to_path_buf();
            let network_command = format!(
                "\"{}\" --silent --max-time 2 --connect-timeout 1 http://127.0.0.1:{port}/",
                curl.display()
            );
            let (network_exit, _) =
                launch_in_empty_appcontainer(&curl, &network_command, Some(&work), None)
                    .expect("launch local-only network probe");
            let loopback_connected = acceptor.join().expect("local accept worker");
            assert_eq!(
                loopback_connected, false,
                "AppContainer without network capability unexpectedly reached local loopback"
            );
            assert_ne!(
                network_exit, 0,
                "curl unexpectedly completed the local HTTP probe"
            );
            assert_eq!(
                dacl_bytes(temp.path()).expect("read restored parent DACL"),
                parent_acl_before
            );
            assert_eq!(
                dacl_bytes(&work).expect("read restored workspace DACL"),
                root_acl_before
            );
            assert_eq!(
                dacl_bytes(&allowed_file).expect("read restored source DACL"),
                source_acl_before
            );
        }
    }
}
