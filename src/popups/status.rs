use ratatui::{style::Color, text::ToLine};

use crate::{
    error::Result,
    ritual::{self, Grimoire, Haunted},
};

const WIDTH: u16 = 32;
const HEIGHT: u16 = 5;

/// Draws a one-off message window, for work that blocks (hosting, joining, downloading).
pub fn summon(
    grimoire: &mut Grimoire,
    title: &str,
    message: &str,
    border_color: Color,
) -> Result<()> {
    grimoire.bewitch(|page| {
        let (window, mut window_area, ..) =
            ritual::conjure_window(title, WIDTH, HEIGHT, border_color, page.area());
        page.render_widget(window, window_area);
        window_area.y += 2;
        page.render_widget(message.to_line().centered(), window_area);
    })?;
    Ok(())
}
