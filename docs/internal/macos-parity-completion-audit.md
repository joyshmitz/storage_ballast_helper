# macOS/Linux Parity Prompt-To-Artifact Completion Audit

Bead: `bd-r7m7.11`
Refresh beads: `bd-r7m7.12`, `bd-r7m7.13`, `bd-r7m7.15`, `bd-r7m7.16`
Parent: `bd-r7m7`
Last audited: 2026-05-10 02:44 UTC
Evidence snapshot: the audit records the live head and run state observed at
refresh time, but every audit-only commit makes those literals stale. Before any
close decision, refresh the live head and newest run with:

The audit avoids pinning exact commit hashes or GitHub Actions run ids as
durable completion proof; any literal below is a point-in-time observation only.

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

## Prompt-To-Artifact Checklist

| Prompt requirement | Concrete artifacts inspected | Current audit result |
|---|---|---|
| "support mac os in addition to linux" | `src/platform/pal.rs`, `src/platform/linux`, `src/platform/macos`, Linux and macOS CI workflow lanes, macOS integration tests, existing Linux unit/integration lanes | Repo-side platform implementation exists for both OS families. Final proof still requires the final pushed head to complete all Linux and macOS CI jobs green. |
| "everything automatically detected during installation" | `src/cli/install.rs`, `src/daemon/service.rs`, launchd/systemd workflow tests, `docs/macos.md`, Homebrew formula and release workflow | Installer/service detection is implemented and documented. Signed/notarized release install remains blocked by Developer ID, notary, Homebrew token, and final tag-release proof. |
| "while running" automatic platform behavior | PAL-backed status/check/scan/clean/blame/daemon paths, APFS/Mach/libproc macOS implementations, Linux PAL preservation, focused protection regression tests | Runtime behavior is routed through platform-specific implementations behind the shared CLI/PAL surface. Final proof still depends on queued hosted CI and live release diagnostics. |
| "always does the right thing" / "just works" | Protected-path daemon tests, active-reference/open-file checks, sacred-path catalog, APFS accounting tests, launchd lifecycle test, docs and doctor diagnostics | Safety and diagnostics are covered in source and tests. Installed sbh 0.4.6 daemons must be upgraded/restarted because they predate the daemon protection fix. |
| "additional testing infrastructure" | `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `.github/workflows/cert-expiration.yml`, macOS platform/coverage/benchmark jobs, Homebrew validation, release-doctor tests, protected-path tests | Infrastructure exists and focused local/rch proof passed, but the current hosted run is still queued and cannot be treated as final green proof. |

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
- Live recheck at 2026-05-10 02:44 UTC showed the current pushed head
  `0da51406462098b02aa58ee150a0ae632433981f`
  (`bd-r7m7 refresh macos parity audit`). The newest CI run for that head,
  `25617688046`, had completed `macOS Platform Tests (intel)` successfully on
  `macos-15-intel`; `Format + Lint`, `macOS Platform Tests (apple-silicon)`,
  `Homebrew Formula Validation`, `macOS Performance Budgets`, and
  `macOS Coverage` remained queued. Do not treat queued CI as final proof, and
  do not treat partial CI as final proof; inspect the latest run for the final
  pushed head before closing.
- The completed Intel macOS lane for run `25617688046` covered unit tests,
  54 integration tests, release build, ad-hoc hardened-runtime signing,
  temporary-tap Homebrew install/test, release-doctor JSON capture,
  current-binary diagnostic artifact upload, E2E smoke checks, and the
  unsupported-PAL log guard.
- Downloaded Intel diagnostic artifact proof for run `25617688046` passed:
  the uploaded SHA-256 matched, `file` reported a Mach-O x86_64 executable,
  `codesign --verify --strict --verbose=2` passed, `sbh-intel version
  --verbose` reported version `0.4.7`, and `sbh-intel --json install --auto
  --dry-run` emitted one JSON object with nested `install`, `release_install`,
  and `wizard` sections.
- The current CI runner labels were cross-checked against GitHub's hosted runner
  reference at refresh time: `macos-latest` is an arm64/M1 macOS runner and
  `macos-15-intel` is an Intel macOS runner, so the macOS matrix still covers
  both architectures with current labels.
- `br ready --json` returned `[]` at the same refresh. The open release beads
  remained blocked by live Apple/GitHub credentials or signed-release proof
  rather than by an unclaimed repo-side implementation task.
- Local Homebrew formula checks passed at refresh time: `brew style
  packaging/homebrew/Formula/sbh.rb` and the generated-formula placeholder
  replacement path both reported no style offenses.
- Additional local formula validation passed with
  `ruby -c packaging/homebrew/Formula/sbh.rb`. The installed Homebrew version
  rejects path-based `brew audit --formula --strict packaging/homebrew/Formula/sbh.rb`
  and requires auditing by formula name, so full `brew audit` remains covered by
  the hosted `Homebrew Formula Validation` job and release/tap verification.
- The macOS platform CI workflow now exercises the Homebrew formula against the
  current ad-hoc-signed release binary: it packages `target/release/sbh` into a
  local `.tar.xz`, rewrites `packaging/homebrew/Formula/sbh.rb` into a temporary
  `SbhCi` formula with a `file://` URL and matching SHA-256, runs `ruby -c`,
  runs `brew install --formula --build-from-source --skip-link`, and runs
  `brew test --force sbh-ci`. Static coverage in `src/cli/mod.rs` asserts those
  CI fragments and uploaded logs remain present.
