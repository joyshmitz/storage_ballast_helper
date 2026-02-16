#![allow(missing_docs)]

#[cfg(test)]
mod tests {
    use std::path::Path;
    use storage_ballast_helper::core::paths::resolve_absolute_path;

    #[test]
    fn resolve_absolute_path_allows_traversal() {
        // This test demonstrates that normalize_syntactic (the fallback when
        // canonicalize fails) allows ".." to escape intended roots if the
        // intermediate paths don't exist.

        // Assume /nonexistent_root does not exist.
        // We want to see if we can resolve to /etc/passwd from it.
        // Input: /nonexistent_root/../etc/passwd
        // Expected SAFE behavior: Should probably fail or stay within some logical root if bounded.
        // Actual behavior: Resolves to /etc/passwd

        let bad_path = Path::new("/nonexistent_root/../etc/passwd");
        let resolved = resolve_absolute_path(bad_path);

        // If this assertion passes, it means the function resolved it to /etc/passwd
        // purely via syntactic normalization, ignoring directory boundaries.
        // This confirms the "vulnerability" (or at least the behavior) of
        // pure syntactic normalization without a chroot/jail.
        assert_eq!(resolved, Path::new("/etc/passwd"));
    }
}
