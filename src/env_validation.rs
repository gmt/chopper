#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvKeyViolation {
    ContainsEquals,
    ContainsNul,
}

pub fn validate_env_key(key: &str) -> Result<(), EnvKeyViolation> {
    if key.contains('=') {
        return Err(EnvKeyViolation::ContainsEquals);
    }
    if key.contains('\0') {
        return Err(EnvKeyViolation::ContainsNul);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnvValueViolation {
    ContainsNul,
}

pub fn validate_env_value(value: &str) -> Result<(), EnvValueViolation> {
    if value.contains('\0') {
        return Err(EnvValueViolation::ContainsNul);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{validate_env_key, validate_env_value, EnvKeyViolation, EnvValueViolation};

    #[test]
    fn env_key_validator_rejects_equals_sign() {
        let err = validate_env_key("BAD=KEY").expect_err("expected invalid key");
        assert_eq!(err, EnvKeyViolation::ContainsEquals);
    }

    #[test]
    fn env_key_validator_rejects_nul_byte() {
        let err = validate_env_key("BAD\0KEY").expect_err("expected invalid key");
        assert_eq!(err, EnvKeyViolation::ContainsNul);
    }

    #[test]
    fn env_value_validator_rejects_nul_byte() {
        let err = validate_env_value("bad\0value").expect_err("expected invalid value");
        assert_eq!(err, EnvValueViolation::ContainsNul);
    }
}
