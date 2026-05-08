#!/usr/bin/env bash
set -euo pipefail

error() {
  echo "::error::$1" >&2
}

changed_files_from_git() {
  local base="${DOCS_UPDATE_BASE:-}"
  local head="${DOCS_UPDATE_HEAD:-HEAD}"

  if [[ -z "${base}" ]]; then
    if [[ "${GITHUB_EVENT_NAME:-}" != "pull_request" ]]; then
      echo "docs-update-check: skipping non-PR event '${GITHUB_EVENT_NAME:-local}'" >&2
      return 0
    fi

    local base_ref="origin/${GITHUB_BASE_REF:-main}"
    if ! git rev-parse --verify --quiet "${base_ref}" >/dev/null; then
      error "docs update check could not find ${base_ref}; use actions/checkout with fetch-depth: 0"
      return 2
    fi

    base="$(git merge-base "${base_ref}" HEAD)"
  fi

  git diff --name-only "${base}" "${head}"
}

changed_files="$(
  if [[ -n "${DOCS_UPDATE_CHANGED_FILES:-}" ]]; then
    printf '%s\n' "${DOCS_UPDATE_CHANGED_FILES}"
  else
    changed_files_from_git
  fi
)"

changed_files="$(printf '%s\n' "${changed_files}" | sed '/^[[:space:]]*$/d')"

if [[ -z "${changed_files}" ]]; then
  echo "docs-update-check: no changed files to inspect"
  exit 0
fi

has_changed_path() {
  local pattern="$1"
  printf '%s\n' "${changed_files}" | grep -Eq "${pattern}"
}

user_facing_pattern='^(src/(main|cli_app)\.rs|src/cli/|src/core/config\.rs|src/scanner/(patterns|protection|deletion|scoring)\.rs|src/daemon/(service|notifications|loop_main)\.rs|scripts/install\.(sh|ps1)|packaging/|\.github/macos/)'
docs_help_pattern='^(README\.md|CHANGELOG\.md|docs/|src/cli_app\.rs|packaging/homebrew/Formula/sbh\.rb)'
config_docs_pattern='^(README\.md|docs/|docs/configs/)'

failed=0

if has_changed_path "${user_facing_pattern}" && ! has_changed_path "${docs_help_pattern}"; then
  error "user-facing code changed without a docs/help companion update. Update README.md, docs/, CHANGELOG.md, src/cli_app.rs help text, or the Homebrew formula."
  failed=1
fi

if [[ -z "${DOCS_UPDATE_CHANGED_FILES:-}" ]]; then
  diff_base="${DOCS_UPDATE_BASE:-}"
  diff_head="${DOCS_UPDATE_HEAD:-HEAD}"

  if [[ -z "${diff_base}" && "${GITHUB_EVENT_NAME:-}" == "pull_request" ]]; then
    diff_base="$(git merge-base "origin/${GITHUB_BASE_REF:-main}" HEAD)"
  fi

  if [[ -n "${diff_base}" ]]; then
    cli_diff="$(git diff --unified=0 "${diff_base}" "${diff_head}" -- src/cli_app.rs || true)"
    if printf '%s\n' "${cli_diff}" | grep -Eq '^\+\s*#\[(arg|command)\b'; then
      if ! printf '%s\n' "${cli_diff}" | grep -Eq '^\+\s*(///|#\[(arg|command)[^]]*(help|about|long_help|long_about))'; then
        error "CLI flag/command annotations changed without added help text. Add clap help/about text or a Rust doc comment in src/cli_app.rs."
        failed=1
      fi
    fi

    config_diff="$(git diff --unified=0 "${diff_base}" "${diff_head}" -- src/core/config.rs || true)"
    if printf '%s\n' "${config_diff}" | grep -Eq '^\+\s*pub [A-Za-z_][A-Za-z0-9_]*:'; then
      if ! has_changed_path "${config_docs_pattern}"; then
        error "configuration fields changed without a config documentation update. Update docs/, docs/configs/, or README.md."
        failed=1
      fi
    fi
  fi
else
  echo "docs-update-check: DOCS_UPDATE_CHANGED_FILES set; skipping diff-content checks"
fi

if [[ "${failed}" -ne 0 ]]; then
  printf 'Changed files:\n%s\n' "${changed_files}" >&2
  exit 1
fi

echo "docs-update-check: user-facing changes have docs/help coverage"
