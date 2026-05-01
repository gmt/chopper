#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

REMOTE="origin"
SKIP_CHECKS=0
UPDATE_DEPS=0
DRY_RUN=0
INDEX_MODE=0
STASHED_WORKTREE=0
STASH_REF=""

usage() {
  cat <<'EOF'
Usage: scripts/plbump.sh [options]

Cut a patch release, push the current branch and matching tag, then bump
Cargo.toml to the next patch version and stage it for the next development
cycle.

The script assumes:
  - the current branch is the branch you want to push
  - Cargo.lock is tracked in the repository

Dirty worktrees:
  - by default, existing tracked/untracked changes are folded into the release commit
  - with --index, only the current index is used for the release commit; unstaged
    and untracked changes are temporarily stashed away and restored afterward
  - unresolved merge conflicts still abort the run

Release version selection:
  - if Cargo.toml and Cargo.lock disagree on the chopper package version,
    Cargo.toml wins and is treated as the release version
  - if they match and that version is already tagged, the script bumps
    Cargo.toml to the next patch version and uses that as the release version
  - if they match and that version is not tagged yet, the script releases the
    current version as-is

Options:
  --remote <name>          Git remote to push (default: origin)
  --index                  Release from the current index only; temporarily
                           stashes unstaged/untracked changes
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
    --index)
      INDEX_MODE=1
      shift
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

cleanup() {
  if [[ "${STASHED_WORKTREE}" -eq 1 ]]; then
    if git stash apply --quiet "${STASH_REF}" >/dev/null 2>&1; then
      git stash drop --quiet "${STASH_REF}" >/dev/null 2>&1 || true
    else
      echo "warning: failed to restore stashed unstaged changes automatically; recover with: git stash apply ${STASH_REF}" >&2
    fi
  fi
}

trap cleanup EXIT

if [[ -n "$(git diff --name-only --diff-filter=U)" ]]; then
  echo "refusing to run with unresolved merge conflicts" >&2
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

if [[ ! -f Cargo.lock ]]; then
  echo "Cargo.lock is required for plbump but was not found" >&2
  exit 1
fi

