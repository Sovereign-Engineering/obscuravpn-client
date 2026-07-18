use std::time::Instant;

use static_assertions::const_assert;
use windows::Win32::Foundation::NTE_BAD_KEYSET;
use windows::Win32::Security::Cryptography::{
    BCRYPT_OAEP_PADDING_INFO, BCRYPT_SHA256_ALGORITHM, CERT_KEY_SPEC, MS_PLATFORM_CRYPTO_PROVIDER, NCRYPT_HANDLE, NCRYPT_KEY_HANDLE,
    NCRYPT_LENGTH_PROPERTY, NCRYPT_MACHINE_KEY_FLAG, NCRYPT_OVERWRITE_KEY_FLAG, NCRYPT_PAD_OAEP_FLAG, NCRYPT_PCP_PLATFORM_BINDING_PCRALGID_PROPERTY,
    NCRYPT_PCP_PLATFORM_BINDING_PCRMASK_PROPERTY, NCRYPT_PROV_HANDLE, NCRYPT_RSA_ALGORITHM, NCRYPT_SILENT_FLAG, NCryptCreatePersistedKey,
    NCryptDecrypt, NCryptDeleteKey, NCryptEncrypt, NCryptFinalizeKey, NCryptOpenKey, NCryptOpenStorageProvider, NCryptSetProperty,
};
use windows::core::{Owned, PCWSTR, w};

use super::{PlaintextWgSecretKey, SealedWgSecretKey};

pub struct WindowsSealingKey {
    handle: Owned<NCRYPT_KEY_HANDLE>,
    _provider: Owned<NCRYPT_PROV_HANDLE>,
}

impl WindowsSealingKey {
    /// Opens the TPM sealing key or creates it if missing or not decryptable (e.g. stale PCR binding).
    pub(crate) async fn open() -> Result<Self, ()> {
        tokio::task::spawn_blocking(|| {
            const SEALING_KEY_NAME: PCWSTR = w!("ObscuraVPN-wg-key-sealing-v1");
            let started_at = Instant::now();
            let mut provider = NCRYPT_PROV_HANDLE::default();
            // SAFETY: the provider out-pointer is valid for the duration of the call.
            unsafe { NCryptOpenStorageProvider(&mut provider, MS_PLATFORM_CRYPTO_PROVIDER, 0) }
                .map_err(|error| tracing::error!(?error, message_id = "Jy4bMh9W", "NCryptOpenStorageProvider failed"))?;
            // SAFETY: the handle was just opened and is exclusively owned here.
            let provider = unsafe { Owned::new(provider) };
            let handle = match open_key(*provider, SEALING_KEY_NAME)? {
                Some(key) => key,
                None => create_key(*provider, SEALING_KEY_NAME)?,
            };
            let sealing_key = Self { handle, _provider: provider };
            tracing::info!(message_id = "uY7dNb3Q", duration =? started_at.elapsed(), "TPM sealing key ready");
            Ok(sealing_key)
        })
        .await
        .map_err(|error| tracing::error!(?error, message_id = "Xw6tNc2H", "sealing key task failed"))?
    }

    pub(crate) fn seal(&self, secret_key: &PlaintextWgSecretKey) -> Result<SealedWgSecretKey, ()> {
        Ok(SealedWgSecretKey::PcpRsaOaep { ciphertext: encrypt(*self.handle, &secret_key.0)? })
    }

    pub(crate) fn unseal(&self, sealed_secret_key: &SealedWgSecretKey) -> Result<PlaintextWgSecretKey, ()> {
        let SealedWgSecretKey::PcpRsaOaep { ciphertext } = sealed_secret_key else {
            let sealing_type: &'static str = sealed_secret_key.into();
            tracing::error!(
                sealing_type,
                message_id = "Rp4vGx7K",
                "sealed wireguard secret key type is not supported on windows"
            );
            return Err(());
        };
        let secret_key = decrypt(*self.handle, ciphertext)?;
        tracing::info!(message_id = "Jw8pRd2M", "unsealed wireguard secret key");
        Ok(PlaintextWgSecretKey::new(secret_key))
    }
}

