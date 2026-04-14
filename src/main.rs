mod claude_state;
mod git;
mod history;
mod snapshot;
mod tmux;
mod ui;
mod util;

use std::io;
use std::path::PathBuf;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::ui::picker::PickerApp;
use crate::ui::{App, ExitAction};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--snapshot-json") {
        let groups = snapshot::snapshot();
        print_snapshot_json(&groups);
        return Ok(());
    }

    // `cmux sessions` — jump straight to the Sessions tab
    if args.get(1).map(String::as_str) == Some("sessions") {
        let groups = snapshot::snapshot();
        let current_pane_id = tmux::current_pane_id();
        return run_app(groups, current_pane_id, Some("sessions"));
    }

    // `cmux new [dir...]` — directory picker mode, one tab per root
    if args.get(1).map(String::as_str) == Some("new") {
        let roots: Vec<PathBuf> = args
            .iter()
            .skip(2)
            .map(|s| PathBuf::from(shellexpand(s)))
            .collect();
        return run_picker(&roots);
    }

    // Normal mode: agent/session switcher
    let groups = snapshot::snapshot();
    let current_pane_id = tmux::current_pane_id();
    run_app(groups, current_pane_id, None)
}

fn run_app(
    groups: Vec<snapshot::Group>,
    current_pane_id: Option<String>,
    initial_tab: Option<&str>,
) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut app = App::new(groups, current_pane_id);
    if let Some(tab) = initial_tab {
        app.set_initial_tab(tab);
    }
    let run_result = app.run(&mut terminal);
    restore_terminal(&mut terminal)?;
    run_result?;

    if let Some(action) = app.exit {
        execute_action(action)?;
    }
    Ok(())
}

fn run_picker(roots: &[PathBuf]) -> io::Result<()> {
    let mut terminal = setup_terminal()?;
    let mut picker = PickerApp::new(roots);
    let run_result = picker.run(&mut terminal);
    restore_terminal(&mut terminal)?;
    run_result?;

    if let Some(dir) = picker.selected {
        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "session".to_string());
        tmux::create_or_switch_session(&name, &dir)?;
    }
    Ok(())
}

/// Expand ~ to $HOME since shell expansion doesn't happen in exec'd binaries.
fn shellexpand(s: &str) -> String {
    if let Some(rest) = s.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{rest}", home.to_string_lossy());
        }
    }
    s.to_string()
}

type Term = Terminal<CrosstermBackend<io::Stdout>>;

fn setup_terminal() -> io::Result<Term> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(stdout))
}

fn restore_terminal(terminal: &mut Term) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn execute_action(action: ExitAction) -> io::Result<()> {
    match action {
        ExitAction::SwitchTo { target } => tmux::switch_to_pane(&target),
        ExitAction::SwitchToSession { name } => {
            std::process::Command::new("tmux")
                .args(["switch-client", "-t", &name])
                .status()?;
            Ok(())
        }
        ExitAction::Resume {
            project_dir,
            session_id,
        } => tmux::new_window_resume(&project_dir, &session_id),
    }
}

fn print_snapshot_json(groups: &[crate::snapshot::Group]) {
    let arr: Vec<serde_json::Value> = groups
        .iter()
        .map(|g| {
            let panes: Vec<serde_json::Value> = g
                .panes
                .iter()
                .map(|p| {
                    serde_json::json!({
                        "target": p.target,
                        "cwd": p.cwd.display().to_string(),
                        "branch": p.git.as_ref().map(|g| g.branch.as_str()).unwrap_or(""),
                        "state": format!("{:?}", p.state),
                        "idle_s": p.idle_secs,
                    })
                })
                .collect();
            serde_json::json!({
                "repo": g.repo_name.as_deref().unwrap_or("(none)"),
                "panes": panes,
            })
        })
        .collect();
    println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
}
