use std::{
    io::{Read, Write},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use ratatui::{style::Color, text::ToLine};
use serde::{Deserialize, Serialize};

use crate::{
    error::Result,
    profile::{Mod, Profile},
    utility::{self, Terminal},
};

pub const DEFAULT_PORT: u16 = 8080;
pub const MAX_FILES: usize = 255;

#[derive(Deserialize, Serialize)]
pub struct Manifest {
    pub entries: Vec<Entry>,
}

impl Manifest {
    pub fn build(mods: &Vec<Mod>) -> Result<Self> {
        let mut entries = Vec::new();
        for r#mod in mods {
            let path = Path::new(&r#mod.path);
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let size = std::fs::metadata(path)?.len();
            let hash = r#mod.hash.clone();
            entries.push(Entry::new(name, size, hash));
        }
        Ok(Self { entries })
    }

    pub fn fetch(ip: &str) -> Result<Self> {
        let response = config()
            .new_agent()
            .get(&format!("{}/manifest", base_url(ip)))
            .call()
            .unwrap();
        let mut body = String::new();
        response
            .into_body()
            .into_reader()
            .read_to_string(&mut body)
            .unwrap();
        let manifest: Manifest = toml::from_str(&body).unwrap();
        if manifest.entries.len() > MAX_FILES {
            panic!("Too many files trying to be downloaded");
        }
        Ok(manifest)
    }

    pub fn serve(self, port: u16, paths: Vec<String>) -> Result<ServerGuard> {
        let server = tiny_http::Server::http(("0.0.0.0", port)).unwrap();
        let should_quit = Arc::new(AtomicBool::new(false));
        let flag = should_quit.clone();
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                if let Some(request) = server.recv_timeout(Duration::from_millis(250)).unwrap() {
                    let url = request.url().to_string();

                    if url == "/manifest" {
                        request
                            .respond(tiny_http::Response::from_string(
                                &toml::to_string(&self).unwrap(),
                            ))
                            .unwrap();
                    } else if let Some(hash) = url.strip_prefix("/file/") {
                        let path = self
                            .entries
                            .iter()
                            .position(|e| e.hash == hash)
                            .map(|i| paths[i].clone());

                        thread::spawn(move || {
                            match path.and_then(|p| std::fs::File::open(p).ok()) {
                                Some(file) => request
                                    .respond(tiny_http::Response::from_file(file))
                                    .unwrap(),
                                None => request
                                    .respond(
                                        tiny_http::Response::from_string("not found")
                                            .with_status_code(404),
                                    )
                                    .unwrap(),
                            }
                        });
                    } else {
                        request
                            .respond(
                                tiny_http::Response::from_string("not found").with_status_code(404),
                            )
                            .unwrap();
                    }
                }
            }
        });
        Ok(ServerGuard {
            should_quit,
            thread: Some(thread),
        })
    }

    pub fn is_compatible_with(&self, profile: &Profile) -> bool {
        if self.entries.len() != profile.mods.len() {
            return false;
        }
        for (entry, r#mod) in self.entries.iter().zip(profile.mods.iter()) {
            if entry.hash != r#mod.hash {
                return false;
            }
        }
        true
    }
}

#[derive(Deserialize, Serialize)]
pub struct Entry {
    pub name: String,
    pub size: u64,
    pub hash: String,
}

impl Entry {
    fn new(name: String, size: u64, hash: String) -> Self {
        Self { name, size, hash }
    }
}

fn config() -> ureq::config::Config {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(3)))
        .timeout_recv_response(Some(Duration::from_secs(10)))
        .build()
}

fn base_url(ip: &str) -> String {
    let host = ip.rsplit_once(":").map_or(ip, |(h, _)| h);
    format!("http://{host}:{DEFAULT_PORT}")
}

pub fn download_profile(
    terminal: &mut Terminal,
    ip: &str,
    entries: &[Entry],
    cache: &Path,
) -> Result<Profile> {
    const WIDTH: u16 = 32;
    const HEIGHT: u16 = 5;
    const TITLE: &'static str = "Downloading";
    const BORDER_COLOR: Color = Color::Magenta;

    terminal.draw(|frame| {
        let (window, mut window_area, ..) =
            utility::new_window(WIDTH, HEIGHT, TITLE, BORDER_COLOR, frame.area());
        frame.render_widget(window, window_area);
        window_area.y += 2;
        frame.render_widget("Downloading...".to_line().centered(), window_area);
    })?;

    let agent = config().new_agent();
    let mut mods = Vec::new();
    for entry in entries {
        let dir = cache.join(&entry.hash);
        if dir.exists() {
            mods.push(Mod::new(
                dir.join(entry.name.clone()).to_string_lossy().to_string(),
                entry.hash.clone(),
            ));
            continue;
        }
        std::fs::create_dir_all(&dir)?;
        let part = dir.join(format!("{}.part", entry.name));
        let url = format!("{}/file/{}", base_url(ip), entry.hash);
        let mut reader = agent.get(&url).call().unwrap().into_body().into_reader();
        let mut file = std::fs::File::create(&part)?;
        let mut hasher = blake3::Hasher::new();
        let mut buf = [0; 64 * 1024];
        let mut total = 0;

        loop {
            let size = reader.read(&mut buf)?;
            if size == 0 {
                break;
            }
            total += size;
            if total as u64 > entry.size {
                drop(file);
                let _ = std::fs::remove_file(&part);
                panic!("File downloading is bigger than it should be!");
            }
            hasher.update(&buf[..size]);
            file.write_all(&buf[..size])?;
        }
        file.flush()?;
        drop(file);
        if hasher.finalize().to_hex().to_string() != entry.hash {
            let _ = std::fs::remove_file(&part);
            panic!("downloade file does not have matching hash!");
        }

        std::fs::rename(&part, dir.join(&entry.name))?;

        mods.push(Mod::new(
            dir.join(entry.name.clone()).to_string_lossy().to_string(),
            entry.hash.clone(),
        ));
    }
    Ok(Profile::new(mods))
}

pub struct ServerGuard {
    should_quit: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Drop for ServerGuard {
    fn drop(&mut self) {
        self.should_quit.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
