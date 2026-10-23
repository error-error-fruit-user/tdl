use std::path::{Path, PathBuf};

use ratatui::{crossterm::event::KeyCode, style::Color};

use crate::{
    error::Result,
    loadout::uac::{Music, SplashQuip, Uac},
    music, picker,
    popups::{legend::Legend, three_way_dialog::ThreeWayDialog, warning::Warning},
    ritual::{Grimoire, Page, Phase, Ritual},
    theme::{self, Dressing, Theme, pride::Pride, splash::Splash},
    widgets::{
        form::{self, Field},
        text,
    },
};

/// The settings in the form: whether the splash screen shows, extra arguments for UZDoom, the
/// music, the song, the quips and the rest, as one place keeps them (see [`Places`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub splash: bool,
    /// Whether the splash is the pride one, for every loadout.
    pub pride: bool,
    pub music: Music,
    pub uzdoom_args: String,
    /// Whether windows show quips at all, for every loadout.
    pub quips: bool,
    /// The splash quip as it is typed: see [`SplashQuip::from_typed`].
    pub quip: String,
    /// A song of the player's own instead of the built-in track, for every loadout.
    pub song: Option<PathBuf>,
    /// The `theme.toml` worn instead of the built-in theme.
    pub theme: Option<PathBuf>,
    /// The `splash.toml` the splash screen wears instead of the built-in one.
    pub splash_look: Option<PathBuf>,
    /// The `pride.toml` the pride splash picks its flags from instead of the built-in one.
    pub pride_flags: Option<PathBuf>,
    /// The ini every profile without its own plays with, instead of UZDoom's usual one.
    pub ini: Option<PathBuf>,
}

/// The settings of both places they can be kept in, the global settings and the loadout's own, and
/// which of them the loadout uses.
#[derive(Clone, Debug, PartialEq)]
pub struct Places {
    /// Whether the loadout uses its own settings: `settings` are the loadout's own, and `other`
    /// the global ones. Otherwise the other way round.
    pub own: bool,
    /// The settings in use.
    pub settings: Settings,
    /// The settings of the place not in use.
    pub other: Settings,
}

impl Settings {
    /// The settings as `global` has them.
    pub fn of(global: &Uac) -> Self {
        Self {
            splash: !global.skip_splash,
            pride: global.pride,
            music: global.music,
            uzdoom_args: global.uzdoom_args.clone(),
            quips: !global.no_quips,
            quip: global.splash_quip.typed(),
            song: global.song.as_ref().map(PathBuf::from),
            theme: global.theme.as_ref().map(PathBuf::from),
            splash_look: global.splash.as_ref().map(PathBuf::from),
            pride_flags: global.pride_flags.as_ref().map(PathBuf::from),
            ini: global.ini.as_ref().map(PathBuf::from),
        }
    }

    /// Writes the settings into `global`, which the caller then saves. Fails, changing nothing, if
    /// the quip is a number that isn't one of the quips.
    pub fn apply(self, global: &mut Uac) -> std::result::Result<(), String> {
        let quip = SplashQuip::from_typed(&self.quip)?;
        global.skip_splash = !self.splash;
        global.pride = self.pride;
        global.uzdoom_args = self.uzdoom_args.trim().to_string();
        global.splash_quip = quip;
        global.no_quips = !self.quips;
        global.music = self.music;
        global.song = self.song.map(PathBuf::into_os_string);
        global.theme = self.theme.map(PathBuf::into_os_string);
        global.splash = self.splash_look.map(PathBuf::into_os_string);
        global.pride_flags = self.pride_flags.map(PathBuf::into_os_string);
        global.ini = self.ini.map(PathBuf::into_os_string);
        Ok(())
    }
}

/// A form for the [`Settings`], opened with `O` in the launcher. Its first field switches between
/// the global settings and the loadout's own, showing the ones switched to, and the form changes
/// whichever it shows. Enter keeps the changes, Esc throws them away.
pub struct SettingsForm {
    /// The settings shown, of the loadout's own if `own`, else the global ones.
    settings: Settings,
    own: bool,
    /// The settings of the place not shown, as they were left when it was switched away from.
    other: Settings,
    /// The settings as the form was opened with, to tell whether Esc has anything to ask about.
    original: Places,
    focus: usize,
    /// Where the cursor is in the focused field, when it is one that is typed into.
    caret: usize,
    /// Where a new theme, splash or pride is written: the standard config dir.
    config_dir: PathBuf,
}

