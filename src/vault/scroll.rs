//! `.scroll` files: profiles to share with someone else, one or a whole vault. Each wad is only
//! its file name, size and hash, with no paths, since paths only mean something on the machine
//! that wrote them. Breaking the seal (importing) finds each wad again by joining its name onto the
//! wad folders.

use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    error::{Error, Result},
    net,
    tally::Tally,
    vault::{
        edition::Edition,
        global::Shared,
        hash,
        host_settings::HostSettings,
        profile::Profile,
        wad::{self, Wad},
    },
};

/// What a scroll's file name ends in, so it is never mistaken for a profile's `.rite`.
pub const EXTENSION: &str = "scroll";
/// The most wads a scroll may hold, across all its profiles. Far more than the network's
/// [`net::MAX_FILES`], since a scroll is only read from disk, but still a bound, since it comes
/// from someone else.
pub const MAX_WADS: usize = 4096;
/// The most profiles a scroll may hold, for the same reason.
pub const MAX_PROFILES: usize = 512;
/// The longest name a profile on a scroll may have.
const MAX_NAME: usize = 64;

/// Profiles to share, in order, and, if their maker chose to, their global settings.
///
/// Saved as `{profiles, settings}`. Scrolls from before several profiles could be shared held one
/// profile's fields at the top level; they still read, as a scroll of that one profile.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(from = "SavedScroll")]
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

/// A scroll as it is read. Untagged, so a scroll of edition 0, which was one profile with no
/// `profiles` list, lands in [`SavedScroll::One`].
#[derive(Deserialize)]
#[serde(untagged)]
enum SavedScroll {
    Many {
        profiles: Vec<SealedProfile>,
        #[serde(default)]
        settings: Option<Shared>,
    },
    One {
        #[serde(flatten)]
        profile: SealedProfile,
        #[serde(default)]
        settings: Option<Shared>,
    },
}

impl From<SavedScroll> for Scroll {
    fn from(saved: SavedScroll) -> Self {
        match saved {
            SavedScroll::Many { profiles, settings } => Self { profiles, settings },
            SavedScroll::One { profile, settings } => Self {
                profiles: vec![profile],
                settings,
            },
        }
    }
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
}

impl Edition for Scroll {
    const UPGRADES: &[fn(Self) -> Self] = &[v0_to_v1];
}

