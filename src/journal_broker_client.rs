use anyhow::{anyhow, Context, Result};
use std::collections::HashMap;

/// D-Bus well-known bus name for the journal broker.
const BUS_NAME: &str = "com.chopperproject.JournalBroker1";

/// D-Bus object path for the journal broker.
const OBJECT_PATH: &str = "/com/chopperproject/JournalBroker1";

/// D-Bus interface name for the journal broker.
const INTERFACE_NAME: &str = "com.chopperproject.JournalBroker1";

/// Journal policy options passed from client alias config to the broker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct JournalPolicyOptions {
    pub max_use: Option<String>,
    pub rate_limit_interval_usec: Option<u64>,
    pub rate_limit_burst: Option<u32>,
}

impl JournalPolicyOptions {
    /// Convert to a `HashMap<String, String>` for D-Bus transport.
    pub fn to_dbus_options(&self) -> HashMap<String, String> {
        let mut map = HashMap::new();
        if let Some(ref max_use) = self.max_use {
            map.insert("max_use".to_string(), max_use.clone());
        }
        if let Some(interval) = self.rate_limit_interval_usec {
            map.insert("rate_limit_interval_usec".to_string(), interval.to_string());
        }
        if let Some(burst) = self.rate_limit_burst {
            map.insert("rate_limit_burst".to_string(), burst.to_string());
        }
        map
    }
}

/// Ensure a journal namespace is ready by calling the broker daemon over D-Bus.
///
/// Connects to the system bus and invokes
/// `com.chopperproject.JournalBroker1.EnsureNamespace(namespace, options)`.
pub fn ensure_namespace_via_dbus(namespace: &str, options: &JournalPolicyOptions) -> Result<()> {
    let connection = zbus::blocking::Connection::system().with_context(|| {
        "failed to connect to system D-Bus; is dbus-daemon running? \
         The chopper-journal-broker service must be installed for journal \
         namespace ensure to work."
    })?;

    let options_dict = options.to_dbus_options();

    let reply = connection
        .call_method(
            Some(BUS_NAME),
            OBJECT_PATH,
            Some(INTERFACE_NAME),
            "EnsureNamespace",
            &(namespace, &options_dict),
        )
        .map_err(|e| map_dbus_error(e, namespace))?;

    // The method returns () on success; just check the reply is valid.
    let _: () = reply.body().deserialize().with_context(|| {
        format!("unexpected response from journal broker for namespace `{namespace}`")
    })?;

    Ok(())
}

/// Map zbus errors into user-friendly anyhow errors.
fn map_dbus_error(err: zbus::Error, namespace: &str) -> anyhow::Error {
    match &err {
        zbus::Error::MethodError(name, detail, _msg) => {
            let detail_str = detail.as_deref().unwrap_or("(no detail)");
            let name_str = name.as_str();

            if name_str.contains("AccessDenied") {
                anyhow!("journal namespace broker denied access for `{namespace}`: {detail_str}")
            } else if name_str.contains("LimitsExceeded") {
                anyhow!(
                    "journal namespace broker: namespace limit exceeded for `{namespace}`: {detail_str}"
                )
            } else {
                anyhow!(
                    "journal namespace broker failed for `{namespace}`: [{name_str}] {detail_str}"
                )
            }
        }
        _ => anyhow!("D-Bus call to journal namespace broker failed for `{namespace}`: {err}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_options_to_dbus_dict_includes_present_fields() {
        let opts = JournalPolicyOptions {
            max_use: Some("256M".into()),
            rate_limit_interval_usec: Some(30_000_000),
            rate_limit_burst: Some(500),
        };
        let dict = opts.to_dbus_options();
        assert_eq!(dict.get("max_use"), Some(&"256M".to_string()));
        assert_eq!(
            dict.get("rate_limit_interval_usec"),
            Some(&"30000000".to_string())
        );
        assert_eq!(dict.get("rate_limit_burst"), Some(&"500".to_string()));
    }

    #[test]
    fn policy_options_to_dbus_dict_omits_none_fields() {
        let opts = JournalPolicyOptions::default();
        let dict = opts.to_dbus_options();
        assert!(dict.is_empty());
    }

    #[test]
    fn policy_options_to_dbus_dict_partial_fields() {
        let opts = JournalPolicyOptions {
            max_use: Some("64M".into()),
            rate_limit_interval_usec: None,
            rate_limit_burst: None,
        };
        let dict = opts.to_dbus_options();
        assert_eq!(dict.len(), 1);
        assert_eq!(dict.get("max_use"), Some(&"64M".to_string()));
    }
}
