//! Current-user Secret Service with encrypted sessions and explicit no-prompt policy.
use anyhow::{ensure, Context, Result};
use secret_service::{blocking::SecretService, EncryptionType};
use std::collections::HashMap;

fn identity(reference: &[u8]) -> Result<&str> {
    let text = std::str::from_utf8(reference)?;
    ensure!(
        uuid::Uuid::parse_str(text)?.hyphenated().to_string() == text,
        "Invalid Secret Service reference"
    );
    Ok(text)
}

fn attributes(account: &str) -> HashMap<&str, &str> {
    HashMap::from([
        ("application", "com.agentz.desktop"),
        ("purpose", "master-key.v1"),
        ("account", account),
    ])
}

#[cfg(debug_assertions)]
fn in_memory_keyring() -> Option<&'static std::sync::Mutex<HashMap<String, Vec<u8>>>> {
    if std::env::var("AGENTZ_TEST_INMEMORY_KEYRING").as_deref() != Ok("1") {
        return None;
    }
    static STORE: std::sync::OnceLock<std::sync::Mutex<HashMap<String, Vec<u8>>>> =
        std::sync::OnceLock::new();
    Some(STORE.get_or_init(Default::default))
}

pub(super) fn protect(key: &[u8]) -> Result<Vec<u8>> {
    ensure!(key.len() == 32, "Invalid master key length");
    let reference = uuid::Uuid::new_v4().hyphenated().to_string().into_bytes();
    #[cfg(debug_assertions)]
    if let Some(store) = in_memory_keyring() {
        let account = identity(&reference)?;
        let mut map = store
            .lock()
            .map_err(|_| anyhow::anyhow!("In-memory keyring poisoned"))?;
        ensure!(
            !map.contains_key(account),
            "Secret Service identity collision"
        );
        map.insert(account.to_string(), key.to_vec());
        return Ok(reference);
    }
    let service = SecretService::connect(EncryptionType::Dh)
        .context("Linux encrypted credential service unavailable")?;
    // No session-only fallback, collection creation, unlock, or prompt execution.
    let collection = service
        .get_default_collection()
        .context("Linux default credential collection unavailable")?;
    collection
        .ensure_unlocked()
        .context("Linux credential collection is locked")?;
    let account = identity(&reference)?;
    ensure!(
        collection.search_items(attributes(account))?.is_empty(),
        "Secret Service identity collision"
    );
    let created = collection
        .create_item(
            "AgentZ local master key",
            attributes(account),
            key,
            false,
            "application/octet-stream",
        )
        .context("Linux credential creation failed; interactive prompts are disabled");
    let item = match created {
        Ok(item) => item,
        Err(error) => {
            // A service may commit before a response is lost. Search only this fresh
            // identity; never remove another account's item or overwrite its secret.
            if let Err(cleanup) = remove(&reference) {
                anyhow::bail!(
                    "Credential creation failed: {error}; cleanup failed for {account}: {cleanup}"
                );
            }
            return Err(error);
        }
    };
    let verified = (|| -> Result<()> {
        item.ensure_unlocked()?;
        ensure!(
            item.get_secret()? == key,
            "Secret Service master key readback mismatch"
        );
        Ok(())
    })();
    if let Err(error) = verified {
        if let Err(cleanup) = item.delete() {
            anyhow::bail!(
                "Credential verification failed: {error}; cleanup failed for {account}: {cleanup}"
            );
        }
        return Err(error);
    }
    Ok(reference)
}

pub(super) fn read(reference: &[u8]) -> Result<Vec<u8>> {
    let account = identity(reference)?;
    #[cfg(debug_assertions)]
    if let Some(store) = in_memory_keyring() {
        let map = store
            .lock()
            .map_err(|_| anyhow::anyhow!("In-memory keyring poisoned"))?;
        let key = map
            .get(account)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Secret Service credential missing or ambiguous"))?;
        ensure!(key.len() == 32, "Invalid Secret Service master key length");
        return Ok(key);
    }
    let service = SecretService::connect(EncryptionType::Dh)
        .context("Linux encrypted credential service unavailable")?;
    let collection = service.get_default_collection()?;
    collection
        .ensure_unlocked()
        .context("Linux credential collection is locked")?;
    let items = collection.search_items(attributes(account))?;
    ensure!(
        items.len() == 1,
        "Secret Service credential missing or ambiguous"
    );
    let item = &items[0];
    item.ensure_unlocked()
        .context("Linux credential is locked")?;
    let key = item.get_secret()?;
    ensure!(key.len() == 32, "Invalid Secret Service master key length");
    Ok(key)
}

