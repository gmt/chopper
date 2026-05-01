mod alias_paths;
mod alias_validation;
mod arg_validation;
mod cache;
mod env_util;
mod env_validation;
mod exe_runtime;
mod exec_resolution;
mod executor;
mod journal_broker_client;
mod journal_validation;
mod manifest;
mod parser;
mod path_mutation;
mod path_mutation_validation;
mod path_validation;
mod reconcile;
mod rhai_api_catalog;
mod rhai_engine;
mod rhai_facade;
mod rhai_facade_validation;
mod rhai_wiring;
mod string_validation;

#[cfg(test)]
mod test_support;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    exe_runtime::run(&args)
}
