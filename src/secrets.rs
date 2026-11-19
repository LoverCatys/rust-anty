//! Local encryption for sensitive profile fields (proxy passwords) stored in `profiliki.json`.
//!
//! On Windows, uses native DPAPI (`CryptProtectData` / `CryptUnprotectData`), binding
//! ciphertext to the current Windows user account without any hardcoded keys on disk.
//! Plaintext values (e.g. legacy `profiliki.json` entries) are transparently read as-is
//! and upgraded to `dpapi:<base64>` on the next save.

const DPAPI_PREFIX: &str = "dpapi:";

pub fn encrypt_secret(plain: &str) -> String {
    if plain.is_empty() || plain.starts_with(DPAPI_PREFIX) {
        return plain.to_string();
    }
    #[cfg(windows)]
    {
        if let Some(blob) = dpapi_protect(plain.as_bytes()) {
            use base64::Engine;
            let b64 = base64::engine::general_purpose::STANDARD.encode(blob);
            return format!("{DPAPI_PREFIX}{b64}");
        }
    }
    plain.to_string()
}

pub fn decrypt_secret(stored: &str) -> String {
    let Some(b64) = stored.strip_prefix(DPAPI_PREFIX) else {
        return stored.to_string();
    };
    #[cfg(windows)]
    {
        use base64::Engine;
        if let Ok(raw) = base64::engine::general_purpose::STANDARD.decode(b64) {
            if let Some(plain_bytes) = dpapi_unprotect(&raw) {
                if let Ok(text) = String::from_utf8(plain_bytes) {
                    return text;
                }
            }
        }
    }
    stored.to_string()
}

pub fn is_encrypted(stored: &str) -> bool {
    stored.starts_with(DPAPI_PREFIX)
}

#[cfg(windows)]
fn dpapi_protect(data: &[u8]) -> Option<Vec<u8>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptProtectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).ok()?,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptProtectData(
            &mut in_blob,
            null(),
            null(),
            null_mut(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };
    if ok == 0 || out_blob.pbData.is_null() {
        return None;
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(out_blob.pbData as *mut _);
    }
    Some(bytes)
}

#[cfg(windows)]
fn dpapi_unprotect(data: &[u8]) -> Option<Vec<u8>> {
    use std::ptr::{null, null_mut};
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Cryptography::{
        CryptUnprotectData, CRYPTPROTECT_UI_FORBIDDEN, CRYPT_INTEGER_BLOB,
    };

    let mut in_blob = CRYPT_INTEGER_BLOB {
        cbData: u32::try_from(data.len()).ok()?,
        pbData: data.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB {
        cbData: 0,
        pbData: null_mut(),
    };

    let ok = unsafe {
        CryptUnprotectData(
            &mut in_blob,
            null_mut(),
            null(),
            null_mut(),
            null(),
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
    };
    if ok == 0 || out_blob.pbData.is_null() {
        return None;
    }

    let bytes =
        unsafe { std::slice::from_raw_parts(out_blob.pbData, out_blob.cbData as usize) }.to_vec();
    unsafe {
        LocalFree(out_blob.pbData as *mut _);
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpapi_secret_roundtrip() {
        let plain = "s3cr3t_Pr0xy_P@ss!";
        let enc = encrypt_secret(plain);
        #[cfg(windows)]
        {
            assert!(is_encrypted(&enc));
            assert!(!enc.contains(plain));
        }
        assert_eq!(decrypt_secret(&enc), plain);
        // Legacy plaintext passthrough
        assert_eq!(decrypt_secret("legacy_plain"), "legacy_plain");
    }
}
