//! The ini a host plays with, served at `/<code>/ini` when the host shares it, and what a joiner
//! who plays with it makes of it.
//!
//! The host's ini comes from someone else, so not all of it is taken: whatever names files or
//! dirs (wads to load, scripts to run, where to look or write) and whatever runs commands (key
//! bindings, aliases) is left out, and so is what is the joiner's own business (their player,
//! their screen, sound and controls). The joiner's own ini fills those in, if they have one.
//!
//! The host's key bindings can be added too ([`HostIni::WithKeys`]), for the keys a mod's
//! players bind themselves: only to keys the joiner hasn't bound, never a command the joiner
//! already has a key for, and never one that could reach a file, rebind keys or run one of the
//! host's own aliases. What is left out that the game may well need is said as a warning.

use std::{
    ffi::OsStr,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use crate::{
    error::{Error, Result},
    net::{Address, new_agent},
};

/// The biggest ini that is served. A well-used `uzdoom.ini` is a few hundred KB at most.
pub const MAX_INI: u64 = 1024 * 1024;

/// The ini UZDoom plays the profile with: the profile's own (`profile_ini`), or else UZDoom's
/// usual one, if one of the places it keeps it has one. `uzdoom` is the UZDoom that will run,
/// since on Windows the ini can be beside it.
pub fn in_use(profile_ini: Option<&OsStr>, uzdoom: &OsStr) -> Option<PathBuf> {
    match profile_ini {
        Some(ini) => Some(PathBuf::from(ini)).filter(|ini| ini.is_file()),
        None => usual_places(Path::new(uzdoom))
            .into_iter()
            .find(|ini| ini.is_file()),
    }
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks.
#[cfg(all(unix, not(target_os = "macos")))]
fn usual_places(_uzdoom: &Path) -> Vec<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.config_dir().join("uzdoom/uzdoom.ini"))
        .into_iter()
        .collect()
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks.
#[cfg(target_os = "macos")]
fn usual_places(_uzdoom: &Path) -> Vec<PathBuf> {
    directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join("Library/Preferences/uzdoom.ini"))
        .into_iter()
        .collect()
}

/// Where UZDoom keeps its ini when it isn't given one, in the order it looks: beside it for a
/// portable install, then in the documents.
#[cfg(windows)]
fn usual_places(uzdoom: &Path) -> Vec<PathBuf> {
    let beside = uzdoom.parent().unwrap_or(Path::new(""));
    let documents = directories::UserDirs::new().and_then(|dirs| {
        dirs.document_dir()
            .map(|dir| dir.join("My Games/UZDoom/uzdoom.ini"))
    });
    [beside.join("uzdoom_portable.ini")]
        .into_iter()
        .chain(documents)
        .chain([beside.join("uzdoom.ini")])
        .collect()
}

/// The ini at `path`, to be served. One bigger than [`MAX_INI`] is turned down.
pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(MAX_INI + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INI {
        return Err(io::Error::other("the ini is too big to share"));
    }
    Ok(bytes)
}

/// The host's ini, or `None` if it doesn't share one (or is a tdl from before inis were shared).
/// One bigger than [`MAX_INI`] is turned down.
pub fn fetch(address: &Address) -> Result<Option<String>> {
    let response = match new_agent().get(&address.url("ini")).call() {
        Err(ureq::Error::StatusCode(404)) => return Ok(None),
        Err(ureq::Error::StatusCode(403)) => return Err(Error::Refused),
        other => other?,
    };
    let mut bytes = Vec::new();
    response
        .into_body()
        .into_reader()
        .take(MAX_INI + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INI {
        return Err(Error::Download("The host's ini is too big"));
    }
    Ok(Some(String::from_utf8_lossy(&bytes).into_owned()))
}

/// Which ini a joiner plays with.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum HostIni {
    /// Their own, as always.
    #[default]
    Yours,
    /// The host's settings, with the joiner's own player, keys, screen and sound.
    Settings,
    /// The host's settings, and its key bindings on the keys the joiner hasn't bound.
    WithKeys,
}

