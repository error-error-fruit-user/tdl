//! `.scroll` files: a profile to share with someone else. Each wad is only its file name, size
//! and hash, with no paths, since paths only mean something on the machine that wrote them.
//! Breaking the seal (importing) finds each wad again by joining its name onto the wad folders.

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
/// The most wads a scroll may hold. Far more than the network's [`net::MAX_FILES`], since a scroll
/// is only read from disk, but still a bound, since it comes from someone else.
pub const MAX_WADS: usize = 4096;

/// A shared profile: its name, its wads in load order, its base wad, its host defaults, and, if its
/// maker chose to, their global settings.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct Scroll {
    pub name: String,
    pub wads: Vec<Inscribed>,
    #[serde(default)]
    pub base_wad: Option<Inscribed>,
    /// Shared without the join code, which is the host's password.
    #[serde(default)]
    pub host_defaults: Option<HostSettings>,
    /// The maker's music and quip settings, if they chose to share them. Never their song. Missing
    /// in older scrolls, which carry none.
    #[serde(default)]
    pub settings: Option<Shared>,
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
    /// Born after editions existed, so it starts at the first one and has nothing to upgrade.
    const UPGRADES: &[fn(Self) -> Self] = &[];
}

/// What breaking the seal turned up: the wads in load order, each found or a placeholder.
#[derive(Debug, PartialEq)]
pub struct Unsealed {
    pub wads: Vec<Wad>,
    pub base_wad: Option<Wad>,
    /// How many of the wads (base wad included) weren't found, and are placeholders.
    pub missing: usize,
}

impl Scroll {
    /// The scroll for `profile`. Fails, naming the wad, if one has no hash to share, since
    /// whoever reads the scroll would have no way to know they found the right file.
    pub fn of(profile: &Profile) -> std::result::Result<Self, String> {
        let inscribe = |wad: &Wad| {
            if wad.hash.is_empty() {
                return Err(format!(
                    "{} has no hash to share: pick it again with R in the editor",
                    wad.file_name()
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
            settings: None,
        })
    }

    /// Whether a scroll from someone else is safe to break: not too many wads, and every name a
    /// bare file name and every hash a real one, the same rule as a host's manifest.
    pub fn check(&self) -> std::result::Result<(), &'static str> {
        if self.wads.len() > MAX_WADS {
            return Err("The scroll lists too many wads");
        }
        if !self
            .wads
            .iter()
            .chain(&self.base_wad)
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
        let wads = self.wads.iter().map(&mut find).collect::<Result<_>>()?;
        let base_wad = self.base_wad.as_ref().map(&mut find).transpose()?;
        Ok(Unsealed {
            wads,
            base_wad,
            missing,
        })
    }

    /// The file name a scroll of the profile `name` gets by default: `<name>.scroll`.
    pub fn file_name(name: &str) -> String {
        format!("{name}.{EXTENSION}")
    }
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
        Err(error @ (Error::Cancelled | Error::Exit)) => return Err(error),
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

    fn scroll(wads: Vec<Inscribed>) -> Scroll {
        Scroll {
            name: "shared".into(),
            wads,
            base_wad: None,
            host_defaults: None,
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
        assert_eq!(unsealed.wads[0].path, first.join("a.wad").into_os_string());
        assert_ne!(unsealed.wads[0].modified, 0);
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
        let placeholder = &unsealed.wads[0];
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
        assert_eq!(unsealed.wads[0].path, second.join("a.wad").into_os_string());
        assert_eq!(unsealed.wads[1].path, first.join("b.wad").into_os_string());
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
        assert!(unsealed.wads[0].volatile);
        assert_eq!(
            unsealed.wads[0].hash,
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
            base_wad: Some(Inscribed {
                name: "../DOOM2.WAD".into(),
                ..good
            }),
            ..scroll(vec![])
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

    #[test]
    fn a_scroll_from_before_settings_could_be_shared_carries_none() {
        #[derive(Serialize)]
        struct Before {
            name: String,
            wads: Vec<Inscribed>,
            base_wad: Option<Inscribed>,
            host_defaults: Option<HostSettings>,
        }
        #[derive(Serialize)]
        struct Sealed {
            version: u32,
            data: Before,
        }
        let altar = Altar::new("scroll-before-settings");
        let path = altar.file("old.scroll");
        let bytes = rmp_serde::to_vec_named(&Sealed {
            version: Scroll::current(),
            data: Before {
                name: "old".into(),
                wads: Vec::new(),
                base_wad: None,
                host_defaults: None,
            },
        })
        .unwrap();
        fs::write(&path, bytes).unwrap();
        let scroll = Scroll::open(&path).unwrap();
        assert_eq!(scroll.settings, None);
        assert_eq!(scroll.name, "old");
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
        let scroll = Scroll::of(&profile).unwrap();
        assert_eq!(scroll.wads[0].name, "brutal.pk3");
        assert_eq!(scroll.base_wad.as_ref().unwrap().name, "DOOM2.WAD");
        let defaults = scroll.host_defaults.unwrap();
        assert_eq!(defaults.code, "");
        assert_eq!(defaults.port, "9000");
    }

    #[test]
    fn a_wad_without_a_hash_cannot_be_exported() {
        let profile = Profile::new(
            "p".into(),
            vec![Wad::new("/w/a.wad".into(), String::new(), 0)],
        );
        assert!(Scroll::of(&profile).unwrap_err().contains("a.wad"));
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
