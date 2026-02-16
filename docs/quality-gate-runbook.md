# Quality-Gate Runbook (bd-xzt.4.6)

This runbook defines the ordered gate sequence for validating correctness,
UX contract compliance, performance, and reliability of the SBH dashboard.

References:
- `docs/tui-acceptance-gates-and-budgets.md` (gate policy, budgets, thresholds)
- `docs/tui-rollout-acceptance-gates.md` (rollout stage gates, rollback triggers)
- `docs/testing-and-logging.md` (test/log conventions)

Automation: `scripts/quality-gate.sh` implements this sequence and emits
machine-readable artifacts per section 8 of `tui-rollout-acceptance-gates.md`.

## Gate Execution Order

Gates run in strict dependency order. A failure at any HARD gate aborts
subsequent stages. SOFT gate failures are recorded but do not block.

```
Stage 1: Format         (local)    → correctness
Stage 2: Check          (rch)      → correctness
Stage 3: Clippy         (rch)      → correctness + style
Stage 4: Unit tests     (rch)      → correctness
Stage 5: Binary tests   (rch)      → CLI correctness
Stage 6: TUI tests      (rch)      → dashboard correctness
Stage 7: Integration    (rch)      → cross-component correctness
Stage 8: Decision plane (rch)      → invariant proofs
Stage 9: E2E            (local)    → system-level behavior
Stage 10: Stress/perf   (rch)      → reliability + performance
```

## Stage Details

### Stage 1: Formatting (HARD)

```bash
cargo fmt --check
```

**Gate**: `G-PAR-CLI-01` (partial)
**Quality dimension**: Correctness (code hygiene)
**Artifacts**: None (pass/fail only)
**Failure triage**: Run `cargo fmt` and commit.

---

### Stage 2: Build Check (HARD)

```bash
rch exec "cargo check --all-targets --features tui"
```

**Gate**: `G-PAR-CLI-01`, `C-07`
**Quality dimension**: Correctness (compilation)
**Artifacts**: stderr capture
**Failure triage**:
- Missing type/field → check recent commits for struct changes
- Feature gate error → verify `Cargo.toml` feature dependencies
- Dependency error → check `Cargo.lock` for version conflicts

---

### Stage 3: Clippy Lint (HARD)

```bash
rch exec "cargo clippy --all-targets --features tui -- -D warnings"
```

**Gate**: `G-PAR-CLI-01`
**Quality dimension**: Correctness + style
**Artifacts**: stderr capture with lint locations
**Failure triage**:
- `clippy::pedantic` / `clippy::nursery` → see `Cargo.toml` for allowed exceptions
- New lint in stable toolchain update → add targeted `#[allow]` with justification
- Cross-file lint (unused import/dead code) → check if another agent removed the callsite

---

### Stage 4: Library Unit Tests (HARD)

```bash
rch exec "cargo test --lib --features tui --nocapture"
```

**Gate**: `G-PAR-CLI-01`, `G-PAR-DATA-02`, `G-PAR-TERM-03`, `G-PAR-IA-04`
**Quality dimension**: Correctness (all components)
**Artifacts**: stdout/stderr with test names and assertions
**Test suites exercised**:
- Core: config, errors, platform, protection, EWMA, PID, scoring, walker
- Ballast: manager, coordinator, pressure response
- Scanner: pattern registry, deletion executor
- Logger: SQLite, JSONL, stats engine, notifications
- Daemon: main loop, signal handling, self-monitoring
- Decision plane: invariants, property tests, guard state machine
- TUI: model reducers, adapters, input handling, render, layout, widgets,
  preferences, telemetry, incident, theme, update logic

