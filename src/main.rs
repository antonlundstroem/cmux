mod claude_state;
mod git;
mod history;
mod snapshot;
mod tmux;
mod ui;
mod util;

use std::io;

use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::ui::{App, ExitAction};

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // Debug escape hatch: `ctui --snapshot-json` prints the snapshot and exits
    // without touching the terminal. Useful for timing the cold path.
    if args.iter().any(|a| a == "--snapshot-json") {
        let groups = snapshot::snapshot();
        print_snapshot_json(&groups);
        return Ok(());
    }

    let groups = snapshot::snapshot();

    let mut terminal = setup_terminal()?;
    let mut app = App::new(groups);
    let run_result = app.run(&mut terminal);
    restore_terminal(&mut terminal)?;
    run_result?;

    // Execute the post-TUI action outside of raw mode. `tmux switch-client`
    // needs to talk to the outer client, which is only safe once our popup's
    // terminal state has been fully restored.
    if let Some(action) = app.exit {
        execute_action(action)?;
    }
    Ok(())
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