- Proof for that Homebrew formula-install patch passed before push:
  `cargo fmt --check`, `git diff --check`, Ruby YAML parsing, a local formula
  transform dry-run, `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_homebrew_signed_install_contract cargo test --lib ci_validates_homebrew_formula_generation -- --nocapture`,
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_homebrew_install_check cargo check --all-targets`, and
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_homebrew_install_clippy cargo clippy --all-targets -- -D warnings`.
- Supplemental current-head proof while hosted CI was queued: local
  `cargo fmt --check` passed, and
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-ci-format-lint-0da5140 cargo clippy --no-default-features --features cli,daemon,sqlite --lib --bin sbh -- -D warnings"`
  returned remote exit 0. This does not replace hosted CI green status.
- Local reproduction of the queued Homebrew Formula Validation lane also passed:
  `ruby -c packaging/homebrew/Formula/sbh.rb`, `brew style
  packaging/homebrew/Formula/sbh.rb`, generated-formula checksum placeholder
  replacement in `/tmp/sbh-homebrew-validation.cHgudl`, `ruby -c` on the
  generated formula, and `brew style` on the generated formula all reported
  success. This does not replace the hosted `Homebrew Formula Validation` job or
  the real signed-release tap update.
- Focused protected-path regression proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_daemon_protection_proof cargo test --lib protected -- --nocapture`.
  The run reported 13 passed tests, including the daemon executor preflight and
  protected `rust_fuzz_target` priority pre-scan regression.
- Focused release-doctor proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_release_doctor_proof cargo test --bin sbh release_doctor -- --nocapture`.
  The run reported 3 passed tests covering missing external credentials,
  credential-present success, and stdin-based release-secret setup steps.
- The release doctor JSON now exposes an aggregate `ok` boolean plus `passed`,
  `warnings`, and `failed` counts, and the human report prints the same
  readiness summary. Focused proof for that current-source update passed with
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-release-doctor-summary-bin cargo test --bin sbh release_doctor -- --nocapture"`
  and
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-release-doctor-summary-lib cargo test --lib developer_id -- --nocapture"`.
  Required compiler gates also passed for the update:
  `cargo fmt --check`, `git diff --check`,
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-release-doctor-summary-check cargo check --all-targets"`,
  and
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-release-doctor-summary-clippy cargo clippy --all-targets -- -D warnings"`.
- Focused release workflow contract proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_workflow_contract_proof cargo test --lib workflow -- --nocapture`.
  The run reported 13 passed tests, including Developer ID import, hardened
  runtime signing, async notarization, PR ad-hoc signing, Homebrew tap PR update,
  and CI cancellation behavior.
- Focused Homebrew contract proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_homebrew_contract_proof cargo test --lib homebrew -- --nocapture`.
  The run reported 5 passed tests covering formula skeleton asset names,
  checksum marker replacement, CI formula generation, tap PR updates, and
  Homebrew install-path discovery.
