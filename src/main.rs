mod executor;
mod manifest;
mod parser;

use anyhow::Result;
use std::env;
use std::path::PathBuf;

fn config_dir() -> PathBuf {
    directories::ProjectDirs::from("", "", "chopper")
        .map(|d| d.config_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".chopper"))
}

fn find_config(name: &str) -> Option<PathBuf> {
    let cfg = config_dir();
    for ext in ["", ".rhai", ".conf"] {
        let path = cfg.join(format!("{}{}", name, ext));
        if path.exists() {
            return Some(path);
        }
    }
    None
}

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();
    let exe_name = PathBuf::from(&args[0])
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("chopper")
        .to_string();

    if exe_name == "chopper" {
        eprintln!("Usage: symlink to chopper with alias name, or chopper <alias>");
        eprintln!("  chopper <alias> [args...]");
        std::process::exit(1);
    }

    let config_path = find_config(&exe_name);
    
    let manifest = match config_path {
        Some(path) => parser::parse(&path)?,
        None => {
            let exe = which::which(&exe_name)
                .unwrap_or_else(|_| PathBuf::from(&exe_name));
            manifest::Manifest::simple(exe)
        }
    };

    let passthrough_args = if args.len() > 1 { &args[1..] } else { &[] };
    executor::run(manifest, passthrough_args)
}
