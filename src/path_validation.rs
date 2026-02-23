fn is_path_separator(ch: char) -> bool {
    ch == '/' || ch == std::path::MAIN_SEPARATOR
}

pub fn has_meaningful_relative_segment(value: &str) -> bool {
    value
        .split(is_path_separator)
        .any(|segment| !segment.is_empty() && !matches!(segment, "." | ".."))
}

pub fn ends_with_dot_component(value: &str) -> bool {
    let trimmed = value.trim_end_matches(is_path_separator);
    matches!(
        trimmed.rsplit(is_path_separator).next(),
        Some(".") | Some("..")
    )
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

    #[test]
    fn relative_segment_supports_platform_separator() {
        let sep = std::path::MAIN_SEPARATOR;
        let real = format!(".{sep}scripts{sep}run");
        let parent_only = format!("..{sep}..");
        assert!(has_meaningful_relative_segment(&real));
        assert!(!has_meaningful_relative_segment(&parent_only));
    }

    #[test]
    fn dot_component_detection_supports_platform_separator() {
        let sep = std::path::MAIN_SEPARATOR;
        assert!(ends_with_dot_component(&format!("scripts{sep}.")));
        assert!(ends_with_dot_component(&format!("scripts{sep}..")));
        assert!(!ends_with_dot_component(&format!("scripts{sep}run")));
    }
}
