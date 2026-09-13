use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders},
};

pub const TITLE_COLOR: Color = Color::LightYellow;

pub fn new_window<'a>(
    width: u16,
    height: u16,
    title: &'a str,
    border_color: Color,
    area: Rect,
) -> (Block<'a>, Rect, Rect) {
    let title = Line::from(vec![
        Span::styled("[", Style::new().fg(border_color)),
        Span::styled(format!(" {title} "), Style::new().fg(TITLE_COLOR)),
        Span::styled("]", Style::new().fg(border_color)),
    ]);
    let window = Block::new()
        .borders(Borders::all())
        .border_type(ratatui::widgets::BorderType::Thick)
        .border_style(Style::new().fg(border_color))
        .title(title)
        .title_alignment(Alignment::Center);
    let window_area = new_window_area(width, height, area);
    let question_area = Rect::new(
        window_area.x,
        window_area.bottom() + 1,
        window_area.width,
        1,
    );
    (window, window_area, question_area)
}

pub fn new_window_area(width: u16, height: u16, area: Rect) -> Rect {
    Rect::new(
        area.width / 2 - width / 2,
        area.height / 2 - height / 2,
        width,
        height,
    )
}
