# `target_os` Configuration Audit

Bead: `bd-21k5.1`

Scope: original E1 audit baseline of `target_os` conditionals found in `src/` by:

```bash
rg -n 'cfg!?\s*\([^\n]*target_os|target_os\s*=|target_os\)' src
```

This document is now maintained as a closeout/status table for the original PAL
refactor audit. New platform-backend modules have additional `target_os` gates by
design; those belong with their backend tests rather than expanding this table on
every implementation change.

Classification values:

- `PAL-method`: move the OS decision behind the platform abstraction or a platform-owned helper.
- `cfg-gate-keep`: keep the conditional because the branch is inherently tied to an OS facility.
- `refactor-out`: redesign the platform-leaky call path before adding more branches.

| File | Line | Current Behavior | Classification | Target PAL Method | Notes |
|---|---:|---|---|---|---|
| `src/scanner/deletion.rs` | 757 | Gates `circuit_breaker_halts_batch_on_consecutive_failures` to Linux because the fixture relies on Unix permissions producing a deletion failure shape known to hold on Linux. | `cfg-gate-keep` | `N/A` | Test-only proof for Linux deletion semantics. Add separate macOS proof if APFS/permission behavior needs equivalent coverage. |
| `src/scanner/deletion.rs` | 825 | Gates `nested_open_file_is_detected_for_parent_directory` to Linux because it depends on `/proc/<pid>/fd` ancestor collection. | `cfg-gate-keep` | `N/A` | Linux-specific `/proc` regression. macOS PAL/libproc coverage now lives in `open_path_ancestors_uses_platform_collector_on_macos` (`bd-r7m7.3`). |
| `src/scanner/walker.rs` | 641 | Selects Linux implementation for the legacy inode-based open-file helper. | `cfg-gate-keep` | `N/A` | Runtime open-file safety uses `collect_open_path_ancestors_cached()` and active-reference indexes; this inode helper is retained for Linux-specific tests/repros. |
| `src/scanner/walker.rs` | 645 | Returns an empty open-file inode set on non-Linux for the legacy helper. | `cfg-gate-keep` | `N/A` | Not a macOS deletion-safety path after `bd-r7m7.3`; cleanup executor and scan scoring use PAL-backed path/open-reference evidence. |
| `src/scanner/walker.rs` | 651 | Compiles `/proc` inode scan only on Linux. | `cfg-gate-keep` | `N/A` | Correct for the legacy Linux inode helper; macOS runtime coverage is through `MacOsPal::open_files_under()`. |
| `src/scanner/walker.rs` | 709 | Allows `OPEN_FILES_SCAN_BUDGET` to be dead code off Linux. | `cfg-gate-keep` | `N/A` | This constant now only budgets the Linux `/proc` inode/path backend. |
| `src/scanner/walker.rs` | 714 | Allows `OPEN_FILES_MAX_PIDS` to be dead code off Linux. | `cfg-gate-keep` | `N/A` | Same Linux `/proc` backend limit as `OPEN_FILES_SCAN_BUDGET`. |
| `src/scanner/walker.rs` | 688 | Dispatches open-path ancestor collection to Linux `/proc` or macOS PAL/libproc by target. | `cfg-gate-keep` | `N/A` | Closed for cleanup executor preflight by `bd-r7m7.3`; the target gate selects backend code rather than skipping macOS. |
| `src/scanner/walker.rs` | 692 | Uses the compile-time PAL on macOS/non-Linux targets instead of returning an empty, complete ancestor set. | `cfg-gate-keep` | `N/A` | Closed for macOS by `bd-r7m7.3`. If more OSes are added, this branch must grow a backend rather than silently returning empty. |
| `src/scanner/walker.rs` | 699 | Compiles `/proc` path-ancestor scan only on Linux. | `cfg-gate-keep` | `N/A` | Correct as the Linux backend; macOS cleanup preflight uses the PAL/libproc backend. |
| `src/scanner/walker.rs` | 1165 | Allows `OpenPathCache` to be dead code off Linux. | `cfg-gate-keep` | `N/A` | Legacy inode-cache helper is no longer the cleanup executor preflight. Current runtime path checks use cached open-path ancestors. |
| `src/scanner/walker.rs` | 1182 | Uses Linux subtree scan in `OpenPathCache::is_path_open`. | `cfg-gate-keep` | `N/A` | Retained for Linux inode-helper tests; runtime scan/clean code calls `is_path_open_by_ancestor()`. |
| `src/scanner/walker.rs` | 1186 | Returns `false` for `OpenPathCache::is_path_open` on non-Linux. | `cfg-gate-keep` | `N/A` | Not a macOS runtime safety veto after `bd-r7m7.3`; keep isolated until the legacy helper is removed. |
| `src/scanner/walker.rs` | 1193 | Compiles recursive inode matching only on Linux. | `cfg-gate-keep` | `N/A` | Linux-specific implementation detail for the legacy helper. |
| `src/scanner/walker.rs` | 1877 | Gates direct inode-helper open-file test to Linux. | `cfg-gate-keep` | `N/A` | Correct backend-specific test for the legacy helper; shared runtime ancestor coverage is `open_path_ancestor_chain_stops_at_outermost_matching_root`. |
| `src/scanner/walker.rs` | 1903 | Gates nested-root open ancestor ordering test to Linux. | `cfg-gate-keep` | `N/A` | Linux backend regression; macOS backend coverage is `open_path_ancestors_uses_platform_collector_on_macos`. |
| `src/cli_app.rs` | 698 | Rejects `--systemd` unless compiled on Linux. | `PAL-method` | `service_manager_kind()` | Install validation should ask the detected platform/service manager, not inline host cfgs. |
| `src/cli_app.rs` | 703 | Rejects `--launchd` unless compiled on macOS. | `PAL-method` | `service_manager_kind()` | Needed for automatic install detection and clearer unsupported-service errors. |
| `src/cli_app.rs` | 2355 | Collects process blame through the PAL via `process_list()`, `process_io()`, and `open_files_under()`. | `PAL-method` | `process_blame_snapshot()` | Closed for macOS: `MacOsPal` supplies libproc/rusage/open-file data, and `macos_synthetic_writer_surfaces_in_blame_top_rows` covers the CLI path. |
| `src/daemon/self_monitor.rs` | 508 | Reads daemon RSS through `Platform::self_stats()`, falling back to zero only if the PAL call fails. | `PAL-method` | `self_stats()` | Closed for macOS: `MacOsPal::self_stats()` uses Mach task usage plus libproc rusage, with PAL backend and self-monitor contract tests. |
| `src/daemon/self_monitor.rs` | 978 | Unit-tests that self-monitor RSS comes from platform `self_stats()`. | `cfg-gate-keep` | `N/A` | Host-independent `MockPlatform` contract covers the daemon path; backend-specific Linux/macOS assertions live under their PAL modules. |
| `src/daemon/notifications.rs` | 413 | Sends desktop notifications through Linux `notify-send`. | `cfg-gate-keep` | `N/A` | This is a small OS command adapter. Keep gated unless notification delivery moves into PAL. |
| `src/daemon/notifications.rs` | 431 | Sends desktop notifications through macOS `osascript`. | `cfg-gate-keep` | `N/A` | Existing macOS branch is appropriate; keep escaping and child reaping local to the adapter. |
| `src/daemon/notifications.rs` | 447 | Suppresses unused `urgency` on non-Linux. | `cfg-gate-keep` | `N/A` | Incidental to Linux `notify-send` urgency. |
| `src/daemon/service.rs` | 220 | Chooses `wheel` as recommended root group on macOS and `root` elsewhere. | `PAL-method` | `service_ownership_policy()` | Service-manager abstraction should own recommended owner/group text. |
| `src/platform/pal.rs` | 738 | Detects Linux or macOS by returning the compile-time `platform::current()` PAL backend. | `cfg-gate-keep` | `N/A` | Closed for macOS: `platform::current()` selects `LinuxPal` or `MacOsPal` at compile time. |
| `src/platform/pal.rs` | 742 | Rejects targets other than Linux/macOS as unsupported. | `cfg-gate-keep` | `N/A` | Correct until another OS is added; Linux and macOS are now the supported runtime set. |
| `src/cli/wizard.rs` | 177 | Auto-selects launchd when compiled on macOS. | `PAL-method` | `default_service_choice()` | Wizard auto mode should use detected platform capabilities. |
| `src/cli/wizard.rs` | 179 | Auto-selects systemd when compiled on Linux. | `PAL-method` | `default_service_choice()` | Same service-default decision as the macOS branch. |
| `src/cli/wizard.rs` | 281 | Uses launchd as the interactive default on macOS. | `PAL-method` | `default_service_choice()` | Interactive wizard should share auto-mode service detection. |
| `src/cli/wizard.rs` | 283 | Uses systemd as the interactive default on Linux. | `PAL-method` | `default_service_choice()` | Avoid duplicating platform-default logic across prompts and tests. |
| `src/cli/wizard.rs` | 550 | Test expects systemd auto-detection on Linux. | `PAL-method` | `default_service_choice()` | Replace host-cfg expectations with injected fake-platform cases plus backend smoke tests. |
| `src/cli/wizard.rs` | 552 | Test expects launchd auto-detection on macOS. | `PAL-method` | `default_service_choice()` | Same test refactor as the Linux branch. |
| `src/cli/wizard.rs` | 590 | Interactive default test asserts Linux systemd only. | `PAL-method` | `default_service_choice()` | Missing macOS assertion is a current coverage gap. |
| `src/cli/wizard.rs` | 813 | Invalid service input falls back to Linux systemd. | `PAL-method` | `default_service_choice()` | Should exercise fallback through an injectable platform default. |
| `src/cli/wizard.rs` | 815 | Invalid service input falls back to macOS launchd. | `PAL-method` | `default_service_choice()` | Same shared default-service contract. |
| `src/cli/wizard.rs` | 949 | Unit test checks Linux auto-detection path. | `PAL-method` | `default_service_choice()` | Prefer explicit Linux/macOS test cases independent of the runner OS. |
| `src/cli/wizard.rs` | 951 | Unit test checks macOS auto-detection path. | `PAL-method` | `default_service_choice()` | Prefer explicit Linux/macOS test cases independent of the runner OS. |
| `src/ballast/manager.rs` | 588 | Tries Linux `fallocate` for fast ballast allocation before random-data fallback. | `PAL-method` | `preallocate_file(file, offset, len)` | macOS should use `fcntl(F_PREALLOCATE)` or report unsupported so fallback remains explicit. |
| `src/ballast/manager.rs` | 638 | Compiles the `nix::fcntl::fallocate` helper only on Linux. | `PAL-method` | `preallocate_file(file, offset, len)` | Move into Linux platform backend; add macOS backend and fallback tests. |
| `src/daemon/signals.rs` | 238 | Sends watchdog status through Linux `sd_notify`. | `cfg-gate-keep` | `N/A` | Systemd notification is Linux-specific and should remain gated in the systemd service manager. |
| `src/daemon/signals.rs` | 244 | No-ops watchdog notification on non-Linux. | `cfg-gate-keep` | `N/A` | Correct for launchd; launchd health is managed differently. |
| `src/daemon/signals.rs` | 251 | Compiles `UnixDatagram` systemd notification helper only on Linux. | `cfg-gate-keep` | `N/A` | Keep with systemd-specific implementation. |

