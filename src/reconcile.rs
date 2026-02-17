use crate::arg_validation::{self, ArgViolation};
use crate::env_util;
use crate::env_validation::{self, EnvKeyViolation, EnvValueViolation};
use crate::manifest::{Manifest, RuntimePatch};
use crate::rhai_engine::{build_engine, RhaiEngineProfile};
use anyhow::{anyhow, Context, Result};
use rhai::{Array, Dynamic, ImmutableString, Map, Scope};
use std::collections::{HashMap, HashSet};
use std::env;

pub fn maybe_reconcile(
    manifest: &Manifest,
    runtime_args: &[String],
) -> Result<Option<RuntimePatch>> {
    if reconcile_disabled() {
        return Ok(None);
    }

    let Some(reconcile) = manifest.reconcile.as_ref() else {
        return Ok(None);
    };

    let engine = build_engine(RhaiEngineProfile::Reconcile);
    let mut scope = Scope::new();
    let ast = engine
        .compile_file(reconcile.script.clone())
        .with_context(|| {
            format!(
                "failed to compile reconcile script {}",
                reconcile.script.display()
            )
        })?;

    let context = build_context(manifest, runtime_args);
    let response: Dynamic = engine
        .call_fn(&mut scope, &ast, &reconcile.function, (context,))
        .with_context(|| {
            format!(
                "reconcile function `{}` failed in {}",
                reconcile.function,
                reconcile.script.display()
            )
        })?;

    parse_patch(response).map(Some)
}

fn reconcile_disabled() -> bool {
    env_util::env_flag_enabled("CHOPPER_DISABLE_RECONCILE")
}

fn build_context(manifest: &Manifest, runtime_args: &[String]) -> Map {
    let mut ctx = Map::new();
    ctx.insert("runtime_args".into(), to_array(runtime_args));
    ctx.insert(
        "runtime_env".into(),
        to_string_map(env::vars().collect::<HashMap<_, _>>()),
    );
    ctx.insert("alias_args".into(), to_array(&manifest.args));
    ctx.insert("alias_env".into(), to_string_map(manifest.env.clone()));
    ctx
}

fn to_array(values: &[String]) -> Dynamic {
    let arr: Array = values
        .iter()
        .cloned()
        .map(|value| Dynamic::from(ImmutableString::from(value)))
        .collect();
    Dynamic::from(arr)
}

fn to_string_map(values: HashMap<String, String>) -> Dynamic {
    let mut out = Map::new();
    for (k, v) in values {
        out.insert(k.into(), Dynamic::from(ImmutableString::from(v)));
    }
    Dynamic::from(out)
}

fn parse_patch(value: Dynamic) -> Result<RuntimePatch> {
    let map = value
        .try_cast::<Map>()
        .ok_or_else(|| anyhow!("reconcile function must return an object/map"))?;
    validate_patch_keys(&map)?;

    let replace_args = optional_string_array(&map, "replace_args")?
        .map(|values| normalize_patch_args(values, "replace_args"))
        .transpose()?;
    let append_args = normalize_patch_args(
        optional_string_array(&map, "append_args")?.unwrap_or_default(),
        "append_args",
    )?;
    let set_env =
        normalize_patch_set_env(optional_string_map(&map, "set_env")?.unwrap_or_default())?;
    let remove_env =
        normalize_patch_remove_env(optional_string_array(&map, "remove_env")?.unwrap_or_default())?;

    Ok(RuntimePatch {
        replace_args,
        append_args,
        set_env,
        remove_env,
    })
}

fn validate_patch_keys(map: &Map) -> Result<()> {
    for key in map.keys() {
        let supported = matches!(
            key.as_str(),
            "append_args" | "replace_args" | "set_env" | "remove_env"
        );
        if !supported {
            return Err(anyhow!(
                "unsupported reconcile patch key `{}`; supported keys: append_args, replace_args, set_env, remove_env",
                key
            ));
        }
    }
    Ok(())
}

fn optional_string_array(map: &Map, key: &str) -> Result<Option<Vec<String>>> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    let arr = value
        .clone()
        .try_cast::<Array>()
        .ok_or_else(|| anyhow!("`{key}` must be an array"))?;

    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        out.push(dynamic_to_string(item, key)?);
    }
    Ok(Some(out))
}

fn optional_string_map(map: &Map, key: &str) -> Result<Option<HashMap<String, String>>> {
    let Some(value) = map.get(key) else {
        return Ok(None);
    };
    let inner = value
        .clone()
        .try_cast::<Map>()
        .ok_or_else(|| anyhow!("`{key}` must be an object/map"))?;

    let mut out = HashMap::with_capacity(inner.len());
    for (k, v) in inner {
        out.insert(k.to_string(), dynamic_to_string(v, key)?);
    }
    Ok(Some(out))
}

