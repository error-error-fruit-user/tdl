use std::ops::Deref;

use ratatui::style::Color;

use crate::{
    error::{Error, Result},
    net,
    popups::{hashing, running, status, warning::Warning},
    ritual::Grimoire,
    uzdoom,
    vault::Vault,
    vault::host_settings::HostSettings,
    vault::wad::Wad,
};

const TITLE: &str = "Hosting";
const BORDER_COLOR: Color = Color::LightMagenta;

/// Serves the profile's wads on the chosen port, behind the join code, then runs the game; the
/// server stops when the game exits. The port is remembered for next time.
pub fn summon(
    grimoire: &mut Grimoire,
    vault: &mut Vault,
    index: usize,
    settings: HostSettings,
) -> Result<()> {
    let Some(port) = settings.port() else {
        return Warning::invalid_port(grimoire);
    };
    // 💾 only a port other than the default is worth remembering
    let remembered = (port != net::DEFAULT_PORT).then_some(port);
    if vault.global.host_port != remembered {
        vault.global.host_port = remembered;
        if vault.keep_global().is_err() {
            Warning::save_failed(grimoire)?;
        }
    }
    let profile = &vault.profiles[index];
    let Some(uzdoom) = profile.uzdoom(&vault.global) else {
        return Warning::uzdoom_not_selected(grimoire);
    };
    let base_wad = profile.base_wad(&vault.global);
    status::summon(grimoire, TITLE, "Hosting...", BORDER_COLOR)?;

    // 🌀 volatile wads are hashed fresh, so what the manifest says is what actually gets served
    let wads = match hashing::refresh(grimoire, &profile.wads) {
        Ok(Some(wads)) => wads,
        Ok(None) => return Ok(()),
        Err(Error::Interrupted) => return Err(Error::Interrupted),
        Err(_) => return Warning::wad_unreadable(grimoire),
    };
    let Ok(manifest) =
        net::Manifest::build(&wads, base_wad.map(|w| w.hash.clone()).unwrap_or_default())
    else {
        return Warning::wad_unreadable(grimoire);
    };
    let code = settings.code();
    let Ok(server) = manifest.serve(
        port,
        code.clone(),
        wads.iter().map(|w| w.path.clone()).collect(),
    ) else {
        return Warning::host_failed(grimoire);
    };

    // 🔌 the port the server really bound, which is the one joiners need; the window kept up while
    // the game runs shows it with the code, for the host to pass on
    let port = server.port;
    let details = [format!("Players join with port {port}, code {code}")];
    let result = uzdoom::host(
        uzdoom,
        base_wad.map(Wad::deref),
        profile,
        &settings,
        profile.uzdoom_args(&vault.global),
        &vault.data_dir.join(&profile.name),
    )
    .and_then(|game| running::summon(grimoire, "hosting", &details, game));
    Warning::report_spawn(grimoire, result)
}