/// The host's ini as a joiner plays with it: where it was written, and what of the host's was
/// left out that the game may need, one line each, for the joiner.
#[derive(Debug, PartialEq)]
pub struct Taken {
    pub path: PathBuf,
    pub warnings: Vec<String>,
}

/// What the joiner plays with, for a host that shares its ini: it is fetched, [`blend`]ed with
/// `own` (the ini the joiner would play with, if there is one), with the host's keys if `keys`,
/// and written to `host.ini` in `dir`, over the one from last time. `None` if the host shares
/// none, or what went wrong, to be said.
pub fn take(
    address: &Address,
    own: Option<&Path>,
    keys: bool,
    dir: &Path,
) -> std::result::Result<Option<Taken>, String> {
    let host = match fetch(address) {
        Ok(Some(host)) => host,
        Ok(None) => return Ok(None),
        Err(_) => return Err("Couldn't get the host's ini".to_string()),
    };
    let own = own
        .and_then(|own| read(own).ok())
        .map(|own| String::from_utf8_lossy(&own).into_owned());
    let blended = blend(&host, own.as_deref(), keys);
    let path = dir.join("host.ini");
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&path, &blended.text))
        .map_err(|error| format!("Could not save the host's ini: {error}"))?;
    Ok(Some(Taken {
        path,
        warnings: blended.warnings,
    }))
}

/// One `[section]` of an ini and the lines under it, up to the next. The lines before the first
/// section have no name.
struct Section<'a> {
    name: Option<&'a str>,
    lines: Vec<&'a str>,
}

/// The sections of `text`, in order.
fn sections(text: &str) -> Vec<Section<'_>> {
    let mut sections = vec![Section {
        name: None,
        lines: Vec::new(),
    }];
    for line in text.lines() {
        let trimmed = line.trim();
        match trimmed
            .strip_prefix('[')
            .and_then(|rest| rest.strip_suffix(']'))
        {
            Some(name) => sections.push(Section {
                name: Some(name),
                lines: Vec::new(),
            }),
            None => sections.last_mut().unwrap().lines.push(line),
        }
    }
    sections
}

/// The key a line sets (`name` in `name=value`), lowercase, if it sets one.
fn key(line: &str) -> Option<String> {
    let (key, _) = line.split_once('=')?;
    let key = key.trim();
    (!key.is_empty() && !key.starts_with([';', '#'])).then(|| key.to_ascii_lowercase())
}

/// Whether a whole section is the joiner's to keep: their player, their key bindings and aliases
/// (which can run any command), and the files and dirs UZDoom loads, runs and looks in.
fn is_personal_section(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    name.ends_with(".player")
        || name.ends_with("bindings")
        || name.ends_with("consolealiases")
        || name.ends_with("autoexec")
        || name.contains("autoload")
        || name.ends_with(".directories")
}

/// Whether a setting is the joiner's to keep: their screen, sound and controls, and any setting
/// that names a file or dir.
fn is_personal_key(key: &str) -> bool {
    const PREFIXES: &[&str] = &[
        "vid_",
        "win_",
        "fullscreen",
        "snd_",
        "m_",
        "mouse_",
        "joy_",
        "in_",
        "invertmouse",
    ];
    PREFIXES.iter().any(|prefix| key.starts_with(prefix)) || is_path_key(key)
}

/// Whether a setting names a file or dir.
fn is_path_key(key: &str) -> bool {
    const SUFFIXES: &[&str] = &[
        "_dir",
        "_path",
        "_exe",
        "_config",
        "_patchset",
        "_bank",
        "_file",
        "_folder",
    ];
    SUFFIXES.iter().any(|suffix| key.ends_with(suffix))
}

/// Whether a section holds key bindings (`Bindings`, `DoubleBindings`, `AutomapBindings`).
fn is_bindings_section(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with("bindings")
}

