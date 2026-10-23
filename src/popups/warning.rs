use ratatui::{crossterm::event::KeyCode, style::Style, text::Line};

use crate::{
    error::{Error, Result},
    loadout::{profile::Part, wad::Fault},
    popups,
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
};

/// A popup of one or more lines, dismissed by any key.
pub struct Warning {
    title: &'static str,
    lines: Vec<Line<'static>>,
}

impl Warning {
    const TITLE: &str = "warning";
}

impl Warning {
    /// Shows `message` and waits for a key.
    pub fn summon(grimoire: &mut Grimoire, message: &str) -> Result<()> {
        Self::summon_lines(grimoire, &[message.to_string()])
    }

    /// Shows several lines at once and waits for a key.
    pub fn summon_lines(grimoire: &mut Grimoire, lines: &[String]) -> Result<()> {
        let lines = lines
            .iter()
            .map(|line| Line::styled(line.clone(), Style::new().fg(theme::worn().colors.text)))
            .collect();
        Self::summon_styled(grimoire, lines)
    }

    /// Shows lines that carry their own styles. A line made of one span is still wrapped to fit;
    /// a line of several is shown as it is, so keep those short.
    pub fn summon_styled(grimoire: &mut Grimoire, lines: Vec<Line<'static>>) -> Result<()> {
        Self::styled(lines).invoke(grimoire)
    }

    /// A warning of lines that carry their own styles, ready to be shown.
    pub fn styled(lines: Vec<Line<'static>>) -> Self {
        Warning {
            title: Self::TITLE,
            lines,
        }
    }

    /// Shows lines under a title of their own, for news that isn't a problem, and waits for a key.
    pub fn announce(grimoire: &mut Grimoire, title: &'static str, lines: &[String]) -> Result<()> {
        let lines = lines
            .iter()
            .map(|line| Line::styled(line.clone(), Style::new().fg(theme::worn().colors.text)))
            .collect();
        Warning { title, lines }.invoke(grimoire)
    }

    /// If any of `faults` stops the profile from running, says so and returns `true`. A changed
    /// file only gets marked, so it never counts. Wads are summed up, since the editor is the
    /// place to fix them; the UZDoom path, base wad and ini are named.
    pub fn faults(grimoire: &mut Grimoire, faults: &[(Part, Fault)]) -> Result<bool> {
        let blocking: Vec<_> = faults
            .iter()
            .filter(|(_, fault)| fault.is_blocking())
            .collect();
        if blocking.is_empty() {
            return Ok(false);
        }
        let mut lines: Vec<String> = blocking
            .iter()
            .filter_map(|(part, fault)| match part {
                Part::Uzdoom => Some(format!("UZDoom {fault}")),
                Part::BaseWad => Some(format!("The base wad {fault}")),
                Part::Ini => Some(format!("The ini file {fault}")),
                Part::Wad(_) => None,
            })
            .collect();
        let wads = blocking
            .iter()
            .filter(|(part, _)| matches!(part, Part::Wad(_)))
            .count();
        match wads {
            0 => {}
            1 => lines.push("1 wad needs attention, open the editor (E)".to_string()),
            n => lines.push(format!("{n} wads need attention, open the editor (E)")),
        }
        Self::summon_lines(grimoire, &lines)?;
        Ok(true)
    }

    /// Turns "UZDoom could not be started" from a crash into a message. Anything else passes through.
    pub fn report_spawn(grimoire: &mut Grimoire, result: Result<()>) -> Result<()> {
        match result {
            Err(Error::Spawn(error)) => {
                Self::summon(grimoire, &format!("Could not start UZDoom: {error}"))
            }
            other => other,
        }
    }

    pub fn uzdoom_not_selected(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "No UZDoom path selected!")
    }

    pub fn empty_profile_name(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Profile name cannot be empty")
    }

    pub fn bad_profile_name(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(
            grimoire,
            "A profile's name is letters, digits, _, - and spaces, and cannot start with - or a space",
        )
    }

    pub fn profile_already_exists(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Profile name already exists")
    }

    pub fn host_unreachable(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Could not connect to host!")
    }

    pub fn wad_unreadable(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "A wad file could not be read!")
    }

    pub fn host_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Could not start the host server!")
    }

    pub fn no_dir_to_open(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "No dir found to open the file picker in!")
    }

    pub fn save_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Could not save, is the disk full?")
    }

    pub fn files_too_new(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Files are from a newer version, please update!")
    }

    pub fn upgrade_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Upgrade failed, your original files were kept!")
    }

    pub fn code_refused(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "The host refused the join code!")
    }

    pub fn invalid_port(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "The port must be between 1 and 65535")
    }

    pub fn download_failed(grimoire: &mut Grimoire) -> Result<()> {
        Self::summon(grimoire, "Download failed!")
    }
}

impl Ritual for Warning {
    type Return = ();

    fn render(&mut self, page: &mut Page) {
        let area = page.area();
        let lines: Vec<_> = self
            .lines
            .iter()
            .flat_map(|line| match &line.spans[..] {
                [span] => popups::wrap(&span.content, area)
                    .into_iter()
                    .map(|text| Line::styled(text, span.style.patch(line.style)))
                    .collect(),
                _ => vec![line.clone()],
            })
            .collect();
        let border = if self.title == Self::TITLE {
            theme::worn().colors.warning
        } else {
            theme::worn().colors.dialog
        };
        popups::draw(page, self.title, border, &lines, None);
    }

    fn prompt(&mut self, _: &mut Grimoire, _: KeyCode) -> Result<Phase<Self::Return>> {
        Ok(Phase::Return(()))
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    #[test]
    fn a_short_warning() {
        let mut warning = Warning::styled(vec![Line::raw(
            "1 wad needs attention, open the editor (E)",
        )]);
        assert_snapshot!(testing::draw(70, 10, |page| warning.render(page)));
    }
}
