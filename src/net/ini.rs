//! The ini a host plays with, served at `/<code>/ini` when the host shares it, and what a joiner
//! who plays with it makes of it.
//!
//! The host's ini comes from someone else, so not all of it is taken: whatever names files or
//! dirs (wads to load, scripts to run, where to look or write) and whatever runs commands (key
//! bindings, aliases) is left out, and so is what is the joiner's own business (their player,
//! their screen, sound and controls). The joiner's own ini fills those in, if they have one.

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

/// What the joiner plays with, for a host that shares its ini: it is fetched, [`blend`]ed with
/// `own` (the ini the joiner would play with, if there is one) and written to `host.ini` in `dir`,
/// over the one from last time. Where it was written, or `None` if the host shares none, or
/// what went wrong, to be said.
pub fn take(
    address: &Address,
    own: Option<&Path>,
    dir: &Path,
) -> std::result::Result<Option<PathBuf>, String> {
    let host = match fetch(address) {
        Ok(Some(host)) => host,
        Ok(None) => return Ok(None),
        Err(_) => return Err("Couldn't get the host's ini".to_string()),
    };
    let own = own
        .and_then(|own| read(own).ok())
        .map(|own| String::from_utf8_lossy(&own).into_owned());
    let path = dir.join("host.ini");
    fs::create_dir_all(dir)
        .and_then(|()| fs::write(&path, blend(&host, own.as_deref())))
        .map_err(|error| format!("Could not save the host's ini: {error}"))?;
    Ok(Some(path))
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
    PREFIXES.iter().any(|prefix| key.starts_with(prefix))
        || SUFFIXES.iter().any(|suffix| key.ends_with(suffix))
}

/// The ini to join with: the `host`'s settings, with everything personal (see the module's
/// notes) taken from `own`, the joiner's ini, if they have one, or else left for UZDoom to fill in
/// with its defaults.
pub fn blend(host: &str, own: Option<&str>) -> String {
    let own = own.map(sections).unwrap_or_default();
    let own_section = |name: &str| {
        own.iter().find(|section| {
            section
                .name
                .is_some_and(|own| own.eq_ignore_ascii_case(name))
        })
    };
    let mut blended = Vec::new();
    for section in sections(host) {
        if let Some(name) = section.name {
            if is_personal_section(name) {
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
        if let Some(name) = section.name.filter(|name| is_personal_section(name)) {
            blended.push(format!("[{name}]"));
            blended.extend(section.lines.iter().map(|line| line.to_string()));
        }
    }
    let mut text = blended.join("\n");
    text.push('\n');
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

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

[Doom.AutoExec]
Path=/home/host/autoexec.cfg

[doom.doom2.Autoload]
Path=/home/host/brightmaps.pk3

[FileSearch.Directories]
Path=/home/host/wads
";

    #[test]
    fn the_hosts_settings_come_without_its_files_commands_or_personal_ones() {
        let blended = blend(HOST, None);
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
        let blended = blend(HOST, Some(own));
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
        let taken = take(&local(sharing.port, CODE), Some(&own), &dir).unwrap();
        assert_eq!(taken, Some(dir.join("host.ini")));
        let written = fs::read_to_string(dir.join("host.ini")).unwrap();
        assert!(written.contains("cl_bloodtype=2") && written.contains("name=Joiner"));

        let keeping = manifest().serve(0, CODE.into(), Vec::new(), None).unwrap();
        assert_eq!(take(&local(keeping.port, CODE), Some(&own), &dir), Ok(None));
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
