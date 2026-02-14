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
  output="$(SBH_TEST_VERBOSE=1 RUST_BACKTRACE=1 "${cmd[@]}" 2>&1)"
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

  local pass=0
  local fail=0

  run_case help "Usage: sbh <COMMAND>" "${bin}" --help && ((pass+=1)) || ((fail+=1))
  run_case version "0.1.0" "${bin}" --version && ((pass+=1)) || ((fail+=1))
  run_case install "install: not yet implemented" "${bin}" install && ((pass+=1)) || ((fail+=1))
  run_case uninstall "uninstall: not yet implemented" "${bin}" uninstall && ((pass+=1)) || ((fail+=1))
  run_case status "status: not yet implemented" "${bin}" status && ((pass+=1)) || ((fail+=1))
  run_case stats "stats: not yet implemented" "${bin}" stats && ((pass+=1)) || ((fail+=1))
  run_case scan "scan: not yet implemented" "${bin}" scan && ((pass+=1)) || ((fail+=1))
  run_case clean "clean: not yet implemented" "${bin}" clean && ((pass+=1)) || ((fail+=1))
  run_case ballast "ballast: not yet implemented" "${bin}" ballast && ((pass+=1)) || ((fail+=1))
  run_case config "config: not yet implemented" "${bin}" config && ((pass+=1)) || ((fail+=1))
  run_case daemon "daemon: not yet implemented" "${bin}" daemon && ((pass+=1)) || ((fail+=1))

  log "summary pass=${pass} fail=${fail}"
  log "case logs at ${CASE_DIR}"

  if [[ ${fail} -gt 0 ]]; then
    exit 1
  fi
}

main "$@"
