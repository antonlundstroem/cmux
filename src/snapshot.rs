//! Top-level snapshot: build the list of live claude panes, enriched with
//! git + state info, grouped by repo.

use rayon::prelude::*;
use std::path::PathBuf;

use crate::claude_state::{self, ClaudeState};
use crate::git::{self, GitInfo};
use crate::tmux::{self, RawPane};

#[derive(Debug, Clone)]
pub struct LivePane {
    pub pane_id: String,  // stable tmux id ("%42") — preview cache key
    pub target: String,   // "session:window.pane"
    pub cwd: PathBuf,
    pub git: Option<GitInfo>,
    pub state: ClaudeState,
    pub idle_secs: u64,
}

/// One repo group + the panes inside it. `repo_name` is `None` for panes that
/// aren't inside any git repo.
#[derive(Debug, Clone)]
pub struct Group {
    pub repo_name: Option<String>,
    pub panes: Vec<LivePane>,
}

pub fn snapshot() -> Vec<Group> {
    let raw = tmux::list_panes();
    let claude_panes: Vec<RawPane> = raw.into_iter().filter(is_claude).collect();

    // Per-pane enrichment runs in parallel: each pane is an independent
    // capture-pane + git rev-parse pair. Shell-outs dominate latency so
    // parallelism buys real wall-clock time.
    let enriched: Vec<LivePane> = claude_panes
        .into_par_iter()
        .map(|p| {
            let tail = tmux::capture_pane_tail(&p.pane_id);
            let state = claude_state::detect(&tail);
            let git = git::probe(&p.cwd);
            LivePane {
                pane_id: p.pane_id,
                target: p.target,
                cwd: p.cwd,
                git,
                state,
                idle_secs: p.activity_secs,
            }
        })
        .collect();

    group_by_repo(enriched)
}

fn is_claude(p: &RawPane) -> bool {
    // The foreground process name tmux reports. A shell wrapping claude would
    // show up as "bash" — we intentionally don't handle that case in v1.
    p.current_command == "claude"
}

fn group_by_repo(panes: Vec<LivePane>) -> Vec<Group> {
    use std::collections::BTreeMap;
    let mut groups: BTreeMap<Option<PathBuf>, Group> = BTreeMap::new();

    for pane in panes {
        let key = pane.git.as_ref().map(|g| g.common_dir.clone());
        let repo_name = pane.git.as_ref().map(|g| g.repo_name.clone());
        let entry = groups.entry(key).or_insert_with(|| Group {
            repo_name,
            panes: Vec::new(),
        });
        entry.panes.push(pane);
    }

    // Sort panes within each group by worktree path, then by target.
    for g in groups.values_mut() {
        g.panes.sort_by(|a, b| {
            let wa = a.git.as_ref().map(|g| g.worktree_path.as_path());
            let wb = b.git.as_ref().map(|g| g.worktree_path.as_path());
            wa.cmp(&wb).then_with(|| a.target.cmp(&b.target))
        });
    }

    // Groups with a repo come first (alphabetical by repo name), then
    // the ungrouped "Other" bucket (common_dir = None) at the end.
    let mut out: Vec<Group> = groups.into_values().collect();
    out.sort_by(|a, b| match (&a.repo_name, &b.repo_name) {
        (Some(x), Some(y)) => x.cmp(y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    out
}
