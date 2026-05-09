#![allow(missing_docs)]

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;
    use storage_ballast_helper::core::config::ScoringConfig;
    use storage_ballast_helper::scanner::patterns::{ArtifactPatternRegistry, StructuralSignals};
    use storage_ballast_helper::scanner::scoring::{
        ActiveReferenceSummary, CandidateInput, DecisionAction, ScoringEngine,
    };

    fn default_engine() -> ScoringEngine {
        ScoringEngine::from_config(&ScoringConfig::default(), 4) // 4 hours min age
    }

    #[test]
    fn source_cargo_prefix_is_not_deleted() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        // Scenario: A source module named "cargo_utils" (e.g. src/cargo_utils/)
        // It has no artifacts, no Cargo.toml (it's a module, not a crate root).
        let path = PathBuf::from("/data/projects/mycrate/src/cargo_utils");
        let signals = StructuralSignals::default(); // No markers

        let classification = registry.classify(&path, signals);

        assert_ne!(classification.pattern_name, "cargo-prefix");
        assert!(
            classification.combined_confidence < 0.2,
            "Confidence should stay low for source dir: {}",
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

        assert_eq!(
            score.decision.action,
            DecisionAction::Keep,
            "Source module 'cargo_utils' must not be flagged for deletion"
        );
    }

    #[test]
    fn source_cache_dir_is_not_deleted() {
        let registry = ArtifactPatternRegistry::default();
        let engine = default_engine();

        // Scenario: A source module named "cache" (e.g. src/cache/)
        let path = PathBuf::from("/data/projects/mycrate/src/cache");
        let signals = StructuralSignals::default();

        let classification = registry.classify(&path, signals);

        assert_eq!(classification.pattern_name, "generic-cache-exact");

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

        let score = engine.score_candidate(&input, 0.5);
        assert_eq!(
            score.decision.action,
            DecisionAction::Keep,
            "Source module 'cache' must not be flagged for deletion"
        );
    }
}
