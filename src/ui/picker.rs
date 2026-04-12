use std::io;
use std::path::{Path, PathBuf};

use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::ui::filter::Filter;
use crate::util;

pub struct PickerApp {
    dirs: Vec<PathBuf>,
    haystacks: Vec<String>,
    visible: Vec<usize>,
    state: ListState,
    filter: Filter,
    query: String,
    pub selected: Option<PathBuf>,
}

impl PickerApp {
    pub fn new(root: &Path) -> Self {
        let dirs = scan_dirs(root, 3);
        let haystacks: Vec<String> = dirs
            .iter()
            .map(|d| util::shorten_home(d))
            .collect();
        let visible: Vec<usize> = (0..dirs.len()).collect();
        let mut state = ListState::default();
        if !visible.is_empty() {
            state.select(Some(0));
        }
        Self {
            dirs,
            haystacks,
            visible,
            state,
            filter: Filter::new(),
            query: String::new(),
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
                    KeyCode::Esc => {
                        if self.query.is_empty() {
                            return Ok(());
                        }
                        self.query.clear();
                        self.rebuild();
                    }
                    KeyCode::Enter => {
                        if let Some(i) = self.state.selected() {
                            if let Some(src) = self.visible.get(i) {
                                self.selected = Some(self.dirs[*src].clone());
                            }
                        }
                        return Ok(());
                    }
                    KeyCode::Up => self.move_selection(-1),
                    KeyCode::Down => self.move_selection(1),
                    KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.move_selection(-1)
                    }
                    KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                        self.move_selection(1)
                    }
                    KeyCode::Backspace => {
                        self.query.pop();
                        self.rebuild();
                    }
                    KeyCode::Char('q') if self.query.is_empty() => return Ok(()),
                    KeyCode::Char(c) => {
                        self.query.push(c);
                        self.rebuild();
                    }
                    _ => {}
                }
            }
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

    fn rebuild(&mut self) {
        let ranked = self
            .filter
            .rank(&self.query, self.haystacks.iter().map(String::as_str));
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

    fn draw(&mut self, f: &mut Frame) {
        let area = f.area();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
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

        let items: Vec<ListItem> = self
            .visible
            .iter()
            .filter_map(|i| self.haystacks.get(*i))
            .map(|s| ListItem::new(format!("  {s}")))
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▌ ");

        f.render_stateful_widget(list, chunks[1], &mut self.state);

        let (filter_text, filter_style) = if self.query.is_empty() {
            (
                "  / to filter".to_string(),
                Style::default().fg(Color::DarkGray),
            )
        } else {
            (
                format!(" > {}_", self.query),
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
            chunks[2],
        );

        f.render_widget(
            Paragraph::new(" \u{21b5} create   esc cancel   q quit ")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[3],
        );
    }
}

fn scan_dirs(root: &Path, max_depth: u32) -> Vec<PathBuf> {
    let mut result = Vec::new();
    scan_recursive(root, max_depth, &mut result);
    result.sort();
    result
}

fn scan_recursive(dir: &Path, depth: u32, result: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        // Skip hidden directories.
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') {
                continue;
            }
        }
        result.push(path.clone());
        if depth > 1 {
            scan_recursive(&path, depth - 1, result);
        }
    }
}
