use segestria_core::mods::ModsManifest;
use segestria_core::profile::Settings;
use segestria_core::progress::{Progress, ProgressSink};
use std::sync::Arc;

/// Prueba de punta a punta del pipeline real (sin Tauri de por medio):
/// java -> vanilla -> forge -> mods -> lanzar. Configurable por env vars:
/// SEGESTRIA_USERNAME, SEGESTRIA_SERVER_IP, SEGESTRIA_SERVER_PORT.
#[tokio::main]
async fn main() {
    let username = std::env::var("SEGESTRIA_USERNAME").unwrap_or_else(|_| "TestPlayer".to_string());
    let server_ip = std::env::var("SEGESTRIA_SERVER_IP").unwrap_or_default();
    let server_port: u16 = std::env::var("SEGESTRIA_SERVER_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(25565);

    let settings = Settings {
        username,
        server_ip,
        server_port,
        ..Default::default()
    };

    let sink: ProgressSink = Arc::new(|p: Progress| match p.fraction {
        Some(f) => println!("[{:?}] {} ({:.0}%)", p.stage, p.message, f * 100.0),
        None => println!("[{:?}] {}", p.stage, p.message),
    });

    let manifest = ModsManifest::empty(segestria_core::MC_VERSION, "forge");

    let panorama = segestria_core::theme::Panorama::Dia;

    match segestria_core::play(&settings, &manifest, panorama, sink).await {
        Ok(()) => println!("OK: Minecraft debería estar abriendo."),
        Err(e) => {
            eprintln!("ERROR: {e}");
            std::process::exit(1);
        }
    }
}
