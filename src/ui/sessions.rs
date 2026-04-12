use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};

use crate::tmux::{self, TmuxSession};
use crate::ui::filter::Filter;

pub struct SessionsView {
    sessions: Vec<TmuxSession>,
    haystacks: Vec<String>,
    visible: Vec<usize>,
    state: ListState,
    loaded: bool,
}

impl SessionsView {
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
        self.sessions = tmux::list_sessions();
        self.haystacks = self
            .sessions
            .iter()
            .map(|s| s.name.clone())
            .collect();
        self.visible = (0..self.sessions.len()).collect();
        if !self.visible.is_empty() {
            self.state.select(Some(0));
        }
        self.loaded = true;
    }

    pub fn selected(&self) -> Option<&TmuxSession> {
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
        let items: Vec<ListItem> = self
            .visible
            .iter()
            .filter_map(|i| self.sessions.get(*i))
            .map(|s| {
                let attached = if s.attached { " (attached)" } else { "" };
                ListItem::new(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        s.name.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {} windows{attached}", s.window_count),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
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

    pub fn render_preview(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(s) = self.selected() else {
            f.render_widget(
                Paragraph::new("(no selection)").style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        };

        let attached = if s.attached { "yes" } else { "no" };
        let lines = vec![
            Line::from(Span::styled(
                s.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("windows: ", Style::default().fg(Color::DarkGray)),
                Span::raw(s.window_count.to_string()),
            ]),
            Line::from(vec![
                Span::styled("attached: ", Style::default().fg(Color::DarkGray)),
                Span::raw(attached),
            ]),
        ];

        f.render_widget(Paragraph::new(lines), inner);
    }
}
