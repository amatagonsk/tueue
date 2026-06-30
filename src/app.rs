use std::{
    process::Command,
    time::{Duration, Instant},
};

use ansi_to_tui::IntoText;
use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseEventKind};
use fast_strip_ansi::*;
use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Layout, Margin, Position, Rect},
    style::{Color, Style},
    widgets::{Block, Clear, Paragraph, Wrap},
};
use tui_scrollbar::{
    ScrollBar, ScrollBarArrows, ScrollBarInteraction, ScrollCommand, ScrollLengths,
};

use crate::ui;

#[derive(Debug, Clone, Copy)]
struct LayoutState {
    content: Rect,
    vertical_bar: Rect,
    horizontal_bar: Rect,
}

pub struct App {
    is_show_popup: bool,
    last_tick: Instant,
    command_output: Vec<u8>,
    pueue_args: String,

    input: String,
    character_index: usize,
    input_mode: InputMode,

    pub vertical_scroll: usize,
    pub horizontal_scroll: usize,
    vertical_interaction: ScrollBarInteraction,
    horizontal_interaction: ScrollBarInteraction,
    layout: Option<LayoutState>,
    vertical_content_len: usize,
    horizontal_content_len: usize,
}

#[derive(PartialEq, Eq)]
enum InputMode {
    Normal,
    Editing,
}

impl App {
    pub fn new(pueue_args: String) -> Self {
        Self {
            is_show_popup: false,
            last_tick: Instant::now(),
            command_output: Vec::new(),
            input: pueue_args.clone(),
            pueue_args,
            character_index: 0,
            input_mode: InputMode::Normal,
            vertical_scroll: 0,
            horizontal_scroll: 0,
            vertical_interaction: ScrollBarInteraction::new(),
            horizontal_interaction: ScrollBarInteraction::new(),
            layout: None,
            vertical_content_len: 0,
            horizontal_content_len: 0,
        }
    }

