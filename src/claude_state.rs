use ratatui::style::Color;

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
pub fn detect_with_hooks(pane_id: &str, tail: &str) -> ClaudeState {
    if let Some(state) = read_hook_state(pane_id) {
        return state;
    }
    detect(tail)
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
}
