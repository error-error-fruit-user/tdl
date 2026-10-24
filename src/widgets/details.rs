//! A small status panel: one line each for a profile's UZDoom, base wad, ini and wads, with a
//! colored mark that says at a glance whether each is fine, broken, changed, or not set.

use std::{ffi::OsStr, path::Path};

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

use crate::{
    layout,
    loadout::{
        profile::{Part, Profile},
        uac::Uac,
        wad::{Fault, Wad},
    },
    theme,
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
            Self::Good => theme::worn().colors.good,
            Self::Bad => theme::worn().colors.bad,
            Self::Changed => theme::worn().colors.changed,
            Self::Unset => theme::worn().colors.muted,
            Self::Volatile => theme::worn().colors.volatile,
        }
    }

    fn mark(self) -> &'static str {
        if self == Self::Unset {
            &theme::worn().glyphs.unset
        } else {
            &theme::worn().glyphs.status
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

/// The line for the dir a profile watches, if it watches one. A dir that isn't there is only
/// marked, since the profile still has its wads.
fn watched_line(profile: &Profile) -> Option<Detail> {
    let dir = profile.watched.as_deref()?;
    let there = Path::new(dir).is_dir();
    Some(Detail {
        label: "dir",
        state: if there { State::Good } else { State::Changed },
        value: file_name(dir),
        note: vec![if there {
            ("watched".to_string(), State::Unset)
        } else {
            ("missing".to_string(), State::Changed)
        }],
    })
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
/// `--base-wad`, `--ini`): then the line is for that file, said to be the session's. `fault` is the one
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
    pub fn launch(profile: &Profile, global: &Uac, faults: &[(Part, Fault)]) -> Self {
        let fault = |part| faults.iter().find(|(p, _)| *p == part).map(|(_, f)| f);
        let mut uzdoom = or_session(
            file_line(
                "uzdoom",
                profile.uzdoom.as_deref(),
                global.uzdoom.as_deref(),
                fault(Part::Uzdoom),
                "not set",
            ),
            global.session_uzdoom.as_deref(),
            fault(Part::Uzdoom),
        );
        if uzdoom.state == State::Unset {
            // 🚫 without UZDoom there is no game, so this one is not optional
            uzdoom.state = State::Bad;
        }
        let mut lines = vec![
            uzdoom,
            or_session(
                file_line(
                    "base wad",
                    profile.base_wad.as_deref(),
                    global.base_wad.as_deref(),
                    fault(Part::BaseWad),
                    "auto",
                ),
                global.session_base_wad.as_deref(),
                fault(Part::BaseWad),
            ),
            or_session(
                file_line(
                    "ini",
                    profile.ini.as_deref(),
                    global.ini.as_deref(),
                    fault(Part::Ini),
                    "uzdoom's own",
                ),
                global.session_ini.as_deref(),
                fault(Part::Ini),
            ),
            args_line(
                profile.uzdoom_args.as_deref(),
                Some(&global.uzdoom_args),
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
        // 👀 just above the wads, which it fills
        if let Some(line) = watched_line(profile) {
            lines.insert(lines.len() - 1, line);
        }
        Self::summed_up(lines)
    }

    /// The global settings, which every profile without its own falls back on.
    pub fn global(global: &Uac) -> Self {
        // 🧪 what is in effect: this session's from the command line, else the saved ones
        let uzdoom_fault = global
            .uzdoom_in_effect()
            .and_then(|p| Fault::of(Path::new(p), 0));
        let base_fault = global.base_wad_in_effect().and_then(Wad::check);
        let ini_fault = global
            .ini_in_effect()
            .and_then(|p| Fault::of(Path::new(p), 0));
        let mut uzdoom = or_session(
            file_line(
                "uzdoom",
                None,
                global.uzdoom.as_deref(),
                uzdoom_fault.as_ref(),
                "not set",
            ),
            global.session_uzdoom.as_deref(),
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
                    global.base_wad.as_deref(),
                    base_fault.as_ref(),
                    "auto",
                ),
                global.session_base_wad.as_deref(),
                base_fault.as_ref(),
            ),
            or_session(
                file_line(
                    "ini",
                    None,
                    global.ini.as_deref(),
                    ini_fault.as_ref(),
                    "uzdoom's own",
                ),
                global.session_ini.as_deref(),
                ini_fault.as_ref(),
            ),
            args_line(None, Some(&global.uzdoom_args), "none"),
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
        let mut lines = vec![
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
        lines.extend(watched_line(profile));
        Self {
            title: "overrides",
            title_color: theme::worn().colors.title,
            lines,
        }
    }

    /// Titled "ready" or "broken" by whether anything stops it from running.
    fn summed_up(lines: Vec<Detail>) -> Self {
        let broken = lines.iter().any(|line| line.state == State::Bad);
        Self {
            title: if broken { "broken" } else { "ready" },
            title_color: if broken {
                theme::worn().colors.bad
            } else {
                theme::worn().colors.good
            },
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
                        Style::new().fg(theme::worn().colors.muted),
                    ),
                    Span::styled(detail.state.mark(), Style::new().fg(detail.state.color())),
                    Span::raw(" "),
                    Span::styled(
                        layout::tail(&detail.value, VALUE_WIDTH),
                        Style::new().fg(if detail.state == State::Unset {
                            theme::worn().colors.muted
                        } else {
                            theme::worn().colors.text
                        }),
                    ),
                ];
                for (i, (note, state)) in detail.note.iter().enumerate() {
                    let separator = if i == 0 { "  " } else { ", " };
                    spans.push(Span::styled(
                        separator,
                        Style::new().fg(theme::worn().colors.muted),
                    ));
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
                let ellipsis = Span::styled("…", Style::new().fg(theme::worn().colors.muted));
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
