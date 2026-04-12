use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct GitInfo {
    /// The shared `.git` directory across all worktrees of a repo. Used as the grouping key.
    pub common_dir: PathBuf,
    /// The current worktree's top-level directory (the cwd of that worktree).
    pub worktree_path: PathBuf,
    /// Current branch, or "(detached)" / "?" for detached/unknown HEAD.
    pub branch: String,
    /// Human-readable repo name — basename of the main worktree.
    pub repo_name: String,
    /// Whether this pane is in a linked worktree (not the main checkout).
    pub is_worktree: bool,
    /// Uncommitted changes present.
    pub dirty: bool,
    /// Commits ahead of upstream.
    pub ahead: u32,
    /// Commits behind upstream.
    pub behind: u32,
}

pub fn probe(cwd: &Path) -> Option<GitInfo> {
    // Grouping info — must succeed for us to treat this as a git repo.
    let out = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args([
            "rev-parse",
            "--git-dir",
            "--git-common-dir",
            "--show-toplevel",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut lines = text.lines();
    let git_dir_raw = lines.next()?.to_string();
    let common_raw = lines.next()?.to_string();
    let toplevel = lines.next()?.to_string();

    // Branch name — best-effort.
    let branch = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();

    // Resolve relative paths against cwd.
    let resolve = |raw: &str| -> PathBuf {
        let p = PathBuf::from(raw);
        let abs = if p.is_absolute() { p } else { cwd.join(p) };
        std::fs::canonicalize(&abs).unwrap_or(abs)
    };
    let git_dir = resolve(&git_dir_raw);
    let common_dir = resolve(&common_raw);

    // Worktree = git-dir differs from common-dir (linked worktrees have their
    // own .git/worktrees/<name> directory).
    let is_worktree = git_dir != common_dir;

    let repo_name = {
        let cd_name = common_dir
            .file_name()
            .map(|s| s.to_string_lossy().into_owned());
        if cd_name.as_deref() == Some(".git") {
            common_dir
                .parent()
                .and_then(|p| p.file_name())
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "repo".to_string())
        } else {
            cd_name
                .map(|s| s.trim_end_matches(".git").to_string())
                .unwrap_or_else(|| "repo".to_string())
        }
    };

    // Git status for dirty/ahead/behind — best-effort.
    let (dirty, ahead, behind) = probe_status(cwd);

    Some(GitInfo {
        common_dir,
        worktree_path: PathBuf::from(toplevel),
        branch: match branch.as_str() {
            "" => "?".to_string(),
            "HEAD" => "(detached)".to_string(),
            _ => branch,
        },
        repo_name,
        is_worktree,
        dirty,
        ahead,
        behind,
    })
}

fn probe_status(cwd: &Path) -> (bool, u32, u32) {
    let out = match Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(["status", "--porcelain=v2", "--branch", "--untracked-files=no"])
        .output()
    {
        Ok(o) if o.status.success() => o,
        _ => return (false, 0, 0),
    };

    let text = String::from_utf8_lossy(&out.stdout);
    let mut dirty = false;
    let mut ahead: u32 = 0;
    let mut behind: u32 = 0;

    for line in text.lines() {
        if let Some(ab) = line.strip_prefix("# branch.ab ") {
            // Format: "+N -M"
            for part in ab.split_whitespace() {
                if let Some(n) = part.strip_prefix('+') {
                    ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    behind = n.parse().unwrap_or(0);
                }
            }
        } else if !line.starts_with('#') && !line.is_empty() {
            dirty = true;
        }
    }

    (dirty, ahead, behind)
}
