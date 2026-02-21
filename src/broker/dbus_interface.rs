use super::policy;
use super::systemd;
use std::collections::HashMap;
use zbus::object_server::SignalEmitter;

/// D-Bus well-known bus name for the journal broker.
pub const BUS_NAME: &str = "com.chopperproject.JournalBroker1";

/// D-Bus object path for the journal broker.
pub const OBJECT_PATH: &str = "/com/chopperproject/JournalBroker1";

/// The journal broker D-Bus object.
pub struct JournalBroker;

#[zbus::interface(name = "com.chopperproject.JournalBroker1")]
impl JournalBroker {
    /// Ensure a journald namespace is ready for the calling user.
    ///
    /// - `namespace`: Must match `u<caller_uid>-...`.
    /// - `options`: Optional policy overrides.  Recognized keys:
    ///   - `max_use`: e.g. `"256M"`, `"1G"`
    ///   - `rate_limit_interval_usec`: microseconds as decimal string
    ///   - `rate_limit_burst`: integer as decimal string
    ///
    /// The broker validates ownership, enforces hard policy limits, writes a
    /// journald drop-in config, and starts the namespace socket units.
    async fn ensure_namespace(
        &self,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(signal_emitter)] _emitter: SignalEmitter<'_>,
        #[zbus(connection)] connection: &zbus::Connection,
        namespace: String,
        options: HashMap<String, String>,
    ) -> zbus::fdo::Result<()> {
        let caller_uid = resolve_caller_uid(&header, connection).await?;

        policy::validate_namespace_ownership(&namespace, caller_uid).map_err(|e| {
            zbus::fdo::Error::AccessDenied(format!("namespace ownership check failed: {e}"))
        })?;

        let active_count = policy::count_active_namespaces_for_uid(caller_uid).map_err(|e| {
            zbus::fdo::Error::Failed(format!("failed to count active namespaces: {e}"))
        })?;

        if should_enforce_namespace_limit(active_count, systemd::is_namespace_active(&namespace)) {
            return Err(zbus::fdo::Error::LimitsExceeded(format!(
                "UID {caller_uid} already has {active_count} active namespaces \
                 (limit: {})",
                policy::MAX_NAMESPACES_PER_UID
            )));
        }

        let config = policy::clamp_journal_options(&options);

        systemd::write_journal_drop_in(&namespace, &config).map_err(|e| {
            zbus::fdo::Error::Failed(format!("failed to write drop-in config: {e}"))
        })?;

        systemd::start_namespace_sockets(&namespace).map_err(|e| {
            zbus::fdo::Error::Failed(format!("failed to start namespace sockets: {e}"))
        })?;

        eprintln!("chopper-journal-broker: ensured namespace `{namespace}` for UID {caller_uid}");

        Ok(())
    }
}

fn should_enforce_namespace_limit(active_count: usize, namespace_is_active: bool) -> bool {
    active_count >= policy::MAX_NAMESPACES_PER_UID && !namespace_is_active
}

/// Resolve the UID of the D-Bus caller via the bus daemon's
/// `GetConnectionUnixUser` method.
async fn resolve_caller_uid(
    header: &zbus::message::Header<'_>,
    connection: &zbus::Connection,
) -> zbus::fdo::Result<u32> {
    let sender = header
        .sender()
        .ok_or_else(|| zbus::fdo::Error::Failed("missing message sender".into()))?;

    let proxy = zbus::fdo::DBusProxy::new(connection)
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("failed to create DBus proxy: {e}")))?;

    let uid = proxy
        .get_connection_unix_user(zbus::names::BusName::Unique(sender.clone()))
        .await
        .map_err(|e| zbus::fdo::Error::Failed(format!("failed to get caller UID: {e}")))?;

    Ok(uid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_limit_enforced_for_new_namespace_at_limit() {
        assert!(should_enforce_namespace_limit(
            policy::MAX_NAMESPACES_PER_UID,
            false
        ));
    }

    #[test]
    fn namespace_limit_not_enforced_for_existing_namespace_at_limit() {
        assert!(!should_enforce_namespace_limit(
            policy::MAX_NAMESPACES_PER_UID,
            true
        ));
    }

    #[test]
    fn namespace_limit_not_enforced_below_limit() {
        assert!(!should_enforce_namespace_limit(
            policy::MAX_NAMESPACES_PER_UID - 1,
            false
        ));
    }
}
