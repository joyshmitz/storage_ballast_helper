# `target_os` Configuration Audit

Bead: `bd-21k5.1`

Scope: every `target_os` conditional found in `src/` by:

```bash
rg -n 'cfg!?\s*\([^\n]*target_os|target_os\s*=|target_os\)' src
```

Classification values:

- `PAL-method`: move the OS decision behind the platform abstraction or a platform-owned helper.
- `cfg-gate-keep`: keep the conditional because the branch is inherently tied to an OS facility.
- `refactor-out`: redesign the platform-leaky call path before adding more branches.

| File | Line | Current Behavior | Classification | Target PAL Method | Notes |
|---|---:|---|---|---|---|
| `src/scanner/deletion.rs` | 757 | Gates `circuit_breaker_halts_batch_on_consecutive_failures` to Linux because the fixture relies on Unix permissions producing a deletion failure shape known to hold on Linux. | `cfg-gate-keep` | `N/A` | Test-only proof for Linux deletion semantics. Add separate macOS proof if APFS/permission behavior needs equivalent coverage. |
| `src/scanner/deletion.rs` | 825 | Gates `nested_open_file_is_detected_for_parent_directory` to Linux because it depends on `/proc/<pid>/fd` ancestor collection. | `PAL-method` | `open_path_ancestors(root_paths)` | Once macOS libproc support exists, this should be a platform contract test plus Linux/macOS backend tests. |
| `src/scanner/walker.rs` | 604 | Selects Linux implementation for inode-based open-file collection. | `PAL-method` | `open_file_keys()` | Deletion safety should not silently downgrade on macOS. Linux can keep `/proc`; macOS needs libproc/fd inspection. |
| `src/scanner/walker.rs` | 608 | Returns an empty open-file set on every non-Linux target for the legacy inode fallback. | `PAL-method` | `open_file_keys()` | The newer PAL active-reference path reports macOS user-scope visibility as incomplete; this fallback should still move behind a platform-owned result before deletion preflight is fully symmetric. |
| `src/scanner/walker.rs` | 614 | Compiles `/proc` inode scan only on Linux. | `PAL-method` | `open_file_keys()` | Move under Linux PAL backend or a Linux-specific implementation module. |
| `src/scanner/walker.rs` | 672 | Allows `OPEN_FILES_SCAN_BUDGET` to be dead code off Linux. | `PAL-method` | `open_file_scan_limits()` | This constant belongs with the Linux open-file backend; macOS should define its own limits. |
| `src/scanner/walker.rs` | 677 | Allows `OPEN_FILES_MAX_PIDS` to be dead code off Linux. | `PAL-method` | `open_file_scan_limits()` | Same implementation-owned limit issue as `OPEN_FILES_SCAN_BUDGET`. |
| `src/scanner/walker.rs` | 688 | Selects Linux implementation for open-path ancestor collection. | `PAL-method` | `open_path_ancestors(root_paths)` | This is the safer subtree-open veto path and should become a platform capability. |
| `src/scanner/walker.rs` | 692 | Returns an empty, complete ancestor set on non-Linux for the legacy `/proc` ancestor fallback. | `PAL-method` | `open_path_ancestors(root_paths)` | The PAL active-reference collector now emits `fd check incomplete: other-user processes not visible` for user-scope macOS runs; this fallback remains a cleanup-preflight parity gap. |
| `src/scanner/walker.rs` | 699 | Compiles `/proc` path-ancestor scan only on Linux. | `PAL-method` | `open_path_ancestors(root_paths)` | Linux backend can retain `/proc`; macOS backend should use libproc path/fd APIs. |
| `src/scanner/walker.rs` | 806 | Allows `OpenPathCache` to be dead code off Linux. | `PAL-method` | `open_path_cache()` | Cache should be platform-owned or backed by a trait object so non-Linux code has a real implementation. |
| `src/scanner/walker.rs` | 823 | Uses Linux subtree scan in `OpenPathCache::is_path_open`. | `PAL-method` | `is_path_open(path)` | Should delegate to platform open-file inspection. |
| `src/scanner/walker.rs` | 827 | Returns `false` for `OpenPathCache::is_path_open` on non-Linux. | `PAL-method` | `is_path_open(path)` | Current macOS behavior weakens the open-file safety veto. |
| `src/scanner/walker.rs` | 834 | Compiles recursive inode matching only on Linux. | `PAL-method` | `is_path_open(path)` | Move to Linux backend after the trait surface is frozen. |
| `src/scanner/walker.rs` | 1255 | Gates sibling-subtree open ancestor test to Linux. | `PAL-method` | `open_path_ancestors(root_paths)` | Convert to shared fake-platform contract test plus backend-specific Linux/macOS tests. |
| `src/scanner/walker.rs` | 1281 | Gates nested-root open ancestor ordering test to Linux. | `PAL-method` | `open_path_ancestors(root_paths)` | Same open-ancestor platform contract as the sibling-subtree test. |
| `src/cli_app.rs` | 698 | Rejects `--systemd` unless compiled on Linux. | `PAL-method` | `service_manager_kind()` | Install validation should ask the detected platform/service manager, not inline host cfgs. |
| `src/cli_app.rs` | 703 | Rejects `--launchd` unless compiled on macOS. | `PAL-method` | `service_manager_kind()` | Needed for automatic install detection and clearer unsupported-service errors. |
| `src/cli_app.rs` | 1506 | Compiles process blame attribution only on Linux through `/proc`. | `PAL-method` | `process_blame_snapshot()` | macOS needs libproc-backed process/cwd discovery instead of returning no attribution. |
| `src/daemon/self_monitor.rs` | 524 | Reads RSS through Linux implementation. | `PAL-method` | `self_stats()` | The daemon self-monitor should get RSS from PAL, with Linux `/proc` and macOS `proc_pid_rusage` or equivalent. |
| `src/daemon/self_monitor.rs` | 528 | Returns zero RSS on non-Linux. | `PAL-method` | `self_stats()` | Returning zero hides daemon memory growth on macOS. |
| `src/daemon/self_monitor.rs` | 534 | Compiles `/proc/self/status` RSS parser only on Linux. | `PAL-method` | `self_stats()` | Move into Linux PAL backend. |
| `src/daemon/self_monitor.rs` | 905 | Gates RSS nonzero test to Linux. | `PAL-method` | `self_stats()` | Convert to PAL contract test with backend-specific Linux/macOS runtime assertions. |
| `src/daemon/notifications.rs` | 413 | Sends desktop notifications through Linux `notify-send`. | `cfg-gate-keep` | `N/A` | This is a small OS command adapter. Keep gated unless notification delivery moves into PAL. |
| `src/daemon/notifications.rs` | 431 | Sends desktop notifications through macOS `osascript`. | `cfg-gate-keep` | `N/A` | Existing macOS branch is appropriate; keep escaping and child reaping local to the adapter. |
| `src/daemon/notifications.rs` | 447 | Suppresses unused `urgency` on non-Linux. | `cfg-gate-keep` | `N/A` | Incidental to Linux `notify-send` urgency. |
| `src/daemon/service.rs` | 220 | Chooses `wheel` as recommended root group on macOS and `root` elsewhere. | `PAL-method` | `service_ownership_policy()` | Service-manager abstraction should own recommended owner/group text. |
| `src/platform/pal.rs` | 276 | Detects Linux by returning `LinuxPlatform::new()`. | `refactor-out` | `detect_platform()` | Factory currently hard-codes a single backend. Add macOS backend instead of growing scattered call-site checks. |
| `src/platform/pal.rs` | 280 | Rejects every non-Linux target as unsupported. | `refactor-out` | `detect_platform()` | This is the main blocker for runtime macOS support. It must instantiate `MacPlatform`. |
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
| `detect_platform()` | `LinuxPlatform::new()` | `MacPlatform::new()` | macOS currently fails at PAL detection. |
| `service_manager_kind()` / `default_service_choice()` | systemd when available | launchd | Install and wizard defaults duplicate cfg logic. |
| `service_ownership_policy()` | `root:root` | `root:wheel` | Recommendation text is scattered in service code. |
| `process_blame_snapshot()` | `/proc/<pid>` | libproc process/cwd APIs | `sbh blame` is Linux-only. |
| `self_stats()` | `/proc/self/status` | `proc_pid_rusage` or equivalent | macOS daemon RSS currently reports zero. |
| `open_file_keys()` / `open_path_ancestors()` / `is_path_open()` | `/proc/<pid>/fd` | libproc fd/path APIs | macOS cleanup can miss open-file vetoes. |
| `preallocate_file(file, offset, len)` | `fallocate` | `fcntl(F_PREALLOCATE)` or unsupported | macOS ballast provisioning falls back to slow random writes. |

## Sign-Off

All current `target_os` sites in `src/` are classified above. No rows are intentionally left blank; `N/A` means the site should remain an OS-specific gate rather than become a PAL method.
