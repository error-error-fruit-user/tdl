use std::{
    ffi::OsStr,
    time::{Duration, Instant},
};

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Gauge,
};

use crate::{
    error::Result,
    layout, quips,
    ritual::{self, Grimoire, Haunted, Page},
    tally::Tally,
    theme,
};

/// What the window is waiting on. Each looks different, so hashing is never taken for a download.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Downloading,
    Hashing,
    /// Breaking a scroll's seal: looking for each of its wads in the wad dirs.
    Unsealing,
}

impl Kind {
    fn title(self) -> &'static str {
        match self {
            Self::Downloading => "downloading",
            Self::Hashing => "hashing",
            Self::Unsealing => "unsealing",
        }
    }

    /// The border and the filled part of the bar.
    fn color(self) -> Color {
        match self {
            Self::Downloading => theme::worn().colors.busy,
            Self::Hashing => theme::worn().colors.hashing,
            Self::Unsealing => theme::worn().colors.unsealing,
        }
    }

    /// Put in front of the file name, where the title alone might be missed.
    fn verb(self) -> &'static str {
        match self {
            Self::Downloading => "",
            Self::Hashing => "hashing ",
            Self::Unsealing => "looking for ",
        }
    }

    /// How long the work runs before the window shows. Downloads show at once, since they always
    /// take a moment; most wads hash faster than a window could be read, so it stays hidden.
    fn hidden_for(self) -> Duration {
        match self {
            Self::Downloading => Duration::ZERO,
            Self::Hashing | Self::Unsealing => Duration::from_millis(250),
        }
    }
}

/// A progress window for downloads and hashing. It holds the grimoire for as long as the work
/// runs and redraws itself as the count changes.
///
/// Call [`Progress::count_file`] when a file *starts*, then [`Progress::count_bytes`] as its
/// bytes are done, and [`Progress::tick`] while nothing happens, so the speed can fall.
pub struct Progress<'a> {
    grimoire: &'a mut Grimoire,
    count: Count,
    opened: Instant,
    /// When the window was last drawn, so a flood of small chunks doesn't redraw for every one.
    drawn_at: Option<Instant>,
}

impl<'a> Progress<'a> {
    /// The most often the window redraws while bytes arrive.
    pub const FRAME: Duration = Duration::from_millis(50);

    /// Nothing is shown until the first [`Progress::count_file`], since the first file's name and
    /// size aren't known here.
    pub fn new(grimoire: &'a mut Grimoire, kind: Kind, max_file_count: u64) -> Self {
        let opened = Instant::now();
        Progress {
            grimoire,
            count: Count {
                show_quip: quips::shown(),
                ..Count::new(kind, max_file_count, opened, quips::seed())
            },
            opened,
            drawn_at: None,
        }
    }

    /// A line shown under the numbers, such as the key that cancels.
    pub fn with_footer(mut self, footer: &'static str) -> Self {
        self.count.footer = Some(footer);
        self
    }

    /// Updates the speed and redraws, for when no bytes have come in for a while.
    pub fn tick(&mut self) -> Result<()> {
        self.count.sample(Instant::now());
        self.render()
    }

    fn render(&mut self) -> Result<()> {
        let now = Instant::now();
        if now.duration_since(self.opened) < self.count.kind.hidden_for() {
            return Ok(());
        }
        self.drawn_at = Some(now);
        let count = &self.count;
        self.grimoire.bewitch(|page| count.draw(page, now))
    }
}

impl Tally for Progress<'_> {
    /// Starts the next file: its size becomes the new 100% and the byte count restarts.
    fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
        self.count.start_file(name, size);
        self.render()
    }

    /// Adds `amount` bytes to the current file, and redraws if a frame has passed or it is done.
    fn count_bytes(&mut self, amount: u64) -> Result<()> {
        self.count.add_bytes(amount);
        let now = Instant::now();
        self.count.sample(now);
        let due = self
            .drawn_at
            .is_none_or(|at| now.duration_since(at) >= Self::FRAME);
        if due || self.count.bytes >= self.count.max_bytes {
            self.render()?;
        }
        Ok(())
    }
}

/// What the window shows, kept apart from the grimoire so it can be drawn anywhere.
struct Count {
    kind: Kind,
    name: String,
    files: u64,
    max_files: u64,
    bytes: u64,
    max_bytes: u64,
    /// Every byte that has arrived, across files, for the speed.
    received: u64,
    /// When `received` was last sampled, and what it was then.
    sampled: (Instant, u64),
    /// Bytes per second, smoothed. `None` until there is a sample to go on.
    speed: Option<f64>,
    footer: Option<&'static str>,
    /// When the wait began and where it starts in the quips, which move on as it goes.
    started: Instant,
    seed: u32,
    /// Whether there is a quip at all, which there isn't with the quips hidden.
    show_quip: bool,
}

impl Count {
    /// The gauge stretches to fit the file name, between these widths. The narrowest still fits
    /// the longest quip, so the window doesn't change size as they come and go.
    const MIN_WIDTH: u16 = quips::LONGEST as u16 + 4;
    const MAX_WIDTH: u16 = 64;
    /// How long the speed is measured over before it changes.
    const SAMPLE: Duration = Duration::from_millis(500);