/// Whether a bound command could reach a file (writing a log, running a script, recording a
/// demo), change a setting that names one, rebind keys or make up commands of its own. Every word
/// of it is looked at, so one hidden in a nested `bind` or `alias` is found too, and a `$`, which
/// UZDoom can fill in with a setting's value, is never trusted.
fn is_risky_command(command: &str) -> bool {
    const RISKY: &[&str] = &[
        "logfile",
        "condump",
        "writeini",
        "exec",
        "pullin",
        "record",
        "playdemo",
        "timedemo",
        "alias",
        "bind",
        "unbind",
        "unbindall",
        "doublebind",
        "undoublebind",
        "mapbind",
        "unmapbind",
        "defaultbind",
        "binddefaults",
    ];
    command.contains('$')
        || command
            .split(|c: char| c.is_whitespace() || matches!(c, ';' | '"' | '\''))
            .map(str::to_ascii_lowercase)
            .any(|word| RISKY.contains(&word.as_str()) || is_path_key(&word))
}

/// The commands a bound line runs, each by its first word, lowercase: `+use; +attack` runs `+use`
/// and `+attack`.
fn command_names(command: &str) -> impl Iterator<Item = String> {
    command.split(';').filter_map(|part| {
        part.split_whitespace()
            .next()
            .map(|name| name.trim_matches('"').to_ascii_lowercase())
    })
}

/// One `key=command` line of a bindings section.
struct Binding<'a> {
    /// The key as the ini names it (`g`, `mouse4`), and lowercase, to compare.
    key: &'a str,
    lowercase: String,
    command: &'a str,
}

impl Binding<'_> {
    /// How it is said in a warning: `G (+user1)`.
    fn shown(&self) -> String {
        format!("{} ({})", self.key.to_ascii_uppercase(), self.command)
    }
}

/// The bindings in `lines`, leaving out keys bound to nothing.
fn bindings<'a>(lines: &[&'a str]) -> Vec<Binding<'a>> {
    lines
        .iter()
        .filter_map(|line| {
            let (key, command) = line.split_once('=')?;
            let (key, command) = (key.trim(), command.trim());
            (!key.is_empty() && !command.is_empty() && !key.starts_with([';', '#'])).then(|| {
                Binding {
                    key,
                    lowercase: key.to_ascii_lowercase(),
                    command,
                }
            })
        })
        .collect()
}

/// The aliases an ini makes, by name, lowercase: its `ConsoleAliases` hold `Name=` and
/// `Command=` lines in turn.
fn aliases(sections: &[Section]) -> Vec<String> {
    sections
        .iter()
        .filter(|section| {
            section
                .name
                .is_some_and(|name| name.to_ascii_lowercase().ends_with("consolealiases"))
        })
        .flat_map(|section| &section.lines)
        .filter_map(|line| {
            let (key, name) = line.split_once('=')?;
            key.trim()
                .eq_ignore_ascii_case("name")
                .then(|| name.trim().to_ascii_lowercase())
        })
        .collect()
}

/// The `Path=` entries of the sections `pick` picks, each by its file name, lowercase, and as
/// written. The host's paths name its own disk, so only their file names mean anything here.
fn paths<'a>(sections: &[Section<'a>], pick: fn(&str) -> bool) -> Vec<(String, &'a str)> {
    sections
        .iter()
        .filter(|section| section.name.is_some_and(pick))
        .flat_map(|section| &section.lines)
        .filter_map(|line| {
            let (key, path) = line.split_once('=')?;
            let path = path.trim();
            (key.trim().eq_ignore_ascii_case("path") && !path.is_empty()).then(|| {
                let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
                (name.to_ascii_lowercase(), path)
            })
        })
        .collect()
}

/// `things`, said in a line: the first few, and how many more there are.
fn listing(things: &[String]) -> String {
    const SHOWN: usize = 5;
    let mut listed = things[..things.len().min(SHOWN)].join(", ");
    if things.len() > SHOWN {
        listed.push_str(&format!(" and {} more", things.len() - SHOWN));
    }
    listed
}

