#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalNamespaceViolation {
    Empty,
    ContainsNul,
}

pub fn normalize_namespace(value: &str) -> Result<String, JournalNamespaceViolation> {
    let namespace = value.trim();
    if namespace.is_empty() {
        return Err(JournalNamespaceViolation::Empty);
    }
    if namespace.contains('\0') {
        return Err(JournalNamespaceViolation::ContainsNul);
    }
    Ok(namespace.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalIdentifierViolation {
    Blank,
    ContainsNul,
}

pub fn normalize_optional_identifier_for_config(
    value: Option<&str>,
) -> Result<Option<String>, JournalIdentifierViolation> {
    let Some(value) = value else {
        return Ok(None);
    };
    let identifier = value.trim();
    if identifier.is_empty() {
        return Ok(None);
    }
    if identifier.contains('\0') {
        return Err(JournalIdentifierViolation::ContainsNul);
    }
    Ok(Some(identifier.to_string()))
}

pub fn normalize_optional_identifier_for_invocation(
    value: Option<&str>,
) -> Result<Option<String>, JournalIdentifierViolation> {
    let Some(value) = value else {
        return Ok(None);
    };
    let identifier = value.trim();
    if identifier.is_empty() {
        return Err(JournalIdentifierViolation::Blank);
    }
    if identifier.contains('\0') {
        return Err(JournalIdentifierViolation::ContainsNul);
    }
    Ok(Some(identifier.to_string()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaxUseViolation {
    Empty,
    ContainsNul,
    InvalidFormat,
}

/// Validate a `max_use` value (e.g. `"256M"`, `"1G"`, `"1048576"`).
pub fn validate_max_use(value: &str) -> Result<String, MaxUseViolation> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(MaxUseViolation::Empty);
    }
    if trimmed.contains('\0') {
        return Err(MaxUseViolation::ContainsNul);
    }
    let upper = trimmed.to_ascii_uppercase();
    let num_part = if let Some(n) = upper.strip_suffix('G') {
        n
    } else if let Some(n) = upper.strip_suffix('M') {
        n
    } else if let Some(n) = upper.strip_suffix('K') {
        n
    } else {
        &upper
    };
    if num_part.trim().parse::<u64>().is_err() {
        return Err(MaxUseViolation::InvalidFormat);
    }
    Ok(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        normalize_namespace, normalize_optional_identifier_for_config,
        normalize_optional_identifier_for_invocation, JournalIdentifierViolation,
        JournalNamespaceViolation,
    };

    #[test]
    fn namespace_normalization_rejects_empty_values() {
        let err = normalize_namespace("   ").expect_err("expected empty namespace error");
        assert_eq!(err, JournalNamespaceViolation::Empty);
    }

    #[test]
    fn namespace_normalization_rejects_nul_values() {
        let err = normalize_namespace("ops\0prod").expect_err("expected nul namespace error");
        assert_eq!(err, JournalNamespaceViolation::ContainsNul);
    }

    #[test]
    fn namespace_normalization_trims_surrounding_whitespace() {
        let out = normalize_namespace("  ops.ns  ").expect("namespace should normalize");
        assert_eq!(out, "ops.ns");
    }

    #[test]
    fn namespace_normalization_accepts_symbolic_and_pathlike_values() {
        let out = normalize_namespace("ops/ns.prod@2026")
            .expect("symbolic/pathlike namespace should normalize");
        assert_eq!(out, "ops/ns.prod@2026");
    }

    #[test]
    fn config_identifier_normalization_treats_blank_as_unset() {
        let out = normalize_optional_identifier_for_config(Some("   "))
            .expect("blank identifier should normalize");
        assert!(out.is_none());
    }

    #[test]
    fn config_identifier_normalization_treats_missing_as_unset() {
        let out = normalize_optional_identifier_for_config(None)
            .expect("missing identifier should normalize");
        assert!(out.is_none());
    }

    #[test]
    fn invocation_identifier_normalization_rejects_blank_values() {
        let err = normalize_optional_identifier_for_invocation(Some("   "))
            .expect_err("blank identifier should be invalid for invocation");
        assert_eq!(err, JournalIdentifierViolation::Blank);
    }

    #[test]
    fn invocation_identifier_normalization_allows_missing_values() {
        let out = normalize_optional_identifier_for_invocation(None)
            .expect("missing identifier should normalize");
        assert!(out.is_none());
    }

    #[test]
    fn identifier_normalization_trims_surrounding_whitespace() {
        let config_identifier = normalize_optional_identifier_for_config(Some("  chopper  "))
            .expect("config identifier should normalize");
        assert_eq!(config_identifier, Some("chopper".to_string()));

        let invocation_identifier =
            normalize_optional_identifier_for_invocation(Some("  chopper  "))
                .expect("invocation identifier should normalize");
        assert_eq!(invocation_identifier, Some("chopper".to_string()));
    }

    #[test]
    fn identifier_normalization_accepts_symbolic_and_pathlike_values() {
        let config_identifier =
            normalize_optional_identifier_for_config(Some("svc.id/worker\\edge@2026"))
                .expect("config identifier should normalize");
        assert_eq!(
            config_identifier,
            Some("svc.id/worker\\edge@2026".to_string())
        );

        let invocation_identifier =
            normalize_optional_identifier_for_invocation(Some("svc.id/worker\\edge@2026"))
                .expect("invocation identifier should normalize");
        assert_eq!(
            invocation_identifier,
            Some("svc.id/worker\\edge@2026".to_string())
        );
    }

    #[test]
    fn identifier_normalization_rejects_nul_values() {
        let err = normalize_optional_identifier_for_config(Some("svc\0id"))
            .expect_err("nul identifier should be invalid");
        assert_eq!(err, JournalIdentifierViolation::ContainsNul);
    }

    #[test]
    fn namespace_normalization_trims_mixed_whitespace() {
        let out = normalize_namespace("\n\t ops.ns \t")
            .expect("namespace with mixed surrounding whitespace should normalize");
        assert_eq!(out, "ops.ns");
    }

    #[test]
    fn max_use_validation_accepts_valid_sizes() {
        assert_eq!(
            super::validate_max_use("256M"),
            Ok("256M".to_string())
        );
        assert_eq!(super::validate_max_use("1G"), Ok("1G".to_string()));
        assert_eq!(super::validate_max_use("1024K"), Ok("1024K".to_string()));
        assert_eq!(
            super::validate_max_use("1048576"),
            Ok("1048576".to_string())
        );
    }

    #[test]
    fn max_use_validation_trims_whitespace() {
        assert_eq!(
            super::validate_max_use("  256M  "),
            Ok("256M".to_string())
        );
    }

    #[test]
    fn max_use_validation_rejects_empty() {
        assert_eq!(
            super::validate_max_use("   "),
            Err(super::MaxUseViolation::Empty)
        );
    }

    #[test]
    fn max_use_validation_rejects_nul() {
        assert_eq!(
            super::validate_max_use("256\0M"),
            Err(super::MaxUseViolation::ContainsNul)
        );
    }

    #[test]
    fn max_use_validation_rejects_invalid_format() {
        assert_eq!(
            super::validate_max_use("abc"),
            Err(super::MaxUseViolation::InvalidFormat)
        );
    }
}