## Immediate PAL Surface Implied by This Audit

The repeated sites point to these first-class platform methods or platform-owned helpers for `bd-21k5.2`:

| Capability | Linux Backend | macOS Backend | Current Risk |
|---|---|---|---|
| `detect_platform()` | `LinuxPal` via `platform::current()` | `MacOsPal` via `platform::current()` | Closed for macOS; unsupported targets fail explicitly. |
| `service_manager_kind()` / `default_service_choice()` | systemd when available | launchd | Install and wizard defaults duplicate cfg logic. |
| `service_ownership_policy()` | `root:root` | `root:wheel` | Recommendation text is scattered in service code. |
| `process_blame_snapshot()` | `/proc/<pid>` | libproc process/cwd/open-file APIs | Closed for macOS; `sbh blame` now uses PAL-backed process and open-file attribution. |
| `self_stats()` | `/proc/self/status` plus `/proc/self/io` | Mach task usage plus libproc rusage | Closed for macOS; daemon self-monitor consumes the PAL method. |
| `open_file_keys()` / `open_path_ancestors()` / `is_path_open()` | `/proc/<pid>/fd` | libproc fd/path APIs | Executor preflight parity is closed for macOS by `bd-r7m7.3`; legacy inode-key fallback remains Linux-specific. |
| `preallocate_file(file, offset, len)` | `fallocate` | `fcntl(F_PREALLOCATE)` or unsupported | macOS ballast provisioning falls back to slow random writes. |

## Sign-Off

All original E1 `target_os` audit rows are classified above and no rows are intentionally left blank. `N/A` means the site should remain an OS-specific gate rather than become a PAL method. Closed rows are retained to show which macOS parity gaps have been resolved by later PAL work.
