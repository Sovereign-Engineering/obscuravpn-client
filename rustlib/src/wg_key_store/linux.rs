use std::str::FromStr;
use std::sync::Mutex;
use std::time::Instant;

use tss_esapi::attributes::ObjectAttributesBuilder;
use tss_esapi::constants::SessionType;
use tss_esapi::handles::{KeyHandle, SessionHandle};
use tss_esapi::interface_types::algorithm::{HashingAlgorithm, PublicAlgorithm};
use tss_esapi::interface_types::ecc::EccCurve;
use tss_esapi::interface_types::resource_handles::Hierarchy;
use tss_esapi::interface_types::session_handles::{AuthSession, PolicySession};
use tss_esapi::structures::{
    Digest, EccPoint, KeyedHashScheme, PcrSelectionList, PcrSelectionListBuilder, PcrSlot, Private, Public, PublicBuilder,
    PublicEccParametersBuilder, PublicKeyedHashParameters, SensitiveData, SymmetricDefinition, SymmetricDefinitionObject,
};
use tss_esapi::tcti_ldr::{DeviceConfig, TctiNameConf};
use tss_esapi::traits::{Marshall, UnMarshall};
use tss_esapi::{Context, Error, WrapperErrorKind};

use super::{PlaintextWgSecretKey, SealedWgSecretKey};

const TPM_DEVICE: &str = "/dev/tpmrm0";

pub struct LinuxSealingKey {
    context: Mutex<Context>,
    primary: KeyHandle,
}

impl LinuxSealingKey {
    /// Opens the TPM and recreates the storage primary key that sealed secrets are wrapped under.
    pub(crate) async fn open() -> Result<Self, ()> {
        tokio::task::spawn_blocking(|| {
            let started_at = Instant::now();
            let device_config = DeviceConfig::from_str(TPM_DEVICE)
                .map_err(|error| tracing::error!(?error, TPM_DEVICE, message_id = "cJ8nWf4T", "invalid TPM device path"))?;
            let mut context = Context::new(TctiNameConf::Device(device_config))
                .map_err(|error| tracing::error!(?error, TPM_DEVICE, message_id = "rB3zGd7Y", "failed to open the TPM"))?;
            let primary = context
                .execute_with_nullauth_session(|context| context.create_primary(Hierarchy::Owner, storage_primary_public()?, None, None, None, None))
                .map_err(|error| tracing::error!(?error, message_id = "pV5xKm2Q", "TPM2_CreatePrimary failed"))?
                .key_handle;
            tracing::info!(message_id = "eK4mVp9S", duration =? started_at.elapsed(), "TPM sealing key ready");
            Ok(Self { context: Mutex::new(context), primary })
        })
        .await
        .map_err(|error| tracing::error!(?error, message_id = "Fz7kQd4W", "sealing key task failed"))?
    }

    pub(crate) fn seal(&self, secret_key: &PlaintextWgSecretKey) -> Result<SealedWgSecretKey, ()> {
        let mut context = self.context.lock().unwrap();
        let public = sealed_object_public(&mut context)
            .map_err(|error| tracing::error!(?error, message_id = "mJ5wRc8X", "failed to build the sealed object template"))?;
        let sensitive_data = SensitiveData::try_from(secret_key.0.to_vec())
            .map_err(|error| tracing::error!(?error, message_id = "dQ3vLb7N", "failed to wrap the secret key as sensitive data"))?;
        let result = context
            .execute_with_nullauth_session(|context| context.create(self.primary, public, None, Some(sensitive_data), None, None))
            .map_err(|error| tracing::error!(?error, message_id = "yF8bWn4C", "TPM2_Create failed"))?;
        Ok(SealedWgSecretKey::Tpm2SealedObject {
            private: result.out_private.value().to_vec(),
            public: result
                .out_public
                .marshall()
                .map_err(|error| tracing::error!(?error, message_id = "Cr2mSw8N", "failed to marshall the sealed public part"))?,
        })
    }

