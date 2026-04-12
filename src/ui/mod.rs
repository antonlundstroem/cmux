pub mod filter;
pub mod live;
pub mod preview;
pub mod resume;

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Tabs};

use crate::snapshot::Group;
use crate::ui::filter::Filter;
use crate::ui::live::LiveView;
use crate::ui::preview::Preview;
use crate::ui::resume::ResumeView;

/// Below this total width, the preview column is hidden.
const PREVIEW_MIN_WIDTH: u16 = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Live,
    Resume,
}

#[derive(Debug, Clone)]
pub enum ExitAction {
    SwitchTo { target: String },
    Resume {
        project_dir: std::path::PathBuf,
        session_id: String,
    },
}

pub struct App {
    groups: Vec<Group>,
    tab: Tab,
    query: String,
    filter: Filter,
    live: LiveView,
    resume: ResumeView,
    preview: Preview,
    pub exit: Option<ExitAction>,
}

impl App {
    pub fn new(groups: Vec<Group>) -> Self {
        let live = LiveView::new(&groups);
        let mut app = Self {
            groups,
            tab: Tab::Live,
            query: String::new(),
            filter: Filter::new(),
            live,
            resume: ResumeView::new(),
            preview: Preview::new(),
            exit: None,
        };
        app.live
            .rebuild(&app.groups, &app.query, &mut app.filter);
        app
    }

    pub fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> std::io::Result<()> {
        const TICK: Duration = Duration::from_millis(500);
        loop {
            terminal.draw(|f| self.draw(f))?;
            if event::poll(TICK)? {
                if let Event::Key(key) = event::read()? {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    if self.handle_key(key) {
                        return Ok(());
                    }
                }
            } else {
                // Tick: re-capture the currently selected pane so the
                // preview stays live.
                self.refresh_preview();
            }
        }
    }

    fn refresh_preview(&mut self) {
        if self.tab != Tab::Live {
            return;
        }
        if let Some(pane) = self.live.selected_pane(&self.groups) {
            self.preview.refresh(&pane.pane_id.clone());
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

        // Tabs
        let pane_count = self.groups.iter().map(|g| g.panes.len()).sum::<usize>();
        let titles: Vec<Line> = vec![
            Line::from(format!(" Live ({pane_count}) ")),
            Line::from(format!(" Resume ({}) ", self.resume.len())),
        ];
        let tabs = Tabs::new(titles)
            .select(match self.tab {
                Tab::Live => 0,
                Tab::Resume => 1,
            })
            .divider(" ")
            .highlight_style(
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
        f.render_widget(tabs, chunks[0]);

        // Split: list on left, preview on right (if wide enough).
        let body = chunks[1];
        let (list_area, preview_area) = if body.width >= PREVIEW_MIN_WIDTH {
            let cols = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(body);
            (cols[0], Some(cols[1]))
        } else {
            (body, None)
        };

        match self.tab {
            Tab::Live => {
                self.live.render(f, list_area, &self.groups);
                if let Some(pa) = preview_area {
                    let selected_pane = self.live.selected_pane(&self.groups);
                    self.preview.render_live(f, pa, selected_pane);
                }
            }
            Tab::Resume => {
                self.resume.render(f, list_area);
                if let Some(pa) = preview_area {
                    let selected_sess = self.resume.selected();
                    self.preview.render_resume(f, pa, selected_sess);
                }
            }
        }

        // Filter input
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
            Paragraph::new(" \u{21b5} switch   tab view   / filter   q quit ")
                .style(Style::default().fg(Color::DarkGray)),
            chunks[3],
        );
    }

    /// Returns true when the app should exit.
    fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            return true;
        }

        match key.code {
            KeyCode::Esc => {
                if self.query.is_empty() {
                    return true;
                }
                self.query.clear();
                self.rebuild_visible();
            }
            KeyCode::Tab => {
                self.tab = match self.tab {
                    Tab::Live => {
                        self.resume.ensure_loaded();
                        self.resume.rebuild(&self.query, &mut self.filter);
                        Tab::Resume
                    }
                    Tab::Resume => Tab::Live,
                };
            }
            KeyCode::Enter => {
                self.commit_selection();
                return true;
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
                self.rebuild_visible();
            }
            // 'q' quits only when there's no active filter input.
            KeyCode::Char('q') if self.query.is_empty() => return true,
            KeyCode::Char(c) => {
                self.query.push(c);
                self.rebuild_visible();
            }
            _ => {}
        }
        false
    }

    fn move_selection(&mut self, delta: i32) {
        match self.tab {
            Tab::Live => self.live.move_selection(delta),
            Tab::Resume => self.resume.move_selection(delta),
        }
    }

    fn rebuild_visible(&mut self) {
        match self.tab {
            Tab::Live => self
                .live
                .rebuild(&self.groups, &self.query, &mut self.filter),
            Tab::Resume => self.resume.rebuild(&self.query, &mut self.filter),
        }
    }

    fn commit_selection(&mut self) {
        match self.tab {
            Tab::Live => {
                if let Some(pane) = self.live.selected_pane(&self.groups) {
                    self.exit = Some(ExitAction::SwitchTo {
                        target: pane.target.clone(),
                    });
                }
            }
            Tab::Resume => {
                if let Some(sess) = self.resume.selected() {
                    self.exit = Some(ExitAction::Resume {
                        project_dir: sess.project_dir.clone(),
                        session_id: sess.id.clone(),
                    });
                }
            }
        }
    }
}
