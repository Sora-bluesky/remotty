use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow};
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};
use windows::core::PCWSTR;

pub fn load_secret(key: &str) -> Result<String> {
    let path = secret_path(key)?;
    let encrypted = fs::read(&path)
        .with_context(|| format!("failed to read protected secret: {}", path.display()))?;
    let decrypted = unprotect_bytes(&encrypted)?;
    String::from_utf8(decrypted).context("secret is not valid UTF-8")
}

pub fn store_secret(key: &str, value: &str) -> Result<PathBuf> {
    let path = secret_path(key)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create secret directory: {}", parent.display()))?;
    }
    let encrypted = protect_bytes(value.as_bytes())?;
    fs::write(&path, encrypted)
        .with_context(|| format!("failed to write protected secret: {}", path.display()))?;
    Ok(path)
}

pub fn delete_secret(key: &str) -> Result<()> {
    let path = secret_path(key)?;
    if path.exists() {
        fs::remove_file(&path)
            .with_context(|| format!("failed to remove protected secret: {}", path.display()))?;
    }
    Ok(())
}

fn secret_path(key: &str) -> Result<PathBuf> {
    if key.trim().is_empty() {
        return Err(anyhow!("secret key must not be empty"));
    }
    let base = std::env::var("LOCALAPPDATA")
        .context("LOCALAPPDATA is not set; cannot resolve secret store path")?;
    Ok(PathBuf::from(base)
        .join("codex-telegram-bridge")
        .join("secrets")
        .join(format!("{key}.bin")))
}

fn protect_bytes(input: &[u8]) -> Result<Vec<u8>> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptProtectData(
            &in_blob,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .context("CryptProtectData failed")?;
    }

    blob_to_vec_and_free(out_blob)
}

fn unprotect_bytes(input: &[u8]) -> Result<Vec<u8>> {
    let in_blob = CRYPT_INTEGER_BLOB {
        cbData: input.len() as u32,
        pbData: input.as_ptr() as *mut u8,
    };
    let mut out_blob = CRYPT_INTEGER_BLOB::default();

    unsafe {
        CryptUnprotectData(
            &in_blob,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut out_blob,
        )
        .context("CryptUnprotectData failed")?;
    }

    blob_to_vec_and_free(out_blob)
}

fn blob_to_vec_and_free(blob: CRYPT_INTEGER_BLOB) -> Result<Vec<u8>> {
    if blob.pbData.is_null() || blob.cbData == 0 {
        return Ok(Vec::new());
    }

    let data = unsafe { std::slice::from_raw_parts(blob.pbData, blob.cbData as usize).to_vec() };
    unsafe {
        let _ = LocalFree(Some(HLOCAL(blob.pbData as *mut _)));
    }
    Ok(data)
}
