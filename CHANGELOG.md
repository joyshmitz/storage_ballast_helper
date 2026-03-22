# Changelog

All notable changes to `storage_ballast_helper` (`sbh`) are documented here.

Releases with GitHub Release assets are marked with **[release]**. Versions without that marker were tagged in-tree but not published as GitHub Releases. Commit links point to the canonical repository at `https://github.com/Dicklesworthstone/storage_ballast_helper`.

---

## [v0.3.16] — 2026-03-15 **[release pending]**

Tag: [`v0.3.16`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.3.16) | Compare: [`v0.3.15...v0.3.16`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.3.15...v0.3.16)

### CI / Build

- Decouple release builds from quality gate so releases are not blocked by unrelated gate failures ([`44f26a4`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/44f26a40792268ad6e40148bd5d36a90fc7968c9))

---

## [v0.3.15] — 2026-03-12

Tag: [`v0.3.15`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.3.15) | Compare: [`v0.2.8...v0.3.15`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.8...v0.3.15)

This tag covers all development from v0.2.8 through v0.3.15 — a rapid series of production-tuning point releases (v0.3.0 through v0.3.15) that were not individually published as GitHub Releases. The intermediate version bumps are listed in the subsections below.

### Prediction Engine (v0.3.0)

- **Burst detection in EWMA rate estimator**: two-factor burst detection (rate acceleration + magnitude) prevents the predictor from extrapolating transient spikes into false exhaustion forecasts ([`6516579`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/6516579b58ce5496e72a1aea0b390840b56c0b06), [`e00c4e3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e00c4e3ee36cd757c148a316d8dff1399b97425e))
- **Prediction scorecard**: tracks prediction accuracy over time, solving the self-defeating prophecy problem where successful interventions make the predictor look wrong ([`c0dcc23`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/c0dcc239a14e36a6d82f3b20be81fb96037303c1))
- **Burst-aware prediction gating**: predictions during detected bursts are suppressed or confidence-degraded ([`392a250`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/392a250ce0c4b9e2c08b96b45bc4fbf276719b8d))
- Move `burst_min_confidence` to `PredictionConfig` for cleaner configuration hierarchy ([`061e33d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/061e33dba15ca2203a284c153d97160a29a6c82e))
- Make `CalibrationBreach` advisory-only, lower escalation threshold to Yellow ([`7f121e5`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/7f121e50493d5d86342338ef3ee1d4a1a3b76ec9))
- Exclude TUI feature from CI/release builds and enable `workflow_call` ([`61efc64`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/61efc6425268ef5ec66aae9099266403eddb885e))

### Production Stability (v0.3.1)

- **False-alarm suppression**: daemon no longer fires notifications or escalates policy during genuinely idle periods ([`120a5b9`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/120a5b9555a5d49aa14f64586575decb59917fc7))
- Scan timeout tracking and circuit breaker backoff log ordering fix ([`55857bf`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/55857bf9a1c3c1fdd02337c38b1ffc034faec6c1))
- Operational improvements for scan efficiency ([`57e8bf5`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/57e8bf54785dc19359a2993842b16784871b88db))
- Add missing `reason` field in predictive policy and fix boundary condition ([`4f18448`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/4f1844899e3b9015f04cf6b8a7d5fe2309460c0c))
- Regression test for green-pressure fallback recovery ([`194fce3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/194fce30555d56469476388f092f4bee8835362c))

### Calibration Guard Hardening (v0.3.2)

- Suppress calibration breach log spam and guard trigger deadlock ([`161ac4a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/161ac4a102ffd6b595974b87151f13dd795645de))

### Predictive Warning Gates (v0.3.3)

Five incremental fixes to prevent the predictive warning system from triggering false alarms on healthy disks:

- Implied-rate sanity gate + breach log suppression ([`b35eabc`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/b35eabcd135da3a39534c0928e2cd9d4c3418094))
- Hard gate for predictions showing >50% free space ([`9cc27a1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9cc27a1f551732579714bdc4585d333a8a43a44d))
- Persist `recalibration_count` across clean windows ([`e07c6d4`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e07c6d4f98e1553bfcf8e793371d2fdb717fe63c))
- Move hard gate before burst-aware path ([`5c2553d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/5c2553d7836f6b145b9efbfe3cbb1c42bf891e55))
- Gate `check_predictive_warning` on predictive policy result ([`a052990`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a052990a42a1ceb9c0703ff0a5da9f0f46e30be6))

