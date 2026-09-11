use discord_rich_presence::{activity, DiscordIpc, DiscordIpcClient};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Client ID de una app registrada en https://discord.com/developers/applications.
/// Con un ID inválido esto simplemente no conecta — el rich presence es un
/// accesorio, nunca algo de lo que dependa el launcher para arrancar.
/// TODO: pisar con el Client ID real de la app de Segestria.
const DISCORD_CLIENT_ID: &str = "0000000000000000000";

/// Imagen grande del rich presence: apunta al ícono ya versionado en el repo,
/// así no hace falta subir nada a mano al Developer Portal de Discord.
const LARGE_IMAGE_URL: &str =
    "https://raw.githubusercontent.com/miguelacaceresrios/Segestria/master/pc/src-tauri/icons/icon.png";

pub struct DiscordState(Mutex<Option<DiscordIpcClient>>);

impl Default for DiscordState {
    fn default() -> Self {
        Self(Mutex::new(None))
    }
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn set_presence(client: &mut DiscordIpcClient, details: &str, activity_state: &str) {
    let payload = activity::Activity::new()
        .details(details)
        .state(activity_state)
        .assets(activity::Assets::new().large_image(LARGE_IMAGE_URL).large_text("Segestria"))
        .timestamps(activity::Timestamps::new().start(now_unix()));
    let _ = client.set_activity(payload);
}

/// Se llama una vez al arrancar el launcher. Si Discord no está corriendo (o
/// el Client ID todavía es el placeholder) no pasa nada: el launcher sigue
/// andando normal, sin presence.
pub fn connect_and_set_idle(state: &DiscordState) {
    let mut client = DiscordIpcClient::new(DISCORD_CLIENT_ID);
    if client.connect().is_err() {
        return;
    }
    set_presence(&mut client, "Segestria Launcher", "Preparando para jugar");
    *state.0.lock().unwrap() = Some(client);
}

/// Se llama cuando el usuario aprieta Jugar.
pub fn set_playing(state: &DiscordState, server_ip: &str) {
    let activity_state = if server_ip.is_empty() {
        "Sin servidor".to_string()
    } else {
        format!("Conectado a {server_ip}")
    };
    if let Some(client) = state.0.lock().unwrap().as_mut() {
        set_presence(client, "Jugando Minecraft 1.20.1 (Forge)", &activity_state);
    }
}
