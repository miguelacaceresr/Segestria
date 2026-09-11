use super::version_manifest::{resolve_args, ResolvedVersion};
use crate::profile::{offline_uuid, suggested_ram_mb, Settings};
use crate::{paths, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct LaunchOptions {
    pub natives_dir: PathBuf,
    /// Classpath de librerías (sin el jar del cliente, que se agrega solo).
    pub library_classpath: Vec<PathBuf>,
    pub client_jar: PathBuf,
}

/// Las "Aikar's flags": tuning de G1GC estándar de facto en toda la
/// comunidad de Minecraft para bajar los freezes/stutters del garbage
/// collector en partidas modeadas largas. https://docs.papermc.io/paper/aikars-flags
/// (pensadas para servers, pero el problema de fondo —presión de GC por
/// modpacks pesados— es el mismo del lado cliente).
const AIKAR_FLAGS: &[&str] = &[
    "-XX:+UseG1GC",
    "-XX:+ParallelRefProcEnabled",
    "-XX:MaxGCPauseMillis=200",
    "-XX:+UnlockExperimentalVMOptions",
    "-XX:+DisableExplicitGC",
    "-XX:+AlwaysPreTouch",
    "-XX:G1NewSizePercent=30",
    "-XX:G1MaxNewSizePercent=40",
    "-XX:G1HeapRegionSize=8M",
    "-XX:G1ReservePercent=20",
    "-XX:G1HeapWastePercent=5",
    "-XX:G1MixedGCCountTarget=4",
    "-XX:InitiatingHeapOccupancyPercent=15",
    "-XX:G1MixedGCLiveThresholdPercent=90",
    "-XX:G1RSetUpdatingPauseTimePercent=5",
    "-XX:SurvivorRatio=32",
    "-XX:+PerfDisableSharedMem",
    "-XX:MaxTenuringThreshold=1",
];

fn performance_jvm_args(settings: &Settings) -> Vec<String> {
    let ram_mb = settings.allocated_ram_mb.unwrap_or_else(suggested_ram_mb);

    let mut args = vec![format!("-Xms{ram_mb}M"), format!("-Xmx{ram_mb}M")];
    args.extend(AIKAR_FLAGS.iter().map(|s| s.to_string()));
    if !settings.extra_jvm_args.trim().is_empty() {
        args.extend(settings.extra_jvm_args.split_whitespace().map(str::to_string));
    }
    args
}

fn classpath_string(paths: &[PathBuf]) -> String {
    let sep = if cfg!(windows) { ";" } else { ":" };
    paths
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect::<Vec<_>>()
        .join(sep)
}

/// Nombre del placeholder de auto-conexión tal cual lo escribe Mojang en el
/// version.json: `${quickPlayMultiplayer}`, en camelCase, a diferencia de
/// todos los demás (`${auth_player_name}`, `${game_directory}`, ...) que van
/// en snake_case. El nombre del *feature* que lo habilita sí es snake_case
/// (`is_quick_play_multiplayer`), lo que hace fácil escribir mal este.
const QUICK_PLAY_MULTIPLAYER: &str = "quickPlayMultiplayer";

fn substitute(template: &str, values: &HashMap<&str, String>) -> String {
    let mut out = template.to_string();
    for (key, value) in values {
        out = out.replace(&format!("${{{key}}}"), value);
    }
    out
}

fn build_placeholder_values<'a>(
    version: &'a ResolvedVersion,
    opts: &'a LaunchOptions,
    settings: &'a Settings,
    connect_to_server: bool,
) -> HashMap<&'static str, String> {
    let mut classpath = opts.library_classpath.clone();
    classpath.push(opts.client_jar.clone());

    let mut values = HashMap::new();
    values.insert("natives_directory", opts.natives_dir.to_string_lossy().to_string());
    values.insert("launcher_name", "SegestriaLauncher".to_string());
    values.insert("launcher_version", env!("CARGO_PKG_VERSION").to_string());
    values.insert("classpath", classpath_string(&classpath));
    // Forge (modlauncher/bootstraplauncher) arma su propio module-path con
    // estos dos, además del ${classpath} de siempre: sin esto la JVM no
    // encuentra "cpw.mods.securejarhandler" y explota con
    // ExceptionInInitializerError / InaccessibleObjectException.
    values.insert("library_directory", paths::libraries_dir().to_string_lossy().to_string());
    values.insert("classpath_separator", if cfg!(windows) { ";" } else { ":" }.to_string());
    values.insert("auth_player_name", settings.username.clone());
    values.insert("version_name", version.id.clone());
    values.insert("game_directory", paths::instance_dir().to_string_lossy().to_string());
    values.insert("assets_root", paths::assets_dir().to_string_lossy().to_string());
    values.insert("assets_index_name", version.assets.clone());
    values.insert("auth_uuid", offline_uuid(&settings.username).to_string());
    values.insert("auth_access_token", "0".to_string());
    values.insert("clientid", "0".to_string());
    values.insert("auth_xuid", "0".to_string());
    values.insert("user_type", "legacy".to_string());
    values.insert("version_type", "release".to_string());

    if connect_to_server {
        values.insert(
            QUICK_PLAY_MULTIPLAYER,
            format!("{}:{}", settings.server_ip.trim(), settings.server_port),
        );
    }

    values
}

