use crate::rhai_facade_validation::{
    array_to_strings, ensure_no_nul, ensure_not_blank, map_to_strings, normalize_timeout_ms,
    RhaiResult,
};
use rhai::{Array, Dynamic, Engine, Map};
use std::time::Duration;

pub fn register(engine: &mut Engine) {
    engine.register_fn("proc_run", proc_run);
    engine.register_fn("proc_run_with", proc_run_with);
}

fn proc_run(exec: &str, args: Array, timeout_ms: i64) -> RhaiResult<Map> {
    proc_run_internal(exec, args, Map::new(), "".into(), timeout_ms)
}

fn proc_run_with(
    exec: &str,
    args: Array,
    env: Map,
    cwd: String,
    timeout_ms: i64,
) -> RhaiResult<Map> {
    proc_run_internal(exec, args, env, cwd, timeout_ms)
}

fn proc_run_internal(
    exec: &str,
    args: Array,
    env: Map,
    cwd: String,
    timeout_ms: i64,
) -> RhaiResult<Map> {
    let exec = ensure_not_blank("exec", exec)?;
    let args = array_to_strings("args", &args)?;
    let env = map_to_strings("env", &env)?;
    ensure_no_nul("cwd", &cwd)?;
    let timeout_ms = normalize_timeout_ms(timeout_ms)?;

    let mut expr = duct::cmd(&exec, args.clone());
    for (key, value) in env {
        expr = expr.env(key, value);
    }
    if !cwd.trim().is_empty() {
        expr = expr.dir(cwd.trim().to_string());
    }
    expr = expr.stdout_capture().stderr_capture().unchecked();

    let handle = expr
        .start()
        .map_err(|err| format!("failed to start process `{exec}`: {err}"))?;

    let mut timed_out = false;
    let output = if let Some(timeout_ms) = timeout_ms {
        match handle
            .wait_timeout(Duration::from_millis(timeout_ms))
            .map_err(|err| format!("failed while waiting for `{exec}`: {err}"))?
        {
            Some(output) => output.clone(),
            None => {
                timed_out = true;
                let _ = handle.kill();
                handle
                    .into_output()
                    .map_err(|err| format!("failed to collect output for `{exec}`: {err}"))?
            }
        }
    } else {
        handle
            .into_output()
            .map_err(|err| format!("failed to collect output for `{exec}`: {err}"))?
    };

    let mut out = Map::new();
    out.insert("ok".into(), Dynamic::from(output.status.success()));
    out.insert("timed_out".into(), Dynamic::from(timed_out));
    out.insert(
        "stdout".into(),
        Dynamic::from(String::from_utf8_lossy(&output.stdout).to_string()),
    );
    out.insert(
        "stderr".into(),
        Dynamic::from(String::from_utf8_lossy(&output.stderr).to_string()),
    );
    out.insert(
        "status".into(),
        Dynamic::from(output.status.code().unwrap_or_default() as i64),
    );
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::proc_run;
    use rhai::{Array, Dynamic};

    #[test]
    fn process_run_captures_stdout() {
        let args: Array = vec![Dynamic::from("-c"), Dynamic::from("echo hello")];
        let out = proc_run("sh", args, 1_000).expect("process run");
        assert!(out
            .get("ok")
            .and_then(|v| v.clone().try_cast::<bool>())
            .unwrap_or(false));
    }
}

