use std::io;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::ui::filter::Filter;
use crate::util;

struct RootView {
    root: PathBuf,
    dirs: Vec<PathBuf>,
    haystacks: Vec<String>,
    visible: Vec<usize>,
    state: ListState,
    query: String,
}

impl RootView {
    fn new(root: PathBuf) -> Self {
        let dirs = scan_dirs(&root, 3);
        let haystacks: Vec<String> = dirs.iter().map(|d| util::shorten_home(d)).collect();
        let visible: Vec<usize> = (0..dirs.len()).collect();
        let mut state = ListState::default();
        if !visible.is_empty() {
            state.select(Some(0));
        }
        Self {
            root,
            dirs,
            haystacks,
            visible,
            state,
            query: String::new(),
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.visible.is_empty() {
            return;
        }
        let len = self.visible.len() as i32;
        let cur = self.state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(len) as usize;
        self.state.select(Some(next));
    }

    fn rebuild(&mut self, filter: &mut Filter) {
        let ranked = filter.rank(&self.query, self.haystacks.iter().map(String::as_str));
        self.visible = if self.query.is_empty() {
            (0..self.dirs.len()).collect()
        } else {
            ranked
        };
        if self.visible.is_empty() {
            self.state.select(None);
        } else {
            let sel = self
                .state
                .selected()
                .unwrap_or(0)
                .min(self.visible.len() - 1);
            self.state.select(Some(sel));
        }
    }
}

pub struct PickerApp {
    views: Vec<RootView>,
    active: usize,
    filter: Filter,
    pub selected: Option<PathBuf>,
}

impl PickerApp {
    pub fn new(roots: &[PathBuf]) -> Self {
        let roots: Vec<PathBuf> = if roots.is_empty() {
            vec![PathBuf::from(".")]
        } else {
            roots.to_vec()
        };
        let views: Vec<RootView> = roots.into_iter().map(RootView::new).collect();
        Self {
            views,
            active: 0,
            filter: Filter::new(),
            selected: None,
        }
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        loop {
            terminal.draw(|f| self.draw(f))?;
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && key.code == KeyCode::Char('c')
                {
                    return Ok(());
                }
                match key.code {
                    KeyCode::Tab => self.cycle_root(1),
                    KeyCode::BackTab => self.cycle_root(-1),
                    KeyCode::Esc => {
                        let view = &mut self.views[self.active];
                        if view.query.is_empty() {
                            return Ok(());
                        }
                        view.query.clear();
                        view.rebuild(&mut self.filter);
                    }
                    KeyCode::Enter => {
                        let view = &self.views[self.active];
                        if let Some(i) = view.state.selected() {
                            if let Some(src) = view.visible.get(i) {
                                self.selected = Some(view.dirs[*src].clone());
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Up => self.views[self.active].move_selection(-1),
                    KeyCode::Down => self.views[self.active].move_selection(1),
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.views[self.active].move_selection(-1)
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.views[self.active].move_selection(1)
                    }
                    KeyCode::Backspace => {
                        let view = &mut self.views[self.active];
                        view.query.pop();
                        view.rebuild(&mut self.filter);
                    }
                    KeyCode::Char('q') if self.views[self.active].query.is_empty() => {
                        return Ok(())
                    }
                    KeyCode::Char(c) => {
                        let view = &mut self.views[self.active];
                        view.query.push(c);
                        view.rebuild(&mut self.filter);
                    }
                    _ => {}
                }
            }
        }
    }

    fn cycle_root(&mut self, delta: i32) {
        let len = self.views.len() as i32;
        if len <= 1 {
            return;
        }
        let next = (self.active as i32 + delta).rem_euclid(len) as usize;
        self.active = next;
    }

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let tab_bar_height = if self.views.len() > 1 { 1 } else { 0 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(tab_bar_height),
                Constraint::Min(1),
                Constraint::Length(2),
                Constraint::Length(1),
            ])
            .split(area);

        f.render_widget(
            Paragraph::new(" New session")
                .style(Style::default().add_modifier(Modifier::BOLD)),
            chunks[0],
        );

        if self.views.len() > 1 {
            let mut spans: Vec<Span> = vec![Span::raw(" ")];
            for (i, view) in self.views.iter().enumerate() {
                if i > 0 {
                    spans.push(Span::styled(
                        " │ ",
                        Style::default().fg(Color::DarkGray),
                    ));
                }
                let label = util::shorten_home(&view.root);
                let style = if i == self.active {
                    Style::default()
                        .bg(Color::Indexed(236))
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::DarkGray)
                };
                spans.push(Span::styled(label, style));
            }
            f.render_widget(Paragraph::new(Line::from(spans)), chunks[1]);
        }

        let view = &mut self.views[self.active];

        let items: Vec<ListItem> = view
            .visible
            .iter()
            .filter_map(|i| view.haystacks.get(*i))
            .map(|s| ListItem::new(format!("  {s}")))
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▌ ");

        f.render_stateful_widget(list, chunks[2], &mut view.state);

        let (filter_text, filter_style) = if view.query.is_empty() {
            (
                "  / to filter".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            (
                format!(" > {}_", view.query),
                Style::default().fg(Color::White),
            )
        };
        f.render_widget(
            Paragraph::new(filter_text)
                .style(filter_style)
                .block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
            chunks[3],
        );

        let footer = if self.views.len() > 1 {
            " \u{21b5} create   \u{21e5} next root   esc cancel   q quit "
        } else {
            " \u{21b5} create   esc cancel   q quit "
        };
        f.render_widget(
            Paragraph::new(footer).style(Style::default().fg(Color::DarkGray)),
            chunks[4],
        );
    }
}

fn scan_dirs(root: &Path, max_depth: u32) -> Vec<PathBuf> {
    let mut result = Vec::new();
    scan_recursive(root, max_depth, &mut result);
    result.sort();
    result
}

/// Returns true if `dir` looks like a bare git repo (has HEAD file + objects/ + refs/).
fn is_bare_git_repo(dir: &Path) -> bool {
    dir.join("HEAD").is_file() && dir.join("objects").is_dir() && dir.join("refs").is_dir()
}

/// Git-internal directory names that live inside a bare repo root.
const BARE_REPO_INTERNALS: &[&str] = &[
    "hooks", "info", "objects", "refs", "branches", "logs", "modules",
];

fn scan_recursive(dir: &Path, depth: u32, result: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let bare = is_bare_git_repo(dir);
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        // Skip hidden directories.
        if name.starts_with('.') {
            continue;
        }
        // Skip git-internal directories inside bare repos.
        if bare && BARE_REPO_INTERNALS.contains(&name) {
            continue;
        }
        result.push(path.clone());
        if depth > 1 {
            scan_recursive(&path, depth - 1, result);
        }
    }
}
