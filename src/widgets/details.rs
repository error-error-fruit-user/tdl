//! A small status panel: one line each for a profile's UZDoom, base wad, ini and wads, with a
//! colored mark that says at a glance whether each is fine, broken, changed, or not set.

use std::{ffi::OsStr, path::Path};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    layout, theme,
    vault::{
        config::Config,
        profile::{Part, Profile},
        wad::{Fault, Wad},
    },
};

/// How a line in the panel is doing, which decides its mark and color.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    Good,
    Bad,
    Changed,
    /// Not set, which is allowed (an ini, a base wad).
    Unset,
    /// Only used in notes: volatile wads, which are fine but worth knowing about.
    Volatile,
}

impl State {
    fn color(self) -> Color {
        match self {
            Self::Good => theme::GOOD,
            Self::Bad => theme::BAD,
            Self::Changed => theme::CHANGED,
            Self::Unset => theme::MUTED,
            Self::Volatile => theme::VOLATILE,
        }
    }

    fn mark(self) -> &'static str {
        if self == Self::Unset {
            theme::UNSET
        } else {
            theme::STATUS
        }
    }

    fn of(fault: Option<&Fault>) -> Self {
        match fault {
            None => Self::Good,
            Some(fault) if fault.is_blocking() => Self::Bad,
            Some(_) => Self::Changed,
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Detail {
    pub label: &'static str,
    pub state: State,
    pub value: String,
    /// Said after the value: where it comes from, or what is wrong with it. Each part is colored
    /// by its own state, so "1 broken, 1 volatile" is red, then blue.
    pub note: Vec<(String, State)>,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Details {
    /// The panel's title, which sums up the lines.
    pub title: &'static str,
    pub title_color: Color,
    pub lines: Vec<Detail>,
}

/// Width of the label column, from the longest label.
const LABEL_WIDTH: usize = 8;
/// The widest a value gets before it is cut, keeping its end.
const VALUE_WIDTH: usize = 24;

fn file_name(path: &OsStr) -> String {
    Path::new(path)
        .file_name()
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}

/// A line for a file that may be set on the profile, set globally, or not set at all.
fn file_line(
    label: &'static str,
    own: Option<&OsStr>,
    global: Option<&OsStr>,
    fault: Option<&Fault>,
    unset: &str,
) -> Detail {
    let (path, source) = match (own, global) {
        (Some(path), _) => (Some(path), "own"),
        (None, Some(path)) => (Some(path), "global"),
        (None, None) => (None, ""),
    };
    match path {
        Some(path) => Detail {
            label,
            state: State::of(fault),
            value: file_name(path),
            note: match fault {
                Some(fault) => vec![(fault.short().to_string(), State::of(Some(fault)))],
                None => vec![(source.to_string(), State::Unset)],
            },
        },
        None => Detail {
            label,
            state: State::Unset,
            value: unset.to_string(),
            note: Vec::new(),
        },
    }
}

/// `line`, unless this session uses a file of its own from the command line (`--uzdoom`,
/// `--base-wad`): then the line is for that file, said to be the session's. `fault` is the one
/// for the file in effect.
fn or_session(line: Detail, session: Option<&OsStr>, fault: Option<&Fault>) -> Detail {
    let Some(path) = session else {
        return line;
    };
    let mut line = file_line(line.label, Some(path), None, fault, "");
    for (note, _) in &mut line.note {
        if note == "own" {
            *note = "session".to_string();
        }
    }
    line
}

/// The line for UZDoom's extra arguments: the profile's `own`, else the `global` ones, else
/// `unset`, each said where it comes from, the way [`file_line`] does for files. Empty arguments
/// count as none.
fn args_line(own: Option<&str>, global: Option<&str>, unset: &str) -> Detail {
    let (own, global) = (
        own.filter(|args| !args.is_empty()),
        global.filter(|args| !args.is_empty()),
    );
    let (args, source) = match (own, global) {
        (Some(args), _) => (args, "own"),
        (None, Some(args)) => (args, "global"),
        (None, None) => {
            return Detail {
                label: "args",
                state: State::Unset,
                value: unset.to_string(),
                note: Vec::new(),
            };
        }
    };
    Detail {
        label: "args",
        state: State::Good,
        value: args.to_string(),
        note: vec![(source.to_string(), State::Unset)],
    }
}

/// The line for the wads: how many, and how many need a look.
fn wads_line(wads: &[Wad], faults: impl Iterator<Item = Fault>) -> Detail {
    let (mut broken, mut changed) = (0, 0);
    for fault in faults {
        if fault.is_blocking() {
            broken += 1
        } else {
            changed += 1
        }
    }
    let volatile = wads.iter().filter(|w| w.volatile).count();
    let note = [
        (broken, "broken", State::Bad),
        (changed, "changed", State::Changed),
        (volatile, "volatile", State::Volatile),
    ]
    .into_iter()
    .filter(|(count, ..)| *count > 0)
    .map(|(count, what, state)| (format!("{count} {what}"), state))
    .collect();
    let state = if wads.is_empty() {
        State::Unset
    } else if broken > 0 {
        State::Bad
    } else if changed > 0 {
        State::Changed
    } else {
        State::Good
    };
    Detail {
        label: "wads",
        state,
        value: match wads.len() {
            0 => "none".to_string(),
            1 => "1 wad".to_string(),
            n => format!("{n} wads"),
        },
        note,
    }
}

impl Details {
    /// What launching `profile` would use, with `faults` from [`Profile::faults`].
    pub fn launch(profile: &Profile, config: &Config, faults: &[(Part, Fault)]) -> Self {
        let fault = |part| faults.iter().find(|(p, _)| *p == part).map(|(_, f)| f);
        let mut uzdoom = or_session(
            file_line(
                "uzdoom",
                profile.uzdoom.as_deref(),
                config.uzdoom.as_deref(),
                fault(Part::Uzdoom),
                "not set",
            ),
            config.session_uzdoom.as_deref(),
            fault(Part::Uzdoom),
        );
        if uzdoom.state == State::Unset {
            // 🚫 without UZDoom there is no game, so this one is not optional
            uzdoom.state = State::Bad;
        }
        let lines = vec![
            uzdoom,
            or_session(
                file_line(
                    "base wad",
                    profile.base_wad.as_deref(),
                    config.base_wad.as_deref(),
                    fault(Part::BaseWad),
                    "auto",
                ),
                config.session_base_wad.as_deref(),
                fault(Part::BaseWad),
            ),
            file_line(
                "ini",
                profile.ini.as_deref(),
                None,
                fault(Part::Ini),
                "uzdoom's own",
            ),
            args_line(
                profile.uzdoom_args.as_deref(),
                Some(&config.uzdoom_args),
                "none",
            ),
            wads_line(
                &profile.wads,
                faults
                    .iter()
                    .filter(|(part, _)| matches!(part, Part::Wad(_)))
                    .map(|(_, fault)| fault.clone()),
            ),
        ];
        Self::summed_up(lines)
    }

    /// The global settings, which every profile without its own falls back on.
    pub fn global(config: &Config) -> Self {
        // 🧪 what is in effect: this session's from the command line, else the saved ones
        let uzdoom_fault = config
            .uzdoom_in_effect()
            .and_then(|p| Fault::of(Path::new(p), 0));
        let base_fault = config.base_wad_in_effect().and_then(Wad::check);
        let mut uzdoom = or_session(
            file_line(
                "uzdoom",
                None,
                config.uzdoom.as_deref(),
                uzdoom_fault.as_ref(),
                "not set",
            ),
            config.session_uzdoom.as_deref(),
            uzdoom_fault.as_ref(),
        );
        if uzdoom.state == State::Unset {
            uzdoom.state = State::Bad;
        }
        let lines = vec![
            uzdoom,
            or_session(
                file_line(
                    "base wad",
                    None,
                    config.base_wad.as_deref(),
                    base_fault.as_ref(),
                    "auto",
                ),
                config.session_base_wad.as_deref(),
                base_fault.as_ref(),
            ),
            args_line(None, Some(&config.uzdoom_args), "none"),
        ];
        // 🌐 everything here is global, so saying so on every line would only be noise
        let lines = lines
            .into_iter()
            .map(|line| Detail {
                note: line
                    .note
                    .into_iter()
                    .filter(|(note, _)| note != "global")
                    .collect(),
                ..line
            })
            .collect();
        Self {
            title: "global",
            ..Self::summed_up(lines)
        }
    }

    /// What a profile sets for itself, for the editor. Anything unset falls back on the globals.
    pub fn overrides(profile: &Profile) -> Self {
        let check =
            |path: Option<&std::ffi::OsString>| path.and_then(|p| Fault::of(Path::new(p), 0));
        let uzdoom_fault = check(profile.uzdoom.as_ref());
        let base_fault = profile.base_wad.as_ref().and_then(Wad::check);
        let ini_fault = check(profile.ini.as_ref());
        let lines = vec![
            file_line(
                "uzdoom",
                profile.uzdoom.as_deref(),
                None,
                uzdoom_fault.as_ref(),
                "global",
            ),
            file_line(
                "base wad",
                profile.base_wad.as_deref(),
                None,
                base_fault.as_ref(),
                "global",
            ),
            file_line(
                "ini",
                profile.ini.as_deref(),
                None,
                ini_fault.as_ref(),
                "uzdoom's own",
            ),
            args_line(profile.uzdoom_args.as_deref(), None, "global"),
        ];
        Self {
            title: "overrides",
            title_color: theme::TITLE,
            lines,
        }
    }

    /// Titled "ready" or "broken" by whether anything stops it from running.
    fn summed_up(lines: Vec<Detail>) -> Self {
        let broken = lines.iter().any(|line| line.state == State::Bad);
        Self {
            title: if broken { "broken" } else { "ready" },
            title_color: if broken { theme::BAD } else { theme::GOOD },
            lines,
        }
    }

    /// Whether anything in the panel stops the profile from running.
    pub fn is_broken(&self) -> bool {
        self.lines.iter().any(|line| line.state == State::Bad)
    }

    /// The text of each line: label, mark, value, then the note.
    pub fn lines(&self) -> Vec<Line<'_>> {
        self.lines
            .iter()
            .map(|detail| {
                let mut spans = vec![
                    Span::styled(
                        format!("{:>LABEL_WIDTH$} ", detail.label),
                        Style::new().fg(theme::MUTED),
                    ),
                    Span::styled(detail.state.mark(), Style::new().fg(detail.state.color())),
                    Span::raw(" "),
                    Span::styled(
                        layout::tail(&detail.value, VALUE_WIDTH),
                        Style::new().fg(if detail.state == State::Unset {
                            theme::MUTED
                        } else {
                            theme::TEXT
                        }),
                    ),
                ];
                for (i, (note, state)) in detail.note.iter().enumerate() {
                    let separator = if i == 0 { "  " } else { ", " };
                    spans.push(Span::styled(separator, Style::new().fg(theme::MUTED)));
                    spans.push(Span::styled(note.clone(), Style::new().fg(state.color())));
                }
                Line::from(spans)
            })
            .collect()
    }

    /// The lines, made to fit `width`: a line that is too wide drops the parts of its note that
    /// don't fit and ends in `…`, instead of being cut off mid-word.
    pub fn lines_fitting(&self, width: u16) -> Vec<Line<'_>> {
        self.lines()
            .into_iter()
            .map(|line| {
                if line.width() <= width as usize {
                    return line;
                }
                let ellipsis = Span::styled("…", Style::new().fg(theme::MUTED));
                let mut spans = line.spans;
                while spans.len() > 4
                    && spans.iter().map(Span::width).sum::<usize>() + 1 > width as usize
                {
                    spans.pop();
                }
                // 🔪 a dangling ", " or "  " before the ellipsis says nothing, so it goes too
                if spans.len() > 4
                    && spans
                        .last()
                        .is_some_and(|s| s.content.trim().is_empty() || s.content == ", ")
                {
                    spans.pop();
                }
                spans.push(ellipsis);
                Line::from(spans)
            })
            .collect()
    }

    /// The space the lines need.
    pub fn size(&self) -> (u16, u16) {
        let width = self
            .lines()
            .iter()
            .map(|l| l.width() as u16)
            .max()
            .unwrap_or(0);
        (width, self.lines.len() as u16)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    impl Details {
        /// The line with this label, wherever it is in the panel.
        fn line(&self, label: &str) -> &Detail {
            self.lines
                .iter()
                .find(|line| line.label == label)
                .unwrap_or_else(|| panic!("no {label} line"))
        }
    }

    impl Detail {
        fn note_text(&self) -> String {
            let parts: Vec<_> = self.note.iter().map(|(note, _)| note.as_str()).collect();
            parts.join(", ")
        }
    }
    use crate::vault::scribe::tests::Altar;

    fn wad(altar: &Altar, name: &str) -> Wad {
        fs::write(altar.file(name), b"1234").unwrap();
        Wad::from_path(&altar.file(name)).unwrap()
    }

    #[test]
    fn a_healthy_profile_on_global_settings_is_ready() {
        let altar = Altar::new("details-ready");
        fs::write(altar.file("uzdoom"), b"").unwrap();
        let config = Config {
            uzdoom: Some(altar.file("uzdoom").into()),
            ..Config::default()
        };
        let profile = Profile {
            wads: vec![wad(&altar, "a.pk3")],
            ..Profile::default()
        };
        let details = Details::launch(&profile, &config, &profile.faults(&config));
        assert_eq!(details.title, "ready");
        assert_eq!(details.lines[0].value, "uzdoom");
        assert_eq!(details.lines[0].note_text(), "global");
        assert_eq!(details.lines[1].state, State::Unset);
        let wads = details.line("wads");
        assert_eq!(wads.value, "1 wad");
        assert_eq!(wads.state, State::Good);
    }

    #[test]
    fn no_uzdoom_at_all_is_broken() {
        let details = Details::launch(&Profile::default(), &Config::default(), &[]);
        assert_eq!(details.title, "broken");
        assert_eq!(details.lines[0].state, State::Bad);
        assert_eq!(details.lines[0].value, "not set");
    }

    #[test]
    fn broken_and_changed_wads_are_counted() {
        let altar = Altar::new("details-wads");
        fs::write(altar.file("uzdoom"), b"").unwrap();
        let config = Config {
            uzdoom: Some(altar.file("uzdoom").into()),
            ..Config::default()
        };
        let mut volatile = wad(&altar, "v.pk3");
        volatile.volatile = true;
        let profile = Profile {
            wads: vec![wad(&altar, "gone.pk3"), wad(&altar, "grew.pk3"), volatile],
            ..Profile::default()
        };
        fs::remove_file(altar.file("gone.pk3")).unwrap();
        fs::write(altar.file("grew.pk3"), b"123456").unwrap();
        let details = Details::launch(&profile, &config, &profile.faults(&config));
        let wads = details.line("wads");
        assert_eq!(wads.state, State::Bad);
        assert_eq!(wads.value, "3 wads");
        assert_eq!(wads.note_text(), "1 broken, 1 changed, 1 volatile");
        let colors: Vec<_> = wads.note.iter().map(|(_, state)| *state).collect();
        assert_eq!(colors, [State::Bad, State::Changed, State::Volatile]);
        assert_eq!(details.title, "broken");
    }

    #[test]
    fn a_missing_uzdoom_says_so_instead_of_where_it_came_from() {
        let altar = Altar::new("details-missing");
        let profile = Profile {
            uzdoom: Some(altar.file("nope").into()),
            ..Profile::default()
        };
        let config = Config::default();
        let details = Details::launch(&profile, &config, &profile.faults(&config));
        assert_eq!(details.lines[0].note_text(), "missing");
        assert_eq!(details.lines[0].state, State::Bad);
    }

    #[test]
    fn the_args_say_whether_they_are_the_profiles_own_the_global_ones_or_none() {
        let line = |own, global| {
            let detail = args_line(own, global, "none");
            (detail.value.clone(), detail.note_text(), detail.state)
        };
        assert_eq!(
            line(Some("+sv_cheats 1"), Some("-nomusic")),
            ("+sv_cheats 1".into(), "own".into(), State::Good)
        );
        assert_eq!(
            line(None, Some("-nomusic")),
            ("-nomusic".into(), "global".into(), State::Good)
        );
        // nothing typed is the same as none at all
        assert_eq!(
            line(Some(""), Some("")),
            ("none".into(), "".into(), State::Unset)
        );
    }

    #[test]
    fn a_uzdoom_and_base_wad_from_the_command_line_are_said_to_be_the_sessions() {
        let altar = crate::vault::scribe::tests::Altar::new("details-session");
        fs::write(altar.file("uzdoom-dev"), b"").unwrap();
        fs::write(altar.file("freedoom2.wad"), b"iwad").unwrap();
        let config = Config {
            uzdoom: Some("/global/uzdoom".into()),
            session_uzdoom: Some(altar.file("uzdoom-dev").into()),
            session_base_wad: Some(Wad::from_path(&altar.file("freedoom2.wad")).unwrap()),
            ..Config::default()
        };
        let profile = Profile {
            uzdoom: Some("/own/uzdoom".into()),
            ..Profile::default()
        };
        for details in [
            Details::launch(&profile, &config, &profile.faults(&config)),
            Details::global(&config),
        ] {
            let uzdoom = details.line("uzdoom");
            assert_eq!(uzdoom.value, "uzdoom-dev");
            assert_eq!(uzdoom.note_text(), "session");
            assert_eq!(uzdoom.state, State::Good);
            let base = details.line("base wad");
            assert_eq!(base.value, "freedoom2.wad");
            assert_eq!(base.note_text(), "session");
        }
    }

    #[test]
    fn the_editor_shows_what_falls_back_on_the_globals() {
        let details = Details::overrides(&Profile::default());
        assert_eq!(details.title, "overrides");
        assert!(details.lines.iter().all(|l| l.state == State::Unset));
        assert_eq!(details.lines[0].value, "global");
        assert_eq!(details.lines[2].value, "uzdoom's own");
    }

    #[test]
    fn the_lines_line_up_and_the_size_fits_them() {
        let details = Details::launch(&Profile::default(), &Config::default(), &[]);
        let lines = details.lines();
        assert!(
            lines
                .iter()
                .all(|l| l.spans[0].content.len() == LABEL_WIDTH + 1)
        );
        let (width, height) = details.size();
        assert_eq!(height, 5);
        assert_eq!(width, lines.iter().map(|l| l.width() as u16).max().unwrap());
    }

    #[test]
    fn a_line_too_wide_drops_its_last_notes_and_says_so() {
        let details = Details {
            title: "broken",
            title_color: theme::BAD,
            lines: vec![Detail {
                label: "wads",
                state: State::Bad,
                value: "4 wads".into(),
                note: vec![
                    ("4 broken".into(), State::Bad),
                    ("1 volatile".into(), State::Volatile),
                ],
            }],
        };
        let full = details.lines()[0].to_string();
        assert_eq!(full.trim_start(), "wads ■ 4 wads  4 broken, 1 volatile");
        let fitted = details.lines_fitting(30)[0].to_string();
        assert_eq!(fitted.trim_start(), "wads ■ 4 wads  4 broken…");
        assert!(fitted.chars().count() <= 30);
        // a line that fits is left alone
        assert_eq!(details.lines_fitting(80)[0].to_string(), full);
    }
}
