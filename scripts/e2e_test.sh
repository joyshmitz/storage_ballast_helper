#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
LOG_DIR="${SBH_E2E_LOG_DIR:-${TMPDIR:-/tmp}/sbh-e2e-$(date +%Y%m%d-%H%M%S)}"
LOG_FILE="${LOG_DIR}/e2e.log"
CASE_DIR="${LOG_DIR}/cases"
VERBOSE=0

if [[ "${1:-}" == "--verbose" ]]; then
  VERBOSE=1
fi

mkdir -p "${CASE_DIR}"

log() {
  local msg="$1"
  printf '[%s] %s\n' "$(date -u +"%Y-%m-%dT%H:%M:%SZ")" "${msg}" | tee -a "${LOG_FILE}"
}

run_case() {
  local name="$1"
  local expected="$2"
  shift 2
  local -a cmd=("$@")
  local case_log="${CASE_DIR}/${name}.log"

  log "CASE START: ${name}"
  {
    echo "name=${name}"
    echo "expected=${expected}"
    echo "command=${cmd[*]}"
  } > "${case_log}"

  set +e
  local output
  output="$(SBH_TEST_VERBOSE=1 SBH_OUTPUT_FORMAT=human RUST_BACKTRACE=1 "${cmd[@]}" 2>&1)"
  local status=$?
  set -e

  {
    echo "status=${status}"
    echo "----- output -----"
    echo "${output}"
  } >> "${case_log}"

  if [[ ${VERBOSE} -eq 1 ]]; then
    printf '%s\n' "${output}" | tee -a "${LOG_FILE}" >/dev/null
  fi

  if [[ ${status} -ne 0 ]]; then
    log "CASE FAIL: ${name} (non-zero status=${status})"
    return 1
  fi

  if ! grep -Fq "${expected}" <<< "${output}"; then
    log "CASE FAIL: ${name} (missing expected text: ${expected})"
    return 1
  fi

  log "CASE PASS: ${name}"
  return 0
}

tally_case() {
  if "$@"; then
    pass=$((pass + 1))
  else
    fail=$((fail + 1))
  fi
}

assert_file_contains() {
  local name="$1"
  local file="$2"
  local expected="$3"

  log "ASSERT START: ${name}"

  if [[ ! -f "${file}" ]]; then
    log "ASSERT FAIL: ${name} (missing file: ${file})"
    return 1
  fi

  if ! grep -Fq "${expected}" "${file}"; then
    log "ASSERT FAIL: ${name} (missing expected text: ${expected})"
    return 1
  fi

  log "ASSERT PASS: ${name}"
  return 0
}

create_installer_fixture() {
  local fixture_dir="$1"
  mkdir -p "${fixture_dir}/payload" "${fixture_dir}/bin"

  cat > "${fixture_dir}/payload/sbh" <<'EOF'
#!/usr/bin/env bash
echo "sbh mock 0.0.0"
EOF
  chmod +x "${fixture_dir}/payload/sbh"

  tar -cJf "${fixture_dir}/artifact.tar.xz" -C "${fixture_dir}/payload" sbh

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "${fixture_dir}/artifact.tar.xz" | awk '{print $1 "  artifact.tar.xz"}' > "${fixture_dir}/artifact.sha256"
  else
    shasum -a 256 "${fixture_dir}/artifact.tar.xz" | awk '{print $1 "  artifact.tar.xz"}' > "${fixture_dir}/artifact.sha256"
  fi
}

main() {
  cd "${ROOT_DIR}"
  : > "${LOG_FILE}"
  log "sbh e2e start"
  log "root=${ROOT_DIR}"
  log "logs=${LOG_DIR}"

  log "building debug binary"
  cargo build --quiet
  local target_dir="${CARGO_TARGET_DIR:-${ROOT_DIR}/target}"
  local bin="${target_dir}/debug/sbh"
  local installer="${ROOT_DIR}/scripts/install.sh"
  local installer_fixture="${LOG_DIR}/installer-fixture"
  local installer_events="${installer_fixture}/events.jsonl"

  local pass=0
  local fail=0

  tally_case run_case help "Usage: sbh [OPTIONS] <COMMAND>" "${bin}" --help
  tally_case run_case version "0.1.0" "${bin}" --version
  tally_case run_case install "install: not yet implemented" "${bin}" install
  tally_case run_case uninstall "uninstall: not yet implemented" "${bin}" uninstall
  tally_case run_case status "status: not yet implemented" "${bin}" status
  tally_case run_case stats "stats: not yet implemented" "${bin}" stats
  tally_case run_case scan "scan: not yet implemented" "${bin}" scan
  tally_case run_case clean "clean: not yet implemented" "${bin}" clean
  tally_case run_case ballast "ballast: not yet implemented" "${bin}" ballast
  tally_case run_case ballast_status "ballast status: not yet implemented" "${bin}" ballast status
  tally_case run_case config "config: not yet implemented" "${bin}" config
  tally_case run_case config_show "config show: not yet implemented" "${bin}" config show
  tally_case run_case daemon "daemon: not yet implemented" "${bin}" daemon
  tally_case run_case emergency "emergency: not yet implemented" "${bin}" emergency /tmp --yes
  tally_case run_case protect "protect: not yet implemented" "${bin}" protect --list
  tally_case run_case unprotect "unprotect: not yet implemented" "${bin}" unprotect /tmp
  tally_case run_case tune "tune: not yet implemented" "${bin}" tune
  tally_case run_case check "check: not yet implemented" "${bin}" check /tmp
  tally_case run_case blame "blame: not yet implemented" "${bin}" blame
  tally_case run_case dashboard "dashboard: not yet implemented" "${bin}" dashboard
  tally_case run_case completions "sbh" "${bin}" completions bash

  create_installer_fixture "${installer_fixture}"
  tally_case run_case installer_help "Usage:" "${installer}" --help
  tally_case run_case installer_dry_run "dry-run complete (no changes applied)" "${installer}" --dry-run --dest "${installer_fixture}/bin" --no-color
  tally_case run_case installer_first_install "installed sbh to" env \
    SBH_INSTALLER_ASSET_URL="file://${installer_fixture}/artifact.tar.xz" \
    SBH_INSTALLER_CHECKSUM_URL="file://${installer_fixture}/artifact.sha256" \
    "${installer}" --dest "${installer_fixture}/bin" --version v0.0.0 --verify --no-color --event-log "${installer_events}" --trace-id "trace-install-1"
  tally_case run_case installer_idempotent_rerun "already up to date" env \
    SBH_INSTALLER_ASSET_URL="file://${installer_fixture}/artifact.tar.xz" \
    SBH_INSTALLER_CHECKSUM_URL="file://${installer_fixture}/artifact.sha256" \
    "${installer}" --dest "${installer_fixture}/bin" --version v0.0.0 --verify --no-color --event-log "${installer_events}" --trace-id "trace-install-2"
  tally_case assert_file_contains installer_events_trace1 "${installer_events}" '"trace_id":"trace-install-1"'
  tally_case assert_file_contains installer_events_trace2 "${installer_events}" '"trace_id":"trace-install-2"'
  tally_case assert_file_contains installer_events_download_phase "${installer_events}" '"phase":"download_artifact"'
  tally_case assert_file_contains installer_events_success "${installer_events}" '"status":"success"'

  log "summary pass=${pass} fail=${fail}"
  log "case logs at ${CASE_DIR}"

  if [[ ${fail} -gt 0 ]]; then
    exit 1
  fi
}

main "$@"