impl SettingsForm {
    const TITLE: &str = "settings";
    fn border_color() -> Color {
        theme::worn().colors.screen
    }
    const FIELDS: usize = 12;
    /// The longest the UZDoom arguments may get, so the field stays something that fits a window.
    const MAX_ARGS: usize = 120;
    /// The longest a quip may be typed; the splash cuts it shorter on a narrow terminal anyway.
    const MAX_QUIP: usize = 60;
    /// The fields before this are switched with J/K; the ones from here are typed into.
    const TOGGLES: usize = 5;
    /// Which settings are kept in: the global ones or the loadout's own.
    const PLACE: usize = 0;
    const SPLASH: usize = 1;
    const PRIDE: usize = 2;
    /// The one toggle with three choices instead of two.
    const MUSIC: usize = 3;
    /// Picked with a file dialog rather than typed, as are the theme, the splash's look, the
    /// pride flags and the ini.
    const SONG: usize = 5;
    const THEME: usize = 6;
    const SPLASH_LOOK: usize = 7;
    const PRIDE_FLAGS: usize = 8;
    const INI: usize = 9;
    const ARGS: usize = 10;
    const QUIP: usize = 11;
    const TOGGLE_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Save"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Switch"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const PLACE_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Save (the Loadout Uses the Settings Shown)"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["J/K, ←/→", "Global Settings or This Loadout's Own"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const ARGS_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Save"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Any but ?", "Input"],
        ["←/→/Home/End", "Move the Cursor"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const SONG_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Pick a Song"],
        ["Backspace", "Back to the Built-in Track"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const THEME_LEGEND: &[[&'static str; 2]] = &[
        ["J/K, ←/→", "Built-in, Then Each by Name"],
        ["Enter", "Built-in, New (Write It to theme.toml) or Select"],
        ["Backspace", "Back to the Built-in Theme"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const SPLASH_LEGEND: &[[&'static str; 2]] = &[
        ["J/K, ←/→", "Built-in, Then Each by Name"],
        ["Enter", "Built-in, New (Write It to splash.toml) or Select"],
        ["Backspace", "Back to the Built-in Splash"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const PRIDE_LEGEND: &[[&'static str; 2]] = &[
        ["J/K, ←/→", "Built-in, Then Each by Name"],
        ["Enter", "Built-in, New (Write It to pride.toml) or Select"],
        ["Backspace", "Back to the Built-in Flags"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const INI_LEGEND: &[[&'static str; 2]] = &[
        ["Enter", "Pick an Ini"],
        ["Backspace", "Back to UZDoom's Own"],
        ["Tab/↓, Backtab/↑", "Traverse"],
        ["Esc", "Quit (Asks to Save Changes)"],
    ];
    const QUIP_LEGEND: &[[&'static str; 2]] = &[
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

    /// The form, starting from `places`, showing the settings in use; a new theme, splash or
    /// pride would be written to `config_dir`.
    pub fn new(places: Places, config_dir: PathBuf) -> Self {
        Self {
            original: places.clone(),
            settings: places.settings,
            own: places.own,
            other: places.other,
            focus: 0,
            caret: 0,
            config_dir,
        }
    }

    /// The settings as they were left, the ones shown last being the ones to use, or `None` if
    /// nothing is to be saved.
    pub fn summon(
        grimoire: &mut Grimoire,
        places: Places,
        config_dir: PathBuf,
    ) -> Result<Option<Places>> {
        Self::new(places, config_dir).invoke(grimoire)
    }

    /// The settings of both places as the form has them.
    fn places(&self) -> Places {
        Places {
            own: self.own,
            settings: self.settings.clone(),
            other: self.other.clone(),
        }
    }
}

impl Ritual for SettingsForm {
    type Return = Option<Places>;

    fn typing(&self) -> bool {
        matches!(self.focus, Self::ARGS | Self::QUIP)
    }

    fn render(&mut self, page: &mut Page) {
        let place = if self.own { "this loadout" } else { "global" };
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
        let file_name = |path: &Option<PathBuf>| {
            path.as_deref()
                .and_then(Path::file_name)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let song = file_name(&self.settings.song);
        let theme = file_name(&self.settings.theme);
        let splash_look = file_name(&self.settings.splash_look);
        let pride_flags = file_name(&self.settings.pride_flags);
        let ini = file_name(&self.settings.ini);
        let fields = [
            Field {
                label: "kept in",
                value: place,
                placeholder: "",
                room: 13,
                caret: None,
            },
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
                label: "theme",
                value: &theme,
                placeholder: "built-in, ←/→ or Enter",
                room: 24,
                caret: None,
            },
            Field {
                label: "splash look",
                value: &splash_look,
                placeholder: "built-in, ←/→ or Enter",
                room: 24,
                caret: None,
            },
            Field {
                label: "pride flags",
                value: &pride_flags,
                placeholder: "built-in, ←/→ or Enter",
                room: 24,
                caret: None,
            },
            Field {
                label: "ini",
                value: &ini,
                placeholder: "uzdoom's own, Enter to pick",
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
        form::draw(page, Self::TITLE, Self::border_color(), &fields, self.focus);
    }

    fn prompt(
        &mut self,
        grimoire: &mut Grimoire,
        key_code: KeyCode,
    ) -> Result<Phase<Self::Return>> {
        // ❓ a quip may well end in `?`, so there the legend is on F1 instead
        let legend = match (self.focus, key_code) {
            (Self::QUIP, KeyCode::F(1)) => Some(Self::QUIP_LEGEND),
            (Self::QUIP, _) => None,
            (Self::ARGS, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::ARGS_LEGEND),
            (Self::SONG, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::SONG_LEGEND),
            (Self::THEME, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::THEME_LEGEND),
            (Self::SPLASH_LOOK, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::SPLASH_LEGEND),
            (Self::PRIDE_FLAGS, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::PRIDE_LEGEND),
            (Self::INI, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::INI_LEGEND),
            (Self::PLACE, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::PLACE_LEGEND),
            (_, KeyCode::Char('?') | KeyCode::F(1)) => Some(Self::TOGGLE_LEGEND),
            _ => None,
        };
        if let Some(legend) = legend {
            Legend::summon(grimoire, legend)?;
            return Ok(Phase::Render);
        }
        // 🎶 on the song, the theme and the ini, Enter picks one instead of saving, the way a
        // button is pressed
        if key_code == KeyCode::Enter {
            match self.focus {
                Self::SONG => self.pick_song(grimoire)?,
                Self::THEME => {
                    let picked = self.pick::<Theme>(grimoire, self.settings.theme.clone())?;
                    self.settings.theme = picked;
                }
                Self::SPLASH_LOOK => {
                    let picked =
                        self.pick::<Splash>(grimoire, self.settings.splash_look.clone())?;
                    self.settings.splash_look = picked;
                }
                Self::PRIDE_FLAGS => {
                    let picked = self.pick::<Pride>(grimoire, self.settings.pride_flags.clone())?;
                    self.settings.pride_flags = picked;
                }
                Self::INI => self.pick_ini(grimoire)?,
                _ => return self.save(grimoire),
            }
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
        self.places() != self.original
    }

    /// Closes the form with the settings, unless the quip has a number that isn't a quip, which is
    /// said first so it can be fixed.
    fn save(&mut self, grimoire: &mut Grimoire) -> Result<Phase<Option<Places>>> {
        for settings in [&self.settings, &self.other] {
            if let Err(problem) = SplashQuip::from_typed(&settings.quip) {
                Warning::summon(grimoire, &problem)?;
                return Ok(Phase::Render);
            }
        }
        Ok(Phase::Return(Some(self.places())))
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

    /// Asks which theme (or splash, or pride) to wear, starting from `current`: the built-in one,
    /// a new one written out from it to change (an existing file of that name is kept as it is),
    /// or one picked from the disk. Returns what it is left with: one that can't be worn is said,
    /// and `current` stays.
    fn pick<T: Dressing>(
        &mut self,
        grimoire: &mut Grimoire,
        current: Option<PathBuf>,
    ) -> Result<Option<PathBuf>> {
        let options = ["Built-in", "New", "Select"];
        let question = format!("Which {} to wear?", T::WHAT);
        let path = match ThreeWayDialog::summon(grimoire, &question, options)? {
            Some(0) => return Ok(None),
            Some(1) => {
                let new = self.config_dir.join(T::FILE);
                if let Err(error) = T::write_new(&new) {
                    let problem = format!("Couldn't write {}: {error}", new.display());
                    Warning::summon(grimoire, &problem)?;
                    return Ok(current);
                }
                new
            }
            Some(_) => {
                let start = current.as_deref().and_then(Path::parent);
                match picker::file(grimoire, Some(("toml", &["toml"])), start)? {
                    Some(path) => path,
                    None => return Ok(current),
                }
            }
            None => return Ok(current),
        };
        match T::read(&path) {
            Ok(_) => Ok(Some(path)),
            Err(problem) => {
                Warning::summon(grimoire, &problem)?;
                Ok(current)
            }
        }
    }

    /// Asks for the ini to play with, starting where the current one is.
    fn pick_ini(&mut self, grimoire: &mut Grimoire) -> Result<()> {
        let start = self.settings.ini.as_deref().and_then(Path::parent);
        if let Some(path) = picker::file(grimoire, Some(("config", &["ini"])), start)? {
            self.settings.ini = Some(path);
        }
        Ok(())
    }

    /// Every key but the legend's and picking a song, a theme or an ini, none of which needs the
    /// terminal.
    fn edit(&mut self, key_code: KeyCode) -> Phase<Option<Places>> {
        match key_code {
            KeyCode::Enter => Phase::Return(Some(self.places())),
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
            // 👗 the theme, the splash and the pride go round the ones that can be picked by name
            KeyCode::Char(key @ ('j' | 'k')) if self.is_dressing() => self.cycle(key == 'j'),
            KeyCode::Right if self.is_dressing() => self.cycle(true),
            KeyCode::Left if self.is_dressing() => self.cycle(false),
            // 🏠 the other place's settings are shown, and the loadout is to use them
            KeyCode::Char('j' | 'k') | KeyCode::Left | KeyCode::Right
                if self.focus == Self::PLACE =>
            {
                self.own = !self.own;
                std::mem::swap(&mut self.settings, &mut self.other);
                Phase::Render
            }
            KeyCode::Char('j' | 'k') | KeyCode::Left | KeyCode::Right
                if self.focus < Self::TOGGLES =>
            {
                let toggle = match self.focus {
                    Self::SPLASH => &mut self.settings.splash,
                    Self::PRIDE => &mut self.settings.pride,
                    _ => &mut self.settings.quips,
                };
                *toggle = !*toggle;
                Phase::Render
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::SONG => {
                Phase::render_if(self.settings.song.take().is_some())
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::THEME => {
                Phase::render_if(self.settings.theme.take().is_some())
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::SPLASH_LOOK => {
                Phase::render_if(self.settings.splash_look.take().is_some())
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::PRIDE_FLAGS => {
                Phase::render_if(self.settings.pride_flags.take().is_some())
            }
            KeyCode::Backspace | KeyCode::Delete if self.focus == Self::INI => {
                Phase::render_if(self.settings.ini.take().is_some())
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

    /// Whether the focus is on the theme, the splash's look or the pride flags.
    fn is_dressing(&self) -> bool {
        matches!(
            self.focus,
            Self::THEME | Self::SPLASH_LOOK | Self::PRIDE_FLAGS
        )
    }

    /// Goes on to the next (or `forward` false, the previous) theme, splash or pride by name.
    fn cycle(&mut self, forward: bool) -> Phase<Option<Places>> {
        let dir = &self.config_dir;
        let (field, names) = match self.focus {
            Self::THEME => (&mut self.settings.theme, names::<Theme>(dir)),
            Self::SPLASH_LOOK => (&mut self.settings.splash_look, names::<Splash>(dir)),
            _ => (&mut self.settings.pride_flags, names::<Pride>(dir)),
        };
        let next = step(field.as_deref(), &names, forward);
        Phase::render_if(std::mem::replace(field, next) != *field)
    }

    /// Moves to the field `by` away, wrapping around, with the cursor at the end of its text.
    fn move_focus(&mut self, by: usize) -> Phase<Option<Places>> {
        self.focus = (self.focus + by) % Self::FIELDS;
        self.caret = match self.focus {
            Self::ARGS => text::end(&self.settings.uzdoom_args),
            Self::QUIP => text::end(&self.settings.quip),
            _ => 0,
        };
        Phase::Render
    }
}

/// The names a theme, splash or pride can be picked by with `←`/`→`, in order: the ones that come
/// with tdl but `hellfire` (the built-in one, which is no name at all here), then the user's own
/// `<name><SUFFIX>` files in `config_dir`, in alphabetical order, that can be worn.
fn names<T: Dressing>(config_dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = T::BUNDLED
        .iter()
        .map(|(name, _)| name.to_string())
        .collect();
    let mut own: Vec<String> = std::fs::read_dir(config_dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let file = entry.file_name().into_string().ok()?;
            let name = file.strip_suffix(T::SUFFIX)?.to_string();
            let wearable = theme::dressing::is_name(&name) && T::read(&entry.path()).is_ok();
            (wearable && !names.contains(&name)).then_some(name)
        })
        .collect();
    own.sort();
    names.extend(own);
    names
}

/// What comes after `current` (or before it, going back) among the built-in one (`None`) and
/// `names`, wrapping around. A file that isn't one of them goes on to the built-in one.
fn step(current: Option<&Path>, names: &[String], forward: bool) -> Option<PathBuf> {
    let choices: Vec<Option<&str>> = std::iter::once(None)
        .chain(names.iter().map(|name| Some(name.as_str())))
        .collect();
    let at = choices
        .iter()
        .position(|choice| choice.map(Path::new) == current);
    let next = match (at, forward) {
        (None, _) => 0,
        (Some(at), true) => (at + 1) % choices.len(),
        (Some(at), false) => (at + choices.len() - 1) % choices.len(),
    };
    choices[next].map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use insta::assert_snapshot;

    use super::*;

    #[test]
    fn snapshot_the_keyboard_over_the_settings() {
        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        form.focus = SettingsForm::ARGS;
        insta::assert_snapshot!(crate::keyboard::over(|page| form.render(page)));
    }
    use crate::testing;

    /// A form with `settings` as the global ones, in use, and the same as the loadout's own.
    fn form(settings: Settings, config_dir: PathBuf) -> SettingsForm {
        SettingsForm::new(
            Places {
                own: false,
                other: settings.clone(),
                settings,
            },
            config_dir,
        )
    }

    /// Sends `keys` until one of them closes the form, and returns what it was left with.
    fn press(form: &mut SettingsForm, keys: &[KeyCode]) -> Option<Option<Places>> {
        keys.iter().find_map(|&key| match form.edit(key) {
            Phase::Return(settings) => Some(settings),
            _ => None,
        })
    }

    #[test]
    fn the_settings_come_from_the_global_ones_and_go_back_into_them() {
        let global = Uac {
            skip_splash: true,
            uzdoom_args: "-nomusic".into(),
            splash_quip: SplashQuip::Custom("#3".into()),
            ..Uac::default()
        };
        let settings = Settings::of(&global);
        assert!(!settings.splash);
        assert_eq!(settings.music, Music::Splash);
        assert_eq!(settings.quip, "#3");

        let mut changed = Uac::default();
        Settings {
            splash: false,
            pride: true,
            music: Music::Kept,
            uzdoom_args: "  +sv_cheats 1  ".into(),
            quips: false,
            quip: "Rip and tear!".into(),
            song: Some("/songs/e1m1.ogg".into()),
            theme: Some("/themes/blue.toml".into()),
            splash_look: Some("/themes/splash.toml".into()),
            pride_flags: Some("/themes/pride.toml".into()),
            ini: Some("/inis/mine.ini".into()),
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
        assert_eq!(changed.theme, Some("/themes/blue.toml".into()));
        assert_eq!(changed.ini, Some("/inis/mine.ini".into()));
        // and they come back the same way
        let back = Settings::of(&changed);
        assert_eq!(back.theme, Some("/themes/blue.toml".into()));
        assert_eq!(back.splash_look, Some("/themes/splash.toml".into()));
        assert_eq!(back.pride_flags, Some("/themes/pride.toml".into()));
        assert_eq!(back.ini, Some("/inis/mine.ini".into()));
    }

    #[test]
    fn the_music_cycles_through_off_splash_only_and_keeps_playing() {
        assert_eq!(Music::Off.next(), Music::Splash);
        assert_eq!(Music::Kept.next(), Music::Off);
        assert_eq!(Music::Off.previous(), Music::Kept);
        assert_eq!(Music::Splash.previous(), Music::Off);

        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        form.focus = SettingsForm::MUSIC;
        assert!(form.edit(KeyCode::Char('k')) == Phase::Render);
        assert_eq!(form.settings.music, Music::Off);

        let mut global = Uac::default();
        form.settings.clone().apply(&mut global).unwrap();
        assert_eq!(global.music, Music::Off);
    }

    #[test]
    fn a_quip_number_that_is_not_one_changes_nothing() {
        let mut global = Uac::default();
        let settings = Settings {
            splash: false,
            quip: "#0".into(),
            ..Settings::of(&global)
        };
        assert!(settings.apply(&mut global).is_err());
        assert_eq!(global, Uac::default());
    }

    #[test]
    fn backspace_on_the_song_goes_back_to_the_built_in_track() {
        let mut form = form(
            Settings {
                song: Some("/songs/e1m1.ogg".into()),
                ..Settings::of(&Uac::default())
            },
            PathBuf::new(),
        );
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
    fn backspace_on_a_picked_file_goes_back_to_the_built_in_one() {
        let mut form = form(
            Settings {
                theme: Some("/themes/blue.toml".into()),
                splash_look: Some("/themes/splash.toml".into()),
                pride_flags: Some("/themes/pride.toml".into()),
                ini: Some("/inis/mine.ini".into()),
                ..Settings::of(&Uac::default())
            },
            PathBuf::new(),
        );
        for focus in [
            SettingsForm::THEME,
            SettingsForm::SPLASH_LOOK,
            SettingsForm::PRIDE_FLAGS,
            SettingsForm::INI,
        ] {
            form.focus = focus;
            assert!(
                form.edit(KeyCode::Char('x')) == Phase::Prompt,
                "neither is typed"
            );
            assert!(form.edit(KeyCode::Backspace) == Phase::Render);
            assert!(form.edit(KeyCode::Delete) == Phase::Prompt);
        }
        assert_eq!((form.settings.theme, form.settings.ini), (None, None));
        assert_eq!(
            (form.settings.splash_look, form.settings.pride_flags),
            (None, None)
        );
    }

    #[test]
    fn the_theme_and_the_splash_go_round_their_names_and_the_users_own() {
        let altar = crate::loadout::scribe::tests::Altar::new("settings-names");
        std::fs::write(altar.file("night_theme.toml"), "[colors]\ntext = \"white\"").unwrap();
        std::fs::write(altar.file("broken_theme.toml"), "[colors]\nnope = 1").unwrap();
        std::fs::write(altar.file("voidless_theme.toml"), "").unwrap();
        let themes = names::<Theme>(&altar.0);
        assert_eq!(themes.first().map(String::as_str), Some("voidless"));
        assert_eq!(themes.last().map(String::as_str), Some("night"));
        assert_eq!(themes.len(), crate::theme::bundled::THEMES.len() + 1);
        assert!(names::<Pride>(&altar.0).is_empty());

        let mut form = form(Settings::of(&Uac::default()), altar.0.clone());
        form.focus = SettingsForm::THEME;
        let mut seen = Vec::new();
        for _ in 0..=themes.len() {
            assert!(form.edit(KeyCode::Right) == Phase::Render);
            seen.push(form.settings.theme.clone());
        }
        assert_eq!(seen[0], Some("voidless".into()));
        assert_eq!(seen[themes.len() - 1], Some("night".into()));
        assert_eq!(seen[themes.len()], None, "round to the built-in one");
        assert!(form.edit(KeyCode::Char('k')) == Phase::Render);
        assert_eq!(form.settings.theme, Some("night".into()));
        // a file of the user's own anywhere else goes on to the built-in one
        form.settings.theme = Some("/themes/blue.toml".into());
        assert!(form.edit(KeyCode::Left) == Phase::Render);
        assert_eq!(form.settings.theme, None);
        // the splash goes round its own, and pride has none to go round
        form.focus = SettingsForm::SPLASH_LOOK;
        assert!(form.edit(KeyCode::Char('j')) == Phase::Render);
        assert_eq!(form.settings.splash_look, Some("voidless".into()));
        form.focus = SettingsForm::PRIDE_FLAGS;
        assert!(form.edit(KeyCode::Right) == Phase::Prompt);
        assert_eq!(form.settings.pride_flags, None);
    }

    #[test]
    fn the_quip_takes_question_marks_as_words() {
        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        form.focus = SettingsForm::QUIP;
        for c in "Minecraft?".chars() {
            assert!(form.edit(KeyCode::Char(c)) == Phase::Render);
        }
        assert_eq!(form.settings.quip, "Minecraft?");
    }

    #[test]
    fn toggles_switch_and_the_args_take_any_character_including_spaces() {
        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        let keys: Vec<_> = [
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Char('j'),
            KeyCode::Tab,
            KeyCode::Tab,
            KeyCode::Tab,
            KeyCode::Tab,
            KeyCode::Tab,
            KeyCode::Tab,
        ]
        .into_iter()
        .chain("-skill 4 x".chars().map(KeyCode::Char))
        .chain([KeyCode::Backspace, KeyCode::Enter])
        .collect();
        let places = press(&mut form, &keys).unwrap().unwrap();
        assert!(!places.own);
        assert_eq!(places.other, Settings::of(&Uac::default()));
        assert_eq!(
            places.settings,
            Settings {
                splash: false,
                pride: true,
                music: Music::Kept,
                uzdoom_args: "-skill 4 ".into(),
                quips: false,
                quip: String::new(),
                song: None,
                theme: None,
                splash_look: None,
                pride_flags: None,
                ini: None,
            }
        );
    }

    #[test]
    fn a_change_is_noticed_and_changing_it_back_is_no_change() {
        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        assert!(!form.is_changed());
        let _ = form.edit(KeyCode::Char('j'));
        assert!(form.is_changed());
        let _ = form.edit(KeyCode::Char('j'));
        assert!(!form.is_changed(), "the splash is back as it was");
    }

    #[test]
    fn the_first_field_switches_to_the_loadouts_own_settings_and_back() {
        let global = Settings::of(&Uac::default());
        let own = Settings {
            music: Music::Off,
            ..global.clone()
        };
        let mut form = SettingsForm::new(
            Places {
                own: false,
                settings: global.clone(),
                other: own.clone(),
            },
            PathBuf::new(),
        );
        assert!(form.edit(KeyCode::Right) == Phase::Render);
        assert!(form.own);
        assert_eq!(form.settings, own, "the loadout's own are shown");
        assert!(form.is_changed());
        // 🎵 a change to them stays with them when switched away and back
        form.focus = SettingsForm::MUSIC;
        let _ = form.edit(KeyCode::Right);
        form.focus = SettingsForm::PLACE;
        let _ = form.edit(KeyCode::Char('k'));
        assert_eq!(form.settings, global);
        let _ = form.edit(KeyCode::Char('j'));
        let places = press(&mut form, &[KeyCode::Enter]).unwrap().unwrap();
        assert!(places.own);
        assert_eq!(places.settings.music, Music::Splash);
        assert_eq!(places.other, global);
    }

    #[test]
    fn escape_throws_the_changes_away() {
        let mut form = form(Settings::of(&Uac::default()), PathBuf::new());
        assert_eq!(
            press(&mut form, &[KeyCode::Char('j'), KeyCode::Esc]),
            Some(None)
        );
    }

    #[test]
    fn snapshot_the_settings_with_the_args_in_focus() {
        let mut form = form(
            Settings {
                splash: true,
                pride: true,
                music: Music::Splash,
                uzdoom_args: "-nomusic +vid_fps 1".into(),
                quips: true,
                quip: "#2".into(),
                song: Some("/songs/e1m1.ogg".into()),
                theme: Some("/themes/blue.toml".into()),
                splash_look: None,
                pride_flags: Some("/themes/pride.toml".into()),
                ini: None,
            },
            PathBuf::new(),
        );
        form.focus = SettingsForm::ARGS;
        assert_snapshot!(testing::draw(60, 20, |page| form.render(page)));
    }

    #[test]
    fn snapshot_the_settings_kept_in_this_loadout() {
        let settings = Settings::of(&Uac::default());
        let mut form = SettingsForm::new(
            Places {
                own: true,
                other: settings.clone(),
                settings,
            },
            PathBuf::new(),
        );
        assert_snapshot!(testing::draw(60, 30, |page| form.render(page)));
    }
}
