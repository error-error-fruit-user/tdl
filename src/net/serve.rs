use std::{
    ffi::OsString,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use crate::{
    error::{Error, Result},
    net::{Manifest, codes_match},
};

impl Manifest {
    /// Serves until the returned guard is dropped, only to requests that carry `code`.
    /// `paths` must be in the same order as `entries`. `ini` is the host's ini, if it shares it;
    /// without one, asking for it is a 404, as it is from a tdl from before inis were shared.
    pub fn serve(
        self,
        port: u16,
        code: String,
        paths: Vec<OsString>,
        ini: Option<Vec<u8>>,
    ) -> Result<ServerGuard> {
        let manifest =
            rmp_serde::to_vec(&self).map_err(|_| Error::Download("Could not encode file list"))?;
        let server = tiny_http::Server::http(("0.0.0.0", port))
            .map_err(|_| Error::Download("Could not start the host server"))?;
        // 🔌 the port actually bound, which differs from `port` only when that is 0 (any free one)
        let port = server
            .server_addr()
            .to_ip()
            .map_or(port, |address| address.port());
        let should_quit = Arc::new(AtomicBool::new(false));
        let flag = should_quit.clone();
        let thread = thread::spawn(move || {
            while !flag.load(Ordering::SeqCst) {
                let request = match server.recv_timeout(Duration::from_millis(250)) {
                    Ok(Some(request)) => request,
                    Ok(None) => continue,
                    Err(_) => break,
                };
                let url = request.url().to_string();
                // 🔐 /<code>/<what>: anything without the right code is turned away at the door
                let Some((given, url)) = url.trim_start_matches('/').split_once('/') else {
                    let _ = request.respond(
                        tiny_http::Response::from_string("not found").with_status_code(404),
                    );
                    continue;
                };
                if !codes_match(given, &code) {
                    let _ = request.respond(
                        tiny_http::Response::from_string("wrong code").with_status_code(403),
                    );
                    continue;
                }

                if url == "manifest" {
                    let _ = request.respond(tiny_http::Response::from_data(manifest.clone()));
                } else if url == "ini"
                    && let Some(ini) = &ini
                {
                    let _ = request.respond(tiny_http::Response::from_data(ini.clone()));
                } else if let Some(hash) = url.strip_prefix("file/") {
                    let path = self
                        .entries
                        .iter()
                        .position(|e| e.hash == hash)
                        .map(|i| paths[i].clone());

                    thread::spawn(move || {
                        let _ = match path.and_then(|p| std::fs::File::open(p).ok()) {
                            Some(file) => request.respond(tiny_http::Response::from_file(file)),
                            None => request.respond(
                                tiny_http::Response::from_string("not found").with_status_code(404),
                            ),
                        };
                    });
                } else {
                    let _ = request.respond(
                        tiny_http::Response::from_string("not found").with_status_code(404),
                    );
                }
            }
        });
        Ok(ServerGuard {
            port,
            should_quit,
            thread: Some(thread),
        })
    }
}

/// Stops and joins the server thread on drop, so the host is served for exactly as long as it's held.
pub struct ServerGuard {
    /// The port being served on.
    pub port: u16,
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