fn u32_to_usize(len: u32) -> usize {
    const_assert!(size_of::<usize>() >= size_of::<u32>());
    len as usize
}

fn open_key(provider: NCRYPT_PROV_HANDLE, name: PCWSTR) -> Result<Option<Owned<NCRYPT_KEY_HANDLE>>, ()> {
    let mut key = NCRYPT_KEY_HANDLE::default();
    // SAFETY: the provider handle is open and the key out-pointer is valid for the call.
    match unsafe { NCryptOpenKey(provider, &mut key, name, CERT_KEY_SPEC(0), NCRYPT_MACHINE_KEY_FLAG | NCRYPT_SILENT_FLAG) } {
        Ok(()) => {}
        Err(error) if error.code() == NTE_BAD_KEYSET => return Ok(None),
        Err(error) => {
            tracing::error!(?error, message_id = "Ls6qDn2F", "NCryptOpenKey failed");
            return Err(());
        }
    }
    // SAFETY: the handle was just opened and is exclusively owned here.
    let key = unsafe { Owned::new(key) };
    // A key with a stale PCR binding (e.g. changed Secure Boot state) still opens and encrypts, only decryption fails, so a decrypt round-trip is the only way to detect that it needs recreation.
    let plaintext = [0u8; 32];
    if encrypt(*key, &plaintext).and_then(|ciphertext| decrypt(*key, &ciphertext)) != Ok(plaintext) {
        tracing::info!(message_id = "qX2jUa8S", "TPM sealing key failed the decrypt probe, recreating");
        delete_key(key)?;
        return Ok(None);
    }
    Ok(Some(key))
}

fn create_key(provider: NCRYPT_PROV_HANDLE, name: PCWSTR) -> Result<Owned<NCRYPT_KEY_HANDLE>, ()> {
    let mut key = NCRYPT_KEY_HANDLE::default();
    // SAFETY: the provider handle is open and the key out-pointer is valid for the call.
    unsafe {
        NCryptCreatePersistedKey(
            provider,
            &mut key,
            NCRYPT_RSA_ALGORITHM,
            name,
            CERT_KEY_SPEC(0),
            NCRYPT_MACHINE_KEY_FLAG | NCRYPT_SILENT_FLAG | NCRYPT_OVERWRITE_KEY_FLAG,
        )
    }
    .map_err(|error| tracing::error!(?error, message_id = "Vg3rTk8P", "NCryptCreatePersistedKey failed"))?;
    // SAFETY: the handle was just created and is exclusively owned here.
    let key = unsafe { Owned::new(key) };
    // SAFETY: the key handle is valid and the property value outlives the call.
    unsafe {
        NCryptSetProperty(
            NCRYPT_HANDLE::from(*key),
            NCRYPT_LENGTH_PROPERTY,
            &2048u32.to_le_bytes(),
            NCRYPT_SILENT_FLAG,
        )
    }
    .map_err(|error| tracing::error!(?error, message_id = "Bw7mFx4N", "NCryptSetProperty failed for the key length"))?;
    // SAFETY: the key handle is valid and the property value outlives the call.
    unsafe {
        NCryptSetProperty(
            NCRYPT_HANDLE::from(*key),
            NCRYPT_PCP_PLATFORM_BINDING_PCRMASK_PROPERTY,
            // The mask selects PCR 7, the firmware's measurement of the Secure Boot configuration (SecureBoot flag, PK/KEK/db/dbx). Binding the key to it blocks decryption from another OS booted with modified Secure Boot state.
            &[0x80, 0x00, 0x00],
            NCRYPT_SILENT_FLAG,
        )
    }
    .map_err(|error| tracing::error!(?error, message_id = "Yk5wBn3T", "NCryptSetProperty failed for the pcr mask"))?;
    // SAFETY: the key handle is valid and the property value outlives the call.
    unsafe {
        NCryptSetProperty(
            NCRYPT_HANDLE::from(*key),
            NCRYPT_PCP_PLATFORM_BINDING_PCRALGID_PROPERTY,
            // TPM_ALG_SHA256
            &[0x0B, 0x00],
            NCRYPT_SILENT_FLAG,
        )
    }
    .map_err(|error| tracing::error!(?error, message_id = "Hf8qLm2X", "NCryptSetProperty failed for the pcr algorithm"))?;
    let started_at = Instant::now();
    // SAFETY: the key handle is valid.
    unsafe { NCryptFinalizeKey(*key, NCRYPT_SILENT_FLAG) }
        .map_err(|error| tracing::error!(?error, message_id = "Cy4tWb6M", "NCryptFinalizeKey failed"))?;
    tracing::info!(message_id = "gD7nVe1P", duration =? started_at.elapsed(), "created TPM sealing key");
    Ok(key)
}