/// What happened to the host's bindings, for the warnings.
#[derive(Default)]
struct Keys {
    /// Bindings that could have been added, but `keys` wasn't asked for.
    missing: Vec<String>,
    /// Bindings on keys the joiner has bound to something else.
    taken: Vec<String>,
    /// Bindings left out as risky.
    risky: Vec<String>,
    /// Bindings that run one of the host's aliases.
    aliased: Vec<String>,
}

impl Keys {
    fn warnings(self) -> impl Iterator<Item = String> {
        [
            (
                self.missing,
                "The host has keys for things you have no key for",
                ": join with the host's keys to get them",
            ),
            (
                self.taken,
                "The host's keys for these are yours for something else, so they weren't added",
                "",
            ),
            (
                self.risky,
                "Left out the host's keys that could reach files or rebind keys",
                "",
            ),
            (
                self.aliased,
                "Left out the host's keys that run its own aliases, which don't come with its ini",
                "",
            ),
        ]
        .into_iter()
        .filter(|(things, _, _)| !things.is_empty())
        .map(|(things, said, after)| format!("{said}: {}{after}", listing(&things)))
    }
}

/// The joiner's bindings in one section (`own`, if they have the section), with the `host`'s
/// added where they may be (if `keys`), and what was done with the rest noted in `notes`.
/// `host_aliases` are the aliases only the host has.
fn merge_bindings(
    host: &[&str],
    own: Option<&[&str]>,
    keys: bool,
    host_aliases: &[String],
    notes: &mut Keys,
) -> Vec<String> {
    let mut merged: Vec<String> = own
        .unwrap_or_default()
        .iter()
        .map(|line| line.to_string())
        .collect();
    let own = own.map(bindings);
    let has_command = |command: &str| {
        own.iter()
            .flatten()
            .any(|binding| binding.command.eq_ignore_ascii_case(command))
    };
    for binding in bindings(host) {
        if has_command(binding.command) {
            continue;
        }
        let taken = own
            .iter()
            .flatten()
            .any(|own| own.lowercase == binding.lowercase);
        if is_risky_command(binding.command) {
            // 🔒 said only when the keys were asked for, or when they would have been added
            if keys || (own.is_some() && !taken) {
                notes.risky.push(binding.shown());
            }
        } else if command_names(binding.command).any(|name| host_aliases.contains(&name)) {
            if keys || (own.is_some() && !taken) {
                notes.aliased.push(binding.shown());
            }
        } else if taken {
            if keys {
                notes.taken.push(binding.shown());
            }
        } else if keys {
            merged.push(format!("{}={}", binding.key, binding.command));
        } else if own.is_some() {
            notes.missing.push(binding.shown());
        }
    }
    merged
}

/// The ini to join with, and the warnings to give with it.
#[derive(Debug, PartialEq)]
pub struct Blended {
    pub text: String,
    pub warnings: Vec<String>,
}

