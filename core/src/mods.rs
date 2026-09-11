use crate::minecraft::download::ensure_file;
use crate::progress::{Progress, ProgressSink, Stage};
use crate::{paths, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
pub struct ModEntry {
    pub filename: String,
    pub url: String,
    #[serde(default)]
    pub sha1: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModsManifest {
    pub mc_version: String,
    pub loader: String,
    #[serde(default)]
    pub mods: Vec<ModEntry>,
}

impl ModsManifest {
    pub fn empty(mc_version: impl Into<String>, loader: impl Into<String>) -> Self {
        Self {
            mc_version: mc_version.into(),
            loader: loader.into(),
            mods: Vec::new(),
        }
    }

    pub fn from_json(raw: &str) -> Result<Self> {
        Ok(serde_json::from_str(raw)?)
    }

    pub fn load_from_file(path: &Path) -> Result<Self> {
        let raw = std::fs::read_to_string(path)?;
        Self::from_json(&raw)
    }
}

/// Nombre "lindo" para mostrar en la UI a partir del nombre de archivo del
/// jar (p. ej. "fps_overlay-4.2.jar" -> "FPS Overlay 4.2"): no hay metadata
/// real de mod name/version acá, así que se infiere del filename nomás.
pub fn display_name(filename: &str) -> String {
    let stem = filename.strip_suffix(".jar").unwrap_or(filename);
    stem.replace(['_', '-'], " ")
        .split_whitespace()
        .map(|word| {
            if word.len() <= 3 && word.chars().all(|c| c.is_alphabetic()) {
                word.to_uppercase()
            } else {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod display_name_tests {
    use super::display_name;

    #[test]
    fn formats_filename_into_display_name() {
        assert_eq!(display_name("fps_overlay-4.2.jar"), "FPS Overlay 4.2");
        assert_eq!(display_name("jei-1.20.1.jar"), "JEI 1.20.1");
    }
}

/// Descarga los mods que falten y borra de `mods/` los jars que ya no estén
/// en el manifest, para que la carpeta siempre refleje exactamente lo que
/// dice el manifest (así es como "se actualiza fácil": se edita el manifest
/// y listo, ver README).
pub async fn sync_mods(client: &reqwest::Client, manifest: &ModsManifest, progress: &ProgressSink) -> Result<()> {
    progress(Progress::new(Stage::SyncingMods, "Sincronizando mods...", None));

    let mods_dir = paths::mods_dir();
    tokio::fs::create_dir_all(&mods_dir).await?;

    let wanted: std::collections::HashSet<String> = manifest.mods.iter().map(|m| m.filename.clone()).collect();

    if let Ok(mut entries) = tokio::fs::read_dir(&mods_dir).await {
        while let Some(entry) = entries.next_entry().await? {
            if !entry.path().is_file() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            if !wanted.contains(&name) {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }

    let total = manifest.mods.len();
    for (i, mod_entry) in manifest.mods.iter().enumerate() {
        progress(Progress::new(
            Stage::SyncingMods,
            format!("Mods: {}/{total} ({})", i + 1, mod_entry.filename),
            Some((i + 1) as f32 / total.max(1) as f32),
        ));

        let dest = mods_dir.join(&mod_entry.filename);
        ensure_file(client, &mod_entry.url, &dest, mod_entry.sha1.as_deref(), None).await?;
    }

    Ok(())
}