- Focused macOS CLI auto-detection proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_bin_macos_proof cargo test --bin sbh macos -- --nocapture`.
  The run reported 10 passed tests covering launchd auto-selection, macOS
  release install defaults, doctor remediation, process attribution visibility,
  and platform-autodetection help text.
- Focused macOS library runtime proof passed with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_lib_macos_proof cargo test --lib macos -- --nocapture`.
  Remote sync failed on one worker with permission errors, so `rch` fell back to
  local execution on macOS. The run reported 112 passed tests covering APFS
  capacity, purgeable and snapshot parsing, Mach/sysctl memory data, libproc
  process/open-file visibility, macOS PAL behavior, cleanup catalog safety,
  sacred catalog coverage, launchd defaults, and macOS release/update contracts.
- Attempted current-source macOS binary proof with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_current_macos_binary cargo build --bin sbh --target aarch64-apple-darwin`.
  The selected Linux worker failed before compiling `sbh` because the
  `aarch64-apple-darwin` Rust target was not installed there. No local
  current-source macOS binary was produced, so `sbh doctor --release --json`
  remains unproven from a current build.
- The macOS platform CI lane now captures `sbh --json doctor --release` from
  the just-built current-source release binary, validates the diagnostic JSON
  shape, uploads the doctor output with the macOS logs, and preserves an
  ad-hoc-signed diagnostic binary artifact plus SHA-256. This improves future
  closeout evidence but does not replace a final passing release doctor with
  real Developer ID, notary, and Homebrew credentials.
- The artifact-retention patch was validated with
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_macos_ci_artifact_proof cargo test --lib macos -- --nocapture`.
  The run reported 46 passed tests, including the updated CI workflow contract
  and macOS completion-audit guard. Full compiler gates also passed:
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_ci_artifact_check cargo check --all-targets`
  and
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_ci_artifact_clippy cargo clippy --all-targets -- -D warnings`.
- The JSON install dry-run regression found in the current-head macOS diagnostic
  artifact is fixed in source. `sbh --json install --auto --dry-run` now uses a
  single aggregate payload instead of emitting multiple top-level JSON objects,
  and a helper-level regression covers the macOS launchd/release-install shape
  where `release_install`, `wizard`, and `install` must remain nested in one
  object. Focused proof passed with
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-install-json-payload-test2 cargo test --no-default-features --features cli,daemon,sqlite --bin sbh install_auto_dry_run_json_payload_nests_macos_release_report -- --nocapture"`
  and
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-install-json-integration2 cargo test --no-default-features --features cli,daemon,sqlite --test integration_tests install_auto_dry_run_json_is_single_payload -- --nocapture"`.
  Required compiler gates passed with `cargo fmt --check`, `git diff --check`,
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-install-json-payload-check cargo check --all-targets"`,
  and
  `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-install-json-payload-clippy2 cargo clippy --all-targets -- -D warnings"`.
- Static workflow validation passed locally with Ruby YAML parsing for
  `.github/workflows/ci.yml`, `.github/workflows/release.yml`, and
  `.github/workflows/cert-expiration.yml`. The expected CI quality gate,
  macOS platform, coverage, benchmark, Homebrew formula, Developer ID signing,
  notarization, Gatekeeper, and Homebrew tap workflow anchors are present.
- Local packaging-input validation passed at refresh time:
  `plutil -lint .github/macos/sbh.entitlements.plist`,
  `ruby -c packaging/homebrew/Formula/sbh.rb`, and
  `brew style packaging/homebrew/Formula/sbh.rb`.
- A read-only REST scan of the 30 most recently pushed owner repositories found
  at least 1,483 queued workflow runs and 14 in-progress runs across the
  account, including large queues in `asupersync`, `pi_agent_rust`,
  `franken_node`, and `agentic_coding_flywheel_setup`. This makes the current
  `storage_ballast_helper` hosted-runner delay look account-wide rather than an
  isolated workflow syntax or repository-permission issue.
- Live `gh api repos/Dicklesworthstone/storage_ballast_helper/actions/runners`
  returned `total_count = 0`; `storage_ballast_helper` has no self-hosted
  Actions runners registered, so
  the final CI gate currently depends entirely on GitHub-hosted runner capacity.
  Registering a self-hosted runner or canceling queued runs in other repositories
  would be remote state changes and needs explicit operator approval.
