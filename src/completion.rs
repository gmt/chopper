use crate::manifest::Manifest;
use crate::rhai_engine::{build_engine, RhaiEngineProfile};
use anyhow::{anyhow, Context, Result};
use rhai::{Array, Dynamic, ImmutableString, Map, Scope};
use std::collections::HashMap;

/// Run the Rhai completion function for an alias and return candidate strings.
///
/// The Rhai function receives a context map with:
/// - `words`: Array of strings (COMP_WORDS from bash)
/// - `cword`: Integer (0-based index of word being completed)
/// - `current`: String (the partial word being completed)
/// - `exec`: String (resolved exec path for the alias)
/// - `alias_args`: Array of strings (alias's configured args)
/// - `alias_env`: Map of string->string (alias's configured env)
///
/// The function must return an array of candidate strings.
pub fn run_complete(manifest: &Manifest, words: &[String], cword: usize) -> Result<Vec<String>> {
    let bashcomp = manifest
        .bashcomp
        .as_ref()
        .ok_or_else(|| anyhow!("alias has no [bashcomp] configuration"))?;

    let rhai_script = bashcomp
        .rhai_script
        .as_ref()
        .ok_or_else(|| anyhow!("alias has no bashcomp.rhai_script configured"))?;

    let function_name = bashcomp.rhai_function.as_deref().unwrap_or("complete");

    let engine = build_engine(RhaiEngineProfile::Completion);
    let mut scope = Scope::new();
    let ast = engine.compile_file(rhai_script.clone()).with_context(|| {
        format!(
            "failed to compile completion script {}",
            rhai_script.display()
        )
    })?;

    let context = build_completion_context(manifest, words, cword);
    let response: Dynamic = engine
        .call_fn(&mut scope, &ast, function_name, (context,))
        .with_context(|| {
            format!(
                "completion function `{function_name}` failed in {}",
                rhai_script.display()
            )
        })?;

    parse_candidates(response)
}

fn build_completion_context(manifest: &Manifest, words: &[String], cword: usize) -> Map {
    let mut ctx = Map::new();
    ctx.insert("words".into(), to_array(words));
    ctx.insert("cword".into(), Dynamic::from(cword as i64));

    let current = words.get(cword).cloned().unwrap_or_default();
    ctx.insert(
        "current".into(),
        Dynamic::from(ImmutableString::from(current)),
    );

    ctx.insert(
        "exec".into(),
        Dynamic::from(ImmutableString::from(
            manifest.exec.to_string_lossy().to_string(),
        )),
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

fn parse_candidates(value: Dynamic) -> Result<Vec<String>> {
    let arr = value
        .try_cast::<Array>()
        .ok_or_else(|| anyhow!("completion function must return an array"))?;

    let mut candidates = Vec::with_capacity(arr.len());
    for item in arr {
        let text = dynamic_to_string(item)?;
        // Silently skip candidates containing NUL bytes.
        if !text.contains('\0') {
            candidates.push(text);
        }
    }
    Ok(candidates)
}

fn dynamic_to_string(value: Dynamic) -> Result<String> {
    if let Some(text) = value.clone().try_cast::<ImmutableString>() {
        return Ok(text.to_string());
    }
    if let Some(text) = value.try_cast::<String>() {
        return Ok(text);
    }
    Err(anyhow!(
        "all values returned by the completion function must be strings"
    ))
}

#[cfg(test)]
mod tests {
    use super::run_complete;
    use crate::manifest::{BashcompConfig, Manifest};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::TempDir;

    #[test]
    fn completion_function_returns_candidates() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn complete(ctx) {
    ["alpha", "beta", "gamma"]
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let candidates =
            run_complete(&manifest, &["myalias".into(), "b".into()], 1).expect("completion call");
        assert_eq!(candidates, vec!["alpha", "beta", "gamma"]);
    }

    #[test]
    fn completion_function_receives_context() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn complete(ctx) {
    let out = [];
    out.push("words=" + ctx.words.len());
    out.push("cword=" + ctx.cword);
    out.push("current=" + ctx.current);
    out.push("exec=" + ctx.exec);
    out.push("alias_args=" + ctx.alias_args.len());
    out
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("/usr/bin/kubectl"));
        manifest.args = vec!["get".into(), "pods".into()];
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let candidates = run_complete(&manifest, &["kpods".into(), "get".into(), "po".into()], 2)
            .expect("completion call");
        assert_eq!(
            candidates,
            vec![
                "words=3",
                "cword=2",
                "current=po",
                "exec=/usr/bin/kubectl",
                "alias_args=2"
            ]
        );
    }

    #[test]
    fn completion_function_uses_custom_function_name() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn my_completer(ctx) {
    ["custom"]
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: Some("my_completer".into()),
        });

        let candidates = run_complete(&manifest, &["myalias".into()], 0).expect("completion call");
        assert_eq!(candidates, vec!["custom"]);
    }

    #[test]
    fn completion_rejects_non_array_return() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn complete(_ctx) {
    "not-an-array"
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let err =
            run_complete(&manifest, &["myalias".into()], 0).expect_err("expected non-array error");
        assert!(err.to_string().contains("must return an array"), "{err}");
    }

    #[test]
    fn completion_rejects_non_string_elements() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn complete(_ctx) {
    ["ok", 42]
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let err =
            run_complete(&manifest, &["myalias".into()], 0).expect_err("expected non-string error");
        assert!(err.to_string().contains("must be strings"), "{err}");
    }

    #[test]
    fn completion_skips_nul_byte_candidates() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(
            &script_path,
            r#"
fn complete(_ctx) {
    ["good", "bad\x00value", "also-good"]
}
"#,
        )
        .expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let candidates = run_complete(&manifest, &["myalias".into()], 0).expect("completion call");
        assert_eq!(candidates, vec!["good", "also-good"]);
    }

    #[test]
    fn completion_errors_without_bashcomp_config() {
        let manifest = Manifest::simple(PathBuf::from("echo"));
        let err = run_complete(&manifest, &["myalias".into()], 0)
            .expect_err("expected missing config error");
        assert!(
            err.to_string().contains("no [bashcomp] configuration"),
            "{err}"
        );
    }

    #[test]
    fn completion_errors_without_rhai_script() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: None,
            rhai_function: None,
        });

        let err = run_complete(&manifest, &["myalias".into()], 0)
            .expect_err("expected missing rhai_script error");
        assert!(err.to_string().contains("no bashcomp.rhai_script"), "{err}");
    }

    #[test]
    fn completion_errors_on_missing_function() {
        let dir = TempDir::new().expect("tempdir");
        let script_path = dir.path().join("comp.rhai");
        fs::write(&script_path, "fn other(_ctx) { [] }\n").expect("write script");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.bashcomp = Some(BashcompConfig {
            disabled: false,
            passthrough: false,
            script: None,
            rhai_script: Some(script_path),
            rhai_function: None,
        });

        let err = run_complete(&manifest, &["myalias".into()], 0)
            .expect_err("expected missing function error");
        assert!(err.to_string().contains("complete"), "{err}");
    }
}
