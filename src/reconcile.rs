use crate::manifest::{Manifest, RuntimePatch};
use anyhow::{anyhow, Context, Result};
use rhai::{Array, Dynamic, Engine, ImmutableString, Map, Scope};
use std::collections::HashMap;
use std::env;

pub fn maybe_reconcile(
    manifest: &Manifest,
    runtime_args: &[String],
) -> Result<Option<RuntimePatch>> {
    let Some(reconcile) = manifest.reconcile.as_ref() else {
        return Ok(None);
    };

    let engine = Engine::new();
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

    let replace_args = optional_string_array(&map, "replace_args")?;
    let append_args = optional_string_array(&map, "append_args")?.unwrap_or_default();
    let set_env = optional_string_map(&map, "set_env")?.unwrap_or_default();
    let remove_env = optional_string_array(&map, "remove_env")?.unwrap_or_default();

    Ok(RuntimePatch {
        replace_args,
        append_args,
        set_env,
        remove_env,
    })
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
    use super::maybe_reconcile;
    use crate::manifest::{Manifest, ReconcileConfig};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn reconcile_patch_is_applied_from_rhai_script() {
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
    fn reconcile_requires_map_return_type() {
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
}
