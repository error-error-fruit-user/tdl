use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct Profile {
    pub name: String,
    pub mods: Vec<String>,
    pub shared_folder: Option<String>,
}
