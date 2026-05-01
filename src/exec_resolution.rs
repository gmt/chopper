use std::env;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    dev: u64,
    ino: u64,
}

pub(crate) fn resolve_command_path(command: &str) -> PathBuf {
    let Ok(candidates) = which::which_all(command) else {
        return PathBuf::from(command);
    };

    if is_direct_chopper_name(command) {
        return candidates
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from(command));
    }

    let skip_identities = skip_identities();
    let mut first_hit: Option<PathBuf> = None;

    for candidate in candidates {
        if first_hit.is_none() {
            first_hit = Some(candidate.clone());
        }

        // Heuristic: skip PATH hits that resolve to this currently-running
        // chopper binary so we can fall through to the next command candidate.
        //
        // NOTE: once we support same-named aliases in multiple PATH locations,
        // this may be too naive: legitimate multi-wrapper chains can exist and
        // selecting the "right" layer may require richer wrapper metadata.
        if matches_any_binary_identity(&skip_identities, &candidate) {
            continue;
        }
        return candidate;
    }

    first_hit.unwrap_or_else(|| PathBuf::from(command))
}

fn is_direct_chopper_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("chopper")
}

fn skip_identities() -> Vec<FileIdentity> {
    let mut identities = Vec::new();
    if let Ok(current) = env::current_exe() {
        if let Some(identity) = file_identity(&current) {
            identities.push(identity);
        }
    }
    if let Some(extra_paths) = env::var_os("CHOPPER_SKIP_EXEC_IDENTITY") {
        for path in env::split_paths(&extra_paths) {
            if let Some(identity) = file_identity(&path) {
                identities.push(identity);
            }
        }
    }
    identities
}

fn matches_any_binary_identity(skip_identities: &[FileIdentity], path: &Path) -> bool {
    let Some(identity) = file_identity(path) else {
        return false;
    };
    skip_identities.contains(&identity)
}

fn file_identity(path: &Path) -> Option<FileIdentity> {
    let metadata = fs::metadata(path).ok()?;
    Some(FileIdentity {
        dev: metadata.dev(),
        ino: metadata.ino(),
    })
}

#[cfg(test)]
mod tests {
    use super::resolve_command_path;
    use crate::test_support::ENV_LOCK;
    use std::env;
    use std::fs;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use tempfile::TempDir;

    fn write_executable_script(path: &std::path::Path, body: &str) {
        fs::write(path, body).expect("write executable script");
        let mut perms = fs::metadata(path).expect("script metadata").permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms).expect("set executable permissions");
    }

    #[test]
    fn skips_self_referential_wrapper_path_hits() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let old_path = env::var("PATH").ok();

        let wrapper_dir = TempDir::new().expect("create wrapper dir");
        let real_dir = TempDir::new().expect("create real dir");
        let wrapper = wrapper_dir.path().join("ghostty");
        let real = real_dir.path().join("ghostty");
        let current = env::current_exe().expect("resolve current executable");

        symlink(current, &wrapper).expect("create self-referential wrapper");
        write_executable_script(
            &real,
            "#!/usr/bin/env bash\nprintf 'REAL_GHOSTTY %s\\n' \"$*\"\n",
        );

        let path_value = format!(
            "{}:{}",
            wrapper_dir.path().display(),
            real_dir.path().display()
        );
        env::set_var("PATH", path_value);

        let resolved = resolve_command_path("ghostty");
        assert_eq!(resolved, real);

        match old_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }

    #[test]
    fn returns_first_hit_when_only_self_candidates_exist() {
        let _guard = ENV_LOCK.lock().expect("lock env");
        let old_path = env::var("PATH").ok();

        let wrapper_dir = TempDir::new().expect("create wrapper dir");
        let wrapper = wrapper_dir.path().join("ghostty");
        let current = env::current_exe().expect("resolve current executable");
        symlink(current, &wrapper).expect("create self-referential wrapper");

        env::set_var("PATH", wrapper_dir.path());
        let resolved = resolve_command_path("ghostty");
        assert_eq!(resolved, wrapper);

        match old_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }
}