/// Edition 1 holds a list of profiles. A scroll of edition 0, one profile, reads as a list of one
/// already (see [`SavedScroll`]); the new edition number is what keeps older builds, which only
/// know one profile, from reading a scroll of several as nothing.
fn v0_to_v1(scroll: Scroll) -> Scroll {
    scroll
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
    /// right file.
    pub fn of(profile: &Profile) -> std::result::Result<Self, String> {
        let inscribe = |wad: &Wad| {
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
            host_defaults: profile.host_defaults.clone().map(|defaults| HostSettings {
                code: String::new(),
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
    /// profile name one tdl could have made (it becomes a file and a folder name), every wad name a
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
            .all(|wad| net::is_safe(&wad.name, &wad.hash))
        {
            return Err("The scroll lists a wad with a name or hash that isn't safe");
        }
        if let Some(settings) = &self.settings {
            settings.check()?;
        }
        Ok(())
    }

    /// Finds each wad by joining its name onto `folders`, in order. A file is taken only if its
    /// size matches and then its hash does (a volatile wad only needs its name). Anything not found
    /// becomes a placeholder in the first folder, so the editor can say where it was looked for.
    ///
    /// `tally` hears about every wad as it is checked, then about the bytes of each file hashed.
    /// `folders` must not be empty, since there would be nowhere for a placeholder to point.
    pub fn unseal(&self, folders: &[PathBuf], tally: &mut impl Tally) -> Result<Unsealed> {
        let mut missing = 0;
        let mut find = |wad: &Inscribed| -> Result<Wad> {
            tally.count_file(OsStr::new(&wad.name), wad.size)?;
            for folder in folders {
                if let Some(found) = check(&folder.join(&wad.name), wad, tally)? {
                    return Ok(found);
                }
            }
            missing += 1;
            Ok(Wad {
                volatile: wad.volatile,
                ..Wad::new(
                    folders[0].join(&wad.name).into_os_string(),
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

/// Whether `name` is one a profile could have: not empty, not too long, and only the characters
/// a profile's name may hold, since it becomes a file and a folder name.
fn is_profile_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_NAME
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vault::scribe::tests::Altar;

    /// A wad on a scroll for `contents`, as if written on another machine.
    fn inscribed(name: &str, contents: &[u8]) -> Inscribed {
        Inscribed {
            name: name.into(),
            size: contents.len() as u64,
            hash: blake3::hash(contents).to_hex().to_string(),
            volatile: false,
        }
    }

    /// A profile on a scroll called `name`, with these wads and no base wad.
    fn sealed(name: &str, wads: Vec<Inscribed>) -> SealedProfile {
        SealedProfile {
            name: name.into(),
            wads,
            base_wad: None,
            host_defaults: None,
        }
    }

    /// A scroll of one profile with these wads.
    fn scroll(wads: Vec<Inscribed>) -> Scroll {
        Scroll {
            profiles: vec![sealed("shared", wads)],
            settings: None,
        }
    }

    /// Two wad folders in `altar`, made fresh.
    fn folders(altar: &Altar) -> [PathBuf; 2] {
        let folders = [altar.file("first"), altar.file("second")];
        for folder in &folders {
            fs::create_dir_all(folder).unwrap();
        }
        folders
    }

    #[test]
    fn a_wad_in_a_folder_is_found() {
        let altar = Altar::new("scroll-found");
        let [first, _] = folders(&altar);
        fs::write(first.join("a.wad"), b"aaaa").unwrap();
        let unsealed = scroll(vec![inscribed("a.wad", b"aaaa")])
            .unseal(std::slice::from_ref(&first), &mut ())
            .unwrap();
        assert_eq!(unsealed.missing, 0);
        assert_eq!(
            unsealed.profiles[0].wads[0].path,
            first.join("a.wad").into_os_string()
        );
        assert_ne!(unsealed.profiles[0].wads[0].modified, 0);
    }

    #[test]
    fn a_file_of_the_wrong_size_is_not_even_hashed() {
        let altar = Altar::new("scroll-wrong-size");
        let [first, _] = folders(&altar);
        fs::write(first.join("a.wad"), b"aaaa-but-longer").unwrap();
        struct Hashed(u64);
        impl Tally for Hashed {
            fn count_file(&mut self, _: &OsStr, _: u64) -> Result<()> {
                Ok(())
            }
            fn count_bytes(&mut self, amount: u64) -> Result<()> {
                self.0 += amount;
                Ok(())
            }
        }
        let mut hashed = Hashed(0);
        let unsealed = scroll(vec![inscribed("a.wad", b"aaaa")])
            .unseal(&[first], &mut hashed)
            .unwrap();
        assert_eq!(unsealed.missing, 1);
        assert_eq!(hashed.0, 0, "no bytes were read");
    }

    #[test]
    fn a_file_of_the_right_size_but_the_wrong_hash_is_not_it() {
        let altar = Altar::new("scroll-wrong-hash");
        let [first, _] = folders(&altar);
        fs::write(first.join("a.wad"), b"AAAA").unwrap();
        let unsealed = scroll(vec![inscribed("a.wad", b"aaaa")])
            .unseal(&[first], &mut ())
            .unwrap();
        assert_eq!(unsealed.missing, 1);
    }

    #[test]
    fn a_wad_not_found_is_a_placeholder_in_the_first_folder() {
        let altar = Altar::new("scroll-not-found");
        let [first, second] = folders(&altar);
        let wad = inscribed("gone.wad", b"gone");
        let unsealed = scroll(vec![wad.clone()])
            .unseal(&[first.clone(), second], &mut ())
            .unwrap();
        assert_eq!(unsealed.missing, 1);
        let placeholder = &unsealed.profiles[0].wads[0];
        assert_eq!(placeholder.path, first.join("gone.wad").into_os_string());
        assert_eq!(
            (placeholder.hash.as_str(), placeholder.size),
            (wad.hash.as_str(), 4)
        );
        assert_eq!(placeholder.check(), Some(wad::Fault::Missing));
    }

    #[test]
    fn folders_are_tried_in_order_and_the_first_match_wins() {
        let altar = Altar::new("scroll-in-order");
        let [first, second] = folders(&altar);
        // the first folder has a file by that name, but not the right one
        fs::write(first.join("a.wad"), b"AAAA").unwrap();
        fs::write(second.join("a.wad"), b"aaaa").unwrap();
        fs::write(second.join("b.wad"), b"bbbb").unwrap();
        fs::write(first.join("b.wad"), b"bbbb").unwrap();
        let unsealed = scroll(vec![
            inscribed("a.wad", b"aaaa"),
            inscribed("b.wad", b"bbbb"),
        ])
        .unseal(&[first.clone(), second.clone()], &mut ())
        .unwrap();
        assert_eq!(
            unsealed.profiles[0].wads[0].path,
            second.join("a.wad").into_os_string()
        );
        assert_eq!(
            unsealed.profiles[0].wads[1].path,
            first.join("b.wad").into_os_string()
        );
    }

    #[test]
    fn a_volatile_wad_is_found_by_name_and_hashed_as_it_is_now() {
        let altar = Altar::new("scroll-volatile");
        let [first, _] = folders(&altar);
        fs::write(first.join("wip.pk3"), b"newer and bigger").unwrap();
        let wad = Inscribed {
            volatile: true,
            ..inscribed("wip.pk3", b"old")
        };
        let unsealed = scroll(vec![wad]).unseal(&[first], &mut ()).unwrap();
        assert_eq!(unsealed.missing, 0);
        assert!(unsealed.profiles[0].wads[0].volatile);
        assert_eq!(
            unsealed.profiles[0].wads[0].hash,
            blake3::hash(b"newer and bigger").to_hex().to_string()
        );
    }

    #[test]
    fn unsafe_names_and_hashes_are_refused() {
        let good = inscribed("a.wad", b"a");
        assert_eq!(scroll(vec![good.clone()]).check(), Ok(()));
        for bad in [
            Inscribed {
                name: "../../escape.wad".into(),
                ..good.clone()
            },
            Inscribed {
                name: "/etc/passwd".into(),
                ..good.clone()
            },
            Inscribed {
                name: "..".into(),
                ..good.clone()
            },
            Inscribed {
                hash: "not-a-hash".into(),
                ..good.clone()
            },
        ] {
            assert!(scroll(vec![bad.clone()]).check().is_err(), "{bad:?}");
        }
        // the base wad is held to the same rule
        let sneaky = Scroll {
            profiles: vec![SealedProfile {
                base_wad: Some(Inscribed {
                    name: "../DOOM2.WAD".into(),
                    ..good
                }),
                ..sealed("shared", vec![])
            }],
            settings: None,
        };
        assert!(sneaky.check().is_err());
    }

    #[test]
    fn settings_that_would_garble_the_screen_are_refused_with_the_scroll() {
        use crate::vault::global::SplashQuip;

        let garbled = Scroll {
            settings: Some(Shared {
                splash_quip: SplashQuip::Custom("\x07".into()),
                ..Shared::default()
            }),
            ..scroll(vec![])
        };
        assert!(garbled.check().is_err());
        let fine = Scroll {
            settings: Some(Shared::default()),
            ..scroll(vec![])
        };
        assert!(fine.check().is_ok());
    }

    /// How edition 0 saved a scroll: one profile's fields at the top, settings beside them.
    #[derive(Serialize)]
    struct EditionZero {
        name: String,
        wads: Vec<Inscribed>,
        base_wad: Option<Inscribed>,
        host_defaults: Option<HostSettings>,
        #[serde(skip_serializing_if = "Option::is_none")]
        settings: Option<Shared>,
    }

    #[derive(Serialize)]
    struct Wrapped<T> {
        version: u32,
        data: T,
    }

    fn write_edition_zero(path: &Path, old: EditionZero) {
        let bytes = rmp_serde::to_vec_named(&Wrapped {
            version: 0,
            data: old,
        })
        .unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn a_scroll_of_one_profile_from_edition_zero_reads_as_a_list_of_one() {
        let altar = Altar::new("scroll-edition-zero");
        let path = altar.file("old.scroll");
        let wad = inscribed("a.wad", b"a");
        write_edition_zero(
            &path,
            EditionZero {
                name: "old".into(),
                wads: vec![wad.clone()],
                base_wad: Some(inscribed("DOOM2.WAD", b"iwad")),
                host_defaults: Some(HostSettings::new(Some(9000))),
                settings: Some(Shared {
                    no_quips: true,
                    ..Shared::default()
                }),
            },
        );
        assert_eq!(Scroll::stored_version(&path), Some(0));
        let scroll = Scroll::open(&path).unwrap();
        assert_eq!(scroll.profiles.len(), 1);
        let profile = &scroll.profiles[0];
        assert_eq!(profile.name, "old");
        assert_eq!(profile.wads, [wad]);
        assert_eq!(profile.base_wad.as_ref().unwrap().name, "DOOM2.WAD");
        assert_eq!(profile.host_defaults.as_ref().unwrap().port, "9000");
        assert!(scroll.settings.unwrap().no_quips);
    }

    #[test]
    fn a_scroll_from_before_settings_could_be_shared_carries_none() {
        let altar = Altar::new("scroll-before-settings");
        let path = altar.file("old.scroll");
        write_edition_zero(
            &path,
            EditionZero {
                name: "old".into(),
                wads: Vec::new(),
                base_wad: None,
                host_defaults: None,
                settings: None,
            },
        );
        let scroll = Scroll::open(&path).unwrap();
        assert_eq!(scroll.settings, None);
        assert_eq!(scroll.profiles[0].name, "old");
    }

    #[test]
    fn a_scroll_of_several_profiles_saves_as_the_new_edition_and_opens_back() {
        let altar = Altar::new("scroll-several");
        let path = altar.file("vault.scroll");
        let written = Scroll {
            profiles: vec![
                sealed("one", vec![inscribed("a.wad", b"a")]),
                sealed(
                    "two",
                    vec![inscribed("b.wad", b"b"), inscribed("c.wad", b"c")],
                ),
            ],
            settings: Some(Shared::default()),
        };
        written.save(&path).unwrap();
        assert_eq!(Scroll::stored_version(&path), Some(1));
        assert_eq!(Scroll::open(&path), Some(written.clone()));
        assert_eq!(written.wad_count(), 3);
    }

    #[test]
    fn every_profile_on_a_scroll_is_unsealed_and_the_missing_wads_are_counted_across_them() {
        let altar = Altar::new("scroll-unseal-several");
        let [first, _] = folders(&altar);
        fs::write(first.join("a.wad"), b"a").unwrap();
        fs::write(first.join("c.wad"), b"c").unwrap();
        let scroll = Scroll {
            profiles: vec![
                sealed("one", vec![inscribed("a.wad", b"a")]),
                SealedProfile {
                    base_wad: Some(inscribed("gone.wad", b"gone")),
                    ..sealed(
                        "two",
                        vec![inscribed("b.wad", b"b"), inscribed("c.wad", b"c")],
                    )
                },
            ],
            settings: None,
        };
        let unsealed = scroll.unseal(&[first], &mut ()).unwrap();
        let names: Vec<_> = unsealed.profiles.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["one", "two"]);
        assert_eq!(unsealed.profiles[1].wads.len(), 2);
        assert!(unsealed.profiles[1].base_wad.is_some());
        // b.wad and the base wad aren't there
        assert_eq!(unsealed.missing, 2);
    }

    #[test]
    fn a_profile_name_tdl_could_not_have_made_gets_the_scroll_refused() {
        for bad in [
            "",
            "../config",
            "has space",
            "dot.name",
            &"a".repeat(MAX_NAME + 1),
        ] {
            let named = Scroll {
                profiles: vec![sealed(bad, vec![])],
                settings: None,
            };
            assert!(named.check().is_err(), "{bad:?}");
        }
        assert!(scroll(vec![]).check().is_ok());
        let empty = Scroll {
            profiles: Vec::new(),
            settings: None,
        };
        assert!(empty.check().is_err(), "a scroll with nothing on it");
    }

    #[test]
    fn too_many_profiles_or_too_many_wads_across_them_are_refused() {
        let many = Scroll {
            profiles: vec![sealed("p", vec![]); MAX_PROFILES + 1],
            settings: None,
        };
        assert!(many.check().is_err());
        // each profile is within the bound, but all of them together aren't
        let wad = inscribed("a.wad", b"a");
        let spread = Scroll {
            profiles: vec![
                sealed("one", vec![wad.clone(); MAX_WADS / 2 + 1]),
                sealed("two", vec![wad; MAX_WADS / 2 + 1]),
            ],
            settings: None,
        };
        assert!(spread.check().is_err());
    }

    #[test]
    fn too_many_wads_are_refused() {
        let wad = inscribed("a.wad", b"a");
        assert!(scroll(vec![wad; MAX_WADS + 1]).check().is_err());
    }

    #[test]
    fn exporting_leaves_out_paths_and_the_join_code() {
        let profile = Profile {
            base_wad: Some(Wad::new("/iwads/DOOM2.WAD".into(), "ab".repeat(32), 9)),
            uzdoom: Some("/usr/bin/uzdoom".into()),
            host_defaults: Some(HostSettings {
                code: "secret".into(),
                ..HostSettings::new(Some(9000))
            }),
            ..Profile::new(
                "brutal".into(),
                vec![Wad::new(
                    "/home/me/wads/brutal.pk3".into(),
                    "cd".repeat(32),
                    4,
                )],
            )
        };
        let scroll = Scroll::of([&profile]).unwrap();
        let sealed = &scroll.profiles[0];
        assert_eq!(sealed.wads[0].name, "brutal.pk3");
        assert_eq!(sealed.base_wad.as_ref().unwrap().name, "DOOM2.WAD");
        assert_eq!(scroll.settings, None, "settings only go on when asked for");
        let defaults = sealed.host_defaults.clone().unwrap();
        assert_eq!(defaults.code, "");
        assert_eq!(defaults.port, "9000");
    }

    #[test]
    fn a_wad_without_a_hash_cannot_be_exported() {
        let profile = Profile::new(
            "p".into(),
            vec![Wad::new("/w/a.wad".into(), String::new(), 0)],
        );
        let problem = Scroll::of([&profile]).unwrap_err();
        assert!(
            problem.contains("a.wad") && problem.contains("'p'"),
            "{problem}"
        );
    }

    #[test]
    fn a_scroll_saves_and_opens_back() {
        let altar = Altar::new("scroll-round-trip");
        let path = altar.file(&Scroll::file_name("shared"));
        let written = scroll(vec![inscribed("a.wad", b"a")]);
        written.save(&path).unwrap();
        assert_eq!(Scroll::open(&path), Some(written));
        assert!(path.to_string_lossy().ends_with(".scroll"));
    }
}
