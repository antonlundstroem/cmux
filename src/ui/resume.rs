use ratatui::prelude::*;
use ratatui::widgets::{List, ListItem, ListState};

use crate::history::ResumableSession;
use crate::ui::filter::Filter;
use crate::util;

pub struct ResumeView {
    sessions: Vec<ResumableSession>,
    /// Pre-built haystack strings for fuzzy matching, one per session.
    haystacks: Vec<String>,
    /// Visible session indices after filtering.
    visible: Vec<usize>,
    state: ListState,
    loaded: bool,
}

impl ResumeView {
    pub fn new() -> Self {
        Self {
            sessions: Vec::new(),
            haystacks: Vec::new(),
            visible: Vec::new(),
            state: ListState::default(),
            loaded: false,
        }
    }

    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    pub fn ensure_loaded(&mut self) {
        if self.loaded {
            return;
        }
        self.sessions = crate::history::load();
        self.haystacks = self
            .sessions
            .iter()
            .map(|s| format!("{} {}", s.project_dir.display(), s.first_user_msg))
            .collect();
        self.visible = (0..self.sessions.len()).collect();
        if !self.visible.is_empty() {
            self.state.select(Some(0));
        }
        self.loaded = true;
    }

    pub fn selected(&self) -> Option<&ResumableSession> {
        let i = self.state.selected()?;
        let src = *self.visible.get(i)?;
        self.sessions.get(src)
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.visible.is_empty() {
            self.state.select(None);
            return;
        }
        let len = self.visible.len() as i32;
        let cur = self.state.selected().unwrap_or(0) as i32;
        let next = (cur + delta).rem_euclid(len) as usize;
        self.state.select(Some(next));
    }

    pub fn rebuild(&mut self, query: &str, filter: &mut Filter) {
        let ranked = filter.rank(query, self.haystacks.iter().map(String::as_str));
        self.visible = if query.is_empty() {
            (0..self.sessions.len()).collect()
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

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let now = std::time::SystemTime::now();
        let items: Vec<ListItem> = self
            .visible
            .iter()
            .filter_map(|i| self.sessions.get(*i))
            .map(|s| {
                let dir = util::shorten_home(&s.project_dir);
                let age = util::fmt_age(now, s.last_modified);
                let short = crate::history::truncate(&s.first_user_msg, 80);
                ListItem::new(vec![
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(dir, Style::default().add_modifier(Modifier::BOLD)),
                        Span::raw("  "),
                        Span::styled(age, Style::default().fg(Color::DarkGray)),
                    ]),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(
                            format!("\"{short}\""),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]),
                ])
            })
            .collect();

        let list = List::new(items)
            .highlight_style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▌ ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}