### Burst Detection + Guard (v0.3.4)

- **MAD-based burst detection**: uses Median Absolute Deviation instead of standard deviation for robust outlier identification ([`ded16b5`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ded16b561b86abd8aefb7b157add63d1a488ecb4))
- Burst-aware guard with median-rate cross-check to prevent false guard triggers during legitimate activity spikes

### Decision-Theoretic Guard (v0.3.5)

- **Multi-level PressureLevel enum**: replaces boolean `pressure_is_green` with Green/Yellow/Orange/Red/Critical levels for fine-grained policy scaling ([`7e5dfe2`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/7e5dfe2a2f8dd27fdbc23b63035f622c28be5eca))
- Decision-theoretic guard override breaks policy rejection deadlock where the guard penalty prevents all deletions even under rising pressure ([`647c574`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/647c574e7388595755eb5cc779ee4b56bd9ec869))
- Rate-limit guard observations to prevent high-frequency tick flooding ([`00e2f78`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/00e2f78a33d2a68093ae13854c150e57d3f85209))

### Yellow Pressure Fixes (v0.3.7 / v0.3.8)

- Fix Yellow-pressure rejection deadlock and suppress Green false alarms ([`d969599`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/d96959981dbcf15edad91fcb8bf9f3dc2246aeb4))
- Extend prediction and guard-trigger suppression to Yellow pressure ([`dce72c6`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/dce72c625e812bb5ecf64209dae6cf1b0ec7d304))
- Reduce guard penalty deadlock at Yellow pressure and suppress false alarm notifications ([`a00c77b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a00c77ba66138c6e73c7e0d339bdba8d5a79e86b))
- Tune guard penalty scaling, suppress Green-pressure predictions, reduce log noise ([`97df2d0`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/97df2d020aaa366458f271afaa95f76bfad1c125))

### Calibration + Diagnostics (v0.3.10 — v0.3.12)

- **Directional calibration guard**: only triggers on predictions in the dangerous direction, ignoring benign miscalibrations ([`0e150dd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/0e150ddc5770715ae2989722941f4bcbe0b6a0f2))
- Widen idle noise threshold and bound `rate_danger_ratio` denominator to prevent division-by-near-zero ([`17bc885`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/17bc88598f0606fa0aa61e18541b6c550bc450bc))
- Double `min_observations` to 60 and fix scanner candidate count reporting ([`a188332`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a1883327a823467b512503780698c5822f8d841b))
- Reduce log noise and improve e-process penalty scaling ([`181518d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/181518da26887575f5631be271977d134d8d19c8))

### Daemon + Scanner Hardening (v0.3.13 — v0.3.15)

- Suppress `HOME`-not-set warning under systemd where `$HOME` may be unset ([`33a973a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/33a973a2e6d7b45455f9beb638bf947b870b5175))
- **Scanner never treats git project roots as deletion candidates**: directories containing `.git` are protected regardless of scoring ([`bc15173`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/bc1517363c276879ec65a9399bf7dae7ebbec919))
- Add Claude session cache pattern (`~/.claude/`) and improve deletion diagnostics ([`582f365`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/582f3658a4eff7c0ccc6e41c4a8068296bd5c3dd))
- **Depth-3 artifact scanning**: walker descends up to 3 levels into directories for pattern matching with breakdown logging ([`ea8e5c0`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ea8e5c00d6c7039478a679a5367f3567449cc6d3))
- Optimize git directory detection cache and suppress cross-platform dead-code warnings ([`75b3716`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/75b3716fe7b27f6cb3b20aa205d8dfd07c0c3698))
- **Heartbeat, cancellation, and backpressure in directory walker**: prevents unbounded memory growth during large scans and allows clean daemon shutdown ([`9c3ba84`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9c3ba84508d090dc2388faa379a2058180fba8cc))

---

## [v0.2.8] — 2026-03-01 **[release]**

Tag: [`v0.2.8`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.2.8) | Compare: [`v0.2.1...v0.2.8`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.1...v0.2.8)

Critical production fix release. The daemon was becoming non-functional on most deployed machines due to a cascade of safety mechanisms triggering during green pressure (plenty of free disk space), which paradoxically blocked cleanup when pressure eventually rose.

Note: version numbers v0.2.2 through v0.2.7 were skipped; development proceeded directly from v0.2.1 to v0.2.8.

### Policy Engine

