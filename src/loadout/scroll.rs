//! `.scroll` files: profiles to share with someone else, one or a whole loadout. Each wad is only
//! its file name, size and hash, with no paths, since paths only mean something on the machine
//! that wrote them. Breaking the seal (importing) finds each wad again by joining its name onto the
//! wad dirs.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    loadout::{
        edition::Edition,
        hash,
        host_settings::HostSettings,
        profile::Profile,
        uac::Shared,
        wad::{self, Wad},
    },
    net,
    tally::Tally,
};

/// What a scroll's file name ends in, so it is never mistaken for a profile's `.rite`.
pub const EXTENSION: &str = "scroll";
/// The most wads a scroll may hold, across all its profiles. Far more than the network's
/// [`net::MAX_FILES`], since a scroll is only read from disk, but still a bound, since it comes
/// from someone else.
pub const MAX_WADS: usize = 4096;
/// The most profiles a scroll may hold, for the same reason.
pub const MAX_PROFILES: usize = 512;

/// Profiles to share, in order, and, if their maker chose to, their global settings.
///
/// Saved as `{profiles, settings}`. Scrolls of edition 0 (tdl 0.6.0), which held one profile's
/// fields at the top level, are legacy: `upgrade_legacy.py --scroll` upgrades them.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Scroll {
    pub profiles: Vec<SealedProfile>,
    /// The maker's music and quip settings, if they chose to share them. Never their song.
    #[serde(default)]
    pub settings: Option<Shared>,
}

/// One profile on a scroll: its name, its wads in load order, its base wad and its host defaults.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct SealedProfile {
    pub name: String,
    pub wads: Vec<Inscribed>,
    #[serde(default)]
    pub base_wad: Option<Inscribed>,
    /// Shared without the join code, which is the host's password.
    #[serde(default)]
    pub host_defaults: Option<HostSettings>,
}

/// One wad on a scroll.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Inscribed {
    pub name: String,
    /// 0 if the profile never recorded it, in which case only the hash is checked.
    pub size: u64,
    pub hash: String,
    /// A wad still being worked on: found by its name alone, since its size and hash are expected
    /// to have moved on.
    #[serde(default)]
    pub volatile: bool,
    /// A mod's folder: found by its name alone, as a folder, and with no size or hash (both stay
    /// empty).
    #[serde(default)]
    pub folder: bool,
}

impl Edition for Scroll {
    /// Edition 1 holds a list of profiles, as tdl 1.0.0 writes it.
    const FIRST: u32 = 1;
    const UPGRADES: &[fn(Self) -> Self] = &[];
}

/// What breaking the seal turned up: each profile with its wads in load order, each wad found or
/// a placeholder.
#[derive(Debug, PartialEq)]
pub struct Unsealed {
    pub profiles: Vec<UnsealedProfile>,
    /// How many of the wads (base wads included) weren't found, and are placeholders.
    pub missing: usize,
}

/// One profile off a scroll, its wads looked for, ready to be named and added.
#[derive(Debug, PartialEq)]
pub struct UnsealedProfile {
    pub name: String,
    pub wads: Vec<Wad>,
    pub base_wad: Option<Wad>,
    pub host_defaults: Option<HostSettings>,
}

impl SealedProfile {
    /// `profile` as it goes on a scroll. Fails, naming the profile and the wad, if a wad has no
    /// hash to share, since whoever reads the scroll would have no way to know they found the
    /// right file. A folder is shared by its name, since it has no hash at all.
    pub fn of(profile: &Profile) -> std::result::Result<Self, String> {
        let inscribe = |wad: &Wad| {
            if wad.folder {
                return Ok(Inscribed {
                    name: wad.file_name().into_owned(),
                    size: 0,
                    hash: String::new(),
                    volatile: false,
                    folder: true,
                });
            }
            if wad.hash.is_empty() {
                return Err(format!(
                    "{} in '{}' has no hash to share: pick it again with R in the editor",
                    wad.file_name(),
                    profile.name
                ));
            }
            Ok(Inscribed {
                name: wad.file_name().into_owned(),
                size: wad.size,
                hash: wad.hash.clone(),
                volatile: wad.volatile,
                folder: false,
            })
        };
        Ok(Self {
            name: profile.name.clone(),
            wads: profile
                .wads
                .iter()
                .map(inscribe)
                .collect::<std::result::Result<_, _>>()?,
            base_wad: profile.base_wad.as_ref().map(inscribe).transpose()?,
            // 🔒 the code is the host's secret, and sharing an ini is for its owner to choose
            host_defaults: profile.host_defaults.clone().map(|defaults| HostSettings {
                code: String::new(),
                share_ini: false,
                ..defaults
            }),
        })
    }

    /// The wads to look for: the list, then the base wad.
    fn all_wads(&self) -> impl Iterator<Item = &Inscribed> {
        self.wads.iter().chain(&self.base_wad)
    }
}

