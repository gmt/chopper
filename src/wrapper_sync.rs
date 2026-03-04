use anyhow::{anyhow, Context, Result};
use std::env;
use std::path::{Path, PathBuf};

#[cfg(unix)]
use std::os::unix::fs::symlink;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrapperLocation {
    pub(crate) wrapper_dir: PathBuf,
    pub(crate) wrapper_path: PathBuf,
    pub(crate) dir_in_path: bool,
}

pub(crate) fn resolve_wrapper_location(alias: &str) -> Result<WrapperLocation> {
    let home = home_dir().ok_or_else(|| {
        anyhow!("unable to resolve HOME for wrapper sync; set HOME or pass --no-wrapper-sync")
    })?;

    let home_bin = home.join("bin");
    let local_bin = home.join(".local").join("bin");
    let path_entries = path_entries();

    let mut selected = local_bin.clone();
    let mut dir_in_path = false;
    for entry in &path_entries {
        if paths_match(entry, &home_bin) {
            selected = home_bin.clone();
            dir_in_path = true;
            break;
        }
        if paths_match(entry, &local_bin) {
            selected = local_bin.clone();
            dir_in_path = true;
            break;
        }
    }

    Ok(WrapperLocation {
        wrapper_path: selected.join(alias),
        wrapper_dir: selected,
        dir_in_path,
    })
}

pub(crate) fn ensure_wrapper(alias: &str) -> Result<Vec<String>> {
    let location = resolve_wrapper_location(alias)?;

    fs_err::create_dir_all(&location.wrapper_dir).with_context(|| {
        format!(
            "failed to create wrapper dir {}",
            location.wrapper_dir.display()
        )
    })?;

    let target = env::current_exe().context("failed to resolve current chopper executable path")?;

    match fs_err::symlink_metadata(&location.wrapper_path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(anyhow!(
                    "wrapper path `{}` exists and is not a symlink",
                    location.wrapper_path.display()
                ));
            }
            fs_err::remove_file(&location.wrapper_path).with_context(|| {
                format!(
                    "failed to replace wrapper symlink {}",
                    location.wrapper_path.display()
                )
            })?;
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
        Err(err) => {
            return Err(err).with_context(|| {
                format!(
                    "failed to inspect wrapper path {}",
                    location.wrapper_path.display()
                )
            })
        }
    }

    #[cfg(unix)]
    {
        symlink(&target, &location.wrapper_path).with_context(|| {
            format!(
                "failed to create wrapper symlink {} -> {}",
                location.wrapper_path.display(),
                target.display()
            )
        })?;
    }

    #[cfg(not(unix))]
    {
        let _ = target;
        return Err(anyhow!("wrapper sync requires unix symlink support"));
    }

    Ok(wrapper_health_warnings(alias))
}

pub(crate) fn remove_wrapper(alias: &str, explicit_path: Option<PathBuf>) -> Result<bool> {
    let path = if let Some(path) = explicit_path {
        path
    } else {
        resolve_wrapper_location(alias)?.wrapper_path
    };

    match fs_err::symlink_metadata(&path) {
        Ok(metadata) => {
            if !metadata.file_type().is_symlink() {
                return Err(anyhow!(
                    "wrapper cleanup only removes symlinks; `{}` is not a symlink",
                    path.display()
                ));
            }
            fs_err::remove_file(&path)
                .with_context(|| format!("failed to remove wrapper symlink {}", path.display()))?;
            Ok(true)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(err) => {
            Err(err).with_context(|| format!("failed to inspect wrapper {}", path.display()))
        }
    }
}

pub(crate) fn wrapper_health_warnings(alias: &str) -> Vec<String> {
    let location = match resolve_wrapper_location(alias) {
        Ok(location) => location,
        Err(err) => return vec![err.to_string()],
    };
    let mut warnings = Vec::new();

    if !location.dir_in_path {
        warnings.push(format!(
            "wrapper dir `{}` is not present in PATH",
            location.wrapper_dir.display()
        ));
    }

    let wrapper_is_symlink = match fs_err::symlink_metadata(&location.wrapper_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                true
            } else {
                warnings.push(format!(
                    "wrapper path `{}` exists but is not a symlink",
                    location.wrapper_path.display()
                ));
                false
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            warnings.push(format!(
                "no wrapper symlink found at `{}`",
                location.wrapper_path.display()
            ));
            false
        }
        Err(err) => {
            warnings.push(format!(
                "failed to inspect wrapper path `{}`: {}",
                location.wrapper_path.display(),
                err
            ));
            false
        }
    };

    if wrapper_is_symlink && location.dir_in_path {
        match which::which(alias) {
            Ok(first_hit) => {
                if !paths_match(&first_hit, &location.wrapper_path) {
                    warnings.push(format!(
                        "wrapper `{}` is shadowed by `{}` earlier in PATH",
                        location.wrapper_path.display(),
                        first_hit.display()
                    ));
                }
            }
            Err(_) => warnings.push(format!("`{alias}` is not currently resolvable on PATH")),
        }
    }

    warnings
}

