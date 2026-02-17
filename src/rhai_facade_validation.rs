use rhai::{Array, EvalAltResult, ImmutableString, Map};
use std::collections::HashMap;
use std::path::PathBuf;

pub type RhaiResult<T> = Result<T, Box<EvalAltResult>>;

pub fn facade_error(message: impl Into<String>) -> Box<EvalAltResult> {
    message.into().into()
}

pub fn ensure_no_nul(field: &str, value: &str) -> RhaiResult<()> {
    if value.contains('\0') {
        return Err(facade_error(format!("{field} cannot contain NUL bytes")));
    }
    Ok(())
}

pub fn ensure_not_blank(field: &str, value: &str) -> RhaiResult<String> {
    ensure_no_nul(field, value)?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(facade_error(format!("{field} cannot be blank")));
    }
    Ok(trimmed.to_string())
}

pub fn ensure_path(field: &str, value: &str) -> RhaiResult<PathBuf> {
    let normalized = ensure_not_blank(field, value)?;
    Ok(PathBuf::from(normalized))
}

pub fn normalize_timeout_ms(timeout_ms: i64) -> RhaiResult<Option<u64>> {
    if timeout_ms < 0 {
        return Err(facade_error("timeout_ms cannot be negative"));
    }
    if timeout_ms == 0 {
        return Ok(None);
    }
    Ok(Some(timeout_ms as u64))
}

pub fn array_to_strings(field: &str, values: &Array) -> RhaiResult<Vec<String>> {
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        if let Some(text) = value.clone().try_cast::<ImmutableString>() {
            let text = text.to_string();
            ensure_no_nul(field, &text)?;
            out.push(text);
            continue;
        }
        if let Some(text) = value.clone().try_cast::<String>() {
            ensure_no_nul(field, &text)?;
            out.push(text);
            continue;
        }
        return Err(facade_error(format!("all {field} entries must be strings")));
    }
    Ok(out)
}

pub fn map_to_strings(field: &str, values: &Map) -> RhaiResult<HashMap<String, String>> {
    let mut out = HashMap::with_capacity(values.len());
    for (key, value) in values {
        let key = ensure_not_blank(&format!("{field} key"), key.as_str())?;
        if let Some(text) = value.clone().try_cast::<ImmutableString>() {
            let text = text.to_string();
            ensure_no_nul(field, &text)?;
            out.insert(key, text);
            continue;
        }
        if let Some(text) = value.clone().try_cast::<String>() {
            ensure_no_nul(field, &text)?;
            out.insert(key, text);
            continue;
        }
        return Err(facade_error(format!("all {field} values must be strings")));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{array_to_strings, ensure_not_blank, map_to_strings, normalize_timeout_ms};
    use rhai::{Array, Dynamic, Map};

    #[test]
    fn timeout_normalization_accepts_zero_and_positive() {
        assert_eq!(normalize_timeout_ms(0).expect("zero timeout"), None);
        assert_eq!(normalize_timeout_ms(1).expect("positive timeout"), Some(1));
    }

    #[test]
    fn timeout_normalization_rejects_negative_values() {
        assert!(normalize_timeout_ms(-1).is_err());
    }

    #[test]
    fn not_blank_trims_and_rejects_blank_values() {
        assert_eq!(
            ensure_not_blank("field", "  value  ").expect("trimmed value"),
            "value"
        );
        assert!(ensure_not_blank("field", "   ").is_err());
    }

    #[test]
    fn array_to_strings_rejects_non_string_values() {
        let values: Array = vec![Dynamic::from(1_i64)];
        assert!(array_to_strings("args", &values).is_err());
    }

    #[test]
    fn map_to_strings_rejects_non_string_values() {
        let mut map = Map::new();
        map.insert("k".into(), Dynamic::from(1_i64));
        assert!(map_to_strings("headers", &map).is_err());
    }
}
