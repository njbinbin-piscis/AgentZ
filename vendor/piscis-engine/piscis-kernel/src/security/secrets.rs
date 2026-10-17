use anyhow::Result;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use std::path::Path;

const KEY_FILE: &str = ".secret_key";
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
const PROTECTED_KEY_MAGIC: &[u8] = b"AGENTZ-KEY-V1:DPAPI\0";
#[cfg(target_os = "linux")]
const PROTECTED_KEY_MAGIC: &[u8] = b"AGENTZ-KEY-V1:SECRET-SERVICE\0";
#[cfg(target_os = "macos")]
const PROTECTED_KEY_MAGIC: &[u8] = b"AGENTZ-KEY-V1:KEYCHAIN\0";

pub struct SecretStore {
    key: [u8; 32],
}

impl SecretStore {
    pub fn new(app_dir: &Path) -> Result<Self> {
        Self::new_with_random(app_dir, random_bytes)
    }

    fn new_with_random(
        app_dir: &Path,
        random: impl FnOnce(&mut [u8]) -> Result<()>,
    ) -> Result<Self> {
        Self::new_with_protection_and_cleanup(
            app_dir,
            random,
            protect_key,
            unprotect_key,
            cleanup_key,
        )
    }

    #[cfg(test)]
    fn new_with_protection(
        app_dir: &Path,
        random: impl FnOnce(&mut [u8]) -> Result<()>,
        protect: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
        unprotect: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        Self::new_with_protection_and_cleanup(app_dir, random, protect, unprotect, |_| Ok(()))
    }

    fn new_with_protection_and_cleanup(
        app_dir: &Path,
        random: impl FnOnce(&mut [u8]) -> Result<()>,
        protect: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
        unprotect: impl FnOnce(&[u8]) -> Result<Vec<u8>>,
        cleanup: impl FnOnce(&[u8]) -> Result<()>,
    ) -> Result<Self> {
        let key_path = app_dir.join(KEY_FILE);
        let key = if key_path.exists() {
            let data = std::fs::read(&key_path)?;
            if let Some(blob) = data.strip_prefix(PROTECTED_KEY_MAGIC) {
                let decoded = unprotect(blob)?;
                return Ok(Self {
                    key: decoded
                        .try_into()
                        .map_err(|_| anyhow::anyhow!("Invalid protected secret key length"))?,
                });
            }
            let key: [u8; 32] = data
                .try_into()
                .map_err(|_| anyhow::anyhow!("Invalid or unsupported secret key file"))?;
            // Keep the same encryption key, and publish the OS-protected form
            // atomically. Failure preserves the legacy key and its ciphertext.
            let protected = protect(&key)?;
            if let Err(error) = persist_protected_key(app_dir, &key_path, &protected, false) {
                return Err(cleanup_failed_publication(error, &protected, cleanup));
            }
            key
        } else {
            let mut key = [0u8; 32];
            random(&mut key)?;
            if let Some(parent) = key_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let protected = protect(&key)?;
            match persist_protected_key(app_dir, &key_path, &protected, true) {
                Ok(()) => key,
                Err(error)
                    if error
                        .downcast_ref::<std::io::Error>()
                        .is_some_and(|error| error.kind() == std::io::ErrorKind::AlreadyExists) =>
                {
                    cleanup(&protected).map_err(|error| {
                        anyhow::anyhow!("Concurrent key publication cleanup failed: {error}")
                    })?;
                    return Self::new(app_dir);
                }
                Err(error) => return Err(cleanup_failed_publication(error, &protected, cleanup)),
            }
        };
        Ok(Self { key })
    }

    pub fn encrypt(&self, plaintext: &str) -> Result<Vec<u8>> {
        self.encrypt_with_random(plaintext, random_bytes)
    }

