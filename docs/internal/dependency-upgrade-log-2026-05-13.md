# Dependency Upgrade Log

Date: 2026-05-12 to 2026-05-13

Project: `storage_ballast_helper`

## Scope

- Preserve the project policy that SBH builds with nightly Rust.
- Apply dependency updates through the library-updater workflow.
- Verify each direct dependency update before moving to the next one.
- Finish with a real local macOS release build, isolated install, daemon run, and log/state inspection.

## Baseline

- Toolchain: `nightly` in `rust-toolchain.toml`.
- Primary features used for verification: `cli,daemon,sqlite`.
- Direct dependency inventory was taken with:
  - `cargo tree --depth 1 --no-default-features --features cli,daemon,sqlite`
  - `cargo tree --target aarch64-apple-darwin --depth 1 --no-default-features --features cli,daemon,sqlite`
  - `cargo outdated --root-deps-only --format json`

## Outdated Direct Dependencies Identified

| Crate | Current | Candidate | Kind | Notes |
| --- | --- | --- | --- | --- |
| `bincode` | `2.0.1` | `3.0.0` | normal | Major upgrade; `serde` feature is obsolete in 3.x. |
| `chrono` | `0.4.43` | `0.4.44` | normal | Compatible patch/minor line. |
| `clap` | `4.5.58` | `4.6.1` | normal | Compatible 4.x line. |
| `clap_complete` | `4.5.66` | `4.6.5` | normal | Compatible 4.x line. |
| `crossterm` | `0.28.1` | `0.29.0` | normal | Minor upgrade; TUI/dashboard surface. |
| `filetime` | `0.2.27` | `0.2.29` | dev | Compatible 0.2 line. |
| `libc` | `0.2.180` | `0.2.186` | normal, unix | Compatible 0.2 line. |
| `nix` | `0.29.0` | `0.31.3` | normal, unix | Minor upgrade; platform/deletion surface. |
| `proptest` | `1.10.0` | `1.11.0` | dev | Compatible 1.x line. |
| `rand` | `0.9.2` | `0.10.1` | normal | Major upgrade candidate; check API. |
| `rusqlite` | `0.33.0` | `0.39.0` | optional normal | Minor upgrade; SQLite logger/stats surface. |
| `rustix` | `1.1.3` | `1.1.4` | normal, unix | Exact-pinned patch update candidate. |
| `sha2` | `0.10.9` | `0.11.0` | normal | Minor upgrade; checksum surface. |
| `signal-hook` | `0.3.18` | `0.4.4` | optional normal | Minor upgrade; daemon signal handling. |
| `sysctl` | `0.6.0` | `0.7.1` | macOS normal | Minor upgrade; macOS platform surface. |
| `tempfile` | `3.25.0` | `3.27.0` | dev | Compatible 3.x line. |
| `toml` | `0.8.23` | `1.1.2` | normal | Major API/semantic-version line jump; config parsing surface. |

## Upgrade Entries

### `chrono` 0.4.43 -> 0.4.44

- Action: `cargo update -p chrono --precise 0.4.44`
- Verification: `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-upgrade-check-chrono CARGO_BUILD_JOBS=1 cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `clap` 4.5.58 -> 4.6.1

- Action: `cargo update -p clap --precise 4.6.1`
- Transitive updates observed: `anstream` 0.6.21 -> 1.0.0, `anstyle-parse` 0.2.7 -> 1.0.0, `clap_builder` 4.5.58 -> 4.6.0, `clap_derive` 4.5.55 -> 4.6.1, `quote` 1.0.44 -> 1.0.45, `syn` 2.0.115 -> 2.0.117.
- Verification: `rch exec "env CARGO_TARGET_DIR=/tmp/sbh-upgrade-check-clap CARGO_BUILD_JOBS=1 cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `clap_complete` 4.5.66 -> 4.6.5

