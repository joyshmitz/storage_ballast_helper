//! Artifact pattern registry: regex-based name matching with structural marker verification.

#![allow(missing_docs)]
#![allow(clippy::cast_precision_loss)]

use std::borrow::Cow;
use std::path::Path;

use crate::platform::cleanup_catalog::{self, CleanupConfidence, CleanupRule, ReclaimCommand};

/// High-level artifact category used by the scorer and CLI reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactCategory {
    RustTarget,
    NodeModules,
    PythonCache,
    BuildOutput,
    CacheDir,
    TempDir,
    AgentWorkspace,
    Unknown,
}

/// Structural features collected from a directory tree.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StructuralSignals {
    pub has_incremental: bool,
    pub has_deps: bool,
    pub has_build: bool,
    pub has_fingerprint: bool,
    pub has_git: bool,
    pub has_cargo_toml: bool,
    pub mostly_object_files: bool,
}

impl StructuralSignals {
    /// Returns true if the signals strongly indicate a build artifact directory
    /// (e.g., Rust target with incremental/deps/fingerprint markers, or mostly
    /// object files).
    #[must_use]
    pub fn has_strong_signal(&self) -> bool {
        // Two or more Rust-specific markers = strong signal.
        let rust_markers = u8::from(self.has_incremental)
            + u8::from(self.has_deps)
            + u8::from(self.has_fingerprint);
        rust_markers >= 2 || self.mostly_object_files || (self.has_build && self.has_deps)
    }
}

/// Classification output for one path.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactClassification {
    pub pattern_name: Cow<'static, str>,
    pub category: ArtifactCategory,
    pub name_confidence: f64,
    pub structural_confidence: f64,
    pub combined_confidence: f64,
}