- A narrower queue triage found the largest queued-run sources were
  `asupersync` (858 queued), `pi_agent_rust` (239 queued), `franken_node` (116
  queued), `agentic_coding_flywheel_setup` (104 queued), and
  `ultimate_bug_scanner` (35 queued). If the operator wants to free hosted
  Actions capacity, cancellation should start with those repositories rather
  than this repo's current run, because this run already contains useful
  completed Intel macOS proof.

## Checklist

| Requirement | Evidence | Current Status |
|---|---|---|
| Fresh macOS install auto-detects launchd and status works | `src/platform/macos/pal.rs`, `src/daemon/service.rs`, `tests/integration_tests.rs::macos_launchd_user_service_lifecycle_bootstrap_kickstart_bootout`, `.github/workflows/ci.yml` `macos-platform`, `docs/macos.md` | Repo-side implementation and CI coverage exist. Signed release install remains blocked by `bd-ykwh.2`, `bd-ykwh.3`, and `bd-ykwh.13`. |
| Status/check JSON shape and APFS accounting match macOS reality | `tests/integration_tests.rs::macos_status_json_matches_diskutil_apfs_capacity`, `tests/integration_tests.rs::macos_check_json_matches_diskutil_apfs_capacity`, `docs/macos.md` | Covered in macOS integration tests and docs. Requires final CI green on the shipped head. |
| Scan finds and ranks macOS reclaim candidates | `src/platform/macos/cleanup_catalog.rs`, `tests/common/mod.rs::SyntheticMacTree`, `src/scanner/patterns.rs` macOS cleanup tests, `docs/macos-incident-case-study.md` | Covered for Xcode, CoreSimulator, Electron caches, `/private/tmp/*-target`, `*_target`, `target_*`, user trash, and sacred paths. |
| Clean/daemon deletion respects protected paths and active builds | `src/daemon/loop_main.rs::should_skip_protected_daemon_candidate`, `src/scanner/walker.rs`, `src/scanner/deletion.rs`, `bd-twgw`, `bd-j40b`, `daemon::loop_main::tests::scanner_prescan_does_not_dispatch_protected_rust_fuzz_target`, `daemon::loop_main::tests::executor_preflight_skips_config_protected_daemon_candidate` | Fixed in current source. Installed sbh 0.4.6 daemons must be upgraded/restarted because they can still delete protected artifact-looking paths. |
| Blame attributes macOS disk growth to processes | `tests/integration_tests.rs::macos_synthetic_writer_surfaces_in_blame_top_rows`, `src/cli_app.rs::collect_blame_report_at`, macOS PAL libproc process I/O and open-file code | Covered by macOS integration test and PAL-backed implementation. |
| CI validates Linux and macOS | `.github/workflows/ci.yml` jobs `check`, `unit`, `integration`, `linux-arm64`, `decision-plane`, `dashboard`, `e2e`, `macos-platform`, `macos-coverage`, `macos-benchmarks`, `stress`, `artifact-contract`, `provenance`, and `Homebrew Formula Validation` | Infrastructure exists. The macOS platform, coverage, and benchmark jobs are independent from the Ubuntu `check` job so Linux runner queueing cannot hide missing macOS proof. Current-head Intel macOS platform proof on `macos-15-intel` includes 54 integration tests, E2E smoke, APFS JSON status, ad-hoc hardened-runtime codesign, temporary-tap Homebrew install/test, release-doctor JSON capture, and diagnostic binary artifact verification. Final goal cannot close until the final head completes all required jobs green. `macos-13` has been replaced with `macos-15-intel` because GitHub retired the old runner label; `macos-latest` remains the arm64 lane. |
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
  `rch exec -- env CARGO_TARGET_DIR=/tmp/rch_target_sbh_daemon_protection_proof cargo test --lib protected -- --nocapture`.
- On 2026-05-09 this lane passed 13 protection-related tests, including
  `scanner_prescan_does_not_dispatch_protected_rust_fuzz_target` and
  `executor_preflight_skips_config_protected_daemon_candidate`.

Operational consequence: do not restore protected files on machines still
running sbh 0.4.6 and assume they are safe. Upgrade/restart the daemon to a build
containing `bd-twgw` and `bd-j40b`, then restore the protected worktree files.

## Live Release Blocker Evidence

The user confirmed Apple Developer Program enrollment, so enrollment itself is
not the current blocker. Live checks at 2026-05-10 02:44 UTC still showed:

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
