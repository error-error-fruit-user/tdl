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
