//! What the terminal shows while UZDoom runs, so a slow start never looks like tdl has frozen: a
//! spinner and a clock that keep moving, and the last thing UZDoom printed, which shows its
//! startup getting along.

use std::time::{Duration, Instant};

use ratatui::{
    crossterm::event::{self, Event, KeyCode, KeyModifiers},
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
};

use crate::{
    error::{Error, Result},
    layout, quips,
    ritual::{self, Grimoire, Haunted, Page},
    theme,
    uzdoom::Game,
};

/// How often the window redraws while it waits.
const FRAME: Duration = Duration::from_millis(100);
/// The spinner, one frame per [`FRAME`].
const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
/// How wide the window is, so it doesn't change size as UZDoom's lines come and go.
const WIDTH: u16 = 50;
const HINT: &str = "quit UZDoom to come back · Ctrl+C stops it";

/// Waits for `game` to quit, redrawing the window as it goes. `details` are extra lines to keep
/// on screen, such as the join code a host has to pass on. Keys pressed meanwhile are thrown away,
/// since the game has the keyboard, except Ctrl+C, which stops the game and then tdl, as it does
/// anywhere else.
pub fn summon(
    grimoire: &mut Grimoire,
    title: &str,
    details: &[String],
    mut game: Game,
) -> Result<()> {
    let started = Instant::now();
    let seed = quips::seed();
    loop {
        if game.has_quit()? {
            return game.wait();
        }
        let waiting = Waiting {
            title,
            details,
            latest: game.latest_line(),
            elapsed: started.elapsed(),
            seed,
            show_quip: quips::shown(),
        };
        grimoire.bewitch(|page| waiting.draw(page))?;
        if event::poll(FRAME)?
            && let Event::Key(key) = event::read()?
            && key.is_press()
            && key.code == KeyCode::Char('c')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            game.stop()?;
            return Err(Error::Interrupted);
        }
    }
}

/// One frame of the window.
struct Waiting<'a> {
    title: &'a str,
    details: &'a [String],
    /// The last line UZDoom printed, empty until it prints one.
    latest: String,
    elapsed: Duration,
    seed: u32,
    /// Whether there is a quip at all, which there isn't with the quips hidden.
    show_quip: bool,
}

impl Waiting<'_> {
    /// The spinner and what UZDoom is up to, then its latest line, any details, a quip, and the
    /// hint, with blank rows between the groups.
    fn draw(&self, page: &mut Page) {
        let frame = (self.elapsed.as_millis() / FRAME.as_millis()) as usize % SPINNER.len();
        let seconds = self.elapsed.as_secs();
        let state = if self.latest.is_empty() {
            "Starting UZDoom"
        } else {
            "UZDoom is running"
        };
        let mut lines = vec![
            Line::styled(
                format!(
                    "{} {state} · {}:{:02}",
                    SPINNER[frame],
                    seconds / 60,
                    seconds % 60
                ),
                Style::new().fg(theme::TEXT).add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                shorten(&self.latest, WIDTH as usize - 4),
                Style::new().fg(theme::MUTED),
            ),
        ];
        if !self.details.is_empty() {
            lines.push(Line::raw(""));
            lines.extend(
                self.details
                    .iter()
                    .map(|detail| Line::styled(detail.clone(), Style::new().fg(theme::TEXT))),
            );
        }
        if self.show_quip {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                quips::cycling(self.seed, self.elapsed),
                Style::new().fg(theme::QUIP).add_modifier(Modifier::ITALIC),
            ));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(HINT, Style::new().fg(theme::MUTED)));

        let (width, height) = layout::window(WIDTH, lines.len() as u16 + 2);
        let (window, window_area, ..) =
            ritual::conjure_window(self.title, width, height, theme::BUSY, page.area());
        let inner = window.inner(window_area);
        page.render_widget(window, window_area);
        for (i, line) in lines.into_iter().enumerate() {
            let row =
                Rect::new(inner.x, inner.y + 1 + i as u16, inner.width, 1).intersection(inner);
            page.render_widget(line.centered(), row);
        }
    }
}

/// `text` cut to at most `width` characters, with `…` where it was cut.
fn shorten(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    text.chars()
        .take(width.saturating_sub(1))
        .chain(['…'])
        .collect()
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    #[test]
    fn snapshot_hosting_while_uzdoom_starts_up() {
        let details = ["Players join with port 8080, code abc234".to_string()];
        let waiting = Waiting {
            title: "hosting",
            details: &details,
            latest: "W_Init: Init WADfiles.".into(),
            elapsed: Duration::from_secs(75),
            seed: 0,
            show_quip: true,
        };
        assert_snapshot!(testing::draw(60, 16, |page| waiting.draw(page)));
    }

    #[test]
    fn with_the_quips_hidden_there_is_no_quip_or_room_for_one() {
        let waiting = |show_quip| Waiting {
            title: "launching",
            details: &[],
            latest: "W_Init: Init WADfiles.".into(),
            elapsed: Duration::ZERO,
            seed: 0,
            show_quip,
        };
        let shown = testing::draw(60, 14, |page| waiting(true).draw(page));
        let hidden = testing::draw(60, 14, |page| waiting(false).draw(page));
        assert!(shown.contains(quips::cycling(0, Duration::ZERO)));
        assert!(!hidden.contains(quips::cycling(0, Duration::ZERO)));
        let rows = |screen: &str| screen.lines().filter(|line| line.contains('┃')).count();
        assert_eq!(rows(&shown), rows(&hidden) + 2);
    }

    #[test]
    fn before_uzdoom_prints_anything_it_is_starting() {
        let waiting = Waiting {
            title: "launching",
            details: &[],
            latest: String::new(),
            elapsed: Duration::ZERO,
            seed: 0,
            show_quip: true,
        };
        let screen = testing::draw(60, 14, |page| waiting.draw(page));
        assert!(screen.contains("Starting UZDoom · 0:00"));
    }

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        assert_eq!(shorten("short", 10), "short");
        assert_eq!(shorten("adding /a/very/long/path.wad", 10), "adding /a…");
    }
}