- **Green-pressure suppression**: guard-triggered FallbackSafe entries suppressed when disk pressure is green — miscalibrated predictions are harmless when no deletions would occur ([`474f700`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/474f7009694c7c33dc25b47bed9a820c74174e4b))
- **FallbackSafe deadlock broken**: emergency escalation to Enforce mode with grace period when FallbackSafe has persisted too long under sustained pressure ([`8ddddb0`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/8ddddb07651a40f70b930cbe3c54a85afebeaecf), [`103957e`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/103957eb843c3ad61caa2c8f40103880f53446a3), [`006ef34`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/006ef342f947b35bb6baacc7c94ff750a9ff1727))
- **Anti-thrash cooldown**: rapid mode oscillation (canary/FallbackSafe) dampened with minimum dwell times ([`82f9d9d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/82f9d9d122ec4a00d8b4b4bf566d4949cc946a2d))
- Canary budget exhaustion pauses deletions until next hour instead of locking down the entire engine ([`82f9d9d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/82f9d9d122ec4a00d8b4b4bf566d4949cc946a2d))

### Scanner + Patterns

- Recognize `rch_target_*`, `rch-target-*`, and `target_codex*` build artifact directories from remote compilation and Codex agents ([`bf03a78`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/bf03a782c1ca681ce8cf775989366e5685a5f2f1))
- Add `/data/tmp` and `/var/tmp` to default scan root paths ([`1f706dd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/1f706dd324809a6baca54a7ebe28ffcf2ae41aeb))
- Configurable `scan_time_budget_secs` (default doubled from 60s to 120s) ([`1f706dd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/1f706dd324809a6baca54a7ebe28ffcf2ae41aeb))

### Daemon

- **Zram false-positive fix**: high zram usage with plenty of free RAM is normal compressed-memory behavior, not disk thrashing ([`9b81294`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9b81294b8213f772574eca7fc2ca45a7c5f0e66f))
- Correct swap thrash detection inversion and add prediction jitter confidence tracking ([`7999715`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/7999715a91de716a612b529232af539470615cb2))
- Cap predictive warning severity by confidence level — 1% confidence no longer triggers CRIT ([`3130ce1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/3130ce1cf3d843210a400baf4626ecd20f38ae86))
- Rate-limit scanner saturation messages to once per 60 seconds ([`9b81294`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9b81294b8213f772574eca7fc2ca45a7c5f0e66f))

### CLI

- **`sbh log` subcommand**: read and tail the JSONL event log with `--follow` and `--type` filtering ([`9e46a58`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9e46a586aadf5cca5c2d15e19e0fd0870c9ce616))
- Cross-user daemon detection via systemd/process scan when config paths differ between root daemon and non-root CLI user ([`9e46a58`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9e46a586aadf5cca5c2d15e19e0fd0870c9ce616))

### Platform + Build

- Gate `--systemd`/`--launchd` by platform before ballast provisioning ([`c49ec5d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/c49ec5d81dbca1708ece01ca4f55c534cf4a3f72))
- Require root for system-scope systemd with clear guidance ([`14e4596`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/14e45966217579706c6f01f491c92a434c7fe2b2))
- Auto-detect non-root on macOS and use user-scope launchd ([`3615ed5`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/3615ed5444e435fef2f7b111ef165ed70e17e6e5))
- Use `root:wheel` for chown recommendation on macOS ([`9d37d47`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9d37d47ac5b0e081cf83acd540c03ce9a6d2b076))
- TUI gated behind optional feature flag + walker cancellation token ([`97ea033`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/97ea03311dfd57446039a77875770bea08ece7ff))
- Switch ftui dependency from local paths to crates.io v0.2.1 ([`f41259c`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f41259c86d5c012cb1efc38d9123baab6b6c04f2))

### Licensing

- License updated to MIT with OpenAI/Anthropic Rider ([`658fe36`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/658fe363b81fcde40e3ad8ad4e6799238898aa0c))

---

## [v0.2.1] — 2026-02-17 **[release]**

Tag: [`v0.2.1`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.2.1) | Compare: [`v0.2.0...v0.2.1`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.0...v0.2.1)

### Features

