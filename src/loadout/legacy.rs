//! Files from before tdl 1.0.0, which this build doesn't read: `upgrade_legacy.py`, which comes
//! with tdl, upgrades them all (see `DATA_LAYOUT.md`). Until it has, tdl says so and changes
//! nothing.

use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::loadout::{
    config::Config, edition::Edition, profile::Profile, profile::SUBDIRECTORY, scribe,
    scroll::Scroll, uac::Uac,
};

/// The script that upgrades legacy files.
pub const SCRIPT: &str = "upgrade_legacy.py";
/// What the global settings were called before they were `uac.rite`.
const OLD_GLOBAL: &str = "global.rite";
/// What every file was called before `.rite`.
const OLD_EXTENSION: &str = "doom";
/// What the profiles dir was called before it had a dot.
const OLD_PROFILES: &str = "profiles";

/// Every legacy file (or dir) in the standard config dir and the open loadout's config dir: the
/// global settings as `global.rite` (unless `uac.rite` is there already), any file saved by an
/// edition older than the type's [`Edition::FIRST`], `.doom` files with no `.rite` beside them,
/// and a `profiles` dir with anything in it.
pub fn found(standard_config_dir: &Path, config_dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let old_global = standard_config_dir.join(OLD_GLOBAL);
    if old_global.exists() && !Uac::path(standard_config_dir).exists() {
        found.push(old_global);
    }
    let uac = Uac::path(standard_config_dir);
    if Uac::is_legacy(&uac) {
        found.push(uac);
    }
    let config = Config::path(config_dir);
    let old_config = config.with_extension(OLD_EXTENSION);
    if Config::is_legacy(&config) {
        found.push(config);
    } else if old_config.exists() && !config.exists() {
        found.push(old_config);
    }
    let old_profiles = config_dir.join(OLD_PROFILES);
    if fs::read_dir(&old_profiles).is_ok_and(|mut entries| entries.next().is_some()) {
        found.push(old_profiles);
    }
    let profiles = fs::read_dir(config_dir.join(SUBDIRECTORY))
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| match path.extension().and_then(|e| e.to_str()) {
            // 🪦 beside a `.rite`, a `.doom` is only a leftover: the `.rite` is what is read
            Some(OLD_EXTENSION) => !path.with_extension(scribe::EXTENSION).exists(),
            Some(scribe::EXTENSION) => Profile::is_legacy(path),
            _ => false,
        });
    let mut profiles: Vec<_> = profiles.collect();
    profiles.sort();
    found.extend(profiles);
    found
}

/// What to tell the user about the legacy files `found`: what they are, and the command that
/// upgrades them, naming the loadout passed on the command line (`loadout`), if one was.
pub fn advice(found: &[PathBuf], loadout: Option<&Path>) -> Vec<String> {
    let mut command = format!("python3 {SCRIPT}");
    if let Some(root) = loadout {
        command += &format!(" --loadout {}", quoted(root));
    }
    let mut lines = vec![
        "Some of your files are from a tdl before 1.0.0, which this one doesn't read:".to_string(),
    ];
    lines.extend(found.iter().map(|path| format!("  {}", path.display())));
    lines.push(format!(
        "Upgrade them with {SCRIPT}, which comes with tdl (your originals are kept as \
         .legacy), then start tdl again: {command}"
    ));
    lines
}

/// What to tell the user about a legacy scroll at `path`.
pub fn scroll_advice(path: &Path) -> String {
    format!(
        "The scroll is from tdl 0.6.0: upgrade it with python3 {SCRIPT} --scroll {}",
        quoted(path)
    )
}

/// `path` as it can be typed into a shell.
fn quoted(path: &Path) -> String {
    let shown = path.display().to_string();
    if shown.contains([' ', '\'', '"']) {
        format!("'{}'", shown.replace('\'', r"'\''"))
    } else {
        shown
    }
}

