use std::path::{Path, PathBuf};

use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    music, picker,
    popups::{legend::Legend, three_way_dialog::ThreeWayDialog, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme,
    vault::global::{Global, Music, SplashQuip},
    widgets::{
        form::{self, Field},
        text,
    },
};

/// The settings: whether the splash screen shows and extra arguments for UZDoom, for this vault;
/// and the music, the song, and the quips, for every vault.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub splash: bool,
    /// Whether the splash is the pride one, for every vault.
    pub pride: bool,
    pub music: Music,
    pub uzdoom_args: String,
    /// Whether windows show quips at all, for every vault.
    pub quips: bool,
    /// The splash quip as it is typed: see [`SplashQuip::from_typed`].
    pub quip: String,
    /// A song of the player's own instead of the built-in track, for every vault.
    pub song: Option<PathBuf>,
}

impl Settings {
    /// The settings as `global` has them.
    pub fn of(global: &Global) -> Self {
        Self {
            splash: !global.skip_splash,
            pride: global.pride,
            music: global.music,
            uzdoom_args: global.uzdoom_args.clone(),
            quips: !global.no_quips,
            quip: global.splash_quip.typed(),
            song: global.song.as_ref().map(PathBuf::from),
        }
    }

    /// Writes the settings into `global`, which the caller then saves. Fails, changing nothing, if
    /// the quip is a number that isn't one of the quips.
    pub fn apply(self, global: &mut Global) -> std::result::Result<(), String> {
        let quip = SplashQuip::from_typed(&self.quip)?;
        global.skip_splash = !self.splash;
        global.pride = self.pride;
        global.uzdoom_args = self.uzdoom_args.trim().to_string();
        global.splash_quip = quip;
        global.no_quips = !self.quips;
        global.music = self.music;
        global.song = self.song.map(PathBuf::into_os_string);
        Ok(())
    }
}

/// A form for the [`Settings`], opened with `O` in the launcher. Enter keeps the changes, Esc
/// throws them away.
pub struct SettingsForm {
    settings: Settings,
    /// The settings as the form was opened with, to tell whether Esc has anything to ask about.
    original: Settings,
    focus: usize,
    /// Where the cursor is in the focused field, when it is one that is typed into.
    caret: usize,
}

