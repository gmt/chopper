#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgViolation {
    ContainsNul,
}

pub fn validate_arg_value(value: &str) -> Result<(), ArgViolation> {
    if value.contains('\0') {
        return Err(ArgViolation::ContainsNul);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_arg_value, ArgViolation};

    #[test]
    fn arg_validator_accepts_empty_and_unicode_values() {
        assert!(validate_arg_value("").is_ok());
        assert!(validate_arg_value("emoji🚀").is_ok());
        assert!(validate_arg_value(" spaced value ").is_ok());
    }

    #[test]
    fn arg_validator_accepts_symbolic_and_pathlike_values() {
        assert!(validate_arg_value("--flag=value").is_ok());
        assert!(validate_arg_value("../relative/path").is_ok());
        assert!(validate_arg_value("semi;colon&and").is_ok());
        assert!(validate_arg_value("$DOLLAR").is_ok());
        assert!(validate_arg_value("brace{value}").is_ok());
        assert!(validate_arg_value(r"windows\path").is_ok());
    }

    #[test]
    fn arg_validator_rejects_nul_byte() {
        let err = validate_arg_value("bad\0arg").expect_err("expected invalid arg");
        assert_eq!(err, ArgViolation::ContainsNul);
    }
}
