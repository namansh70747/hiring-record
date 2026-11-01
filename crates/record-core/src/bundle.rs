//! Portable integrity envelope for the synthetic record.
//! The signature covers the exact UTF-8 bytes of `payload`, not a reserialized value.
use crate::{evaluate_report, hash_event, Event, Report, State};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct BundlePayload {
    pub format: String,
    pub role_id: String,
    pub standard_id: String,
    pub report: Report,
    pub events: Vec<Event>,
    pub disclosure: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_references: Vec<SourceReference>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub baseline_digest: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub baseline_schema: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub rules_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub opening_json: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub signer_key_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_from: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_to: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct SourceReference {
    pub id: String,
    pub person: String,
    pub kind: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
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
    /// verified, unsupported, or failed. Legacy bundles without a baseline are unsupported.
    pub replay: String,
    pub rules_version: String,
}

pub fn create_bundle(state: &State, signing_key: &SigningKey) -> Result<Bundle, String> {
    let payload = BundlePayload {
        format: "hiring-record-payload/v2".into(),
        role_id: state.role_id.clone(),
        standard_id: state.active.id.clone(),
        report: evaluate_report(state),
        events: state.events.clone(),
        disclosure: "Synthetic demo. Signature authenticates these bytes against the included key; source references are included, but source documents and truth are not verified.".into(),
        source_references: state.sources.iter().map(|source| SourceReference { id:source.id.clone(), person:source.person.clone(), kind:source.kind.clone(), timestamp:source.timestamp.clone() }).collect(),
        baseline_digest: String::new(),
        baseline_schema: String::new(),
        rules_version: String::new(),
        opening_json: String::new(),
        signer_key_id: String::new(),
        event_from: None,
        event_to: None,
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

/// Self-contained payload. Replay uses the embedded baseline, not the local fixture.
pub fn create_replay_bundle(
    state: &State,
    signing_key: &SigningKey,
    key_id: &str,
) -> Result<Bundle, String> {
    if state.opening_json.is_empty() || state.baseline_digest.is_empty() {
        return Err("this state has no sealed baseline".into());
    }
    let mut bundle = create_bundle(state, signing_key)?;
    let mut payload: BundlePayload =
        serde_json::from_str(&bundle.payload).map_err(|e| e.to_string())?;
    payload.format = "hiring-record-payload/v3".into();
    payload.baseline_digest = state.baseline_digest.clone();
    payload.baseline_schema = state.baseline_schema.clone();
    payload.rules_version = state.rules_version.clone();
    payload.opening_json = state.opening_json.clone();
    payload.signer_key_id = key_id.into();
    payload.event_from = Some(1);
    payload.event_to = Some(state.events.len() as u64);
    payload.disclosure = "Synthetic demo. Signature authenticates these bytes. Replay checks the embedded baseline and rules version. It does not establish source truth, fairness, or signer identity.".into();
    let payload = serde_json::to_string(&payload).map_err(|e| e.to_string())?;
    let signature = signing_key.sign(payload.as_bytes());
    bundle.payload = payload;
    bundle.signature = hex::encode(signature.to_bytes());
    Ok(bundle)
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
    if payload.format != "hiring-record-payload/v1"
        && payload.format != "hiring-record-payload/v2"
        && payload.format != "hiring-record-payload/v3"
    {
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
    let legacy = payload.format != "hiring-record-payload/v3"
        || payload.opening_json.is_empty()
        || payload.rules_version != crate::RULES_VERSION;
    let replay = if legacy {
        "unsupported".into()
    } else {
        match replay_agrees(&payload) {
            Ok(true) => "verified".into(),
            _ => "failed".into(),
        }
    };
    Ok(Verification {
        signature_valid,
        chain_valid,
        trusted_signer: trusted_key
            .is_some_and(|trusted| trusted.eq_ignore_ascii_case(&bundle.public_key)),
        event_count: payload.events.len(),
        role_id: payload.role_id,
        replay,
        rules_version: payload.rules_version,
    })
}

fn replay_agrees(payload: &BundlePayload) -> Result<bool, String> {
    let mut state: crate::State =
        serde_json::from_str(&payload.opening_json).map_err(|e| e.to_string())?;
    if !state.baseline_digest.is_empty() && state.baseline_digest != payload.baseline_digest {
        return Ok(false);
    }
    for event in &payload.events {
        crate::replay_event(&mut state, event.clone()).map_err(|e| e.to_string())?;
    }
    let report = crate::evaluate_report(&state);
    Ok(report == payload.report)
}
