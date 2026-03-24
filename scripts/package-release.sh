#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

VERSION=""
TARGET="x86_64-unknown-linux-gnu"
OUTPUT_DIR="target/release-artifacts"

usage() {
  cat <<'EOF'
Usage: scripts/package-release.sh --version <semver> [options]

Builds the chopper release binaries and assembles a GitHub-release-friendly
archive containing both binaries, install helpers, and dist/ assets.

Options:
  --version <semver>       Release version without leading "v" (required)
  --target <triple>        Cargo target triple (default: x86_64-unknown-linux-gnu)
  --output-dir <path>      Directory for generated artifacts
                           (default: target/release-artifacts)
  -h, --help               Show this help
EOF
}

need_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    echo "missing required command: $1" >&2
    exit 1
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version)
      if [[ $# -lt 2 ]]; then
        echo "--version requires a value" >&2
        exit 2
      fi
      VERSION="$2"
      shift 2
      ;;
    --target)
      if [[ $# -lt 2 ]]; then
        echo "--target requires a value" >&2
        exit 2
      fi
      TARGET="$2"
      shift 2
      ;;
    --output-dir)
      if [[ $# -lt 2 ]]; then
        echo "--output-dir requires a value" >&2
        exit 2
      fi
      OUTPUT_DIR="$2"
      shift 2
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

if [[ -z "${VERSION}" ]]; then
  echo "--version is required" >&2
  usage >&2
  exit 2
fi

if [[ "${VERSION}" == v* ]]; then
  echo "--version must not include a leading v (got: ${VERSION})" >&2
  exit 2
fi

if [[ "${OUTPUT_DIR}" != /* ]]; then
  OUTPUT_DIR="${REPO_ROOT}/${OUTPUT_DIR}"
fi

need_cmd cargo
need_cmd install
need_cmd mktemp
need_cmd tar

CHECKSUM_CMD=()
if command -v sha256sum >/dev/null 2>&1; then
  CHECKSUM_CMD=(sha256sum)
elif command -v shasum >/dev/null 2>&1; then
  CHECKSUM_CMD=(shasum -a 256)
else
  echo "missing required command: sha256sum or shasum" >&2
  exit 1
fi

cd "${REPO_ROOT}"

cargo build --locked --release --target "${TARGET}" --bin chopper --bin chopper-journal-broker

BIN_DIR="${REPO_ROOT}/target/${TARGET}/release"
for binary in chopper chopper-journal-broker; do
  if [[ ! -x "${BIN_DIR}/${binary}" ]]; then
    echo "expected built binary not found: ${BIN_DIR}/${binary}" >&2
    exit 1
  fi
done

ARCHIVE_STEM="chopper-v${VERSION}-${TARGET}"
ARCHIVE_PATH="${OUTPUT_DIR}/${ARCHIVE_STEM}.tar.gz"
CHECKSUM_PATH="${OUTPUT_DIR}/${ARCHIVE_STEM}.sha256"

tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

STAGE_DIR="${tmpdir}/${ARCHIVE_STEM}"
mkdir -p "${STAGE_DIR}/bin" "${STAGE_DIR}/scripts" "${OUTPUT_DIR}"

install -m 0755 "${BIN_DIR}/chopper" "${STAGE_DIR}/bin/chopper"
install -m 0755 "${BIN_DIR}/chopper-journal-broker" "${STAGE_DIR}/bin/chopper-journal-broker"
cp -R "${REPO_ROOT}/dist" "${STAGE_DIR}/dist"
install -m 0755 "${REPO_ROOT}/scripts/install-journal-broker.sh" "${STAGE_DIR}/scripts/install-journal-broker.sh"
install -m 0644 "${REPO_ROOT}/README.md" "${STAGE_DIR}/README.md"

tar -C "${tmpdir}" -czf "${ARCHIVE_PATH}" "${ARCHIVE_STEM}"

archive_basename="$(basename -- "${ARCHIVE_PATH}")"
"${CHECKSUM_CMD[@]}" "${ARCHIVE_PATH}" | sed "s#  ${ARCHIVE_PATH}#  ${archive_basename}#" > "${CHECKSUM_PATH}"

echo "Created release archive: ${ARCHIVE_PATH}"
echo "Created checksum: ${CHECKSUM_PATH}"