fn delete_key(key: Owned<NCRYPT_KEY_HANDLE>) -> Result<(), ()> {
    let raw_key = *key;
    std::mem::forget(key);
    // SAFETY: the handle is valid and exclusively owned; NCryptDeleteKey frees it.
    unsafe { NCryptDeleteKey(raw_key, NCRYPT_SILENT_FLAG.0) }
        .map_err(|error| tracing::error!(?error, message_id = "Mv2fKz8C", "NCryptDeleteKey failed"))
}

const OAEP_PADDING: BCRYPT_OAEP_PADDING_INFO =
    BCRYPT_OAEP_PADDING_INFO { pszAlgId: BCRYPT_SHA256_ALGORITHM, pbLabel: std::ptr::null_mut(), cbLabel: 0 };

fn encrypt(key: NCRYPT_KEY_HANDLE, plaintext: &[u8; 32]) -> Result<Vec<u8>, ()> {
    let padding_ptr = Some(std::ptr::from_ref(&OAEP_PADDING).cast::<core::ffi::c_void>());
    let flags = NCRYPT_PAD_OAEP_FLAG | NCRYPT_SILENT_FLAG;
    let mut len = 0u32;
    // SAFETY: the key handle is valid and padding and all buffers outlive the call.
    unsafe { NCryptEncrypt(key, Some(plaintext.as_slice()), padding_ptr, None, &mut len, flags) }
        .map_err(|error| tracing::error!(?error, message_id = "Ws6hBn4J", "NCryptEncrypt failed to report the ciphertext length"))?;
    let mut ciphertext = vec![0u8; u32_to_usize(len)];
    // SAFETY: the key handle is valid and padding and all buffers outlive the call.
    unsafe { NCryptEncrypt(key, Some(plaintext.as_slice()), padding_ptr, Some(&mut ciphertext), &mut len, flags) }
        .map_err(|error| tracing::error!(?error, message_id = "Ty8jQd2X", "NCryptEncrypt failed"))?;
    ciphertext.truncate(u32_to_usize(len));
    Ok(ciphertext)
}

fn decrypt(key: NCRYPT_KEY_HANDLE, ciphertext: &[u8]) -> Result<[u8; 32], ()> {
    let padding_ptr = Some(std::ptr::from_ref(&OAEP_PADDING).cast::<core::ffi::c_void>());
    let flags = NCRYPT_PAD_OAEP_FLAG | NCRYPT_SILENT_FLAG;
    let mut len = 0u32;
    // SAFETY: the key handle is valid and padding and all buffers outlive the call.
    unsafe { NCryptDecrypt(key, Some(ciphertext), padding_ptr, None, &mut len, flags) }
        .map_err(|error| tracing::error!(?error, message_id = "Gp3lRc7K", "NCryptDecrypt failed to report the plaintext length"))?;
    let mut plaintext = vec![0u8; u32_to_usize(len)];
    // SAFETY: the key handle is valid and padding and all buffers outlive the call.
    unsafe { NCryptDecrypt(key, Some(ciphertext), padding_ptr, Some(&mut plaintext), &mut len, flags) }
        .map_err(|error| tracing::error!(?error, message_id = "Ax9vMt5B", "NCryptDecrypt failed"))?;
    plaintext.truncate(u32_to_usize(len));
    <[u8; 32]>::try_from(plaintext.as_slice()).map_err(|_| {
        tracing::error!(
            length = plaintext.len(),
            message_id = "Ef4wZn6H",
            "unsealed wireguard secret key has an unexpected length"
        );
    })
}
