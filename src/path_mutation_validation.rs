#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathMutationViolation {
    ContainsNul,
}

pub fn validate_path_mutation_value(value: &str) -> Result<(), PathMutationViolation> {
    match crate::string_validation::reject_nul(value) {
        Ok(()) => Ok(()),
        Err(crate::string_validation::StringViolation::ContainsNul) => {
            Err(PathMutationViolation::ContainsNul)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{validate_path_mutation_value, PathMutationViolation};

    #[test]
    fn path_mutation_values_reject_nul_bytes() {
        assert_eq!(
            validate_path_mutation_value("bad\0value"),
            Err(PathMutationViolation::ContainsNul)
        );
    }

    #[test]
    fn path_mutation_values_allow_blank_and_symbolic_content() {
        assert!(validate_path_mutation_value("").is_ok());
        assert!(validate_path_mutation_value(r"~/bin:@weird ${PATH}").is_ok());
    }
}
