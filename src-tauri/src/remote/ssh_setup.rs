//! One-time password login that installs the user's public key on a host, so
//! every later connection (which runs non-interactively) uses key auth.

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

use super::{dirs_home, shell_quote, RemoteTarget};

const OK_MARKER: &str = "AGENTZ_KEY_OK";

fn ssh_dir() -> Result<PathBuf, String> {
    Ok(dirs_home().ok_or("cannot locate home directory")?.join(".ssh"))
}

/// Existing public key, or a freshly generated passphrase-less ed25519 key.
pub async fn ensure_public_key() -> Result<String, String> {
    let dir = ssh_dir()?;
    for name in ["id_ed25519.pub", "id_ecdsa.pub", "id_rsa.pub"] {
        if let Ok(k) = std::fs::read_to_string(dir.join(name)) {
            if !k.trim().is_empty() {
                return Ok(k.trim().to_string());
            }
        }
    }
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let key = dir.join("id_ed25519");
    let out = piscis_kernel::proc::tokio_command("ssh-keygen")
        .args(["-t", "ed25519", "-q", "-N", "", "-C", "agentz", "-f"])
        .arg(&key)
        .output()
        .await
        .map_err(|e| format!("ssh-keygen unavailable: {e}"))?;
    if !out.status.success() {
        return Err(format!("ssh-keygen failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    std::fs::read_to_string(key.with_extension("pub"))
        .map(|k| k.trim().to_string())
        .map_err(|e| e.to_string())
}

/// Log in with `password` once and append `pubkey` to `authorized_keys`.
fn install_key_blocking(host: &str, password: &str, pubkey: &str) -> Result<(), String> {
    let pair = native_pty_system()
        .openpty(PtySize { rows: 24, cols: 200, pixel_width: 0, pixel_height: 0 })
        .map_err(|e| format!("open pty: {e}"))?;
    let q = shell_quote(pubkey);
    let remote = format!(
        "umask 077; mkdir -p ~/.ssh && touch ~/.ssh/authorized_keys && \
         (grep -qxF {q} ~/.ssh/authorized_keys || echo {q} >> ~/.ssh/authorized_keys) && echo {OK_MARKER}"
    );
    let mut cmd = CommandBuilder::new("ssh");
    cmd.args([
        "-o",
        "StrictHostKeyChecking=accept-new",
        "-o",
        "NumberOfPasswordPrompts=1",
        "-o",
        "PreferredAuthentications=password,keyboard-interactive",
        host,
        &remote,
    ]);
    let mut child = pair.slave.spawn_command(cmd).map_err(|e| format!("spawn ssh: {e}"))?;
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().map_err(|e| e.to_string())?;
    let mut writer = pair.master.take_writer().map_err(|e| e.to_string())?;

    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 1024];
        while let Ok(n) = reader.read(&mut buf) {
            if n == 0 || tx.send(String::from_utf8_lossy(&buf[..n]).into_owned()).is_err() {
                break;
            }
        }
    });

    let deadline = Instant::now() + Duration::from_secs(60);
    let mut seen = String::new();
    let mut sent = false;
    let result = loop {
        if Instant::now() > deadline {
            break Err("timed out waiting for the host".to_string());
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => seen.push_str(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                break if seen.contains(OK_MARKER) {
                    Ok(())
                } else {
                    Err(last_line(&seen))
                };
            }
        }
        let lower = seen.to_lowercase();
        if seen.contains(OK_MARKER) {
            break Ok(());
        }
        if lower.contains("permission denied") || lower.contains("could not resolve") || lower.contains("connection refused") {
            break Err(last_line(&seen));
        }
        if !sent && (lower.contains("password:") || lower.contains("password for")) {
            writer
                .write_all(format!("{password}\r").as_bytes())
                .map_err(|e| e.to_string())?;
            let _ = writer.flush();
            sent = true;
        }
    };
    let _ = child.kill();
    result
}

fn last_line(s: &str) -> String {
    s.lines()
        .map(str::trim)
        .rfind(|l| !l.is_empty())
        .unwrap_or("ssh exited without output")
        .to_string()
}

/// Install key auth for `host`, then confirm a non-interactive login works.
pub async fn setup_key_auth(host: &str, password: &str) -> Result<String, String> {
    let pubkey = ensure_public_key().await?;
    let (h, p, k) = (host.to_string(), password.to_string(), pubkey.clone());
    tokio::task::spawn_blocking(move || install_key_blocking(&h, &p, &k))
        .await
        .map_err(|e| e.to_string())??;
    RemoteTarget::Ssh { host: host.to_string() }
        .run("echo ok")
        .await
        .map_err(|e| format!("key installed but key login still fails: {e}"))?;
    Ok(pubkey)
}

#[cfg(test)]
mod live {
    /// Manual: `AGENTZ_LIVE_HOST=user@host AGENTZ_LIVE_PW=... cargo test --lib live_setup_key -- --ignored --nocapture`
    #[tokio::test]
    #[ignore]
    async fn live_setup_key() {
        let host = std::env::var("AGENTZ_LIVE_HOST").expect("AGENTZ_LIVE_HOST");
        let pw = std::env::var("AGENTZ_LIVE_PW").expect("AGENTZ_LIVE_PW");
        let r = super::setup_key_auth(&host, &pw).await;
        println!("setup_key_auth => {r:?}");
        assert!(r.is_ok());
    }
}
