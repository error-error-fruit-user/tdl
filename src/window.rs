use std::io::Stdout;

use ratatui::{
    Terminal,
    crossterm::event::{self, KeyModifiers},
    layout::{Alignment, Rect},
    prelude::CrosstermBackend,
    style::{Color, Style},
    text::{Line, Span, ToLine},
    widgets::{Block, BorderType, Borders},
};

use crate::{
    error::{Error, Result},
    resize,
};

pub const TITLE_COLOR: Color = Color::LightYellow;

pub fn new<'a>(
    width: u16,
    height: u16,
    title: &'a str,
    border_color: Color,
    area: Rect,
) -> (Block<'a>, Rect, Line<'a>, Rect) {
    let title = Line::from(vec![
        Span::styled("[", Style::new().fg(border_color)),
        Span::styled(format!(" {title} "), Style::new().fg(TITLE_COLOR)),
        Span::styled("]", Style::new().fg(border_color)),
    ]);
    let window = Block::new()
        .borders(Borders::all())
        .border_type(BorderType::Thick)
        .border_style(Style::new().fg(border_color))
        .title(title)
        .title_alignment(Alignment::Center);
    let window_area = new_area(width, height, area);
    let indicator = "[?]".to_line().alignment(Alignment::Center);
    let indicator_area = Rect::new(
        window_area.x,
        window_area.bottom() + 1,
        window_area.width,
        1,
    );
    (window, window_area, indicator, indicator_area)
}

pub fn new_area(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.width / 2 - width / 2,
        area.height / 2 - height / 2,
        width,
        height,
    )
}

pub fn new_input<'a>(message: &'a str, input: &'a str) -> Line<'a> {
    Line::from(vec![
        Span::from(message).style(Style::new().fg(Color::LightYellow)),
        Span::from(": "),
        Span::from(input),
    ])
    .alignment(Alignment::Center)
}

#[must_use]
#[derive(Eq, PartialEq)]
pub enum Phase<T> {
    Render,
    Prompt,
    Return(T),
}

pub fn process<T>(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
    prompt: &mut impl FnMut(&mut Terminal<CrosstermBackend<Stdout>>, event::KeyCode) -> Result<Phase<T>>,
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
        if key.code == event::KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            Err(Error::Exit)
        } else {
            prompt(terminal, key.code)
        }
    } else {
        Ok(Phase::Prompt)
    }
}

#[macro_export]
macro_rules! dispatch {
    [$expr:expr] => {
        match $expr {
            Phase::Render => break,
            Phase::Prompt => (),
            Phase::Return(x) => return Ok(x),
        }
    };
}

#[macro_export]
macro_rules! render {
    [$terminal:expr, $widget:expr] => {
        $terminal.draw(|frame| frame.render_widget($widget, frame.area()))?;
    };
    [$terminal:expr, $widget:expr, $state:expr] => {
        $terminal.draw(|frame| frame.render_stateful_widget($widget, frame.area(), $state))?;
    };
}

#[macro_export]
macro_rules! resolve {
    [$terminal:expr, $window:expr] => {
        loop {
            crate::dispatch![$window.prompt($terminal)?];
        }
    };
    [$terminal:expr, $window:expr, $state:expr] => {
        loop {
            crate::dispatch![$window.prompt($terminal, $state)?]
        }
    };
}

#[macro_export]
macro_rules! present {
    [$terminal:expr, $window:expr] => {
        loop {
            crate::render![$terminal, &$window];
            crate::resolve![$terminal, $window];
        }
    };
    [$terminal:expr, $window:expr, $state:expr] => {
        loop {
            crate::render![$terminal, &$window, $state];
            crate::resolve![$terminal, $window, $state];
        }
    };
}

#[macro_export]
macro_rules! present_with {
    [$terminal:expr, $window:expr, $renderer:expr] => {
        loop {
            $terminal.draw(|frame| {
                $renderer(frame);
                frame.render_widget(&$window);
            })?;
            crate::resolve![$terminal, $window];
        }
    };
    [$terminal:expr, $window:expr, $state:expr, $renderer:expr] => {
        loop {
            $terminal.draw(|frame| {
                $renderer(frame);
                frame.render_stateful_widget(&$window, frame.area(), $state);
            })?;
            crate::resolve![$terminal, $window, $state];
        }
    };
}

#[macro_export]
macro_rules! stage {
    [$terminal:expr, $renderer:expr, $prompt:expr] => {
        loop {
            $terminal.draw($renderer)?;
            loop {
                crate::dispatch![crate::window::process($terminal, $prompt)?];
            }
        }
    };
}
