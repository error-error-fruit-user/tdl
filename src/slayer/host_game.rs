use std::{ffi::OsString, ops::Deref};

use crate::{
    error::{Error, Result},
    loadout::Loadout,
    loadout::host_settings::HostSettings,
    loadout::wad::Wad,
    net,
    popups::{hashing, running, status, warning::Warning},
    ritual::Grimoire,
    theme, uzdoom,
};

const TITLE: &str = "Hosting";

/// Serves the profile's wads on the chosen port, behind the join code (and the ini it is hosted
/// with, if the settings share it), then runs the game; the server stops when the game exits. The
/// port is remembered for next time.
pub fn summon(
    grimoire: &mut Grimoire,
    loadout: &mut Loadout,
    index: usize,
    settings: HostSettings,
) -> Result<()> {
    let Some(port) = settings.port() else {
        return Warning::invalid_port(grimoire);
    };
    // 💾 only a port other than the default is worth remembering
    let remembered = (port != net::DEFAULT_PORT).then_some(port);
    if loadout.uac.host_port != remembered {
        loadout.uac.host_port = remembered;
        if let Err(error) = loadout.keep_uac() {
            Warning::save_failed(grimoire, &error)?;
        }
    }
    let profile = &loadout.profiles[index];
    let Some(uzdoom) = profile.uzdoom(&loadout.uac) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&loadout.uac);
    status::summon(grimoire, TITLE, "Hosting...", theme::worn().colors.netplay)?;

    // 🌀 volatile wads are hashed fresh and folders packed, so what the manifest says is what
    // actually gets served; the game itself still plays the folders
    let wads = match hashing::served(grimoire, &profile.wads, &loadout.packed_dir()) {
        Ok(Some(wads)) => wads,
        Ok(None) => return Ok(()),
        Err(Error::Interrupted) => return Err(Error::Interrupted),
        Err(Error::Pack(problem)) => return Warning::summon(grimoire, &problem),
        Err(_) => return Warning::wad_unreadable(grimoire),
    };
    let Ok(manifest) =
        net::Manifest::build(&wads, base_wad.map(|w| w.hash.clone()).unwrap_or_default())
    else {
        return Warning::wad_unreadable(grimoire);
    };
    let ini = if settings.share_ini {
        match shared_ini(profile.ini(&loadout.uac).map(OsString::as_os_str), uzdoom) {
            Ok(ini) => Some(ini),
            Err(problem) => {
                Warning::summon(grimoire, &format!("{problem}, so it isn't shared"))?;
                None
            }
        }
    } else {
        None
    };
    let sharing = ini.is_some();
    let code = settings.code();
    let Ok(server) = manifest.serve(
        port,
        code.clone(),
        wads.iter().map(|w| w.path.clone()).collect(),
        ini,
    ) else {
        return Warning::host_failed(grimoire);
    };

    // 🔌 the port the server really bound, which is the one joiners need; the window kept up while
    // the game runs shows it with the code, for the host to pass on
    let port = server.port;
    let mut details = vec![format!("Players join with port {port}, code {code}")];
    if sharing {
        details.push("Your ini is shared".to_string());
    }
    let result = uzdoom::host(
        uzdoom,
        base_wad.map(Wad::deref),
        profile.ini(&loadout.uac).map(OsString::as_os_str),
        profile,
        &settings,
        profile.uzdoom_args(&loadout.uac),
        &loadout.data_dir.join(&profile.name),
    )
    .and_then(|game| running::summon(grimoire, "hosting", &details, game));
    Warning::report_spawn(grimoire, result)
}

/// The ini the game is hosted with, read to be served, or what stands in the way, to be said.
pub fn shared_ini(
    profile_ini: Option<&std::ffi::OsStr>,
    uzdoom: &std::ffi::OsStr,
) -> std::result::Result<Vec<u8>, String> {
    let Some(path) = net::ini::in_use(profile_ini, uzdoom) else {
        return Err(match profile_ini {
            Some(_) => "The profile's ini is missing".to_string(),
            None => "UZDoom's own ini wasn't found".to_string(),
        });
    };
    net::ini::read(&path).map_err(|error| format!("Could not read {}: {error}", path.display()))
}
