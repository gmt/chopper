use anyhow::{anyhow, Result};

pub fn parse_env_assignment(value: &str) -> Result<(String, String)> {
    let mut parts = value.splitn(2, '=');
    let key = parts.next().unwrap_or_default().trim();
    let val = parts.next();
    let Some(val) = val else {
        return Err(anyhow!(
            "env assignment must be in KEY=VALUE form; got `{value}`"
        ));
    };
    if key.is_empty() {
        return Err(anyhow!("env assignment key cannot be blank"));
    }
    Ok((key.to_string(), val.to_string()))
}

pub fn parse_bool_flag(value: &str, field: &str) -> Result<bool> {
    let normalized = value.trim();
    match normalized.to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow!(
            "{field} must be one of true/false/1/0/yes/no/on/off"
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_bool_flag, parse_env_assignment};

    #[test]
    fn env_assignment_parses_key_value_pairs() {
        let parsed = parse_env_assignment("KEY=value").expect("assignment parse");
        assert_eq!(parsed, ("KEY".to_string(), "value".to_string()));
    }

    #[test]
    fn env_assignment_rejects_missing_equals_separator() {
        let err = parse_env_assignment("KEY").expect_err("assignment should fail");
        assert!(err.to_string().contains("KEY=VALUE"));
    }

    #[test]
    fn env_assignment_rejects_blank_key() {
        let err = parse_env_assignment(" =value").expect_err("blank key should fail");
        assert!(err.to_string().contains("cannot be blank"));
    }

    #[test]
    fn bool_flag_parser_accepts_truthy_and_falsey_values() {
        assert!(parse_bool_flag("true", "flag").expect("true should parse"));
        assert!(parse_bool_flag("ON", "flag").expect("on should parse"));
        assert!(!parse_bool_flag("false", "flag").expect("false should parse"));
        assert!(!parse_bool_flag("0", "flag").expect("0 should parse"));
    }

    #[test]
    fn bool_flag_parser_rejects_unknown_values() {
        let err = parse_bool_flag("maybe", "flag").expect_err("unknown should fail");
        assert!(err.to_string().contains("true/false"));
    }
}