**TUI-specific test modules** (require `--features tui`):
| Module | Tests | Coverage |
| --- | --- | --- |
| `test_unit_coverage` | model/adapter/keymap/render helpers | C-08..C-18 |
| `test_properties` | reducer invariants, navigation, scheduler | G-PAR-IA-04 |
| `test_replay` | deterministic state replay regression | C-13, C-17 |
| `test_fault_injection` | adapter/state degradation and recovery | G-ERR-DEGRADE-04, G-ERR-RECOVER-05 |
| `test_snapshot_golden` | per-screen golden frame hashes | C-18, G-PAR-TERM-03 |
| `test_scenario_drills` | multi-phase operator workflow e2e | G-PAR-IA-04 |
| `test_operator_benchmark` | task-time/error-rate workflow validation | operator workflow gates |
| `test_stress` | long-run stability, burst handling | G-PERF-MEM-08, G-ERR-RENDER-06 |
| `parity_harness` | legacy-vs-new frozen contract matrix | G-PAR-CLI-01..G-PAR-TERM-03 |

**Failure triage**:
- Single test failure → read assertion message, check test name for contract ID
- Batch failure in one module → likely a struct/enum change broke a seam
- Intermittent failure → check for test ordering dependency (run isolated with `-- test_name`)
- TUI test failure without `--features tui` → ensure feature flag is included

---

### Stage 5: Binary Tests (HARD)

```bash
rch exec "cargo test --bin sbh --features tui --nocapture"
```

**Gate**: `G-PAR-CLI-01`, `C-01..C-06`
**Quality dimension**: CLI correctness
**Artifacts**: stdout/stderr
**Test suites exercised**:
- CLI argument parsing and routing
- Dashboard mode resolution (7-level priority chain)
- Config validation and env var overrides
- Error display formatting

**Failure triage**:
- clap argument conflict → check for duplicate arg names across subcommands
- Dashboard resolution → verify `DashboardSelectionReason` priority chain in cli_app.rs

---

### Stage 6: TUI-Specific Tests (HARD)

```bash
rch exec "cargo test --lib --features tui -- tui:: --nocapture"
```

This is a focused re-run of TUI module tests to isolate dashboard regressions.

**Gate**: `G-PAR-DATA-02`, `G-PAR-TERM-03`, `G-PAR-IA-04`, `C-07..C-18`
**Quality dimension**: Dashboard correctness
**Artifacts**: per-test pass/fail with assertion context

**Failure triage by test module**:

| Module | Likely cause | Action |
| --- | --- | --- |
| `test_unit_coverage` | Model field added/renamed | Update test fixtures |
| `test_properties` | Reducer invariant violated | Check update.rs for logic change |
| `test_replay` | State transition change | Regenerate replay fixtures or update expectations |
| `test_fault_injection` | Adapter/fallback path change | Verify DashboardStateAdapter still degrades safely |
| `test_snapshot_golden` | Render output changed | Compare golden hashes; update if intentional |
| `test_scenario_drills` | Cross-component workflow broke | Check which phase failed for component isolation |
| `test_operator_benchmark` | Workflow step count changed | Update benchmark expectations |
| `test_stress` | Memory/stability regression | Profile with sustained load |
| `parity_harness` | Legacy contract violation | Map failure to C-xx contract ID |

---

### Stage 7: Integration Tests (HARD)

```bash
rch exec "cargo test --test integration_tests --features tui --nocapture"
rch exec "cargo test --test dashboard_integration_tests --features tui --nocapture"
rch exec "cargo test --test fallback_verification --features tui --nocapture"
rch exec "cargo test --test installer_e2e --nocapture"
```

**Gate**: `G-PAR-CLI-01`, `G-PAR-DATA-02`, `G-PAR-TERM-03`
**Quality dimension**: Cross-component correctness
**Artifacts**: stdout/stderr per test binary, 14-day retention

**Test files and coverage**:
| File | Tests | Coverage |
| --- | --- | --- |
| `integration_tests.rs` | CLI smoke, full-pipeline, decision-plane e2e | C-01..C-06, C-13 |
| `dashboard_integration_tests.rs` | Command semantics, state-file contract, adapter staleness | C-08..C-13 |
| `fallback_verification.rs` | Config rollback, env overrides, runtime conversion, degradation chains | C-14..C-18 |
| `installer_e2e.rs` | Install/update/rollback orchestration | installer contracts |

