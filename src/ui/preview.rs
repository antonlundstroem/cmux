use std::collections::HashMap;

use ansi_to_tui::IntoText;
use ratatui::prelude::*;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::history::ResumableSession;
use crate::snapshot::LivePane;
use crate::tmux;
use crate::util;

pub struct Preview {
    cache: HashMap<String, Text<'static>>,
}

impl Preview {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn capture(&mut self, pane_id: &str) {
        let raw = tmux::capture_pane_visible(pane_id);
        let text = raw
            .into_text()
            .unwrap_or_else(|_| Text::raw(raw.clone()));
        self.cache.insert(pane_id.to_string(), text);
    }

    fn ensure_cached(&mut self, pane_id: &str) {
        if !self.cache.contains_key(pane_id) {
            self.capture(pane_id);
        }
    }

    /// Re-capture the given pane, replacing the cached version.
    pub fn refresh(&mut self, pane_id: &str) {
        self.capture(pane_id);
    }

    pub fn render_live(&mut self, f: &mut Frame, area: Rect, pane: Option<&LivePane>) {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(p) = pane else {
            f.render_widget(
                Paragraph::new("(no selection)").style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        };

        self.ensure_cached(&p.pane_id);

        // Split the inner area: header (3-4 lines) + captured pane text.
        let header_height = if p.git.is_some() { 4 } else { 3 };
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(header_height),
                Constraint::Min(0),
            ])
            .split(inner);

        // Header
        let mut header_lines: Vec<Line> = vec![
            Line::from(vec![
                Span::styled(p.state.glyph(), Style::default().fg(p.state.color())),
                Span::raw(" "),
                Span::styled(
                    p.target.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("cwd: ", Style::default().fg(Color::DarkGray)),
                Span::raw(util::shorten_home(&p.cwd)),
            ]),
        ];
        if let Some(gi) = &p.git {
            header_lines.push(Line::from(vec![
                Span::styled("branch: ", Style::default().fg(Color::DarkGray)),
                Span::styled(gi.branch.clone(), Style::default().fg(Color::Cyan)),
            ]));
        }
        header_lines.push(Line::from(""));
        f.render_widget(Paragraph::new(header_lines), chunks[0]);

        if let Some(text) = self.cache.get(&p.pane_id) {
            let content_height = text.lines.len() as u16;
            let visible_height = chunks[1].height;
            let scroll = content_height.saturating_sub(visible_height);
            f.render_widget(
                Paragraph::new(text.clone()).scroll((scroll, 0)),
                chunks[1],
            );
        }
    }

    pub fn render_resume(&self, f: &mut Frame, area: Rect, session: Option<&ResumableSession>) {
        let block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray));
        let inner = block.inner(area);
        f.render_widget(block, area);

        let Some(s) = session else {
            f.render_widget(
                Paragraph::new("(no selection)").style(Style::default().fg(Color::DarkGray)),
                inner,
            );
            return;
        };

        let now = std::time::SystemTime::now();
        let lines = vec![
            Line::from(Span::styled(
                util::shorten_home(&s.project_dir),
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled("session: ", Style::default().fg(Color::DarkGray)),
                Span::raw(s.id.clone()),
            ]),
            Line::from(vec![
                Span::styled("age: ", Style::default().fg(Color::DarkGray)),
                Span::raw(util::fmt_age(now, s.last_modified)),
            ]),
            Line::from(""),
            Line::from(Span::styled(
                "first prompt:",
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(s.first_user_msg.clone()),
        ];

        f.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: false }),
            inner,
        );
    }
}
