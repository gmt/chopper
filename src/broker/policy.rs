use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::path::Path;

/// Hard limit: max number of namespaces any single UID may create.
pub const MAX_NAMESPACES_PER_UID: usize = 16;

/// Hard ceiling for SystemMaxUse (bytes).  512 MiB.
pub const MAX_SYSTEM_MAX_USE_BYTES: u64 = 512 * 1024 * 1024;

/// Default SystemMaxUse when the client does not specify one.
pub const DEFAULT_SYSTEM_MAX_USE: &str = "64M";

/// Hard ceiling for RateLimitBurst.
pub const MAX_RATE_LIMIT_BURST: u32 = 10_000;

/// Default RateLimitBurst when the client does not specify one.
pub const DEFAULT_RATE_LIMIT_BURST: u32 = 1_000;

/// Minimum RateLimitIntervalSec (microseconds).  1 ms.
pub const MIN_RATE_LIMIT_INTERVAL_USEC: u64 = 1_000;

/// Default RateLimitIntervalSec in microseconds (30 s).
pub const DEFAULT_RATE_LIMIT_INTERVAL_USEC: u64 = 30_000_000;

/// Hard ceiling for RateLimitIntervalSec in microseconds (1 hour).
pub const MAX_RATE_LIMIT_INTERVAL_USEC: u64 = 3_600_000_000;

/// Validated and clamped journal drop-in configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalDropInConfig {
    pub system_max_use: String,
    pub rate_limit_interval_usec: u64,
    pub rate_limit_burst: u32,
}

impl Default for JournalDropInConfig {
    fn default() -> Self {
        Self {
            system_max_use: DEFAULT_SYSTEM_MAX_USE.to_string(),
            rate_limit_interval_usec: DEFAULT_RATE_LIMIT_INTERVAL_USEC,
            rate_limit_burst: DEFAULT_RATE_LIMIT_BURST,
        }
    }
}

/// Validate that `namespace` belongs to the caller identified by `caller_uid`.
///
/// The namespace must start with `u<caller_uid>-`.
pub fn validate_namespace_ownership(namespace: &str, caller_uid: u32) -> Result<()> {
    let expected_prefix = format!("u{caller_uid}-");
    if !namespace.starts_with(&expected_prefix) {
        return Err(anyhow!(
            "namespace `{namespace}` is not owned by UID {caller_uid}; \
             expected prefix `{expected_prefix}`"
        ));
    }
    if namespace.len() <= expected_prefix.len() {
        return Err(anyhow!(
            "namespace `{namespace}` has no logical name after the UID prefix"
        ));
    }
    if namespace.contains('\0') {
        return Err(anyhow!("namespace cannot contain NUL bytes"));
    }
    Ok(())
}

/// Count how many journal namespace runtime directories exist for a given UID
/// by scanning `/run/systemd/` for directories matching `journal.u<uid>-*`.
pub fn count_active_namespaces_for_uid(uid: u32) -> Result<usize> {
    let run_dir = Path::new("/run/systemd");
    if !run_dir.is_dir() {
        return Ok(0);
    }
    let prefix = format!("journal.u{uid}-");
    let count = std::fs::read_dir(run_dir)
        .map_err(|e| anyhow!("failed to read /run/systemd: {e}"))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with(&prefix))
        })
        .count();
    Ok(count)
}

/// Parse and clamp client-supplied options into a safe [`JournalDropInConfig`].
pub fn clamp_journal_options(options: &HashMap<String, String>) -> JournalDropInConfig {
    let mut config = JournalDropInConfig::default();

    if let Some(max_use) = options.get("max_use") {
        if let Some(clamped) = parse_and_clamp_size(max_use.trim(), MAX_SYSTEM_MAX_USE_BYTES) {
            config.system_max_use = clamped;
        }
    }

    if let Some(interval) = options.get("rate_limit_interval_usec") {
        if let Ok(value) = interval.trim().parse::<u64>() {
            config.rate_limit_interval_usec =
                value.clamp(MIN_RATE_LIMIT_INTERVAL_USEC, MAX_RATE_LIMIT_INTERVAL_USEC);
        }
    }

    if let Some(burst) = options.get("rate_limit_burst") {
        if let Ok(value) = burst.trim().parse::<u32>() {
            if value > 0 {
                config.rate_limit_burst = value.min(MAX_RATE_LIMIT_BURST);
            }
        }
    }

    config
}