**Failure triage**:
- State-file contract → check `DaemonState` serialization format
- Adapter staleness → verify `STALENESS_THRESHOLD_SECS` constant
- Fallback path → check `resolve_dashboard_runtime()` priority chain
- Installer → verify `InstallOrchestrator` step ordering

---

### Stage 8: Decision Plane Proofs (HARD)

```bash
rch exec "cargo test --test proof_harness --nocapture"
rch exec "cargo test --test decision_plane_e2e --nocapture"
```

**Gate**: Invariant proofs (not directly mapped to G-PAR but load-bearing for scanner safety)
**Quality dimension**: Correctness (mathematical invariants)
**Artifacts**: stdout/stderr, 30-day retention
**Failure triage**:
- Ranking stability → scoring weights or RRF fusion changed
- Monotonicity → factor computation violated expected ordering
- Guard state machine → circuit breaker or policy engine transition broken
- Merkle equivalence → hash computation or tree structure changed

---

### Stage 9: End-to-End Suite (HARD)

```bash
./scripts/e2e_test.sh
```

**Gate**: `G-PAR-CLI-01`, `G-PAR-DATA-02`, `G-PAR-TERM-03`
**Quality dimension**: System-level behavior
**Artifacts**: `$SBH_E2E_LOG_DIR` (defaults to `/tmp/sbh-e2e-TIMESTAMP/`)
**Environment variables**:
| Variable | Default | Purpose |
| --- | --- | --- |
| `SBH_E2E_LOG_DIR` | `/tmp/sbh-e2e-*` | Log output directory |
| `SBH_E2E_CASE_TIMEOUT` | `60` | Per-case timeout (seconds) |
| `SBH_E2E_SUITE_BUDGET` | `600` | Total suite time budget (seconds) |
| `SBH_E2E_FLAKY_RETRIES` | `1` | Retry count for flaky tests |
| `SBH_E2E_BIN` | auto-detected | Override binary path |

**Coverage** (33 sections):
Core CLI, config system, status, scan, clean, ballast lifecycle, protection
markers, check, blame, tune, stats, emergency, scoring determinism, daemon
stubs, output formatting, installer, offline update, large-tree perf,
concurrent CLI, multi-path scan, JSON coverage.

**Failure triage**:
- Build failure → ensure `cargo build --release` completed
- Timeout → check `SBH_E2E_CASE_TIMEOUT`, look for hung process
- Assertion failure → read case name, check stdout/stderr in log dir
- Performance failure (large tree) → profile scan path, check walker parallelism

---

### Stage 10: Stress and Performance (HARD for budgets, SOFT for CPU)

```bash
rch exec "cargo test --test stress_tests --nocapture"
rch exec "cargo test --test stress_harness --nocapture"
```

**Gate**: `G-PERF-*`, `G-ERR-*`
**Quality dimension**: Reliability + performance
**Artifacts**: stdout/stderr with timing data, 14-day retention
**Failure triage**:
- Memory growth → check for unbounded Vec/HashMap growth in model or adapter
- CPU spike → profile hot path (likely render or adapter polling)
- Panic → check backtrace for unwrap/expect in production code path
- Flaky → check for time-dependent assertions, increase tolerance

---

## Gate Report Schema

Each gate stage emits a JSON record to the gate report artifact:

```json
{
  "gate_id": "STAGE_4_LIB_TESTS",
  "mapped_gates": ["G-PAR-CLI-01", "G-PAR-DATA-02"],
  "status": "pass",
  "command": "cargo test --lib --features tui --nocapture",
  "execution": "rch",
  "started_at": "2026-02-16T08:00:00Z",
  "finished_at": "2026-02-16T08:01:30Z",
  "duration_secs": 90,
  "test_count": 1775,
  "failures": 0,
  "artifact_path": "/tmp/sbh-gates-TRACE/stage4-lib-tests.log",
  "failure_summary": null,
  "environment": {
    "os": "linux",
    "rustc": "1.85.0",
    "features": "tui"
  }
}
```

