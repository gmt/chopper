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