fn normalize_patch_set_env(values: HashMap<String, String>) -> Result<HashMap<String, String>> {
    let mut normalized = HashMap::with_capacity(values.len());
    for (key, value) in values {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            return Err(anyhow!("`set_env` cannot contain empty keys"));
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!(
                    "`set_env` keys cannot contain `=`: `{normalized_key}`"
                ));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!("`set_env` keys cannot contain NUL bytes"));
            }
        }
        if matches!(
            env_validation::validate_env_value(&value),
            Err(EnvValueViolation::ContainsNul)
        ) {
            return Err(anyhow!(
                "`set_env` values cannot contain NUL bytes for key `{normalized_key}`"
            ));
        }
        if normalized.contains_key(normalized_key) {
            return Err(anyhow!(
                "`set_env` contains duplicate keys after trimming: `{normalized_key}`"
            ));
        }
        normalized.insert(normalized_key.to_string(), value);
    }
    Ok(normalized)
}

fn normalize_patch_args(values: Vec<String>, field: &str) -> Result<Vec<String>> {
    for value in &values {
        if matches!(
            arg_validation::validate_arg_value(value),
            Err(ArgViolation::ContainsNul)
        ) {
            return Err(anyhow!("`{field}` entries cannot contain NUL bytes"));
        }
    }
    Ok(values)
}

fn normalize_patch_remove_env(values: Vec<String>) -> Result<Vec<String>> {
    let mut seen = HashSet::with_capacity(values.len());
    let mut normalized = Vec::with_capacity(values.len());
    for key in values {
        let normalized_key = key.trim();
        if normalized_key.is_empty() {
            continue;
        }
        match env_validation::validate_env_key(normalized_key) {
            Ok(()) => {}
            Err(EnvKeyViolation::ContainsEquals) => {
                return Err(anyhow!(
                    "`remove_env` entries cannot contain `=`: `{normalized_key}`"
                ));
            }
            Err(EnvKeyViolation::ContainsNul) => {
                return Err(anyhow!("`remove_env` entries cannot contain NUL bytes"));
            }
        }
        let normalized_key = normalized_key.to_string();
        if seen.insert(normalized_key.clone()) {
            normalized.push(normalized_key);
        }
    }
    Ok(normalized)
}

fn dynamic_to_string(value: Dynamic, field: &str) -> Result<String> {
    if let Some(text) = value.clone().try_cast::<ImmutableString>() {
        return Ok(text.to_string());
    }
    if let Some(text) = value.try_cast::<String>() {
        return Ok(text);
    }
    Err(anyhow!("all values in `{field}` must be strings"))
}

#[cfg(test)]
mod tests {
    use super::{
        maybe_reconcile, normalize_patch_remove_env, normalize_patch_set_env, reconcile_disabled,
    };
    use crate::manifest::{Manifest, ReconcileConfig};
    use crate::test_support::ENV_LOCK;
    use std::collections::HashMap;
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn reconcile_patch_is_applied_from_rhai_script() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("patch.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(ctx) {
  let out = #{};
  if ctx.runtime_args.contains("--verbose") {
    out["append_args"] = ["-v"];
  }
  out["set_env"] = #{ "RUNTIME_MODE": "true" };
  out["remove_env"] = ["OLD_VAR"];
  out
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &["--verbose".into()])
            .expect("reconcile call")
            .expect("patch present");

