//! Local Keychain references; this module never passes key material to a subprocess.
use anyhow::{ensure, Result};
use std::ffi::c_void;
use std::ptr;

type Ref = *const c_void;
const SERVICE: &str = "com.agentz.desktop.master-key.v1";
// errSecItemNotFound: cleanup may be retried after an earlier successful delete.
const ITEM_NOT_FOUND: i32 = -25300;

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(value: Ref);
    fn CFStringCreateWithBytes(a: Ref, bytes: *const u8, len: isize, enc: u32, external: u8)
        -> Ref;
    fn CFDataCreate(a: Ref, bytes: *const u8, len: isize) -> Ref;
    fn CFDataGetLength(data: Ref) -> isize;
    fn CFDataGetBytePtr(data: Ref) -> *const u8;
    fn CFGetTypeID(value: Ref) -> usize;
    fn CFDataGetTypeID() -> usize;
    fn CFDictionaryCreate(
        a: Ref,
        keys: *const Ref,
        values: *const Ref,
        count: isize,
        k: Ref,
        v: Ref,
    ) -> Ref;
    static kCFBooleanFalse: Ref;
    static kCFBooleanTrue: Ref;
}

#[link(name = "Security", kind = "framework")]
extern "C" {
    fn SecItemAdd(query: Ref, result: *mut Ref) -> i32;
    fn SecItemCopyMatching(query: Ref, result: *mut Ref) -> i32;
    fn SecItemDelete(query: Ref) -> i32;
    static kSecClass: Ref;
    static kSecClassGenericPassword: Ref;
    static kSecAttrService: Ref;
    static kSecAttrAccount: Ref;
    static kSecAttrSynchronizable: Ref;
    static kSecUseAuthenticationUI: Ref;
    static kSecUseAuthenticationUIFail: Ref;
    static kSecValueData: Ref;
    static kSecReturnData: Ref;
}

struct Owned(Ref);
impl Owned {
    fn checked(value: Ref) -> Result<Self> {
        ensure!(!value.is_null(), "Keychain query allocation failed");
        Ok(Self(value))
    }
    fn string(value: &str) -> Result<Self> {
        // kCFStringEncodingUTF8. The copied string outlives every query using it.
        Self::checked(unsafe {
            CFStringCreateWithBytes(
                ptr::null(),
                value.as_ptr(),
                value.len().try_into()?,
                0x08000100,
                0,
            )
        })
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.0) };
    }
}

enum Operation<'a> {
    Add(&'a [u8]),
    Read,
    Delete,
}

fn perform(account: &str, operation: Operation<'_>) -> Result<Option<Vec<u8>>> {
    let service = Owned::string(SERVICE)?;
    let account = Owned::string(account)?;
    let mut keys = unsafe {
        vec![
            kSecClass,
            kSecAttrService,
            kSecAttrAccount,
            kSecAttrSynchronizable,
            kSecUseAuthenticationUI,
        ]
    };
    let mut values = unsafe {
        vec![
            kSecClassGenericPassword,
            service.0,
            account.0,
            kCFBooleanFalse,
            kSecUseAuthenticationUIFail,
        ]
    };
    let data = if let Operation::Add(key) = &operation {
        Some(Owned::checked(unsafe {
            CFDataCreate(ptr::null(), key.as_ptr(), key.len().try_into()?)
        })?)
    } else {
        None
    };
    if let Some(data) = &data {
        keys.push(unsafe { kSecValueData });
        values.push(data.0);
    }
    if matches!(operation, Operation::Read) {
        keys.push(unsafe { kSecReturnData });
        values.push(unsafe { kCFBooleanTrue });
    }
    // Null callbacks intentionally borrow keys/values. All referenced objects remain
    // alive until the synchronous Security call completes, and query drops first.
    let query = Owned::checked(unsafe {
        CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            keys.len().try_into()?,
            ptr::null(),
            ptr::null(),
        )
    })?;
    let mut result = ptr::null();
    let status = unsafe {
        match operation {
            Operation::Add(_) => SecItemAdd(query.0, ptr::null_mut()),
            Operation::Read => SecItemCopyMatching(query.0, &mut result),
            Operation::Delete => SecItemDelete(query.0),
        }
    };
    // Preserve ownership even if a failing OS call unexpectedly returned an object.
    let result = if result.is_null() {
        None
    } else {
        Some(Owned(result))
    };
    ensure!(
        status == 0 || (matches!(operation, Operation::Delete) && status == ITEM_NOT_FOUND),
        "macOS credential protection unavailable (OSStatus {status})"
    );
    if let Some(result) = result {
        ensure!(
            unsafe { CFGetTypeID(result.0) == CFDataGetTypeID() },
            "Keychain returned unexpected value type"
        );
        let len = unsafe { CFDataGetLength(result.0) };
        ensure!(len == 32, "Invalid Keychain master key length");
        let bytes = unsafe { CFDataGetBytePtr(result.0) };
        ensure!(!bytes.is_null(), "Keychain returned empty data");
        return Ok(Some(
            unsafe { std::slice::from_raw_parts(bytes, 32) }.to_vec(),
        ));
    }
    Ok(None)
}

