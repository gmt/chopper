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
    fn config_identifier_normalization_treats_blank_as_unset() {
        let out = normalize_optional_identifier_for_config(Some("   "))
            .expect("blank identifier should normalize");
        assert!(out.is_none());
    }

    #[test]
    fn invocation_identifier_normalization_rejects_blank_values() {
        let err = normalize_optional_identifier_for_invocation(Some("   "))
            .expect_err("blank identifier should be invalid for invocation");
        assert_eq!(err, JournalIdentifierViolation::Blank);
    }

    #[test]
    fn identifier_normalization_rejects_nul_values() {
        let err = normalize_optional_identifier_for_config(Some("svc\0id"))
            .expect_err("nul identifier should be invalid");
        assert_eq!(err, JournalIdentifierViolation::ContainsNul);
    }
}
