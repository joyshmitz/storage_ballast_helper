#![allow(missing_docs)]

use std::path::Path;
use storage_ballast_helper::scanner::protection::ProtectionRegistry;

#[test]
fn repro_glob_double_star_boundary() {
    // Pattern: src/**/main.rs
    // Should match: src/main.rs, src/foo/main.rs
    // Should NOT match: src/badmain.rs

    let patterns = vec!["src/**/main.rs".to_string()];
    let reg = ProtectionRegistry::new(Some(&patterns)).expect("valid pattern");

    // Expectation:
    assert!(
        reg.is_protected(Path::new("src/main.rs")),
        "should match src/main.rs"
    );
    assert!(
        reg.is_protected(Path::new("src/foo/main.rs")),
        "should match src/foo/main.rs"
    );

    // THIS IS THE BUG:
    // It currently matches src/badmain.rs because ** becomes .* (matching "bad")
    // and the /? (optional slash) matches nothing.
    assert!(
        !reg.is_protected(Path::new("src/badmain.rs")),
        "should NOT match src/badmain.rs"
    );
}
