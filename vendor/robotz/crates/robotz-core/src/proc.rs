//! Cross-platform process spawning that never flashes a console window on
//! Windows.
//!
//! Every short-lived child process (xdotool, cliclick, powershell, …) on
//! Windows would otherwise flash a blue console window. Routing every spawn
//! through these helpers applies `CREATE_NO_WINDOW` uniformly. The helpers
//! are no-ops on non-Windows platforms.
//!
//! Vendored from `piscis_kernel::proc` so RobotZ has no runtime dependency on
//! the piscis engine.

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Build a [`tokio::process::Command`] that hides any child console window on
/// Windows. Use this everywhere instead of `tokio::process::Command::new`.
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
    fn commands_construct() {
        let _ = tokio_command("echo");
        let _ = std_command("echo");
    }
}
