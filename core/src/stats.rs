use crate::paths;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Cuánto se jugó y cómo terminó la última sesión. Vive aparte de `Settings`
/// a propósito: son datos que el launcher escribe solo, no cosas que el
/// usuario configura, y así un archivo corrupto de uno no se lleva puesto al
/// otro.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Stats {
    pub total_seconds: u64,
    pub sessions: u32,
    pub last_session_seconds: u64,
    /// Época Unix en segundos del final de la última sesión. El formateo a
    /// fecha legible lo hace el frontend, que sí conoce la zona horaria.
    pub last_played_epoch: u64,
    /// Código de salida de la última partida. 0 (o None, si nunca se jugó) es
    /// una salida normal; cualquier otra cosa significa que el juego se cerró
    /// solo y que vale la pena mirar el log.
    pub last_exit_code: Option<i32>,
}

pub fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn load() -> Stats {
    std::fs::read_to_string(paths::stats_file())
        .ok()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

/// Cierra una sesión: suma el tiempo jugado y anota cómo terminó.
///
/// Sesiones muy cortas no cuentan como partida: si Forge explota al arrancar,
/// el proceso vive dos segundos y sumarlo solo ensucia el total. El código de
/// salida sí se guarda igual, que es justo el caso donde hace falta.
pub fn record_session(started_epoch: u64, exit_code: Option<i32>) {
    const MIN_SESSION_SECONDS: u64 = 30;

    let ended = now_epoch();
    let elapsed = ended.saturating_sub(started_epoch);

    let mut stats = load();
    stats.last_exit_code = exit_code;
    stats.last_played_epoch = ended;
    if elapsed >= MIN_SESSION_SECONDS {
        stats.total_seconds += elapsed;
        stats.sessions += 1;
        stats.last_session_seconds = elapsed;
    }

    if let Ok(raw) = serde_json::to_string_pretty(&stats) {
        let _ = std::fs::create_dir_all(paths::data_dir());
        let _ = std::fs::write(paths::stats_file(), raw);
    }
}
