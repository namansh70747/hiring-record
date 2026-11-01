//! Portable integrity envelope for the synthetic record.
//! The signature covers the exact UTF-8 bytes of `payload`, not a reserialized value.
use crate::{evaluate_report, hash_event, Event, Report, State};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BundlePayload {
    pub format: String,
    pub role_id: String,
    pub standard_id: String,
    pub report: Report,
    pub events: Vec<Event>,
    pub disclosure: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bundle {
    pub format: String,
    pub algorithm: String,
    pub public_key: String,
    pub payload: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verification {
    pub signature_valid: bool,
    pub chain_valid: bool,
    pub trusted_signer: bool,
    pub event_count: usize,
    pub role_id: String,
}

pub fn create_bundle(state: &State, signing_key: &SigningKey) -> Result<Bundle, String> {
    let payload = BundlePayload {
        format: "hiring-record-payload/v1".into(),
        role_id: state.role_id.clone(),
        standard_id: state.active.id.clone(),
        report: evaluate_report(state),
        events: state.events.clone(),
        disclosure: "Synthetic demo. Signature authenticates these bytes against the included key; source evidence is not included.".into(),
    };
    let payload = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let signature = signing_key.sign(payload.as_bytes());
    Ok(Bundle {
        format: "hiring-record-bundle/v1".into(),
        algorithm: "Ed25519".into(),
        public_key: hex::encode(signing_key.verifying_key().as_bytes()),
        payload,
        signature: hex::encode(signature.to_bytes()),
    })
}

pub fn verify_bundle(bundle: &Bundle, trusted_key: Option<&str>) -> Result<Verification, String> {
    if bundle.format != "hiring-record-bundle/v1" || bundle.algorithm != "Ed25519" {
        return Err("Unsupported bundle format or signature algorithm".into());
    }
    let key_bytes: [u8; 32] = hex::decode(&bundle.public_key)
        .map_err(|_| "Invalid public key encoding")?
        .try_into()
        .map_err(|_| "Invalid public key length")?;
    let key = VerifyingKey::from_bytes(&key_bytes).map_err(|_| "Invalid public key")?;
    let sig_bytes: [u8; 64] = hex::decode(&bundle.signature)
        .map_err(|_| "Invalid signature encoding")?
        .try_into()
        .map_err(|_| "Invalid signature length")?;
    let signature_valid = key
        .verify(
            bundle.payload.as_bytes(),
            &Signature::from_bytes(&sig_bytes),
        )
        .is_ok();
    let payload: BundlePayload =
        serde_json::from_str(&bundle.payload).map_err(|_| "Invalid payload JSON")?;
    if payload.format != "hiring-record-payload/v1" {
        return Err("Unsupported payload format".into());
    }
    let mut previous = "genesis".to_string();
    let mut chain_valid = true;
    for (index, event) in payload.events.iter().enumerate() {
        if event.seq != index as u64 + 1
            || event.prev_hash != previous
            || hash_event(event) != event.hash
        {
            chain_valid = false;
            break;
        }
        previous.clone_from(&event.hash);
    }
    Ok(Verification {
        signature_valid,
        chain_valid,
        trusted_signer: trusted_key
            .is_some_and(|trusted| trusted.eq_ignore_ascii_case(&bundle.public_key)),
        event_count: payload.events.len(),
        role_id: payload.role_id,
    })
}