        assert_eq!(patch.append_args, vec!["-v"]);
        assert_eq!(patch.set_env.get("RUNTIME_MODE"), Some(&"true".to_string()));
        assert_eq!(patch.remove_env, vec!["OLD_VAR"]);
    }

    #[test]
    fn reconcile_accepts_empty_unicode_and_whitespace_arg_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("arg-shapes.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    replace_args: ["", "emoji🚀", " spaced value "],
    append_args: ["tail", ""]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(
            patch.replace_args,
            Some(vec![
                "".to_string(),
                "emoji🚀".to_string(),
                " spaced value ".to_string()
            ])
        );
        assert_eq!(patch.append_args, vec!["tail".to_string(), "".to_string()]);
    }

    #[test]
    fn reconcile_accepts_symbolic_and_pathlike_arg_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("arg-symbols.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    replace_args: [
      "--replace=value",
      "../relative/path",
      "semi;colon&and"
    ],
    append_args: [
      "$DOLLAR",
      "brace{value}",
      "windows\\path"
    ]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(
            patch.replace_args,
            Some(vec![
                "--replace=value".to_string(),
                "../relative/path".to_string(),
                "semi;colon&and".to_string()
            ])
        );
        assert_eq!(
            patch.append_args,
            vec![
                "$DOLLAR".to_string(),
                "brace{value}".to_string(),
                r"windows\path".to_string()
            ]
        );
    }

    #[test]
    fn reconcile_accepts_empty_and_unicode_set_env_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("env-shapes.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{
      "EMPTY_VALUE": "",
      "UNICODE_VALUE": "emoji🚀",
      " SPACED_KEY ": " spaced value "
    }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(patch.set_env.get("EMPTY_VALUE"), Some(&"".to_string()));
        assert_eq!(
            patch.set_env.get("UNICODE_VALUE"),
            Some(&"emoji🚀".to_string())
        );
        assert_eq!(
            patch.set_env.get("SPACED_KEY"),
            Some(&" spaced value ".to_string())
        );
    }

    #[test]
    fn reconcile_accepts_symbolic_and_pathlike_set_env_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("env-symbols.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{
      "CHOPPER_EQ": "--flag=value",
      "CHOPPER_REL": "../relative/path",
      "CHOPPER_SHELL": "semi;colon&and",
      "CHOPPER_DOLLAR": "$DOLLAR",
      " CHOPPER_BRACE ": "brace{value}",
      "CHOPPER_WIN": "windows\\path"
    }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(
            patch.set_env.get("CHOPPER_EQ"),
            Some(&"--flag=value".to_string())
        );
        assert_eq!(
            patch.set_env.get("CHOPPER_REL"),
            Some(&"../relative/path".to_string())
        );
        assert_eq!(
            patch.set_env.get("CHOPPER_SHELL"),
            Some(&"semi;colon&and".to_string())
        );
        assert_eq!(
            patch.set_env.get("CHOPPER_DOLLAR"),
            Some(&"$DOLLAR".to_string())
        );
        assert_eq!(
            patch.set_env.get("CHOPPER_BRACE"),
            Some(&"brace{value}".to_string())
        );
        assert_eq!(
            patch.set_env.get("CHOPPER_WIN"),
            Some(&r"windows\path".to_string())
        );
    }

    #[test]
    fn reconcile_accepts_symbolic_and_pathlike_set_env_keys() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("env-key-symbols.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{
      " KEY-WITH-DASH ": "dash",
      "KEY.WITH.DOT": "dot",
      "KEY/WITH/SLASH": "slash",
      "KEY\\WITH\\BACKSLASH": "backslash"
    }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(
            patch.set_env.get("KEY-WITH-DASH"),
            Some(&"dash".to_string())
        );
        assert_eq!(patch.set_env.get("KEY.WITH.DOT"), Some(&"dot".to_string()));
        assert_eq!(
            patch.set_env.get("KEY/WITH/SLASH"),
            Some(&"slash".to_string())
        );
        assert_eq!(
            patch.set_env.get(r"KEY\WITH\BACKSLASH"),
            Some(&"backslash".to_string())
        );
    }

    #[test]
    fn reconcile_requires_map_return_type() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("invalid-return.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  "not-a-map"
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected reconcile return shape error")
            .to_string();
        assert!(err.contains("reconcile function must return an object/map"));
    }

    #[test]
    fn reconcile_rejects_non_string_env_values() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("invalid-env.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "BROKEN": 42 }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected reconcile field validation error")
            .to_string();
        assert!(err.contains("all values in `set_env` must be strings"));
    }

    #[test]
    fn reconcile_rejects_append_args_entries_containing_nul_bytes() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("invalid-append-args.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    append_args: ["ok", "bad\x00arg"]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected append_args validation error")
            .to_string();
        assert!(
            err.contains("`append_args` entries cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_replace_args_entries_containing_nul_bytes() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("invalid-replace-args.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    replace_args: ["ok", "bad\x00arg"]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected replace_args validation error")
            .to_string();
        assert!(
            err.contains("`replace_args` entries cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_disable_flag_is_optional_and_case_insensitive() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        assert!(!reconcile_disabled());

        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n1\r\n");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "TRUE");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "on");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "  yEs  ");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nYeS\r\n");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nTrUe\r\n");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{00A0}TrUe\u{00A0}");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{3000}TrUe\u{3000}");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n\u{00A0}TrUe\u{00A0}\r\n");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "  ON  ");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nOn\r\n");
        assert!(reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "0");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n0\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "false");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{00A0}FaLsE\u{00A0}");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{3000}FaLsE\u{3000}");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n\u{00A0}FaLsE\u{00A0}\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nFaLsE\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "no");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nNo\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "off");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\noFf\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n   \r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n\t \r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", " ");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\t\t");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "definitely-not");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nmaybe\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "ＴＲＵＥ");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\nＴＲＵＥ\r\n");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "Ｔrue");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{00A0}ＴＲＵＥ\u{00A0}");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\u{3000}ＴＲＵＥ\u{3000}");
        assert!(!reconcile_disabled());
        env::set_var("CHOPPER_DISABLE_RECONCILE", "\r\n\u{00A0}Ｔrue\u{00A0}\r\n");
        assert!(!reconcile_disabled());
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
    }

    #[test]
    fn reconcile_can_be_disabled_to_skip_scripts() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("patch.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    append_args: ["never-applied"]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        env::set_var("CHOPPER_DISABLE_RECONCILE", "1");
        let patch = maybe_reconcile(&manifest, &[]).expect("skip reconcile");
        assert!(patch.is_none());
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
    }

    #[test]
    fn reconcile_trims_and_normalizes_remove_env_entries() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("remove-env-trim.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    remove_env: ["  FOO  ", "FOO", "   ", "BAR", " BAR "]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(patch.remove_env, vec!["FOO", "BAR"]);
    }

    #[test]
    fn reconcile_preserves_symbolic_and_pathlike_remove_env_entries() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("remove-env-symbols.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    remove_env: [
      " KEY-WITH-DASH ",
      "KEY.WITH.DOT",
      "KEY/WITH/SLASH",
      "KEY\\WITH\\BACKSLASH",
      "KEY/WITH/SLASH"
    ]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let patch = maybe_reconcile(&manifest, &[])
            .expect("reconcile call")
            .expect("patch present");
        assert_eq!(
            patch.remove_env,
            vec![
                "KEY-WITH-DASH".to_string(),
                "KEY.WITH.DOT".to_string(),
                "KEY/WITH/SLASH".to_string(),
                r"KEY\WITH\BACKSLASH".to_string()
            ]
        );
    }

    #[test]
    fn reconcile_rejects_remove_env_entries_containing_equals_sign() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("remove-env-equals.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    remove_env: ["BAD=KEY"]
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected remove_env validation error")
            .to_string();
        assert!(
            err.contains("`remove_env` entries cannot contain `=`"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_remove_env_entries_containing_nul_bytes() {
        let err = normalize_patch_remove_env(vec!["BAD\0KEY".to_string()])
            .expect_err("expected remove_env validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_blank_set_env_keys_after_trimming() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("invalid-set-env-key.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "   ": "value" }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected set_env key validation error")
            .to_string();
        assert!(err.contains("`set_env` cannot contain empty keys"), "{err}");
    }

    #[test]
    fn reconcile_rejects_duplicate_set_env_keys_after_trimming() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("duplicate-set-env-key.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "FOO": "a", " FOO ": "b" }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected set_env duplicate validation error")
            .to_string();
        assert!(
            err.contains("`set_env` contains duplicate keys after trimming"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_set_env_keys_containing_equals_sign() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("equals-set-env-key.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    set_env: #{ "BAD=KEY": "value" }
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected set_env key validation error")
            .to_string();
        assert!(err.contains("keys cannot contain `=`"), "{err}");
    }

    #[test]
    fn reconcile_rejects_set_env_keys_containing_nul_bytes() {
        let err = normalize_patch_set_env(HashMap::from([(
            "BAD\0KEY".to_string(),
            "value".to_string(),
        )]))
        .expect_err("expected set_env key validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_set_env_values_containing_nul_bytes() {
        let err = normalize_patch_set_env(HashMap::from([(
            "GOOD_KEY".to_string(),
            "bad\0value".to_string(),
        )]))
        .expect_err("expected set_env value validation error");
        assert!(
            err.to_string().contains("cannot contain NUL bytes"),
            "{err}"
        );
    }

    #[test]
    fn reconcile_rejects_unsupported_patch_keys() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        env::remove_var("CHOPPER_DISABLE_RECONCILE");
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("unsupported-key.rhai");
        fs::write(
            &script_path,
            r#"
fn reconcile(_ctx) {
  #{
    bogus_key: "value"
  }
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.reconcile = Some(ReconcileConfig {
            script: script_path,
            function: "reconcile".into(),
        });

        let err = maybe_reconcile(&manifest, &[])
            .expect_err("expected unsupported key validation error")
            .to_string();
        assert!(
            err.contains("unsupported reconcile patch key `bogus_key`"),
            "{err}"
        );
    }
}
