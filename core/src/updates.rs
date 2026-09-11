use crate::Result;
use serde::{Deserialize, Serialize};

/// Chequeo de versión contra los releases de GitHub del propio repo. El
/// launcher NO se actualiza solo: avisa que hay una versión nueva y abre la
/// página de descarga. Instalar el .exe bajado sin verificar firma sería
/// confiar en cualquier cosa que llegue por esa URL; para hacerlo de verdad
/// automático hace falta tauri-plugin-updater con un par de claves de firma
/// (ver README).
const RELEASES_API: &str = "https://api.github.com/repos/miguelacaceresrios/Segestria/releases/latest";

/// Las notas del release se muestran tal cual en la ventana: cortamos para
/// que un changelog largo no empuje toda la interfaz.
const MAX_NOTES: usize = 600;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
    pub notes: String,
}

#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    body: String,
}

/// `None` = ya estás en la última (o el repo todavía no publicó ningún
/// release, que da lo mismo para el usuario).
pub async fn check(client: &reqwest::Client, current_version: &str) -> Result<Option<UpdateInfo>> {
    let response = client.get(RELEASES_API).send().await?;
    if !response.status().is_success() {
        return Ok(None);
    }

    let release: Release = response.json().await?;
    let latest = release.tag_name.trim_start_matches('v').to_string();

    if latest.is_empty() || !is_newer(&latest, current_version) {
        return Ok(None);
    }

    // La URL termina en el navegador del usuario: aceptamos solo páginas de
    // GitHub, no cualquier cosa que venga en el JSON.
    if !release.html_url.starts_with("https://github.com/") {
        return Ok(None);
    }

    Ok(Some(UpdateInfo {
        version: latest,
        url: release.html_url,
        notes: truncate(release.body.trim(), MAX_NOTES),
    }))
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    text.chars().take(max_chars).collect::<String>() + "…"
}

/// Compara "1.2.10" contra "1.2.9" por número y no por texto (que diría que
/// 9 es mayor que 10). Lo que no sea un número se ignora, así un tag tipo
/// "5.0.0-beta" se compara como 5.0.0.
fn is_newer(candidate: &str, current: &str) -> bool {
    parts(candidate) > parts(current)
}

fn parts(version: &str) -> Vec<u64> {
    version
        .split(['.', '-', '+'])
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
                .parse()
                .unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_versions_by_number() {
        assert!(is_newer("5.1.0", "5.0.0"));
        assert!(is_newer("1.2.10", "1.2.9"));
        assert!(is_newer("2.0", "1.9.9"));
        assert!(!is_newer("5.0.0", "5.0.0"));
        assert!(!is_newer("4.9.9", "5.0.0"));
    }

    #[test]
    fn ignores_suffixes_in_tags() {
        assert_eq!(parts("5.0.0-beta.2"), vec![5, 0, 0, 0, 2]);
        assert!(is_newer("5.0.1-rc1", "5.0.0"));
    }

    #[test]
    fn truncates_long_release_notes() {
        assert_eq!(truncate("hola", 10), "hola");
        assert_eq!(truncate(&"a".repeat(20), 5), "aaaaa…");
    }
}
