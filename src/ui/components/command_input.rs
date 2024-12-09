use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, StatefulWidget, Widget},
    text::{Line, Span},
};

use std::collections::HashSet;

#[derive(Default)]
pub struct TabCompletion {
    suggestions: Vec<String>,
    current_index: Option<usize>,
    partial_command: String,
}

impl TabCompletion {
    fn new() -> Self {
        Self {
            suggestions: Vec::new(),
            current_index: None,
            partial_command: String::new(),
        }
    }

    fn update_suggestions(&mut self, input: &str, commands: &HashSet<&str>) {
        self.partial_command = input.to_string();
        self.suggestions = commands
            .iter()
            .filter(|cmd| cmd.starts_with(input))
            .map(|&cmd| cmd.to_string())
            .collect();
        self.suggestions.sort();
        self.current_index = if self.suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn next_suggestion(&mut self) -> Option<&str> {
        if let Some(index) = self.current_index {
            let suggestion = &self.suggestions[index];
            self.current_index = Some((index + 1) % self.suggestions.len());
            Some(suggestion)
        } else {
            None
        }
    }
}

pub struct CommandInputState {
    pub is_active: bool,
}

pub struct CommandInput {
    pub content: String,
    pub cursor_position: usize,
    pub command_history: Vec<String>,
    pub history_position: Option<usize>,
    commands: HashSet<&'static str>,
    tab_completion: TabCompletion,
}

impl CommandInput {
    pub fn new() -> Self {
        let mut commands = HashSet::new();
        commands.insert("post");
        commands.insert("reply");
        commands.insert("refresh");
        commands.insert("notifications");
        commands.insert("timeline");
        commands.insert("profile");
        // commands.insert("help");
        // commands.insert("search");
        // commands.insert("block");
        // commands.insert("mute");
        commands.insert("delete");

        Self {
            content: String::new(),
            cursor_position: 0,
            command_history: Vec::new(),
            history_position: None,
            commands,
            tab_completion: TabCompletion::new(),
        }
    }

    pub fn handle_tab(&mut self) {
        // Get the current word being typed
        let input = self.get_current_word().to_lowercase();
        
        // If this is the first tab, update suggestions
        if self.tab_completion.partial_command != input {
            self.tab_completion.update_suggestions(&input, &self.commands);
        }
        
        // Get next suggestion
        if let Some(suggestion) = self.tab_completion.next_suggestion() {
            // Replace current word with suggestion
            let (before, _) = self.content.split_at(self.cursor_position - input.len());
            self.content = format!("{}{}", before, suggestion);
            self.cursor_position = self.content.len();
        }
    }

    fn get_current_word(&self) -> String {
        let before_cursor = &self.content[..self.cursor_position];
        before_cursor
            .split_whitespace()
            .last()
            .unwrap_or("")
            .to_string()
    }

    pub fn insert_char(&mut self, c: char) {
        self.content.insert(self.cursor_position, c);
        self.cursor_position += 1;
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
        self.history_position = None;
    }

    pub fn history_up(&mut self) {
        if self.command_history.is_empty() {
            return;
        }

        let new_pos = match self.history_position {
            None => Some(self.command_history.len() - 1),
            Some(0) => Some(0),
            Some(pos) => Some(pos - 1),
        };

        if let Some(pos) = new_pos {
            self.content = self.command_history[pos].clone();
            self.cursor_position = self.content.len();
            self.history_position = Some(pos);
        }
    }

    pub fn history_down(&mut self) {
        if let Some(current_pos) = self.history_position {
            if current_pos < self.command_history.len() - 1 {
                let new_pos = current_pos + 1;
                self.content = self.command_history[new_pos].clone();
                self.cursor_position = self.content.len();
                self.history_position = Some(new_pos);
            } else {
                self.clear();
            }
        }
    }

    pub fn submit_command(&mut self) -> Option<String> {
        if !self.content.is_empty() {
            let command = self.content.clone();
            self.command_history.push(command.clone());
            self.clear();
            Some(command)
        } else {
            None
        }
    }
}

impl StatefulWidget for &CommandInput {
    type State = CommandInputState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(if state.is_active { Color::Yellow } else { Color::White }));

        let inner_area = block.inner(area);
        
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

        // Prefix with ':'
        let line = Line::from(vec![
            Span::styled(":", Style::default().fg(Color::Yellow)),
            Span::raw(" "),
        ]);
        buf.set_line(inner_area.x, inner_area.y, &line, inner_area.width);

        // Render the command text after the prefix
        let content_line = Line::from(spans);
        buf.set_line(inner_area.x + 2, inner_area.y, &content_line, inner_area.width - 2);
    }
}
