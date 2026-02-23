#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringViolation {
    ContainsNul,
}

pub fn reject_nul(value: &str) -> Result<(), StringViolation> {
    if value.contains('\0') {
        return Err(StringViolation::ContainsNul);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{reject_nul, StringViolation};

    #[test]
    fn reject_nul_accepts_plain_text() {
        assert_eq!(reject_nul("plain"), Ok(()));
        assert_eq!(reject_nul("emoji🚀"), Ok(()));
    }

    #[test]
    fn reject_nul_rejects_nul_bytes() {
        assert_eq!(reject_nul("bad\0value"), Err(StringViolation::ContainsNul));
    }
}
