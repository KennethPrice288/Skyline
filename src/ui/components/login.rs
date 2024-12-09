use ratatui::{
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

pub struct LoginView {
    pub username: Option<String>,
    pub password_mode: bool,
    pub error: Option<String>,
    pub loading: bool,
}

impl LoginView {
    pub fn new() -> Self {
        Self {
            username: None,
            password_mode: false,
            error: None,
            loading: false,
        }
    }
}

impl Widget for &LoginView {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title("🌆 Welcome to Skyline");

        let inner_area = block.inner(area);
        block.render(area, buf);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Logo
                Constraint::Length(2),  // Status
                Constraint::Min(1),     // Content
            ])
            .split(inner_area);

        // Logo/Title
        let title = vec![
            Line::from(vec![
                Span::styled("Sky", Style::default().fg(Color::Cyan)),
                Span::styled("line", Style::default().fg(Color::White)),
            ]),
            Line::from(Span::styled(
                "A terminal client for Bluesky",
                Style::default().fg(Color::Gray),
            )),
        ];
        Paragraph::new(title).render(chunks[0], buf);

        // Login status/error
        let status = if self.loading {
            vec![Line::from(Span::styled(
                "Logging in...",
                Style::default().fg(Color::Yellow),
            ))]
        } else if let Some(error) = &self.error {
            vec![Line::from(Span::styled(
                error,
                Style::default().fg(Color::Red),
            ))]
        } else if self.password_mode {
            vec![Line::from(vec![
                Span::raw("Enter password for "),
                Span::styled(self.username.clone().unwrap(), Style::default().fg(Color::Cyan)),
                Span::raw(" (input is hidden)"),
            ])]
        } else {
            vec![Line::from(Span::raw(
                "Use :login username to begin",
            ))]
        };
        
        Paragraph::new(status).render(chunks[1], buf);
    }
}