    fn encrypt_with_random(
        &self,
        plaintext: &str,
        random: impl FnOnce(&mut [u8]) -> Result<()>,
    ) -> Result<Vec<u8>> {
        if plaintext.is_empty() {
            return Ok(Vec::new());
        }
        let cipher = ChaCha20Poly1305::new((&self.key).into());
        let mut nonce_bytes = [0u8; 12];
        random(&mut nonce_bytes)?;
        let nonce = Nonce::from_slice(&nonce_bytes);
        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| anyhow::anyhow!("Encryption failed: {}", e))?;
        let mut result = Vec::with_capacity(12 + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);
        Ok(result)
    }

    pub fn decrypt(&self, ciphertext: &[u8]) -> Result<String> {
        if ciphertext.is_empty() {
            return Ok(String::new());
        }
        if ciphertext.len() < 12 + 16 {
            return Err(anyhow::anyhow!(
                "Ciphertext too short for ChaCha20-Poly1305"
            ));
        }
        let cipher = ChaCha20Poly1305::new((&self.key).into());
        let nonce = Nonce::from_slice(&ciphertext[..12]);
        let plaintext = cipher
            .decrypt(nonce, &ciphertext[12..])
            .map_err(|_| anyhow::anyhow!("Decryption failed — wrong key or corrupted data"))?;
        String::from_utf8(plaintext).map_err(|e| anyhow::anyhow!("UTF-8 decode failed: {}", e))
    }

    pub fn encrypt_hex(&self, plaintext: &str) -> Result<String> {
        let bytes = self.encrypt(plaintext)?;
        Ok(hex::encode(bytes))
    }

    pub fn decrypt_hex(&self, hex_str: &str) -> Result<String> {
        if hex_str.is_empty() {
            return Ok(String::new());
        }
        let bytes = hex::decode(hex_str)?;
        self.decrypt(&bytes)
    }
}

fn cleanup_failed_publication(
    publication_error: anyhow::Error,
    protected: &[u8],
    cleanup: impl FnOnce(&[u8]) -> Result<()>,
) -> anyhow::Error {
    match cleanup(protected) {
        Ok(()) => publication_error,
        Err(error) => anyhow::anyhow!(
            "Protected key publication failed: {publication_error}; credential cleanup failed: {error}"
        ),
    }
}

fn persist_protected_key(
    app_dir: &Path,
    path: &Path,
    protected: &[u8],
    no_clobber: bool,
) -> Result<()> {
    use std::io::Write;
    let mut file = tempfile::NamedTempFile::new_in(app_dir)?;
    file.write_all(PROTECTED_KEY_MAGIC)?;
    file.write_all(protected)?;
    file.as_file().sync_all()?;
    if no_clobber {
        file.persist_noclobber(path).map_err(|error| error.error)?;
    } else {
        file.persist(path).map_err(|error| error.error)?;
    }
    Ok(())
}

#[cfg(windows)]
fn protect_key(key: &[u8]) -> Result<Vec<u8>> {
    dpapi_transform(key, true)
}
#[cfg(windows)]
fn unprotect_key(blob: &[u8]) -> Result<Vec<u8>> {
    dpapi_transform(blob, false)
}
#[cfg(windows)]
fn dpapi_transform(input: &[u8], protect: bool) -> Result<Vec<u8>> {
    use windows::{
        core::PCWSTR,
        Win32::{
            Foundation::{LocalFree, HLOCAL},
            Security::Cryptography::{
                CryptProtectData, CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
            },
        },
    };
    struct Output(CRYPT_INTEGER_BLOB);
    impl Drop for Output {
        fn drop(&mut self) {
            if !self.0.pbData.is_null() {
                unsafe {
                    let _ = LocalFree(HLOCAL(self.0.pbData.cast()));
                }
            }
        }
    }
    let source = CRYPT_INTEGER_BLOB {
        cbData: input.len().try_into()?,
        pbData: input.as_ptr().cast_mut(),
    };
    let mut output = Output(CRYPT_INTEGER_BLOB::default());
    // No machine-wide flag: DPAPI binds the blob to the current Windows user.
    unsafe {
        let result = if protect {
            CryptProtectData(
                &source,
                PCWSTR::null(),
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.0,
            )
        } else {
            CryptUnprotectData(
                &source,
                None,
                None,
                None,
                None,
                CRYPTPROTECT_UI_FORBIDDEN,
                &mut output.0,
            )
        };
        result.map_err(|error| {
            anyhow::anyhow!("Windows credential protection unavailable: {error}")
        })?;
        if output.0.pbData.is_null() || output.0.cbData == 0 {
            anyhow::bail!("Windows credential protection returned an empty result");
        }
        Ok(std::slice::from_raw_parts(output.0.pbData, output.0.cbData as usize).to_vec())
    }
}

