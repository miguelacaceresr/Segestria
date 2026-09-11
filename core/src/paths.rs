use std::path::PathBuf;

/// Carpeta raíz de datos del launcher (equivalente a %APPDATA%\SegestriaLauncher).
pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("SegestriaLauncher")
}

/// Carpeta de la instancia de juego (equivalente a un ".minecraft").
/// Por ahora manejamos una sola instancia ("default"); si el día de mañana
/// hace falta soportar varios modpacks, esto pasa a tomar un nombre de instancia.
pub fn instance_dir() -> PathBuf {
    data_dir().join("instances").join("default")
}

pub fn versions_dir() -> PathBuf {
    instance_dir().join("versions")
}

pub fn libraries_dir() -> PathBuf {
    instance_dir().join("libraries")
}

pub fn assets_dir() -> PathBuf {
    instance_dir().join("assets")
}

pub fn mods_dir() -> PathBuf {
    instance_dir().join("mods")
}

pub fn natives_dir(version_id: &str) -> PathBuf {
    data_dir().join("natives").join(version_id)
}

pub fn cache_dir() -> PathBuf {
    data_dir().join("cache")
}

pub fn settings_file() -> PathBuf {
    data_dir().join("settings.json")
}

pub fn stats_file() -> PathBuf {
    data_dir().join("stats.json")
}

/// Copias de los mundos, fuera de la instancia a propósito: si algún día hay
/// que borrar y rehacer la instancia, los backups no se van con ella.
pub fn backups_dir() -> PathBuf {
    data_dir().join("backups")
}

pub fn saves_dir() -> PathBuf {
    instance_dir().join("saves")
}

pub fn version_dir(version_id: &str) -> PathBuf {
    versions_dir().join(version_id)
}

pub fn version_json_path(version_id: &str) -> PathBuf {
    version_dir(version_id).join(format!("{version_id}.json"))
}

pub fn version_jar_path(version_id: &str) -> PathBuf {
    version_dir(version_id).join(format!("{version_id}.jar"))
}

pub fn ensure_dirs() -> std::io::Result<()> {
    for dir in [
        data_dir(),
        instance_dir(),
        versions_dir(),
        libraries_dir(),
        assets_dir(),
        mods_dir(),
        cache_dir(),
    ] {
        std::fs::create_dir_all(dir)?;
    }
    Ok(())
}
