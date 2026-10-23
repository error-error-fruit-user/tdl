//! `tdl-install`: puts UZDoom and Freedoom 2 next to tdl, so a new player has something to play
//! with, and sets tdl up to use them. It makes two folders beside it (or beside `--dir`):
//!
//! ```text
//! uzdoom/                   UZDoom's own folder: the Windows release unpacked, or the AppImage
//! wads/base_wads/           freedoom2.wad, with Freedoom's license
//! ```
//!
//! tdl then offers `wads` and `wads/base_wads` as wad pools when it starts. Each download is a
//! pinned release, checked against its size and BLAKE3 hash before anything is unpacked. What is
//! already there is left alone.
//!
//! One thing happens outside those folders, and `--no-link` leaves it out: the UZDoom and the base
//! wad are kept as tdl's global ones by running the `tdl` beside this program with `--setup`.

use std::{
    fs,
    io::{self, Read, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::Parser;

/// A file to download: where from, and what it must be.
struct Download {
    name: &'static str,
    url: &'static str,
    size: u64,
    blake3: &'static str,
}

/// Freedoom 0.13.0, whose zip holds both Freedoom wads; only Phase 2 is taken. (Its SHA-256
/// matches the project's signed `freedoom-0.13.0-CHECKSUM`.)
const FREEDOOM: Download = Download {
    name: "freedoom-0.13.0.zip",
    url: "https://github.com/freedoom/freedoom/releases/download/v0.13.0/freedoom-0.13.0.zip",
    size: 24_143_781,
    blake3: "5f947833ecce402da78af8f63e9d647191b48f97dfa0ebd16b464548492d9719",
};
/// Freedoom 2 and its license inside [`FREEDOOM`], and what each is called once it is out.
const FREEDOOM_FILES: [(&str, &str); 2] = [
    ("freedoom-0.13.0/freedoom2.wad", "freedoom2.wad"),
    ("freedoom-0.13.0/COPYING.txt", "freedoom-COPYING.txt"),
];

/// UZDoom 5.0.3 for Windows: a zip of its folder, licenses included.
const UZDOOM_WINDOWS: Download = Download {
    name: "Windows-UZDoom-Release-x86_64.zip",
    url: "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Windows-UZDoom-Release-x86_64.zip",
    size: 29_337_585,
    blake3: "408b8017240488e648bbf5a3b99f05105f224b5960e31531ef626461f150baf1",
};
/// UZDoom 5.0.3 for Linux: one AppImage, which runs from wherever it is.
const UZDOOM_LINUX: Download = Download {
    name: "Linux-UZDoom-Release-x86_64.AppImage",
    url: "https://github.com/UZDoom/UZDoom/releases/download/5.0.3/Linux-UZDoom-Release-x86_64.AppImage",
    size: 34_560_504,
    blake3: "0f1d5177e2bf034b535ddf3fcee5755852378ef44533538b56c17e313a3b802d",
};
/// What the AppImage is called in `uzdoom/`.
const APPIMAGE: &str = "uzdoom.AppImage";

/// Puts UZDoom and Freedoom 2 next to tdl: `uzdoom/` and `wads/base_wads/freedoom2.wad`, beside
/// this program unless `--dir` says where. What is already there is left alone.
#[derive(Parser)]
#[command(version, about)]
struct Cli {
    /// The folder tdl is in, to put `uzdoom` and `wads` in. Defaults to the one this program is in.
    #[arg(long, value_name = "DIR")]
    dir: Option<PathBuf>,
    /// Leaves UZDoom out.
    #[arg(long)]
    no_uzdoom: bool,
    /// Leaves Freedoom 2 out.
    #[arg(long)]
    no_freedoom: bool,
    /// Doesn't point tdl at the UZDoom and the base wad.
    #[arg(long)]
    no_link: bool,
}

/// What went wrong, for the user.
type Outcome<T> = Result<T, String>;

fn main() -> ExitCode {
    let cli = Cli::parse();
    let dir = match cli.dir {
        Some(dir) => std::path::absolute(&dir).unwrap_or(dir),
        None => match std::env::current_exe() {
            Ok(exe) => exe.parent().map(Path::to_path_buf).unwrap_or_default(),
            Err(error) => {
                eprintln!("Couldn't tell where this program is ({error}); pass --dir");
                return ExitCode::FAILURE;
            }
        },
    };
    if !dir.is_dir() {
        eprintln!("{} isn't a folder", dir.display());
        return ExitCode::FAILURE;
    }
    println!("Installing next to tdl, in {}", dir.display());
    let scratch = Scratch::new();
    let mut failed = false;
    let mut report = |what: &str, outcome: Outcome<Option<PathBuf>>| match outcome {
        Ok(Some(path)) => println!("{what}: {}", path.display()),
        Ok(None) => {}
        Err(problem) => {
            eprintln!("{what} wasn't installed: {problem}");
            failed = true;
        }
    };
    if !cli.no_uzdoom {
        report("UZDoom", install_uzdoom(&dir, &scratch));
    }
    if !cli.no_freedoom {
        report("Freedoom 2", install_freedoom(&dir, &scratch));
    }
    if !cli.no_link {
        link(&dir);
    }
    if failed {
        return ExitCode::FAILURE;
    }
    println!("Done. tdl offers the wads folders as wad pools when it starts.");
    ExitCode::SUCCESS
}

/// Where UZDoom is inside `<dir>/uzdoom/` on this system, if it is there.
fn uzdoom_in(dir: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "uzdoom.exe"
    } else {
        APPIMAGE
    };
    Some(dir.join("uzdoom").join(name)).filter(|path| path.is_file())
}

/// The arguments that make tdl keep `uzdoom` and `base_wad` as its global ones, without windows.
fn setup_args(uzdoom: Option<&Path>, base_wad: Option<&Path>) -> Vec<std::ffi::OsString> {
    let mut args = vec!["--setup".into()];
    if let Some(uzdoom) = uzdoom {
        args.extend(["--keep-uzdoom".into(), uzdoom.into()]);
    }
    if let Some(base_wad) = base_wad {
        args.extend(["--keep-base-wad".into(), base_wad.into()]);
    }
    args
}

/// Points tdl at the UZDoom and the base wad in `dir`, by running the `tdl` there with `--setup`.
/// Without a `tdl` there, or if it fails, says what to run instead. Nothing is an error: the files
/// are in place, and tdl can pick them (`U` and `B`).
fn link(dir: &Path) {
    let uzdoom = uzdoom_in(dir);
    let base_wad = Some(dir.join("wads").join("base_wads").join(FREEDOOM_FILES[0].1))
        .filter(|path| path.is_file());
    if uzdoom.is_none() && base_wad.is_none() {
        return;
    }
    let args = setup_args(uzdoom.as_deref(), base_wad.as_deref());
    let tdl = dir.join(if cfg!(windows) { "tdl.exe" } else { "tdl" });
    let manually = || {
        let shown: Vec<_> = args.iter().map(|arg| arg.to_string_lossy()).collect();
        println!("To point tdl at them, run: tdl {}", shown.join(" "));
    };
    if !tdl.is_file() {
        println!(
            "tdl isn't in {}, so it isn't linked to them yet.",
            dir.display()
        );
        manually();
        return;
    }
    match std::process::Command::new(&tdl).args(&args).status() {
        Ok(status) if status.success() => {
            if uzdoom.is_some() {
                println!("tdl now uses that UZDoom");
            }
            if base_wad.is_some() {
                println!("tdl now uses that base wad");
            }
        }
        Ok(status) => {
            eprintln!("tdl --setup failed ({status}), so it isn't linked to them yet.");
            manually();
        }
        Err(error) => {
            eprintln!("Couldn't run {}: {error}", tdl.display());
            manually();
        }
    }
}

/// Unpacks or places UZDoom in `<dir>/uzdoom/`, unless that is there already. Returns the
/// executable to pick in tdl, or `None` if nothing was done.
fn install_uzdoom(dir: &Path, scratch: &Scratch) -> Outcome<Option<PathBuf>> {
    let folder = dir.join("uzdoom");
    if folder.exists() {
        println!(
            "UZDoom: {} is there already, so it is left alone",
            folder.display()
        );
        return Ok(None);
    }
    let download = if cfg!(all(windows, target_arch = "x86_64")) {
        &UZDOOM_WINDOWS
    } else if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        &UZDOOM_LINUX
    } else {
        return Err(
            "UZDoom has no release here for this system; get it from https://github.com/UZDoom/UZDoom/releases"
                .into(),
        );
    };
    let file = fetch(download, scratch)?;
    // 🧱 built beside the real one, which only appears once it is whole
    let part = dir.join("uzdoom.part");
    let _ = fs::remove_dir_all(&part);
    let built = if download.name.ends_with(".zip") {
        unpack_all(&file, &part).map(|()| part.join("uzdoom.exe"))
    } else {
        place_appimage(&file, &part)
    };
    let executable = match built {
        Ok(executable) => executable,
        Err(problem) => {
            let _ = fs::remove_dir_all(&part);
            return Err(problem);
        }
    };
    fs::rename(&part, &folder).map_err(|error| {
        let _ = fs::remove_dir_all(&part);
        format!("couldn't make {}: {error}", folder.display())
    })?;
    Ok(Some(
        folder.join(executable.file_name().unwrap_or_default()),
    ))
}

