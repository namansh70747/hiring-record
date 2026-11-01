//! Retention policy for expired synthetic sessions. Active sessions are never eligible.

pub fn may_purge(now: u64, expires_at: u64, grace_seconds: u64, has_active_lease: bool) -> bool {
    !has_active_lease && now > expires_at.saturating_add(grace_seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_and_leased_sessions_cannot_be_purged() {
        let now = 1_000_000;
        assert!(!may_purge(now, now + 10, 86_400, false));
        assert!(!may_purge(now, now - 10, 86_400, false));
        assert!(!may_purge(now, now - 200_000, 86_400, true));
    }

    #[test]
    fn expired_session_past_grace_without_a_lease_can_be_purged() {
        assert!(may_purge(200_000, 1_000, 86_400, false));
    }
}
