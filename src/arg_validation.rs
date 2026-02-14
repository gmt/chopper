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
    fn arg_validator_rejects_nul_byte() {
        let err = validate_arg_value("bad\0arg").expect_err("expected invalid arg");
        assert_eq!(err, ArgViolation::ContainsNul);
    }
}
