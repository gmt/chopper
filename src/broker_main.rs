mod broker;

use broker::dbus_interface::{JournalBroker, BUS_NAME, OBJECT_PATH};
use std::process;

fn main() {
    let verbose = std::env::args().any(|a| a == "--verbose" || a == "-v");

    if verbose {
        eprintln!("chopper-journal-broker: starting on system bus as {BUS_NAME}");
    }

    if let Err(err) = run_broker(verbose) {
        eprintln!("chopper-journal-broker: fatal: {err}");
        process::exit(1);
    }
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
