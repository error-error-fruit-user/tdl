//! Brings the voices from installation-groans (`toiling-through-hell.toml`, `computer-speak.toml`)
//! into `OUT_DIR/groans/`, for `const_groan!` to build in: from the workspace's checkout beside
//! tdl's when there is one, or else from GitHub. A voice found in neither is left empty, and the
//! installer speaks plainly.

use std::{env, fs, path::PathBuf, process::Command};

const VOICES: [&str; 2] = ["toiling-through-hell.toml", "computer-speak.toml"];
const GITHUB: &str =
    "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main";

fn main() {
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("groans");
    fs::create_dir_all(&out).unwrap();
    let workspace =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../installation-groans");
    for voice in VOICES {
        let local = workspace.join(voice);
        println!("cargo:rerun-if-changed={}", local.display());
        let text = fs::read_to_string(&local).ok().or_else(|| {
            let fetched = Command::new("curl")
                .args(["-fsSL", &format!("{GITHUB}/{voice}")])
                .output()
                .ok()?;
            fetched
                .status
                .success()
                .then(|| String::from_utf8_lossy(&fetched.stdout).into_owned())
        });
        if text.is_none() {
            println!(
                "cargo:warning=installation-groans' {voice} is neither beside tdl nor on GitHub: the installer speaks plainly"
            );
        }
        fs::write(out.join(voice), text.unwrap_or_default()).unwrap();
    }
}