/// Arma el proceso de Minecraft listo para lanzar (sin ejecutarlo todavía),
/// resolviendo `arguments.jvm` / `arguments.game` del version.json contra las
/// reglas de SO/features vigentes (ver `rules::rules_allow`).
pub fn build_command(
    version: &ResolvedVersion,
    java_exe: &Path,
    opts: &LaunchOptions,
    settings: &Settings,
) -> Result<tokio::process::Command> {
    let connect_to_server = !settings.server_ip.trim().is_empty();

    let mut features = HashMap::new();
    features.insert("is_quick_play_multiplayer".to_string(), connect_to_server);

    let values = build_placeholder_values(version, opts, settings, connect_to_server);

    let jvm_args: Vec<String> = resolve_args(&version.arguments.jvm, &features)
        .into_iter()
        .map(|a| substitute(&a, &values))
        .collect();

    let game_args: Vec<String> = resolve_args(&version.arguments.game, &features)
        .into_iter()
        .map(|a| substitute(&a, &values))
        .collect();

    let mut cmd = tokio::process::Command::new(java_exe);
    cmd.current_dir(paths::instance_dir());
    cmd.args(performance_jvm_args(settings));
    cmd.args(&jvm_args);
    cmd.arg(&version.main_class);
    cmd.args(&game_args);

    Ok(cmd)
}

/// Lanza Minecraft como proceso independiente (no bloquea ni queda atado al
/// launcher: si cerrás Segestria, el juego sigue corriendo). stdout/stderr
/// van a un archivo de log en vez de perderse, para poder diagnosticar
/// crashes (los mismos que reportaría cualquiera en el Discord del server).
pub fn spawn_detached(mut cmd: tokio::process::Command) -> Result<tokio::process::Child> {
    use std::process::Stdio;

    // java.exe es un binario de subsistema "consola": aunque redirijamos
    // stdin/stdout/stderr, Windows igual le abre una ventana de consola
    // vacía a menos que se lo pidamos explícitamente que no lo haga.
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let log_path = latest_log_path();
    if let Some(parent) = log_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let log_file = std::fs::File::create(&log_path)?;
    let stderr_file = log_file.try_clone()?;

    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::from(log_file));
    cmd.stderr(Stdio::from(stderr_file));
    Ok(cmd.spawn()?)
}

pub fn latest_log_path() -> PathBuf {
    paths::instance_dir().join("logs").join("segestria-latest.log")
}

/// Últimos `max_bytes` del log de la partida anterior, para poder mirar un
/// crash desde el launcher sin ir a buscar el archivo. Se lee la cola y no el
/// archivo entero porque un modpack escupe megabytes de log y el stacktrace
/// que interesa siempre está al final.
pub fn read_latest_log_tail(max_bytes: usize) -> Option<String> {
    read_log_tail(&latest_log_path(), max_bytes)
}

fn read_log_tail(path: &Path, max_bytes: usize) -> Option<String> {
    use std::io::{Read, Seek, SeekFrom};

    let mut file = std::fs::File::open(path).ok()?;
    let size = file.metadata().ok()?.len();
    let start = size.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start)).ok()?;

    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;

    // El corte cae en cualquier byte, así que puede partir un carácter UTF-8
    // (y un log de Java trae acentos): from_utf8_lossy en vez de fallar.
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let text = if start > 0 {
        // Arrancar a mitad de renglón se lee como basura; empezamos en el
        // primer salto de línea completo.
        match text.find('\n') {
            Some(index) => text[index + 1..].to_string(),
            None => text,
        }
    } else {
        text
    };

    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regresión: la clave era `quick_play_multiplayer` (snake_case) mientras
    /// que el version.json trae `${quickPlayMultiplayer}`. Como no coincidían,
    /// la sustitución no pasaba nunca y Minecraft arrancaba intentando
    /// conectarse a un server llamado, literalmente,
    /// "${quickPlayMultiplayer}" — la auto-conexión al server no funcionaba.
    #[test]
    fn substitutes_the_quick_play_server() {
        let mut values = HashMap::new();
        values.insert(QUICK_PLAY_MULTIPLAYER, "mc.slav.gg:25565".to_string());

        assert_eq!(
            substitute("${quickPlayMultiplayer}", &values),
            "mc.slav.gg:25565"
        );
    }

    #[test]
    fn substitutes_every_placeholder_in_an_argument() {
        let mut values = HashMap::new();
        values.insert("auth_player_name", "gato".to_string());
        values.insert("version_name", "1.20.1-forge".to_string());

        assert_eq!(
            substitute("--user=${auth_player_name}@${version_name}", &values),
            "--user=gato@1.20.1-forge"
        );
    }

    #[test]
    fn reads_the_whole_log_when_it_is_short() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.txt");
        std::fs::write(&path, "primera\nsegunda\n").unwrap();

        assert_eq!(read_log_tail(&path, 1024).unwrap(), "primera\nsegunda\n");
    }

    /// El corte cae en cualquier byte: si arrancara ahí, el primer renglón
    /// aparecería partido a la mitad.
    #[test]
    fn skips_the_half_line_at_the_cut() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("log.txt");
        std::fs::write(&path, "primera\nsegunda\ntercera\n").unwrap();

        assert_eq!(read_log_tail(&path, 14).unwrap(), "tercera\n");
    }

    #[test]
    fn reports_nothing_when_there_is_no_log_yet() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_log_tail(&dir.path().join("no-existe.log"), 1024).is_none());
    }
}
