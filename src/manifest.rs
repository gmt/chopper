use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub exec: PathBuf,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub env_remove: Vec<String>,
    pub capture_stderr: Option<String>,
}

impl Manifest {
    pub fn simple(exec: PathBuf) -> Self {
        Self {
            exec,
            args: Vec::new(),
            env: HashMap::new(),
            env_remove: Vec::new(),
            capture_stderr: None,
        }
    }

    pub fn with_args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    pub fn with_env(mut self, env: HashMap<String, String>) -> Self {
        self.env = env;
        self
    }

    pub fn env(mut self, key: impl Into<String>, val: impl Into<String>) -> Self {
        self.env.insert(key.into(), val.into());
        self
    }

    pub fn remove_env(mut self, key: impl Into<String>) -> Self {
        self.env_remove.push(key.into());
        self
    }
}