    pub(crate) fn unseal(&self, sealed_secret_key: &SealedWgSecretKey) -> Result<PlaintextWgSecretKey, ()> {
        let SealedWgSecretKey::Tpm2SealedObject { private, public } = sealed_secret_key else {
            let sealing_type: &'static str = sealed_secret_key.into();
            tracing::error!(
                sealing_type,
                message_id = "Wn6cJq3F",
                "sealed wireguard secret key type is not supported on linux"
            );
            return Err(());
        };
        let private = Private::try_from(private.clone())
            .map_err(|error| tracing::error!(?error, message_id = "Fu3qVa9L", "failed to parse the sealed private part"))?;
        let public = Public::unmarshall(public)
            .map_err(|error| tracing::error!(?error, message_id = "Gv6rWb5H", "failed to unmarshall the sealed public part"))?;
        let mut context = self.context.lock().unwrap();
        let sealed_object = context
            .execute_with_nullauth_session(|context| context.load(self.primary, private, public))
            .map_err(|error| tracing::error!(?error, message_id = "Uk2pXs9M", "TPM2_Load failed"))?;
        let unsealed_data = context
            .execute_with_temporary_object(sealed_object.into(), |context, sealed_object| {
                let session = start_policy_session(context, SessionType::Policy)?;
                context.execute_with_temporary_object(SessionHandle::from(session).into(), |context, _| {
                    let policy_session = PolicySession::try_from(session)?;
                    context.policy_pcr(policy_session, Digest::default(), pcr7_selection()?)?;
                    context.execute_with_session(Some(session), |context| context.unseal(sealed_object))
                })
            })
            .map_err(|error| tracing::error!(?error, message_id = "Tz7cKp5W", "unsealing the wireguard secret key with the TPM failed"))?;
        let secret_key = <[u8; 32]>::try_from(unsealed_data.value()).map_err(|_| {
            tracing::error!(
                length = unsealed_data.value().len(),
                message_id = "Ug2nQx6L",
                "unsealed wireguard secret key has an unexpected length"
            );
        })?;
        tracing::info!(message_id = "nH6tXs9W", "unsealed wireguard secret key");
        Ok(PlaintextWgSecretKey::new(secret_key))
    }
}

fn start_policy_session(context: &mut Context, session_type: SessionType) -> tss_esapi::Result<AuthSession> {
    context
        .start_auth_session(None, None, None, session_type, SymmetricDefinition::AES_128_CFB, HashingAlgorithm::Sha256)?
        .ok_or(Error::WrapperError(WrapperErrorKind::WrongValueFromTpm))
}

fn pcr7_selection() -> tss_esapi::Result<PcrSelectionList> {
    PcrSelectionListBuilder::new()
        .with_selection(HashingAlgorithm::Sha256, &[PcrSlot::Slot7])
        .build()
}

fn storage_primary_public() -> tss_esapi::Result<Public> {
    let object_attributes = ObjectAttributesBuilder::new()
        // Required since the sealed child is fixedTPM; private part can never leave this TPM.
        .with_fixed_tpm(true)
        // Required by fixedTPM; no duplication to another parent.
        .with_fixed_parent(true)
        // Required for a primary; the TPM derives the key itself.
        .with_sensitive_data_origin(true)
        // Required since no auth policy is set; authorize with the (empty) auth value.
        .with_user_with_auth(true)
        // Required for a storage key; only operates on child objects.
        .with_restricted(true)
        // Required for a storage key; may unwrap children.
        .with_decrypt(true)
        // Auth failures don't count toward lockout.
        .with_no_da(true)
        .build()?;
    PublicBuilder::new()
        .with_public_algorithm(PublicAlgorithm::Ecc)
        .with_name_hashing_algorithm(HashingAlgorithm::Sha256)
        .with_object_attributes(object_attributes)
        .with_ecc_parameters(
            PublicEccParametersBuilder::new_restricted_decryption_key(SymmetricDefinitionObject::AES_128_CFB, EccCurve::NistP256).build()?,
        )
        .with_ecc_unique_identifier(EccPoint::default())
        .build()
}

fn sealed_object_public(context: &mut Context) -> tss_esapi::Result<Public> {
    let session = start_policy_session(context, SessionType::Trial)?;
    // PCR 7 holds the firmware's measurement of the Secure Boot configuration (SecureBoot flag, PK/KEK/db/dbx).
    // Binding the seal to it blocks unsealing from another OS booted with modified Secure Boot state.
    let auth_policy = context.execute_with_temporary_object(SessionHandle::from(session).into(), |context, _| {
        let policy_session = PolicySession::try_from(session)?;
        context.policy_pcr(policy_session, Digest::default(), pcr7_selection()?)?;
        context.policy_get_digest(policy_session)
    })?;
    let object_attributes = ObjectAttributesBuilder::new()
        // Sealed key can never be duplicated off this TPM.
        .with_fixed_tpm(true)
        // Required by fixedTPM; no duplication to another parent.
        .with_fixed_parent(true)
        // Required unset since we provide the sealed data instead of the TPM generating it.
        .with_sensitive_data_origin(false)
        // Unset so only the PCR policy can authorize unsealing, not the (empty) auth value.
        .with_user_with_auth(false)
        // Auth failures don't count toward lockout.
        .with_no_da(true)
        .build()?;
    PublicBuilder::new()
        .with_public_algorithm(PublicAlgorithm::KeyedHash)
        .with_name_hashing_algorithm(HashingAlgorithm::Sha256)
        .with_object_attributes(object_attributes)
        .with_auth_policy(auth_policy)
        .with_keyed_hash_parameters(PublicKeyedHashParameters::new(KeyedHashScheme::Null))
        .with_keyed_hash_unique_identifier(Digest::default())
        .build()
}