/// Copies the AppImage into `folder` and lets it run. Returns where it is.
fn place_appimage(file: &Path, folder: &Path) -> Outcome<PathBuf> {
    let made = |error: io::Error| format!("couldn't make {}: {error}", folder.display());
    fs::create_dir_all(folder).map_err(made)?;
    let appimage = folder.join(APPIMAGE);
    fs::copy(file, &appimage).map_err(made)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&appimage, fs::Permissions::from_mode(0o755)).map_err(made)?;
    }
    Ok(appimage)
}

/// Takes Freedoom 2 and its license out of the Freedoom zip into `<dir>/wads/base_wads/`, unless
/// the wad is there already. Returns the wad, or `None` if nothing was done.
fn install_freedoom(dir: &Path, scratch: &Scratch) -> Outcome<Option<PathBuf>> {
    let base_wads = dir.join("wads").join("base_wads");
    let wad = base_wads.join(FREEDOOM_FILES[0].1);
    if wad.exists() {
        println!(
            "Freedoom 2: {} is there already, so it is left alone",
            wad.display()
        );
        return Ok(None);
    }
    let file = fetch(&FREEDOOM, scratch)?;
    fs::create_dir_all(&base_wads)
        .map_err(|error| format!("couldn't make {}: {error}", base_wads.display()))?;
    let mut archive = open_zip(&file)?;
    // 📜 the license first, so the wad never arrives without it
    for (inside, name) in FREEDOOM_FILES.iter().rev() {
        let mut entry = archive
            .by_name(inside)
            .map_err(|error| format!("{inside} isn't in {}: {error}", FREEDOOM.name))?;
        write_whole(&mut entry, &base_wads.join(name))?;
    }
    Ok(Some(wad))
}