impl Scroll {
    /// The scroll for `profiles`, in their order, with no settings. Fails if a wad has no hash
    /// (see [`SealedProfile::of`]).
    pub fn of<'a>(
        profiles: impl IntoIterator<Item = &'a Profile>,
    ) -> std::result::Result<Self, String> {
        Ok(Self {
            profiles: profiles
                .into_iter()
                .map(SealedProfile::of)
                .collect::<std::result::Result<_, _>>()?,
            settings: None,
        })
    }

    /// How many wads (base wads included) breaking the seal looks for.
    pub fn wad_count(&self) -> usize {
        self.profiles
            .iter()
            .map(|profile| profile.all_wads().count())
            .sum()
    }

    /// Whether a scroll from someone else is safe to break: not too many profiles or wads, every
    /// profile name one tdl could have made (it becomes a file and a dir name), every wad name a
    /// bare file name and every hash a real one, the same rule as a host's manifest.
    pub fn check(&self) -> std::result::Result<(), &'static str> {
        if self.profiles.is_empty() {
            return Err("The scroll has no profiles on it");
        }
        if self.profiles.len() > MAX_PROFILES {
            return Err("The scroll lists too many profiles");
        }
        if self.wad_count() > MAX_WADS {
            return Err("The scroll lists too many wads");
        }
        if !self
            .profiles
            .iter()
            .all(|profile| is_profile_name(&profile.name))
        {
            return Err("The scroll has a profile whose name tdl can't use");
        }
        if !self
            .profiles
            .iter()
            .flat_map(SealedProfile::all_wads)
            .all(Inscribed::is_safe)
        {
            return Err("The scroll lists a wad with a name or hash that isn't safe");
        }
        if let Some(settings) = &self.settings {
            settings.check()?;
        }
        Ok(())
    }

    /// Finds each wad by joining its name onto `dirs`, in order. A file is taken only if its
    /// size matches and then its hash does (a volatile wad only needs its name, and a folder only a
    /// folder of its name). Anything not found
    /// becomes a placeholder in the first dir, so the editor can say where it was looked for.
    ///
    /// `tally` hears about every wad as it is checked, then about the bytes of each file hashed.
    /// `dirs` must not be empty, since there would be nowhere for a placeholder to point.
    pub fn unseal(&self, dirs: &[PathBuf], tally: &mut impl Tally) -> Result<Unsealed> {
        let mut missing = 0;
        let mut find = |wad: &Inscribed| -> Result<Wad> {
            tally.count_file(OsStr::new(&wad.name), wad.size)?;
            if wad.folder {
                let found = dirs
                    .iter()
                    .map(|dir| dir.join(&wad.name))
                    .find(|path| path.is_dir());
                missing += usize::from(found.is_none());
                return Ok(Wad::folder(
                    &found.unwrap_or_else(|| dirs[0].join(&wad.name)),
                ));
            }
            for dir in dirs {
                if let Some(found) = check(&dir.join(&wad.name), wad, tally)? {
                    return Ok(found);
                }
            }
            missing += 1;
            Ok(Wad {
                volatile: wad.volatile,
                ..Wad::new(
                    dirs[0].join(&wad.name).into_os_string(),
                    wad.hash.clone(),
                    wad.size,
                )
            })
        };
        let mut profiles = Vec::with_capacity(self.profiles.len());
        for profile in &self.profiles {
            profiles.push(UnsealedProfile {
                name: profile.name.clone(),
                wads: profile.wads.iter().map(&mut find).collect::<Result<_>>()?,
                base_wad: profile.base_wad.as_ref().map(&mut find).transpose()?,
                host_defaults: profile.host_defaults.clone(),
            });
        }
        Ok(Unsealed { profiles, missing })
    }

    /// The file name a scroll called `name` gets by default: `<name>.scroll`.
    pub fn file_name(name: &str) -> String {
        format!("{name}.{EXTENSION}")
    }
}

impl Inscribed {
    /// Whether the name is a bare file name and the hash a real one, or for a folder, empty.
    fn is_safe(&self) -> bool {
        if self.folder {
            net::is_safe_name(&self.name) && self.hash.is_empty()
        } else {
            net::is_safe(&self.name, &self.hash)
        }
    }
}

/// Whether `name` is one a profile could have (see [`super::profile::is_valid_name`]), since it
/// becomes a file and a dir name.
fn is_profile_name(name: &str) -> bool {
    super::profile::is_valid_name(name)
}

/// `path`, if it is `wad`: the right size (unless the size is unknown) and then the right hash.
/// A volatile wad is taken by name, and hashed so the profile records what the file is now.
/// A file that can't be read just isn't it; only stopping (Esc, Ctrl+C) is an error.
fn check(path: &Path, wad: &Inscribed, tally: &mut impl Tally) -> Result<Option<Wad>> {
    let Ok(metadata) = fs::metadata(path) else {
        return Ok(None);
    };
    if !metadata.is_file() || (!wad.volatile && wad.size != 0 && metadata.len() != wad.size) {
        return Ok(None);
    }
    let hash = match hash::hash(path, tally) {
        Ok(hash) => hash,
        Err(error @ (Error::Cancelled | Error::Interrupted)) => return Err(error),
        Err(_) => return Ok(None),
    };
    if !wad.volatile && hash != wad.hash {
        return Ok(None);
    }
    Ok(Some(Wad {
        volatile: wad.volatile,
        modified: wad::modified(&metadata),
        ..Wad::new(path.as_os_str().into(), hash, metadata.len())
    }))
}