- Action: `cargo update -p clap_complete --precise 4.6.5`
- Verification: `rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Compile/check passed on remote worker `vmi1293453`; after the successful compiler exit, `rch` hung while retrieving a one-file target artifact, so the retrieval process was terminated.

### `filetime` 0.2.27 -> 0.2.29

- Action: `cargo update -p filetime --precise 0.2.29`
- Transitive removals observed: `libredox` 0.1.12, `redox_syscall` 0.7.1.
- Verification: `rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Compile/check passed on remote worker `vmi1152480`; after the successful compiler exit, `rch` hung while retrieving a one-file target artifact, so the retrieval process was terminated.

### `libc` 0.2.180 -> 0.2.186

- Action attempted: `cargo update -p libc --precise 0.2.186`
- Result: Blocked by dependency resolution before any verification run.
- Constraint: the path dependency chain through `/dp/frankentui/crates/ftui-tty` uses `nix` 0.31.1, whose published dependency resolution requires `libc = "=0.2.180"`.
- Follow-up: the later `rustix` patch update also advanced the external path dependency lock from `nix` 0.31.1 to 0.31.3, which allowed `libc` 0.2.186 to resolve.

### `proptest` 1.10.0 -> 1.11.0

- Action: `cargo update -p proptest --precise 1.11.0`
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `rand` 0.9.2 -> 0.9.4

- Action: `cargo update -p rand --precise 0.9.4`
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.
- Note: the 0.10.x major upgrade remains a separate higher-risk candidate.

### `rustix` 1.1.3 -> 1.1.4

- Action: updated the exact pin in `Cargo.toml` from `=1.1.3` to `=1.1.4`, then ran `cargo update -p rustix@1.1.3 --precise 1.1.4`.
- Transitive updates observed: `libc` 0.2.180 -> 0.2.186, `linux-raw-sys` 0.11.0 -> 0.12.1, `nix` 0.31.1 -> 0.31.3 for the external `ftui-tty` path dependency chain.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `tempfile` 3.25.0 -> 3.27.0

- Action: `cargo update -p tempfile --precise 3.27.0`
- Transitive removals observed: stale WASI/WIT-related packages and `getrandom` 0.4.1 were pruned from the lockfile.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `crossterm` 0.28.1 -> 0.29.0

- Action: updated `Cargo.toml` to `crossterm = "0.29"`, then ran `cargo update -p crossterm --precise 0.29.0`.
- Transitive updates/additions observed: `derive_more`, `document-features`, `convert_case`, `litrs`, `rustc_version`; stale Windows and old `rustix` lock entries were pruned.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `nix` 0.29.0 -> 0.31.3

- Action: updated `Cargo.toml` to `nix = "0.31.3"`, then ran `cargo update -p nix@0.29.0 --precise 0.31.3`.
- Resulting lockfile change: the old `nix` 0.29.0 entry was removed; the graph now uses `nix` 0.31.3.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `rusqlite` 0.33.0 -> 0.39.0

- Action: updated `Cargo.toml` to `rusqlite = "0.39"`, then ran `cargo update -p rusqlite --precise 0.39.0`.
- Transitive updates/additions observed: `libsqlite3-sys` 0.31.0 -> 0.37.0, `hashlink` 0.10.0 -> 0.11.0, `rsqlite-vfs`, `sqlite-wasm-rs`; old hash dependencies were pruned.
- Initial verification: failed because `rusqlite` 0.39 no longer implements `FromSql` for `u64`.
- Fix: changed `src/logger/stats.rs` to read SQLite aggregates as `i64` and convert with a non-negative checked helper for `PatternStat`.
- Verification after fix: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `sha2` 0.10.9 -> 0.11.0

- Action: updated `Cargo.toml` to `sha2 = "0.11"`, then ran `cargo update -p sha2 --precise 0.11.0`.
- Transitive updates/additions observed: `block-buffer` 0.10.4 -> 0.12.0, `crypto-common` 0.1.7 -> 0.2.1, `digest` 0.10.7 -> 0.11.3, `cpufeatures` 0.2.17 -> 0.3.0; `const-oid` and `hybrid-array` were added.
- Initial verification: failed because the new digest array type no longer implements lowercase hex formatting.
- Fix: added `core::hex_lower` and migrated SHA-256 checksum/digest callsites in CLI, TUI, and integration tests.
- Verification after fix: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `signal-hook` 0.3.18 -> 0.4.4