impl SettingsForm {
    const TITLE: &str = "settings";
    const BORDER_COLOR: Color = theme::SCREEN;
    const FIELDS: usize = 7;
    /// The longest the UZDoom arguments may get, so the field stays something that fits a window.
    const MAX_ARGS: usize = 120;
    /// The longest a quip may be typed; the splash cuts it shorter on a narrow terminal anyway.
    const MAX_QUIP: usize = 60;
    /// The fields before this are switched with J/K; the ones from here are typed into.
    const TOGGLES: usize = 4;
    const PRIDE: usize = 1;
    /// The one toggle with three choices instead of two.
    const MUSIC: usize = 2;
    /// Picked with a file dialog rather than typed.
    const SONG: usize = 4;
    const ARGS: usize = 5;
    const QUIP: usize = 6;
    const TOGGLE_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Switch"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const ARGS_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Any but ?", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const SONG_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Pick a Song"],
        ["Backspace", "Back to the Built-in Track"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const QUIP_LEGEND: &[[&str; 2]] = &[
        ["Enter", "Save"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Any Key", "Your Own Quip"],
        ["#12", "Built-in Quip 12, Words Can Follow"],
        ["##", "A Plain #"],
        ["(empty)", "A Random Quip"],
        ["F1", "These Keys"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];

    pub fn new(settings: Settings) -> Self {
        Self {
            original: settings.clone(),
            settings,
            focus: 0,
            caret: 0,
        }
    }

    /// The settings as they were left, or `None` if nothing is to be saved.
    pub fn summon(grimoire: &mut Grimoire, settings: Settings) -> Result<Option<Settings>> {
        Self::new(settings).invoke(grimoire)
    }
}

impl Ritual for SettingsForm {
    type Return = Option<Settings>;

    fn render(&mut self, page: &mut Page) {
        let splash = if self.settings.splash {
            "shown"
        } else {
            "skipped"
        };
        let pride = if self.settings.pride { "on" } else { "off" };
        let music = self.settings.music.label();
        let quips = if self.settings.quips {
            "shown"
        } else {
            "hidden"
        };
        let song = self
            .settings
            .song
            .as_deref()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let fields = [
            Field {
                label: "splash",
                value: splash,
                placeholder: "",
                room: 13,
                caret: None,
            },
            Field {
                label: "pride",
                value: pride,
                placeholder: "",
                room: 13,
                caret: None,
            },
            Field {
                label: "music",
                value: music,
                placeholder: "",
                room: 13,
                caret: None,
            },
            Field {
                label: "quips",
                value: quips,
                placeholder: "",
                room: 13,
                caret: None,
            },
            Field {
                label: "song",
                value: &song,
                placeholder: "built-in, Enter to pick",
                room: 24,
                caret: None,
            },
            Field {
                label: "uzdoom args",
                value: &self.settings.uzdoom_args,
                placeholder: "none",
                room: 24,
                caret: Some(self.caret),
            },
            Field {
                label: "splash quip",
                value: &self.settings.quip,
                placeholder: "random, #12, or your own",
                room: 24,
                caret: Some(self.caret),
            },
        ];
        form::draw(page, Self::TITLE, Self::BORDER_COLOR, &fields, self.focus);
    }

    fn prompt(&mut self, grimoire: &mut Grimoire, key_code: KeyCode) -> Result<Phase<Self::Tea>> {
        // ❓ a quip may well end in `?`, so there the legend is on F1 instead
        let legend = match (self.focus, key_code) {
            (Self::QUIP, KeyCode::F(1)) => Some(Self::QUIP_LEGEND),
            (Self::QUIP, _) => None,
            (Self::ARGS, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::ARGS_LEGEND),
            (Self::SONG, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::SONG_LEGEND),
            (_, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::TOGGLE_LEGEND),
            _ => None,
        };
        if let Some(legend) = legend {
            Legend::summon(grimoire, legend)?;
            return Ok(Phase::Render);
        }
        // 🎶 on the song, Enter picks one instead of saving, the way a button is pressed
        if self.focus == Self::SONG && key_code == KeyCode::Enter {
            self.pick_song(grimoire)?;
            return Ok(Phase::Render);
        }
        match key_code {
            KeyCode::Enter => self.save(grimoire),
            // 💾 leaving with changes asks what to do with them; with none it just leaves
            KeyCode::Esc if self.is_changed() => {
                match ThreeWayDialog::summon(
                    grimoire,
                    "Save the changes to the settings?",
                    ["Save", "Discard", "Back"],
                )? {
                    Some(0) => self.save(grimoire),
                    Some(1) => Ok(Phase::Return(None)),
                    _ => Ok(Phase::Render),
                }
            }
            key_code => Ok(self.edit(key_code)),
        }
    }
}

impl SettingsForm {
    /// Whether anything is different from what the form was opened with.
    fn is_changed(&self) -> bool {
        self.settings != self.original
    }

    /// Closes the form with the settings, unless the quip has a number that isn't a quip, which is
    /// said first so it can be fixed.
    fn save(&mut self, grimoire: &mut Grimoire) -> Result<Phase<Option<Settings>>> {
        if let Err(problem) = SplashQuip::from_typed(&self.settings.quip) {
            Warning::summon(grimoire, &problem)?;
            return Ok(Phase::Render);
        }
        Ok(Phase::Return(Some(self.settings.clone())))
    }

    /// Asks for a song, starting where the current one is, and takes it if it is one that can be
    /// played. A file that can't be is said, and the song stays as it was.
    fn pick_song(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let start = self.settings.song.as_deref().and_then(Path::parent);
        let Some(path) = picker::file(grimoire, Some(("songs", music::SONG_EXTENSIONS)), start)?
        else {
            return Ok(());
        };
        match music::check_song(&path) {
            Ok(_) => self.settings.song = Some(path),
            Err(problem) => Warning::summon(grimoire, &problem)?,
        }
        Ok(())
    }

    /// Every key but the legend's and picking a song, none of which needs the terminal.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Option<Settings>> {
        match key_code {
            KeyCode::Enter => Phase::Return(Some(self.settings.clone())),
            KeyCode::Esc => Phase::Return(None),
            KeyCode::Tab | KeyCode::Down => self.move_focus(1),
            KeyCode::BackTab | KeyCode::Up => self.move_focus(Self::FIELDS - 1),
            // 🎵 the music has three choices: J or → goes on to the next, K or ← back
            KeyCode::Char('j') | KeyCode::Right if self.focus == Self::MUSIC => {
                self.settings.music = self.settings.music.next();
                Phase::Render
            }
            KeyCode::Char('k') | KeyCode::Left if self.focus == Self::MUSIC => {
                self.settings.music = self.settings.music.previous();
                Phase::Render
            }
            KeyCode::Char('j' | 'k') | KeyCode::Left | KeyCode::Right
                if self.focus < Self::TOGGLES =>
            {
                let toggle = match self.focus {
                    0 => &mut self.settings.splash,
                    Self::PRIDE => &mut self.settings.pride,
                    _ => &mut self.settings.quips,
                };
                *toggle = !*toggle;
                Phase::Render
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::SONG => {
                Phase::render_if(self.settings.song.take().is_some())
            }
            key_code => {
                let (text, max) = match self.focus {
                    Self::ARGS => (&mut self.settings.uzdoom_args, Self::MAX_ARGS),
                    Self::QUIP => (&mut self.settings.quip, Self::MAX_QUIP),
                    _ => return Phase::Prompt,
                };
                match text::edit(text, &mut self.caret, key_code, max, Some) {
                    Some(changed) => Phase::render_if(changed),
                    None => Phase::Prompt,
                }
            }
        }
    }

    /// Moves to the field `by` away, wrapping around, with the cursor at the end of its text.
    fn move_focus(&mut self, by: usize) -> Phase<Option<Settings>> {
        self.focus = (self.focus + by) % Self::FIELDS;
        self.caret = match self.focus {
            Self::ARGS => text::end(&self.settings.uzdoom_args),
            Self::QUIP => text::end(&self.settings.quip),
            _ => 0,
        };
        Phase::Render
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;
    use crate::testing;

    /// Sends `keys` until one of them closes the form, and returns what it was left with.
    fn press(form: &mut SettingsForm, keys: &[KeyCode]) -> Option<Option<Settings>> {
        keys.iter().find_map(|&key| match form.edit(key) {
            Phase::Return(settings) => Some(settings),
            _ => None,
        })
    }

    #[test]
    fn the_settings_come_from_the_global_ones_and_go_back_into_them() {
        let global = Global {
            skip_splash: true,
            uzdoom_args: "-nomusic".into(),
            splash_quip: SplashQuip::Custom("#3".into()),
            ..Global::default()
        };
        let settings = Settings::of(&global);
        assert!(!settings.splash);
        assert_eq!(settings.music, Music::Splash);
        assert_eq!(settings.quip, "#3");

        let mut changed = Global::default();
        Settings {
            splash: false,
            pride: true,
            music: Music::Kept,
            uzdoom_args: "  +sv_cheats 1  ".into(),
            quips: false,
            quip: "Rip and tear!".into(),
            song: Some("/songs/e1m1.ogg".into()),
        }
        .apply(&mut changed)
        .unwrap();
        assert!(changed.skip_splash);
        assert!(changed.pride);
        assert_eq!(changed.music, Music::Kept);
        // spaces around the arguments mean nothing, so they aren't kept
        assert_eq!(changed.uzdoom_args, "+sv_cheats 1");
        assert_eq!(
            changed.splash_quip,
            SplashQuip::Custom("Rip and tear!".into())
        );
        assert!(changed.no_quips);
        assert_eq!(changed.song, Some("/songs/e1m1.ogg".into()));
    }

    #[test]
    fn the_music_cycles_through_off_splash_only_and_keeps_playing() {
        assert_eq!(Music::Off.next(), Music::Splash);
        assert_eq!(Music::Kept.next(), Music::Off);
        assert_eq!(Music::Off.previous(), Music::Kept);
        assert_eq!(Music::Splash.previous(), Music::Off);

        let mut form = SettingsForm::new(Settings::of(&Global::default()));
        form.focus = SettingsForm::MUSIC;
        assert!(form.edit(KeyCode::Char('k')) == Phase::Render);
        assert_eq!(form.settings.music, Music::Off);

        let mut global = Global::default();
        form.settings.clone().apply(&mut global).unwrap();
        assert_eq!(global.music, Music::Off);
    }

    #[test]
    fn a_quip_number_that_is_not_one_changes_nothing() {
        let mut global = Global::default();
        let settings = Settings {
            splash: false,
            quip: "#0".into(),
            ..Settings::of(&global)
        };
        assert!(settings.apply(&mut global).is_err());
        assert_eq!(global, Global::default());
    }

    #[test]
    fn backspace_on_the_song_goes_back_to_the_built_in_track() {
        let mut form = SettingsForm::new(Settings {
            song: Some("/songs/e1m1.ogg".into()),
            ..Settings::of(&Global::default())
        });
        form.focus = SettingsForm::SONG;
        assert!(
            form.edit(KeyCode::Char('x')) == Phase::Prompt,
            "a song isn't typed"
        );
        assert!(form.edit(KeyCode::Backspace) == Phase::Render);
        assert_eq!(form.settings.song, None);
        assert!(form.edit(KeyCode::Backspace) == Phase::Prompt);
    }

    #[test]
    fn the_quip_takes_question_marks_as_words() {
        let mut form = SettingsForm::new(Settings::of(&Global::default()));
        form.focus = SettingsForm::QUIP;
        for c in "Minecraft?".chars() {
            assert!(form.edit(KeyCode::Char(c)) == Phase::Render);
        }
        assert_eq!(form.settings.quip, "Minecraft?");
    }

    #[test]
    fn toggles_switch_and_the_args_take_any_character_including_spaces() {
        let mut form = SettingsForm::new(Settings::of(&Global::default()));
        let keys: Vec<_> = [
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Tab,
        ]
        .into_iter()
        .chain("-skill 4 x".chars().map(KeyCode::Char))
        .chain([KeyCode::Backspace, KeyCode::Enter])
        .collect();
        let settings = press(&mut form, &keys).unwrap().unwrap();
        assert_eq!(
            settings,
            Settings {
                splash: false,
                pride: true,
                music: Music::Kept,
                uzdoom_args: "-skill 4 ".into(),
                quips: false,
                quip: String::new(),
                song: None,
            }
        );
    }

    #[test]
    fn a_change_is_noticed_and_changing_it_back_is_no_change() {
        let mut form = SettingsForm::new(Settings::of(&Global::default()));
        assert!(!form.is_changed());
        let _ = form.edit(KeyCode::Char('j'));
        assert!(form.is_changed());
        let _ = form.edit(KeyCode::Char('j'));
        assert!(!form.is_changed(), "the splash is back as it was");
    }

    #[test]
    fn escape_throws_the_changes_away() {
        let mut form = SettingsForm::new(Settings::of(&Global::default()));
        assert_eq!(
            press(&mut form, &[KeyCode::Char('j'), KeyCode::Esc]),
            Some(None)
        );
    }

    #[test]
    fn snapshot_the_settings_with_the_args_in_focus() {
        let mut form = SettingsForm::new(Settings {
            splash: true,
            pride: true,
            music: Music::Splash,
            uzdoom_args: "-nomusic +vid_fps 1".into(),
            quips: true,
            quip: "#2".into(),
            song: Some("/songs/e1m1.ogg".into()),
        });
        form.focus = SettingsForm::ARGS;
        assert_snapshot!(testing::draw(60, 20, |page| form.render(page)));
    }
}