    const TICK_RATE: Duration = Duration::from_millis(5_000);
    const SCROLL_STEP: i8 = 20;
    pub fn run(&mut self, mut terminal: DefaultTerminal) -> Result<()> {
        self.run_command();
        loop {
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => match self.input_mode {
                        InputMode::Normal => match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                            KeyCode::Char('i') => self.toggle_popup(),

                            KeyCode::Char('j') | KeyCode::Down => self.scroll_down(None),
                            KeyCode::Char('k') | KeyCode::Up => self.scroll_up(None),
                            KeyCode::PageDown => self.scroll_down(Some(Self::SCROLL_STEP)),
                            KeyCode::PageUp => self.scroll_up(Some(Self::SCROLL_STEP)),

                            KeyCode::Char('h') | KeyCode::Left => self.scroll_left(None),
                            KeyCode::Char('l') | KeyCode::Right => self.scroll_right(None),
                            KeyCode::Home => self.scroll_left(Some(Self::SCROLL_STEP)),
                            KeyCode::End => self.scroll_right(Some(Self::SCROLL_STEP)),
                            _ => {}
                        },
                        InputMode::Editing => match key.code {
                            KeyCode::Enter => self.submit_pueue(),
                            KeyCode::Char(to_insert) => self.enter_char(to_insert),
                            KeyCode::Backspace => self.backspace_char(),
                            KeyCode::Delete => self.delete_char(),
                            KeyCode::Left => self.move_cursor_left(),
                            KeyCode::Right => self.move_cursor_right(),
                            KeyCode::Home => self.move_cursor_leftest(),
                            KeyCode::End => self.move_cursor_rightest(),
                            KeyCode::Esc => self.toggle_popup(),
                            _ => {}
                        },
                    },
                    Event::Mouse(mouse) => self.handle_mouse_event(mouse),
                    _ => (),
                }
            }
            if self.last_tick.elapsed() > Self::TICK_RATE && !self.is_show_popup {
                self.run_command();
            }
            terminal.draw(|frame| self.draw(frame))?;
        }
    }

    fn scroll_down(&mut self, scroll_val: Option<i8>) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_add(scroll_val.unwrap_or(1) as usize);
    }

    fn scroll_up(&mut self, scroll_val: Option<i8>) {
        self.vertical_scroll = self
            .vertical_scroll
            .saturating_sub(scroll_val.unwrap_or(1) as usize);
    }

    fn scroll_left(&mut self, scroll_val: Option<i8>) {
        self.horizontal_scroll = self
            .horizontal_scroll
            .saturating_sub(scroll_val.unwrap_or(1) as usize);
    }

    fn scroll_right(&mut self, scroll_val: Option<i8>) {
        self.horizontal_scroll = self
            .horizontal_scroll
            .saturating_add(scroll_val.unwrap_or(1) as usize);
    }

    fn handle_mouse_event(&mut self, event: crossterm::event::MouseEvent) {
        let Some(layout) = self.layout else {
            return;
        };

        let ctrl_pressed = event.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl_pressed {
            match event.kind {
                MouseEventKind::ScrollDown => {
                    self.scroll_right(None);
                    return;
                }
                MouseEventKind::ScrollUp => {
                    self.scroll_left(None);
                    return;
                }
                _ => {}
            }
        }

        let v_viewport = layout.content.height as usize;
        let h_viewport = layout.content.width as usize;

        let v_lengths = ScrollLengths {
            content_len: self.vertical_content_len.max(1),
            viewport_len: v_viewport.max(1),
        };
        let v_scrollbar = ScrollBar::vertical(v_lengths).offset(self.vertical_scroll);

        if let Some(ScrollCommand::SetOffset(offset)) = v_scrollbar.handle_mouse_event(
            layout.vertical_bar,
            event,
            &mut self.vertical_interaction,
        ) {
            self.vertical_scroll = offset;
        }

        let h_lengths = ScrollLengths {
            content_len: self.horizontal_content_len.max(1),
            viewport_len: h_viewport.max(1),
        };
        let h_scrollbar = ScrollBar::horizontal(h_lengths).offset(self.horizontal_scroll);

        if let Some(ScrollCommand::SetOffset(offset)) = h_scrollbar.handle_mouse_event(
            layout.horizontal_bar,
            event,
            &mut self.horizontal_interaction,
        ) {
            self.horizontal_scroll = offset;
        }
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
        self.is_show_popup = !self.is_show_popup
    }

    fn move_cursor_left(&mut self) {
        let cursor_moved_left = self.character_index.saturating_sub(1);
        self.character_index = self.clamp_cursor(cursor_moved_left);
    }

    fn move_cursor_right(&mut self) {
        let cursor_moved_right = self.character_index.saturating_add(1);
        self.character_index = self.clamp_cursor(cursor_moved_right);
    }

    fn move_cursor_leftest(&mut self) {
        self.character_index = self.clamp_cursor(0);
    }

    fn move_cursor_rightest(&mut self) {
        self.character_index = self.clamp_cursor(self.input.len());
    }

    fn enter_char(&mut self, new_char: char) {
        let index = self.byte_index();
        self.input.insert(index, new_char);
        self.move_cursor_right();
    }

    fn byte_index(&self) -> usize {
        self.input
            .char_indices()
            .map(|(i, _)| i)
            .nth(self.character_index)
            .unwrap_or(self.input.len())
    }

    fn backspace_char(&mut self) {
        let is_not_cursor_leftmost = self.character_index != 0;
        if is_not_cursor_leftmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index - 1;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
            self.move_cursor_left();
        }
    }

    fn delete_char(&mut self) {
        let is_not_cursor_rightmost = self.character_index != (self.input.len());
        if is_not_cursor_rightmost {
            let current_index = self.character_index;
            let from_left_to_current_index = current_index;

            let before_char_to_delete = self.input.chars().take(from_left_to_current_index);
            let after_char_to_delete = self.input.chars().skip(current_index + 1);

            self.input = before_char_to_delete.chain(after_char_to_delete).collect();
        }
    }

    fn clamp_cursor(&self, new_cursor_pos: usize) -> usize {
        new_cursor_pos.clamp(0, self.input.chars().count())
    }

    fn run_command(&mut self) {
        self.last_tick = Instant::now();
        let output = if cfg!(target_os = "windows") {
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

        self.command_output = if output.status.success() {
            output.stdout
        } else {
            output.stderr
        };

        self.vertical_content_len = str::from_utf8(&self.command_output)
            .unwrap()
            .lines()
            .count();
        self.horizontal_content_len =
            strip_ansi_string(str::from_utf8(&self.command_output).unwrap())
                .lines()
                .map(|line| line.chars().count())
                .max()
                .unwrap_or(1);
    }

    fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let vertical = Layout::vertical([
            Constraint::Length(1),
            Constraint::Percentage(99),
            Constraint::Length(1),
        ]);
        let [instructions, content_row, bar_row] = vertical.areas(area);

        let [content, vertical_bar] = content_row.layout(&Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));
        let [horizontal_bar, _corner] = bar_row.layout(&Layout::horizontal([
            Constraint::Fill(1),
            Constraint::Length(1),
        ]));

        let text = if self.is_show_popup {
            "<esc> to close"
        } else {
            "<i> :input / <q>, <Esc> :exit / <j> <k> , ▲ ▼ :scroll"
        };
        let paragraph = Paragraph::new(text).centered().wrap(Wrap { trim: true });
        frame.render_widget(paragraph, instructions);

        let block = Paragraph::new(self.command_output.into_text().unwrap())
            .scroll((self.vertical_scroll as u16, self.horizontal_scroll as u16));
        frame.render_widget(block, content);

        let h_bar_inner = horizontal_bar.inner(Margin {
            vertical: 0,
            horizontal: 1,
        });
        self.layout = Some(LayoutState {
            content,
            vertical_bar,
            horizontal_bar: h_bar_inner,
        });

        let v_viewport = content.height as usize;
        let h_viewport = content.width as usize;

        let v_lengths = ScrollLengths {
            content_len: self.vertical_content_len.max(1),
            viewport_len: v_viewport.max(1),
        };
        let v_scrollbar = ScrollBar::vertical(v_lengths)
            .offset(self.vertical_scroll)
            .arrows(ScrollBarArrows::Both);
        frame.render_widget(&v_scrollbar, vertical_bar);

        let h_lengths = ScrollLengths {
            content_len: self.horizontal_content_len.max(1),
            viewport_len: h_viewport.max(1),
        };
        let h_scrollbar = ScrollBar::horizontal(h_lengths)
            .offset(self.horizontal_scroll)
            .arrows(ScrollBarArrows::Both);
        frame.render_widget(&h_scrollbar, h_bar_inner);

        if self.is_show_popup {
            let input = Paragraph::new(self.input.as_str()).block(
                Block::bordered()
                    .title(" pueue status $args_input ")
                    .border_style(Style::default().fg(Color::LightMagenta)),
            );

            let area = ui::popup_area(area, 60);
            frame.render_widget(Clear, area);
            frame.render_widget(input, area);

            match self.input_mode {
                InputMode::Normal => {}

                #[allow(clippy::cast_possible_truncation)]
                InputMode::Editing => frame.set_cursor_position(Position::new(
                    area.x + self.character_index as u16 + 1,
                    area.y + 1,
                )),
            }
        }
    }
}