- Research note: `signal-hook` 0.4 only changes `low_level::pipe` ownership handling to `OwnedFd`; SBH uses `flag::register` and `low_level::raise`, whose signatures remain compatible.
- Action: updated SBH's direct dependency to `signal-hook = "0.4"` and ran `cargo update -p signal-hook@0.4.3 --precise 0.4.4`.
- Graph note: `crossterm` 0.29 still depends on `signal-hook` 0.3.x, so the lockfile legitimately contains both `signal-hook` 0.3.18 and 0.4.4.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `sysctl` 0.6.0 -> 0.7.1

- Research note: `sysctl` 0.7.x updates to Rust edition 2024 / `thiserror` 2 and keeps the `Ctl`, `CtlValue`, `CtlType`, and `Sysctl` APIs SBH uses.
- Action: updated `Cargo.toml` to `sysctl = "0.7.1"`, then ran `cargo update -p sysctl --precise 0.7.1`.
- Transitive removals observed: `thiserror` 1.0.69 and `thiserror-impl` 1.0.69.
- Verification: `env CARGO_TARGET_DIR=/tmp/sbh-macos-check-sysctl-<timestamp> CARGO_BUILD_JOBS=2 cargo +nightly check --no-default-features --features cli,daemon,sqlite --all-targets`
- Result: Passed on the local macOS host, covering the macOS-only `sysctl` code path.

### `toml` 0.8.23 -> 1.1.2+spec-1.1.0

- Research note: the 1.1 crate still exports `from_str`, `to_string`, `to_string_pretty`, `Value`, and `toml::map::Map`, which are the core APIs SBH uses for config parsing and editing.
- Action: updated `Cargo.toml` to `toml = "1.1"`, then ran `cargo update -p toml --precise 1.1.2+spec-1.1.0`.
- Transitive updates/additions observed: `serde_spanned` 0.6.9 -> 1.1.1, `toml_datetime` 0.6.11 -> 1.1.1+spec-1.1.0, `winnow` 0.7.14 -> 1.0.2, `toml_parser`, `toml_writer`; old `toml_edit` and `toml_write` were removed.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `rand` 0.9.4 -> 0.10.1

- Research note: `rand` 0.10 renames the core generator trait from `RngCore` to `Rng`; SBH's direct use is limited to ballast byte filling and `random::<T>()` nonces.
- Action: updated `Cargo.toml` to `rand = "0.10.1"`, changed `src/ballast/manager.rs` to import `rand::Rng`, then ran `cargo update -p rand --precise 0.10.1`.
- Follow-up: re-ran `cargo update -p proptest --precise 1.11.0` after the rand update so the dev dependency stayed on its latest previously verified version.
- Verification: `env -u CARGO_TARGET_DIR rch exec "cargo check --no-default-features --features cli,daemon,sqlite --all-targets"`
- Result: Passed on remote worker `vmi1293453`.

### `bincode` 2.0.1 -> 3.0.0

- Research result: skipped intentionally. The `bincode` 3.0.0 crate payload contains only a README explaining the crate is unmaintained plus a `compile_error!` in `src/lib.rs`; it is not a viable dependency update.
- Action: left `bincode = { version = "2.0.1", features = ["serde"] }` in place.
- Verification: `cargo outdated --root-deps-only --format json` now reports only `bincode` as outdated, with the expected warning that its `serde` feature is obsolete in 3.0.0.

### Final quality gates

- `cargo fmt --check`: passed locally.
- `env -u CARGO_TARGET_DIR rch exec "cargo test --lib dry_run_does_not_delete -- --nocapture"`: passed on remote worker `vmi1264463`.
- `env -u CARGO_TARGET_DIR rch exec "cargo test --test integration_tests dry_run_deletes_nothing -- --nocapture"`: passed on remote worker `vmi1264463`.
- `env -u CARGO_TARGET_DIR rch exec "cargo check --all-targets"`: passed on remote worker `vmi1264463`.
- `env -u CARGO_TARGET_DIR rch exec "cargo clippy --all-targets -- -D warnings"`: passed on remote worker `vmi1293453`.

