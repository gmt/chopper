use crate::path_mutation::{self, PathMutationConfig};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub exec: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub env_remove: Vec<String>,
    pub path: Option<PathMutationConfig>,
    pub journal: Option<JournalConfig>,
    pub reconcile: Option<ReconcileConfig>,
    pub bashcomp: Option<BashcompConfig>,
}

impl Manifest {
    pub fn simple(exec: PathBuf) -> Self {
        Self {
            exec,
            args: Vec::new(),
            env: HashMap::new(),
            env_remove: Vec::new(),
            path: None,
            journal: None,
            reconcile: None,
            bashcomp: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_journal(mut self, journal: JournalConfig) -> Self {
        self.journal = Some(journal);
        self
    }

    pub fn with_reconcile(mut self, reconcile: ReconcileConfig) -> Self {
        self.reconcile = Some(reconcile);
        self
    }

    pub fn build_invocation(
        &self,
        runtime_args: &[String],
        patch: Option<RuntimePatch>,
    ) -> Result<Invocation> {
        let mut args = self.args.clone();
        args.extend(runtime_args.iter().cloned());

        let mut env = self.env.clone();
        let mut env_remove = self.env_remove.clone();

        if let Some(path_config) = self.path.as_ref() {
            let inherited_path = env::var("PATH").ok();
            let base_path =
                effective_path_before_patch(inherited_path.as_deref(), &env, &env_remove);
            let path_value = path_mutation::apply_runtime_path(base_path, path_config)?;
            env_remove.retain(|remove_key| remove_key != "PATH");
            env.insert("PATH".into(), path_value);
        }

        if let Some(patch) = patch {
            if let Some(replace) = patch.replace_args {
                args = replace;
            }
            args.extend(patch.append_args);

            for (key, value) in patch.set_env {
                env_remove.retain(|remove_key| remove_key != &key);
                env.insert(key, value);
            }
            env_remove.extend(patch.remove_env);
        }

        env_remove = dedupe_preserving_order(env_remove);
        for key in &env_remove {
            env.remove(key);
        }

        Ok(Invocation {
            exec: self.exec.clone(),
            args,
            env,
            env_remove,
            journal: self.journal.clone(),
        })
    }
}

fn effective_path_before_patch<'a>(
    inherited_path: Option<&'a str>,
    env: &'a HashMap<String, String>,
    env_remove: &[String],
) -> Option<&'a str> {
    if let Some(path) = env.get("PATH") {
        return Some(path.as_str());
    }
    if env_remove.iter().any(|key| key == "PATH") {
        return None;
    }
    inherited_path
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalConfig {
    pub namespace: String,
    pub stderr: bool,
    pub identifier: Option<String>,
    #[serde(default)]
    pub user_scope: bool,
    #[serde(default)]
    pub ensure: bool,
    pub max_use: Option<String>,
    pub rate_limit_interval_usec: Option<u64>,
    pub rate_limit_burst: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileConfig {
    pub script: PathBuf,
    pub function: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BashcompConfig {
    pub disabled: bool,
    pub passthrough: bool,
    pub script: Option<PathBuf>,
    pub rhai_script: Option<PathBuf>,
    pub rhai_function: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct RuntimePatch {
    pub replace_args: Option<Vec<String>>,
    pub append_args: Vec<String>,
    pub set_env: HashMap<String, String>,
    pub remove_env: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub exec: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub env_remove: Vec<String>,
    pub journal: Option<JournalConfig>,
}

fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(values.len());
    for value in values {
        if seen.insert(value.clone()) {
            out.push(value);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{Manifest, RuntimePatch};
    use crate::path_mutation::PathMutationConfig;
    use crate::test_support::ENV_LOCK;
    use std::collections::HashMap;
    use std::env;
    use std::path::PathBuf;

    #[test]
    fn invocation_merges_runtime_and_patch() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.args = vec!["base".into()];
        manifest.env.insert("A".into(), "1".into());
        manifest.env_remove = vec!["OLD".into()];

        let patch = RuntimePatch {
            append_args: vec!["patch".into()],
            set_env: HashMap::from([("B".into(), "2".into())]),
            remove_env: vec!["REMOVE".into()],
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&["runtime".into()], Some(patch))
            .expect("build invocation");

        assert_eq!(invocation.args, vec!["base", "runtime", "patch"]);
        assert_eq!(invocation.env.get("A"), Some(&"1".to_string()));
        assert_eq!(invocation.env.get("B"), Some(&"2".to_string()));
        assert_eq!(invocation.env_remove, vec!["OLD", "REMOVE"]);
    }

    #[test]
    fn replace_args_overrides_composed_args() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.args = vec!["base".into()];

        let patch = RuntimePatch {
            replace_args: Some(vec!["replaced".into()]),
            append_args: vec!["extra".into()],
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&["runtime".into()], Some(patch))
            .expect("build invocation");
        assert_eq!(invocation.args, vec!["replaced", "extra"]);
    }

    #[test]
    fn patch_set_env_overrides_alias_env_remove() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec!["PROMOTE".into(), "KEEP_REMOVED".into()];

        let patch = RuntimePatch {
            set_env: HashMap::from([("PROMOTE".into(), "patched".into())]),
            remove_env: vec!["PATCH_REMOVED".into()],
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&[], Some(patch))
            .expect("build invocation");
        assert_eq!(invocation.env.get("PROMOTE"), Some(&"patched".to_string()));
        assert_eq!(invocation.env_remove, vec!["KEEP_REMOVED", "PATCH_REMOVED"]);
    }

    #[test]
    fn patch_remove_env_still_wins_over_patch_set_env() {
        let manifest = Manifest::simple(PathBuf::from("echo"));
        let patch = RuntimePatch {
            set_env: HashMap::from([("CLASH".into(), "patched".into())]),
            remove_env: vec!["CLASH".into()],
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&[], Some(patch))
            .expect("build invocation");
        assert!(!invocation.env.contains_key("CLASH"));
        assert_eq!(invocation.env_remove, vec!["CLASH"]);
    }

    #[test]
    fn env_remove_is_deduplicated_and_applied_to_env_map() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env = HashMap::from([("A".into(), "1".into()), ("B".into(), "2".into())]);
        manifest.env_remove = vec!["A".into(), "A".into()];

        let patch = RuntimePatch {
            remove_env: vec!["B".into(), "B".into()],
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&[], Some(patch))
            .expect("build invocation");
        assert_eq!(invocation.env_remove, vec!["A", "B"]);
        assert!(!invocation.env.contains_key("A"));
        assert!(!invocation.env.contains_key("B"));
    }

    #[test]
    fn static_path_mutation_overrides_removed_inherited_path() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let original_path = env::var("PATH").ok();
        env::set_var("PATH", "/usr/bin:/bin");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.env_remove = vec!["PATH".into()];
        manifest.path = Some(PathMutationConfig {
            prepend_one: vec!["/custom/bin".into()],
            ..PathMutationConfig::default()
        });

        let invocation = manifest
            .build_invocation(&[], None)
            .expect("build invocation");
        assert_eq!(invocation.env.get("PATH"), Some(&"/custom/bin".to_string()));
        assert!(invocation.env_remove.is_empty());

        match original_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }

    #[test]
    fn patch_set_env_path_overrides_static_path_mutation() {
        let _guard = ENV_LOCK.lock().expect("lock env mutex");
        let original_path = env::var("PATH").ok();
        env::set_var("PATH", "/usr/bin:/bin");

        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.path = Some(PathMutationConfig {
            prepend_one: vec!["/custom/bin".into()],
            ..PathMutationConfig::default()
        });

        let patch = RuntimePatch {
            set_env: HashMap::from([("PATH".into(), "/override/bin".into())]),
            ..RuntimePatch::default()
        };

        let invocation = manifest
            .build_invocation(&[], Some(patch))
            .expect("build invocation");
        assert_eq!(
            invocation.env.get("PATH"),
            Some(&"/override/bin".to_string())
        );

        match original_path {
            Some(value) => env::set_var("PATH", value),
            None => env::remove_var("PATH"),
        }
    }
}