/// Whether the scroll at `path` is a legacy one.
pub fn is_legacy_scroll(path: &Path) -> bool {
    Scroll::is_legacy(path)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use super::*;
    use crate::loadout::scribe::tests::Altar;

    #[test]
    fn current_files_are_never_legacy() {
        let altar = Altar::new("legacy-none");
        let config_dir = altar.file("config");
        Config::default().write(&config_dir, &[]).ok();
        fs::create_dir_all(config_dir.join(SUBDIRECTORY)).unwrap();
        Config::default().write(&config_dir, &[]).unwrap();
        Profile::new("brutal".into(), Vec::new())
            .save(&Profile::path(&config_dir, "brutal"))
            .unwrap();
        Uac::default().keep(&Uac::path(&config_dir)).unwrap();
        // the old global file is a leftover once uac.rite is there, and an empty old dir is nothing
        fs::write(config_dir.join(OLD_GLOBAL), b"old").unwrap();
        fs::create_dir_all(config_dir.join(OLD_PROFILES)).unwrap();
        assert_eq!(found(&config_dir, &config_dir), Vec::<PathBuf>::new());
        assert_eq!(
            found(&altar.file("nothing"), &altar.file("nothing")),
            Vec::<PathBuf>::new()
        );
    }

    #[test]
    fn every_kind_of_legacy_file_is_found() {
        let altar = Altar::new("legacy-all");
        let standard = altar.file("standard");
        let config_dir = altar.file("loadout/config");
        fs::create_dir_all(standard.join(OLD_PROFILES)).unwrap();
        fs::write(standard.join(OLD_GLOBAL), b"old").unwrap();
        fs::create_dir_all(config_dir.join(OLD_PROFILES)).unwrap();
        fs::write(config_dir.join("profiles/brutal.doom"), b"old").unwrap();
        fs::write(config_dir.join("config.doom"), b"old").unwrap();
        fs::create_dir_all(config_dir.join(SUBDIRECTORY)).unwrap();
        fs::write(config_dir.join(".profiles/nacht.doom"), b"old").unwrap();
        // edition 0: a bare array
        let bare = rmp_serde::to_vec(&("doom", Vec::<()>::new())).unwrap();
        fs::write(Profile::path(&config_dir, "old"), bare).unwrap();
        Profile::new("new".into(), Vec::new())
            .save(&Profile::path(&config_dir, "new"))
            .unwrap();
        assert_eq!(
            found(&standard, &config_dir),
            [
                standard.join(OLD_GLOBAL),
                config_dir.join("config.doom"),
                config_dir.join(OLD_PROFILES),
                config_dir.join(".profiles/nacht.doom"),
                Profile::path(&config_dir, "old"),
            ]
        );
    }

    #[test]
    fn the_advice_names_the_files_and_the_command() {
        let lines = advice(
            &[PathBuf::from("/c/global.rite")],
            Some(Path::new("/my doom")),
        );
        assert!(lines[1].contains("/c/global.rite"));
        assert!(
            lines
                .last()
                .unwrap()
                .ends_with("python3 upgrade_legacy.py --loadout '/my doom'")
        );
        assert!(
            advice(&[], None)
                .last()
                .unwrap()
                .ends_with("python3 upgrade_legacy.py")
        );
        assert_eq!(
            scroll_advice(Path::new("/s/old.scroll")),
            "The scroll is from tdl 0.6.0: upgrade it with python3 upgrade_legacy.py --scroll \
             /s/old.scroll"
        );
    }

    /// Every shape of file a tdl before 1.0.0 saved, written the way it saved them.
    mod shapes {
        use std::ffi::OsString;

        use serde::Serialize;

        #[derive(Serialize)]
        pub struct Sealed<T> {
            pub version: u32,
            pub data: T,
        }

        /// A wad with its whole path, as a tagged `OsString` (editions 0 and 1).
        #[derive(Serialize)]
        pub struct OldWad {
            pub path: OsString,
            pub hash: String,
        }

        #[derive(Serialize)]
        pub struct SizedWad {
            pub path: OsString,
            pub hash: String,
            pub size: u64,
            pub volatile: bool,
        }

        #[derive(Serialize)]
        pub enum Music {
            Kept,
        }

        #[derive(Serialize)]
        pub enum Quip {
            Custom(String),
        }

        /// `global.rite`, edition 0.
        #[derive(Serialize)]
        pub struct GlobalZero {
            pub vault: Option<String>,
            pub wad_folders: Vec<String>,
            pub music: Music,
            pub splash_quip: Quip,
        }

        /// `config.rite`, edition 4: the settings were still the loadout's.
        #[derive(Serialize)]
        pub struct ConfigFour {
            pub uzdoom: Option<String>,
            pub base_wad: Option<()>,
            pub profile_names: Vec<String>,
            pub skip_splash: bool,
            pub host_port: Option<u16>,
            pub wad_dirs: Vec<String>,
            pub uzdoom_args: String,
        }

        /// A profile of edition 1: named fields, every wad with its whole path.
        #[derive(Serialize)]
        pub struct ProfileOne {
            pub name: String,
            pub wads: Vec<SizedWad>,
            pub base_wad: Option<SizedWad>,
            pub uzdoom: Option<OsString>,
            pub ini: Option<OsString>,
        }

        /// Host defaults from before the ini could be shared.
        #[derive(Serialize)]
        pub struct HostZero {
            pub player_count: String,
            pub is_deathmatch: bool,
            pub map: String,
            pub port: String,
            pub code: String,
        }

        /// A profile of editions 2 and 3: a dir table, still called folders.
        #[derive(Serialize)]
        pub struct ProfileThree {
            pub name: String,
            pub wads: Vec<FiledThree>,
            pub base_wad: Option<FiledThree>,
            pub uzdoom: Option<String>,
            pub ini: Option<String>,
            pub host_defaults: Option<HostZero>,
            pub folders: Vec<String>,
        }

        #[derive(Serialize)]
        pub struct FiledThree {
            pub folder: u32,
            pub name: String,
            pub hash: String,
            pub size: u64,
            pub volatile: bool,
            pub modified: u64,
        }

        /// A scroll of edition 0: one profile's fields at the top.
        #[derive(Serialize)]
        pub struct ScrollZero {
            pub name: String,
            pub wads: Vec<Inscribed>,
            pub base_wad: Option<Inscribed>,
            pub host_defaults: Option<HostZero>,
        }

        #[derive(Serialize)]
        pub struct Inscribed {
            pub name: String,
            pub size: u64,
            pub hash: String,
        }
    }

    /// Writes `data` as edition `version` would have, in a named map.
    fn write_sealed<T: serde::Serialize>(path: &Path, version: u32, data: T) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let bytes = rmp_serde::to_vec_named(&shapes::Sealed { version, data }).unwrap();
        fs::write(path, bytes).unwrap();
    }

    /// Writes `data` as a bare array, the way every file was saved before editions.
    fn write_bare<T: serde::Serialize>(path: &Path, data: T) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, rmp_serde::to_vec(&data).unwrap()).unwrap();
    }

    /// Runs the script with `args`, or `None` if there is no `python3` to run it with.
    fn script(args: &[&std::ffi::OsStr]) -> Option<std::process::Output> {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join(SCRIPT);
        match std::process::Command::new("python3")
            .arg(script)
            .args(args)
            .output()
        {
            Ok(output) => Some(output),
            Err(error) => {
                eprintln!("skipped: python3 couldn't be run ({error})");
                None
            }
        }
    }

    fn hash(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    /// A standard config dir and a loadout with a file of every legacy shape: `global.rite`
    /// remembering the loadout, a bare `config.doom` and `profiles/brutal.doom` in the standard
    /// one, and an edition 4 config, an edition 1 profile and an edition 3 profile (with an edition
    /// 2 backup) in the loadout, and a scroll of edition 0. Returns the standard dir, the
    /// loadout's root and the scroll.
    fn legacy_world(altar: &Altar) -> (PathBuf, PathBuf, PathBuf) {
        use shapes::*;
        let standard = altar.file("standard");
        let mine = altar.file("mine");
        let wads = altar.file("wads");
        fs::create_dir_all(&wads).unwrap();
        fs::write(wads.join("brutal.pk3"), b"12345").unwrap();
        let text = |path: &Path| path.to_string_lossy().into_owned();
        write_sealed(
            &standard.join(OLD_GLOBAL),
            0,
            GlobalZero {
                vault: Some(text(&mine)),
                wad_folders: vec!["/global-wads".into()],
                music: Music::Kept,
                splash_quip: Quip::Custom("#2".into()),
            },
        );
        write_bare(
            &standard.join("config.doom"),
            (
                Some(OsString::from("/usr/bin/uzdoom")),
                Some(OldWad {
                    path: "/iwads/DOOM2.WAD".into(),
                    hash: hash('a'),
                }),
                ["brutal"],
            ),
        );
        write_bare(
            &standard.join("profiles/brutal.doom"),
            (
                "brutal",
                [OldWad {
                    path: wads.join("brutal.pk3").into(),
                    hash: hash('b'),
                }],
                None::<OldWad>,
                Some(OsString::from("/usr/bin/uzdoom-brutal")),
                None::<OsString>,
            ),
        );
        let config_dir = mine.join("config");
        write_sealed(
            &Config::path(&config_dir),
            4,
            ConfigFour {
                uzdoom: Some("/opt/other-uzdoom".into()),
                base_wad: None,
                profile_names: vec!["nacht".into(), "deep".into()],
                skip_splash: true,
                host_port: Some(9000),
                wad_dirs: vec!["/mine-wads".into()],
                uzdoom_args: "-nomusic".into(),
            },
        );
        write_sealed(
            &Profile::path(&config_dir, "nacht"),
            1,
            ProfileOne {
                name: "nacht".into(),
                wads: vec![SizedWad {
                    path: "/wads/nacht.wad".into(),
                    hash: hash('c'),
                    size: 40,
                    volatile: true,
                }],
                base_wad: None,
                uzdoom: None,
                ini: Some("/inis/nacht.ini".into()),
            },
        );
        let deep = |size| ProfileThree {
            name: "deep".into(),
            wads: vec![FiledThree {
                folder: 0,
                name: "deep.wad".into(),
                hash: hash('d'),
                size,
                volatile: false,
                modified: 1,
            }],
            base_wad: None,
            uzdoom: None,
            ini: None,
            host_defaults: Some(HostZero {
                player_count: "4".into(),
                is_deathmatch: false,
                map: "MAP07".into(),
                port: String::new(),
                code: "doom2".into(),
            }),
            folders: vec!["/deep/wads".into()],
        };
        write_sealed(&Profile::path(&config_dir, "deep"), 3, deep(7));
        write_sealed(
            &scribe::suffixed(&Profile::path(&config_dir, "deep"), ".bak"),
            2,
            deep(0),
        );
        let scroll = altar.file("shared/old.scroll");
        write_sealed(
            &scroll,
            0,
            ScrollZero {
                name: "shared".into(),
                wads: vec![Inscribed {
                    name: "a.wad".into(),
                    size: 1,
                    hash: hash('e'),
                }],
                base_wad: None,
                host_defaults: None,
            },
        );
        (standard, mine, scroll)
    }

    #[test]
    fn the_script_upgrades_every_legacy_file_tdl_ever_wrote() {
        use crate::loadout::{
            Loadout,
            uac::{Music, SplashQuip},
            wad::Wad,
        };

        let altar = Altar::new("legacy-script");
        let (standard, mine, scroll) = legacy_world(&altar);
        let mine_config = mine.join("config");
        assert!(!found(&standard, &standard).is_empty());
        assert!(!found(&standard, &mine_config).is_empty());
        let Some(output) = script(&[
            "--standard".as_ref(),
            standard.as_os_str(),
            "--scroll".as_ref(),
            scroll.as_os_str(),
        ]) else {
            return;
        };
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(found(&standard, &standard), Vec::<PathBuf>::new());
        assert_eq!(found(&standard, &mine_config), Vec::<PathBuf>::new());

        // 🌐 global.rite is uac.rite, with the old configs' settings, the standard one's first
        let uac = Uac::open(&Uac::path(&standard)).unwrap();
        assert_eq!(uac.loadout, Some(mine.clone().into_os_string()));
        assert_eq!(uac.wad_dirs, [OsString::from("/global-wads")]);
        assert_eq!(uac.music, Music::Kept);
        assert_eq!(uac.splash_quip, SplashQuip::Custom("#2".into()));
        assert_eq!(uac.uzdoom, Some("/usr/bin/uzdoom".into()));
        assert_eq!(
            uac.base_wad,
            Some(Wad::new("/iwads/DOOM2.WAD".into(), hash('a'), 0))
        );
        assert!(uac.skip_splash);
        assert_eq!(uac.host_port, Some(9000));
        assert_eq!(uac.uzdoom_args, "-nomusic");

        // 📂 the standard loadout: its profiles dir has its dot, and the wad its size
        let standard_loadout = Loadout::new(standard.clone(), altar.file("data"));
        assert!(standard_loadout.notices.is_empty());
        assert!(!standard.join(OLD_PROFILES).exists());
        let brutal = &standard_loadout.profiles[0];
        assert_eq!(brutal.name, "brutal");
        assert_eq!(brutal.uzdoom, Some("/usr/bin/uzdoom-brutal".into()));
        assert_eq!(brutal.wads.len(), 1);
        assert_eq!(brutal.wads[0].path, altar.file("wads/brutal.pk3"));
        assert_eq!((brutal.wads[0].size, &brutal.wads[0].hash), (5, &hash('b')));

        // 📂 the loadout global.rite remembered
        let loadout = Loadout::new(mine_config.clone(), mine.join("data"));
        assert!(loadout.notices.is_empty());
        assert_eq!(loadout.config.wad_dirs, [OsString::from("/mine-wads")]);
        let [nacht, deep] = &loadout.profiles[..] else {
            panic!("expected two profiles, got {:?}", loadout.profiles);
        };
        assert_eq!(nacht.wads[0].path, "/wads/nacht.wad");
        assert!(nacht.wads[0].volatile);
        assert_eq!(nacht.wads[0].size, 40);
        assert_eq!(nacht.ini, Some("/inis/nacht.ini".into()));
        assert_eq!(deep.wads[0].path, "/deep/wads/deep.wad");
        assert_eq!((deep.wads[0].size, deep.wads[0].modified), (7, 1));
        let defaults = deep.host_defaults.as_ref().unwrap();
        assert_eq!(
            (defaults.map.as_str(), defaults.share_ini),
            ("MAP07", false)
        );
        // the backup is one tdl can restore from too
        let backup = scribe::suffixed(&Profile::path(&mine_config, "deep"), ".bak");
        assert_eq!(
            Profile::open(&backup).unwrap().wads[0].path,
            "/deep/wads/deep.wad"
        );

        // 📜 and the scroll is a scroll of its one profile
        let sealed = Scroll::open(&scroll).unwrap();
        assert_eq!(sealed.profiles.len(), 1);
        assert_eq!(sealed.profiles[0].name, "shared");
        assert_eq!(sealed.profiles[0].wads[0].name, "a.wad");

        // 🗃️ every original is kept beside its upgrade
        for kept in [
            standard.join("global.rite.legacy"),
            standard.join("config.doom.legacy"),
            standard.join(".profiles/brutal.doom.legacy"),
            mine_config.join("config.rite.legacy"),
            mine_config.join(".profiles/nacht.rite.legacy"),
            mine_config.join(".profiles/deep.rite.legacy"),
            mine_config.join(".profiles/deep.rite.bak.legacy"),
            altar.file("shared/old.scroll.legacy"),
        ] {
            assert!(kept.is_file(), "{} wasn't kept", kept.display());
        }

        // 🔁 a second run has nothing to do, and changes nothing
        let uac_bytes = fs::read(Uac::path(&standard)).unwrap();
        let again = script(&["--standard".as_ref(), standard.as_os_str()]).unwrap();
        assert!(again.status.success());
        assert!(String::from_utf8_lossy(&again.stdout).contains("Nothing to upgrade"));
        assert_eq!(fs::read(Uac::path(&standard)).unwrap(), uac_bytes);
    }

    #[test]
    fn a_dry_run_of_the_script_changes_nothing() {
        let altar = Altar::new("legacy-script-dry");
        let (standard, mine, _) = legacy_world(&altar);
        let before = found(&standard, &mine.join("config"));
        let Some(output) = script(&[
            "--standard".as_ref(),
            standard.as_os_str(),
            "--dry-run".as_ref(),
        ]) else {
            return;
        };
        assert!(output.status.success());
        assert!(String::from_utf8_lossy(&output.stdout).contains("would upgrade"));
        assert_eq!(found(&standard, &mine.join("config")), before);
        assert!(!Uac::path(&standard).exists());
        assert!(standard.join(OLD_PROFILES).is_dir());
    }

    #[test]
    fn the_script_leaves_a_file_from_a_newer_tdl_alone_and_says_so() {
        let altar = Altar::new("legacy-script-newer");
        let standard = altar.file("standard");
        let path = Profile::path(&standard, "future");
        write_sealed(&path, 99, vec![1u8, 2, 3]);
        let bytes = fs::read(&path).unwrap();
        let Some(output) = script(&["--standard".as_ref(), standard.as_os_str()]) else {
            return;
        };
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("newer tdl"));
        assert_eq!(fs::read(&path).unwrap(), bytes);
    }
}
