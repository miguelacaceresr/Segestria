use crate::{paths, theme, Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Galería de skins: en vez de una sola skin "activa" que se pisa cada vez,
/// el launcher guarda las que el usuario fue eligiendo y le deja cambiar de
/// una a otra con un clic. La activa sigue viviendo adentro del resource pack
/// (ver `theme::set_custom_skin`); esto es el cajón de donde se saca.
fn skins_dir() -> PathBuf {
    paths::data_dir().join("skins")
}

/// La capa no va al resource pack: Minecraft no deja darse una capa a sí
/// mismo desde un pack (las capas vienen de la cuenta de Mojang), así que
/// esto es puramente para el visor 3D del launcher.
fn cape_path() -> PathBuf {
    paths::data_dir().join("cape.png")
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedSkin {
    /// Nombre del archivo sin extensión; es lo que la UI manda de vuelta para
    /// aplicar o borrar.
    pub id: String,
    pub data_uri: String,
}

/// Tope de la galería. No es una restricción técnica, es para que la tira de
/// caritas siga entrando en una ventana de 440px de ancho.
const MAX_SKINS: usize = 30;

/// Nombre de archivo armado desde cero con un alfabeto cerrado: como la UI
/// manda ids de vuelta ("aplicá esta", "borrá esta"), es más simple garantizar
/// que un id nunca puede ser otra cosa que `[a-z0-9_-]` que andar validando
/// rutas después (ver el mismo problema resuelto en `packs::file_name_of`).
fn slug(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| c.to_ascii_lowercase())
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();

    let cleaned = cleaned.trim_matches('-').to_string();
    let cleaned: String = cleaned.chars().take(40).collect();

    if cleaned.is_empty() {
        "skin".to_string()
    } else {
        cleaned
    }
}

/// Segunda barrera, por si `slug` alguna vez deja pasar algo: la ruta final
/// tiene que quedar adentro de la carpeta de skins.
fn path_of(id: &str) -> Result<PathBuf> {
    let id = slug(id);
    let dir = skins_dir();
    let path = dir.join(format!("{id}.png"));
    if path.parent() != Some(dir.as_path()) {
        return Err(Error::Other("Nombre de skin inválido.".to_string()));
    }
    Ok(path)
}

fn unique_path(base: &str) -> Result<(String, PathBuf)> {
    let base = slug(base);
    for suffix in 0..1000 {
        let id = if suffix == 0 { base.clone() } else { format!("{base}-{suffix}") };
        let path = path_of(&id)?;
        if !path.exists() {
            return Ok((id, path));
        }
    }
    Err(Error::Other("Ya hay demasiadas skins con ese nombre.".to_string()))
}

pub fn list() -> Vec<SavedSkin> {
    let Ok(entries) = std::fs::read_dir(skins_dir()) else {
        return Vec::new();
    };

    let mut skins: Vec<SavedSkin> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if path.extension()?.to_str()? != "png" {
                return None;
            }
            let id = path.file_stem()?.to_str()?.to_string();
            let bytes = std::fs::read(&path).ok()?;
            Some(SavedSkin {
                id,
                data_uri: theme::png_data_uri(&bytes),
            })
        })
        .collect();

    skins.sort_by(|a, b| a.id.cmp(&b.id));
    skins.truncate(MAX_SKINS);
    skins
}

/// Guarda una skin en la galería. Si ya hay una idéntica, devuelve esa en vez
/// de duplicarla: agregar dos veces el mismo PNG es el accidente más fácil de
/// cometer con un selector de archivos.
pub fn save(png_bytes: &[u8], suggested_name: &str) -> Result<SavedSkin> {
    theme::validate_skin_png(png_bytes)?;

    let dir = skins_dir();
    std::fs::create_dir_all(&dir)?;

    if let Some(existing) = find_identical(&dir, png_bytes) {
        return Ok(existing);
    }

    if std::fs::read_dir(&dir).map(|e| e.flatten().count()).unwrap_or(0) >= MAX_SKINS {
        return Err(Error::Other(format!(
            "La galería llegó al máximo de {MAX_SKINS} skins. Borrá alguna para sumar otra."
        )));
    }

    let (id, path) = unique_path(suggested_name)?;
    std::fs::write(&path, png_bytes)?;
    Ok(SavedSkin {
        id,
        data_uri: theme::png_data_uri(png_bytes),
    })
}