### Real local macOS release verification

- Built a fresh release binary locally with nightly Rust on the Mac:
  `env CARGO_TARGET_DIR=/tmp/sbh-real-macos-20260512204259/target CARGO_BUILD_JOBS=4 cargo +nightly build --release`.
- Installed the binary into the isolated prefix `/tmp/sbh-real-macos-20260512204259/install/bin/sbh`.
- Verified the installed binary:
  - `sbh 0.4.22`
  - `Mach-O 64-bit executable arm64`
  - ad hoc linker signature present.
- Validated the isolated runtime config at `/tmp/sbh-real-macos-20260512204259/runtime/config.toml`.
- Manual scan proof:
  - `candidates_count = 1`
  - candidate path: `/private/tmp/sbh-real-macos-20260512204259/runtime/scan-root/example_project/target/debug`
  - category: `RustTarget`
  - pattern: `structural-rust-target`
  - configured protected path entries: 4.
- Live daemon proof:
  - daemon started under real local disk pressure (`Critical`, about 3.2% free on `/`).
  - JSONL recorded `daemon_start`, Full Disk Access diagnostics, behavior mode transition, memory pressure subscription, and `pressure_change`.
  - daemon scanner honored `.sbh-protect`: `protected candidate skipped: .../protected_project (protected by .sbh-protect ...)`.
  - daemon executor logged the fixed dry-run wording: `dry-run would_delete=2 failed=0 skipped=0 would_free=256B`.
  - SQLite stats reported zero real deletions and zero bytes freed, matching dry-run semantics.
  - The protected artifact and the unprotected candidate artifact both remained on disk after the daemon run.
- Operational finding from the real daemon run: shutdown is sluggish during critical-pressure scan/executor work. The isolated test daemon required `SIGKILL` after `SIGTERM`, so shutdown responsiveness still needs a follow-up fix.

### Residual daemon fixes and final macOS proof

- Fixed the daemon cleanup path that bypassed protection during priority pre-scan and background execution:
  - workers now receive a shared shutdown token and poll channels with short timeouts;
  - shutdown broadcasts cancellation before dropping worker channels and bounds worker joins;
  - the priority pre-scan checks the same protected path registry before dispatching candidates;
  - special-location scans stay inside configured scanner roots instead of broadening an isolated `/tmp/.../scan-root` config to all of `/tmp`;
  - scan budgets are computed once per scan and enforced during priority pre-scan as well as the walker;
  - the daemon health check no longer respawns worker threads after shutdown is already pending.
- Fixed macOS special-location noise discovered during the long local run:
  - auto-discovery now skips non-reclaimable device pseudo-filesystems such as `/dev`/`devfs`;
  - `/dev/shm` remains registered on Linux.
- Fixed shutdown latency under adaptive tick backoff:
  - `sleep_with_memory_pressure_events` now checks the shutdown flag on each wake chunk, so SIGTERM cuts through the 30s/60s throttle sleeps.
- Verification after the residual fixes:
  - `cargo fmt --check`: passed locally.
  - `rch exec "cargo test --lib special_locations -- --nocapture"`: passed, including the new `/dev` skip and `/dev/shm` keep tests.
  - `CARGO_TARGET_DIR=/tmp/sbh-rch-check-final-20260512225454 rch exec "cargo check --all-targets"`: passed on remote worker `vmi1152480`.
  - `CARGO_TARGET_DIR=/tmp/sbh-rch-clippy-final-20260512225454 rch exec "cargo clippy --all-targets -- -D warnings"`: passed on remote worker `vmi1264463`.
  - Local nightly release build: passed and installed to `/tmp/sbh-real-macos-shutdownfix-20260512225224/install/bin/sbh`.
  - Final isolated dry-run daemon run: `/tmp/sbh-real-macos-final-shutdownfix-20260512225455`.
