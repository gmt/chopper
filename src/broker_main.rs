mod broker;

use broker::dbus_interface::{JournalBroker, BUS_NAME, OBJECT_PATH};
use std::process;

fn main() {
    match parse_action(std::env::args()) {
        BrokerAction::Help => print_help(),
        BrokerAction::Version => println!("chopper-journal-broker {}", env!("CARGO_PKG_VERSION")),
        BrokerAction::Run { verbose } => {
            if verbose {
                eprintln!("chopper-journal-broker: starting on system bus as {BUS_NAME}");
            }

            if let Err(err) = run_broker(verbose) {
                eprintln!("chopper-journal-broker: fatal: {err}");
                process::exit(1);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrokerAction {
    Help,
    Version,
    Run { verbose: bool },
}

fn parse_action(args: impl IntoIterator<Item = String>) -> BrokerAction {
    let mut verbose = false;

    for arg in args.into_iter().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => return BrokerAction::Help,
            "-V" | "--version" => return BrokerAction::Version,
            "-v" | "--verbose" => verbose = true,
            _ => {}
        }
    }

    BrokerAction::Run { verbose }
}

fn print_help() {
    println!("Usage:");
    println!("  chopper-journal-broker [options]");
    println!();
    println!("Options:");
    println!("  -h, --help                   Show this help");
    println!("  -V, --version                Show version");
    println!("  -v, --verbose                Log broker startup to stderr");
}

fn run_broker(verbose: bool) -> anyhow::Result<()> {
    let connection = zbus::blocking::Connection::system()
        .map_err(|e| anyhow::anyhow!("failed to connect to system D-Bus: {e}"))?;

    connection
        .object_server()
        .at(OBJECT_PATH, JournalBroker)
        .map_err(|e| anyhow::anyhow!("failed to register D-Bus object: {e}"))?;

    connection
        .request_name(BUS_NAME)
        .map_err(|e| anyhow::anyhow!("failed to acquire bus name {BUS_NAME}: {e}"))?;

    if verbose {
        eprintln!("chopper-journal-broker: listening for requests");
    }

    // Block forever, serving D-Bus requests.  The zbus blocking connection
    // handles dispatching internally.  We park this thread until the process
    // receives a signal (SIGTERM from systemd, etc.).
    loop {
        std::thread::park();
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_action, BrokerAction};

    fn parse(raw: &[&str]) -> BrokerAction {
        parse_action(raw.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn help_short_circuits_before_run() {
        assert_eq!(parse(&["chopper-journal-broker", "-h"]), BrokerAction::Help);
    }

    #[test]
    fn version_long_circuits_before_run() {
        assert_eq!(
            parse(&["chopper-journal-broker", "--version"]),
            BrokerAction::Version
        );
    }

    #[test]
    fn verbose_runs_broker() {
        assert_eq!(
            parse(&["chopper-journal-broker", "--verbose"]),
            BrokerAction::Run { verbose: true }
        );
    }

    #[test]
    fn help_wins_over_verbose() {
        assert_eq!(
            parse(&["chopper-journal-broker", "--verbose", "--help"]),
            BrokerAction::Help
        );
    }

    #[test]
    fn unknown_args_are_ignored_for_now() {
        assert_eq!(
            parse(&["chopper-journal-broker", "--mystery"]),
            BrokerAction::Run { verbose: false }
        );
    }
}
