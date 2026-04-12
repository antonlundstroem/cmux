use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitInfo {
    /// The shared `.git` directory across all worktrees of a repo. Used as the grouping key.
    pub common_dir: PathBuf,
    /// The current worktree's top-level directory (the cwd of that worktree).
    pub worktree_path: PathBuf,
    /// Current branch, or "(detached)" for detached HEAD.
    pub branch: String,
    /// Human-readable repo name — basename of the main worktree, derived from `common_dir`'s parent.
    pub repo_name: String,
}

/// Probe git info for a given cwd. Returns None when cwd isn't in a git repo
/// or when git itself fails.
pub fn probe(cwd: &Path) -> Option<GitInfo> {
    // Grouping info — must succeed for us to treat this as a git repo.
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--git-common-dir", "--show-toplevel"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let common_raw = lines.next()?.to_string();
    let toplevel = lines.next()?.to_string();

    // Branch name — best-effort. Fails on bare repos with no commits, orphan
    // branches, or ambiguous HEAD. That's fine; we just show "?".
    let branch = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // --git-common-dir may return a relative path (e.g. ".git") — resolve it against cwd.
    let common_dir_rel = PathBuf::from(&common_raw);
    let common_dir = if common_dir_rel.is_absolute() {
        common_dir_rel
    } else {
        cwd.join(common_dir_rel)
    };
    // Canonicalize to collapse `..` and symlinks so two worktrees of the same repo
    // produce identical grouping keys.
    let common_dir = std::fs::canonicalize(&common_dir).unwrap_or(common_dir);

    // For a normal repo, common_dir ends in `.git` (e.g. `/repo/.git`), so
    // the repo name is the parent directory's basename.
    // For a bare repo, common_dir IS the repo (e.g. `/work/infrastructure.git`),
    // so the repo name is its own basename with `.git` stripped.
    let repo_name = {
        let cd_name = common_dir.file_name().map(|s| s.to_string_lossy().into_owned());
        if cd_name.as_deref() == Some(".git") {
            // Normal repo: /repo/.git → parent basename
            common_dir
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "repo".to_string())
        } else {
            // Bare repo: /work/infrastructure.git → "infrastructure"
            cd_name
                .map(|s| s.trim_end_matches(".git").to_string())
                .unwrap_or_else(|| "repo".to_string())
        }
    };

    Some(GitInfo {
        common_dir,
        worktree_path: PathBuf::from(toplevel),
        branch: match branch.as_str() {
            "" => "?".to_string(),
            "HEAD" => "(detached)".to_string(),
            _ => branch,
        },
        repo_name,
    })
}
