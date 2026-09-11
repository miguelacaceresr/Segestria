use crate::{paths, Error, Result};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Copia de seguridad de los mundos de un jugador antes de entrar a jugar.
/// Son los mundos locales (un solo jugador): lo del server vive en el server
/// y no pasa por acá.
///
/// Se guarda un .zip por partida en vez de copiar carpetas sueltas para que
/// restaurar sea "descomprimir esto acá" sin ambigüedad, y se conservan solo
/// las últimas: un mundo grande son cientos de MB y nadie quiere descubrir
/// que el launcher le llenó el disco.
const MAX_BACKUPS: usize = 5;

#[derive(Debug, Clone, Serialize)]
pub struct BackupInfo {
    pub filename: String,
    pub size_bytes: u64,
    /// Época Unix en segundos; el frontend lo muestra en hora local.
    pub epoch: u64,
}

pub fn list() -> Vec<BackupInfo> {
    let Ok(entries) = std::fs::read_dir(paths::backups_dir()) else {
        return Vec::new();
    };

    let mut backups: Vec<BackupInfo> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let filename = path.file_name()?.to_str()?.to_string();
            if !filename.ends_with(".zip") {
                return None;
            }
            let meta = entry.metadata().ok()?;
            Some(BackupInfo {
                filename,
                size_bytes: meta.len(),
                epoch: meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            })
        })
        .collect();

    backups.sort_by(|a, b| b.epoch.cmp(&a.epoch));
    backups
}

/// Comprime `saves/` entero. Devuelve `None` (sin error) si todavía no hay
/// mundos: es el caso normal de alguien que solo juega en el server.
pub fn backup_worlds() -> Result<Option<PathBuf>> {
    let saves = paths::saves_dir();
    if !has_any_world(&saves) {
        return Ok(None);
    }

    let dir = paths::backups_dir();
    std::fs::create_dir_all(&dir)?;

    let dest = dir.join(format!("mundos-{}.zip", stamp(crate::stats::now_epoch())));
    zip_dir(&saves, &dest)?;
    prune(&dir);

    Ok(Some(dest))
}

fn has_any_world(saves: &Path) -> bool {
    std::fs::read_dir(saves)
        .map(|entries| entries.flatten().any(|e| e.path().is_dir()))
        .unwrap_or(false)
}

fn prune(dir: &Path) {
    let backups = list();
    for old in backups.iter().skip(MAX_BACKUPS) {
        let _ = std::fs::remove_file(dir.join(&old.filename));
    }
}

fn zip_dir(source: &Path, dest: &Path) -> Result<()> {
    let file = std::fs::File::create(dest)?;
    let mut writer = zip::ZipWriter::new(file);
    let options: zip::write::SimpleFileOptions =
        zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let zip_error = |e: zip::result::ZipError| Error::Zip {
        path: dest.to_path_buf(),
        source: e,
    };

    for entry in walk(source) {
        let relative = entry
            .strip_prefix(source)
            .map_err(|_| Error::Other("ruta inesperada al armar el backup".to_string()))?
            .to_string_lossy()
            // Los .zip usan "/" en cualquier sistema operativo.
            .replace('\\', "/");

        writer.start_file(relative, options).map_err(zip_error)?;
        let bytes = std::fs::read(&entry)?;
        writer.write_all(&bytes)?;
    }

    writer.finish().map_err(zip_error)?;
    Ok(())
}

/// Recorrido iterativo (no recursivo) de los archivos de una carpeta: los
/// mundos de Minecraft anidan bastante y así no dependemos del stack.
fn walk(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];

    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }

    files.sort();
    files
}

/// "20260910-0314" a partir de una época Unix, para que los backups se
/// ordenen solos por nombre. Va en UTC: sumar una crate de fechas entera para
/// que el nombre del archivo coincida con el reloj de pared no vale la pena
/// (la UI muestra la fecha local, que es la que el usuario lee).
fn stamp(epoch: u64) -> String {
    let days = (epoch / 86_400) as i64;
    let seconds_of_day = epoch % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (hour, minute) = (seconds_of_day / 3600, (seconds_of_day % 3600) / 60);
    format!("{year:04}{month:02}{day:02}-{hour:02}{minute:02}")
}

/// Algoritmo `civil_from_days` de Howard Hinnant (el mismo que usa la
/// implementación de referencia de <chrono> en C++), con la era arrancando en
/// 1970-01-01. Es la parte que nadie quiere reescribir a ojo.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamp_matches_known_dates() {
        assert_eq!(stamp(0), "19700101-0000");
        // 2026-09-10 03:14 UTC
        assert_eq!(stamp(1_789_010_040), "20260910-0314");
        // Un 29 de febrero, que es donde se cae cualquier cuenta hecha a ojo.
        assert_eq!(stamp(1_709_208_000), "20240229-1200");
    }

    #[test]
    fn zips_every_file_keeping_the_folder_layout() {
        let dir = tempfile::tempdir().unwrap();
        let saves = dir.path().join("saves");
        std::fs::create_dir_all(saves.join("mundo/region")).unwrap();
        std::fs::write(saves.join("mundo/level.dat"), b"nivel").unwrap();
        std::fs::write(saves.join("mundo/region/r.0.0.mca"), b"chunks").unwrap();

        let dest = dir.path().join("backup.zip");
        zip_dir(&saves, &dest).unwrap();

        let mut archive = zip::ZipArchive::new(std::fs::File::open(&dest).unwrap()).unwrap();
        let names: Vec<String> = archive.file_names().map(str::to_string).collect();
        assert!(names.contains(&"mundo/level.dat".to_string()), "{names:?}");
        assert!(names.contains(&"mundo/region/r.0.0.mca".to_string()), "{names:?}");
        assert_eq!(archive.len(), 2);

        let mut file = archive.by_name("mundo/level.dat").unwrap();
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents).unwrap();
        assert_eq!(contents, "nivel");
    }

    #[test]
    fn reports_no_worlds_when_saves_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!has_any_world(&dir.path().join("no-existe")));

        let saves = dir.path().join("saves");
        std::fs::create_dir_all(&saves).unwrap();
        assert!(!has_any_world(&saves));

        std::fs::create_dir(saves.join("mundo")).unwrap();
        assert!(has_any_world(&saves));
    }
}