#[cfg(target_os = "macos")]
fn protect_key(key: &[u8]) -> Result<Vec<u8>> {
    super::macos_keychain::protect(key)
}
#[cfg(target_os = "macos")]
fn unprotect_key(reference: &[u8]) -> Result<Vec<u8>> {
    super::macos_keychain::read(reference)
}
#[cfg(target_os = "macos")]
fn cleanup_key(reference: &[u8]) -> Result<()> {
    super::macos_keychain::remove(reference)
}
#[cfg(target_os = "linux")]
fn protect_key(key: &[u8]) -> Result<Vec<u8>> {
    super::linux_keyring::protect(key)
}
#[cfg(target_os = "linux")]
fn unprotect_key(reference: &[u8]) -> Result<Vec<u8>> {
    super::linux_keyring::read(reference)
}
#[cfg(target_os = "linux")]
fn cleanup_key(reference: &[u8]) -> Result<()> {
    super::linux_keyring::remove(reference)
}
#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn cleanup_key(_: &[u8]) -> Result<()> {
    Ok(())
}

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn protect_key(_: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("System credential protection is not implemented for this platform")
}
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
fn unprotect_key(_: &[u8]) -> Result<Vec<u8>> {
    anyhow::bail!("Windows protected credentials cannot be opened on this platform")
}

