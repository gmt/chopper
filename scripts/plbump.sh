#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

REMOTE="origin"
SKIP_CHECKS=0
UPDATE_DEPS=0
DRY_RUN=0

usage() {
  cat <<'EOF'
Usage: scripts/plbump.sh [options]

Cut a patch release from the current committed version, push the current branch
and matching tag, then bump Cargo.toml/Cargo.lock to the next patch version and
stage those files for the next development cycle.

The script assumes:
  - the worktree is clean before release work starts
  - Cargo.toml already contains the version you want to release
  - the current branch is the branch you want to push

Options:
  --remote <name>          Git remote to push (default: origin)
  --skip-checks            Skip fmt/clippy/test before tagging and pushing
  --update-deps            Run cargo update before release checks
                           (default is cargo update -w for lockfile sync only)
  --dry-run                Print the planned release/bump targets and exit
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
    --remote)
      if [[ $# -lt 2 ]]; then
        echo "--remote requires a value" >&2
        exit 2
      fi
      REMOTE="$2"
      shift 2
      ;;
    --skip-checks)
      SKIP_CHECKS=1
      shift
      ;;
    --update-deps)
      UPDATE_DEPS=1
      shift
      ;;
    --dry-run)
      DRY_RUN=1
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

need_cmd cargo
need_cmd git
need_cmd mktemp

cd "${REPO_ROOT}"

if [[ -n "$(git status --porcelain)" ]]; then
  echo "refusing to run with a dirty worktree; commit or stash changes first" >&2
  exit 1
fi

if ! git remote get-url "${REMOTE}" >/dev/null 2>&1; then
  echo "unknown git remote: ${REMOTE}" >&2
  exit 1
fi

BRANCH="$(git rev-parse --abbrev-ref HEAD)"
if [[ "${BRANCH}" == "HEAD" ]]; then
  echo "detached HEAD is not supported; check out a branch first" >&2
  exit 1
fi

CURRENT_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n1)"
if [[ -z "${CURRENT_VERSION}" ]]; then
  echo "failed to parse package version from Cargo.toml" >&2
  exit 1
fi

if [[ ! "${CURRENT_VERSION}" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  echo "expected Cargo.toml version in MAJOR.MINOR.PATCH form, got: ${CURRENT_VERSION}" >&2
  exit 1
fi

MAJOR="${BASH_REMATCH[1]}"
MINOR="${BASH_REMATCH[2]}"
PATCH="${BASH_REMATCH[3]}"
NEXT_PATCH=$((PATCH + 1))
NEXT_VERSION="${MAJOR}.${MINOR}.${NEXT_PATCH}"
RELEASE_TAG="v${CURRENT_VERSION}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "Repository root : ${REPO_ROOT}"
  echo "Remote          : ${REMOTE}"
  echo "Branch          : ${BRANCH}"
  echo "Release version : ${CURRENT_VERSION}"
  echo "Release tag     : ${RELEASE_TAG}"
  echo "Next version    : ${NEXT_VERSION}"
  echo "Checks          : $([[ ${SKIP_CHECKS} -eq 1 ]] && echo skipped || echo enabled)"
  echo "Dep refresh     : $([[ ${UPDATE_DEPS} -eq 1 ]] && echo cargo-update || echo workspace-only)"
  exit 0
fi

if git rev-parse -q --verify "refs/tags/${RELEASE_TAG}" >/dev/null 2>&1; then
  echo "local tag already exists: ${RELEASE_TAG}" >&2
  exit 1
fi

if git ls-remote --exit-code --tags "${REMOTE}" "refs/tags/${RELEASE_TAG}" >/dev/null 2>&1; then
  echo "remote tag already exists on ${REMOTE}: ${RELEASE_TAG}" >&2
  exit 1
fi

run_lock_refresh() {
  if [[ "${UPDATE_DEPS}" -eq 1 ]]; then
    cargo update
  else
    cargo update -w
  fi
}

bump_manifest_version() {
  local new_version="$1"
  local tmpfile
  tmpfile="$(mktemp)"
  sed "0,/^version = \".*\"$/s//version = \"${new_version}\"/" Cargo.toml > "${tmpfile}"
  mv "${tmpfile}" Cargo.toml
}

run_lock_refresh

if [[ "${SKIP_CHECKS}" -eq 0 ]]; then
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test -- --nocapture
fi

if ! git diff --quiet -- Cargo.toml Cargo.lock; then
  git add Cargo.toml Cargo.lock
  git commit -m "Cut ${CURRENT_VERSION}"
fi

git tag "${RELEASE_TAG}"
git push "${REMOTE}" "${BRANCH}"
git push "${REMOTE}" "${RELEASE_TAG}"

bump_manifest_version "${NEXT_VERSION}"
cargo update -w
git add Cargo.toml Cargo.lock

echo "Released ${CURRENT_VERSION} from ${BRANCH} and pushed ${RELEASE_TAG} to ${REMOTE}."
echo "Bumped Cargo.toml/Cargo.lock to ${NEXT_VERSION} and staged them for the next commit."