LOCK_VERSION="$(awk '
  $0 == "[[package]]" {
    in_pkg = 1
    pkg_name = ""
    next
  }
  in_pkg && /^name = "/ {
    pkg_name = $0
    sub(/^name = "/, "", pkg_name)
    sub(/"$/, "", pkg_name)
    next
  }
  in_pkg && pkg_name == "chopper" && /^version = "/ {
    version = $0
    sub(/^version = "/, "", version)
    sub(/"$/, "", version)
    print version
    exit
  }
' Cargo.lock)"

if [[ -z "${LOCK_VERSION}" ]]; then
  echo "failed to parse chopper package version from Cargo.lock" >&2
  exit 1
fi

increment_patch_version() {
  local version="$1"
  if [[ ! "${version}" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
    echo "expected version in MAJOR.MINOR.PATCH form, got: ${version}" >&2
    exit 1
  fi
  local major="${BASH_REMATCH[1]}"
  local minor="${BASH_REMATCH[2]}"
  local patch="${BASH_REMATCH[3]}"
  printf '%s.%s.%s' "${major}" "${minor}" "$((patch + 1))"
}

local_tag_exists() {
  local version="$1"
  git rev-parse -q --verify "refs/tags/v${version}" >/dev/null 2>&1
}

remote_tag_exists() {
  local version="$1"
  git ls-remote --exit-code --tags "${REMOTE}" "refs/tags/v${version}" >/dev/null 2>&1
}

has_unstaged_or_untracked_changes() {
  [[ -n "$(git diff --name-only)" || -n "$(git ls-files --others --exclude-standard)" ]]
}

stash_ref_head() {
  git rev-parse -q --verify refs/stash 2>/dev/null || true
}

stash_unstaged_for_index_mode() {
  local before_ref after_ref
  before_ref="$(stash_ref_head)"
  git stash push --keep-index --include-untracked -m "plbump --index isolate" >/dev/null
  after_ref="$(stash_ref_head)"
  if [[ -n "${after_ref}" && "${after_ref}" != "${before_ref}" ]]; then
    STASH_REF="${after_ref}"
    STASHED_WORKTREE=1
  fi
}

RELEASE_VERSION="${CURRENT_VERSION}"
AUTO_BUMPED_RELEASE=0
VERSION_SOURCE="existing-toml-mismatch"
if [[ "${CURRENT_VERSION}" == "${LOCK_VERSION}" ]]; then
  VERSION_SOURCE="existing-untagged-match"
  if local_tag_exists "${CURRENT_VERSION}"; then
    RELEASE_VERSION="$(increment_patch_version "${CURRENT_VERSION}")"
    AUTO_BUMPED_RELEASE=1
    VERSION_SOURCE="auto-bumped-from-existing-tag"
  fi
fi

NEXT_VERSION="$(increment_patch_version "${RELEASE_VERSION}")"
RELEASE_TAG="v${RELEASE_VERSION}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
  echo "Repository root : ${REPO_ROOT}"
  echo "Remote          : ${REMOTE}"
  echo "Branch          : ${BRANCH}"
  echo "Mode            : $([[ ${INDEX_MODE} -eq 1 ]] && echo index-only || echo fold-worktree)"
  echo "Worktree        : $([[ -n "$(git status --porcelain)" ]] && echo dirty || echo clean)"
  echo "Cargo.toml      : ${CURRENT_VERSION}"
  echo "Cargo.lock      : ${LOCK_VERSION}"
  echo "Release version : ${RELEASE_VERSION}"
  echo "Release tag     : ${RELEASE_TAG}"
  echo "Next version    : ${NEXT_VERSION}"
  echo "Version source  : ${VERSION_SOURCE}"
  echo "Checks          : $([[ ${SKIP_CHECKS} -eq 1 ]] && echo skipped || echo enabled)"
  echo "Dep refresh     : $([[ ${UPDATE_DEPS} -eq 1 ]] && echo cargo-update || echo workspace-only)"
  exit 0
fi

if [[ "${INDEX_MODE}" -eq 1 ]]; then
  if [[ -n "$(git diff --name-only -- Cargo.toml Cargo.lock)" ]]; then
    echo "with --index, unstaged changes to Cargo.toml or Cargo.lock must be staged or stashed first" >&2
    exit 1
  fi
  if has_unstaged_or_untracked_changes; then
    stash_unstaged_for_index_mode
  fi
fi

if [[ "${CURRENT_VERSION}" == "${LOCK_VERSION}" ]] && remote_tag_exists "${CURRENT_VERSION}"; then
  RELEASE_VERSION="$(increment_patch_version "${CURRENT_VERSION}")"
  AUTO_BUMPED_RELEASE=1
  VERSION_SOURCE="auto-bumped-from-existing-tag"
fi

NEXT_VERSION="$(increment_patch_version "${RELEASE_VERSION}")"
RELEASE_TAG="v${RELEASE_VERSION}"

if local_tag_exists "${RELEASE_VERSION}"; then
  echo "local tag already exists: ${RELEASE_TAG}" >&2
  exit 1
fi

if remote_tag_exists "${RELEASE_VERSION}"; then
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

if [[ "${AUTO_BUMPED_RELEASE}" -eq 1 ]]; then
  bump_manifest_version "${RELEASE_VERSION}"
fi

run_lock_refresh
cargo build --release \
  --bin chopper \
  --bin chopper-exe \
  --bin chopper-journal-broker

if [[ "${SKIP_CHECKS}" -eq 0 ]]; then
  cargo fmt --check
  cargo clippy --all-targets --all-features -- -D warnings
  cargo test -- --nocapture
fi

if [[ "${INDEX_MODE}" -eq 1 ]]; then
  git add Cargo.toml Cargo.lock
else
  git add -A
fi
if ! git diff --cached --quiet; then
  git commit -m "Cut ${RELEASE_VERSION}"
fi

git tag "${RELEASE_TAG}"
git push "${REMOTE}" "${BRANCH}"
git push "${REMOTE}" "${RELEASE_TAG}"

bump_manifest_version "${NEXT_VERSION}"
git add Cargo.toml

echo "Released ${RELEASE_VERSION} from ${BRANCH} and pushed ${RELEASE_TAG} to ${REMOTE}."
echo "Bumped Cargo.toml to ${NEXT_VERSION} and staged it for the next commit."
echo "Cargo.lock remains at ${RELEASE_VERSION} intentionally so the next plbump run can detect the pending patch release."