/// Downloads `download` into the scratch folder, checking its size and hash on the way. Returns
/// where it is.
fn fetch(download: &Download, scratch: &Scratch) -> Outcome<PathBuf> {
    let path = scratch.dir()?.join(download.name);
    let failed = |error: &dyn std::fmt::Display| format!("downloading {}: {error}", download.url);
    let mut reader = ureq::get(download.url)
        .call()
        .map_err(|error| failed(&error))?
        .into_body()
        .into_reader();
    let mut file = fs::File::create(&path).map_err(|error| failed(&error))?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0; 256 * 1024];
    let mut total = 0;
    let mut shown = u64::MAX;
    loop {
        let read = reader.read(&mut buf).map_err(|error| failed(&error))?;
        if read == 0 {
            break;
        }
        total += read as u64;
        if total > download.size {
            return Err(failed(&"it is bigger than it should be"));
        }
        hasher.update(&buf[..read]);
        file.write_all(&buf[..read])
            .map_err(|error| failed(&error))?;
        let percent = total * 100 / download.size;
        if percent != shown {
            shown = percent;
            eprint!("\r{}: {percent:>3}%", download.name);
        }
    }
    eprintln!();
    file.sync_all().map_err(|error| failed(&error))?;
    if total != download.size {
        return Err(failed(&"it ended early"));
    }
    if hasher.finalize().to_hex().as_str() != download.blake3 {
        return Err(failed(&"it isn't the file it should be (its hash differs)"));
    }
    Ok(path)
}

fn open_zip(file: &Path) -> Outcome<zip::ZipArchive<fs::File>> {
    let opened = fs::File::open(file).map_err(|error| format!("{}: {error}", file.display()))?;
    zip::ZipArchive::new(opened).map_err(|error| format!("{}: {error}", file.display()))
}

