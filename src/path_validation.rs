pub fn has_meaningful_relative_segment(value: &str) -> bool {
    value
        .split('/')
        .any(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

pub fn ends_with_dot_component(value: &str) -> bool {
    let trimmed = value.trim_end_matches('/');
    matches!(trimmed.rsplit('/').next(), Some(".") | Some(".."))
}

#[cfg(test)]
mod tests {
    use super::{ends_with_dot_component, has_meaningful_relative_segment};

    #[test]
    fn relative_segment_requires_real_path_component() {
        assert!(!has_meaningful_relative_segment("."));
        assert!(!has_meaningful_relative_segment("../.."));
        assert!(has_meaningful_relative_segment("./scripts/run"));
    }

    #[test]
    fn dot_component_detection_only_checks_trailing_segment() {
        assert!(ends_with_dot_component("scripts/."));
        assert!(ends_with_dot_component("scripts/.."));
        assert!(!ends_with_dot_component("scripts/run"));
    }
}
