use std::{
    ffi::OsString,
    io,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    vault::config::Config,
    vault::edition::Edition,
    vault::scribe,
    vault::wad::{Fault, Wad},
};

/// The profiles folder inside the config folder. The dot keeps it from ever matching a profile's
/// name (names can't contain one), which matters where the config and data folders are the same.
pub const SUBDIRECTORY: &str = ".profiles";

/// A named, ordered list of wads (load order matters) plus optional per-profile overrides.
///
/// Stored as `<config>/.profiles/<name>.rite` (MessagePack); saves live in `<data>/<name>/saves`.
/// The name is also the file name, so it is restricted to `A-Z a-z 0-9 _`.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub wads: Vec<Wad>,
    /// Overrides [`Config::base_wad`].
    pub base_wad: Option<Wad>,
    /// Overrides [`Config::uzdoom`].
    pub uzdoom: Option<OsString>,
    pub ini: Option<OsString>,
}

/// Which piece of a profile a [`Fault`] was found in.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Part {
    /// The wad at this position in the list.
    Wad(usize),
    BaseWad,
    Uzdoom,
    Ini,
}

impl Edition for Profile {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1];
}

/// Edition 1 only wrapped the same data in a named, versioned map, so nothing needs fixing.
fn v0_to_v1(profile: Profile) -> Profile {
    profile
}

impl Profile {
    /// A profile with no overrides, as created when downloading from a host.
    pub fn new(name: String, wads: Vec<Wad>) -> Self {
        Self {
            name,
            wads,
            base_wad: None,
            uzdoom: None,
            ini: None,
        }
    }

    /// The profile's own UZDoom path, falling back to the global one.
    pub fn uzdoom<'a>(&'a self, config: &'a Config) -> Option<&'a OsString> {
        self.uzdoom.as_ref().or(config.uzdoom.as_ref())
    }

    /// Everything wrong with the files this profile needs to run, UZDoom first. The UZDoom path,
    /// base wad and ini are the effective ones, so the global settings count too. This only looks
    /// at the disk; nothing is hashed.
    pub fn faults(&self, config: &Config) -> Vec<(Part, Fault)> {
        let uzdoom = self
            .uzdoom(config)
            .and_then(|path| Fault::of(Path::new(path), 0))
            .map(|fault| (Part::Uzdoom, fault));
        let base_wad = self
            .base_wad(config)
            .and_then(Wad::check)
            .map(|fault| (Part::BaseWad, fault));
        let ini = self
            .ini
            .as_ref()
            .and_then(|path| Fault::of(Path::new(path), 0))
            .map(|fault| (Part::Ini, fault));
        let wads = self
            .wads
            .iter()
            .enumerate()
            .filter_map(|(i, wad)| wad.check().map(|fault| (Part::Wad(i), fault)));
        uzdoom
            .into_iter()
            .chain(base_wad)
            .chain(ini)
            .chain(wads)
            .collect()
    }

    /// The profile's own base wad, falling back to the global one.
    pub fn base_wad<'a>(&'a self, config: &'a Config) -> Option<&'a Wad> {
        self.base_wad.as_ref().or(config.base_wad.as_ref())
    }

    /// Where the profile called `name` is kept.
    pub fn path(config_dir: &Path, name: &str) -> PathBuf {
        config_dir
            .join(SUBDIRECTORY)
            .join(format!("{name}.{}", scribe::EXTENSION))
    }

    /// Writes the profile file and makes sure its `saves` folder exists.
    pub fn write(&self, config_dir: &Path, data_dir: &Path) -> io::Result<()> {
        self.save(&Self::path(config_dir, &self.name))?;
        self.create_saves_dir(data_dir)
    }

    /// UZDoom's save games for this profile live here.
    pub fn create_saves_dir(&self, data_dir: &Path) -> io::Result<()> {
        std::fs::create_dir_all(data_dir.join(&self.name).join("saves"))
    }

    /// Does nothing for an empty name, which would otherwise resolve to the whole data directory.
    pub fn remove(&self, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::remove_dir_all(data_dir.join(&self.name));
        scribe::burn(&Self::path(config_dir, &self.name));
    }