/// The ini to join with: the `host`'s settings, with everything personal (see the module's
/// notes) taken from `own`, the joiner's ini, if they have one, or else left for UZDoom to fill in
/// with its defaults, and with `keys` the host's key bindings added where they may be. What the
/// game may need that was left out is warned about: the host's keys the joiner has none like,
/// and files the host autoloads or runs at startup that the joiner doesn't.
pub fn blend(host: &str, own: Option<&str>, keys: bool) -> Blended {
    let own = own.map(sections).unwrap_or_default();
    let host = sections(host);
    let own_section = |name: &str| {
        own.iter().find(|section| {
            section
                .name
                .is_some_and(|own| own.eq_ignore_ascii_case(name))
        })
    };
    let own_aliases = aliases(&own);
    let host_aliases: Vec<String> = aliases(&host)
        .into_iter()
        .filter(|alias| !own_aliases.contains(alias))
        .collect();
    let mut notes = Keys::default();
    let mut blended = Vec::new();
    for section in &host {
        if let Some(name) = section.name {
            if is_personal_section(name) {
                // ⌨️ a bindings section the joiner has is merged where theirs goes, below
                if is_bindings_section(name) && own_section(name).is_none() {
                    let merged =
                        merge_bindings(&section.lines, None, keys, &host_aliases, &mut notes);
                    if keys {
                        blended.push(format!("[{name}]"));
                        blended.extend(merged);
                    }
                }
                continue;
            }
            blended.push(format!("[{name}]"));
        }
        blended.extend(
            section
                .lines
                .iter()
                .filter(|line| !key(line).is_some_and(|key| is_personal_key(&key)))
                .map(|line| line.to_string()),
        );
        // 🎮 the joiner's own screen, sound and controls in this section, where the host's were
        if let Some(own) = section.name.and_then(own_section) {
            blended.extend(
                own.lines
                    .iter()
                    .filter(|line| key(line).is_some_and(|key| is_personal_key(&key)))
                    .map(|line| line.to_string()),
            );
        }
    }
    for section in &own {
        let Some(name) = section.name.filter(|name| is_personal_section(name)) else {
            continue;
        };
        blended.push(format!("[{name}]"));
        let host_section = host.iter().find(|host| {
            host.name
                .is_some_and(|host| host.eq_ignore_ascii_case(name))
        });
        match host_section {
            Some(host) if is_bindings_section(name) => blended.extend(merge_bindings(
                &host.lines,
                Some(&section.lines),
                keys,
                &host_aliases,
                &mut notes,
            )),
            _ => blended.extend(section.lines.iter().map(|line| line.to_string())),
        }
    }
    let mut warnings: Vec<String> = notes.warnings().collect();
    // 📦 files the host plays with that the joiner may need too, by name, since a path of the
    // host's names its own disk
    // (UZDoom's own entries start with a `$` for a dir of its own, like `$PROGDIR`, so with
    // `skip_defaults` those aren't counted)
    let missing = |pick: fn(&str) -> bool, skip_defaults: bool| -> Vec<String> {
        let own_names: Vec<String> = paths(&own, pick)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let mut names: Vec<String> = Vec::new();
        for (name, path) in paths(&host, pick) {
            if (skip_defaults && path.starts_with('$')) || own_names.contains(&name) {
                continue;
            }
            let shown = path.rsplit(['/', '\\']).next().unwrap_or(path).to_string();
            if !names.contains(&shown) {
                names.push(shown);
            }
        }
        names
    };
    let autoloaded = missing(|name| name.to_ascii_lowercase().contains("autoload"), false);
    if !autoloaded.is_empty() {
        warnings.push(format!(
            "The host autoloads {}, which you don't: if the game needs them, load them yourself",
            listing(&autoloaded)
        ));
    }
    let autoexec = missing(|name| name.to_ascii_lowercase().ends_with("autoexec"), true);
    if !autoexec.is_empty() {
        warnings.push(format!(
            "The host runs {} at startup, which doesn't come with its ini",
            listing(&autoexec)
        ));
    }
    let mut text = blended.join("\n");
    text.push('\n');
    Blended { text, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loadout::scribe::tests::Altar;

    #[test]
    fn the_profiles_own_ini_is_the_one_in_use_if_it_is_there() {
        let altar = Altar::new("ini-in-use");
        let ini = altar.file("uzdoom.ini");
        fs::write(&ini, b"[Doom.Player]").unwrap();
        let uzdoom = OsStr::new("/usr/bin/uzdoom");
        assert_eq!(in_use(Some(ini.as_os_str()), uzdoom), Some(ini.clone()));
        // a missing one isn't swapped for UZDoom's usual one, since it isn't what UZDoom plays with
        assert_eq!(
            in_use(Some(altar.file("gone.ini").as_os_str()), uzdoom),
            None
        );
    }

    const HOST: &str = "\
# This file was generated by UZDoom
[GlobalSettings]
vid_fullscreen=true
gl_texture_filter=6
fluid_patchset=/home/host/sf2/big.sf2

[Doom.Player]
name=Host
color=ff 00 00

[Doom.ConsoleVariables]
sv_cheats=false
m_sensitivity_x=4
save_dir=/home/host/saves
cl_bloodtype=2

[Doom.Bindings]
w=+forward
f12=logfile /home/joiner/.bashrc
g=+user1
h=netevent dash
k=myalias

[Doom.ConsoleAliases]
Name=myalias
Command=echo hi

[Doom.AutoExec]
Path=$PROGDIR/autoexec.cfg
Path=/home/host/autoexec.cfg

[doom.doom2.Autoload]
Path=/home/host/brightmaps.pk3

[FileSearch.Directories]
Path=/home/host/wads
";

    #[test]
    fn the_hosts_settings_come_without_its_files_commands_or_personal_ones() {
        let blended = blend(HOST, None, false).text;
        assert!(blended.contains("gl_texture_filter=6"));
        assert!(blended.contains("sv_cheats=false"));
        assert!(blended.contains("cl_bloodtype=2"));
        assert!(blended.starts_with("# This file was generated by UZDoom"));
        for left_out in [
            "vid_fullscreen",
            "fluid_patchset",
            "m_sensitivity_x",
            "save_dir",
            "name=Host",
            "logfile",
            "[Doom.Bindings]",
            "myalias",
            "autoexec.cfg",
            "brightmaps.pk3",
            "/home/host/wads",
        ] {
            assert!(!blended.contains(left_out), "{left_out}");
        }
    }

    #[test]
    fn the_joiners_own_player_controls_and_screen_go_in_their_place() {
        let own = "\
[GlobalSettings]
vid_fullscreen=false
gl_texture_filter=0

[doom.player]
name=Joiner

[Doom.ConsoleVariables]
m_sensitivity_x=2
cl_bloodtype=0

[Doom.Bindings]
w=+forward
";
        let blended = blend(HOST, Some(own), false).text;
        // the host's settings win, and the joiner's own business is theirs
        assert!(blended.contains("gl_texture_filter=6"));
        assert!(!blended.contains("gl_texture_filter=0"));
        assert!(blended.contains("cl_bloodtype=2"));
        assert!(blended.contains("vid_fullscreen=false"));
        assert!(blended.contains("m_sensitivity_x=2"));
        assert!(blended.contains("[doom.player]\nname=Joiner"));
        assert!(blended.contains("[Doom.Bindings]\nw=+forward"));
        assert!(!blended.contains("name=Host") && !blended.contains("logfile"));
        // and each setting is under the section it belongs to
        let global = blended.split("[Doom.ConsoleVariables]").next().unwrap();
        assert!(global.contains("vid_fullscreen=false"));
        assert!(!global.contains("m_sensitivity_x"));
    }

    #[test]
    fn a_joiner_takes_the_hosts_shared_ini_with_their_own_player_or_says_there_is_none() {
        use crate::net::{
            Manifest,
            tests::{CODE, local},
        };
        let altar = Altar::new("ini-take");
        let own = altar.file("own.ini");
        fs::write(&own, "[Doom.Player]\nname=Joiner\n").unwrap();
        let dir = altar.file("data/joined");
        let manifest = || Manifest::build(&[], String::new()).unwrap();

        let sharing = manifest()
            .serve(0, CODE.into(), Vec::new(), Some(HOST.as_bytes().to_vec()))
            .unwrap();
        let taken = take(&local(sharing.port, CODE), Some(&own), true, &dir).unwrap();
        assert_eq!(taken.map(|taken| taken.path), Some(dir.join("host.ini")));
        let written = fs::read_to_string(dir.join("host.ini")).unwrap();
        assert!(written.contains("cl_bloodtype=2") && written.contains("name=Joiner"));

        let keeping = manifest().serve(0, CODE.into(), Vec::new(), None).unwrap();
        assert_eq!(
            take(&local(keeping.port, CODE), Some(&own), true, &dir),
            Ok(None)
        );
    }

    /// A joiner's ini with a key of their own on G, and W bound as the host has it.
    const OWN: &str = "\
[Doom.Bindings]
w=+forward
g=+jump

[doom.doom2.Autoload]
Path=/home/joiner/mods/brightmaps.pk3
";

    #[test]
    fn the_hosts_keys_go_only_on_keys_that_are_free_and_never_risky_ones() {
        let blended = blend(HOST, Some(OWN), true);
        let keys = blended.text.split("[Doom.Bindings]").nth(1).unwrap();
        assert!(keys.contains("w=+forward") && keys.contains("g=+jump"));
        assert!(keys.contains("h=netevent dash"));
        // G is the joiner's, the log could be written anywhere, and the alias isn't brought over
        for left_out in ["+user1", "logfile", "myalias"] {
            assert!(!blended.text.contains(left_out), "{left_out}");
        }
        let warnings = blended.warnings.join("\n");
        assert!(warnings.contains("yours for something else, so they weren't added: G (+user1)"));
        assert!(warnings.contains("reach files or rebind keys: F12 (logfile"));
        assert!(warnings.contains("its own aliases, which don't come with its ini: K (myalias)"));
        assert!(!warnings.contains("no key for"));
    }

    #[test]
    fn without_the_hosts_keys_the_ones_you_have_none_like_are_said() {
        let blended = blend(HOST, Some(OWN), false);
        assert!(!blended.text.contains("netevent"));
        let warnings = blended.warnings.join("\n");
        assert!(warnings.contains("things you have no key for: H (netevent dash)"));
        // G only ever goes in with the keys, where it would be said
        assert!(!warnings.contains("+user1"));
    }

    #[test]
    fn a_joiner_with_no_ini_takes_the_hosts_keys_but_the_risky_ones() {
        let blended = blend(HOST, None, true);
        let keys = blended.text.split("[Doom.Bindings]").nth(1).unwrap();
        assert!(keys.contains("w=+forward") && keys.contains("g=+user1"));
        assert!(!keys.contains("logfile") && !keys.contains("myalias"));
        // with no keys of their own to compare, nothing is said about the ones left at home
        assert!(
            blend(HOST, None, false)
                .warnings
                .iter()
                .all(|w| !w.contains("key"))
        );
    }

    #[test]
    fn files_the_host_autoloads_or_runs_that_you_dont_are_said() {
        let warnings = blend(HOST, None, false).warnings.join("\n");
        assert!(warnings.contains("The host autoloads brightmaps.pk3"));
        assert!(warnings.contains("The host runs autoexec.cfg at startup"));
        // UZDoom's own autoexec entry, which every joiner has, isn't news
        assert_eq!(warnings.matches("autoexec.cfg").count(), 1);
        let warnings = blend(HOST, Some(OWN), false).warnings.join("\n");
        assert!(!warnings.contains("autoloads"), "the joiner loads it too");
    }

    #[test]
    fn a_bound_command_that_could_reach_a_file_is_risky_however_it_hides() {
        for risky in [
            "logfile /tmp/x",
            "+use; condump out.txt",
            "bind q \"exec evil.cfg\"",
            "alias a b",
            "set screenshot_dir /etc",
            "echo $name",
        ] {
            assert!(is_risky_command(risky), "{risky}");
        }
        for fine in [
            "+user1",
            "netevent dash",
            "+use; +attack",
            "openmenu ModOptions",
        ] {
            assert!(!is_risky_command(fine), "{fine}");
        }
    }

    #[test]
    fn an_ini_too_big_is_not_served() {
        let altar = Altar::new("ini-too-big");
        let ini = altar.file("uzdoom.ini");
        fs::write(&ini, b"[Doom.Player]\nname=Player\n").unwrap();
        assert_eq!(read(&ini).unwrap(), b"[Doom.Player]\nname=Player\n");
        fs::write(&ini, vec![b'x'; MAX_INI as usize + 1]).unwrap();
        assert!(read(&ini).is_err());
    }
}
