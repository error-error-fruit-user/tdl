use ratatui::{
    Terminal,
    crossterm::event::{self, KeyModifiers},
    layout::{Alignment, Rect},
    prelude::Backend,
    text::ToLine,
};

const MIN_WIDTH: u16 = 32;
const MIN_HEIGHT: u16 = 20;

pub fn should_resize(width: u16, height: u16) -> bool {
    width < MIN_WIDTH || height < MIN_HEIGHT
}

pub fn run(terminal: &mut Terminal<impl Backend>) -> bool {
    loop {
        terminal
            .draw(|frame| {
                let area = frame.area();
                frame.render_widget(
                    "Text too big!".to_line().alignment(Alignment::Center),
                    Rect::new(0, area.height / 2, area.width, 1),
                );
            })
            .unwrap();
        let event = event::read().unwrap();
        if let event::Event::Resize(width, height) = event {
            if !should_resize(width, height) {
                return false;
            }
        } else if let event::Event::Key(key) = event
            && key.code == event::KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            return true;
        }
    }
}
