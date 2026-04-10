/// file_loader.rs — Recursive JS/TS file discovery.
///
/// Walks a directory tree (or returns a single file) and collects all
/// files with React-relevant extensions: .js, .jsx, .ts, .tsx
///
/// Automatically ignores:
///   - node_modules/
///   - dist/
///   - build/
///   - coverage/
///   - .git/ and any hidden directory (starts with '.')
///   - __tests__/ directories (unless `include_tests` is true)
///
/// Test/Storybook files are also filtered out by default:
///   *.test.{js,ts,jsx,tsx}
///   *.spec.{js,ts,jsx,tsx}
///   *.stories.{js,ts,jsx,tsx}
///   *.story.{js,ts,jsx,tsx}
use std::path::{Path, PathBuf};

use walkdir::{DirEntry, WalkDir};

/// Extensions we care about. Checked against `file.extension()`.
const VALID_EXTENSIONS: &[&str] = &["js", "jsx", "ts", "tsx"];

/// Directory names we always skip. Checked against each path component.
const IGNORED_DIRS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "coverage",
    ".git",
    "artifacts",
    "storybook-static",
];

/// Directory names skipped when `include_tests` is false.
const TEST_DIRS: &[&str] = &["__tests__", "__mocks__", "__fixtures__", "e2e", "cypress"];

/// File stem suffixes that identify test/storybook files.
/// Matched against the filename stem segments separated by '.'.
const TEST_SUFFIXES: &[&str] = &["test", "spec", "stories", "story"];

/// Collect all JS/TS/JSX/TSX files under `root`.
///
/// If `root` is a file, it is returned directly (after extension check).
/// If `root` is a directory, it is walked recursively.
///
/// Set `include_tests = true` to also include test, spec, stories, and
/// __tests__ files (skipped by default because those patterns produce
/// high false-positive rates).
///
/// # Errors
/// Files that cannot be read are silently skipped. If `root` does not
/// exist, an empty Vec is returned and a warning is printed.
pub fn collect_files(root: &Path, include_tests: bool) -> Vec<PathBuf> {
    // If the user pointed at a single file, return it directly.
    if root.is_file() {
        return if has_valid_extension(root) {
            vec![root.to_path_buf()]
        } else {
            eprintln!(
                "Warning: {} is not a JS/TS file — skipping.",
                root.display()
            );
            vec![]
        };
    }

    if !root.exists() {
        eprintln!("Error: path '{}' does not exist.", root.display());
        return vec![];
    }

    // Walk the directory tree.
    // `filter_entry` prunes entire subtrees when it returns false,
    // which is more efficient than filtering leaf-by-leaf.
    WalkDir::new(root)
        .follow_links(false) // Don't follow symlinks to avoid cycles.
        .into_iter()
        .filter_entry(|entry| !is_ignored_dir(entry, include_tests))
        .filter_map(|result| match result {
            Ok(entry) => {
                if entry.file_type().is_file()
                    && has_valid_extension(entry.path())
                    && (include_tests || !is_test_file(entry.path()))
                {
                    Some(entry.into_path())
                } else {
                    None
                }
            }
            Err(err) => {
                // Log permission errors etc. but don't abort.
                eprintln!("Warning: could not read entry — {err}");
                None
            }
        })
        .collect()
}

/// Returns `true` if the directory entry should be pruned from the walk.
///
/// `filter_entry` is called on *both* files and directories; we only
/// prune directories here — file-level filtering happens in the caller.
fn is_ignored_dir(entry: &DirEntry, include_tests: bool) -> bool {
    if !entry.file_type().is_dir() {
        return false;
    }

    let dir_name = entry.file_name().to_str().unwrap_or("");

    // Skip hidden directories (e.g. .git, .cache, .next, .storybook).
    if dir_name.starts_with('.') {
        return true;
    }

    // Always-ignored directories.
    if IGNORED_DIRS.contains(&dir_name) {
        return true;
    }

    // Test-related directories — skip unless the user opted in.
    if !include_tests && TEST_DIRS.contains(&dir_name) {
        return true;
    }

    false
}

/// Returns `true` if the file is a test, spec, or storybook file.
///
/// Checks whether any dot-separated stem segment (other than the
/// final extension) matches a known test suffix:
///
/// ```
/// Button.test.tsx      → true   ("test" segment found)
/// Button.stories.tsx   → true   ("stories" segment found)
/// Button.spec.ts       → true   ("spec" segment found)
/// Button.tsx           → false
/// useMyHook.ts         → false
/// ```
fn is_test_file(path: &Path) -> bool {
    // file_name gives us "Button.test.tsx"
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };

    // Split on '.' → ["Button", "test", "tsx"]
    // We check every segment except the last one (the extension).
    let parts: Vec<&str> = name.split('.').collect();
    if parts.len() <= 2 {
        // "Button.tsx" — single-segment stem, no suffix to check.
        return false;
    }

    // Check all middle segments (everything except first and last).
    parts[1..parts.len() - 1]
        .iter()
        .any(|seg| TEST_SUFFIXES.contains(seg))
}

/// Returns `true` if the file's extension is in our allow-list.
fn has_valid_extension(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| VALID_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_has_valid_extension() {
        assert!(has_valid_extension(Path::new("App.tsx")));
        assert!(has_valid_extension(Path::new("index.js")));
        assert!(has_valid_extension(Path::new("Component.jsx")));
        assert!(has_valid_extension(Path::new("types.ts")));
        assert!(!has_valid_extension(Path::new("style.css")));
        assert!(!has_valid_extension(Path::new("README.md")));
        assert!(!has_valid_extension(Path::new("config.json")));
        assert!(!has_valid_extension(Path::new("no_extension")));
    }

    #[test]
    fn test_is_test_file() {
        // Should be detected as test/storybook files
        assert!(is_test_file(Path::new("Button.test.tsx")));
        assert!(is_test_file(Path::new("Button.spec.tsx")));
        assert!(is_test_file(Path::new("Button.stories.tsx")));
        assert!(is_test_file(Path::new("Button.story.tsx")));
        assert!(is_test_file(Path::new("useHook.test.ts")));
        assert!(is_test_file(Path::new("utils.spec.js")));
        assert!(is_test_file(Path::new("Card.stories.jsx")));

        // Should NOT be detected as test files
        assert!(!is_test_file(Path::new("Button.tsx")));
        assert!(!is_test_file(Path::new("useHook.ts")));
        assert!(!is_test_file(Path::new("index.js")));
        assert!(!is_test_file(Path::new("TestUtils.tsx"))); // PascalCase, not a suffix
    }
}
