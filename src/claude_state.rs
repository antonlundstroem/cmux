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

/// Inspect the last ~20 lines of a claude pane and guess its current state.
///
/// These are brittle string matches against the stock claude TUI. The cost of
/// a miss is low: we fall back to `Unknown` and render a neutral glyph.
pub fn detect(tail: &str) -> ClaudeState {
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
