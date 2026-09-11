mod discord;

use discord::DiscordState;
use segestria_core::backup::{self, BackupInfo};
use segestria_core::mods::ModsManifest;
use segestria_core::packs;
use segestria_core::profile::Settings;
use segestria_core::progress::ProgressSink;
use segestria_core::server_status::{self, ServerStatus};
use segestria_core::skins::{self, SavedSkin};
use segestria_core::stats::{self, Stats};
use segestria_core::theme;
use segestria_core::updates::{self, UpdateInfo};
use serde::Deserialize;
use std::path::Path;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager};

/// Lista de mods embebida en el binario. Hoy arranca vacía (ver README);
/// el día que haya mods reales, esto se reemplaza por una descarga desde la
/// nube en vez de un archivo empaquetado en el instalador.
const DEFAULT_MANIFEST: &str = include_str!("../../../manifest.json");
const NEWS: &str = include_str!("../../../news.json");

#[derive(Debug, Deserialize, serde::Serialize)]
struct NewsItem {
    date: String,
    title: String,
    body: String,
}

#[derive(Debug, Deserialize, serde::Serialize)]
struct NewsFeed {
    items: Vec<NewsItem>,
}

#[tauri::command]
fn get_settings() -> Settings {
    Settings::load()
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_news() -> Result<NewsFeed, String> {
    serde_json::from_str(NEWS).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ping_server(ip: String, port: u16) -> ServerStatus {
    server_status::ping(&ip, port).await
}

#[derive(serde::Serialize)]
struct ModInfo {
    name: String,
    filename: String,
}

#[tauri::command]
fn get_mods() -> Result<Vec<ModInfo>, String> {
    let manifest = ModsManifest::from_json(DEFAULT_MANIFEST).map_err(|e| e.to_string())?;
    Ok(manifest
        .mods
        .iter()
        .map(|m| ModInfo {
            name: segestria_core::mods::display_name(&m.filename),
            filename: m.filename.clone(),
        })
        .collect())
}

#[derive(serde::Serialize)]
struct RamInfo {
    total_mb: u32,
    suggested_mb: u32,
}

#[tauri::command]
fn get_ram_info() -> RamInfo {
    RamInfo {
        total_mb: segestria_core::profile::total_ram_mb(),
        suggested_mb: segestria_core::profile::suggested_ram_mb(),
    }
}

#[tauri::command]
fn get_custom_skin_preview() -> Option<String> {
    theme::custom_skin_preview_data_uri()
}

/// Sin el detalle del error del SO: si no, la respuesta distingue "no existe"
/// de "sin permiso" para cualquier ruta que le pasen.
fn read_picked_file(path: &str) -> Result<Vec<u8>, String> {
    std::fs::read(path).map_err(|_| "No se pudo leer el archivo elegido.".to_string())
}

#[tauri::command]
fn clear_custom_skin() -> Result<(), String> {
    theme::clear_custom_skin().map_err(|e| e.to_string())
}

#[tauri::command]
fn list_skins() -> Vec<SavedSkin> {
    skins::list()
}

/// Suma la skin elegida a la galería y la deja aplicada de una: el usuario
/// abrió el selector para ponerse esa skin, no para archivarla.
#[tauri::command]
fn add_skin(path: String) -> Result<SavedSkin, String> {
    let bytes = read_picked_file(&path)?;
    let name = Path::new(&path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("skin");

    let saved = skins::save(&bytes, name).map_err(|e| e.to_string())?;
    theme::set_custom_skin(&bytes).map_err(|e| e.to_string())?;
    Ok(saved)
}

#[tauri::command]
fn apply_skin(id: String) -> Result<String, String> {
    skins::apply(&id).map_err(|e| e.to_string())?;
    theme::custom_skin_preview_data_uri().ok_or_else(|| "no se pudo generar la vista previa".to_string())
}

#[tauri::command]
fn delete_skin(id: String) -> Result<(), String> {
    skins::delete(&id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_cape() -> Option<String> {
    skins::cape_data_uri()
}

#[tauri::command]
fn set_cape(path: String) -> Result<String, String> {
    let bytes = read_picked_file(&path)?;
    skins::set_cape(&bytes).map_err(|e| e.to_string())?;
    skins::cape_data_uri().ok_or_else(|| "no se pudo generar la vista previa".to_string())
}

#[tauri::command]
fn clear_cape() -> Result<(), String> {
    skins::clear_cape().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_stats() -> Stats {
    stats::load()
}

/// Cola del log de la última partida (ver `launch::read_latest_log_tail`).
/// 256 KB alcanzan de sobra para el stacktrace del final y siguen entrando en
/// la ventana sin trabarla.
#[tauri::command(async)]
fn get_latest_log() -> Option<String> {
    segestria_core::minecraft::launch::read_latest_log_tail(256 * 1024)
}

#[tauri::command]
fn list_backups() -> Vec<BackupInfo> {
    backup::list()
}

/// Comprimir un mundo grande puede tardar, así que va a un hilo aparte para
/// no congelar la ventana mientras corre.
#[tauri::command]
async fn backup_now() -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(|| {
        backup::backup_worlds().map(|path| {
            path.and_then(|p| p.file_name().and_then(|n| n.to_str()).map(str::to_string))
        })
    })
    .await
    .map_err(|_| "no se pudo correr la copia de seguridad".to_string())?
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn check_update() -> Result<Option<UpdateInfo>, String> {
    let client = segestria_core::http_client();
    updates::check(&client, env!("CARGO_PKG_VERSION"))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Abre una de las carpetas del launcher en el explorador del sistema. El
/// frontend manda un nombre de una lista cerrada, nunca una ruta: así no hay
/// forma de pedirle que abra cualquier cosa del disco.
#[tauri::command]
fn open_folder(which: String) -> Result<(), String> {
    use segestria_core::paths;

    let path = match which.as_str() {
        "instancia" => paths::instance_dir(),
        "logs" => segestria_core::minecraft::launch::latest_log_path()
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(paths::instance_dir),
        "backups" => paths::backups_dir(),
        "mods" => paths::mods_dir(),
        _ => return Err("Carpeta desconocida.".to_string()),
    };

    std::fs::create_dir_all(&path).map_err(|_| "No se pudo crear la carpeta.".to_string())?;
    open_in_file_manager(&path)
}

fn open_in_file_manager(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = std::process::Command::new("open");
        c.arg(path);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(path);
        c
    };

    // explorer.exe devuelve códigos de salida raros incluso cuando abre bien,
    // así que solo nos importa haber podido lanzarlo.
    command
        .spawn()
        .map(|_| ())
        .map_err(|_| "No se pudo abrir la carpeta.".to_string())
}

#[tauri::command]
fn list_texture_packs() -> Vec<String> {
    packs::list_texture_packs()
}

#[tauri::command]
fn add_texture_pack(path: String) -> Result<String, String> {
    packs::add_texture_pack(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_texture_pack(filename: String) -> Result<(), String> {
    packs::remove_texture_pack(&filename).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_shader_packs() -> Vec<String> {
    packs::list_shader_packs()
}

#[tauri::command]
fn add_shader_pack(path: String) -> Result<String, String> {
    packs::add_shader_pack(&path).map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_shader_pack(filename: String) -> Result<(), String> {
    packs::remove_shader_pack(&filename).map_err(|e| e.to_string())
}

/// `local_hour` la manda el frontend porque el core no conoce la zona horaria
/// del sistema; se usa para elegir el viraje del panorama cuando el usuario lo
/// dejó en automático (ver `theme::Panorama`).
#[tauri::command]
async fn play(app: AppHandle, settings: Settings, local_hour: u32) -> Result<(), String> {
    settings.save().map_err(|e| e.to_string())?;

    discord::set_playing(&app.state::<DiscordState>(), &settings.server_ip);

    let panorama = theme::Panorama::resolve(settings.panorama.as_deref(), local_hour);
    let manifest = ModsManifest::from_json(DEFAULT_MANIFEST).map_err(|e| e.to_string())?;

    let sink: ProgressSink = {
        let app = app.clone();
        Arc::new(move |progress| {
            let _ = app.emit("progress", progress);
        })
    };

    segestria_core::play(&settings, &manifest, panorama, sink)
        .await
        .map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DiscordState::default())
        .setup(|app| {
            // El handshake IPC con Discord tarda ~200ms (o se cuelga si Discord
            // está en un estado raro) y setup() corre en el hilo principal
            // *antes* de crear la ventana: si se llamara acá directo, cada
            // arranque del launcher esperaría a Discord para poder mostrar
            // algo en pantalla. Lo mandamos a un hilo aparte para que la
            // ventana aparezca de una sin importar cuánto tarde.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                discord::connect_and_set_idle(&handle.state::<DiscordState>());
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_settings,
            save_settings,
            get_news,
            ping_server,
            get_mods,
            get_ram_info,
            get_custom_skin_preview,
            clear_custom_skin,
            list_skins,
            add_skin,
            apply_skin,
            delete_skin,
            get_cape,
            set_cape,
            clear_cape,
            get_stats,
            get_latest_log,
            list_backups,
            backup_now,
            check_update,
            get_app_version,
            open_folder,
            list_texture_packs,
            add_texture_pack,
            remove_texture_pack,
            list_shader_packs,
            add_shader_pack,
            remove_shader_pack,
            play
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
