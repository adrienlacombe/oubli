use std::time::{Duration, Instant};

use crate::session::SessionConfig;

/// Security tiers — each gate unlocks more capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AuthTier {
    /// Device locked — nothing visible.
    T0Locked,
    /// Biometric passed — can view balances and sign transactions.
    T2Transact,
    /// Re-auth for critical ops (export seed, wipe).
    T3Critical,
}

/// Actions that can drive tier transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthAction {
    BiometricSuccess,
    Timeout,
    Background,
    Lock,
}

/// Result of an attempted transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthTransitionResult {
    TierChanged(AuthTier),
    Denied,
}

/// Mutable authentication state tracked at runtime.
#[derive(Debug, Clone)]
pub struct AuthState {
    pub tier: AuthTier,
    tier_expiry: Option<Instant>,
    session_config: SessionConfig,
}

impl AuthState {
    pub fn new(session_config: SessionConfig) -> Self {
        Self {
            tier: AuthTier::T0Locked,
            tier_expiry: None,
            session_config,
        }
    }

    /// Drive the state machine. Returns the outcome.
    pub fn apply(&mut self, action: AuthAction) -> AuthTransitionResult {
        match (self.tier, action) {
            // T0 -> T2 via biometric
            (AuthTier::T0Locked, AuthAction::BiometricSuccess) => {
                self.transition_to(AuthTier::T2Transact)
            }
            // Timeout/background drops tier
            (_, AuthAction::Timeout) | (_, AuthAction::Background) => match self.tier {
                AuthTier::T3Critical => self.transition_to(AuthTier::T2Transact),
                AuthTier::T2Transact => self.transition_to(AuthTier::T0Locked),
                AuthTier::T0Locked => AuthTransitionResult::TierChanged(AuthTier::T0Locked),
            },
            // Lock always returns to T0
            (_, AuthAction::Lock) => self.transition_to(AuthTier::T0Locked),
            // Anything else is denied
            _ => AuthTransitionResult::Denied,
        }
    }

    /// Check whether the current tier has expired. If so, apply timeout.
    pub fn check_expiry(&mut self) -> Option<AuthTransitionResult> {
        if let Some(expiry) = self.tier_expiry {
            if Instant::now() >= expiry {
                return Some(self.apply(AuthAction::Timeout));
            }
        }
        None
    }

    /// Remaining time before the current tier expires (if any).
    pub fn remaining(&self) -> Option<Duration> {
        self.tier_expiry
            .and_then(|e| e.checked_duration_since(Instant::now()))
    }

    fn transition_to(&mut self, tier: AuthTier) -> AuthTransitionResult {
        self.tier = tier;
        self.tier_expiry = match tier {
            AuthTier::T2Transact => Some(Instant::now() + self.session_config.t2_timeout),
            AuthTier::T3Critical => Some(Instant::now() + self.session_config.t2_timeout),
            AuthTier::T0Locked => {
                self.tier_expiry = None;
                None
            }
        };
        AuthTransitionResult::TierChanged(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> AuthState {
        AuthState::new(SessionConfig::default())
    }

    #[test]
    fn biometric_goes_straight_to_t2() {
        let mut s = state();
        assert_eq!(s.tier, AuthTier::T0Locked);
        assert_eq!(
            s.apply(AuthAction::BiometricSuccess),
            AuthTransitionResult::TierChanged(AuthTier::T2Transact)
        );
    }

    #[test]
    fn lock_resets_to_t0() {
        let mut s = state();
        s.apply(AuthAction::BiometricSuccess);
        assert_eq!(
            s.apply(AuthAction::Lock),
            AuthTransitionResult::TierChanged(AuthTier::T0Locked)
        );
    }

    #[test]
    fn timeout_drops_to_locked() {
        let mut s = state();
        s.apply(AuthAction::BiometricSuccess);
        assert_eq!(s.tier, AuthTier::T2Transact);
        assert_eq!(
            s.apply(AuthAction::Timeout),
            AuthTransitionResult::TierChanged(AuthTier::T0Locked)
        );
    }

    #[test]
    fn background_drops_to_locked() {
        let mut s = state();
        s.apply(AuthAction::BiometricSuccess);
        assert_eq!(
            s.apply(AuthAction::Background),
            AuthTransitionResult::TierChanged(AuthTier::T0Locked)
        );
    }

    #[test]
    fn denied_on_invalid_transition() {
        let mut s = state();
        // Duplicate biometric at T2 is denied
        s.apply(AuthAction::BiometricSuccess);
        assert_eq!(
            s.apply(AuthAction::BiometricSuccess),
            AuthTransitionResult::Denied
        );
    }
}
