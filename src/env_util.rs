use std::env;
use std::path::PathBuf;

pub fn env_flag_enabled(name: &str) -> bool {
    let Ok(value) = env::var(name) else {
        return false;
    };
    let normalized = value.trim();
    normalized == "1"
        || normalized.eq_ignore_ascii_case("true")
        || normalized.eq_ignore_ascii_case("yes")
        || normalized.eq_ignore_ascii_case("on")
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
        env::set_var("CHOPPER_TEST_FLAG", "  1  ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " TRUE ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " YeS ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "on");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " ON ");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n1\r\n");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nTrUe\r\n");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\u{00A0}TrUe\u{00A0}");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n\u{00A0}TrUe\u{00A0}\r\n");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nYeS\r\n");
        assert!(env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nOn\r\n");
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
        env::set_var("CHOPPER_TEST_FLAG", "\u{00A0}FaLsE\u{00A0}");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nFaLsE\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n\u{00A0}FaLsE\u{00A0}\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n0\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " No ");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "off");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", " OFF ");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nNo\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\noFf\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n   \r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\n\t \r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "  ");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\t\t");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "definitely-not");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nmaybe\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "ＴＲＵＥ");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "\r\nＴＲＵＥ\r\n");
        assert!(!env_flag_enabled("CHOPPER_TEST_FLAG"));
        env::set_var("CHOPPER_TEST_FLAG", "Ｔrue");
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

        env::set_var("CHOPPER_TEST_PATH", "\n\t/tmp/chopper-mixed\t\n");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("/tmp/chopper-mixed"))
        );
        env::remove_var("CHOPPER_TEST_PATH");
    }

    #[test]
    fn env_path_override_preserves_symbolic_and_windows_like_shapes() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");

        env::set_var("CHOPPER_TEST_PATH", "\n\t/tmp/chopper symbolic/@v1\t\n");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("/tmp/chopper symbolic/@v1"))
        );

        env::set_var("CHOPPER_TEST_PATH", r" C:\Users\me\AppData\Local\chopper ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from(r"C:\Users\me\AppData\Local\chopper"))
        );

        env::set_var("CHOPPER_TEST_PATH", "./relative path/@v1");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("./relative path/@v1"))
        );

        env::set_var(
            "CHOPPER_TEST_PATH",
            r" \\server\share\chopper symbolic\cache@v2 ",
        );
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from(r"\\server\share\chopper symbolic\cache@v2"))
        );

        env::set_var("CHOPPER_TEST_PATH", r" ..\parent\cfg @v3 ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from(r"..\parent\cfg @v3"))
        );

        env::set_var("CHOPPER_TEST_PATH", r" /tmp\mixed/windows@v4 ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from(r"/tmp\mixed/windows@v4"))
        );

        env::set_var("CHOPPER_TEST_PATH", " /tmp/chopper trailing/ ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("/tmp/chopper trailing/"))
        );

        env::set_var("CHOPPER_TEST_PATH", " C:\\tmp\\chopper trailing\\ ");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("C:\\tmp\\chopper trailing\\"))
        );

        env::set_var("CHOPPER_TEST_PATH", "\r\n/tmp/chopper crlf/@v5\r\n");
        assert_eq!(
            env_path_override("CHOPPER_TEST_PATH"),
            Some(PathBuf::from("/tmp/chopper crlf/@v5"))
        );

        env::remove_var("CHOPPER_TEST_PATH");
    }
}
