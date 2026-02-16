# FrankentUI Licensing and Toolchain Compliance Plan (bd-xzt.1.6)

This document establishes the licensing, toolchain, and dependency compliance
framework for reusing FrankentUI code in the SBH TUI overhaul (bd-xzt).

Referenced by: bd-xzt.1.3 (integration ADR), all implementation PRs in
bd-xzt.2\*/3\*/4\*/5\*.

---

## 1. License Compliance

### Summary

| Property | FrankentUI | SBH |
| --- | --- | --- |
| License | MIT | MIT |
| Copyright holder | Jeffrey Emanuel | Jeffrey Emanuel |
| License file | `LICENSE` (root) | `LICENSE` (root) |
| Per-file headers | None | None |

Both projects share the same author and the same license. MIT permits
unrestricted copying, modification, and redistribution with attribution.

### Attribution Requirements

**For copied code (verbatim or near-verbatim):**

1. Preserve the FrankentUI MIT copyright notice in `LICENSE` or a dedicated
   `THIRD_PARTY_NOTICES` section if SBH ever adopts one.
2. Add a file-level comment at the top of any file containing substantial
   copied code:
   ```rust
   // Adapted from FrankentUI (MIT, Copyright 2026 Jeffrey Emanuel)
   // https://github.com/Dicklesworthstone/frankentui
   ```
3. Since both projects share the same author and license, a single root-level
   acknowledgment in `LICENSE` or `README.md` is sufficient. Per-file headers
   are optional but recommended for traceability.

**For adapted code (substantially rewritten):**

1. No formal attribution required under MIT, but a comment noting the origin
   is good practice for future maintainers.
2. Format: `// Inspired by FrankentUI <module>` is sufficient.

**For design/architecture inspiration (no code copied):**

1. No attribution required. Document the lineage in the ADR (bd-xzt.1.3) for
   architectural traceability.

### Third-Party Dependency Licenses

All FrankentUI direct dependencies use permissive licenses (MIT, Apache-2.0,
or MIT/Apache-2.0 dual). No GPL, AGPL, LGPL, or copyleft dependencies were
found in the core crates targeted for reuse.

**Verification checklist for each import:**

- [ ] Run `cargo license` (or manual Cargo.toml audit) on any new dependency
      pulled in by adopted FrankentUI code.
- [ ] Reject any dependency with GPL/AGPL/LGPL license unless it is strictly
      dev-only (`[dev-dependencies]`).
- [ ] Document new transitive dependencies in the PR description.

---

## 2. Toolchain Posture: Stable-First Strategy

### Current State

| Property | FrankentUI | SBH |
| --- | --- | --- |
| Edition | 2024 | 2024 |
| Toolchain | nightly | **stable** |
| `#![feature(...)]` | None found in core crates | None (forbidden by policy) |

### Key Finding

FrankentUI's `rust-toolchain.toml` pins `channel = "nightly"`, but the core
crates (ftui-core, ftui-render, ftui-style, ftui-text, ftui-layout) do **not**
use any `#![feature(...)]` directives. Edition 2024 was stabilized in Rust
1.85.0 (February 2025). This means the core crates likely compile on stable
Rust without modification.

### Policy

**SBH MUST remain on stable Rust.** This is non-negotiable.

1. **No nightly-only features may be introduced into SBH**, even behind feature
   gates, unless explicitly approved with a documented exception and a
   stable-fallback path.

2. **Copied FrankentUI code must compile on stable.** Before merging any
   FrankentUI-derived code:
   - [ ] Verify compilation with `rch exec "cargo check --all-targets"` on
         the stable toolchain.
   - [ ] Remove or gate any nightly-only constructs.

3. **Feature gates for conditional nightly support are NOT permitted** at this
   time. The added complexity of maintaining dual-toolchain code paths is not
   justified. If a future need arises, it must be approved through a new ADR.

4. **Edition 2024 compatibility:** Both projects use Edition 2024, which is
   available on stable since Rust 1.85.0. No edition-related barriers exist.

### Dependency Toolchain Constraints

Adopted dependencies must satisfy:

- [ ] Published on crates.io (no git-only dependencies in release builds).
- [ ] Compatible with stable Rust (check MSRV or CI badges).
- [ ] No `build.rs` scripts that require nightly compiler internals.

---

## 3. Dependency Impact Assessment

### Candidates for Reuse (Safe)

These FrankentUI crates have minimal dependency footprints and `forbid(unsafe_code)`:

| Crate | Direct Deps | Unsafe | Stable-Compatible |
| --- | --- | --- | --- |
| ftui-core | unicode-\*, arc-swap, bitflags, ahash, web-time | `forbid(unsafe_code)` | Likely yes (no `#![feature]`) |
| ftui-render | ahash, bitflags, memchr, smallvec, bumpalo | `forbid(unsafe_code)` | Likely yes |
| ftui-style | tracing (optional) | No unsafe found | Likely yes |
| ftui-text | ropey, lru, rustc-hash | `forbid(unsafe_code)` | Likely yes |
| ftui-layout | rustc-hash, serde | `forbid(unsafe_code)` | Likely yes |

### Candidates Requiring Caution

| Crate | Concern | Mitigation |
| --- | --- | --- |
| ftui-runtime | Elm-style architecture may conflict with SBH's existing event loop | Adapt patterns, don't import wholesale |
| ftui-widgets | Opinionated widget library | Cherry-pick individual widgets |
| ftui-tty | Unix-only, adds nix + rustix deps | SBH already depends on nix; rustix is additive |

