//! What the terminal shows while UZDoom runs (launching, hosting or joining), so a slow start
//! never looks like tdl has frozen: a spinner and a clock that keep moving, and the last thing
//! UZDoom printed, which shows its startup getting along. `L` opens UZDoom's log over it: all it
//! has printed, kept up to date as it prints more.

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
const LOG_HINT: &str = "L shows UZDoom's log";
/// The log window's own hint.
const LOG_KEYS: &str = "↑↓ PgUp PgDn Home End · L or Esc closes";

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
    // 📜 the log window while it is open
    let mut log: Option<Log> = None;
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
        let lines = if log.is_some() { game.lines() } else { vec![] };
        grimoire.bewitch(|page| {
            waiting.draw(page);
            if let Some(log) = &mut log {
                log.draw(page, &lines);
            }
        })?;
        if !event::poll(FRAME)? {
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if !key.is_press() {
            continue;
        }
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            game.stop()?;
            return Err(Error::Interrupted);
        }
        match (&mut log, key.code) {
            (None, KeyCode::Char('l' | 'L')) => log = Some(Log::default()),
            (Some(_), KeyCode::Char('l' | 'L') | KeyCode::Esc) => log = None,
            (Some(log), code) => log.scroll(code),
            _ => {}
        }
    }
}

/// UZDoom's log window: how far back from its latest line it is scrolled (0 follows the lines
/// as they come), and how many lines it showed last, for a page's worth of scrolling.
#[derive(Default)]
struct Log {
    back: usize,
    rows: usize,
}

impl Log {
    fn scroll(&mut self, code: KeyCode) {
        let page = self.rows.max(1);
        self.back = match code {
            KeyCode::Up | KeyCode::Char('k') => self.back + 1,
            KeyCode::Down | KeyCode::Char('j') => self.back.saturating_sub(1),
            KeyCode::PageUp => self.back + page,
            KeyCode::PageDown => self.back.saturating_sub(page),
            KeyCode::Home | KeyCode::Char('g') => usize::MAX,
            KeyCode::End | KeyCode::Char('G') => 0,
            _ => self.back,
        };
    }

    /// The window over most of the screen: the lines that fit, ending `back` from the latest, and
    /// the hint under them.
    fn draw(&mut self, page: &mut Page, lines: &[String]) {
        let area = page.area();
        let (window, window_area, ..) = ritual::conjure_window(
            "UZDoom's log",
            area.width,
            area.height,
            theme::worn().colors.busy,
            area,
        );
        let inner = window.inner(window_area);
        page.render_widget(ratatui::widgets::Clear, window_area);
        page.render_widget(window, window_area);
        // 🧾 a blank row and the hint go under the lines
        self.rows = inner.height.saturating_sub(2) as usize;
        let longest_back = lines.len().saturating_sub(self.rows);
        self.back = self.back.min(longest_back);
        let end = lines.len() - self.back;
        let start = end.saturating_sub(self.rows);
        let text = Style::new().fg(theme::worn().colors.text);
        if lines.is_empty() {
            let row = Rect::new(inner.x, inner.y, inner.width, 1).intersection(inner);
            page.render_widget(
                Line::styled("UZDoom hasn't printed anything yet", text),
                row,
            );
        }
        for (i, line) in lines[start..end].iter().enumerate() {
            let row = Rect::new(inner.x, inner.y + i as u16, inner.width, 1).intersection(inner);
            page.render_widget(Line::styled(shorten(line, inner.width as usize), text), row);
        }
        let where_ = if self.back == 0 {
            String::new()
        } else {
            format!(
                "{} line{} back · ",
                self.back,
                if self.back == 1 { "" } else { "s" }
            )
        };
        let hint = Rect::new(
            inner.x,
            inner.y + inner.height.saturating_sub(1),
            inner.width,
            1,
        )
        .intersection(inner);
        page.render_widget(
            Line::styled(
                format!("{where_}{LOG_KEYS}"),
                Style::new().fg(theme::worn().colors.muted),
            )
            .centered(),
            hint,
        );
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
                Style::new()
                    .fg(theme::worn().colors.text)
                    .add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                shorten(&self.latest, WIDTH as usize - 4),
                Style::new().fg(theme::worn().colors.muted),
            ),
        ];
        if !self.details.is_empty() {
            lines.push(Line::raw(""));
            lines.extend(self.details.iter().map(|detail| {
                Line::styled(detail.clone(), Style::new().fg(theme::worn().colors.text))
            }));
        }
        if self.show_quip {
            lines.push(Line::raw(""));
            lines.push(Line::styled(
                quips::cycling(self.seed, self.elapsed),
                Style::new()
                    .fg(theme::worn().colors.quip)
                    .add_modifier(Modifier::ITALIC),
            ));
        }
        lines.push(Line::raw(""));
        lines.push(Line::styled(
            LOG_HINT,
            Style::new().fg(theme::worn().colors.muted),
        ));
        lines.push(Line::styled(
            HINT,
            Style::new().fg(theme::worn().colors.muted),
        ));

        let (width, height) = layout::window(WIDTH, lines.len() as u16 + 2);
        let (window, window_area, ..) = ritual::conjure_window(
            self.title,
            width,
            height,
            theme::worn().colors.busy,
            page.area(),
        );
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

    fn uzdoom_said(count: usize) -> Vec<String> {
        (1..=count).map(|i| format!("line {i}")).collect()
    }

    #[test]
    fn snapshot_the_log_follows_the_latest_lines() {
        let lines = uzdoom_said(30);
        let screen = testing::draw(60, 16, |page| Log::default().draw(page, &lines));
        assert!(screen.contains("line 30") && !screen.contains("line 1 "));
        assert_snapshot!(screen);
    }

    #[test]
    fn the_log_scrolls_back_no_further_than_its_first_line() {
        let lines = uzdoom_said(30);
        let mut log = Log::default();
        testing::draw(60, 16, |page| log.draw(page, &lines));
        log.scroll(KeyCode::Up);
        let screen = testing::draw(60, 16, |page| log.draw(page, &lines));
        assert!(screen.contains("line 29") && !screen.contains("line 30"));
        assert!(screen.contains("1 line back"));
        log.scroll(KeyCode::Home);
        let screen = testing::draw(60, 16, |page| log.draw(page, &lines));
        assert!(screen.lines().any(|row| row.contains("line 1 ")));
        log.scroll(KeyCode::End);
        assert_eq!(log.back, 0);
    }

    #[test]
    fn an_empty_log_says_so() {
        let screen = testing::draw(60, 16, |page| Log::default().draw(page, &[]));
        assert!(screen.contains("UZDoom hasn't printed anything yet"));
    }

    #[test]
    fn long_lines_are_cut_with_an_ellipsis() {
        assert_eq!(shorten("short", 10), "short");
        assert_eq!(shorten("adding /a/very/long/path.wad", 10), "adding /a…");
    }
}
