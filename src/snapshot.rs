//! Top-level snapshot: build the list of live agent panes, enriched with
//! git + state info, grouped by repo.

use rayon::prelude::*;
use std::path::PathBuf;

use crate::claude_state::{self, AgentKind, ClaudeState};
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
    pub agent_kind: AgentKind,
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
    let agent_panes: Vec<(RawPane, AgentKind)> = raw
        .into_iter()
        .filter_map(|p| agent_kind_of(&p).map(|k| (p, k)))
        .collect();

    // Per-pane enrichment runs in parallel: each pane is an independent
    // capture-pane + git rev-parse pair. Shell-outs dominate latency so
    // parallelism buys real wall-clock time.
    let enriched: Vec<LivePane> = agent_panes
        .into_par_iter()
        .map(|(p, kind)| {
            let tail = tmux::capture_pane_tail(&p.pane_id);
            let state = claude_state::detect_with_hooks(&p.pane_id, &tail, kind);
            let git = git::probe(&p.cwd);
            LivePane {
                pane_id: p.pane_id,
                target: p.target,
                cwd: p.cwd,
                git,
                state,
                idle_secs: p.activity_secs,
                agent_kind: kind,
            }
        })
        .collect();

    group_by_repo(enriched)
}

fn agent_kind_of(p: &RawPane) -> Option<AgentKind> {
    // The foreground process name tmux reports. A shell wrapping an agent would
    // show up as "bash" — we intentionally don't handle that case.
    //
    // NixOS wraps claude in a shell script that execs `.claude-unwrapped`, so the
    // process name leaks through as that. Match both forms.
    if p.current_command == "claude" || p.current_command == ".claude-unwrapped" {
        Some(AgentKind::Claude)
    } else if p.current_command.starts_with("cursor") {
        Some(AgentKind::Cursor)
    } else {
        None
    }
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