fn account(reference: &[u8]) -> Result<&str> {
    let value = std::str::from_utf8(reference)?;
    let id = uuid::Uuid::parse_str(value)?;
    ensure!(
        id.hyphenated().to_string() == value,
        "Invalid Keychain reference format"
    );
    Ok(value)
}

pub(super) fn protect(key: &[u8]) -> Result<Vec<u8>> {
    ensure!(key.len() == 32, "Invalid master key length");
    let reference = uuid::Uuid::new_v4().hyphenated().to_string().into_bytes();
    perform(account(&reference)?, Operation::Add(key))?;
    let verified = read(&reference).and_then(|saved| {
        ensure!(saved == key, "Keychain master key readback mismatch");
        Ok(())
    });
    if let Err(error) = verified {
        if let Err(cleanup) = remove(&reference) {
            anyhow::bail!(
                "Keychain verification failed: {error}; cleanup failed for {}: {cleanup}",
                account(&reference)?
            );
        }
        return Err(error);
    }
    Ok(reference)
}

pub(super) fn read(reference: &[u8]) -> Result<Vec<u8>> {
    perform(account(reference)?, Operation::Read)?
        .ok_or_else(|| anyhow::anyhow!("Keychain master key missing"))
}

pub(super) fn remove(reference: &[u8]) -> Result<()> {
    let identity = account(reference)?;
    perform(identity, Operation::Delete)
        .map_err(|error| anyhow::anyhow!("Keychain cleanup failed for {identity}: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "Requires an unlocked macOS Keychain; creates and removes temporary test credentials"]
    fn macos_keychain_reference_roundtrip_revocation_and_migration() -> Result<()> {
        struct Cleanup(Vec<u8>);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                if !self.0.is_empty() {
                    if let Err(error) = remove(&self.0) {
                        eprintln!("Temporary Keychain test credential cleanup failed: {error}");
                    }
                }
            }
        }
        let key = [47u8; 32];
        let reference = protect(&key)?;
        let mut cleanup = Cleanup(reference.clone());
        ensure!(read(&reference)? == key);
        ensure!(read(b"invalid-reference").is_err());
        remove(&reference)?;
        remove(&reference)?;
        cleanup.0.clear();
        ensure!(
            read(&reference).is_err(),
            "Revoked credential was still readable"
        );

        let directory = tempfile::tempdir()?;
        let path = directory.path().join(".secret_key");
        std::fs::write(&path, key)?;
        let migrated = super::super::secrets::SecretStore::new(directory.path())?;
        let data = std::fs::read(&path)?;
        let reference = data
            .strip_prefix(b"AGENTZ-KEY-V1:KEYCHAIN\0")
            .ok_or_else(|| anyhow::anyhow!("Missing Keychain reference header"))?;
        let mut cleanup = Cleanup(reference.to_vec());
        ensure!(
            read(reference)? == key,
            "Legacy key changed during migration"
        );
        let ciphertext = migrated.encrypt_hex("temporary-fake-credential")?;
        let reopened = super::super::secrets::SecretStore::new(directory.path())?;
        ensure!(reopened.decrypt_hex(&ciphertext)? == "temporary-fake-credential");
        remove(reference)?;
        cleanup.0.clear();
        let before = std::fs::read(&path)?;
        ensure!(super::super::secrets::SecretStore::new(directory.path()).is_err());
        ensure!(
            std::fs::read(&path)? == before,
            "Revocation replaced the key reference"
        );
        Ok(())
    }
}
