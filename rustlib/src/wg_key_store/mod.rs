//! Where the WireGuard secret key is persisted.
//!
//! [`WgKeyStore::Sealed`] seals the key with a TPM-resident key that is bound to PCR 7
//! (Secure Boot state), so sealed blobs only unseal under the same boot trust path on
//! the same machine. Sealing is only supported on Windows and Linux.

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

use obscuravpn_api::types::WgPubkey;
use serde::{Deserialize, Serialize};
use x25519_dalek::{PublicKey, StaticSecret};

#[cfg(target_os = "linux")]
pub use linux::LinuxSealingKey as SealingKey;
#[cfg(target_os = "windows")]
pub use windows::WindowsSealingKey as SealingKey;

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub enum SealingKey {}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
impl SealingKey {
    pub(crate) async fn open() -> Result<Self, ()> {
        tracing::error!(message_id = "mW4qJn8D", "TPM sealing is only supported on windows and linux");
        Err(())
    }

    pub(crate) fn seal(&self, _secret_key: &PlaintextWgSecretKey) -> Result<SealedWgSecretKey, ()> {
        match *self {}
    }

    pub(crate) fn unseal(&self, _sealed_secret_key: &SealedWgSecretKey) -> Result<PlaintextWgSecretKey, ()> {
        match *self {}
    }
}

#[serde_with::serde_as]
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize, zeroize::Zeroize, zeroize::ZeroizeOnDrop)]
pub struct PlaintextWgSecretKey(#[serde_as(as = "serde_with::base64::Base64")] [u8; 32]);

impl PlaintextWgSecretKey {
    pub fn new(secret_key: [u8; 32]) -> Self {
        Self(secret_key)
    }

    pub fn static_secret(&self) -> StaticSecret {
        StaticSecret::from(self.0)
    }

    pub fn public_key(&self) -> WgPubkey {
        WgPubkey(PublicKey::from(&self.static_secret()).to_bytes())
    }
}

impl std::fmt::Debug for PlaintextWgSecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PlaintextWgSecretKey(redacted)")
    }
}

#[serde_with::serde_as]
#[derive(Clone, Deserialize, Eq, PartialEq, Serialize, strum::IntoStaticStr)]
#[serde(tag = "type", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum SealedWgSecretKey {
    Tpm2SealedObject {
        #[serde_as(as = "serde_with::base64::Base64")]
        private: Vec<u8>,
        #[serde_as(as = "serde_with::base64::Base64")]
        public: Vec<u8>,
    },
    PcpRsaOaep {
        #[serde_as(as = "serde_with::base64::Base64")]
        ciphertext: Vec<u8>,
    },
}

pub type KeychainSetSecretKeyFn = Box<dyn (Fn(&[u8; 32]) -> bool) + Sync + Send>;

#[derive(strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub enum WgKeyStore {
    Plaintext,
    Keychain {
        secret_key: Option<Vec<u8>>,
        set_secret_key: KeychainSetSecretKeyFn,
    },
    Sealed(SealingKey),
    None,
}

impl WgKeyStore {
    pub async fn sealed() -> Result<Self, ()> {
        Ok(Self::Sealed(SealingKey::open().await?))
    }
}