fn random_bytes(bytes: &mut [u8]) -> Result<()> {
    getrandom::getrandom(bytes)
        .map_err(|error| anyhow::anyhow!("Secure random source failed: {}", error))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_foreign_platform_reference_is_rejected_without_backend_calls() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(KEY_FILE);
        let foreign = if cfg!(target_os = "macos") {
            b"AGENTZ-KEY-V1:DPAPI\0foreign-protection-data".as_slice()
        } else {
            b"AGENTZ-KEY-V1:KEYCHAIN\0f3bb9c24-d87a-41cb-907b-87c3dccd0910".as_slice()
        };
        std::fs::write(&path, foreign).unwrap();
        let result = SecretStore::new_with_protection(
            directory.path(),
            |_| unreachable!("must not generate a replacement key"),
            |_| unreachable!("must not migrate a foreign platform reference"),
            |_| unreachable!("must not call current platform backend"),
        );
        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), foreign);
    }

    #[test]
    fn secrets_failed_reference_publication_cleans_created_credential() {
        let directory = tempfile::tempdir().unwrap();
        let key_path = directory.path().join(KEY_FILE);
        std::fs::write(&key_path, [37u8; 32]).unwrap();
        let cleaned = std::cell::Cell::new(false);
        let result = SecretStore::new_with_protection_and_cleanup(
            directory.path(),
            random_bytes,
            |_| {
                // Inject publication failure after external credential creation.
                std::fs::remove_file(&key_path)?;
                std::fs::create_dir(&key_path)?;
                Ok(b"temporary-reference".to_vec())
            },
            |_| unreachable!(),
            |reference| {
                assert_eq!(reference, b"temporary-reference");
                cleaned.set(true);
                Ok(())
            },
        );
        assert!(result.is_err());
        assert!(cleaned.get());
        assert!(key_path.is_dir());
    }

    #[test]
    fn secrets_failed_credential_cleanup_is_reported() {
        let error = cleanup_failed_publication(
            anyhow::anyhow!("injected publish failure"),
            b"temporary-reference",
            |_| anyhow::bail!("injected cleanup failure"),
        );
        let message = error.to_string();
        assert!(message.contains("injected publish failure"));
        assert!(message.contains("injected cleanup failure"));
    }

    #[test]
    fn secrets_protection_failure_preserves_legacy_key() {
        let directory = tempfile::tempdir().unwrap();
        let key = [73u8; 32];
        let key_path = directory.path().join(KEY_FILE);
        std::fs::write(&key_path, key).unwrap();
        assert!(SecretStore::new_with_protection(
            directory.path(),
            random_bytes,
            |_| anyhow::bail!("Injected protection failure"),
            |_| unreachable!()
        )
        .is_err());
        assert_eq!(std::fs::read(&key_path).unwrap(), key);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        std::fs::remove_file(&key_path).unwrap();
        assert!(SecretStore::new_with_protection(
            directory.path(),
            random_bytes,
            |_| anyhow::bail!("Injected protection failure"),
            |_| unreachable!()
        )
        .is_err());
        assert!(!key_path.exists());
    }

    #[cfg(windows)]
    #[test]
    fn secrets_dpapi_migration_preserves_ciphertext_and_reopens() {
        let directory = tempfile::tempdir().unwrap();
        let key = [61u8; 32];
        let ciphertext = SecretStore { key }
            .encrypt("fixture-migrated-secret")
            .unwrap();
        let key_path = directory.path().join(KEY_FILE);
        std::fs::write(&key_path, key).unwrap();
        let migrated = SecretStore::new(directory.path()).unwrap();
        let bytes = std::fs::read(&key_path).unwrap();
        assert!(bytes.starts_with(PROTECTED_KEY_MAGIC));
        assert!(!bytes.windows(key.len()).any(|window| window == key));
        assert_eq!(
            migrated.decrypt(&ciphertext).unwrap(),
            "fixture-migrated-secret"
        );
        assert_eq!(
            SecretStore::new(directory.path())
                .unwrap()
                .decrypt(&ciphertext)
                .unwrap(),
            "fixture-migrated-secret"
        );
        let mut damaged = bytes.clone();
        damaged.truncate(PROTECTED_KEY_MAGIC.len() + 3);
        std::fs::write(&key_path, &damaged).unwrap();
        assert!(SecretStore::new(directory.path()).is_err());
        assert_eq!(std::fs::read(&key_path).unwrap(), damaged);
        std::fs::write(&key_path, bytes).unwrap();
        assert_eq!(
            SecretStore::new(directory.path())
                .unwrap()
                .decrypt(&ciphertext)
                .unwrap(),
            "fixture-migrated-secret"
        );
    }

    #[cfg(windows)]
    #[test]
    fn secrets_locked_legacy_key_migration_preserves_original() {
        use std::os::windows::fs::OpenOptionsExt;
        let directory = tempfile::tempdir().unwrap();
        let key_path = directory.path().join(KEY_FILE);
        let key = [42u8; 32];
        std::fs::write(&key_path, key).unwrap();
        let lock = std::fs::OpenOptions::new()
            .read(true)
            .share_mode(1)
            .open(&key_path)
            .unwrap();
        assert!(SecretStore::new(directory.path()).is_err());
        assert_eq!(std::fs::read(&key_path).unwrap(), key);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
        drop(lock);
        assert!(SecretStore::new(directory.path()).is_ok());
    }

    #[test]
    fn secrets_random_failure_publishes_neither_key_nor_ciphertext() {
        let directory = tempfile::tempdir().unwrap();
        assert!(SecretStore::new_with_random(directory.path(), |_| {
            anyhow::bail!("Injected key random-source failure")
        })
        .is_err());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        let store = SecretStore::new(directory.path()).unwrap();
        assert!(store
            .encrypt_with_random("fixture-secret", |_| {
                anyhow::bail!("Injected nonce random-source failure")
            })
            .is_err());
    }

    #[test]
    fn secrets_existing_key_is_never_replaced() {
        let directory = tempfile::tempdir().unwrap();
        let store = SecretStore::new(directory.path()).unwrap();
        let key = std::fs::read(directory.path().join(KEY_FILE)).unwrap();
        let other = SecretStore::new(directory.path()).unwrap();
        assert_eq!(std::fs::read(directory.path().join(KEY_FILE)).unwrap(), key);
        assert_eq!(
            other
                .decrypt(&store.encrypt("fixture-secret").unwrap())
                .unwrap(),
            "fixture-secret"
        );
    }
}
