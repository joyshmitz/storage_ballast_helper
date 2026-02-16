//! Shared path manipulation utilities.

use std::env;
use std::path::{Component, Path, PathBuf};

/// Resolve a path to an absolute, normalized path.
///
/// If `fs::canonicalize` succeeds (path exists), it is used to resolve symlinks
/// and normalize components.
///
/// If it fails (e.g. path does not exist), the path is made absolute relative
/// to CWD and `..`/`.` components are resolved syntactically.
pub fn resolve_absolute_path(path: &Path) -> PathBuf {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir().map_or_else(|_| path.to_path_buf(), |cwd| cwd.join(path))
    };

    // Try filesystem resolution first (handles symlinks).
    if let Ok(canonical) = std::fs::canonicalize(&absolute) {
        return canonical;
    }

    // Fallback: syntactic normalization.
    normalize_syntactic(&absolute)
}

fn normalize_syntactic(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(..) | Component::RootDir | Component::Normal(_) => {
                components.push(component);
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if let Some(Component::Normal(_)) = components.last() {
                    components.pop();
                }
            }
        }
    }
    components.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_existing_path_canonically() {
        let cwd = env::current_dir().unwrap();
        let resolved = resolve_absolute_path(Path::new("."));
        assert_eq!(resolved, std::fs::canonicalize(&cwd).unwrap());
    }

    #[test]
    fn normalizes_nonexistent_path_syntactically() {
        // /nonexistent/foo/../bar -> /nonexistent/bar
        // Note: we assume /nonexistent doesn't exist.
        #[cfg(unix)]
        let root = Path::new("/");
        #[cfg(windows)]
        let root = Path::new("C:");

        let input = root.join("nonexistent").join("foo").join("..").join("bar");
        let expected = root.join("nonexistent").join("bar");

        // Ensure input doesn't exist so we trigger fallback
        assert!(std::fs::canonicalize(&input).is_err());

        let resolved = resolve_absolute_path(&input);
        assert_eq!(resolved, expected);
    }

    #[test]
    fn handles_parent_at_root() {
        #[cfg(unix)]
        {
            let input = Path::new("/../foo");
            let resolved = normalize_syntactic(input);
            assert_eq!(resolved, Path::new("/foo"));
        }
    }
}
