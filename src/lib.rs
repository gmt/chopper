pub mod alias_admin;
mod alias_admin_parse;
mod alias_doc;
pub mod alias_paths;
mod alias_validation;
mod arg_validation;
pub mod broker;
pub mod cache;
pub mod completion;
pub mod config_diagnostics;
pub mod env_util;
mod env_validation;
pub mod exe_runtime;
pub mod exec_resolution;
mod executor;
mod journal_broker_client;
mod journal_validation;
pub mod manifest;
pub mod parser;
mod path_mutation;
mod path_mutation_validation;
mod path_validation;
mod reconcile;
mod rhai_api_catalog;
mod rhai_engine;
mod rhai_facade;
mod rhai_facade_validation;
mod rhai_wiring;
pub mod runner_resolution;
mod string_validation;
pub mod tui;
mod tui_nvim;
mod wrapper_sync;

#[cfg(test)]
pub mod test_support;

use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    if let Some(override_path) = env_util::env_path_override("CHOPPER_CONFIG_DIR") {
        return override_path;
    }

    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper"))
}

pub fn find_config(name: &str) -> Option<PathBuf> {
    alias_paths::find_exec_config(&config_dir(), name)
}
