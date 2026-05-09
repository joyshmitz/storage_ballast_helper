# macOS/Linux Parity Prompt-To-Artifact Completion Audit

Bead: `bd-r7m7.11`
Refresh beads: `bd-r7m7.12`, `bd-r7m7.13`, `bd-r7m7.15`, `bd-r7m7.16`
Parent: `bd-r7m7`
Last audited: 2026-05-09 10:13 UTC
Evidence snapshot: the audit intentionally avoids pinning exact commit hashes or
GitHub Actions run ids because every audit-only commit would make those literals
stale. Before any close decision, refresh the live head and newest run with:

```bash
git rev-parse HEAD
git status --short --branch
gh run list --repo Dicklesworthstone/storage_ballast_helper --branch main --limit 5 \
  --json databaseId,headSha,status,conclusion,workflowName,url,createdAt,event
```

Then inspect the run for the current head with:

```bash
gh run view <latest-run> --repo Dicklesworthstone/storage_ballast_helper --json status,conclusion,headSha,jobs
```

This is the closeout gate for the active objective: make `sbh` seamlessly
support macOS in addition to Linux, with automatic platform detection during
installation and runtime, plus testing infrastructure that validates both
environments.

## Objective Restated

The goal is not complete until the repository and release system prove these
operator-visible outcomes:

- One source tree and one `sbh` CLI surface support Linux and macOS through the
  PAL rather than platform-specific user workflows.
- Install, uninstall, service control, daemon, status, check, scan, clean,
  emergency, blame, ballast, dashboard, log, setup, config, protect, unprotect,
  and tune resolve the correct Linux or macOS behavior automatically.
- macOS service lifecycle uses launchd while Linux continues to use systemd.
- macOS disk accounting handles APFS capacity, purgeable-space risk, snapshots,
  and `/private/tmp` behavior.
- macOS cleanup rules rank real reclaim candidates such as Xcode DerivedData,
  CoreSimulator caches, Electron caches, user-named trash directories, and
  `*_target` build directories without touching sacred user data.
- Deletion safety is enforced in scanner pre-scan, normal walker traversal, and
  executor preflight, including protected paths, `.sbh-protect` markers, active
  build file handles, and source repositories.
- macOS process and daemon health data come from Mach, sysctl, and libproc
  backends rather than unsupported Linux-only code paths.
- Release artifacts for macOS are signed, notarized, checked, and distributed by
  GitHub Releases and Homebrew.
- CI proves Linux and macOS behavior with unit, integration, snapshot, release,
  coverage, benchmark, and formula lanes.
- README and docs let a Mac user install, configure, verify, and diagnose the
  release path without reading workflow internals.

## Current Tracker And CI State

- `bd-r7m7` remains open. Use live `br epic status --json` output before any
  close decision because audit refresh beads change child counts.
- `bd-ykwh` remains open. The remaining work is release-credential and Homebrew
  distribution plumbing.
- `br ready --json` returned `[]`; remaining open actionable release work was
  blocked or already assigned at audit time.
- In-progress release blockers are `bd-ykwh.2`, `bd-ykwh.3`, `bd-ykwh.10`, and
  `bd-ykwh.13`.
- `bd-ykwh.20` is closed; release CI now runs `spctl -a -t execute -vv` after
  notarization acceptance and before packaging macOS tarballs.
- The newest CI run for the current head was queued at audit time with
  `Homebrew Formula Validation` and `Format + Lint` materialized but not green.
  Do not treat queued CI as proof; inspect the latest run for the final pushed
  head before closing.
- Local Homebrew formula checks passed at refresh time: `brew style
  packaging/homebrew/Formula/sbh.rb` and the generated-formula placeholder
  replacement path both reported no style offenses.

## Checklist

