use crate::rhai_facade_validation::{ensure_path, RhaiResult};
use rhai::{Dynamic, Engine, ImmutableString, Map};

pub fn register(engine: &mut Engine) {
    engine.register_fn("platform_info", platform_info);
    engine.register_fn("platform_is_unix", platform_is_unix);
    engine.register_fn("executable_intent", executable_intent);
    engine.register_fn(
        "can_execute_without_confirmation",
        can_execute_without_confirmation,
    );
    engine.register_fn(
        "can_execute_with_confirmation",
        can_execute_with_confirmation,
    );
}

fn platform_info() -> Map {
    let mut out = Map::new();
    out.insert(
        "os".into(),
        Dynamic::from(ImmutableString::from(std::env::consts::OS)),
    );
    out.insert(
        "family".into(),
        Dynamic::from(ImmutableString::from(std::env::consts::FAMILY)),
    );
    out.insert(
        "arch".into(),
        Dynamic::from(ImmutableString::from(std::env::consts::ARCH)),
    );
    out.insert(
        "exe_suffix".into(),
        Dynamic::from(ImmutableString::from(std::env::consts::EXE_SUFFIX)),
    );
    out.insert("supports_posix_mode_bits".into(), Dynamic::from(true));
    out
}

fn platform_is_unix() -> bool {
    true
}

fn executable_intent(path: &str) -> RhaiResult<Map> {
    let path = ensure_path("path", path)?;
    let mut out = Map::new();
    let metadata = match fs_err::metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(format!("failed to inspect {}: {err}", path.display()).into()),
    };

    out.insert("path".into(), Dynamic::from(path.display().to_string()));
    out.insert("exists".into(), Dynamic::from(metadata.is_some()));

    let is_file = metadata.as_ref().is_some_and(|m| m.is_file());
    let is_dir = metadata.as_ref().is_some_and(|m| m.is_dir());
    out.insert("is_file".into(), Dynamic::from(is_file));
    out.insert("is_dir".into(), Dynamic::from(is_dir));

    let can_without = can_execute_without_confirmation_impl(&path, metadata.as_ref());
    out.insert(
        "can_execute_without_confirmation".into(),
        Dynamic::from(can_without),
    );
    out.insert(
        "can_execute_with_confirmation".into(),
        Dynamic::from(can_without),
    );
    out.insert("requires_user_confirmation".into(), Dynamic::from(false));
    Ok(out)
}

fn can_execute_without_confirmation(path: &str) -> RhaiResult<bool> {
    let path = ensure_path("path", path)?;
    let metadata = match fs_err::metadata(&path) {
        Ok(metadata) => Some(metadata),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => return Err(format!("failed to inspect {}: {err}", path.display()).into()),
    };
    Ok(can_execute_without_confirmation_impl(
        &path,
        metadata.as_ref(),
    ))
}

fn can_execute_with_confirmation(path: &str) -> RhaiResult<bool> {
    can_execute_without_confirmation(path)
}

fn can_execute_without_confirmation_impl(
    _path: &std::path::Path,
    metadata: Option<&std::fs::Metadata>,
) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Some(metadata) = metadata else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    metadata.permissions().mode() & 0o111 != 0
}
