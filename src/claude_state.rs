use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentKind {
    Claude,
    Cursor,
}

impl AgentKind {
    pub fn badge(self) -> &'static str {
        match self {
            AgentKind::Claude => "[c] ",
            AgentKind::Cursor => "[a] ",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            AgentKind::Claude => "claude",
            AgentKind::Cursor => "cursor",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClaudeState {
    Idle,
    Running,
    WaitingPermission,
    Unknown,
}

impl ClaudeState {
    pub fn glyph(self) -> &'static str {
        match self {
            ClaudeState::Running => "●",
            ClaudeState::WaitingPermission => "◐",
            ClaudeState::Idle => "○",
            ClaudeState::Unknown => "·",
        }
    }

    pub fn color(self) -> Color {
        match self {
            ClaudeState::Running => Color::Green,
            ClaudeState::WaitingPermission => Color::Yellow,
            ClaudeState::Idle => Color::Blue,
            ClaudeState::Unknown => Color::DarkGray,
        }
    }
}

/// Try hook-based state file first, fall back to TUI scraping.
///
/// Hook files are written by the claude-code wrapper at
/// `$XDG_RUNTIME_DIR/claude-agents/<pane_id>.state`. Format is either
/// `status=WORK|WAIT|IDLE` key-value lines or bare `WORK`/`WAIT`/`IDLE` words.
pub fn detect_with_hooks(pane_id: &str, tail: &str, kind: AgentKind) -> ClaudeState {
    match kind {
        AgentKind::Claude => {
            if let Some(state) = read_hook_state(pane_id) {
                return state;
            }
            detect(tail)
        }
        AgentKind::Cursor => detect_cursor(tail),
    }
}

fn read_hook_state(pane_id: &str) -> Option<ClaudeState> {
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").ok()?;
    let path = std::path::PathBuf::from(runtime_dir)
        .join("claude-agents")
        .join(format!("{pane_id}.state"));
    let content = std::fs::read_to_string(path).ok()?;
    for line in content.lines() {
        let word = if let Some(val) = line.strip_prefix("status=") {
            val.trim()
        } else {
            line.trim()
        };
        match word {
            "WORK" => return Some(ClaudeState::Running),
            "WAIT" => return Some(ClaudeState::WaitingPermission),
            "IDLE" => return Some(ClaudeState::Idle),
            _ => {}
        }
    }
    None
}

fn detect(tail: &str) -> ClaudeState {
    // Running has highest precedence — "esc to interrupt" only appears while
    // the assistant is actively working.
    if tail.contains("esc to interrupt") {
        return ClaudeState::Running;
    }
    // Permission prompts show a "Do you want to" header with numbered options.
    if tail.contains("Do you want to") || tail.contains("❯ 1.") {
        return ClaudeState::WaitingPermission;
    }
    // The idle input footer.
    if tail.contains("? for shortcuts") {
        return ClaudeState::Idle;
    }
    ClaudeState::Unknown
}

/// Cursor Agent TUI scraping. Patterns derived from observed cursor-agent output.
fn detect_cursor(tail: &str) -> ClaudeState {
    if tail.contains("Generating") {
        return ClaudeState::Running;
    }
    if tail.contains("/ commands") {
        return ClaudeState::Idle;
    }
    ClaudeState::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_wins_over_idle() {
        // A pane can technically contain both strings if the idle footer is
        // still in the scrollback. Running wins.
        let tail = "? for shortcuts\n... (esc to interrupt)";
        assert_eq!(detect(tail), ClaudeState::Running);
    }

    #[test]
    fn idle_detected() {
        let tail = "╭──────────────╮\n│ > _          │\n╰──────────────╯\n? for shortcuts";
        assert_eq!(detect(tail), ClaudeState::Idle);
    }

    #[test]
    fn permission_detected() {
        let tail = "Do you want to run this command?\n❯ 1. Yes\n  2. No";
        assert_eq!(detect(tail), ClaudeState::WaitingPermission);
    }

    #[test]
    fn empty_is_unknown() {
        assert_eq!(detect(""), ClaudeState::Unknown);
    }

    #[test]
    fn cursor_generating_is_running() {
        let tail = "Generating...";
        assert_eq!(detect_cursor(tail), ClaudeState::Running);
    }

    #[test]
    fn cursor_idle_detected() {
        let tail = "  Codex 5.3\n  / commands · @ files · ! shell";
        assert_eq!(detect_cursor(tail), ClaudeState::Idle);
    }

    #[test]
    fn cursor_empty_is_unknown() {
        assert_eq!(detect_cursor(""), ClaudeState::Unknown);
    }
}