/// Unpacks every file in the zip at `file` into `folder`. A name that would land outside it is
/// refused.
fn unpack_all(file: &Path, folder: &Path) -> Outcome<()> {
    let mut archive = open_zip(file)?;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| format!("{}: {error}", file.display()))?;
        let Some(name) = entry.enclosed_name() else {
            return Err(format!(
                "{} has a file named {:?}",
                file.display(),
                entry.name()
            ));
        };
        let to = folder.join(name);
        if entry.is_dir() {
            fs::create_dir_all(&to).map_err(|error| format!("{}: {error}", to.display()))?;
        } else {
            write_whole(&mut entry, &to)?;
        }
    }
    Ok(())
}

/// Writes all of `from` to `to`, by way of `<to>.part`, so a file cut short is never left as if it
/// were whole.
fn write_whole(from: &mut impl Read, to: &Path) -> Outcome<()> {
    let problem = |error: io::Error| format!("{}: {error}", to.display());
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(problem)?;
    }
    let mut part = to.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    let written = (|| {
        let mut file = fs::File::create(&part)?;
        io::copy(from, &mut file)?;
        file.sync_all()?;
        fs::rename(&part, to)
    })();
    if let Err(error) = written {
        let _ = fs::remove_file(&part);
        return Err(problem(error));
    }
    Ok(())
}

/// A folder for the downloads, in the system's temporary folder rather than next to tdl, made
/// the first time it is needed and deleted with everything in it at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("tdl-install-{}", std::process::id())))
    }

    fn dir(&self) -> Outcome<&Path> {
        fs::create_dir_all(&self.0)
            .map_err(|error| format!("couldn't make {}: {error}", self.0.display()))?;
        Ok(&self.0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zip_of(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, contents) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contents).unwrap();
        }
        writer.finish().unwrap();
    }

    fn altar(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tdl-install-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_zip_unpacks_whole_and_one_that_reaches_outside_is_refused() {
        let dir = altar("unpack");
        let good = dir.join("good.zip");
        zip_of(
            &good,
            &[("uzdoom.exe", b"exe"), ("soundfonts/uzdoom.sf2", b"sf2")],
        );
        unpack_all(&good, &dir.join("out")).unwrap();
        assert_eq!(
            fs::read(dir.join("out/soundfonts/uzdoom.sf2")).unwrap(),
            b"sf2"
        );
        assert!(!dir.join("out/uzdoom.exe.part").exists());

        let evil = dir.join("evil.zip");
        zip_of(&evil, &[("../escaped.txt", b"gotcha")]);
        assert!(unpack_all(&evil, &dir.join("evil")).is_err());
        assert!(!dir.join("escaped.txt").exists());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_pinned_downloads_have_hashes() {
        for download in [&FREEDOOM, &UZDOOM_WINDOWS, &UZDOOM_LINUX] {
            assert_eq!(download.blake3.len(), 64, "{}", download.name);
            assert!(download.url.ends_with(download.name));
        }
    }

    #[test]
    fn the_setup_arguments_name_only_what_is_there() {
        let args = |uzdoom, wad| setup_args(uzdoom, wad);
        assert_eq!(args(None, None), ["--setup"]);
        assert_eq!(
            args(Some(Path::new("u")), Some(Path::new("w"))),
            ["--setup", "--keep-uzdoom", "u", "--keep-base-wad", "w"]
        );
        assert_eq!(
            args(None, Some(Path::new("w"))),
            ["--setup", "--keep-base-wad", "w"]
        );
    }

    #[test]
    fn whats_there_already_is_left_alone() {
        let dir = altar("there");
        fs::create_dir_all(dir.join("uzdoom")).unwrap();
        fs::create_dir_all(dir.join("wads/base_wads")).unwrap();
        fs::write(dir.join("wads/base_wads/freedoom2.wad"), b"mine").unwrap();
        let scratch = Scratch::new();
        assert_eq!(install_uzdoom(&dir, &scratch), Ok(None));
        assert_eq!(install_freedoom(&dir, &scratch), Ok(None));
        assert_eq!(
            fs::read(dir.join("wads/base_wads/freedoom2.wad")).unwrap(),
            b"mine"
        );
        let _ = fs::remove_dir_all(&dir);
    }
}
