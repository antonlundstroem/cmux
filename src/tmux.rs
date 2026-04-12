use std::path::PathBuf;
use std::process::Command;

/// One row from `tmux list-panes -a` before enrichment.
#[derive(Debug, Clone)]
pub struct RawPane {
    pub pane_id: String,         // "%42"
    pub current_command: String, // "claude", "bash", ...
    pub cwd: PathBuf,
    pub target: String,          // "session:window.pane"
    pub activity_secs: u64,      // seconds since pane_activity
}

const LIST_FMT: &str = "#{pane_id}\t#{pane_current_command}\t#{pane_current_path}\t#{session_name}\t#{window_index}\t#{pane_index}\t#{pane_activity}";

/// Snapshot all panes across all tmux sessions on the current server.
///
/// Returns empty vec on any tmux failure (no server, command missing, etc.) —
/// ctui should still launch and show an empty list rather than crash.
pub fn list_panes() -> Vec<RawPane> {
    let out = match Command::new("tmux")
        .args(["list-panes", "-a", "-F", LIST_FMT])
        .output()
    {
        Ok(o) if o.status.success() => o.stdout,
        _ => return Vec::new(),
    };

    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    String::from_utf8_lossy(&out)
        .lines()
        .filter_map(|line| parse_row(line, now_secs))
        .collect()
}

fn parse_row(line: &str, now_secs: u64) -> Option<RawPane> {
    let mut it = line.split('\t');
    let pane_id = it.next()?.to_string();
    let current_command = it.next()?.to_string();
    let cwd = PathBuf::from(it.next()?);
    let session = it.next()?;
    let window_index = it.next()?;
    let pane_index = it.next()?;
    let activity: u64 = it.next().and_then(|s| s.parse().ok()).unwrap_or(now_secs);

    let target = format!("{session}:{window_index}.{pane_index}");
    let activity_secs = now_secs.saturating_sub(activity);

    Some(RawPane {
        pane_id,
        current_command,
        cwd,
        target,
        activity_secs,
    })
}

/// Grab the last ~20 lines of a pane's visible buffer. Used for state detection.
pub fn capture_pane_tail(pane_id: &str) -> String {
    match Command::new("tmux")
        .args(["capture-pane", "-p", "-t", pane_id, "-S", "-20"])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Grab the entire current visible area of a pane (no scrollback), preserving
/// ANSI escape sequences (`-e`) so the preview can render colors.
pub fn capture_pane_visible(pane_id: &str) -> String {
    match Command::new("tmux")
        .args(["capture-pane", "-p", "-e", "-t", pane_id])
        .output()
    {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
        _ => String::new(),
    }
}

/// Switch the *attached* tmux client to a specific pane. `target` is a fully
/// qualified tmux target like `"session:window.pane"`.
pub fn switch_to_pane(target: &str) -> std::io::Result<()> {
    // Parse "session:window.pane" into its components.
    let session_window = target.split_once('.').map_or(target, |(sw, _)| sw);
    let session = target.split_once(':').map_or(target, |(s, _)| s);
    Command::new("tmux")
        .args(["switch-client", "-t", session])
        .status()?;
    Command::new("tmux")
        .args(["select-window", "-t", session_window])
        .status()?;
    Command::new("tmux")
        .args(["select-pane", "-t", target])
        .status()?;
    Ok(())
}

/// Open a new window in the current tmux session running `claude --resume <id>`.
pub fn new_window_resume(project_dir: &std::path::Path, session_id: &str) -> std::io::Result<()> {
    Command::new("tmux")
        .args([
            "new-window",
            "-c",
            &project_dir.to_string_lossy(),
            "claude",
            "--resume",
            session_id,
        ])
        .status()?;
    Ok(())
}
