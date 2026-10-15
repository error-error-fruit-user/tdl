use std::io::Stdout;

use ratatui::{
    crossterm::event::{self, KeyCode, KeyModifiers},
    layout::{Alignment, Position, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style, Stylize},
    text::{Line, Span, ToLine, ToSpan},
    widgets::{Block, BorderType, Borders},
};

use crate::{
    error::{Error, Result},
    resize,
};

pub type Terminal = ratatui::Terminal<CrosstermBackend<Stdout>>;
const TITLE_COLOR: Color = Color::LightYellow;
const STATUS_WIDTH: u16 = 32;
const STATUS_HEIGHT: u16 = 5;

pub fn new_window<'a>(
    title: &'a str,
    width: u16,
    height: u16,
    border_color: Color,
    area: Rect,
) -> (Block<'a>, Rect, Line<'a>, Rect) {
    let title = [
        Span::styled("[", Style::new().fg(border_color)),
        Span::styled(format!(" {title} "), Style::new().fg(TITLE_COLOR)),
        Span::styled("]", Style::new().fg(border_color)),
    ]
    .into_iter()
    .collect::<Line>();

    let window = Block::new()
        .borders(Borders::all())
        .border_type(BorderType::Thick)
        .border_style(Style::new().fg(border_color))
        .title(title)
        .title_alignment(Alignment::Center);
    let window_area = new_window_area(width, height, area);
    let indicator = ["[".to_span(), "?".to_span().light_yellow(), "]".to_span()]
        .into_iter()
        .collect::<Line>()
        .centered();
    let indicator_area = Rect::new(
        window_area.x,
        window_area.bottom() + 1,
        window_area.width,
        1,
    );
    (window, window_area, indicator, indicator_area)
}

pub fn new_window_area(width: u16, height: u16, area: Rect) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect::new(
        (area.width - width) / 2,
        (area.height - height) / 2,
        width,
        height,
    )
}

pub fn show_status(
    terminal: &mut Terminal,
    title: &str,
    message: &str,
    border_color: Color,
) -> Result<()> {
    terminal.draw(|frame| {
        let (window, mut window_area, ..) = new_window(
            title,
            STATUS_WIDTH,
            STATUS_HEIGHT,
            border_color,
            frame.area(),
        );
        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget(message.to_line().centered(), window_area);
    })?;
    Ok(())
}

pub fn new_query<'a>(
    label: &'a str,
    input: &'a str,
    frame_width: u16,
    y: u16,
) -> (Line<'a>, Position) {
    let line = [
        Span::from(label).light_yellow(),
        ": ".to_span(),
        Span::from(input),
    ]
    .into_iter()
    .collect::<Line>()
    .centered();

    let position = Position::new(
        (frame_width + (label.len() + input.len()) as u16).div_ceil(2),
        y,
    );

    (line, position)
}

#[must_use]
#[derive(Eq, PartialEq)]
pub enum Phase<T> {
    Render,
    Prompt,
    Return(T),
}

pub fn prompt<T>(
    terminal: &mut Terminal,
    prompt: &mut impl FnMut(&mut Terminal, KeyCode) -> Result<Phase<T>>,
) -> Result<Phase<T>> {
    let event = event::read()?;
    if let event::Event::Resize(width, height) = event {
        if resize::should_resize(width, height) {
            resize::run(terminal)?;
        }
        Ok(Phase::Render)
    } else if let event::Event::Key(key) = event
        && key.is_press()
    {
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            Err(Error::Exit)
        } else {
            prompt(terminal, key.code)
        }
    } else {
        Ok(Phase::Prompt)
    }
}
