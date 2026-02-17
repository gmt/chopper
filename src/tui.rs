use crossterm::style::Stylize;
use std::io::{self, IsTerminal, Write};
use std::path::PathBuf;

pub fn run_tui() -> i32 {
    match run_tui_inner() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("{err}");
            1
        }
    }
}

fn run_tui_inner() -> anyhow::Result<()> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        anyhow::bail!("--tui requires an interactive terminal");
    }

    loop {
        println!();
        println!("{}", "chopper TUI".bold());
        println!("  [l] list aliases");
        println!("  [g] get alias details");
        println!("  [a] add alias");
        println!("  [s] set alias");
        println!("  [r] remove alias");
        println!("  [e] edit Rhai script in (n)vim");
        println!("  [q] quit");

        let command = prompt("choice")?;
        match command.trim().to_ascii_lowercase().as_str() {
            "l" => run_alias_action(vec!["list".to_string()])?,
            "g" => {
                let alias = prompt("alias name")?;
                run_alias_action(vec!["get".to_string(), alias])?;
            }
            "a" => {
                let alias = prompt("alias name")?;
                let exec = prompt("exec command")?;
                let args = prompt("args (shell words, optional)")?;
                let env_assignments =
                    prompt("env assignments KEY=VALUE (comma-separated, optional)")?;
                let env_remove = prompt("env remove keys (comma-separated, optional)")?;
                let journal_ns = prompt("journal namespace (optional)")?;
                let journal_stderr = prompt("journal stderr true/false (optional)")?;
                let journal_identifier = prompt("journal identifier (optional)")?;

                let mut raw = vec!["add".to_string(), alias, "--exec".to_string(), exec];
                for arg in parse_shell_words(&args)? {
                    raw.push("--arg".to_string());
                    raw.push(arg);
                }
                for assignment in split_csv(&env_assignments) {
                    raw.push("--env".to_string());
                    raw.push(assignment);
                }
                for key in split_csv(&env_remove) {
                    raw.push("--env-remove".to_string());
                    raw.push(key);
                }
                if !journal_ns.trim().is_empty() {
                    raw.push("--journal-namespace".to_string());
                    raw.push(journal_ns.trim().to_string());
                }
                if !journal_stderr.trim().is_empty() {
                    raw.push("--journal-stderr".to_string());
                    raw.push(journal_stderr.trim().to_string());
                }
                if !journal_identifier.trim().is_empty() {
                    raw.push("--journal-identifier".to_string());
                    raw.push(journal_identifier.trim().to_string());
                }
                run_alias_action(raw)?;
            }
            "s" => {
                let alias = prompt("alias name")?;
                let exec = prompt("new exec (optional)")?;
                let args = prompt("new args (shell words, optional)")?;
                let env_assignments =
                    prompt("env assignments KEY=VALUE (comma-separated, optional)")?;
                let env_remove = prompt("env remove keys (comma-separated, optional)")?;
                let journal_clear = prompt("clear journal? (yes/no, optional)")?;
                let journal_ns = prompt("journal namespace (optional)")?;
                let journal_stderr = prompt("journal stderr true/false (optional)")?;
                let journal_identifier = prompt("journal identifier (optional)")?;

                let mut raw = vec!["set".to_string(), alias];
                if !exec.trim().is_empty() {
                    raw.push("--exec".to_string());
                    raw.push(exec.trim().to_string());
                }
                for arg in parse_shell_words(&args)? {
                    raw.push("--arg".to_string());
                    raw.push(arg);
                }
                for assignment in split_csv(&env_assignments) {
                    raw.push("--env".to_string());
                    raw.push(assignment);
                }
                for key in split_csv(&env_remove) {
                    raw.push("--env-remove".to_string());
                    raw.push(key);
                }
                if is_yes(&journal_clear) {
                    raw.push("--journal-clear".to_string());
                }
                if !journal_ns.trim().is_empty() {
                    raw.push("--journal-namespace".to_string());
                    raw.push(journal_ns.trim().to_string());
                }
                if !journal_stderr.trim().is_empty() {
                    raw.push("--journal-stderr".to_string());
                    raw.push(journal_stderr.trim().to_string());
                }
                if !journal_identifier.trim().is_empty() {
                    raw.push("--journal-identifier".to_string());
                    raw.push(journal_identifier.trim().to_string());
                }
                run_alias_action(raw)?;
            }
            "r" => {
                let alias = prompt("alias name")?;
                let mode = prompt("mode (clean|dirty)")?;
                let symlink_path = prompt("symlink path (optional)")?;

                let mut raw = vec!["remove".to_string(), alias];
                if !mode.trim().is_empty() {
                    raw.push("--mode".to_string());
                    raw.push(mode.trim().to_string());
                }
                if !symlink_path.trim().is_empty() {
                    raw.push("--symlink-path".to_string());
                    raw.push(symlink_path.trim().to_string());
                }
                run_alias_action(raw)?;
            }
            "e" => {
                let input = prompt("Rhai script path")?;
                let path = PathBuf::from(input.trim());
                crate::tui_nvim::open_rhai_editor(
                    &path,
                    &crate::rhai_api_catalog::exported_api_names(),
                )?;
            }
            "q" => break,
            _ => println!("{}", "unknown command".red()),
        }
    }

    Ok(())
}

fn run_alias_action(raw: Vec<String>) -> anyhow::Result<()> {
    if raw.is_empty() {
        return Ok(());
    }
    let code = crate::alias_admin::run_alias_action(&raw);
    if code != 0 {
        anyhow::bail!("alias action failed");
    }
    Ok(())
}

fn prompt(label: &str) -> anyhow::Result<String> {
    print!("{label}: ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim_end().to_string())
}

fn parse_shell_words(input: &str) -> anyhow::Result<Vec<String>> {
    if input.trim().is_empty() {
        return Ok(Vec::new());
    }
    Ok(shell_words::split(input)?)
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn is_yes(input: &str) -> bool {
    matches!(
        input.trim().to_ascii_lowercase().as_str(),
        "y" | "yes" | "true" | "1" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::{is_yes, parse_shell_words, split_csv};

    #[test]
    fn split_csv_ignores_blanks() {
        assert_eq!(
            split_csv("A, B ,,C"),
            vec!["A".to_string(), "B".to_string(), "C".to_string()]
        );
    }

    #[test]
    fn parse_shell_words_preserves_quoted_values() {
        let parsed = parse_shell_words(r#"--flag "two words""#).expect("parse shell words");
        assert_eq!(parsed, vec!["--flag".to_string(), "two words".to_string()]);
    }

    #[test]
    fn yes_parser_accepts_common_truthy_values() {
        assert!(is_yes("yes"));
        assert!(is_yes("TRUE"));
        assert!(!is_yes("no"));
        assert!(!is_yes(""));
    }
}