    fn new(kind: Kind, max_files: u64, now: Instant, seed: u32) -> Self {
        Count {
            kind,
            name: String::new(),
            files: 0,
            max_files,
            bytes: 0,
            max_bytes: 0,
            received: 0,
            sampled: (now, 0),
            speed: None,
            footer: None,
            started: now,
            seed,
            show_quip: true,
        }
    }

    /// The next file starts: its size becomes the new 100% and the byte count restarts.
    fn start_file(&mut self, name: &OsStr, size: u64) {
        self.name = name.to_string_lossy().into_owned();
        self.max_bytes = size;
        self.files += 1;
        self.bytes = 0;
    }

    /// `amount` more bytes of the current file are done.
    fn add_bytes(&mut self, amount: u64) {
        self.bytes += amount;
        self.received += amount;
    }

    /// How full the gauge is, clamped because the gauge panics outside 0..=1, and a bad count
    /// shouldn't take the UI down.
    fn ratio(&self) -> f64 {
        if self.max_bytes == 0 {
            0.0
        } else {
            (self.bytes as f64 / self.max_bytes as f64).clamp(0.0, 1.0)
        }
    }

    /// Measures the speed since the last sample, once one has lasted long enough. It is averaged
    /// with the one before, so it moves smoothly instead of flickering with every chunk.
    fn sample(&mut self, now: Instant) {
        let (at, received) = self.sampled;
        let elapsed = now.duration_since(at);
        if elapsed < Self::SAMPLE {
            return;
        }
        let latest = (self.received - received) as f64 / elapsed.as_secs_f64();
        self.speed = Some(self.speed.map_or(latest, |speed| (speed + latest) / 2.0));
        self.sampled = (now, self.received);
    }

    /// The bytes as `12.3 / 48.0 MB`, then the speed once it is known.
    fn numbers(&self) -> String {
        let unit = Unit::for_size(self.max_bytes);
        let mut numbers = format!(
            "{} / {} {}",
            unit.show(self.bytes),
            unit.show(self.max_bytes),
            unit.name
        );
        if let Some(speed) = self.speed {
            let unit = Unit::for_size(speed as u64);
            numbers.push_str(&format!(" · {} {}/s", unit.show(speed as u64), unit.name));
        }
        numbers
    }

    /// The gauge shows the current file's bytes, labelled `[files done/total] name`; the numbers
    /// sit under it, then the quip of the moment (`now` says which), then the footer, if any.
    fn draw(&self, page: &mut Page, now: Instant) {
        let label = format!(
            "[{}/{}] {}{}",
            self.files,
            self.max_files,
            self.kind.verb(),
            self.name
        );
        let numbers = self.numbers();
        let quip = quips::cycling(self.seed, now.duration_since(self.started));
        let widest = layout::width(&label)
            .max(layout::width(&numbers))
            .max(self.footer.map_or(0, layout::width));
        let width = (widest + 4).clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);
        // the gauge and the numbers, then a blank row and the quip, then a blank row and the footer
        let quip_rows = if self.show_quip { 2 } else { 0 };
        let rows = 2 + quip_rows + self.footer.map_or(0, |_| 2);
        let (width, height) = layout::window(width, rows);
        let color = self.kind.color();
        let (window, window_area, ..) =
            ritual::conjure_window(self.kind.title(), width, height, color, page.area());
        let inner = window.inner(window_area);
        page.render_widget(window, window_area);
        let row =
            |offset: u16| Rect::new(inner.x, inner.y + offset, inner.width, 1).intersection(inner);

        let gauge = Gauge::default()
            .ratio(self.ratio())
            .gauge_style(Style::new().fg(color).bg(theme::worn().colors.gauge))
            .label(Span::styled(
                label,
                Style::new().fg(theme::worn().colors.gauge_label),
            ));
        page.render_widget(gauge, row(0));
        page.render_widget(
            Line::styled(numbers, Style::new().fg(theme::worn().colors.muted)).centered(),
            row(1),
        );
        if self.show_quip {
            page.render_widget(
                Line::styled(
                    quip,
                    Style::new()
                        .fg(theme::worn().colors.quip)
                        .add_modifier(Modifier::ITALIC),
                )
                .centered(),
                row(3),
            );
        }
        if let Some(footer) = self.footer {
            page.render_widget(
                Line::styled(footer, Style::new().fg(theme::worn().colors.muted)).centered(),
                row(3 + quip_rows),
            );
        }
    }
}

/// A size unit: how many bytes it is, and what it is called.
struct Unit {
    bytes: u64,
    name: &'static str,
}

impl Unit {
    /// The largest unit that `size` is at least one of, so the numbers stay short.
    fn for_size(size: u64) -> Self {
        let (bytes, name) = match size {
            1_000_000_000.. => (1_000_000_000, "GB"),
            1_000_000.. => (1_000_000, "MB"),
            1_000.. => (1_000, "KB"),
            _ => (1, "B"),
        };
        Unit { bytes, name }
    }

    /// `amount` in this unit, to one decimal (none for plain bytes).
    fn show(&self, amount: u64) -> String {
        if self.bytes == 1 {
            amount.to_string()
        } else {
            format!("{:.1}", amount as f64 / self.bytes as f64)
        }
    }
}
