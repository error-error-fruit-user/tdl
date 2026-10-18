use serde::{Deserialize, Serialize};

use crate::net;

/// The options for a hosted game. `player_count` stays a string since it is passed straight to UZDoom.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct HostSettings {
    pub player_count: String,
    pub is_deathmatch: bool,
    pub map: String,
    /// The port the wads are served on; empty means [`net::DEFAULT_PORT`].
    pub port: String,
    /// What joiners must enter; empty means a random one is made up.
    pub code: String,
    /// Whether the ini the game is hosted with is served too, for joiners who want to play with
    /// it (see [`crate::net::ini`]).
    #[serde(default)]
    pub share_ini: bool,
}

impl HostSettings {
    pub fn new(port: Option<u16>) -> Self {
        Self {
            player_count: 2.to_string(),
            is_deathmatch: true,
            map: String::new(),
            port: port.map(|port| port.to_string()).unwrap_or_default(),
            code: String::new(),
            share_ini: false,
        }
    }

    /// The port to serve on, or `None` if what was typed isn't one.
    pub fn port(&self) -> Option<u16> {
        if self.port.is_empty() {
            return Some(net::DEFAULT_PORT);
        }
        self.port.parse().ok().filter(|&port| port > 0)
    }

    /// The join code: the one typed in, or a fresh random one.
    pub fn code(&self) -> String {
        if self.code.is_empty() {
            net::random_code()
        } else {
            self.code.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_saved_before_the_ini_could_be_shared_still_load_and_keep_it_to_themselves() {
        #[derive(Serialize)]
        struct Old {
            player_count: String,
            is_deathmatch: bool,
            map: String,
            port: String,
            code: String,
        }
        let bytes = rmp_serde::to_vec_named(&Old {
            player_count: "4".into(),
            is_deathmatch: false,
            map: "MAP07".into(),
            port: "9000".into(),
            code: "doom2".into(),
        })
        .unwrap();
        let settings: HostSettings = rmp_serde::from_slice(&bytes).unwrap();
        assert_eq!(settings.player_count, "4");
        assert_eq!(settings.code, "doom2");
        assert!(!settings.share_ini);
    }

    #[test]
    fn the_port_starts_as_the_last_one_and_empty_is_the_default() {
        assert_eq!(HostSettings::new(Some(9000)).port(), Some(9000));
        let mut settings = HostSettings::new(None);
        assert_eq!(settings.port(), Some(net::DEFAULT_PORT));
        settings.port = "0".into();
        assert_eq!(settings.port(), None);
        settings.port = "99999".into();
        assert_eq!(settings.port(), None);
    }

    #[test]
    fn an_empty_code_is_made_up_and_a_typed_one_is_kept() {
        let mut settings = HostSettings::new(None);
        assert!(net::is_valid_code(&settings.code()));
        settings.code = "doom2".into();
        assert_eq!(settings.code(), "doom2");
    }
}
