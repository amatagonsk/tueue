use std::{
    io::stdout,
    process::Command,
    time::{Duration, Instant},
};

use ansi_to_tui::IntoText;
use color_eyre::Result;
use crossterm::{
    ExecutableCommand,
    event::{self, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind},
};
use fast_strip_ansi::*;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Flex, Layout, Margin, Position, Rect},
    style::{Color, Style},
    symbols::scrollbar::Set,
    widgets::{Block, Clear, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

fn main() -> Result<()> {
    color_eyre::install()?;
    stdout().execute(EnableMouseCapture)?;
    let terminal = ratatui::init();
    let app_result = App::new().run(terminal);
    ratatui::restore();
    app_result
}

pub struct App {
    show_popup: bool,
    last_tick: Instant,
    command_output: Vec<u8>,
    is_windows: bool,
    pueue_args: String,

    /// Current value of the input box
    input: String,
    /// Position of cursor in the editor area.
    character_index: usize,
    /// Current input mode
    input_mode: InputMode,

    pub vertical_scroll_state: ScrollbarState,
    pub horizontal_scroll_state: ScrollbarState,
    pub vertical_scroll: usize,
    pub horizontal_scroll: usize,
}

#[derive(PartialEq, Eq)]
enum InputMode {
    Normal,
    Editing,
}

impl App {
    fn new() -> Self {
        Self {
            show_popup: false,
            last_tick: Instant::now(),
            command_output: Vec::new(),
            is_windows: if cfg!(target_os = "windows") {
                true
            } else {
                false
            },
            pueue_args: String::new(),
            input: String::new(),
            character_index: 0,
            input_mode: InputMode::Normal,
            vertical_scroll_state: ScrollbarState::new(0),
            vertical_scroll: 0,
            horizontal_scroll_state: ScrollbarState::new(0),
            horizontal_scroll: 0,
        }
    }

    const TICK_RATE: Duration = Duration::from_millis(5_000);
    fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.run_command();
        loop {
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) => {
                        if key.kind == KeyEventKind::Press {
                            match self.input_mode {
                                InputMode::Normal => match key.code {
                                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                                    KeyCode::Char('i') => self.toggle_popup(),

                                    KeyCode::Char('j') | KeyCode::Down => self.scroll_down(None),
                                    KeyCode::Char('k') | KeyCode::Up => self.scroll_up(None),
                                    KeyCode::PageDown => self.scroll_down(Some(20)),
                                    KeyCode::PageUp => self.scroll_up(Some(20)),

                                    KeyCode::Char('h') | KeyCode::Left => self.scroll_left(None),
                                    KeyCode::Char('l') | KeyCode::Right => self.scroll_right(None),
                                    KeyCode::Home => self.scroll_left(Some(20)),
                                    KeyCode::End => self.scroll_right(Some(20)),
                                    _ => {}
                                },
                                InputMode::Editing => match key.code {
                                    KeyCode::Enter => self.submit_pueue(),
                                    KeyCode::Char(to_insert) => self.enter_char(to_insert),
                                    KeyCode::Backspace => self.delete_char(),
                                    KeyCode::Left => self.move_cursor_left(),
                                    KeyCode::Right => self.move_cursor_right(),
                                    KeyCode::Esc => self.toggle_popup(),
                                    _ => {}
                                },
                            }
                        }
                    }
                    Event::Mouse(mouse) => {
                        let ctrl_pressed = mouse.modifiers.contains(KeyModifiers::CONTROL);
                        match mouse.kind {
                            MouseEventKind::ScrollDown => {
                                if ctrl_pressed {
                                    self.scroll_right(None)
                                } else {
                                    self.scroll_down(None)
                                }
                            }
                            MouseEventKind::ScrollUp => {
                                if ctrl_pressed {
                                    self.scroll_left(None)
                                } else {
                                    self.scroll_up(None)
                                }
                            }

                            // mouse tilt not work
                            MouseEventKind::ScrollLeft => self.scroll_left(None),
                            MouseEventKind::ScrollRight => self.scroll_right(None),
                            _ => {}
                        }
                    }
                    _ => (),
                }
            }
            if self.last_tick.elapsed() > Self::TICK_RATE && self.show_popup == false {
                self.run_command();
            }
            terminal.draw(|frame| self.draw(frame))?;
        }
    }

    fn scroll_down(&mut self, scroll_val: Option<i8>) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_add(scroll_val.unwrap_or(1) as usize);
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
    }

    fn scroll_up(&mut self, scroll_val: Option<i8>) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_sub(scroll_val.unwrap_or(1) as usize);
        self.vertical_scroll_state = self.vertical_scroll_state.position(self.vertical_scroll);
    }

    fn scroll_left(&mut self, scroll_val: Option<i8>) {
        self.horizontal_scroll = self
            .horizontal_scroll
            .saturating_sub(scroll_val.unwrap_or(1) as usize);
        self.horizontal_scroll_state = self
            .horizontal_scroll_state
            .position(self.horizontal_scroll);
    }

    fn scroll_right(&mut self, scroll_val: Option<i8>) {
        self.horizontal_scroll = self
            .horizontal_scroll
            .saturating_add(scroll_val.unwrap_or(1) as usize);
        self.horizontal_scroll_state = self
            .horizontal_scroll_state
            .position(self.horizontal_scroll);
    }

    fn submit_pueue(&mut self) {
        self.pueue_args = self.input.clone();
        self.run_command();
        self.toggle_popup()
    }

    fn toggle_popup(&mut self) {
        self.input_mode = if self.input_mode == InputMode::Normal {
            InputMode::Editing
        } else {
            InputMode::Normal
        };
        self.show_popup = !self.show_popup
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    /// Returns the byte index based on the character position.
    ///
    /// Since each character in a string can be contain multiple bytes, it's necessary to calculate
    /// the byte index based on the index of the character.
    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn delete_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            // Method "remove" is not used on the saved text for deleting the selected char.
            // Reason: Using remove on String works on bytes instead of the chars.
            // Using remove would require special care because of char boundaries.

            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            // Getting all characters before the selected character.
            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            // Getting all characters after selected character.
            let after_char_to_delete = self.input.chars().skip(current_index);

            // Put all characters together except the selected one.
            // By leaving the selected one out, it is forgotten and therefore deleted.
            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn run_command(&mut self) {
        self.last_tick = Instant::now();
        let output = if self.is_windows {
            Command::new("cmd")
                .args([
                    "/C",
                    &format!("pueue --color always status {}", self.pueue_args),
                ])
                .output()
                .expect("failed to execute process")
        } else {
            Command::new("sh")
                .args([
                    "-c",
                    &format!("pueue --color always status {}", self.pueue_args),
                ])
                .output()
                .expect("failed to execute process")
        };

        self.command_output = output.stdout;
        self.vertical_scroll_state = self.vertical_scroll_state.content_length(
            String::from_utf8(self.command_output.clone())
                .unwrap()
                .lines()
                .count(),
        );
        self.horizontal_scroll_state = self.horizontal_scroll_state.content_length(
            strip_ansi_string(str::from_utf8(&self.command_output).unwrap())
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap(),
        );
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Percentage(99),
            Constraint::Length(1),
        ]);
        let [instructions, content, horizontal_bar] = vertical.areas(area);

        let text = if self.show_popup {
            "<esc> to close"
        } else {
            "<i> :input / <q>, <Esc> :exit / <j> <k> , ▲ ▼ :scroll"
        };
        let paragraph = Paragraph::new(text).centered().wrap(Wrap { trim: true });
        frame.render_widget(paragraph, instructions);

        let block = Paragraph::new(self.command_output.into_text().unwrap())
            .scroll((self.vertical_scroll as u16, self.horizontal_scroll as u16));
        frame.render_widget(block, content);

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("▲"))
                .end_symbol(Some("▼")),
            content,
            &mut self.vertical_scroll_state,
        );

        frame.render_stateful_widget(
            Scrollbar::new(ScrollbarOrientation::HorizontalBottom).symbols(Set {
                track: "═",
                thumb: "━",
                begin: "⯇ ",
                end: " ⯈",
            }),
            horizontal_bar.inner(Margin {
                vertical: 0,
                horizontal: 1,
            }),
            &mut self.horizontal_scroll_state,
        );

        if self.show_popup {
            let input = Paragraph::new(self.input.as_str()).block(
                Block::bordered()
                    .title(" pueue status $args_input ")
                    .border_style(Style::default().fg(Color::LightMagenta)),
            );

            let area = popup_area(area, 60);
            frame.render_widget(Clear, area); //this clears out the background
            frame.render_widget(input, area);

            match self.input_mode {
                // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
                InputMode::Normal => {}

                // Make the cursor visible and ask ratatui to put it at the specified coordinates after
                // rendering
                #[allow(clippy::cast_possible_truncation)]
                InputMode::Editing => frame.set_cursor_position(Position::new(
                    // Draw the cursor at the current position in the input field.
                    // This position is can be controlled via the left and right arrow key
                    area.x + self.character_index as u16 + 1,
                    // Move one line down, from the border to the input line
                    area.y + 1,
                )),
            }
        }
    }
}

/// helper function to create a centered rect using up certain percentage of the available rect `r`
fn popup_area(area: Rect, percent_x: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Length(3)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}