impl ArtifactClassification {
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            pattern_name: Cow::Borrowed("unknown"),
            category: ArtifactCategory::Unknown,
            name_confidence: 0.0,
            structural_confidence: 0.0,
            combined_confidence: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MatchKind {
    Exact(&'static str),
    Prefix(&'static str),
    #[allow(dead_code)] // variant exists for future pattern use
    Suffix(&'static str),
    #[allow(dead_code)] // match arm exists but no patterns use this variant yet
    Contains(&'static str),
}

#[derive(Debug, Clone)]
struct ArtifactPattern {
    name: &'static str,
    kind: MatchKind,
    confidence: f64,
    category: ArtifactCategory,
}

/// User-provided pattern extension.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomPattern {
    pub name: String,
    pub needle: String,
    pub confidence: f64,
    pub category: ArtifactCategory,
}

#[derive(Debug, Clone)]
struct NormalizedCustomPattern {
    pattern: CustomPattern,
    lowercase_needle: String,
}

/// Registry of built-in and custom patterns.
#[derive(Debug, Clone)]
pub struct ArtifactPatternRegistry {
    builtins: Vec<ArtifactPattern>,
    custom: Vec<NormalizedCustomPattern>,
}

impl Default for ArtifactPatternRegistry {
    fn default() -> Self {
        Self {
            builtins: builtin_patterns(),
            custom: Vec::new(),
        }
    }
}

impl ArtifactPatternRegistry {
    #[must_use]
    pub fn with_custom(mut self, custom: Vec<CustomPattern>) -> Self {
        self.custom = custom
            .into_iter()
            .map(|pattern| NormalizedCustomPattern {
                lowercase_needle: pattern.needle.to_lowercase(),
                pattern,
            })
            .collect();
        self
    }

    /// Classify one path name with optional structural evidence.
    #[must_use]
    pub fn classify(&self, path: &Path, signals: StructuralSignals) -> ArtifactClassification {
        self.classify_with_cleanup_rules(path, signals, platform_cleanup_rules())
    }

    /// Classify one path name against an explicit cleanup catalog.
    #[must_use]
    pub fn classify_with_cleanup_rules(
        &self,
        path: &Path,
        signals: StructuralSignals,
        cleanup_rules: &'static [CleanupRule],
    ) -> ArtifactClassification {
        self.classify_with_cleanup_rules_inner(path, signals, cleanup_rules, None)
    }

    /// Classify one path against an explicit cleanup catalog and synthetic home root.
    #[must_use]
    pub fn classify_with_cleanup_rules_and_home(
        &self,
        path: &Path,
        signals: StructuralSignals,
        cleanup_rules: &'static [CleanupRule],
        home: &Path,
    ) -> ArtifactClassification {
        self.classify_with_cleanup_rules_inner(path, signals, cleanup_rules, Some(home))
    }

    fn classify_with_cleanup_rules_inner(
        &self,
        path: &Path,
        signals: StructuralSignals,
        cleanup_rules: &'static [CleanupRule],
        home: Option<&Path>,
    ) -> ArtifactClassification {
        let Some(name_os) = path.file_name() else {
            return ArtifactClassification::unknown();
        };
        let normalized = name_os.to_string_lossy().to_lowercase();

        let catalog_classification = cleanup_catalog_path_classification(path, cleanup_rules, home);
        let mut best = catalog_classification
            .clone()
            .unwrap_or_else(ArtifactClassification::unknown);
        for pattern in &self.builtins {
            if matches_builtin(pattern.kind, &normalized)
                && pattern.confidence > best.name_confidence
            {
                best = ArtifactClassification {
                    pattern_name: Cow::Borrowed(pattern.name),
                    category: pattern.category,
                    name_confidence: pattern.confidence,
                    structural_confidence: 0.0,
                    combined_confidence: pattern.confidence,
                };
            }
        }

        for custom in &self.custom {
            if normalized.contains(&custom.lowercase_needle)
                && custom.pattern.confidence > best.name_confidence
            {
                best = ArtifactClassification {
                    pattern_name: Cow::Owned(custom.pattern.name.clone()),
                    category: custom.pattern.category,
                    name_confidence: custom.pattern.confidence,
                    structural_confidence: 0.0,
                    combined_confidence: custom.pattern.confidence,
                };
            }
        }

        if let Some(classification) = catalog_classification {
            best = classification;
        }

        // Structural rescue path: name is ambiguous but layout screams "Rust target".
        // Graduated confidence based on how many cargo-specific markers are present:
        // only cargo build output directories have .fingerprint + deps + incremental +
        // build together, so 3+ markers is definitive and warrants higher confidence.
        if best.category == ArtifactCategory::Unknown
            && (signals.has_fingerprint || (signals.has_incremental && signals.has_deps))
        {
            let marker_count = u8::from(signals.has_fingerprint)
                + u8::from(signals.has_incremental)
                + u8::from(signals.has_deps)
                + u8::from(signals.has_build);
            let rescue_confidence = if marker_count >= 3 { 0.75 } else { 0.55 };

            best = ArtifactClassification {
                pattern_name: Cow::Borrowed("structural-rust-target"),
                category: ArtifactCategory::RustTarget,
                name_confidence: rescue_confidence,
                structural_confidence: 0.0,
                combined_confidence: rescue_confidence,
            };
        }

        let structural = structural_score(best.category, signals);
        let mut combined = 0.70f64
            .mul_add(best.name_confidence, 0.30 * structural)
            .clamp(0.0, 1.0);

        // Structural safety override: if the structural score is very low (indicating
        // a project root signal like Cargo.toml or .git), we must severely penalize
        // the combined score to prevent deletion, even if the name matches a pattern.
        if structural < 0.1 {
            combined *= 0.1;
        }

        ArtifactClassification {
            structural_confidence: structural,
            combined_confidence: combined,
            ..best
        }
    }
}

#[must_use]
pub fn platform_cleanup_rules() -> &'static [CleanupRule] {
    #[cfg(target_os = "macos")]
    {
        crate::platform::macos::cleanup_catalog::cleanup_rules()
    }
    #[cfg(target_os = "linux")]
    {
        crate::platform::linux::cleanup_catalog::cleanup_rules()
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        &[]
    }
}

fn matches_builtin(kind: MatchKind, normalized: &str) -> bool {
    match kind {
        MatchKind::Exact(token) => normalized == token,
        MatchKind::Prefix(token) => normalized.starts_with(token),
        MatchKind::Suffix(token) => normalized.ends_with(token),
        MatchKind::Contains(token) => normalized.contains(token),
    }
}

fn cleanup_catalog_path_classification(
    path: &Path,
    cleanup_rules: &'static [CleanupRule],
    home: Option<&Path>,
) -> Option<ArtifactClassification> {
    let rule = home.map_or_else(
        || cleanup_catalog::match_path_scanner_rule(path, cleanup_rules),
        |home| cleanup_catalog::match_path_scanner_rule_with_home(path, cleanup_rules, home),
    )?;
    let confidence = cleanup_rule_name_confidence(rule);
    Some(ArtifactClassification {
        pattern_name: Cow::Borrowed(rule.scanner_label()),
        category: cleanup_rule_category(rule),
        name_confidence: confidence,
        structural_confidence: 0.0,
        combined_confidence: confidence,
    })
}

fn cleanup_rule_name_confidence(rule: &CleanupRule) -> f64 {
    match rule.confidence {
        CleanupConfidence::Definite => 0.96,
        CleanupConfidence::Likely => 0.92,
        CleanupConfidence::Unclear => 0.56,
        CleanupConfidence::ReportOnly | CleanupConfidence::Sacred => 0.0,
    }
}

fn cleanup_rule_category(rule: &CleanupRule) -> ArtifactCategory {
    let name = rule.name;
    if name.contains("target") {
        ArtifactCategory::RustTarget
    } else if name.starts_with("electron")
        || name.contains("cache")
        || name.contains("logs")
        || name.contains("ipsw")
    {
        ArtifactCategory::CacheDir
    } else if name.contains("derived-data") || name.contains("buildroot") {
        ArtifactCategory::BuildOutput
    } else if rule.reclaim_command == ReclaimCommand::PromptBeforeRemove {
        ArtifactCategory::TempDir
    } else {
        ArtifactCategory::Unknown
    }
}

fn structural_score(category: ArtifactCategory, signals: StructuralSignals) -> f64 {
    if signals.has_git {
        return 0.0;
    }
    match category {
        ArtifactCategory::RustTarget => {
            if signals.has_fingerprint {
                0.98
            } else if signals.has_incremental && signals.has_deps {
                0.92
            } else if signals.has_build && signals.has_deps {
                0.85
            } else if signals.has_cargo_toml {
                0.05
            } else if signals.mostly_object_files {
                0.90
            } else {
                0.40
            }
        }
        ArtifactCategory::NodeModules => 0.80,
        ArtifactCategory::PythonCache => 0.75,
        ArtifactCategory::BuildOutput | ArtifactCategory::CacheDir | ArtifactCategory::TempDir => {
            if signals.mostly_object_files {
                0.80
            } else {
                0.40
            }
        }
        ArtifactCategory::AgentWorkspace => 0.78,
        ArtifactCategory::Unknown => {
            if signals.has_fingerprint || (signals.has_incremental && signals.has_deps) {
                0.75
            } else {
                0.0
            }
        }
    }
}

#[allow(clippy::too_many_lines)]
fn builtin_patterns() -> Vec<ArtifactPattern> {
    vec![
        ArtifactPattern {
            name: "cargo-target",
            kind: MatchKind::Exact("target"),
            confidence: 0.70,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "dot-target-prefix",
            kind: MatchKind::Prefix(".target"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "underscore-target-prefix",
            kind: MatchKind::Prefix("_target_"),
            confidence: 0.88,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "cargo-target-prefix",
            kind: MatchKind::Prefix("cargo-target-"),
            confidence: 0.94,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "target-prefix",
            kind: MatchKind::Prefix("target-"),
            confidence: 0.82,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "target-underscore-prefix",
            kind: MatchKind::Prefix("target_"),
            confidence: 0.82,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "target-suffix",
            kind: MatchKind::Suffix("-target"),
            confidence: 0.88,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "tmp-target-prefix",
            kind: MatchKind::Prefix(".tmp_target"),
            confidence: 0.90,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "tmp-cargo-home",
            kind: MatchKind::Prefix(".tmp_cargo_home_"),
            confidence: 0.90,
            category: ArtifactCategory::TempDir,
        },
        ArtifactPattern {
            name: "tmp-codex",
            kind: MatchKind::Prefix(".tmp-codex-"),
            confidence: 0.86,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "tmp-pijs",
            kind: MatchKind::Prefix(".tmp-pijs-"),
            confidence: 0.86,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "tmp-ext",
            kind: MatchKind::Prefix(".tmp-ext-"),
            confidence: 0.82,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "pi-agent",
            kind: MatchKind::Prefix("pi_agent_"),
            confidence: 0.85,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "pi-target",
            kind: MatchKind::Prefix("pi_target_"),
            confidence: 0.85,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "pi-opus",
            kind: MatchKind::Prefix("pi_opus_"),
            confidence: 0.84,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "br-build",
            kind: MatchKind::Prefix("br-build"),
            confidence: 0.82,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "frankenterm-prefix",
            kind: MatchKind::Prefix("frankenterm-"),
            confidence: 0.90,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "cargo-home-prefix",
            kind: MatchKind::Prefix("cargo-home-"),
            confidence: 0.88,
            category: ArtifactCategory::TempDir,
        },
        ArtifactPattern {
            name: "dot-cargo-prefix",
            kind: MatchKind::Prefix(".cargo_"),
            confidence: 0.86,
            category: ArtifactCategory::CacheDir,
        },
        ArtifactPattern {
            name: "agent-ft-suffix",
            kind: MatchKind::Suffix("-ft"),
            confidence: 0.65,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "cass-target",
            kind: MatchKind::Prefix("cass-target"),
            // Must stay above `cass-prefix-hyphen` (0.86) below so a name
            // like `cass-target-thatlilac` keeps its RustTarget label and
            // the cargo-marker structural boost (0.92–0.98) instead of
            // collapsing to the flat 0.78 AgentWorkspace score.
            confidence: 0.94,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "node-modules",
            kind: MatchKind::Exact("node_modules"),
            confidence: 0.97,
            category: ArtifactCategory::NodeModules,
        },
        ArtifactPattern {
            name: "next-build",
            kind: MatchKind::Exact(".next"),
            confidence: 0.90,
            category: ArtifactCategory::BuildOutput,
        },
        ArtifactPattern {
            name: "python-pycache",
            kind: MatchKind::Exact("__pycache__"),
            confidence: 0.96,
            category: ArtifactCategory::PythonCache,
        },
        ArtifactPattern {
            name: "python-venv",
            kind: MatchKind::Exact(".venv"),
            confidence: 0.85,
            category: ArtifactCategory::PythonCache,
        },
        ArtifactPattern {
            name: "pytest-cache",
            kind: MatchKind::Exact(".pytest_cache"),
            confidence: 0.84,
            category: ArtifactCategory::PythonCache,
        },
        ArtifactPattern {
            name: "generic-cache-prefix-hyphen",
            kind: MatchKind::Prefix("cache-"),
            confidence: 0.45,
            category: ArtifactCategory::CacheDir,
        },
        ArtifactPattern {
            name: "generic-cache-prefix-underscore",
            kind: MatchKind::Prefix("cache_"),
            confidence: 0.45,
            category: ArtifactCategory::CacheDir,
        },
        ArtifactPattern {
            name: "generic-cache-exact",
            kind: MatchKind::Exact("cache"),
            confidence: 0.45,
            category: ArtifactCategory::CacheDir,
        },
        ArtifactPattern {
            name: "dot-cache",
            kind: MatchKind::Prefix(".cache"),
            confidence: 0.62,
            category: ArtifactCategory::CacheDir,
        },
        ArtifactPattern {
            name: "generic-tmp-prefix-hyphen",
            kind: MatchKind::Prefix("tmp-"),
            confidence: 0.45,
            category: ArtifactCategory::TempDir,
        },
        ArtifactPattern {
            name: "generic-tmp-prefix-underscore",
            kind: MatchKind::Prefix("tmp_"),
            confidence: 0.45,
            category: ArtifactCategory::TempDir,
        },
        ArtifactPattern {
            name: "generic-tmp-exact",
            kind: MatchKind::Exact("tmp"),
            confidence: 0.45,
            category: ArtifactCategory::TempDir,
        },
        ArtifactPattern {
            name: "dot-tmp",
            kind: MatchKind::Prefix(".tmp"),
            confidence: 0.60,
            category: ArtifactCategory::TempDir,
        },
        // rch (remote compilation helper) build artifacts — can be 70+ GB.
        ArtifactPattern {
            name: "rch-target-underscore",
            kind: MatchKind::Prefix("rch_target_"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "rch-target-dot",
            kind: MatchKind::Prefix(".rch_target_"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "rch-target-hyphen",
            kind: MatchKind::Prefix("rch-target-"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "dot-rch-target-hyphen",
            kind: MatchKind::Prefix(".rch-target-"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        // Bare in-tree shared rch target dirs. rch creates a directory
        // literally named `.rch-target/` (no trailing identifier) in the
        // project root as the default `CARGO_TARGET_DIR` when no per-job
        // override is supplied; on a heavily used worker this can grow
        // to 100+ GB. Without these exact matches the bare names only
        // hit the generic suffix rules — `target-suffix` (0.88) for the
        // hyphen variants and `underscore-target-suffix` (0.92) for the
        // underscore variants — and the dir's mtime gets bumped
        // continuously by active builds, so the age veto fires forever
        // and the disk runs out. Explicit, high-confidence patterns
        // both stabilize classification and unlock the in-tree
        // pressure-based age fast-track in `daemon/loop_main.rs`.
        // Confidences are set above BOTH conflicting suffix matchers
        // (0.88 and 0.92) so `classify()` deterministically picks the
        // specific rch pattern over the generic suffix fallback.
        ArtifactPattern {
            name: "rch-target-bare-dot",
            kind: MatchKind::Exact(".rch-target"),
            confidence: 0.95,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "rch-target-bare-dot-underscore",
            kind: MatchKind::Exact(".rch_target"),
            confidence: 0.94,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "rch-target-bare-hyphen",
            kind: MatchKind::Exact("rch-target"),
            confidence: 0.93,
            category: ArtifactCategory::RustTarget,
        },
        ArtifactPattern {
            name: "rch-target-bare-underscore",
            kind: MatchKind::Exact("rch_target"),
            confidence: 0.93,
            category: ArtifactCategory::RustTarget,
        },
        // codex agent build artifacts.
        ArtifactPattern {
            name: "target-codex",
            kind: MatchKind::Prefix("target_codex"),
            confidence: 0.88,
            category: ArtifactCategory::RustTarget,
        },
        // Claude Code session caches — can grow to 100+ GB in /tmp/claude-<uid>/.
        ArtifactPattern {
            name: "claude-session-cache",
            kind: MatchKind::Prefix("claude-"),
            confidence: 0.88,
            category: ArtifactCategory::CacheDir,
        },
        // cass (Cross-Agent Session Search) bench/profile/scratch dirs.
        // Observed in the wild filling /tmp with 100+ GB of `cass_*_target`
        // (cargo target dirs), `cass_*_bench`, `cass_*_profile`, plus many
        // small per-mode `cass_*.txt`/`.log` artifacts. Both underscore
        // (canonical) and hyphen forms.
        ArtifactPattern {
            name: "cass-prefix-underscore",
            kind: MatchKind::Prefix("cass_"),
            confidence: 0.86,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "cass-prefix-hyphen",
            kind: MatchKind::Prefix("cass-"),
            confidence: 0.86,
            category: ArtifactCategory::AgentWorkspace,
        },
        // Underscore-`_target` cargo build dirs — a sibling shape to the
        // existing `-target` suffix pattern. cass_*_target, pi_*_target,
        // and similar agent-prefixed patterns produce these.
        //
        // Confidence intentionally above the cass-/frankentui- workspace
        // prefixes (0.86, 0.90) so a name like `cass_append_baseline_target`
        // — which matches BOTH `cass_` (workspace) and `_target` (RustTarget)
        // — classifies as the more specific RustTarget. Both lead to the
        // same cleanup outcome; the distinction matters for stats and for
        // the structural-score boost RustTarget gets when cargo markers
        // are present.
        ArtifactPattern {
            name: "underscore-target-suffix",
            kind: MatchKind::Suffix("_target"),
            confidence: 0.92,
            category: ArtifactCategory::RustTarget,
        },
        // FrankenTUI (sibling project to FrankenTerm) — codex bead
        // workspace dirs `frankentui-codex-bd-<id>-*`, profile/sweep
        // outputs `frankentui_profile_*`. Observed in the wild at
        // multiple GB per workspace.
        ArtifactPattern {
            name: "frankentui-prefix-hyphen",
            kind: MatchKind::Prefix("frankentui-"),
            confidence: 0.90,
            category: ArtifactCategory::AgentWorkspace,
        },
        ArtifactPattern {
            name: "frankentui-prefix-underscore",
            kind: MatchKind::Prefix("frankentui_"),
            confidence: 0.90,
            category: ArtifactCategory::AgentWorkspace,
        },
    ]
}

/// Extract a recognizable pattern label from a path string.
///
/// Used by stats aggregation to group deleted items by pattern.
/// Returns a simplified pattern string like "target/" or ".target*".
pub fn extract_pattern_label(path: &str) -> String {
    extract_pattern_label_with_cleanup_rules(path, platform_cleanup_rules())
}

#[must_use]
pub fn extract_pattern_label_with_cleanup_rules(
    path: &str,
    cleanup_rules: &'static [CleanupRule],
) -> String {
    extract_pattern_label_with_cleanup_context(path, cleanup_rules, None)
}

#[must_use]
pub fn extract_pattern_label_with_cleanup_rules_and_home(
    path: &str,
    cleanup_rules: &'static [CleanupRule],
    home: &Path,
) -> String {
    extract_pattern_label_with_cleanup_context(path, cleanup_rules, Some(home))
}

fn extract_pattern_label_with_cleanup_context(
    path: &str,
    cleanup_rules: &'static [CleanupRule],
    home: Option<&Path>,
) -> String {
    let p = Path::new(path);
    let cleanup_rule = home.map_or_else(
        || cleanup_catalog::match_path_scanner_rule(p, cleanup_rules),
        |home| cleanup_catalog::match_path_scanner_rule_with_home(p, cleanup_rules, home),
    );
    if let Some(rule) = cleanup_rule {
        return rule.scanner_label().to_string();
    }

    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("unknown");

    // Match known artifact patterns.
    let lower = name.to_ascii_lowercase();
    if lower == "target" || lower.starts_with("target-") {
        return "target/".to_string();
    }
    // rch's bare in-tree targets (`.rch-target`, `.rch_target`,
    // `rch-target`, `rch_target`) are checked here — before the
    // generic `*-target` suffix branch — so they group with their
    // per-job siblings (`rch_target_*`) in stats output instead of
    // being lumped under the generic `*-target` bucket.
    if lower == "rch_target"
        || lower == ".rch_target"
        || lower == "rch-target"
        || lower == ".rch-target"
    {
        return "rch_target_*".to_string();
    }
    if lower.ends_with("-target") {
        return "*-target".to_string();
    }
    if lower.starts_with(".target") || lower.starts_with("_target_") {
        return ".target*".to_string();
    }
    if lower.starts_with("cargo-target") || lower.starts_with("cargo_target") {
        return "cargo-target-*".to_string();
    }
    if lower.starts_with("pi_agent")
        || lower.starts_with("pi_target")
        || lower.starts_with("pi_opus")
    {
        return "pi_*".to_string();
    }
    if lower.starts_with("cass-target") {
        return "cass-target*".to_string();
    }
    if lower.starts_with("br-build") {
        return "br-build*".to_string();
    }
    if lower.starts_with("rch_target_")
        || lower.starts_with(".rch_target_")
        || lower.starts_with("rch-target-")
        || lower.starts_with(".rch-target-")
    {
        return "rch_target_*".to_string();
    }
    if lower.starts_with("target_codex") {
        return "target_codex*".to_string();
    }
    if lower.starts_with("target_") {
        return "target_*".to_string();
    }
    if lower.starts_with("frankenterm-") {
        return "frankenterm-*".to_string();
    }
    if lower.starts_with("cargo-home-") {
        return "cargo-home-*".to_string();
    }
    if lower.starts_with(".cargo_") {
        return ".cargo_*".to_string();
    }
    if lower.ends_with("-ft") {
        return "*-ft".to_string();
    }
    if lower.starts_with(".tmp_target") {
        return ".tmp_target*".to_string();
    }
    if lower.starts_with("claude-") {
        return "claude-*".to_string();
    }
    // `*_target` checked BEFORE `cass_*` so `cass_append_baseline_target`
    // groups under the more-specific RustTarget label, not the generic
    // cass workspace label. Mirrors the confidence ordering in
    // `builtin_patterns()` (underscore-target-suffix at 0.92 above
    // cass-prefix at 0.86).
    if lower.ends_with("_target") {
        return "*_target".to_string();
    }
    if lower.starts_with("cass_") || lower.starts_with("cass-") {
        return "cass_*".to_string();
    }
    if lower.starts_with("frankentui-") || lower.starts_with("frankentui_") {
        return "frankentui-*".to_string();
    }
    if lower == "node_modules" {
        return "node_modules/".to_string();
    }

    // Fallback: use the directory name.
    name.to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        ArtifactCategory, ArtifactClassification, ArtifactPatternRegistry, CustomPattern,
        StructuralSignals, extract_pattern_label, extract_pattern_label_with_cleanup_rules,
    };
    use crate::platform::{linux, macos};
    use std::path::Path;

    fn classify_macos(
        registry: &ArtifactPatternRegistry,
        path: &Path,
        signals: StructuralSignals,
    ) -> ArtifactClassification {
        registry.classify_with_cleanup_rules(path, signals, macos::cleanup_catalog::cleanup_rules())
    }

    fn extract_macos_pattern_label(path: &str) -> String {
        extract_pattern_label_with_cleanup_rules(path, macos::cleanup_catalog::cleanup_rules())
    }

    fn classify_linux(
        registry: &ArtifactPatternRegistry,
        path: &Path,
        signals: StructuralSignals,
    ) -> ArtifactClassification {
        registry.classify_with_cleanup_rules(path, signals, linux::cleanup_catalog::cleanup_rules())
    }

    #[test]
    fn rust_target_with_markers_gets_high_confidence() {
        let registry = ArtifactPatternRegistry::default();
        let classification = registry.classify(
            Path::new(".target_opus_main"),
            StructuralSignals {
                has_incremental: true,
                has_deps: true,
                has_fingerprint: true,
                ..StructuralSignals::default()
            },
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert!(classification.combined_confidence > 0.90);
    }

    #[test]
    fn ambiguous_target_without_markers_stays_lower_confidence() {
        let registry = ArtifactPatternRegistry::default();
        let classification = registry.classify(Path::new("target"), StructuralSignals::default());
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert!(classification.combined_confidence < 0.80);
    }

    #[test]
    fn target_underscore_prefix_classifies_as_rust_target() {
        let registry = ArtifactPatternRegistry::default();
        let classification = registry.classify(
            Path::new("target_rust_fuzz_42"),
            StructuralSignals::default(),
        );

        assert_eq!(classification.pattern_name, "target-underscore-prefix");
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert_eq!(
            extract_pattern_label("/data/projects/target_rust_fuzz_42"),
            "target_*"
        );
    }

    #[test]
    fn node_modules_is_classified_correctly() {
        let registry = ArtifactPatternRegistry::default();
        let classification =
            registry.classify(Path::new("node_modules"), StructuralSignals::default());
        assert_eq!(classification.category, ArtifactCategory::NodeModules);
        assert!(classification.combined_confidence > 0.60);
    }

    #[test]
    fn custom_patterns_are_honored() {
        let registry = ArtifactPatternRegistry::default().with_custom(vec![CustomPattern {
            name: "my-cache".to_string(),
            needle: "mytool-cache".to_string(),
            confidence: 0.88,
            category: ArtifactCategory::CacheDir,
        }]);
        let classification =
            registry.classify(Path::new("mytool-cache-prod"), StructuralSignals::default());
        assert_eq!(classification.pattern_name, "my-cache");
        assert_eq!(classification.category, ArtifactCategory::CacheDir);
        assert!(classification.combined_confidence > 0.60);
    }

    #[test]
    fn xcode_derived_data_project_root_is_build_output() {
        let registry = ArtifactPatternRegistry::default();
        let path = Path::new("/Users/operator/Library/Developer/Xcode/DerivedData/sbh-demo-abc123");
        let classification = classify_macos(
            &registry,
            path,
            StructuralSignals {
                has_build: true,
                mostly_object_files: true,
                ..StructuralSignals::default()
            },
        );

        assert_eq!(classification.pattern_name, "xcode-derived-data");
        assert_eq!(classification.category, ArtifactCategory::BuildOutput);
        assert!(classification.combined_confidence > 0.80);
        assert_eq!(
            extract_macos_pattern_label(path.to_str().unwrap()),
            "xcode-derived-data"
        );
    }

    #[test]
    fn xcode_derived_data_root_itself_is_not_the_cleanup_candidate() {
        let registry = ArtifactPatternRegistry::default();
        let classification = classify_macos(
            &registry,
            Path::new("/Users/operator/Library/Developer/Xcode/DerivedData"),
            StructuralSignals::default(),
        );

        assert_ne!(classification.pattern_name, "xcode-derived-data");
    }

    #[test]
    fn core_simulator_caches_are_classified_but_devices_are_not() {
        let registry = ArtifactPatternRegistry::default();
        let cache_path =
            Path::new("/Users/operator/Library/Developer/CoreSimulator/Caches/device-cache");
        let cache_classification =
            classify_macos(&registry, cache_path, StructuralSignals::default());

        assert_eq!(cache_classification.pattern_name, "core-simulator-caches");
        assert_eq!(cache_classification.category, ArtifactCategory::CacheDir);
        assert!(cache_classification.combined_confidence > 0.75);
        assert_eq!(
            extract_macos_pattern_label(cache_path.to_str().unwrap()),
            "core-simulator-caches"
        );

        let device_path = Path::new(
            "/Users/operator/Library/Developer/CoreSimulator/Devices/ABCDEF/data/Library/Caches",
        );
        let device_classification =
            classify_macos(&registry, device_path, StructuralSignals::default());

        assert_ne!(device_classification.pattern_name, "core-simulator-caches");
        assert_ne!(
            extract_macos_pattern_label(device_path.to_str().unwrap()),
            "core-simulator-caches"
        );
    }

    #[test]
    fn electron_application_support_cache_shapes_are_classified() {
        let registry = ArtifactPatternRegistry::default();
        let cases = [
            (
                "/Users/operator/Library/Application Support/Claude/Cache",
                "electron-cache",
            ),
            (
                "/Users/operator/Library/Application Support/Slack/Service Worker/CacheStorage/session",
                "electron-service-worker-cache",
            ),
            (
                "/Users/operator/Library/Application Support/Code/Code Cache/js",
                "electron-code-cache",
            ),
            (
                "/Users/operator/Library/Application Support/Discord/GPUCache",
                "electron-gpu-cache",
            ),
            (
                "/Users/operator/Library/Application Support/Cursor/IndexedDB",
                "electron-indexed-db",
            ),
            (
                "/Users/operator/Library/Application Support/Claude/vm_bundles/claudevm.bundle",
                "electron-vm-bundles",
            ),
        ];

        for (path, expected_pattern) in cases {
            let classification =
                classify_macos(&registry, Path::new(path), StructuralSignals::default());
            assert_eq!(
                classification.pattern_name, expected_pattern,
                "unexpected pattern for {path}"
            );
            assert_eq!(classification.category, ArtifactCategory::CacheDir);
            assert!(classification.combined_confidence > 0.70);
            assert_eq!(extract_macos_pattern_label(path), expected_pattern);
        }
    }

    #[test]
    fn application_support_app_root_is_not_an_electron_cache_candidate() {
        let registry = ArtifactPatternRegistry::default();
        let classification = classify_macos(
            &registry,
            Path::new("/Users/operator/Library/Application Support/Claude"),
            StructuralSignals::default(),
        );

        assert_ne!(classification.pattern_name, "electron-cache");
        assert_ne!(classification.pattern_name, "electron-service-worker-cache");
    }

    #[test]
    fn release_work_buildroot_shapes_are_classified() {
        let registry = ArtifactPatternRegistry::default();
        for path in [
            "/Users/operator/release-work/mcp_agent_mail_rust_buildroot",
            "/Users/operator/release-work/mcp-agent-mail-rust-buildroot",
        ] {
            let classification =
                classify_macos(&registry, Path::new(path), StructuralSignals::default());

            assert_eq!(classification.pattern_name, "release-work-buildroot");
            assert_eq!(classification.category, ArtifactCategory::BuildOutput);
            assert!(classification.combined_confidence > 0.70);
            assert_eq!(extract_macos_pattern_label(path), "release-work-buildroot");
        }
    }

    #[test]
    fn ipsw_updates_are_classified_only_in_software_updates_dir() {
        let registry = ArtifactPatternRegistry::default();
        let firmware =
            Path::new("/Users/operator/Library/iTunes/iPhone Software Updates/iPhone_17.ipsw");
        let classification = classify_macos(&registry, firmware, StructuralSignals::default());

        assert_eq!(classification.pattern_name, "ipsw-software-updates");
        assert_eq!(classification.category, ArtifactCategory::CacheDir);
        assert!(classification.combined_confidence > 0.75);
        assert_eq!(
            extract_macos_pattern_label(firmware.to_str().unwrap()),
            "ipsw-software-updates"
        );

        let downloads = Path::new("/Users/operator/Downloads/iPhone_17.ipsw");
        let downloads_classification =
            classify_macos(&registry, downloads, StructuralSignals::default());

        assert_ne!(
            downloads_classification.pattern_name,
            "ipsw-software-updates"
        );
        assert_ne!(
            extract_macos_pattern_label(downloads.to_str().unwrap()),
            "ipsw-software-updates"
        );
    }

    #[test]
    fn buildroot_outside_release_work_is_not_the_release_work_pattern() {
        let registry = ArtifactPatternRegistry::default();
        for path in [
            "/Users/operator/projects/app-buildroot",
            "/Users/operator/projects/app_buildroot",
        ] {
            let classification =
                classify_macos(&registry, Path::new(path), StructuralSignals::default());

            assert_ne!(classification.pattern_name, "release-work-buildroot");
        }
    }

    #[test]
    fn volatile_user_named_trash_shapes_are_review_candidates() {
        let registry = ArtifactPatternRegistry::default();
        for path in [
            "/private/tmp/trash",
            "/private/tmp/trashed",
            "/private/tmp/frankenterm-trash-20260503",
            "/tmp/agent-trash-20260507",
        ] {
            let classification =
                classify_macos(&registry, Path::new(path), StructuralSignals::default());

            assert_eq!(
                classification.pattern_name, "user-named-trash",
                "unexpected pattern for {path}"
            );
            assert_eq!(classification.category, ArtifactCategory::TempDir);
            assert_eq!(extract_macos_pattern_label(path), "user-named-trash");
        }
    }

    #[test]
    fn project_trash_name_is_not_a_cleanup_pattern() {
        let registry = ArtifactPatternRegistry::default();
        let classification = classify_macos(
            &registry,
            Path::new("/data/projects/app/trash"),
            StructuralSignals::default(),
        );

        assert_ne!(classification.pattern_name, "user-named-trash");
    }

    #[test]
    fn linux_temp_target_catalog_uses_shared_classification() {
        let registry = ArtifactPatternRegistry::default();
        let path = Path::new("/data/tmp/cass_append_baseline_target");
        let classification = classify_linux(&registry, path, StructuralSignals::default());

        assert_eq!(
            classification.pattern_name,
            "linux-data-tmp-underscore-target"
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert_eq!(
            extract_pattern_label_with_cleanup_rules(
                path.to_str().unwrap(),
                linux::cleanup_catalog::cleanup_rules(),
            ),
            "linux-data-tmp-underscore-target"
        );
    }

    #[test]
    fn cargo_toml_presence_penalizes_score() {
        let registry = ArtifactPatternRegistry::default();
        // A name that would normally get 0.94 confidence ("cargo-target-...")
        let classification = registry.classify(
            Path::new("cargo-target-project"),
            StructuralSignals {
                has_cargo_toml: true,
                ..StructuralSignals::default()
            },
        );
        // Should be crushed to < 0.1 despite the name match.
        assert!(
            classification.combined_confidence < 0.1,
            "score {} was not penalized enough",
            classification.combined_confidence
        );
    }

    #[test]
    fn underscore_target_source_root_is_penalized_by_cargo_toml() {
        let registry = ArtifactPatternRegistry::default();
        let classification = registry.classify(
            Path::new("/data/projects/asupersync_ansi_c/tools/rust_fuzz_target"),
            StructuralSignals {
                has_cargo_toml: true,
                ..StructuralSignals::default()
            },
        );

        assert_eq!(classification.pattern_name, "underscore-target-suffix");
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert!(
            classification.combined_confidence < 0.1,
            "Cargo.toml source root should crush *_target confidence, got {}",
            classification.combined_confidence
        );
    }

    #[test]
    fn structural_rescue_with_few_markers_gets_base_confidence() {
        let registry = ArtifactPatternRegistry::default();
        // Only fingerprint present (1 marker) — should get base rescue confidence.
        let classification = registry.classify(
            Path::new("debug"),
            StructuralSignals {
                has_fingerprint: true,
                ..StructuralSignals::default()
            },
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert_eq!(classification.pattern_name, "structural-rust-target");
        // combined = 0.70 * 0.55 + 0.30 * structural ≈ 0.68
        assert!(
            classification.combined_confidence < 0.72,
            "few markers should yield moderate confidence, got {:.3}",
            classification.combined_confidence
        );
    }

    #[test]
    fn structural_rescue_with_many_markers_gets_boosted_confidence() {
        let registry = ArtifactPatternRegistry::default();
        // Three cargo markers present — should get boosted rescue confidence.
        let classification = registry.classify(
            Path::new("debug"),
            StructuralSignals {
                has_fingerprint: true,
                has_incremental: true,
                has_deps: true,
                ..StructuralSignals::default()
            },
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert_eq!(classification.pattern_name, "structural-rust-target");
        // combined = 0.70 * 0.75 + 0.30 * 0.98 ≈ 0.82
        assert!(
            classification.combined_confidence > 0.80,
            "3+ markers should yield high confidence, got {:.3}",
            classification.combined_confidence
        );
    }

    #[test]
    fn structural_rescue_with_all_markers_gets_boosted_confidence() {
        let registry = ArtifactPatternRegistry::default();
        // All four cargo markers — definitive evidence.
        let classification = registry.classify(
            Path::new("release"),
            StructuralSignals {
                has_fingerprint: true,
                has_incremental: true,
                has_deps: true,
                has_build: true,
                ..StructuralSignals::default()
            },
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert!(
            classification.combined_confidence > 0.80,
            "all markers should yield high confidence, got {:.3}",
            classification.combined_confidence
        );
    }

    #[test]
    fn structural_rescue_does_not_fire_without_markers() {
        let registry = ArtifactPatternRegistry::default();
        // A directory named "debug" with NO cargo markers — should NOT be rescued.
        let classification = registry.classify(Path::new("debug"), StructuralSignals::default());
        assert_eq!(classification.category, ArtifactCategory::Unknown);
    }

    #[test]
    fn dot_rch_target_hyphen_is_classified() {
        let registry = ArtifactPatternRegistry::default();
        let classification = registry.classify(
            Path::new(".rch-target-quietwillow"),
            StructuralSignals::default(),
        );
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert!(classification.combined_confidence > 0.60);
    }

    #[test]
    fn dot_rch_target_bare_is_classified() {
        let registry = ArtifactPatternRegistry::default();
        let classification =
            registry.classify(Path::new(".rch-target"), StructuralSignals::default());
        assert_eq!(classification.category, ArtifactCategory::RustTarget);
        assert_eq!(classification.pattern_name, "rch-target-bare-dot");
        assert!(classification.combined_confidence > 0.60);
    }

    #[test]
    fn rch_target_bare_variants_all_classify_as_rust_target() {
        let registry = ArtifactPatternRegistry::default();
        for (name, expected_pattern) in [
            (".rch-target", "rch-target-bare-dot"),
            (".rch_target", "rch-target-bare-dot-underscore"),
            ("rch-target", "rch-target-bare-hyphen"),
            ("rch_target", "rch-target-bare-underscore"),
        ] {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_eq!(
                classification.category,
                ArtifactCategory::RustTarget,
                "{name} should classify as RustTarget"
            );
            assert_eq!(
                classification.pattern_name, expected_pattern,
                "{name} should match {expected_pattern}"
            );
        }
    }

    #[test]
    fn rch_target_bare_label_groups_with_other_rch_variants() {
        use super::extract_pattern_label;
        for name in [".rch-target", "rch-target", ".rch_target", "rch_target"] {
            assert_eq!(extract_pattern_label(name), "rch_target_*");
        }
    }

    #[test]
    fn tmp_agent_and_cache_patterns_from_real_world_dirs() {
        let registry = ArtifactPatternRegistry::default();
        let cases = [
            ("green-ft", ArtifactCategory::AgentWorkspace),
            ("frankenterm-build-1234", ArtifactCategory::AgentWorkspace),
            ("cargo-home-pearlstone", ArtifactCategory::TempDir),
            (".cargo_cache_runner", ArtifactCategory::CacheDir),
            ("work-target", ArtifactCategory::RustTarget),
        ];

        for (name, expected) in cases {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_eq!(
                classification.category, expected,
                "unexpected classification for {name}"
            );
            assert!(
                classification.combined_confidence > 0.55,
                "low confidence {:.2} for {name}",
                classification.combined_confidence
            );
        }
    }

    #[test]
    fn cass_workspace_patterns_real_world_dirs() {
        // Real /tmp dir names observed during a recent agent session that
        // filled the tmpfs with 100+ GB of leftover cass dirs. Only the
        // `cass_*`/`cass-*` workspace shape — the cargo-target sibling
        // `cass-target-*` is asserted separately below since it has its
        // own (more specific) pattern that must keep winning.
        let registry = ArtifactPatternRegistry::default();
        let cases = [
            "cass_next_profile",          // 1.4 GB profile dir
            "cass_batch20_bench",         // 4 GB cargo target
            "cass_append_sqlcache_bench", // 4 GB cargo target
            "cass_swarm",                 // small scratch
            "cass_orchestrator",          // small scratch
            "cass_marching_orders.txt",   // small text artifact
        ];

        for name in cases {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_ne!(
                classification.category,
                ArtifactCategory::Unknown,
                "{name} should be classified, not Unknown"
            );
            assert!(
                classification.combined_confidence > 0.55,
                "low confidence {:.2} for {name}",
                classification.combined_confidence
            );
        }
    }

    #[test]
    fn cass_target_prefix_not_shadowed_by_cass_workspace_prefix() {
        // Regression guard: `cass-target-*` names match BOTH the pre-existing
        // `cass-target` Prefix pattern (RustTarget) and the broader new
        // `cass-prefix-hyphen` Prefix pattern (AgentWorkspace). The more
        // specific one must win — otherwise these dirs lose the cargo-marker
        // structural boost (0.92–0.98) and collapse to the flat 0.78
        // AgentWorkspace score, which can drop them below the deletion
        // confidence floor on machines without populated cargo signals.
        let registry = ArtifactPatternRegistry::default();
        for name in ["cass-target-thatlilac", "cass-target", "cass-target-foo"] {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_eq!(
                classification.category,
                ArtifactCategory::RustTarget,
                "{name} should classify as RustTarget, got {:?}",
                classification.category
            );
            assert_eq!(
                classification.pattern_name.as_ref(),
                "cass-target",
                "{name} should match the cass-target pattern, got {}",
                classification.pattern_name
            );
        }
    }

    #[test]
    fn underscore_target_suffix_classifies_as_rust_target() {
        // `cass_append_baseline_target`, `pi_agent_rust_target`, and any
        // future agent-prefixed `..._target` cargo dir.
        let registry = ArtifactPatternRegistry::default();
        for name in [
            "cass_append_baseline_target",
            "cass_append_patch_target",
            "build_target",
            "release_target",
        ] {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_eq!(
                classification.category,
                ArtifactCategory::RustTarget,
                "{name} should classify as RustTarget"
            );
        }
    }

    #[test]
    fn frankentui_workspace_patterns_real_world_dirs() {
        // Codex bead workspace dirs: 3.7 GB each, observed in /tmp.
        let registry = ArtifactPatternRegistry::default();
        let cases = [
            "frankentui-codex-bd-2vr05-10-2-workspace",
            "frankentui-codex-bd-2vr05-10-3-workspace",
            "frankentui-codex-bd-2vr05-10-4-workspace",
            "frankentui-codex-bd-2vr05-6-text-corpus",
            "frankentui-bd-2vr05-10-4-fuzz-pass3",
            "frankentui_profile_sweep_view.data",
            "frankentui_git_clone_stderr",
        ];

        for name in cases {
            let classification = registry.classify(Path::new(name), StructuralSignals::default());
            assert_eq!(
                classification.category,
                ArtifactCategory::AgentWorkspace,
                "{name} should classify as AgentWorkspace"
            );
            assert!(
                classification.combined_confidence > 0.55,
                "low confidence {:.2} for {name}",
                classification.combined_confidence
            );
        }
    }

    #[test]
    fn frankentui_does_not_collide_with_frankenterm() {
        // The pre-existing `frankenterm-` pattern still wins for its own
        // names; the new `frankentui-` does not steal them.
        let registry = ArtifactPatternRegistry::default();
        let frankenterm = registry.classify(
            Path::new("frankenterm-build-1234"),
            StructuralSignals::default(),
        );
        assert_eq!(frankenterm.category, ArtifactCategory::AgentWorkspace);
        let frankentui = registry.classify(
            Path::new("frankentui-codex-bd-2vr05-4"),
            StructuralSignals::default(),
        );
        assert_eq!(frankentui.category, ArtifactCategory::AgentWorkspace);
    }

    #[test]
    fn extract_pattern_label_groups_new_families() {
        use super::extract_pattern_label;
        // cass family (NOT cass-target — that has its own pre-existing
        // `cass-target*` label which is more specific and should win).
        assert_eq!(extract_pattern_label("/tmp/cass_next_profile"), "cass_*");
        assert_eq!(extract_pattern_label("/tmp/cass_swarm"), "cass_*");
        assert_eq!(extract_pattern_label("/tmp/cass-orchestrator"), "cass_*");
        // The pre-existing cass-target family is unchanged.
        assert_eq!(
            extract_pattern_label("/tmp/cass-target-thatlilac"),
            "cass-target*"
        );
        // _target suffix family.
        assert_eq!(
            extract_pattern_label("/data/projects/cass_append_baseline_target"),
            "*_target"
        );
        assert_eq!(
            extract_pattern_label("/data/projects/build_target"),
            "*_target"
        );
        // frankentui family — both forms collapse.
        assert_eq!(
            extract_pattern_label("/tmp/frankentui-codex-bd-2vr05-10-2-workspace"),
            "frankentui-*"
        );
        assert_eq!(
            extract_pattern_label("/tmp/frankentui_profile_sweep_view.data"),
            "frankentui-*"
        );
    }
}
