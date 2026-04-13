use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};

use crate::snapshot::{Group, LivePane};
use crate::ui::filter::Filter;
use crate::util;

#[derive(Debug, Clone, Copy)]
enum RowKind {
    Header,
    Pane { group_idx: usize, pane_idx: usize },
}

pub struct LiveView {
    rows: Vec<RowKind>,
    state: ListState,
    /// Pre-computed per-pane haystack strings for fuzzy matching, built once
    /// from the snapshot. Index is flat across all groups/panes; `coords`
    /// maps back to (group_idx, pane_idx).
    haystacks: Vec<String>,
    coords: Vec<(usize, usize)>,
}

impl LiveView {
    pub fn new(groups: &[Group]) -> Self {
        let (haystacks, coords) = build_haystacks(groups);
        Self {
            rows: Vec::new(),
            state: ListState::default(),
            haystacks,
            coords,
        }
    }

    pub fn selected_pane<'a>(&self, groups: &'a [Group]) -> Option<&'a LivePane> {
        let idx = self.state.selected()?;
        match self.rows.get(idx)? {
            RowKind::Pane { group_idx, pane_idx } => {
                groups.get(*group_idx)?.panes.get(*pane_idx)
            }
            RowKind::Header => None,
        }
    }

    pub fn move_selection(&mut self, delta: i32) {
        if self.rows.is_empty() {
            self.state.select(None);
            return;
        }
        let current = self.state.selected().unwrap_or(0) as i32;
        let mut next = current;
        for _ in 0..self.rows.len() {
            next = (next + delta).rem_euclid(self.rows.len() as i32);
            if matches!(self.rows[next as usize], RowKind::Pane { .. }) {
                break;
            }
        }
        self.state.select(Some(next as usize));
    }

    fn ensure_valid_selection(&mut self) {
        if self.rows.is_empty() {
            self.state.select(None);
            return;
        }
        let needs_reset = match self.state.selected() {
            None => true,
            Some(i) if i >= self.rows.len() => true,
            Some(i) => matches!(self.rows[i], RowKind::Header),
        };
        if needs_reset {
            let first_pane = self
                .rows
                .iter()
                .position(|r| matches!(r, RowKind::Pane { .. }));
            self.state.select(first_pane);
        }
    }

    /// Recompute visible rows from the cached haystacks and a filter query.
    pub fn rebuild(&mut self, groups: &[Group], query: &str, filter: &mut Filter) {
        self.rows.clear();

        let ranked = filter.rank(query, self.haystacks.iter().map(String::as_str));
        // Preserve group ordering from the snapshot (not filter ranking) so the
        // grouped layout stays coherent.
        let mut matched = vec![false; self.haystacks.len()];
        for i in ranked {
            matched[i] = true;
        }

        for (gi, g) in groups.iter().enumerate() {
            let visible: Vec<usize> = g
                .panes
                .iter()
                .enumerate()
                .filter(|(pi, _)| {
                    self.coords
                        .iter()
                        .position(|c| *c == (gi, *pi))
                        .is_some_and(|flat| matched[flat])
                })
                .map(|(pi, _)| pi)
                .collect();
            if visible.is_empty() {
                continue;
            }
            if g.repo_name.is_some() {
                self.rows.push(RowKind::Header);
            }
            for pi in visible {
                self.rows.push(RowKind::Pane {
                    group_idx: gi,
                    pane_idx: pi,
                });
            }
        }

        self.ensure_valid_selection();
    }

    pub fn render(
        &mut self,
        f: &mut Frame,
        area: Rect,
        groups: &[Group],
        current_pane_id: Option<&str>,
    ) {
        let items: Vec<ListItem> = self
            .rows
            .iter()
            .enumerate()
            .map(|(idx, row)| match row {
                RowKind::Header => {
                    let group_idx = self.rows[idx + 1..].iter().find_map(|r| match r {
                        RowKind::Pane { group_idx, .. } => Some(*group_idx),
                        RowKind::Header => None,
                    });
                    let g = group_idx.and_then(|i| groups.get(i));
                    let name = g
                        .and_then(|g| g.repo_name.as_deref())
                        .unwrap_or("?");
                    let count = g.map(|g| g.panes.len()).unwrap_or(0);
                    ListItem::new(Line::from(vec![
                        Span::styled("▾ ", Style::default().fg(Color::DarkGray)),
                        Span::styled(
                            name.to_string(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" ({count})"),
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]))
                }
                RowKind::Pane {
                    group_idx,
                    pane_idx,
                } => {
                    let Some(p) =
                        groups.get(*group_idx).and_then(|g| g.panes.get(*pane_idx))
                    else {
                        return ListItem::new("?");
                    };
                    render_pane_row(
                        p,
                        groups[*group_idx].repo_name.is_some(),
                        current_pane_id,
                    )
                }
            })
            .collect();

        let list = List::new(items)
            .block(Block::default().borders(Borders::NONE))
            .highlight_style(
                Style::default()
                    .bg(Color::Indexed(236))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol("▌ ");

        f.render_stateful_widget(list, area, &mut self.state);
    }
}

fn build_haystacks(groups: &[Group]) -> (Vec<String>, Vec<(usize, usize)>) {
    let mut haystacks = Vec::new();
    let mut coords = Vec::new();
    for (gi, g) in groups.iter().enumerate() {
        for (pi, p) in g.panes.iter().enumerate() {
            let repo = g.repo_name.as_deref().unwrap_or("");
            let branch = p.git.as_ref().map(|g| g.branch.as_str()).unwrap_or("");
            let worktree = p
                .git
                .as_ref()
                .and_then(|g| g.worktree_path.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            let agent = p.agent_kind.name();
            haystacks.push(format!(
                "{agent} {repo} {worktree} {branch} {} {}",
                p.cwd.display(),
                p.target
            ));
            coords.push((gi, pi));
        }
    }
    (haystacks, coords)
}

fn render_pane_row(
    p: &LivePane,
    in_group: bool,
    current_pane_id: Option<&str>,
) -> ListItem<'static> {
    let marker = if current_pane_id == Some(p.pane_id.as_str()) {
        Span::styled("▸", Style::default().fg(Color::Magenta))
    } else {
        Span::raw(" ")
    };

    // Fixed indent so all columns align regardless of grouping.
    let indent = if in_group { "  " } else { "  " };

    let label = match &p.git {
        Some(gi) => gi.branch.clone(),
        None => util::shorten_home(&p.cwd),
    };

    let mut indicators = String::new();
    if let Some(gi) = &p.git {
        if gi.dirty {
            indicators.push('*');
        }
        if gi.is_worktree {
            indicators.push('⚘');
        }
        if gi.ahead > 0 {
            indicators.push_str(&format!("↑{}", gi.ahead));
        }
        if gi.behind > 0 {
            indicators.push_str(&format!("↓{}", gi.behind));
        }
    }

    let branch_with_flags = if indicators.is_empty() {
        label.clone()
    } else {
        format!("{label} {indicators}")
    };

    let spans = vec![
        Span::raw(indent),
        marker,
        Span::styled(p.agent_kind.badge(), Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} ", p.state.glyph()),
            Style::default().fg(p.state.color()),
        ),
        Span::styled(rpad(&branch_with_flags, 28), Style::default().fg(Color::Cyan)),
        Span::styled(rpad(&p.target, 20), Style::default().fg(Color::DarkGray)),
    ];

    ListItem::new(Line::from(spans))
}

fn rpad(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        let mut out: String = s.chars().take(width.saturating_sub(1)).collect();
        out.push('…');
        out
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}
