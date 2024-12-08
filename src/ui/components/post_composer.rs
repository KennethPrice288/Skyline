use ratatui::{
    buffer::Buffer,
    layout::{Rect, Layout, Direction, Constraint},
    style::{Color, Style},
    widgets::{Block, Borders, Widget, StatefulWidget, Paragraph},
    text::{Line, Span},
};

const CHARACTER_LIMIT: usize = 300;

pub struct PostComposer {
    pub content: String,
    pub cursor_position: usize,
    pub reply_to: Option<String>, // URI of post being replied to
}

pub struct PostComposerState {
    pub is_active: bool,
}

impl PostComposer {
    pub fn new(reply_to: Option<String>) -> Self {
        Self {
            content: String::new(),
            cursor_position: 0,
            reply_to,
        }
    }

    pub fn insert_char(&mut self, c: char) {
        if self.content.chars().count() < CHARACTER_LIMIT {
            self.content.insert(self.cursor_position, c);
            self.cursor_position += 1;
        }
    }

    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.content.remove(self.cursor_position);
        }
    }

    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.content.len() {
            self.cursor_position += 1;
        }
    }

    pub fn clear(&mut self) {
        self.content.clear();
        self.cursor_position = 0;
    }

    pub fn get_content(&self) -> &str {
        &self.content
    }

    fn get_character_count(&self) -> usize {
        self.content.chars().count()
    }

    fn get_character_count_status(&self) -> (String, Color) {
        let count = self.get_character_count();
        let color = match count {
            0..=250 => Color::Green,
            251..=290 => Color::Yellow,
            291..=300 => Color::Red,
            _ => Color::Red,
        };
        
        (format!("{}/{}", count, CHARACTER_LIMIT), color)
    }
}

impl StatefulWidget for &PostComposer {
    type State = PostComposerState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .borders(Borders::ALL)
            .title(if self.reply_to.is_some() { "🌇 Reply" } else { "🏙️ New Post" })
            .border_style(Style::default().fg(if state.is_active { Color::Green } else { Color::White }));

        let inner_area = block.inner(area);

        // Create a layout that splits the inner area into the text area and status line
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),
                Constraint::Length(1),
            ])
            .split(inner_area);

        // Render the main block
        block.render(area, buf);

        // Render content with cursor
        let content = self.content.clone();
        let (before_cursor, after_cursor) = content.split_at(self.cursor_position);
        
        let mut spans = vec![
            Span::raw(before_cursor),
            Span::styled(
                if after_cursor.is_empty() { "_" } else { &after_cursor[..1] },
                Style::default().bg(Color::White).fg(Color::Black)
            ),
        ];

        if !after_cursor.is_empty() {
            spans.push(Span::raw(&after_cursor[1..]));
        }

        let paragraph = Paragraph::new(Line::from(spans))
            .wrap(ratatui::widgets::Wrap { trim: true });

        // Render the text area
        paragraph.render(chunks[0], buf);

        // Render character count and status line
        let (count_text, count_color) = self.get_character_count_status();
        let status_line = Line::from(vec![
            Span::raw("Press Ctrl+Enter to post, Esc to cancel | "),
            Span::styled(count_text, Style::default().fg(count_color))
        ]);
        
        Paragraph::new(status_line)
            .render(chunks[1], buf);
    }
}
