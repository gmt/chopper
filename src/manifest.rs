use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub exec: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub env_remove: Vec<String>,
    pub journal: Option<JournalConfig>,
    pub reconcile: Option<ReconcileConfig>,
}

impl Manifest {
    pub fn simple(exec: PathBuf) -> Self {
        Self {
            exec,
            args: Vec::new(),
            env: HashMap::new(),
            env_remove: Vec::new(),
            journal: None,
            reconcile: None,
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
    ) -> Invocation {
        let mut args = self.args.clone();
        args.extend(runtime_args.iter().cloned());

        let mut env = self.env.clone();
        let mut env_remove = self.env_remove.clone();

        if let Some(patch) = patch {
            if let Some(replace) = patch.replace_args {
                args = replace;
            }
            args.extend(patch.append_args);

            for (key, value) in patch.set_env {
                env.insert(key, value);
            }
            env_remove.extend(patch.remove_env);
        }

        Invocation {
            exec: self.exec.clone(),
            args,
            env,
            env_remove,
            journal: self.journal.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct JournalConfig {
    pub namespace: String,
    pub stderr: bool,
    pub identifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReconcileConfig {
    pub script: PathBuf,
    pub function: String,
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

#[cfg(test)]
mod tests {
    use super::{Manifest, RuntimePatch};
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn invocation_merges_runtime_and_patch() {
        let mut manifest = Manifest::simple(PathBuf::from("echo"));
        manifest.args = vec!["base".into()];
        manifest.env.insert("A".into(), "1".into());
        manifest.env_remove = vec!["OLD".into()];

        let mut patch = RuntimePatch::default();
        patch.append_args = vec!["patch".into()];
        patch.set_env = HashMap::from([("B".into(), "2".into())]);
        patch.remove_env = vec!["REMOVE".into()];

        let invocation = manifest.build_invocation(&["runtime".into()], Some(patch));

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

        let invocation = manifest.build_invocation(&["runtime".into()], Some(patch));
        assert_eq!(invocation.args, vec!["replaced", "extra"]);
    }
}