/// Parse a human-readable size string (e.g. "256M", "1G", "1024K") to bytes,
/// clamp to `max_bytes`, and return the clamped value as a journald-style
/// string.  Returns `None` on parse failure.
fn parse_and_clamp_size(value: &str, max_bytes: u64) -> Option<String> {
    if value.is_empty() {
        return None;
    }

    let value_upper = value.to_ascii_uppercase();
    let (num_part, multiplier) = if let Some(num) = value_upper.strip_suffix('G') {
        (num, 1024u64 * 1024 * 1024)
    } else if let Some(num) = value_upper.strip_suffix('M') {
        (num, 1024u64 * 1024)
    } else if let Some(num) = value_upper.strip_suffix('K') {
        (num, 1024u64)
    } else {
        (value_upper.as_str(), 1u64)
    };

    let number: u64 = num_part.trim().parse().ok()?;
    let bytes = number.checked_mul(multiplier)?;
    if bytes == 0 {
        return None;
    }

    let clamped = bytes.min(max_bytes);

    // Re-express in the largest clean unit.
    if clamped % (1024 * 1024 * 1024) == 0 {
        Some(format!("{}G", clamped / (1024 * 1024 * 1024)))
    } else if clamped % (1024 * 1024) == 0 {
        Some(format!("{}M", clamped / (1024 * 1024)))
    } else if clamped % 1024 == 0 {
        Some(format!("{}K", clamped / 1024))
    } else {
        Some(clamped.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn ownership_validation_accepts_matching_uid() {
        validate_namespace_ownership("u1000-alice-ops", 1000).expect("should accept");
    }

    #[test]
    fn ownership_validation_rejects_mismatching_uid() {
        let err = validate_namespace_ownership("u1000-alice-ops", 1001)
            .expect_err("should reject wrong uid");
        assert!(err.to_string().contains("not owned by UID 1001"), "{err}");
    }

    #[test]
    fn ownership_validation_rejects_empty_logical_name() {
        let err = validate_namespace_ownership("u1000-", 1000)
            .expect_err("should reject empty suffix");
        assert!(err.to_string().contains("no logical name"), "{err}");
    }

    #[test]
    fn ownership_validation_rejects_nul_bytes() {
        let err = validate_namespace_ownership("u1000-alice\0ops", 1000)
            .expect_err("should reject NUL");
        assert!(err.to_string().contains("NUL"), "{err}");
    }

    #[test]
    fn clamp_options_uses_defaults_when_empty() {
        let config = clamp_journal_options(&HashMap::new());
        assert_eq!(config, JournalDropInConfig::default());
    }

    #[test]
    fn clamp_options_parses_and_clamps_max_use() {
        let opts = HashMap::from([("max_use".into(), "1G".into())]);
        let config = clamp_journal_options(&opts);
        assert_eq!(config.system_max_use, "512M"); // clamped to MAX
    }

    #[test]
    fn clamp_options_accepts_value_within_limit() {
        let opts = HashMap::from([("max_use".into(), "128M".into())]);
        let config = clamp_journal_options(&opts);
        assert_eq!(config.system_max_use, "128M");
    }

    #[test]
    fn clamp_options_clamps_rate_limit_burst() {
        let opts = HashMap::from([("rate_limit_burst".into(), "99999".into())]);
        let config = clamp_journal_options(&opts);
        assert_eq!(config.rate_limit_burst, MAX_RATE_LIMIT_BURST);
    }

    #[test]
    fn clamp_options_clamps_rate_limit_interval() {
        let opts = HashMap::from([("rate_limit_interval_usec".into(), "100".into())]);
        let config = clamp_journal_options(&opts);
        assert_eq!(config.rate_limit_interval_usec, MIN_RATE_LIMIT_INTERVAL_USEC);
    }

    #[test]
    fn clamp_options_ignores_invalid_values() {
        let opts = HashMap::from([
            ("max_use".into(), "not-a-number".into()),
            ("rate_limit_burst".into(), "abc".into()),
        ]);
        let config = clamp_journal_options(&opts);
        assert_eq!(config, JournalDropInConfig::default());
    }

    #[test]
    fn parse_size_accepts_various_units() {
        assert_eq!(parse_and_clamp_size("256M", u64::MAX), Some("256M".into()));
        assert_eq!(parse_and_clamp_size("1G", u64::MAX), Some("1G".into()));
        assert_eq!(parse_and_clamp_size("1024K", u64::MAX), Some("1M".into()));
        assert_eq!(
            parse_and_clamp_size("1048576", u64::MAX),
            Some("1M".into())
        );
    }

    #[test]
    fn parse_size_rejects_zero_and_empty() {
        assert_eq!(parse_and_clamp_size("0M", u64::MAX), None);
        assert_eq!(parse_and_clamp_size("", u64::MAX), None);
    }
}