pub(super) fn remove(reference: &[u8]) -> Result<()> {
    let account = identity(reference)?;
    #[cfg(debug_assertions)]
    if let Some(store) = in_memory_keyring() {
        let mut map = store
            .lock()
            .map_err(|_| anyhow::anyhow!("In-memory keyring poisoned"))?;
        map.remove(account);
        return Ok(());
    }
    let service = SecretService::connect(EncryptionType::Dh)
        .context("Linux encrypted credential service unavailable during cleanup")?;
    let collection = service.get_default_collection()?;
    collection.ensure_unlocked()?;
    let items = collection.search_items(attributes(account))?;
    ensure!(
        items.len() <= 1,
        "Ambiguous Secret Service cleanup identity {account}"
    );
    if let Some(item) = items.first() {
        item.ensure_unlocked()?;
        item.delete().with_context(|| {
            format!("Secret Service cleanup failed for {account}; prompts disabled")
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires an unlocked Linux Secret Service; creates and removes temporary credentials"]
    fn linux_secret_service_roundtrip_revocation_and_migration() -> Result<()> {
        struct Cleanup(Vec<u8>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if !self.0.is_empty() {
                    if let Err(error) = remove(&self.0) {
                        eprintln!("Temporary Secret Service credential cleanup failed: {error}");
                    }
                }
            }
        }
        let key = [59u8; 32];
        let reference = protect(&key)?;
        let mut cleanup = Cleanup(reference.clone());
        ensure!(read(&reference)? == key);
        ensure!(read(b"invalid-reference").is_err());
        remove(&reference)?;
        remove(&reference)?;
        cleanup.0.clear();
        ensure!(
            read(&reference).is_err(),
            "Revoked credential remained readable"
        );

        let directory = tempfile::tempdir()?;
        let path = directory.path().join(".secret_key");
        std::fs::write(&path, key)?;
        let migrated = super::super::secrets::SecretStore::new(directory.path())?;
        let data = std::fs::read(&path)?;
        let reference = data
            .strip_prefix(b"AGENTZ-KEY-V1:SECRET-SERVICE\0")
            .ok_or_else(|| anyhow::anyhow!("Missing Secret Service reference header"))?;
        let mut cleanup = Cleanup(reference.to_vec());
        ensure!(read(reference)? == key, "Migration changed legacy key");
        let ciphertext = migrated.encrypt_hex("temporary-fake-credential")?;
        let reopened = super::super::secrets::SecretStore::new(directory.path())?;
        ensure!(reopened.decrypt_hex(&ciphertext)? == "temporary-fake-credential");
        remove(reference)?;
        cleanup.0.clear();
        ensure!(super::super::secrets::SecretStore::new(directory.path()).is_err());
        ensure!(
            std::fs::read(&path)? == data,
            "Revocation replaced the existing reference"
        );
        Ok(())
    }

    #[test]
    #[ignore = "Run on Linux with DBUS_SESSION_BUS_ADDRESS=unix:path=/nonexistent/agentz-test-bus"]
    fn linux_secret_service_unavailable_preserves_legacy_key() -> Result<()> {
        ensure!(
            std::env::var("DBUS_SESSION_BUS_ADDRESS")? == "unix:path=/nonexistent/agentz-test-bus",
            "Explicit isolated invalid bus address required"
        );
        let directory = tempfile::tempdir()?;
        let path = directory.path().join(".secret_key");
        let key = [71u8; 32];
        std::fs::write(&path, key)?;
        ensure!(super::super::secrets::SecretStore::new(directory.path()).is_err());
        ensure!(std::fs::read(&path)? == key);
        let empty = tempfile::tempdir()?;
        ensure!(super::super::secrets::SecretStore::new(empty.path()).is_err());
        ensure!(!empty.path().join(".secret_key").exists());
        Ok(())
    }
}