- **Predictive cleanup policy**: per-event throttling in the daemon prevents redundant scans ([`fb601b3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/fb601b369d11aa1014af3fa8ce451388e4bbe13d))
- **TUI rendering overhaul**: enhanced theme, widget styling, and dashboard rendering ([`b5d7794`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/b5d77940bce1f19e0519825b388ad4619859bf3e))
- Agent skill definition (`.claude/skills/sbh`) for AI agent integration ([`ddd5045`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ddd5045c5f4838e0982443f8637db33108a73f35))

### Bug Fixes

- Suppress bogus predictions and fix wrong mount path in state/logs ([`28e0c4e`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/28e0c4eab8ea56c161e321e5072fc41d86300421))
- Resolve clippy lints, compilation errors, and swap-thrash logic bug ([`fde0f2b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/fde0f2bc7120c9118bd55569355938ad84328616))

### Tests

- Merkle index integration and symlink loop reproduction tests ([`a7eefac`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a7eefac4c8c320ddddcc68ee854a4df6b8b1bfe6))

---

## [v0.2.0] — 2026-02-16 **[release]**

Tag: [`v0.2.0`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.2.0) | Compare: [`v0.1.0...v0.2.0`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.1.0...v0.2.0)

Massive release adding the interactive TUI dashboard, extensive hardening from deep code audits, cross-platform fixes, and a full test suite overhaul. 170+ commits.

### TUI Dashboard

