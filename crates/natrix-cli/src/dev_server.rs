//! Spawn live reloading server

use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;
use std::{fs, thread};

use anyhow::Result;
use notify::Watcher;
use tiny_http::{Header, Response, Server};

use crate::build::assets::AssetManifest;
use crate::build::build;
use crate::options;
use crate::prelude::*;

/// Find the closest gitignore
fn find_gitignore() -> Result<ignore::gitignore::Gitignore> {
    let mut current_dir = std::env::current_dir()?.canonicalize()?;
    while !current_dir.join(".gitignore").exists() {
        if let Some(parent) = current_dir.parent() {
            current_dir = parent.to_owned();
        } else {
            return Ok(ignore::gitignore::Gitignore::empty());
        }
    }
    let (matcher, _) = ignore::gitignore::Gitignore::new(current_dir.join(".gitignore"));
    Ok(matcher)
}

/// Do the dev server
pub(crate) fn do_dev(args: &options::DevArguments) -> Result<()> {
    let config = args.get_build_config()?;

    let (tx_notify, rx_notify) = mpsc::channel();
    let (tx_reload, rx_reload) = mpsc::channel();

    let matcher = find_gitignore()?;
    let mut watcher = notify::recommended_watcher(move |event: Result<notify::Event, _>| {
        if let Ok(event) = event
            && (event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove())
            && event.paths.iter().any(|path| {
                !matcher
                    .matched_path_or_any_parents(path, path.is_dir())
                    .is_ignore()
            })
        {
            let _ = tx_notify.send(event);
        }
    })?;
    watcher.watch(&PathBuf::from("."), notify::RecursiveMode::Recursive)?;

    let asset_manifest_mutex = Arc::new(Mutex::new(AssetManifest::default()));

    match build(&config) {
        Err(err) => {
            uwuln!(err, red);
        }
        Ok(manifest) => {
            let mut lock = asset_manifest_mutex
                .lock()
                .map_err(|_| anyhow!("Failed to lock mutex"))?;
            *lock = manifest;
        }
    }

    let ip = if args.allow_external {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };

    let dist = config.dist.clone();
    let mutex_clone = Arc::clone(&asset_manifest_mutex);
    let server_port = args.port; // Pass the user-specified port to spawn_server
    thread::spawn(move || spawn_server(dist, mutex_clone, server_port, config.live_reload, ip));

    if let Some(port) = config.live_reload {
        thread::spawn(move || spawn_websocket(port, rx_reload, ip));
    }

    loop {
        rx_notify.recv()?;
        std::thread::sleep(Duration::from_millis(100));
        while rx_notify.try_recv().is_ok() {}

        match build(&config) {
            Err(err) => {
                uwuln!(err, red);
            }
            Ok(manifest) => {
                let mut lock = asset_manifest_mutex
                    .lock()
                    .map_err(|_| anyhow!("Failed to lock mutex"))?;
                *lock = manifest;
                tx_reload.send(())?;
            }
        }
    }
}

/// Spawn a websocket server to send reload signals
#[expect(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    reason = "This is running in a thread"
)]
fn spawn_websocket(port: u16, reload_signal: mpsc::Receiver<()>, ip: Ipv4Addr) {
    let server = TcpListener::bind((ip, port)).expect("Failed to bind websocket");
    let clients = Arc::new(Mutex::new(Vec::new()));

    let clients_2 = clients.clone();
    thread::spawn(move || {
        for stream in server.incoming() {
            let Ok(stream) = stream else {
                continue;
            };
            let Ok(ws) = tungstenite::accept(stream) else {
                continue;
            };
            let mut clients = clients_2.lock().expect("Mutex gone");
            clients.push(ws);
        }
    });

    loop {
        if let Ok(()) = reload_signal.recv() {
            let mut clients = clients.lock().expect("Mutex gone");
            for mut client in clients.drain(..) {
                let _ = client.write(tungstenite::Message::from("RELOAD NOW PLS"));
                client.flush().expect("Failed to flush");
            }
        }
    }
}

/// Find a free port
pub(crate) fn get_free_port(preferred: u16) -> Result<u16> {
    if TcpListener::bind((Ipv4Addr::LOCALHOST, preferred)).is_ok() {
        return Ok(preferred);
    }

    Ok(TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?
        .local_addr()?
        .port())
}

/// Spawn a dev server for serving files
#[expect(
    clippy::expect_used,
    clippy::needless_pass_by_value,
    reason = "This is running in a thread"
)]
pub(crate) fn spawn_server(
    folder: PathBuf,
    asset_manifest: Arc<Mutex<AssetManifest>>,
    preferred_port: Option<u16>,
    live_reload: Option<u16>,
    ip: Ipv4Addr,
) {
    // Use the specified port if provided, otherwise start at 8000
    let port = match preferred_port {
        Some(port) => port,
        None => get_free_port(8000).expect("Failed to find free port for server"),
    };

    let server = Server::http((ip, port)).expect("Failed to start server");
    let port = server
        .server_addr()
        .to_ip()
        .expect("Failed to get ip")
        .port();
    uwu!("🚀 Dev server running at http://", green);
    uwu!(ip, bright_red);
    uwu!(":", bright_red);
    uwu!(&port.to_string(), bright_red);

    let live_reload_text = if let Some(live_reload_url) = live_reload {
        format!(" (with live-reload via {live_reload_url})")
    } else {
        String::new()
    };
    uwu!(&live_reload_text, bright_black);
    println!();

    for request in server.incoming_requests() {
        let asset_manifest = asset_manifest.lock().expect("Failed to lock mutex");

        let url = request.url();
        let url = url.strip_prefix("/").unwrap_or(url);

        let path = if url.is_empty() {
            folder.join("index.html")
        } else if let Some(path) = asset_manifest.mapping.get(url) {
            path.clone()
        } else {
            if url.contains("..") {
                let response =
                    Response::from_string("PATH TRAVERSAL DETECTED").with_status_code(404);
                let _ = request.respond(response);
                uwuln!(
                    "Path traversal detected in URL, terminating server for security.",
                    bold.red.on_black
                );
                return;
            }
            folder.join(url)
        };

        let response = if path.exists() && path.is_file() {
            let content_type: &[u8] = match path.extension().and_then(|x| x.to_str()) {
                Some("html") => b"text/html",
                Some("js") => b"text/javascript",
                Some("css") => b"text/css",
                Some("wasm") => b"application/wasm",
                None | Some(_) => b"text/plain",
            };
            match fs::read(path) {
                Ok(content) => Response::from_data(content).with_header(
                    Header::from_bytes(b"Content-Type", content_type).expect("Invalid header"),
                ),
                Err(err) => {
                    println!("{}", err.red());
                    let error_message = format!("😢 Error reading file: {err}");
                    Response::from_string(error_message).with_status_code(500)
                }
            }
        } else {
            let not_found_message = "🚫 404 Not Found!";
            Response::from_string(not_found_message).with_status_code(404)
        };

        let _ = request.respond(response);
    }
}
