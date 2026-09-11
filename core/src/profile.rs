use crate::{paths, Result};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Configuración persistida del launcher. El login con Discord queda para más
/// adelante (ver README) — por ahora el usuario es una cuenta "offline" como
/// la de cualquier server no-premium: un nombre de usuario y listo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub username: String,
    pub server_ip: String,
    pub server_port: u16,
    /// None = usar el default sugerido según la RAM de la PC (ver
    /// `suggested_ram_mb`). Se guarda como Option en vez de precalcular el
    /// valor para que, si el launcher corre en otra PC, el default se siga
    /// recalculando en vez de arrastrar un número viejo.
    #[serde(default)]
    pub allocated_ram_mb: Option<u32>,
    /// Flags de JVM extra que el usuario quiera agregar a mano, además de las
    /// Aikar's flags que ya mete el launcher (ver `launch::AIKAR_FLAGS`).
    #[serde(default)]
    pub extra_jvm_args: String,
    /// Comprimir `saves/` antes de cada partida (ver `backup`). Apagado por
    /// default: para quien solo juega en el server no hay nada que copiar, y
    /// un mundo grande tarda.
    #[serde(default)]
    pub backup_worlds: bool,
    /// Variante de color del panorama del menú del juego: "dia", "atardecer",
    /// "noche", o None/"auto" para que siga la hora (ver `theme::Panorama`).
    #[serde(default)]
    pub panorama: Option<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            username: String::new(),
            server_ip: String::new(),
            server_port: 25565,
            allocated_ram_mb: None,
            extra_jvm_args: String::new(),
            backup_worlds: false,
            panorama: None,
        }
    }
}

pub fn total_ram_mb() -> u32 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    (sys.total_memory() / 1024 / 1024) as u32
}

/// RAM sugerida por default: la mitad de la RAM total del sistema, acotada a
/// un rango razonable (2-8 GiB) para no sugerir ni muy poco ni dejar la PC
/// sin memoria para el resto.
pub fn suggested_ram_mb() -> u32 {
    (total_ram_mb() / 2).clamp(2048, 8192)
}

impl Settings {
    pub fn load() -> Self {
        let path = paths::settings_file();
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        paths::ensure_dirs()?;
        let raw = serde_json::to_string_pretty(self)?;
        std::fs::write(paths::settings_file(), raw)?;
        Ok(())
    }
}

/// UUID "offline" determinístico a partir del username, con el mismo algoritmo
/// que usa un server vanilla en online-mode=false:
/// UUID.nameUUIDFromBytes(("OfflinePlayer:" + username).getBytes(UTF_8))
/// (un UUID v3 con el namespace implícito de la JVM, no un v3 estándar con
/// namespace RFC — por eso se arma "a mano" en vez de usar Uuid::new_v3).
pub fn offline_uuid(username: &str) -> Uuid {
    let input = format!("OfflinePlayer:{username}");
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&digest);

    bytes[6] = (bytes[6] & 0x0f) | 0x30; // version 3
    bytes[8] = (bytes[8] & 0x3f) | 0x80; // variant RFC 4122

    Uuid::from_bytes(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_is_stable() {
        let a = offline_uuid("Notch");
        let b = offline_uuid("Notch");
        assert_eq!(a, b);
        assert_ne!(a, offline_uuid("jeb_"));
    }
}
