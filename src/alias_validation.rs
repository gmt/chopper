#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliasViolation {
    Empty,
    ContainsNul,
    IsSeparator,
    StartsWithDash,
    ContainsWhitespace,
    IsDotToken,
    ContainsPathSeparator,
}

pub fn validate_alias_identifier(alias: &str) -> Result<(), AliasViolation> {
    if alias.trim().is_empty() {
        return Err(AliasViolation::Empty);
    }
    if alias.contains('\0') {
        return Err(AliasViolation::ContainsNul);
    }
    if alias == "--" {
        return Err(AliasViolation::IsSeparator);
    }
    if alias.starts_with('-') {
        return Err(AliasViolation::StartsWithDash);
    }
    if alias.chars().any(char::is_whitespace) {
        return Err(AliasViolation::ContainsWhitespace);
    }
    if alias == "." || alias == ".." {
        return Err(AliasViolation::IsDotToken);
    }
    if alias.contains('/') || alias.contains('\\') {
        return Err(AliasViolation::ContainsPathSeparator);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_alias_identifier, AliasViolation};

    #[test]
    fn validator_accepts_typical_alias_identifiers() {
        assert!(validate_alias_identifier("deploy.prod").is_ok());
        assert!(validate_alias_identifier("alpha:beta").is_ok());
        assert!(validate_alias_identifier("emoji🚀").is_ok());
    }

    #[test]
    fn validator_rejects_whitespace_and_pathlike_aliases() {
        let whitespace = validate_alias_identifier("bad alias")
            .expect_err("whitespace aliases should be rejected");
        assert_eq!(whitespace, AliasViolation::ContainsWhitespace);

        let pathlike = validate_alias_identifier("bad/alias")
            .expect_err("path-like aliases should be rejected");
        assert_eq!(pathlike, AliasViolation::ContainsPathSeparator);
    }

    #[test]
    fn validator_rejects_flag_and_separator_aliases() {
        let separator = validate_alias_identifier("--").expect_err("separator should be rejected");
        assert_eq!(separator, AliasViolation::IsSeparator);

        let flaglike = validate_alias_identifier("-alias")
            .expect_err("dash-prefixed aliases should be rejected");
        assert_eq!(flaglike, AliasViolation::StartsWithDash);
    }

    #[test]
    fn validator_rejects_dot_and_nul_aliases() {
        let dot = validate_alias_identifier(".").expect_err("dot alias should be rejected");
        assert_eq!(dot, AliasViolation::IsDotToken);

        let parent = validate_alias_identifier("..").expect_err("parent alias should be rejected");
        assert_eq!(parent, AliasViolation::IsDotToken);

        let nul =
            validate_alias_identifier("bad\0alias").expect_err("nul alias should be rejected");
        assert_eq!(nul, AliasViolation::ContainsNul);
    }

    #[test]
    fn validator_rejects_empty_and_blank_aliases() {
        let empty = validate_alias_identifier("").expect_err("empty alias should be rejected");
        assert_eq!(empty, AliasViolation::Empty);

        let blank = validate_alias_identifier("   ").expect_err("blank alias should be rejected");
        assert_eq!(blank, AliasViolation::Empty);
    }
}