### Explicitly Excluded

| Crate | Reason |
| --- | --- |
| ftui-extras | 30+ feature-gated modules, pulls wgpu/tokio/axum; massive dep tree |
| ftui-web / ftui-showcase-wasm | WASM targets not relevant to SBH |
| ftui-pty | PTY utilities not needed |
| ftui-i18n | Internationalization not in SBH scope |
| frankenterm-core / frankenterm-web | Terminal emulator core; out of scope |

### SBH Existing Dependencies That Overlap

These crates are already in SBH and would not add new transitive dependencies:

| Crate | SBH Version | FrankentUI Version |
| --- | --- | --- |
| serde | 1.0 | 1.0 |
| memchr | 2.7 | 2.7 |
| crossterm | 0.28 (optional) | 0.29 (optional, legacy) |
| signal-hook | 0.3 | 0.4 |
| nix | 0.29 | 0.31 |
| bitflags | (transitive) | 2.10 |

**Note:** Version mismatches (crossterm 0.28 vs 0.29, nix 0.29 vs 0.31) must
be resolved by upgrading SBH's pinned versions or vendoring compatible
subsets. Cargo handles minor version differences via semver, but major version
mismatches (signal-hook 0.3 vs 0.4) may require explicit resolution.

---

## 4. Integration Approach Options

This section provides context for the integration ADR (bd-xzt.1.3). The
compliance plan does not select a strategy; it defines constraints.

### Option A: Selective Code Adaptation (Recommended for Compliance)

- Copy specific algorithms/patterns from FrankentUI core crates.
- Rewrite to match SBH conventions (error handling, config integration, etc.).
- Minimal new dependencies.
- Full control over stable compatibility.

### Option B: Crate Dependency (ftui-core, ftui-render as deps)

- Add FrankentUI crates as Cargo dependencies.
- Risk: FrankentUI may update its toolchain requirements or break API.
- Risk: Increases transitive dependency count.
- Mitigation: Pin exact versions; vendor if needed.

### Option C: Vendored Subset

- Copy FrankentUI crate source into SBH's `src/` or a `vendor/` directory.
- Modify freely without upstream concerns.
- Risk: Maintenance burden for diverged code.

**Compliance constraint:** All three options are MIT-legal. Options B and C
require stable-compilation verification. Option A has the lowest compliance
risk.

---

## 5. Import Review Checklist

Use this checklist for every PR that introduces FrankentUI-derived code into
SBH. This prevents silent policy drift.

### Before Writing Code

- [ ] Identify the source FrankentUI crate and file(s).
- [ ] Verify the source code is MIT-licensed (check crate Cargo.toml).
- [ ] Check for `#![feature(...)]` in the source; reject if nightly-only.
- [ ] List all new transitive dependencies the import would add.
- [ ] Verify all new dependencies are permissively licensed (MIT/Apache-2.0).

### During Implementation

- [ ] Add attribution comment at the top of files with substantial copied code.
- [ ] Remove or replace any nightly-only syntax/APIs.
- [ ] Ensure `#![forbid(unsafe_code)]` is not violated.
- [ ] Match SBH code conventions (error types, config integration, logging).
- [ ] Gate new optional dependencies behind SBH feature flags if appropriate.

### Before Merging

- [ ] `rch exec "cargo check --all-targets"` passes on stable.
- [ ] `rch exec "cargo clippy --all-targets -- -D warnings"` passes.
- [ ] `cargo fmt --check` passes.
- [ ] `rch exec "cargo test --lib"` passes (no regressions).
- [ ] PR description lists: source file(s), new dependencies, license(s),
      affected contract IDs from the baseline checklist (bd-xzt.1.1).

### Periodic Audit

- [ ] Quarterly: review all FrankentUI-derived code for upstream security
      advisories (via `cargo audit` or GitHub Dependabot).
- [ ] On FrankentUI major release: evaluate whether SBH's vendored/adapted
      code should be updated.

---

## 6. Risk Summary

| Risk | Severity | Mitigation |
| --- | --- | --- |
| Nightly lock-in via imported code | High | Stable-only policy; CI gate; pre-merge check |
| License contamination from transitive deps | Medium | Per-import license audit checklist |
| Dependency bloat from FrankentUI extras | Medium | Explicit exclusion list; minimal crate selection |
| Version drift between SBH and FrankentUI deps | Low | Pin versions; test on upgrade |
| Loss of attribution for copied code | Low | File-level comments; import checklist |

---

## 7. Decision Record

| Decision | Rationale |
| --- | --- |
| SBH remains on stable Rust | Production reliability; no nightly breakage risk |
| MIT attribution via file comments | Sufficient under MIT; lightweight compliance |
| No feature-gated nightly paths | Complexity not justified at this stage |
| Core crates (ftui-core/render/style/text/layout) are safe candidates | `forbid(unsafe_code)`, minimal deps, no `#![feature]` |
| ftui-extras and WASM crates are excluded | Dependency bloat; out of scope |
| Per-PR import checklist is mandatory | Prevents silent policy drift |

---

*This document is the source of truth for FrankentUI compliance. All
implementation PRs in the bd-xzt epic must reference this plan.*
