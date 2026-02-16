# Legacy Dashboard Deprecation Decision (bd-xzt.5.6)

References:
- `docs/tui-signoff-decision.md` (go/no-go: GO)
- `docs/post-rollout-monitoring-and-handoff.md` (monitoring signals, rollback procedures)
- `docs/tui-rollout-acceptance-gates.md` (stage gates, rollback triggers)

## Decision: Retain with Deprecation Timeline

**Retain** the legacy dashboard (`src/cli/dashboard.rs`) for one full release
cycle after Stage C (new dashboard becomes default). Remove in the release
after that. Kill-switch env var and config key persist permanently as no-ops.

## Rationale

### Cost of Retention

| Cost | Magnitude | Notes |
| --- | --- | --- |
| Maintenance burden | LOW | Single 748-line file, self-contained, 18 stable tests |
| Security surface | NEGLIGIBLE | Read-only display; no file writes, no network, no user input beyond exit keys |
| Build time | NEGLIGIBLE | Compiled unconditionally (no feature gate) but small |
| Cognitive load | LOW | Isolated in `src/cli/dashboard.rs`; no imports from other modules |
| Test overhead | LOW | 18 unit tests + ~12 integration tests; ~2s total |

### Value of Retention

| Benefit | Importance | Notes |
| --- | --- | --- |
| Incident recovery fallback | HIGH | If new dashboard panics/corrupts terminal, `--legacy-dashboard` provides immediate recovery without binary rebuild |
| Offline/minimal environments | MEDIUM | Legacy works without `--features tui`; useful for stripped-down deployments |
| Operator comfort transition | MEDIUM | Familiar interface during Stage B/C for operators who haven't adopted new workflows |
| Regression detection | LOW | Parity harness (`C-01..C-18`) validates new dashboard against legacy behavior |

### Risk Assessment

Keeping legacy code has low risk because:
1. The file is self-contained — no other module imports from it
2. Its only external touch points are `DaemonState` (read-only) and `FsStatsCollector` (shared)
3. The resolution priority chain ensures kill-switch always wins
4. Tests verify both paths independently

Removing legacy too early has medium risk because:
1. Stage B/C canary may uncover PTY-specific issues only visible on real terminals
2. The new dashboard requires `--features tui` — stripped builds lose dashboard entirely
3. An emergency with no legacy fallback requires a binary rebuild to restore dashboard access

## Deprecation Timeline

### Phase 1: New Dashboard Default (Stage C Entry)

**Trigger**: Stage C entry criteria met (72h canary window clean).

Changes:
- Change `DashboardMode::default()` from `Legacy` to `New` in `config.rs`
- Add deprecation notice to `--legacy-dashboard` help text:
  `"[DEPRECATED] Use the new dashboard. Legacy will be removed in the next release."`
- Log warning when legacy path is selected: `"legacy dashboard is deprecated; use new dashboard or report issues"`
- Update README to reflect new default

**Legacy code status**: Present, functional, deprecated.

### Phase 2: Deprecation Warning Period (One Full Release Cycle)

**Duration**: One full release cycle after Phase 1.

Changes:
- `--legacy-dashboard` emits visible stderr warning on every invocation
- Release notes document deprecation with migration guide
- Kill-switch documentation updated to note it will become a no-op

**Legacy code status**: Present, functional, actively warned.

### Phase 3: Removal (Release After Phase 2)

**Trigger**: One full release cycle elapsed since Phase 1.

Changes to make:
1. Delete `src/cli/dashboard.rs` (748 lines)
2. Remove `pub mod dashboard;` from `src/cli/mod.rs`
3. Remove `DashboardRuntimeMode::LegacyFallback` variant from `tui/runtime.rs`
4. Remove `run_legacy_fallback()` and `as_legacy_config()` from `tui/runtime.rs`
5. Simplify `resolve_dashboard_runtime()` in `cli_app.rs`:
   - Remove `DashboardRuntimeSelection::Legacy` variant
   - Remove `DashboardSelectionReason::KillSwitchEnv/Config/CliFlagLegacy`
   - `--legacy-dashboard` flag → error: "Legacy dashboard removed in vX.Y. Please use the new dashboard."
6. Remove `DashboardConfig.kill_switch` field from `config.rs`
7. Remove resolution tests that test legacy-specific paths
8. Remove or update legacy-specific integration tests in `dashboard_integration_tests.rs`
9. Remove parity harness tests that compare against legacy behavior (no longer needed)

**Preserved permanently** (no-op for compatibility):
- `SBH_DASHBOARD_KILL_SWITCH` env var: checked, logged as "ignored (legacy removed)", no effect
- `dashboard.kill_switch` config key: parsed without error, ignored
- `dashboard.mode = "legacy"` config value: parsed without error, logged as "ignored"

### Feature Gate Decision

After Phase 3, the `tui` feature flag should be made a **default feature** in `Cargo.toml`:

```toml
[features]
default = ["tui"]
tui = ["crossterm"]
```

This ensures `sbh dashboard` works in standard builds. Stripped builds can
opt out with `--no-default-features` (dashboard command returns clear error).

## Summary

| Phase | Trigger | Legacy Status | Duration |
| --- | --- | --- | --- |
| Current (Stage A) | Now | Default, fully supported | Until Stage C |
| Phase 1 | Stage C entry | Present, deprecated | One release cycle |
| Phase 2 | Phase 1 + 1 release | Present, warned | One release cycle |
| Phase 3 | Phase 2 + 1 release | Removed | Permanent |

Total legacy retention from now: approximately 2-3 release cycles depending on
Stage B/C transition speed. The kill-switch env var and config key persist
permanently for compatibility.
