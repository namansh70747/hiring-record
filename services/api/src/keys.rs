//! Purpose-separated keys. Report, session, and workflow keys are not interchangeable.
use ed25519_dalek::SigningKey;

pub fn derive_key(base: &[u8; 32], purpose: &str) -> SigningKey {
    let material = blake3::derive_key(purpose, base);
    SigningKey::from_bytes(&material)
}

pub fn production_keys_acceptable(
    report: &[u8; 32],
    session: &[u8; 32],
    workflow: &[u8; 32],
) -> bool {
    report != session && report != workflow && session != workflow
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn purposes_do_not_share_key_bytes() {
        let base = [9u8; 32];
        let report = derive_key(&base, "hiring-record/report/v1");
        let session = derive_key(&base, "hiring-record/session/v1");
        let workflow = derive_key(&base, "hiring-record/workflow/v1");
        assert!(production_keys_acceptable(
            &report.to_bytes(),
            &session.to_bytes(),
            &workflow.to_bytes()
        ));
    }

    #[test]
    fn identical_keys_are_rejected_for_production() {
        let key = [4u8; 32];
        assert!(!production_keys_acceptable(&key, &key, &key));
    }
}
