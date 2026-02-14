use std::env;
use std::path::PathBuf;

pub fn env_flag_enabled(name: &str) -> bool {
    let Ok(value) = env::var(name) else {
        return false;
    };
    let normalized = value.trim().to_ascii_lowercase();
    matches!(normalized.as_str(), "1" | "true" | "yes" | "on")
}

pub fn env_path_override(name: &str) -> Option<PathBuf> {
    let value = env::var(name).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

#[cfg(test)]
mod tests {
    use super::{env_flag_enabled, env_path_override};
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn env_flag_enabled_interprets_truthy_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_TEST_FLAG");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));

        env::set_var("CHOPPER_TEST_FLAG", "1");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " TRUE ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " YeS ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "on");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "0");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::remove_var("CHOPPER_TEST_FLAG");
    }

    #[test]
    fn env_flag_enabled_rejects_falsey_and_unknown_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::set_var("CHOPPER_TEST_FLAG", "false");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "off");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "  ");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "definitely-not");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::remove_var("CHOPPER_TEST_FLAG");
    }

    #[test]
    fn env_path_override_requires_non_blank_value() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_TEST_PATH");
        assert!(env_path_override("CHOPPER_TEST_PATH").is_none());

        env::set_var("CHOPPER_TEST_PATH", "   ");
        assert!(env_path_override("CHOPPER_TEST_PATH").is_none());

        env::set_var("CHOPPER_TEST_PATH", " /tmp/chopper ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("/tmp/chopper"))
        );
        env::remove_var("CHOPPER_TEST_PATH");
    }
}
