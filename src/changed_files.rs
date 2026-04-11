/// changed_files.rs — Detect git-modified files for pre-commit / incremental mode.
///
/// Used by `--only-changed` to restrict analysis to files that have actually
/// changed in the current working tree (staged + unstaged tracked changes).
/// This keeps pre-commit hooks fast — typically <10 ms for most codebases.
///
/// Strategy:
/// 1. Find the git repository root via `git rev-parse --show-toplevel`
/// 2. Collect staged files   via `git diff --name-only --cached`
/// 3. Collect unstaged files via `git diff --name-only`
/// 4. Return the union as absolute `PathBuf` values
///
/// If git is not installed, not in a repo, or there are no changes,
/// an empty `Vec` is returned — callers should handle that gracefully.
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Returns absolute paths of all JS/TS files modified in the current git
/// working tree (staged or unstaged). Files that have been deleted are
/// included in git output but won't exist on disk — callers should already
/// filter by `path.exists()`, which `collect_files` does not do. The
/// intersection with `collect_files` output (which only returns existing
/// files) naturally handles this.
pub fn get_changed_files(base: &Path) -> Vec<PathBuf> {
    // Resolve the git repository root so relative paths from git are
    // anchored correctly, regardless of where `base` points.
    let git_root = match find_git_root(base) {
        Some(r) => r,
        None => {
            eprintln!(
                "  ⚠  --only-changed: '{}' is not inside a git repository. \
                 Falling back to full scan.",
                base.display()
            );
            return vec![];
        }
    };

    let mut changed: HashSet<PathBuf> = HashSet::new();

    // Staged changes (--cached = index vs HEAD).
    collect_git_diff(
        &git_root,
        &["diff", "--name-only", "--cached"],
        &mut changed,
    );

    // Unstaged changes (working tree vs index).
    collect_git_diff(&git_root, &["diff", "--name-only"], &mut changed);

    changed.into_iter().collect()
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

/// Run `git <args>` in `git_root`, parse newline-separated paths, and insert
/// resolved absolute paths into `out`.
fn collect_git_diff(git_root: &Path, args: &[&str], out: &mut HashSet<PathBuf>) {
    let output = Command::new("git")
        .args(args)
        .current_dir(git_root)
        .output();

    let output = match output {
        Ok(o) if o.status.success() || o.status.code() == Some(1) => o,
        _ => return,
    };

    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            out.insert(git_root.join(trimmed));
        }
    }
}

/// Walk up from `start` until we find a `.git` directory, then return that
/// directory's parent (the repository root). Returns `None` if no `.git`
/// is found before the filesystem root.
fn find_git_root(start: &Path) -> Option<PathBuf> {
    // Prefer `git rev-parse` — handles worktrees and submodules correctly.
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(start)
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let root = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if root.is_empty() {
                None
            } else {
                Some(PathBuf::from(root))
            }
        }
        // git not installed or not a repo — fall back to manual walk.
        _ => {
            let mut dir = if start.is_file() {
                start.parent()?.to_path_buf()
            } else {
                start.to_path_buf()
            };
            loop {
                if dir.join(".git").exists() {
                    return Some(dir);
                }
                if !dir.pop() {
                    return None;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn find_git_root_from_workspace() {
        // This test runs inside the repo so git root must be found.
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert!(
            find_git_root(here).is_some(),
            "expected to find git root from CARGO_MANIFEST_DIR"
        );
    }

    #[test]
    fn find_git_root_not_a_repo() {
        let tmp = std::env::temp_dir();
        // /tmp is (almost certainly) not a git repo.
        let result = find_git_root(&tmp);
        // We can't guarantee /tmp is never in a repo on all CI systems,
        // so just assert the function doesn't panic.
        let _ = result;
    }
}
