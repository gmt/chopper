#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

CLEANUP_USER_INSTALL=0
PREFIX="/usr/local"
DESTDIR=""
SKIP_SYSTEMCTL=0
USE_SUDO=1

usage() {
  cat <<'EOF'
Usage: scripts/install-journal-broker.sh [options]

Builds chopper-journal-broker, installs it to <prefix>/bin, installs the
required D-Bus/polkit/systemd assets from dist/, and enables the broker
service.

Options:
  --cleanup-user-install   Remove ~/.cargo/bin/chopper-journal-broker first
  --prefix <path>          Runtime install prefix (default: /usr/local)
  --destdir <path>         Staging root for packaging (default: empty)
  --skip-systemctl         Do not run daemon-reload/reload/enable
  --no-sudo                Run install/systemctl without sudo
  -h, --help               Show this help
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --cleanup-user-install)
      CLEANUP_USER_INSTALL=1
      shift
      ;;
    --prefix)
      if [[ $# -lt 2 ]]; then
        echo "--prefix requires a value" >&2
        exit 2
      fi
      PREFIX="$2"
      shift 2
      ;;
    --destdir)
      if [[ $# -lt 2 ]]; then
        echo "--destdir requires a value" >&2
        exit 2
      fi
      DESTDIR="$2"
      shift 2
      ;;
    --skip-systemctl)
      SKIP_SYSTEMCTL=1
      shift
      ;;
    --no-sudo)
      USE_SUDO=0
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
done

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

need_cmd cargo
if [[ "${USE_SUDO}" -eq 1 ]]; then
  need_cmd sudo
fi

if [[ "${CLEANUP_USER_INSTALL}" -eq 1 ]]; then
  rm -f "${HOME}/.cargo/bin/chopper-journal-broker"
  cargo uninstall --bin chopper-journal-broker >/dev/null 2>&1 || true
fi

strip_trailing_slash() {
  local value="$1"
  if [[ "${value}" == "/" ]]; then
    printf '%s' "/"
    return
  fi
  while [[ "${value}" == */ ]]; do
    value="${value%/}"
  done
  printf '%s' "${value}"
}

path_join() {
  local a="$1"
  local b="$2"
  if [[ -z "${a}" ]]; then
    printf '%s' "${b}"
    return
  fi
  if [[ "${a}" == "/" ]]; then
    printf '/%s' "${b#/}"
    return
  fi
  printf '%s/%s' "${a%/}" "${b#/}"
}

stage_path() {
  local absolute_path="$1"
  if [[ -z "${DESTDIR}" ]]; then
    printf '%s' "${absolute_path}"
    return
  fi
  printf '%s' "$(path_join "${DESTDIR}" "${absolute_path#/}")"
}

PREFIX="$(strip_trailing_slash "${PREFIX}")"
DESTDIR="$(strip_trailing_slash "${DESTDIR}")"

if [[ "${PREFIX}" != /* ]]; then
  echo "--prefix must be an absolute path (got: ${PREFIX})" >&2
  exit 2
fi

if [[ -n "${DESTDIR}" && "${DESTDIR}" != /* ]]; then
  echo "--destdir must be an absolute path when set (got: ${DESTDIR})" >&2
  exit 2
fi

if [[ -n "${DESTDIR}" ]]; then
  SKIP_SYSTEMCTL=1
fi

if [[ "${SKIP_SYSTEMCTL}" -eq 0 ]]; then
  need_cmd systemctl
fi

SUDO_CMD=()
if [[ "${USE_SUDO}" -eq 1 ]]; then
  SUDO_CMD=(sudo)
fi

BINARY_RUNTIME_PATH="$(path_join "${PREFIX}" "bin/chopper-journal-broker")"
BINARY_INSTALL_PATH="$(stage_path "${BINARY_RUNTIME_PATH}")"
DBUS_SYSTEM_D_INSTALL_PATH="$(stage_path "/usr/share/dbus-1/system.d/com.chopperproject.JournalBroker1.conf")"
DBUS_SYSTEM_SERVICES_INSTALL_PATH="$(stage_path "/usr/share/dbus-1/system-services/com.chopperproject.JournalBroker1.service")"
POLKIT_ACTIONS_INSTALL_PATH="$(stage_path "/usr/share/polkit-1/actions/com.chopperproject.JournalBroker1.policy")"
POLKIT_RULES_INSTALL_PATH="$(stage_path "/usr/share/polkit-1/rules.d/50-chopper-journal-broker.rules")"
SYSTEMD_UNIT_INSTALL_PATH="$(stage_path "/etc/systemd/system/chopper-journal-broker.service")"

cd "${REPO_ROOT}"
cargo build --release --bin chopper-journal-broker

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

sed "s|^ExecStart=.*$|ExecStart=${BINARY_RUNTIME_PATH}|" \
  dist/systemd/chopper-journal-broker.service \
  > "${tmpdir}/chopper-journal-broker.service"

sed "s|^Exec=.*$|Exec=${BINARY_RUNTIME_PATH}|" \
  dist/dbus-1/system-services/com.chopperproject.JournalBroker1.service \
  > "${tmpdir}/com.chopperproject.JournalBroker1.service"

"${SUDO_CMD[@]}" install -D -m 0755 target/release/chopper-journal-broker "${BINARY_INSTALL_PATH}"
"${SUDO_CMD[@]}" install -D -m 0644 dist/dbus-1/system.d/com.chopperproject.JournalBroker1.conf "${DBUS_SYSTEM_D_INSTALL_PATH}"
"${SUDO_CMD[@]}" install -D -m 0644 "${tmpdir}/com.chopperproject.JournalBroker1.service" "${DBUS_SYSTEM_SERVICES_INSTALL_PATH}"
"${SUDO_CMD[@]}" install -D -m 0644 dist/polkit-1/actions/com.chopperproject.JournalBroker1.policy "${POLKIT_ACTIONS_INSTALL_PATH}"
"${SUDO_CMD[@]}" install -D -m 0644 dist/polkit-1/rules.d/50-chopper-journal-broker.rules "${POLKIT_RULES_INSTALL_PATH}"
"${SUDO_CMD[@]}" install -D -m 0644 "${tmpdir}/chopper-journal-broker.service" "${SYSTEMD_UNIT_INSTALL_PATH}"

if [[ "${SKIP_SYSTEMCTL}" -eq 0 ]]; then
  "${SUDO_CMD[@]}" systemctl daemon-reload
  "${SUDO_CMD[@]}" systemctl reload dbus
  "${SUDO_CMD[@]}" systemctl enable --now chopper-journal-broker
else
  echo "Skipped systemctl actions."
fi

echo "Broker installed. Verify with:"
echo "  systemctl status chopper-journal-broker"
echo "  busctl --system introspect com.chopperproject.JournalBroker1 /com/chopperproject/JournalBroker1"