fn path_entries() -> Vec<PathBuf> {
    let Some(path) = env::var_os("PATH") else {
        return Vec::new();
    };
    env::split_paths(&path).collect()
}

fn paths_match(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (fs_err::canonicalize(left), fs_err::canonicalize(right)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(|| directories::BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::{ensure_wrapper, resolve_wrapper_location, wrapper_health_warnings};
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use tempfile::TempDir;

    struct EnvRestore {
        home: Option<OsString>,
        path: Option<OsString>,
    }

    impl EnvRestore {
        fn capture() -> Self {
            Self {
                home: env::var_os("HOME"),
                path: env::var_os("PATH"),
            }
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            match &self.home {
                Some(value) => env::set_var("HOME", value),
                None => env::remove_var("HOME"),
            }
            match &self.path {
                Some(value) => env::set_var("PATH", value),
                None => env::remove_var("PATH"),
            }
        }
    }

    #[test]
    fn selects_first_candidate_directory_present_on_path() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let _restore = EnvRestore::capture();
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let home_bin = home.join("bin");
        let local_bin = home.join(".local/bin");
        fs::create_dir_all(&home_bin).expect("home/bin");
        fs::create_dir_all(&local_bin).expect("home/.local/bin");

        env::set_var("HOME", &home);
        env::set_var(
            "PATH",
            format!("{}:{}", local_bin.display(), home_bin.display()),
        );
        let location = resolve_wrapper_location("demo").expect("resolve wrapper location");
        assert_eq!(location.wrapper_dir, local_bin);
        assert!(location.dir_in_path);
    }

    #[test]
    fn falls_back_to_local_bin_when_candidates_are_missing_from_path() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let _restore = EnvRestore::capture();
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let local_bin = home.join(".local/bin");

        env::set_var("HOME", &home);
        env::set_var("PATH", "/usr/bin:/bin");
        let location = resolve_wrapper_location("demo").expect("resolve wrapper location");
        assert_eq!(location.wrapper_dir, local_bin);
        assert!(!location.dir_in_path);
    }

    #[test]
    fn ensure_wrapper_creates_missing_wrapper_directory() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let _restore = EnvRestore::capture();
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let local_bin = home.join(".local/bin");

        env::set_var("HOME", &home);
        env::set_var("PATH", "/usr/bin:/bin");
        let warnings = ensure_wrapper("ensured").expect("ensure wrapper");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("not present in PATH")),
            "{warnings:?}"
        );
        let wrapper = local_bin.join("ensured");
        let metadata = fs::symlink_metadata(&wrapper).expect("wrapper metadata");
        assert!(metadata.file_type().is_symlink());
    }

    #[test]
    fn wrapper_health_warns_when_wrapper_is_shadowed() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let _restore = EnvRestore::capture();
        let temp = TempDir::new().expect("tempdir");
        let home = temp.path().join("home");
        let local_bin = home.join(".local/bin");
        let shadow_dir = temp.path().join("shadow");
        fs::create_dir_all(&local_bin).expect("local bin");
        fs::create_dir_all(&shadow_dir).expect("shadow dir");

        env::set_var("HOME", &home);
        env::set_var(
            "PATH",
            format!("{}:{}:/usr/bin", shadow_dir.display(), local_bin.display()),
        );

        ensure_wrapper("shadowed").expect("create wrapper");
        let shadow_target = shadow_dir.join("shadowed");
        fs::write(&shadow_target, "#!/bin/sh\nexit 0\n").expect("write shadow executable");
        let mut perms = fs::metadata(&shadow_target)
            .expect("shadow metadata")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&shadow_target, perms).expect("chmod shadow executable");

        let warnings = wrapper_health_warnings("shadowed");
        assert!(
            warnings
                .iter()
                .any(|warning| warning.contains("shadowed by")),
            "{warnings:?}"
        );
    }
}