| Requirement | Evidence | Current Status |
|---|---|---|
| Fresh macOS install auto-detects launchd and status works | `src/platform/macos/pal.rs`, `src/daemon/service.rs`, `tests/integration_tests.rs::macos_launchd_user_service_lifecycle_bootstrap_kickstart_bootout`, `.github/workflows/ci.yml` `macos-platform`, `docs/macos.md` | Repo-side implementation and CI coverage exist. Signed release install remains blocked by `bd-ykwh.2`, `bd-ykwh.3`, and `bd-ykwh.13`. |
| Status/check JSON shape and APFS accounting match macOS reality | `tests/integration_tests.rs::macos_status_json_matches_diskutil_apfs_capacity`, `tests/integration_tests.rs::macos_check_json_matches_diskutil_apfs_capacity`, `docs/macos.md` | Covered in macOS integration tests and docs. Requires final CI green on the shipped head. |
| Scan finds and ranks macOS reclaim candidates | `src/platform/macos/cleanup_catalog.rs`, `tests/common/mod.rs::SyntheticMacTree`, `src/scanner/patterns.rs` macOS cleanup tests, `docs/macos-incident-case-study.md` | Covered for Xcode, CoreSimulator, Electron caches, `/private/tmp/*-target`, `*_target`, `target_*`, user trash, and sacred paths. |
| Clean/daemon deletion respects protected paths and active builds | `src/daemon/loop_main.rs::should_skip_protected_daemon_candidate`, `src/scanner/walker.rs`, `src/scanner/deletion.rs`, `bd-twgw`, `bd-j40b`, `daemon::loop_main::tests::scanner_prescan_does_not_dispatch_protected_rust_fuzz_target`, `daemon::loop_main::tests::executor_preflight_skips_config_protected_daemon_candidate` | Fixed in current source. Installed sbh 0.4.6 daemons must be upgraded/restarted because they can still delete protected artifact-looking paths. |
| Blame attributes macOS disk growth to processes | `tests/integration_tests.rs::macos_synthetic_writer_surfaces_in_blame_top_rows`, `src/cli_app.rs::collect_blame_report_at`, macOS PAL libproc process I/O and open-file code | Covered by macOS integration test and PAL-backed implementation. |
| CI validates Linux and macOS | `.github/workflows/ci.yml` jobs `check`, `unit`, `integration`, `linux-arm64`, `decision-plane`, `dashboard`, `e2e`, `macos-platform`, `macos-coverage`, `macos-benchmarks`, `stress`, `artifact-contract`, `provenance`, and `Homebrew Formula Validation` | Infrastructure exists. Final goal cannot close until the final head completes green. `macos-13` has been replaced with `macos-15-intel` because GitHub retired the old runner label; `macos-latest` remains the arm64 lane. |
| Docs explain install, configure, verify, and diagnose | `README.md`, `docs/macos.md`, `docs/macos-full-disk-access.md`, `docs/cleanup-rules-macos.md`, `docs/testing-and-logging.md`, sample configs in `docs/configs/` | Covered in docs. Keep docs update lint green for future CLI/config changes. |
| Release is signed, notarized, Gatekeeper-assessed, and distributed through Homebrew | `.github/workflows/release.yml`, `.github/workflows/cert-expiration.yml`, `.github/macos/sbh.entitlements.plist`, `packaging/homebrew/Formula/sbh.rb`, `docs/macos.md` release diagnostics, `src/cli/mod.rs::release_workflow_notarizes_macos_binaries_asynchronously` | Workflow and docs exist. `bd-ykwh.20` added release-side `spctl -a -t execute -vv` before packaging. Live credentials and a successful tag release are still missing, so this is not complete. |

## Protected-Path Daemon Regression

Live incident evidence from another machine showed sbh 0.4.6 deleting
`/data/projects/asupersync_ansi_c/tools/rust_fuzz_target` even though
`sbh scan` honored both config protections and `.sbh-protect` markers. The root
cause was the daemon cleanup path, especially priority pre-scan and executor
dispatch, bypassing the same protection checks used by manual scans.

Current source status:

- `bd-twgw` hardened daemon cleanup candidates with protection checks before
  dispatch and executor preflight.
- `bd-j40b` added the exact incident regression for the protected
  `asupersync_ansi_c/tools/rust_fuzz_target` path shape.
- The focused proof lane is
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_daemon_protection cargo test --lib protected -- --nocapture`.
- On 2026-05-09 this lane passed 12 protection-related tests, including
  `scanner_prescan_does_not_dispatch_protected_rust_fuzz_target` and
  `executor_preflight_skips_config_protected_daemon_candidate`.

Operational consequence: do not restore protected files on machines still
running sbh 0.4.6 and assume they are safe. Upgrade/restart the daemon to a build
containing `bd-twgw` and `bd-j40b`, then restore the protected worktree files.

## Live Release Blocker Evidence

The user confirmed Apple Developer Program enrollment, so enrollment itself is
not the current blocker. Live checks at 2026-05-09 10:13 UTC still showed:

- `security find-identity -v -p codesigning`: `0 valid identities found`
- `xcrun notarytool history --keychain-profile sbh-notary --output-format json`:
  missing `sbh-notary` keychain profile
- `gh secret list --repo Dicklesworthstone/storage_ballast_helper --json name,updatedAt`:
  `[]`

Remaining release blockers:

- Create/import a `Developer ID Application` certificate and private key.
- Configure the local `sbh-notary` notary profile.
- Configure GitHub Actions secrets for release signing and notarization.
- Configure `HOMEBREW_TAP_TOKEN` for the Homebrew formula PR workflow.
- Run `sbh doctor --release --json` from the current build and require all
  release diagnostics to pass.

## Not Complete

Do not close `bd-r7m7`, mark the active parity goal complete, or call the macOS
release done until all of these are true:

1. `sbh doctor --release --json` passes from the current source build.
2. A `Developer ID Application` identity is present and release secrets are
   configured in GitHub Actions.
3. The notary profile `sbh-notary` authenticates successfully.
4. `HOMEBREW_TAP_TOKEN` is configured and the formula PR path is verified.
5. The release workflow succeeds on a tag and produces signed/notarized macOS
   artifacts.
6. The final pushed head completes CI green, including `macos-platform`,
   `macos-coverage`, `macos-benchmarks`, Linux lanes, and
   `Homebrew Formula Validation`.
