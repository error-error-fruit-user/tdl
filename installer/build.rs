//! Brings installation-groans' files (`groans.rs`, the journal it is built with, and the voices
//! `toiling-through-hell.toml` and `computer-speak.toml`) into `OUT_DIR/groans/`: from tdl's
//! installation-groans submodule, else the workspace's checkout beside tdl, else GitHub.

use std::{
    env, fs, io,
    path::{Path, PathBuf},
};

const GROANS: [&str; 3] = [
    "groans.rs",
    "toiling-through-hell.toml",
    "computer-speak.toml",
];

/// installation-groans' files into `OUT_DIR/groans/`, for the installer to build in (`groans.rs`, its
/// journal, and the voices for `const_groan!`): from the project's installation-groans submodule,
/// else the workspace's checkout beside the project, else GitHub. groans.rs is needed to build;
/// a voice found nowhere is left empty, and the installer speaks plainly.
fn groans(project: &Path) -> io::Result<()> {
    const GITHUB: &str =
        "https://raw.githubusercontent.com/error-error-fruit-user/installation-groans/main";
    let out = PathBuf::from(env::var("OUT_DIR").unwrap()).join("groans");
    fs::create_dir_all(&out)?;
    let places = [
        project.join("installation-groans"),
        project.join("..").join("installation-groans"),
    ];
    for name in GROANS {
        for place in &places {
            println!("cargo:rerun-if-changed={}", place.join(name).display());
        }
        let text = places
            .iter()
            .find_map(|place| fs::read_to_string(place.join(name)).ok())
            .or_else(|| {
                let fetched = std::process::Command::new("curl")
                    .args(["-fsSL", &format!("{GITHUB}/{name}")])
                    .output()
                    .ok()?;
                fetched
                    .status
                    .success()
                    .then(|| String::from_utf8_lossy(&fetched.stdout).into_owned())
            });
        let text = match text {
            Some(text) => text,
            None if name.ends_with(".toml") => {
                println!(
                    "cargo:warning=installation-groans' {name} is nowhere to be found: the installer speaks plainly"
                );
                String::new()
            }
            None => {
                return Err(io::Error::other(format!(
                    "installation-groans' {name} is nowhere to be found: run git submodule update --init"
                )));
            }
        };
        // 📜 the journal is built in with include!, where a file's inner doc comments can't be
        let text = if name.ends_with(".rs") {
            text.replace("\n//! ", "\n// ")
                .replace("\n//!\n", "\n//\n")
                .replacen("//! ", "// ", 1)
        } else {
            text
        };
        fs::write(out.join(name), text)?;
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let installer = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    groans(&installer.join(".."))
}