- Final daemon evidence:
  - ran for 80 seconds on the local Mac under real critical disk pressure;
  - accepted two forced `SIGUSR1` scans;
  - logged protected `asupersync_ansi_c/tools/rust_fuzz_target` candidates as skipped;
  - logged dry-run summaries only (`would_delete=3`) and performed no real deletes;
  - emitted no `/dev` special-location errors;
  - emitted no broad `/tmp` scan paths outside the isolated run root;
  - logged `daemon_stop` in JSONL;
  - exited SIGTERM cleanly with `SIGTERM_WAIT_STATUS=0`, `shutdown requested`, and `shutdown complete (uptime=80s)`.

### Extended one-hour macOS soak and residual fixes

- Baseline soak:
  - Built and ran the local nightly release binary from `target/codex-hour/release/sbh`.
  - Run root: `/tmp/sbh-hour-macos-20260513194438`.
  - Runtime: `2026-05-13T19:44:38-04:00` through `2026-05-13T20:44:43-04:00` (`uptime=3605s`).
  - Shutdown: `SIGTERM_WAIT_STATUS=0`; stderr logged `shutdown complete (uptime=3605s)`.
  - Protection remained effective: the `asupersync_ansi_c/tools/rust_fuzz_target` subtree was repeatedly logged as skipped, not deleted.
  - Residual issues seen during the hour: repeated priority pre-scan budget overruns, repeated active-reference visibility warnings, repeated `/sbin/mount timed out after 6s`, and multi-second dry-run executor batches.
- Residual fixes from the soak:
  - Priority pre-scan and walker scoring now skip active-reference probes when the remaining scan deadline cannot cover the platform probe budget, preserving the hard safety veto without overrunning the scan.
  - macOS mount enumeration now backs off `/sbin/mount` for five minutes after timeout/failure and uses the `whichdisk` fallback during that window.
  - macOS/open-file probing groups related candidate roots under one common probe root when safe.
  - Dry-run deletion batches no longer run the expensive global open-file safety scan, because dry-run does not mutate filesystem state.
  - Dry-run deletion reports now distinguish actual deletes/freed bytes from would-delete/would-free counts.
- Focused local and remote verification after the fixes:
  - `cargo fmt --check`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/tmp/sbh-test-active-ref cargo test --lib active_reference_probe_respects_scan_deadline`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/tmp/sbh-test-mount-fallback cargo test --lib macos_mount_timeout_uses_whichdisk_fallback`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/tmp/sbh-test-protected-prescan cargo test --lib scanner_prescan_does_not_dispatch_protected_rust_fuzz_target`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/tmp/sbh-test-common-probe cargo test --lib common_open_file_probe_root`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/tmp/sbh-test-dry-run cargo test --lib dry_run_does_not_delete`: passed.
  - `RUSTUP_TOOLCHAIN=nightly CARGO_TARGET_DIR=/Users/jemanuel/projects/storage_ballast_helper/target/codex-fixed cargo build --release --bin sbh`: passed.
  - `rch exec -- env CARGO_TARGET_DIR=/tmp/sbh-rch-check cargo check --all-targets`: passed.
  - `rch exec -- env CARGO_TARGET_DIR=/tmp/sbh-rch-clippy cargo clippy --all-targets -- -D warnings`: passed.
- Final fixed local macOS run:
  - Binary: `/Users/jemanuel/projects/storage_ballast_helper/target/codex-fixed/release/sbh`.
  - Run root: `/tmp/sbh-fixed3-macos-20260513211533`.
  - Runtime: `2026-05-13T21:15:33-04:00` through `2026-05-13T21:20:34-04:00` (`uptime=300s`).
  - Shutdown: `SIGTERM_WAIT_STATUS=0`; stderr logged `shutdown complete (uptime=300s)`.
  - Dry-run executor batches dropped to millisecond-scale durations: `1.505084ms`, `1.444625ms`, `1.67075ms`, `1.569875ms`, `1.383ms`, `1.601334ms`, `1.621791ms`.
  - No priority pre-scan budget overruns.
  - No active-reference visibility warnings.
  - No `/dev` references.
  - Only one `/sbin/mount timed out after 6s` warning remained at startup before backoff took over.
  - Protected `asupersync_ansi_c/tools/rust_fuzz_target` candidates were logged as skipped on every scan cycle.