    /// After the data folder moved from `old_name` to this profile's name, an ini that lived inside
    /// it moved too, so its path has to follow.
    pub fn follow_rename(&mut self, data_dir: &Path, old_name: &str) {
        let moved = self
            .ini
            .as_ref()
            .and_then(|ini| Path::new(ini).strip_prefix(data_dir.join(old_name)).ok())
            .map(|rest| data_dir.join(&self.name).join(rest));
        if let Some(moved) = moved {
            self.ini = Some(moved.into());
        }
    }

    /// Moves the saves and deletes the old file; the caller then writes the profile under its new name.
    pub fn rename(&self, new_name: &str, config_dir: &Path, data_dir: &Path) {
        if self.name.is_empty() {
            return;
        }
        let _ = std::fs::rename(data_dir.join(&self.name), data_dir.join(new_name));
        scribe::burn(&Self::path(config_dir, &self.name));
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::vault::scribe::tests::Altar;

    fn wad(altar: &Altar, name: &str) -> Wad {
        fs::write(altar.file(name), b"1234").unwrap();
        Wad::from_path(&altar.file(name)).unwrap()
    }

    #[test]
    fn a_healthy_profile_has_no_faults() {
        let altar = Altar::new("profile-healthy");
        fs::write(altar.file("uzdoom"), b"").unwrap();
        fs::write(altar.file("a.ini"), b"").unwrap();
        let profile = Profile {
            wads: vec![wad(&altar, "a.wad"), wad(&altar, "b.pk3")],
            base_wad: Some(wad(&altar, "base.wad")),
            uzdoom: Some(altar.file("uzdoom").into()),
            ini: Some(altar.file("a.ini").into()),
            ..Profile::default()
        };
        assert_eq!(profile.faults(&Config::default()), []);
    }

    #[test]
    fn every_part_reports_its_own_fault_with_uzdoom_first() {
        let altar = Altar::new("profile-faults");
        let mut profile = Profile {
            wads: vec![wad(&altar, "a.wad"), wad(&altar, "b.pk3")],
            base_wad: Some(wad(&altar, "base.wad")),
            uzdoom: Some(altar.file("no-uzdoom").into()),
            ini: Some(altar.file("no.ini").into()),
            ..Profile::default()
        };
        fs::remove_file(altar.file("b.pk3")).unwrap();
        fs::remove_file(altar.file("base.wad")).unwrap();
        fs::write(altar.file("a.wad"), b"123456789").unwrap();
        let parts: Vec<_> = profile.faults(&Config::default());
        assert_eq!(
            parts,
            [
                (Part::Uzdoom, Fault::Missing),
                (Part::BaseWad, Fault::Missing),
                (Part::Ini, Fault::Missing),
                (
                    Part::Wad(0),
                    Fault::Changed {
                        expected: 4,
                        found: 9
                    }
                ),
                (Part::Wad(1), Fault::Missing),
            ]
        );
        // only the changed wad is non-blocking
        assert_eq!(parts.iter().filter(|(_, f)| f.is_blocking()).count(), 4);
        profile.wads.clear();
    }

    #[test]
    fn the_global_settings_count_when_the_profile_has_no_overrides() {
        let altar = Altar::new("profile-global");
        let config = Config {
            uzdoom: Some(altar.file("nowhere").into()),
            ..Config::default()
        };
        assert_eq!(
            Profile::default().faults(&config),
            [(Part::Uzdoom, Fault::Missing)]
        );
        // no UZDoom chosen at all is not a fault; that has its own warning
        assert_eq!(Profile::default().faults(&Config::default()), []);
    }

    #[test]
    fn an_ini_inside_the_data_folder_follows_a_rename() {
        let data = Path::new("/data");
        let mut profile = Profile {
            name: "new".into(),
            ini: Some("/data/old/uzdoom.ini".into()),
            ..Profile::default()
        };
        profile.follow_rename(data, "old");
        assert_eq!(profile.ini, Some("/data/new/uzdoom.ini".into()));
    }

    #[test]
    fn an_ini_somewhere_else_stays_where_it_is() {
        let mut profile = Profile {
            name: "new".into(),
            ini: Some("/home/me/uzdoom.ini".into()),
            ..Profile::default()
        };
        profile.follow_rename(Path::new("/data"), "old");
        assert_eq!(profile.ini, Some("/home/me/uzdoom.ini".into()));
        // and no ini stays no ini
        let mut none = Profile::default();
        none.follow_rename(Path::new("/data"), "old");
        assert_eq!(none.ini, None);
    }
}
