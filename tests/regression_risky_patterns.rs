#![allow(missing_docs)]

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::Duration;
    use storage_ballast_helper::core::config::ScoringConfig;
    use storage_ballast_helper::platform::sacred_catalog::cross_platform_sacred_paths;
    use storage_ballast_helper::scanner::patterns::{
        ArtifactCategory, ArtifactPatternRegistry, StructuralSignals,
    };
    use storage_ballast_helper::scanner::protection::find_sacred_overlaps;
    use storage_ballast_helper::scanner::scoring::{
        ActiveReferenceSummary, CandidateInput, DecisionAction, ScoringEngine,
    };
    use tempfile::Builder as TempDirBuilder;

    fn default_engine() -> ScoringEngine {
        ScoringEngine::from_config(&ScoringConfig::default(), 4) // 4 hours min age
    }

    #[test]
    fn fixed_cargo_prefix_safe_for_source_module() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        // Scenario: A source module named "cargo_utils" (e.g. src/cargo_utils/)
        // It has no artifacts, no Cargo.toml (it's a module, not a crate root).
        let path = PathBuf::from("/data/projects/mycrate/src/cargo_utils");
        let signals = StructuralSignals::default(); // No markers

        let classification = registry.classify(&path, signals);

        // Assert that it DOES NOT match the dangerous "cargo-prefix" pattern anymore.
        // It should fall back to unknown or some other low-confidence match.
        assert_ne!(classification.pattern_name, "cargo-prefix");
        assert!(
            classification.combined_confidence < 0.2,
            "Confidence should be low for source dir: {}",
            classification.combined_confidence
        );

        let input = CandidateInput {
            path,
            size_bytes: 4096,
            age: Duration::from_hours(24), // Old
            classification,
            signals,
            active_references: ActiveReferenceSummary::default(),
            is_open: false,
            excluded: false,
        };

        let score = engine.score_candidate(&input, 0.5);

        // Should now be Keep (score too low).
        assert_eq!(
            score.decision.action,
            DecisionAction::Keep,
            "SAFE: Source module 'cargo_utils' kept."
        );
    }

    #[test]
    fn fixed_cache_name_safe_for_source_module() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        // Scenario: A source module named "cache" (e.g. src/cache/)
        let path = PathBuf::from("/data/projects/mycrate/src/cache");
        let signals = StructuralSignals::default();

        let classification = registry.classify(&path, signals);

        // Matches generic-cache-exact, but confidence should be lower (0.45).
        assert_eq!(classification.pattern_name, "generic-cache-exact");
        assert!(classification.name_confidence <= 0.45);

        let input = CandidateInput {
            path,
            size_bytes: 4096,
            age: Duration::from_hours(24),
            classification,
            signals,
            active_references: ActiveReferenceSummary::default(),
            is_open: false,
            excluded: false,
        };

        // Score:
        // Location (0.40) * 0.25 = 0.10
        // Name (0.45) * 0.25 = 0.1125
        // Age (1.0) * 0.20 = 0.20
        // Size (0.05) * 0.15 = 0.0075
        // Structure (0.40) * 0.15 = 0.06
        // Total = 0.48 < 0.5 (min_score).

        let score = engine.score_candidate(&input, 0.5); // Moderate pressure
        assert_eq!(
            score.decision.action,
            DecisionAction::Keep,
            "SAFE: Source module 'cache' kept."
        );
    }

    #[test]
    fn fixed_underscore_target_source_crate_is_hard_kept() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        let path = PathBuf::from("/data/projects/asupersync_ansi_c/tools/rust_fuzz_target");
        let signals = StructuralSignals {
            has_cargo_toml: true,
            ..StructuralSignals::default()
        };
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "underscore-target-suffix");
        assert!(
            classification.combined_confidence < 0.1,
            "Cargo.toml source root should not retain high *_target confidence: {}",
            classification.combined_confidence
        );

        let input = CandidateInput {
            path,
            size_bytes: 5 * 1_073_741_824,
            age: Duration::from_hours(336),
            classification,
            signals,
            active_references: ActiveReferenceSummary::default(),
            is_open: false,
            excluded: false,
        };

        let score = engine.score_candidate(&input, 0.95);

        assert_eq!(score.decision.action, DecisionAction::Keep);
        assert_eq!(
            score.veto_reason.as_deref(),
            Some("contains Cargo.toml without build-artifact markers")
        );
    }

    #[test]
    fn generic_target_suffix_source_dir_without_markers_is_hard_kept() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        let path = PathBuf::from("/data/projects/asupersync_ansi_c/tools/rust_fuzz_target");
        let signals = StructuralSignals::default();
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "underscore-target-suffix");

        let score = engine.score_candidate(
            &CandidateInput {
                path,
                size_bytes: 5 * 1_073_741_824,
                age: Duration::from_hours(336),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.95,
        );

        assert_eq!(score.decision.action, DecisionAction::Keep);
        assert_eq!(
            score.veto_reason.as_deref(),
            Some("target-like name lacks Cargo build markers outside temporary storage")
        );
    }

    #[test]
    fn private_tmp_target_underscore_prefix_is_actionable_when_old_and_unreferenced() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        let path = PathBuf::from("/private/tmp/target_rust_fuzz_42");
        let signals = StructuralSignals::default();
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "target-underscore-prefix");
        assert_eq!(classification.category, ArtifactCategory::RustTarget);

        let score = engine.score_candidate(
            &CandidateInput {
                path,
                size_bytes: 5 * 1_073_741_824,
                age: Duration::from_hours(48),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.95,
        );

        assert!(!score.vetoed);
        assert_eq!(score.decision.action, DecisionAction::Delete);
    }

    #[test]
    fn xcode_derived_data_project_dir_is_actionable_build_output() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();
        let path =
            PathBuf::from("/Users/operator/Library/Developer/Xcode/DerivedData/sbh-demo-abc123");
        let signals = StructuralSignals {
            has_build: true,
            mostly_object_files: true,
            ..StructuralSignals::default()
        };
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "xcode-derived-data");
        assert_eq!(classification.category, ArtifactCategory::BuildOutput);

        let score = engine.score_candidate(
            &CandidateInput {
                path,
                size_bytes: 3 * 1_073_741_824,
                age: Duration::from_hours(24 * 8),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.8,
        );

        assert!(!score.vetoed);
        assert_eq!(score.decision.action, DecisionAction::Delete);
    }

    #[test]
    fn electron_service_worker_cache_is_actionable_cache_dir() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();
        let path = PathBuf::from(
            "/Users/operator/Library/Application Support/Claude/Service Worker/CacheStorage/session-1",
        );
        let signals = StructuralSignals::default();
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "electron-service-worker-cache");
        assert_eq!(classification.category, ArtifactCategory::CacheDir);

        let score = engine.score_candidate(
            &CandidateInput {
                path,
                size_bytes: 2 * 1_073_741_824,
                age: Duration::from_hours(12),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.8,
        );

        assert!(!score.vetoed);
        assert_eq!(score.decision.action, DecisionAction::Delete);
    }

    #[test]
    fn private_tmp_user_named_trash_requires_review_not_delete() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();
        let path = PathBuf::from("/tmp/frankenterm-trash-20260503");
        let signals = StructuralSignals::default();
        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "user-named-trash");
        assert_eq!(classification.category, ArtifactCategory::TempDir);

        let score = engine.score_candidate(
            &CandidateInput {
                path,
                size_bytes: 8 * 1_073_741_824,
                age: Duration::from_hours(336),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.95,
        );

        assert!(!score.vetoed);
        assert_eq!(score.decision.action, DecisionAction::Review);
        assert!(score.ledger.summary.contains("action=Review"));
    }

    #[test]
    fn private_tmp_user_named_trash_with_beads_stowaway_is_hard_kept() {
        let dir = TempDirBuilder::new()
            .prefix("sbh-agent-trash-")
            .tempdir_in("/tmp")
            .expect("create tmp trash candidate");
        let candidate_path = dir.path().to_path_buf();
        fs::create_dir_all(candidate_path.join("nested").join(".beads"))
            .expect("create beads stowaway");
        fs::write(
            candidate_path
                .join("nested")
                .join(".beads")
                .join("beads.db"),
            b"project state",
        )
        .expect("write beads stowaway");

        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();
        let signals = StructuralSignals::default();
        let classification = registry.classify(&candidate_path, signals);
        let overlaps = find_sacred_overlaps(&candidate_path, cross_platform_sacred_paths())
            .expect("scan stowaways");

        assert_eq!(classification.pattern_name, "user-named-trash");
        assert!(overlaps.iter().any(|overlap| overlap.pattern == ".beads/"));

        let score = engine.score_candidate_with_sacred_overlaps(
            &CandidateInput {
                path: candidate_path,
                size_bytes: 8 * 1_073_741_824,
                age: Duration::from_hours(336),
                classification,
                signals,
                active_references: ActiveReferenceSummary::default(),
                is_open: false,
                excluded: false,
            },
            0.95,
            &overlaps,
        );

        assert!(score.vetoed);
        assert_eq!(score.decision.action, DecisionAction::Keep);
        assert!(score.veto_reason.as_deref().is_some_and(|reason| {
            reason.contains("sacred path overlap") && reason.contains(".beads/")
        }));
    }
}