- **Full interactive TUI** with 7 screens: Overview cockpit, Explainability, Scan Candidates, Ballast Operations, Diagnostics, Incident Playbook, and Preferences ([`429c1a3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/429c1a3de91a45015602ee997c18f2ee90c1ceee), [`dd8a8c1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/dd8a8c1d57a3529723b89c5cb7f8e8628a5257e1), [`40a219d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/40a219d6876e41d2e6885772a2193caaa1cc7dca), [`f1b7dfc`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f1b7dfc3b38bc3aec561476d7df0c29456ad914d), [`054ed6a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/054ed6ad5fd33faae94f7cb833c759f7b0198a7a))
- TUI always compiled — no feature flag needed ([`25388e8`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/25388e80cc4de24ec24ca43629695ff5bf123aaf))
- Migrated from crossterm to ftui with layout engine, theme system, and rich overview rendering ([`4cc1010`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/4cc1010e1fb7513c71f20bcfc295270bc4665c14))
- Panic-safe terminal guard prevents TUI crashes from corrupting the terminal ([`c0d305a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/c0d305adf6a52358f08418ae38dcd1a2b7c142dd))
- Frame-based rendering pipeline ([`0d0b5d2`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/0d0b5d2c3ee2f147c6ab9f8068d42e9733c4b3f8))
- Guard against zero-width terminal panics ([`b9f118d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/b9f118dbd5f1f7790876f1e90a2e9f37f5f10576))
- Synthesize ballast volumes from daemon state for inventory display ([`daeac2c`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/daeac2c4f795e3378b61fd9506e180f657554b3d))
- Interactive pane navigation and mouse support ([`429c1a3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/429c1a3de91a45015602ee997c18f2ee90c1ceee))
- Schema-shielding layer for dashboard data models ([`373803c`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/373803c933ef097c51343744c8e105b724795502))
- User preferences model for dashboard ([`bf54cf8`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/bf54cf8905f7fdc90e8cc718b221d259125b3716))

### Scanner + Scoring

- **Production 0-deletion bug fixed**: rebalanced Bayesian decision thresholds that caused no deletions across the entire fleet ([`d6bbd81`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/d6bbd814f8c4c47ed66e916b481f34d15d5914b6), [`e5987f9`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e5987f909fb8d4172c33614bcbfb4b16c55bbc0f))
- Queue starvation fix — 15K entries/0.5s vs 17/60s ([`3bb9232`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/3bb9232de1a3cb7cf848589abf54ca0bae32d573))
- Cap per-dir iteration at 2000 entries + deferred child dispatch ([`fd1e197`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/fd1e19799bdad38805d36d4f471394a20949e32c))
- Parallelize `/proc` scan, optimize walker hot path ([`96ed3da`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/96ed3dad9c0d814f53b5d74d422835c509c2db5c))
- Reorder location checks so `.tmp_` and `.target` match before generic `/target` ([`f2b0b7d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f2b0b7d9488b67651bac02b8009cb882d7567c3d))
- Consolidate and simplify builtin artifact patterns ([`5e93e15`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/5e93e158b81abe3eb974b95a6febc62c0d03d069))
- Case-insensitive pattern matching ([`9e789f3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9e789f37f44b086b27bba5651c4a8d9eca487175))
- Defer open-file checks to post-scoring for faster scan startup ([`dd2ccc2`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/dd2ccc2ff9c00e856d25aa9acee712a5c515a445))

### Daemon + Policy Engine

- **Scanner deadlock resolved** that caused 0 scans on all production machines ([`7d87a75`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/7d87a75620eb204de17ce5302209d2bffd0ac120))
- Per-mount release tracking, incremental release logic, and project-root protection ([`ea36631`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ea36631b07a312aff3d4aac5ff614e96893ea1d5))
- Gradual ballast replenishment and cumulative release targets ([`2b1309e`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/2b1309e9d79dd44e7693a81514a5a7c6a10ba91c))
- Repeat-deletion dampening to break agent rebuild loops ([`ffb7fdf`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ffb7fdf2f3a926c17c3b31ea89b491f1346cae64))
- Swap-thrash detection and temp artifact fast-track deletion under pressure ([`301543e`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/301543e07ea7ec56452c58ca3cd1b93b9c47a9b0))
- Memory/swap diagnostics and expanded artifact detection patterns ([`139a70d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/139a70d07a6a9260dd9b4c97364a92a1424edc27))
- PID slow-decay hysteresis steps one level at a time instead of jumping to raw ([`e3b8087`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e3b8087654026cf7b0446b45fb4fe6aad20ca0cc))
- Propagate `poll_interval`, prediction disable, and notification config on SIGHUP reload ([`f6124d8`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f6124d8812a12bcbea30440628a1c99c8dcdc20d))
- Trigger root filesystem scan on special location pressure ([`fd683b3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/fd683b30f4fad79e4be4d5a4642e51f26064e356))
- Production reclaim failure resolved ([`c289bca`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/c289bcafbc55650f55d263ea28157621a06f6247))

### Security + Hardening

- Hyphen injection guard, ancestor-set open-file detection, composite index ([`5e0a2db`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/5e0a2dbd15fef1027b5a0812ca3f3aacb9721f80))
- Security hardening: ballast release rework, walker streaming, idiomatic Rust modernization ([`745d119`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/745d1192874d95946b2c5a30abf1f60f20721cd7))
- Multi-volume ballast, inode-based open-file detection, Cow allocation reduction ([`31b165c`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/31b165cf3628cfdec3764f8b1350a495843ef90a))
- Correct `glob_to_regex` for `**/` pattern boundary matching ([`817028c`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/817028c2bda2b5c7b686f708f266868c97ebd40d))
- VOI config extraction, decision record `effective_action` ([`9e789f3`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/9e789f37f44b086b27bba5651c4a8d9eca487175))

### CLI

- Cap help text width at 100 columns ([`a18b8fd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a18b8fd1f565cf44d816fa16788259d86d1383fb))
- Cosign v2 identity flags + `is_writable` parent dir check ([`4ca87af`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/4ca87afb3019aa57dd5521c9ea9cbc48f6697835))
- Implement actual curl-based asset download and robust build-dir creation ([`14f0afc`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/14f0afc2176b58e19329232d33638a7f90f1e982))
- 6 bugs from deep audit: mount check, zombies, template, deprecated keys, `bytes_freed`, writable ([`0306e6b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/0306e6bb87f73d9ff8bf05221c8b132126052a33))

### Logging

- Circuit-breaker logic improvements and rotation resilience ([`80708cd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/80708cd945eb9fe65adf667b44b1038a91bb7058))
- Failure-injection test suites for self_monitor and JSONL logger ([`11bfb22`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/11bfb22ad843b3bcb3c2e4e892f57168f19d357d))
- Handle JSONC block comments in root-brace parser ([`e39e9d1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e39e9d1ba1f6d39f05e9def65c3967cb420de0d7))

### Tests

Extensive test suite expansion as part of the TUI dashboard rollout:

- 37 snapshot/golden tests for dashboard screens ([`af667b5`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/af667b5d5ca7f77a3b75ef84556903b627a89463))
- 44 fallback/rollback verification tests ([`0b44620`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/0b4462097b50ef82df5e817fca90a5c5fd30a986))
- 31 integration tests for dashboard CLI and state-file contract ([`f4978e8`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f4978e833e2362cd28126321a8ee4fe6dac2d59b))
- 22 unit tests with 10 duplicate test name fixes ([`5343369`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/53433691fe18b72129e34c6b493d34773db03221))
- 8 property tests for scheduler/overlay/history/detail invariants ([`7088195`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/708819583fa0b70dbd02dfd65d4f516795508924))
- Stress/performance test suite ([`81557a6`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/81557a66d7e6f854e9e9f4eaf7b1088579cc11d9))
- Deterministic replay regression suite ([`5491318`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/549131863426935ddf18ddf7dc1065901e2aa43e))
- Parity harness covering all 18 contracts ([`43f24ae`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/43f24ae70dcf8e22779d4826b7c8879d241f09b1))
- 9 comprehensive e2e dashboard test cases ([`a3f093a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a3f093abd7a371a6f55d2b33b7c045f5fc4b57cc))
- Scenario-driven dashboard e2e drills ([`ef0c42d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/ef0c42d4c43c03572038cfc582e9b44abecccd46))

### Platform

- Stats module: push pattern extraction into SQLite custom function for server-side aggregation ([`caed1d1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/caed1d1057e0f41fce74978e856945bb483df1cb))
- Backup dir fallback, UTF-8 path truncation, RateHistory div-by-zero fix ([`748283f`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/748283f46a199d95d55b8a5ebef19b1dfde7abaf))
- CoW filesystem fallocate bypass + VOI budget=1 fix ([`2e00c1a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/2e00c1a8d14942a6d9dcd72f6341a7ed16939e36))

---

## [v0.1.0] — 2026-02-15 **[release]**

Tag: [`v0.1.0`](https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.1.0) | Compare: [`91a5e28...v0.1.0`](https://github.com/Dicklesworthstone/storage_ballast_helper/compare/91a5e28...v0.1.0)

Initial release of Storage Ballast Helper — a cross-platform disk-pressure defense system for AI coding workloads.

### Core Capabilities

- **Continuous disk pressure monitoring** with EWMA forecasting and PID controller
- **Three-pronged defense**: ballast file pools, artifact scanner, special location monitor
- **Multi-factor scoring engine** for safe artifact cleanup with deterministic ranking
- **Decision-plane policy engine** with shadow/canary/enforce modes and evidence ledger
- **Hard safety vetoes**: `.git` directories, protected paths, too-recent files, open files

### CLI Commands

- `sbh check` — inspect pressure and forecast
- `sbh scan` — run cleanup scan and review candidates
- `sbh clean` — execute safe cleanup with confirmation
- `sbh emergency` — zero-write emergency recovery mode
- `sbh ballast provision` — provision per-volume ballast pools
- `sbh protect` / `unprotect` — project protection via `.sbh-protect` markers
- `sbh explain` — show decision evidence and rationale
- `sbh stats` — storage trend statistics
- `sbh blame` — identify top space consumers
- `sbh dashboard` — legacy text-mode dashboard
- `sbh install` — systemd/launchd service integration
- `sbh setup` — bootstrap migration and self-healing
- `sbh tune` — tuning recommendations

### Daemon

- Systemd and launchd service manager integration
- Configurable poll interval and notification thresholds
- Coordinator for scan/cleanup/ballast orchestration
- Predictive cleanup with configurable confidence thresholds
- Self-monitoring with health integration

### Observability

- Dual logging: SQLite + JSONL with full explainability
- Decision records with traceable evidence and rationale
- Structured event log with type-filtered queries

### Safety

- `#![forbid(unsafe_code)]` in both `lib.rs` and `main.rs`
- No async runtime — OS threads with `crossbeam-channel` and `parking_lot`
- Pedantic + nursery Clippy lints enabled project-wide
- Deterministic builds: `opt-level = "z"`, LTO, `codegen-units = 1`, `panic = "abort"`, stripped

### Platform Support

- Linux x86_64 (`sbh-x86_64-unknown-linux-gnu.tar.xz`)
- macOS arm64 (`sbh-aarch64-apple-darwin.tar.xz`)
- macOS `statvfs` type mismatch fix in platform abstraction layer ([`e78e3a1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/e78e3a1))

### Foundational Work (pre-v0.1.0)

The following significant capabilities were built in the 60+ commits before the initial release tag:

- Repository initialization and project scaffold ([`91a5e28`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/91a5e28f315d9869b37add11caeaa9ab27cd64f7))
- Core CLI commands and scanner subsystems ([`61332bc`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/61332bc4ba1a2edd8d3b2149b6c3c713bf091dc0))
- Coordinator, predictive engine, notifications ([`5f0176b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/5f0176b8405cafc86c92b80d6db8ba31c2d171bd))
- Service managers, stats, blame, tune commands ([`62ba3e1`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/62ba3e183d0e8e9617afee1f64d429db424a0ab0))
- Merkle scan index and Windows PowerShell installer ([`3bcd099`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/3bcd099c26d15956206b12ffd032f7fb933d6b48))
- VOI scan scheduler ([`d4da084`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/d4da08410c3bb038537ec3466b93d47d4a317b2d))
- Asset management and from-source build modules ([`0560d66`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/0560d66de886aae15b22f88eedd7a9b890829dc2))
- Uninstall with safe cleanup modes ([`bf36edd`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/bf36eddf073a2d2fc869031796494c50097288fb))
- Decision-plane policy engine and evidence ledger ([`52a0877`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/52a087791b4eb606bfcc132fa80e4eb86c9f24c0))
- Live TUI dashboard with pressure gauges and sparklines ([`c5992fc`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/c5992fc4e56efe0a3bf73a3b09e448b40dc8eb90))
- Sigstore bundle verification and install/update commands ([`7f57b7f`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/7f57b7f0d12f2eab75eee27f2d22099f0c1c3f95))
- Backup/rollback/prune for updater ([`6c81f3a`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/6c81f3a56b04d5b86f68f1aada9b1f61f1fbcd12))
- 8 extreme-pressure stress scenarios ([`f38b96f`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/f38b96ff61560f2e1c84d13e9d1e28e16b1e2a56))
- Decision-plane proof harness with 26 tests ([`a3eaade`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/a3eaadee8d494c3b257ef8562bb1f3b0d582ca05))
- Full-pipeline integration tests ([`972ab33`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/972ab33bbe2a26109bcef01bcc9041466203869e))
- 105 unit tests across 5 installer/CLI modules ([`2c94431`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/2c94431c1697e6acf4d2de6e1b8f9e9df2aa2fa5))
- Deep code audit fixes across scoring, deletion, PID, EWMA, PAL, bootstrap, guardrails ([`822e5ce`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/822e5ce4d01f8e21e44d2e3e5b7d86ebdeefbd0a))
- Security: 0o600 permissions on ballast files, log files, state files, merkle checkpoints ([`b7ebeb4`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/b7ebeb4eb7b2499dd6c2e96e15daf0ac30a6ba5e), [`848211d`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/848211d80e3adf5a7dae8dcdb4ebd26eaab58ac8), [`49a01f8`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/49a01f8badf1e4e118e5af04a7a5994156e1e3ee), [`fe765c9`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/fe765c98a7ecda457eb18e5d5b7f1cc4afb3f316))
- Canonicalize paths in protect/unprotect to prevent symlink traversal ([`b4e9412`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/b4e9412060c70c3a06eab47e92107a8c8f14e80b))
- Guard ballast size calculation against integer overflow ([`5ef86fe`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/5ef86fe03eb424e1dd2af5c944c53fcafabd28cd))
- Parse meminfo unit suffix instead of assuming kB ([`085238b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/085238bff33fde6f16e71a2e0a1e16cb21f48671))
- Decode all octal escape sequences in mount paths ([`0156847`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/015684784e1c94ce7f44f9dc2dbde6e8bb6dc0ad))
- SQLite recovery mechanism and new activity event types ([`af00f9b`](https://github.com/Dicklesworthstone/storage_ballast_helper/commit/af00f9bc2b60ce21fdebf5e44b80b5c0b94a0e46))

---

## Statistics

| Metric | Value |
|--------|-------|
| Total commits | 308 |
| Tagged releases | 6 (v0.1.0, v0.2.0, v0.2.1, v0.2.8, v0.3.15, v0.3.16) |
| GitHub Releases | 4 (v0.1.0, v0.2.0, v0.2.1, v0.2.8) |
| Development period | 2026-02-14 to 2026-03-15 |
| Intermediate point releases (untagged) | v0.3.0 through v0.3.14 (in-tree only) |
| Skipped versions | v0.2.2 through v0.2.7, v0.3.6, v0.3.9 |

[v0.3.16]: https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.3.15...v0.3.16
[v0.3.15]: https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.8...v0.3.15
[v0.2.8]: https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.1...v0.2.8
[v0.2.1]: https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/Dicklesworthstone/storage_ballast_helper/compare/v0.1.0...v0.2.0
[v0.1.0]: https://github.com/Dicklesworthstone/storage_ballast_helper/releases/tag/v0.1.0
