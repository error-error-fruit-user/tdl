use std::path::PathBuf;

use crate::{error::Result, picker, popups::three_way_dialog::ThreeWayDialog, ritual::Grimoire};

/// What the user chose for a profile's UZDoom config file.
pub enum IniChoice {
    /// No file of the profile's own: UZDoom keeps using its normal, global config.
    Global,
    /// A new, empty config for this profile alone. The caller makes the file.
    New,
    /// A config file that already exists.
    Existing(PathBuf),
}

/// Asks which UZDoom config file a profile should use, with a three-way dialog.
pub struct ConfigSelector;

impl ConfigSelector {
    const MESSAGE: &str = "Which UZDoom config file to use?";
    const OPTIONS: [&str; 3] = ["Global", "New", "Select"];

    /// `None` if the user backs out, including by cancelling the file picker.
    pub fn summon(grimoire: &mut Grimoire) -> Result<Option<IniChoice>> {
        Ok(
            match ThreeWayDialog::summon(grimoire, Self::MESSAGE, Self::OPTIONS)? {
                Some(0) => Some(IniChoice::Global),
                Some(1) => Some(IniChoice::New),
                Some(_) => picker::file(grimoire, Some(("config", &["ini"])), None)?
                    .map(IniChoice::Existing),
                None => None,
            },
        )
    }
}
