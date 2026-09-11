use crate::{paths, theme, Error, Result};
use std::path::{Component, Path, PathBuf};

/// A diferencia de la skin o el tema (que son overrides puntuales que
/// Segestria genera solas), acá el usuario suma sus propios .zip de texture
/// packs/shaders tal cual los descargó — el launcher solo los copia a la
/// carpeta correcta y (para texture packs) los prende en `options.txt`.
fn texture_packs_dir() -> PathBuf {
    paths::instance_dir().join("resourcepacks")
}

/// Oculus (ver manifest.json) lee esta carpeta solo; a diferencia de los
/// resource packs no hay nada que activar en `options.txt`, el juego lo
/// resuelve desde su propio menú de shaders.
fn shader_packs_dir() -> PathBuf {
    paths::instance_dir().join("shaderpacks")
}

/// Chequeo mínimo de que el archivo sea un .zip: alcanza con la firma "PK"
/// del local file header, sin necesitar un lector de zip completo acá.
fn validate_zip(bytes: &[u8]) -> Result<()> {
    if bytes.len() < 4 || &bytes[0..2] != b"PK" {
        return Err(Error::Other("El archivo elegido no es un .zip válido.".to_string()));
    }
    Ok(())
}

/// Se queda solo con el nombre de archivo de una ruta elegida en el selector
/// nativo (que viene con la ruta completa), para no arrastrar carpetas del
/// disco del usuario adentro de la instancia.
///
/// OJO: no alcanza con `Path::file_name()`. En Windows, el último componente
/// puede traer prefijo de unidad ("x/C:cosa.zip" -> "C:cosa.zip") y ahí
/// `PathBuf::join` DESCARTA el directorio base y termina escribiendo/borrando
/// en la raíz relativa de esa unidad. Por eso exigimos un nombre simple.
fn file_name_of(source_path: &str) -> Result<String> {
    let invalid = || Error::Other("Nombre de archivo inválido.".to_string());

    let name = Path::new(source_path).file_name().and_then(|n| n.to_str()).ok_or_else(invalid)?;

    let is_plain_segment = !name.is_empty()
        && !name.contains([':', '/', '\\'])
        && !name.trim_end_matches([' ', '.']).is_empty()
        && Path::new(name).components().count() == 1
        && matches!(Path::new(name).components().next(), Some(Component::Normal(_)));

    if !is_plain_segment {
        return Err(invalid());
    }
    Ok(name.to_string())
}

/// Segunda barrera: después de armar la ruta destino, confirmamos que de
/// verdad quedó adentro de `dir`. Si `join` se comió el base (ver arriba),
/// el padre no coincide y cortamos.
fn dest_in_dir(dir: &Path, filename: &str) -> Result<PathBuf> {
    let dest = dir.join(filename);
    if dest.parent() != Some(dir) {
        return Err(Error::Other("Nombre de archivo inválido.".to_string()));
    }
    Ok(dest)
}

fn list_zip_files(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() {
                return None;
            }
            let name = path.file_name()?.to_str()?.to_string();
            name.to_lowercase().ends_with(".zip").then_some(name)
        })
        .collect();
    names.sort();
    names
}

pub fn list_texture_packs() -> Vec<String> {
    list_zip_files(&texture_packs_dir())
}

pub fn add_texture_pack(source_path: &str) -> Result<String> {
    let bytes = std::fs::read(source_path)?;
    validate_zip(&bytes)?;
    let filename = file_name_of(source_path)?;

    let dir = texture_packs_dir();
    let dest = dest_in_dir(&dir, &filename)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dest, &bytes)?;

    theme::add_resource_pack_entry(&paths::instance_dir().join("options.txt"), &filename)?;
    Ok(filename)
}

pub fn remove_texture_pack(filename: &str) -> Result<()> {
    let filename = file_name_of(filename)?;
    let dest = dest_in_dir(&texture_packs_dir(), &filename)?;
    let _ = std::fs::remove_file(dest);
    theme::remove_resource_pack_entry(&paths::instance_dir().join("options.txt"), &filename)
}

pub fn list_shader_packs() -> Vec<String> {
    list_zip_files(&shader_packs_dir())
}

pub fn add_shader_pack(source_path: &str) -> Result<String> {
    let bytes = std::fs::read(source_path)?;
    validate_zip(&bytes)?;
    let filename = file_name_of(source_path)?;

    let dir = shader_packs_dir();
    let dest = dest_in_dir(&dir, &filename)?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dest, &bytes)?;
    Ok(filename)
}

pub fn remove_shader_pack(filename: &str) -> Result<()> {
    let filename = file_name_of(filename)?;
    let dest = dest_in_dir(&shader_packs_dir(), &filename)?;
    let _ = std::fs::remove_file(dest);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_zip() -> Vec<u8> {
        b"PK\x03\x04fake zip contents".to_vec()
    }

    #[test]
    fn rejects_non_zip_files() {
        assert!(validate_zip(b"not a zip").is_err());
        assert!(validate_zip(&fake_zip()).is_ok());
    }

    #[test]
    fn strips_directories_from_source_path() {
        assert_eq!(file_name_of("C:\\Users\\gato\\Downloads\\Faithful.zip").unwrap(), "Faithful.zip");
        assert_eq!(file_name_of("/home/gato/shaders/Complementary.zip").unwrap(), "Complementary.zip");
    }

    /// Regresión: `Path::file_name()` sola dejaba pasar nombres con prefijo
    /// de unidad, y ahí `join` descarta el directorio base y se escapa a otra
    /// parte del disco (borrando/escribiendo fuera de la instancia).
    #[test]
    fn rejects_names_that_would_escape_the_packs_dir() {
        for evil in ["x/C:evil.zip", "a/D:secret.txt", "algo/C:", "..", "  ", "..."] {
            assert!(file_name_of(evil).is_err(), "deberia rechazar {evil:?}");
        }
        // Una barra al final es inofensiva: "sub/dir/" es simplemente "dir".
        assert_eq!(file_name_of("sub/dir/").unwrap(), "dir");
    }

    #[test]
    fn dest_in_dir_rejects_escaping_names() {
        let base = Path::new("C:\\base\\packs");
        assert!(dest_in_dir(base, "C:evil.zip").is_err());
        assert!(dest_in_dir(base, "D:evil.zip").is_err());
        assert_eq!(dest_in_dir(base, "ok.zip").unwrap(), base.join("ok.zip"));
    }

    #[test]
    fn lists_only_zip_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Faithful.zip"), fake_zip()).unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"hola").unwrap();
        std::fs::create_dir(dir.path().join("segestria-theme")).unwrap();

        assert_eq!(list_zip_files(dir.path()), vec!["Faithful.zip".to_string()]);
    }
}