fn find_identical(dir: &Path, png_bytes: &[u8]) -> Option<SavedSkin> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if std::fs::read(&path).map(|bytes| bytes != png_bytes).unwrap_or(true) {
            continue;
        }
        let Some(id) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        return Some(SavedSkin {
            id: id.to_string(),
            data_uri: theme::png_data_uri(png_bytes),
        });
    }
    None
}

pub fn read(id: &str) -> Result<Vec<u8>> {
    let path = path_of(id)?;
    std::fs::read(&path).map_err(|_| Error::Other("Esa skin ya no está guardada.".to_string()))
}

pub fn delete(id: &str) -> Result<()> {
    let path = path_of(id)?;
    let _ = std::fs::remove_file(path);
    Ok(())
}

/// Aplica una skin de la galería como la activa del juego.
pub fn apply(id: &str) -> Result<()> {
    let bytes = read(id)?;
    theme::set_custom_skin(&bytes)
}

/// Las capas de Minecraft son 64x32 (o múltiplos); las viejísimas eran 22x17.
fn validate_cape_png(bytes: &[u8]) -> Result<()> {
    let (width, height) = theme::png_dimensions(bytes)?;
    let modern = height > 0 && width == height * 2 && width >= 32;
    let legacy = width == 22 && height == 17;
    if !modern && !legacy {
        return Err(Error::Other(format!(
            "La capa tiene que ser un PNG con el doble de ancho que de alto, tipo 64x32 (el archivo elegido es {width}x{height})."
        )));
    }
    Ok(())
}

pub fn set_cape(png_bytes: &[u8]) -> Result<()> {
    validate_cape_png(png_bytes)?;
    paths::ensure_dirs()?;
    std::fs::write(cape_path(), png_bytes)?;
    Ok(())
}

pub fn clear_cape() -> Result<()> {
    let _ = std::fs::remove_file(cape_path());
    Ok(())
}

pub fn cape_data_uri() -> Option<String> {
    Some(theme::png_data_uri(&std::fs::read(cape_path()).ok()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let img = image::RgbaImage::from_pixel(width, height, image::Rgba([1, 2, 3, 255]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn slug_only_ever_produces_a_plain_name() {
        assert_eq!(slug("Mi Skin Copada.png"), "mi-skin-copada-png");
        assert_eq!(slug("../../etc/passwd"), "etc-passwd");
        assert_eq!(slug("C:\\Windows\\system32"), "c--windows-system32");
        assert_eq!(slug(""), "skin");
        assert_eq!(slug("///"), "skin");
        assert!(slug(&"a".repeat(200)).len() <= 40);
    }

    /// Aunque la UI mande un id raro, la ruta tiene que quedar adentro de la
    /// carpeta de skins y nunca en otra parte del disco.
    #[test]
    fn path_of_stays_inside_the_skins_dir() {
        for evil in ["../../evil", "C:evil", "x/C:evil", "..", "/etc/passwd"] {
            let path = path_of(evil).unwrap();
            assert_eq!(path.parent(), Some(skins_dir().as_path()), "se escapó con {evil:?}");
        }
    }

    #[test]
    fn validates_cape_dimensions() {
        assert!(validate_cape_png(&png(64, 32)).is_ok());
        assert!(validate_cape_png(&png(128, 64)).is_ok());
        assert!(validate_cape_png(&png(22, 17)).is_ok());
        assert!(validate_cape_png(&png(64, 64)).is_err());
        assert!(validate_cape_png(b"no soy un png").is_err());
    }

    #[test]
    fn rejects_skins_that_are_not_64x64() {
        assert!(theme::validate_skin_png(&png(64, 64)).is_ok());
        assert!(theme::validate_skin_png(&png(64, 32)).is_err());
    }
}
