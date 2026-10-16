use std::ffi::OsStr;

use ratatui::{style::Style, text::Span, widgets::Gauge};

use crate::{
    error::Result,
    layout,
    ritual::{self, Grimoire, Haunted},
    theme,
};

/// A progress window for downloads. It holds the grimoire for as long as the download runs and
/// redraws itself every time the count changes.
///
/// Call [`Progress::count_file`] when a file *starts*, then [`Progress::count_bytes`] as its
/// bytes arrive.
pub struct Progress<'a> {
    grimoire: &'a mut Grimoire,
    file_name: String,
    file_count: u64,
    byte_count: u64,
    max_file_count: u64,
    max_byte_count: u64,
}

impl<'a> Progress<'a> {
    const TITLE: &str = "downloading";
    /// The gauge stretches to fit the file name, between these widths.
    const MIN_WIDTH: u16 = 36;
    const MAX_WIDTH: u16 = 64;

    /// Nothing is shown until the first [`Progress::count_file`], since the first file's name and
    /// size aren't known here.
    pub fn new(grimoire: &'a mut Grimoire, max_file_count: u64) -> Self {
        Progress {
            grimoire,
            max_file_count,
            file_name: String::new(),
            file_count: 0,
            byte_count: 0,
            max_byte_count: 0,
        }
    }

    /// The gauge shows the current file's bytes; the label shows `[files done/total] name`.
    fn render(&mut self) -> Result<()> {
        self.grimoire.bewitch(|page| {
            let label = format!(
                "[{}/{}] {}",
                self.file_count, self.max_file_count, self.file_name
            );
            let width = (layout::width(&label) + 4).clamp(Self::MIN_WIDTH, Self::MAX_WIDTH);
            let (width, height) = layout::window(width, 1);
            let (window, window_area, ..) =
                ritual::conjure_window(Self::TITLE, width, height, theme::BUSY, page.area());
            // clamped because the gauge panics outside 0..=1, and a bad count shouldn't take the UI down
            let ratio = if self.max_byte_count == 0 {
                0.0
            } else {
                (self.byte_count as f64 / self.max_byte_count as f64).clamp(0.0, 1.0)
            };
            let gauge = Gauge::default()
                .block(window)
                .ratio(ratio)
                .gauge_style(Style::new().fg(theme::HELLFIRE).bg(theme::IRON))
                .label(Span::styled(label, Style::new().fg(theme::BONE)));
            page.render_widget(gauge, window_area);
        })
    }

    /// Starts the next file: its size becomes the new 100% and the byte count restarts.
    pub fn count_file(&mut self, name: &OsStr, size: u64) -> Result<()> {
        self.file_name = name.to_string_lossy().into_owned();
        self.max_byte_count = size;
        self.file_count += 1;
        self.byte_count = 0;
        self.render()
    }

    /// Adds `amount` bytes to the current file and redraws.
    pub fn count_bytes(&mut self, amount: u64) -> Result<()> {
        self.byte_count += amount;
        self.render()
    }
}
