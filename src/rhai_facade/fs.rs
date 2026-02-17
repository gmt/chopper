use crate::rhai_facade_validation::{ensure_path, ensure_no_nul, RhaiResult};
use cap_std::ambient_authority;
use cap_std::fs::Dir;
use rhai::{Array, Dynamic, Engine, ImmutableString, Map};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub fn register_read_only(engine: &mut Engine) {
    engine.register_fn("fs_exists", fs_exists);
    engine.register_fn("fs_stat", fs_stat);
    engine.register_fn("fs_list", fs_list);
    engine.register_fn("fs_read_text", fs_read_text);
}

pub fn register_full(engine: &mut Engine) {
    register_read_only(engine);
    engine.register_fn("fs_write_text", fs_write_text);
    engine.register_fn("fs_mkdir", fs_mkdir);
    engine.register_fn("fs_remove", fs_remove);
}

fn fs_exists(path: &str) -> RhaiResult<bool> {
    let path = ensure_path("path", path)?;
    Ok(path.exists())
}

fn fs_stat(path: &str) -> RhaiResult<Map> {
    let path = ensure_path("path", path)?;
    let mut out = Map::new();
    out.insert("path".into(), Dynamic::from(path.display().to_string()));
    match fs_err::metadata(&path) {
        Ok(metadata) => {
            out.insert("exists".into(), Dynamic::from(true));
            out.insert("is_file".into(), Dynamic::from(metadata.is_file()));
            out.insert("is_dir".into(), Dynamic::from(metadata.is_dir()));
            out.insert("len".into(), Dynamic::from(metadata.len() as i64));
            out.insert(
                "readonly".into(),
                Dynamic::from(metadata.permissions().readonly()),
            );
            let modified = metadata
                .modified()
                .ok()
                .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            out.insert("modified_epoch_secs".into(), Dynamic::from(modified));
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            out.insert("exists".into(), Dynamic::from(false));
            out.insert("is_file".into(), Dynamic::from(false));
            out.insert("is_dir".into(), Dynamic::from(false));
            out.insert("len".into(), Dynamic::from(0_i64));
            out.insert("readonly".into(), Dynamic::from(false));
            out.insert("modified_epoch_secs".into(), Dynamic::from(0_i64));
        }
        Err(err) => return Err(format!("failed to stat {}: {err}", path.display()).into()),
    }
    Ok(out)
}

fn fs_list(path: &str) -> RhaiResult<Array> {
    let path = ensure_path("path", path)?;
    let dir = Dir::open_ambient_dir(&path, ambient_authority())
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut names = Vec::new();
    for entry in dir
        .entries()
        .map_err(|err| format!("failed to list {}: {err}", path.display()))?
    {
        let entry = entry.map_err(|err| format!("failed to list {}: {err}", path.display()))?;
        let name = entry.file_name();
        names.push(name.to_string_lossy().to_string());
    }
    names.sort();
    Ok(names
        .into_iter()
        .map(|name| Dynamic::from(ImmutableString::from(name)))
        .collect())
}

fn fs_read_text(path: &str) -> RhaiResult<String> {
    let path = ensure_path("path", path)?;
    let (dir, basename) = open_parent_dir(&path)?;
    let mut file = dir
        .open(&basename)
        .map_err(|err| format!("failed to open {}: {err}", path.display()))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|err| format!("failed to read {}: {err}", path.display()))?;
    Ok(text)
}

fn fs_write_text(path: &str, text: &str) -> RhaiResult<Map> {
    let path = ensure_path("path", path)?;
    ensure_no_nul("text", text)?;
    let (dir, basename) = open_parent_dir(&path)?;
    dir.write(&basename, text.as_bytes())
        .map_err(|err| format!("failed to write {}: {err}", path.display()))?;
    let mut out = Map::new();
    out.insert("ok".into(), Dynamic::from(true));
    out.insert("path".into(), Dynamic::from(path.display().to_string()));
    out.insert("bytes_written".into(), Dynamic::from(text.len() as i64));
    Ok(out)
}

fn fs_mkdir(path: &str, recursive: bool) -> RhaiResult<Map> {
    let path = ensure_path("path", path)?;
    if recursive {
        fs_err::create_dir_all(&path)
            .map_err(|err| format!("failed to create {}: {err}", path.display()))?;
    } else {
        let (parent, basename) = open_parent_dir(&path)?;
        parent
            .create_dir(&basename)
            .map_err(|err| format!("failed to create {}: {err}", path.display()))?;
    }
    let mut out = Map::new();
    out.insert("ok".into(), Dynamic::from(true));
    out.insert("path".into(), Dynamic::from(path.display().to_string()));
    out.insert("recursive".into(), Dynamic::from(recursive));
    Ok(out)
}

fn fs_remove(path: &str, recursive: bool) -> RhaiResult<Map> {
    let path = ensure_path("path", path)?;
    let metadata = match fs_err::metadata(&path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            let mut out = Map::new();
            out.insert("ok".into(), Dynamic::from(true));
            out.insert("removed".into(), Dynamic::from(false));
            out.insert("path".into(), Dynamic::from(path.display().to_string()));
            return Ok(out);
        }
        Err(err) => return Err(format!("failed to inspect {}: {err}", path.display()).into()),
    };

    if metadata.is_dir() {
        if recursive {
            fs_err::remove_dir_all(&path)
                .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        } else {
            let (parent, basename) = open_parent_dir(&path)?;
            parent
                .remove_dir(&basename)
                .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
        }
    } else {
        let (parent, basename) = open_parent_dir(&path)?;
        parent
            .remove_file(&basename)
            .map_err(|err| format!("failed to remove {}: {err}", path.display()))?;
    }

    let mut out = Map::new();
    out.insert("ok".into(), Dynamic::from(true));
    out.insert("removed".into(), Dynamic::from(true));
    out.insert("path".into(), Dynamic::from(path.display().to_string()));
    Ok(out)
}

fn open_parent_dir(path: &Path) -> RhaiResult<(Dir, PathBuf)> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let basename = path.file_name().ok_or_else(|| {
        format!(
            "path {} must include a terminal file or directory component",
            path.display()
        )
    })?;
    let dir = Dir::open_ambient_dir(parent, ambient_authority())
        .map_err(|err| format!("failed to open parent {}: {err}", parent.display()))?;
    Ok((dir, PathBuf::from(basename)))
}

#[cfg(test)]
mod tests {
    use super::{fs_exists, fs_read_text, fs_write_text};
    use tempfile::TempDir;

    #[test]
    fn read_write_round_trip() {
        let temp = TempDir::new().expect("tempdir");
        let file = temp.path().join("demo.txt");
        fs_write_text(file.to_str().expect("utf8 path"), "hello").expect("write");
        let text = fs_read_text(file.to_str().expect("utf8 path")).expect("read");
        assert_eq!(text, "hello");
        assert!(fs_exists(file.to_str().expect("utf8 path")).expect("exists"));
    }
}

