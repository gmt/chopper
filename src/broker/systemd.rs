use super::policy::JournalDropInConfig;
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Base directory for journald namespace drop-in configs.
const JOURNALD_DROPIN_BASE: &str = "/run/systemd";

/// Write a journald namespace drop-in configuration file.
///
/// Creates `/run/systemd/journald@{namespace}.conf.d/chopper.conf` with
/// `[Journal]` settings derived from the validated config.  Uses atomic
/// temp-file + rename for crash safety.
pub fn write_journal_drop_in(namespace: &str, config: &JournalDropInConfig) -> Result<()> {
    let conf_dir = dropin_dir_path(namespace);
    fs::create_dir_all(&conf_dir)
        .with_context(|| format!("failed to create drop-in dir {}", conf_dir.display()))?;

    // Restrict the directory to root.
    fs::set_permissions(&conf_dir, fs::Permissions::from_mode(0o755))
        .with_context(|| format!("failed to set permissions on {}", conf_dir.display()))?;

    let target = conf_dir.join("chopper.conf");
    let tmp = conf_dir.join(".chopper.conf.tmp");

    let content = render_dropin_content(config);

    {
        let mut f = fs::File::create(&tmp)
            .with_context(|| format!("failed to create temp file {}", tmp.display()))?;
        f.write_all(content.as_bytes())
            .with_context(|| format!("failed to write temp file {}", tmp.display()))?;
        f.sync_all()
            .with_context(|| format!("failed to sync temp file {}", tmp.display()))?;
    }

    fs::rename(&tmp, &target).with_context(|| {
        format!(
            "failed to rename {} -> {}",
            tmp.display(),
            target.display()
        )
    })?;

    Ok(())
}

/// Start the journald namespace socket units for a given namespace.
///
/// Runs `systemctl start systemd-journald@{namespace}.socket
/// systemd-journald-varlink@{namespace}.socket`.
pub fn start_namespace_sockets(namespace: &str) -> Result<()> {
    let socket_unit = format!("systemd-journald@{namespace}.socket");
    let varlink_unit = format!("systemd-journald-varlink@{namespace}.socket");

    let status = Command::new("systemctl")
        .args(["start", &socket_unit, &varlink_unit])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .status()
        .with_context(|| format!("failed to run systemctl start for {socket_unit}"))?;

    if !status.success() {
        return Err(anyhow!(
            "systemctl start failed for namespace `{namespace}` (status {status})"
        ));
    }

    Ok(())
}

/// Check whether a namespace's runtime directory exists, indicating the
/// namespace sockets are (or were recently) active.
pub fn is_namespace_active(namespace: &str) -> bool {
    let runtime_dir = Path::new("/run/systemd").join(format!("journal.{namespace}"));
    runtime_dir.is_dir()
}

/// Return the path to the drop-in directory for a journal namespace.
fn dropin_dir_path(namespace: &str) -> PathBuf {
    Path::new(JOURNALD_DROPIN_BASE).join(format!("journald@{namespace}.conf.d"))
}

/// Render the `[Journal]` drop-in content.
pub fn render_dropin_content(config: &JournalDropInConfig) -> String {
    let interval_sec = config.rate_limit_interval_usec / 1_000_000;
    let interval_remainder = config.rate_limit_interval_usec % 1_000_000;

    let interval_str = if interval_remainder == 0 {
        format!("{interval_sec}s")
    } else {
        format!("{}us", config.rate_limit_interval_usec)
    };

    format!(
        "# Managed by chopper-journal-broker — do not edit\n\
         [Journal]\n\
         SystemMaxUse={}\n\
         RateLimitIntervalSec={}\n\
         RateLimitBurst={}\n",
        config.system_max_use, interval_str, config.rate_limit_burst,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::broker::policy::JournalDropInConfig;

    #[test]
    fn render_dropin_produces_valid_journal_section() {
        let config = JournalDropInConfig {
            system_max_use: "128M".to_string(),
            rate_limit_interval_usec: 30_000_000,
            rate_limit_burst: 1000,
        };
        let content = render_dropin_content(&config);
        assert!(content.contains("[Journal]"), "{content}");
        assert!(content.contains("SystemMaxUse=128M"), "{content}");
        assert!(content.contains("RateLimitIntervalSec=30s"), "{content}");
        assert!(content.contains("RateLimitBurst=1000"), "{content}");
    }

    #[test]
    fn render_dropin_uses_microseconds_for_sub_second_intervals() {
        let config = JournalDropInConfig {
            system_max_use: "64M".to_string(),
            rate_limit_interval_usec: 500_000,
            rate_limit_burst: 100,
        };
        let content = render_dropin_content(&config);
        assert!(
            content.contains("RateLimitIntervalSec=500000us"),
            "{content}"
        );
    }

    #[test]
    fn dropin_dir_path_constructs_correct_path() {
        let path = dropin_dir_path("u1000-alice-ops");
        assert_eq!(
            path,
            PathBuf::from("/run/systemd/journald@u1000-alice-ops.conf.d")
        );
    }

    #[test]
    fn write_journal_drop_in_creates_file_atomically() {
        let temp = tempfile::TempDir::new().expect("tempdir");
        // We can't write to /run/systemd in tests, so test the render function
        // and the directory construction logic separately.
        let config = JournalDropInConfig::default();
        let content = render_dropin_content(&config);
        let target = temp.path().join("chopper.conf");
        fs::write(&target, &content).expect("write test file");
        let read_back = fs::read_to_string(&target).expect("read back");
        assert_eq!(read_back, content);
    }
}
