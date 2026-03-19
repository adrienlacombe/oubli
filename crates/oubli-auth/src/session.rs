use std::time::Duration;

/// Session timeout configuration for each auth tier.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// T2 (transact) timeout — default 2 minutes.
    pub t2_timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            t2_timeout: Duration::from_secs(2 * 60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_timeouts() {
        let cfg = SessionConfig::default();
        assert_eq!(cfg.t2_timeout, Duration::from_secs(120));
    }
}
