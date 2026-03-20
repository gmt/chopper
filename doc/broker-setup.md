# Journal Broker Setup

The `chopper-journal-broker` daemon is a D-Bus system service that creates and
manages journald namespace instances on behalf of unprivileged users.

It also supports `--help` and `--version` as safe metadata flags. Those return
immediately without touching D-Bus, which makes install verification much less
awkward than launching the daemon directly.

## Overview

When an alias has `journal.ensure = true`, chopper calls the broker via D-Bus
before starting `systemd-cat`. The broker:

1. Validates the caller's UID owns the requested namespace (`u<uid>-*`)
2. Writes a journald drop-in config at
   `/run/systemd/journald@<namespace>.conf.d/chopper.conf`
3. Starts `systemd-journald@<namespace>.service` (which owns the namespace sockets)
4. Enforces anti-abuse limits (max 16 namespaces per UID, storage caps)

## Installation

Preferred (one-shot, from repo root):

```bash
scripts/install-journal-broker.sh --cleanup-user-install
```

What this does:

1. Removes any prior `~/.cargo/bin/chopper-journal-broker` copy.
2. Builds `chopper-journal-broker`.
3. Installs the binary and service policy files.
4. Reloads systemd + D-Bus.
5. Enables and starts `chopper-journal-broker`.

Useful variants:

```bash
# Install runtime broker at /usr/bin/chopper-journal-broker
scripts/install-journal-broker.sh --prefix /usr

# Stage files under /tmp/chopper-pkgroot for packaging (no systemctl calls)
scripts/install-journal-broker.sh --prefix /usr --destdir /tmp/chopper-pkgroot --no-sudo
```

Manual path:

### 1. Remove prior user-local install (optional)

```bash
rm -f ~/.cargo/bin/chopper-journal-broker
cargo uninstall --bin chopper-journal-broker || true
```

### 2. Install the binary

```bash
cargo build --release --bin chopper-journal-broker
sudo install -m 0755 target/release/chopper-journal-broker /usr/local/bin/chopper-journal-broker
```

### 3. Install D-Bus configuration

```bash
sudo cp dist/dbus-1/system.d/com.chopperproject.JournalBroker1.conf \
   /usr/share/dbus-1/system.d/

sudo cp dist/dbus-1/system-services/com.chopperproject.JournalBroker1.service \
   /usr/share/dbus-1/system-services/
```

### 4. Install polkit policy

```bash
sudo cp dist/polkit-1/actions/com.chopperproject.JournalBroker1.policy \
   /usr/share/polkit-1/actions/

sudo cp dist/polkit-1/rules.d/50-chopper-journal-broker.rules \
   /usr/share/polkit-1/rules.d/
```

### 5. Install systemd unit

```bash
sudo cp dist/systemd/chopper-journal-broker.service \
   /etc/systemd/system/

sudo systemctl daemon-reload
sudo systemctl enable --now chopper-journal-broker
```

### 6. Reload D-Bus

```bash
sudo systemctl reload dbus
```

## Verification

```bash
# Check the service is running
systemctl status chopper-journal-broker

# Introspect the D-Bus interface
busctl --system introspect \
  com.chopperproject.JournalBroker1 \
  /com/chopperproject/JournalBroker1

# Test with an alias
cat > ~/.config/chopper/aliases/test-broker.toml <<'EOF'
exec = "sh"
args = ["-c", "echo test; echo err >&2"]

[journal]
namespace = "test"
stderr = true
ensure = true
EOF

chopper test-broker
```

Expected:

1. `test` prints in terminal.
2. Command exits normally.
3. `systemctl status chopper-journal-broker` remains active.

## D-Bus Interface

- **Bus name:** `com.chopperproject.JournalBroker1`
- **Object path:** `/com/chopperproject/JournalBroker1`
- **Interface:** `com.chopperproject.JournalBroker1`

### Method: `EnsureNamespace`

```
EnsureNamespace(namespace: String, options: Dict<String,String>) -> ()
```

**Parameters:**

- `namespace` — must match `u<caller_uid>-*`
- `options` — optional policy overrides:
  - `max_use` — journald `SystemMaxUse` (e.g. `"256M"`), clamped to 512M
  - `rate_limit_interval_usec` — microseconds, clamped to [1000, 3600000000]
  - `rate_limit_burst` — integer, clamped to max 10000

**Errors:**

- `org.freedesktop.DBus.Error.AccessDenied` — namespace not owned by caller UID
- `org.freedesktop.DBus.Error.LimitsExceeded` — too many namespaces for UID
- `org.freedesktop.DBus.Error.Failed` — drop-in write or socket start failure

## Anti-Abuse Limits

| Limit | Default | Description |
|---|---|---|
| Max namespaces per UID | 16 | Prevents namespace instantiation bomb |
| Max SystemMaxUse | 512M | Hard ceiling for per-namespace storage |
| Max RateLimitBurst | 10000 | Hard ceiling for rate limit burst |
| Min RateLimitIntervalSec | 1ms | Minimum rate limit interval |

## Troubleshooting

- Quick first checks:

```bash
which systemd-cat
systemctl status chopper-journal-broker
journalctl -u chopper-journal-broker -n 100 --no-pager
```

- **D-Bus connection refused:** Ensure `dbus-daemon` is running and the bus
  policy file is installed at `/usr/share/dbus-1/system.d/`.
- **Access denied:** Check that the namespace starts with `u<your-uid>-`.
  Run `id -u` to verify your UID.
- **Limits exceeded:** You have too many active namespaces. Check
  `/run/systemd/journal.u<uid>-*` directories.
- **Socket start fails:** The broker needs `CAP_SYS_ADMIN` and write access
  to `/run/systemd/`. Verify the systemd unit has correct capabilities.