Required fields: `gate_id`, `status`, `command`, `started_at`, `finished_at`,
`duration_secs`, `failures`.

## Failure Escalation

1. **HARD gate failure**: Cutover blocked. Create regression bead, link to
   failing gate ID, fix, and re-run full gate sequence with same trace context.
2. **SOFT gate failure**: Record waiver with mitigation, owner, and fix bead.
   Promotion proceeds but waiver is visible in signoff artifact.
3. **Intermittent failure**: Run failing stage in isolation 3x. If it passes
   consistently, flag as flaky and add to `SBH_E2E_FLAKY_RETRIES` list.
   If it fails 2/3 times, treat as HARD failure.

## Quick Reference: Full Gate Sequence

```bash
# Stage 1: Format (local)
cargo fmt --check

# Stage 2: Build check (rch)
rch exec "cargo check --all-targets --features tui"

# Stage 3: Clippy (rch)
rch exec "cargo clippy --all-targets --features tui -- -D warnings"

# Stage 4: Library tests (rch)
rch exec "cargo test --lib --features tui --nocapture"

# Stage 5: Binary tests (rch)
rch exec "cargo test --bin sbh --features tui --nocapture"

# Stage 6: TUI-focused tests (rch, subset of Stage 4 for isolation)
rch exec "cargo test --lib --features tui -- tui:: --nocapture"

# Stage 7: Integration tests (rch)
rch exec "cargo test --test integration_tests --features tui --nocapture"
rch exec "cargo test --test dashboard_integration_tests --features tui --nocapture"
rch exec "cargo test --test fallback_verification --features tui --nocapture"
rch exec "cargo test --test installer_e2e --nocapture"

# Stage 8: Decision plane proofs (rch)
rch exec "cargo test --test proof_harness --nocapture"
rch exec "cargo test --test decision_plane_e2e --nocapture"

# Stage 9: E2E suite (local, needs binary)
./scripts/e2e_test.sh

# Stage 10: Stress and performance (rch)
rch exec "cargo test --test stress_tests --nocapture"
rch exec "cargo test --test stress_harness --nocapture"
```

## Contract-to-Test Mapping

| Contract | Test Location | Stage |
| --- | --- | --- |
| C-01..C-06 | `cli_app::tests`, `integration_tests.rs`, `e2e_test.sh` sections 1-8 | 5, 7, 9 |
| C-07 | `cargo check --features tui` (build gate) | 2 |
| C-08..C-13 | `dashboard_integration_tests.rs`, `tui::test_unit_coverage`, `tui::adapters::tests` | 4, 6, 7 |
| C-14 | `tui::test_unit_coverage` (atomic write), `fallback_verification.rs` | 4, 7 |
| C-15..C-16 | `tui::test_properties` (exit keys), `tui::test_replay` (terminal restore) | 4, 6 |
| C-17 | `tui::test_fault_injection` (degraded mode), `fallback_verification.rs` | 4, 6, 7 |
| C-18 | `tui::test_snapshot_golden` (required sections), `tui::test_unit_coverage` | 4, 6 |
| G-PAR-IA-04 | `tui::test_scenario_drills`, `tui::test_operator_benchmark` | 4, 6 |
| G-PERF-* | `stress_tests.rs`, `stress_harness.rs`, `tui::test_stress` | 4, 10 |
| G-ERR-PANIC-01 | All test stages (panic = test failure) | 4-10 |
| G-ERR-DEGRADE-04 | `tui::test_fault_injection` | 4, 6 |
| G-ERR-RECOVER-05 | `tui::test_fault_injection`, `fallback_verification.rs` | 4, 6, 7 |
