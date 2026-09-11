use crate::{paths, Error, Result};
use base64::Engine;
use std::path::Path;

/// Resource pack que Segestria instala solo para personalizar el fondo
/// (panorama) y la música del menú principal. Por ahora trae assets
/// genéricos de relleno (ver README/manifest del launcher); reemplazarlos
/// por los definitivos es tan simple como pisar los archivos en
/// `core/assets/theme/` y recompilar.
const PACK_NAME: &str = "segestria-theme";

const PACK_MCMETA: &[u8] = include_bytes!("../assets/theme/pack.mcmeta");
const SOUNDS_JSON: &[u8] = include_bytes!("../assets/theme/sounds.json");
const MENU_THEME_OGG: &[u8] = include_bytes!("../assets/theme/menu_theme.ogg");
const PANORAMAS: [(&str, &[u8]); 6] = [
    ("panorama_0.png", include_bytes!("../assets/theme/panorama_0.png")),
    ("panorama_1.png", include_bytes!("../assets/theme/panorama_1.png")),
    ("panorama_2.png", include_bytes!("../assets/theme/panorama_2.png")),
    ("panorama_3.png", include_bytes!("../assets/theme/panorama_3.png")),
    ("panorama_4.png", include_bytes!("../assets/theme/panorama_4.png")),
    ("panorama_5.png", include_bytes!("../assets/theme/panorama_5.png")),
];

/// Variante de color del panorama del menú. Es la misma foto siempre (el
/// buzo ahogándose); lo único que cambia es el viraje de color, así el menú
/// del juego acompaña la hora a la que estás jugando.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Panorama {
    Dia,
    Atardecer,
    Noche,
}

impl Panorama {
    pub fn id(self) -> &'static str {
        match self {
            Panorama::Dia => "dia",
            Panorama::Atardecer => "atardecer",
            Panorama::Noche => "noche",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "dia" => Some(Panorama::Dia),
            "atardecer" => Some(Panorama::Atardecer),
            "noche" => Some(Panorama::Noche),
            _ => None,
        }
    }

    /// La hora la manda el frontend (`new Date().getHours()`): el core no
    /// tiene forma de saber la zona horaria del sistema sin sumar una crate
    /// de fechas solo para esto.
    pub fn for_hour(hour: u32) -> Self {
        match hour {
            6..=16 => Panorama::Dia,
            17..=20 => Panorama::Atardecer,
            _ => Panorama::Noche,
        }
    }

    /// Resuelve lo que haya guardado en settings: un id concreto manda, y
    /// cualquier otra cosa (None, "auto", un valor viejo) cae en automático.
    pub fn resolve(setting: Option<&str>, local_hour: u32) -> Self {
        setting.and_then(Self::from_id).unwrap_or_else(|| Self::for_hour(local_hour))
    }

    /// Multiplicador y offset por canal, aplicados después del difuminado.
    /// Los números salen de mirar la foto: el agua ya es azul verdosa, así
    /// que el atardecer sube rojo y hunde azul, y la noche hace lo contrario
    /// sin apagar del todo la escena. Suben un poco el brillo para compensar
    /// el `DIM` de abajo.
    fn grade(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Panorama::Dia => ([1.16, 1.16, 1.12], [8.0, 8.0, 6.0]),
            // El atardecer va a violeta y no a naranja: bajo el agua el rojo
            // es lo primero que se pierde, y hundir el azul dejaba un verde
            // oliva feo. Además pega con el azul violáceo del bicho.
            Panorama::Atardecer => ([1.22, 0.92, 0.98], [18.0, 2.0, 4.0]),
            Panorama::Noche => ([0.58, 0.70, 1.00], [0.0, 2.0, 8.0]),
        }
    }
}

/// El panorama del menú va difuminado y bajado de brillo a propósito. La foto
/// original es un recorte corrido de una sola imagen, así que el buzo aparece
/// en cuatro de las seis caras: nítida, el menú te lo cruza una y otra vez
/// mientras gira, y encima las costuras entre caras cantan. Difuminada queda
/// como ambiente —se intuye la escena, no te la grita— y de paso los botones
/// del juego se leen sin pelear con el fondo.
///
/// El difuminado se hace bajando y volviendo a subir la resolución en vez de
/// con un blur gaussiano: sobre seis imágenes de 793x793 da el mismo aspecto
/// y tarda milisegundos en lugar de segundos.
const BLUR_DIVISOR: u32 = 14;
const DIM: f32 = 0.72;

/// Archivo suelto adentro del pack con la variante que quedó escrita. Virar
/// y re-comprimir seis PNG de 793x793 cuesta ~1s; con esto solo se paga
/// cuando de verdad cambió la hora del día, no en cada arranque.
const PANORAMA_MARKER: &str = ".segestria-panorama";

/// Versión del tratamiento (difuminado + viraje). Va en el marcador junto a
/// la variante: sin esto, a quien ya tenía el pack escrito no le llegaría un
/// cambio de estilo hasta que además cambiara la hora del día. **Subir este
/// número cada vez que se toque `grade`, `BLUR_DIVISOR` o `DIM`.**
const PANORAMA_STYLE: u32 = 2;

fn marker_value(panorama: Panorama) -> String {
    format!("{} v{PANORAMA_STYLE}", panorama.id())
}

/// Escribe el resource pack de tema en la instancia y lo activa en
/// `options.txt`, dejando el panorama como está. Lo usan las acciones que
/// tocan el pack sin tener opinión sobre la hora (por ejemplo aplicar una
/// skin).
pub fn ensure_theme_pack() -> Result<()> {
    let dir = paths::instance_dir().join("resourcepacks");
    let current = current_panorama(&dir).unwrap_or(Panorama::Dia);
    apply_theme_pack(current)
}

/// Igual que `ensure_theme_pack`, pero fijando la variante del panorama. Se
/// corre en cada `play()`: es barato (solo archivos locales, nada de red) y
/// así una actualización del tema en una nueva versión del launcher se
/// aplica sola.
pub fn apply_theme_pack(panorama: Panorama) -> Result<()> {
    write_pack_files(&paths::instance_dir().join("resourcepacks"), panorama)?;
    enable_pack_in_options(&paths::instance_dir().join("options.txt"))?;
    Ok(())
}

fn current_panorama(resourcepacks_dir: &Path) -> Option<Panorama> {
    let raw = std::fs::read_to_string(resourcepacks_dir.join(PACK_NAME).join(PANORAMA_MARKER)).ok()?;
    Panorama::from_id(raw.split_whitespace().next()?)
}

/// Aplica el viraje de color a los seis lados del panorama y los escribe.
/// Se saltea entera si la variante ya es la que está en disco.
fn write_panoramas(pack_dir: &Path, panorama: Panorama) -> Result<()> {
    let marker = pack_dir.join(PANORAMA_MARKER);
    let backgrounds_dir = pack_dir.join("assets/minecraft/textures/gui/title/background");

    let expected = marker_value(panorama);
    let already_written = std::fs::read_to_string(&marker)
        .map(|raw| raw.trim() == expected)
        .unwrap_or(false);
    if already_written && backgrounds_dir.join(PANORAMAS[0].0).exists() {
        return Ok(());
    }

    std::fs::create_dir_all(&backgrounds_dir)?;
    for (name, bytes) in PANORAMAS {
        let graded = grade_png(bytes, panorama)?;
        std::fs::write(backgrounds_dir.join(name), graded)?;
    }
    std::fs::write(marker, expected)?;
    Ok(())
}

fn blur(img: &image::RgbImage) -> image::RgbImage {
    use image::imageops::FilterType;

    let small = image::imageops::resize(
        img,
        (img.width() / BLUR_DIVISOR).max(1),
        (img.height() / BLUR_DIVISOR).max(1),
        // Lanczos al bajar promedia bien y no deja moiré; Triangle al subir
        // interpola suave en vez de dejar los bloques del downscale.
        FilterType::Lanczos3,
    );
    image::imageops::resize(&small, img.width(), img.height(), FilterType::Triangle)
}

fn grade_png(bytes: &[u8], panorama: Panorama) -> Result<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    use image::{ExtendedColorType, ImageEncoder};

    let decode_error = |e: image::ImageError| Error::Other(format!("no se pudo procesar el panorama: {e}"));

    let img = image::load_from_memory(bytes).map_err(decode_error)?.to_rgb8();
    let mut img = blur(&img);

    let (mul, add) = panorama.grade();
    for pixel in img.pixels_mut() {
        for channel in 0..3 {
            let value = pixel[channel] as f32 * DIM * mul[channel] + add[channel];
            pixel[channel] = value.clamp(0.0, 255.0) as u8;
        }
    }

    // Compresión rápida a propósito: son texturas que se escriben en el
    // disco del propio usuario, no algo que viaje por la red, y con la
    // compresión default cada arranque con cambio de hora se iba a ~4s.
    let mut out = Vec::new();
    PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
        .write_image(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgb8)
        .map_err(decode_error)?;
    Ok(out)
}

/// Rutas, dentro del pack, de la textura default de skin que usa cualquier
/// jugador sin skin propia. Una cuenta offline SIEMPRE cae en uno de estos
/// dos según un hash de su UUID (ancho/"Steve" o flaco/"Alex"), y esa
/// elección no es configurable desde el juego — así que en vez de exponer un
/// selector que en la práctica no haría nada, pisamos las dos: la que el
/// cliente termine usando para ese username va a mostrar la skin elegida.
/// Se escribe también en la ruta "legacy" (sin wide/slim) por compatibilidad
/// con packs/versiones que todavía buscan ahí.
const SKIN_TEXTURE_PATHS: [&str; 4] = [
    "assets/minecraft/textures/entity/player/wide/steve.png",
    "assets/minecraft/textures/entity/player/slim/alex.png",
    "assets/minecraft/textures/entity/steve.png",
    "assets/minecraft/textures/entity/alex.png",
];

/// Instala `png_bytes` como la skin que ve el propio jugador. Es puramente
/// cosmético del lado del cliente: en un server con más gente sin skin
/// propia, todos van a terminar usando la misma textura de base a menos que
/// el server tenga un plugin de skins (algo fuera del alcance del launcher).
pub fn set_custom_skin(png_bytes: &[u8]) -> Result<()> {
    validate_skin_png(png_bytes)?;
    ensure_theme_pack()?;
    write_custom_skin(&paths::instance_dir().join("resourcepacks"), png_bytes)
}

/// Vuelve a la skin default de Minecraft, sacando nuestros overrides.
pub fn clear_custom_skin() -> Result<()> {
    clear_custom_skin_from(&paths::instance_dir().join("resourcepacks"))
}

/// PNG (crudo) de la skin activa, si hay una, para mostrarla de nuevo en la
/// UI al reabrir el launcher sin tener que guardar nada aparte en settings.
pub fn custom_skin_preview() -> Option<Vec<u8>> {
    read_custom_skin_preview(&paths::instance_dir().join("resourcepacks"))
}

fn write_custom_skin(resourcepacks_dir: &Path, png_bytes: &[u8]) -> Result<()> {
    let pack_dir = resourcepacks_dir.join(PACK_NAME);
    for rel in SKIN_TEXTURE_PATHS {
        let dest = pack_dir.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(dest, png_bytes)?;
    }
    Ok(())
}

fn clear_custom_skin_from(resourcepacks_dir: &Path) -> Result<()> {
    let pack_dir = resourcepacks_dir.join(PACK_NAME);
    for rel in SKIN_TEXTURE_PATHS {
        let _ = std::fs::remove_file(pack_dir.join(rel));
    }
    Ok(())
}

fn read_custom_skin_preview(resourcepacks_dir: &Path) -> Option<Vec<u8>> {
    std::fs::read(resourcepacks_dir.join(PACK_NAME).join(SKIN_TEXTURE_PATHS[0])).ok()
}

/// PNG crudo de la skin, en base64: el visor 3D del frontend (skinview3d)
/// hace su propio mapeo UV sobre un modelo real, así que necesita la hoja de
/// textura tal cual viene, no un recorte/composición 2D nuestro.
pub fn custom_skin_preview_data_uri() -> Option<String> {
    Some(png_data_uri(&custom_skin_preview()?))
}

pub(crate) fn png_data_uri(bytes: &[u8]) -> String {
    format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];

/// Lee ancho/alto "a mano", sin depender de un decoder completo: viven
/// siempre en bytes fijos del chunk IHDR, que por especificación es el primer
/// chunk del archivo, justo después de la firma de 8 bytes.
pub(crate) fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32)> {
    if bytes.len() < 24 || bytes[0..8] != PNG_MAGIC {
        return Err(Error::Other("El archivo elegido no es un PNG válido.".to_string()));
    }

    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    Ok((width, height))
}

pub(crate) fn validate_skin_png(bytes: &[u8]) -> Result<()> {
    let (width, height) = png_dimensions(bytes)?;
    if width != 64 || height != 64 {
        return Err(Error::Other(format!(
            "La skin tiene que ser un PNG de 64x64 (el archivo elegido es {width}x{height})."
        )));
    }
    Ok(())
}

/// Escribe los archivos del resource pack bajo `resourcepacks_dir/<pack>/...`.
/// Separado de `ensure_theme_pack` para poder testear contra un directorio
/// temporal en vez de la carpeta real de datos del launcher.
fn write_pack_files(resourcepacks_dir: &Path, panorama: Panorama) -> Result<()> {
    let pack_dir = resourcepacks_dir.join(PACK_NAME);

    std::fs::create_dir_all(&pack_dir)?;
    std::fs::write(pack_dir.join("pack.mcmeta"), PACK_MCMETA)?;

    let assets_dir = pack_dir.join("assets/minecraft");
    std::fs::create_dir_all(&assets_dir)?;
    std::fs::write(assets_dir.join("sounds.json"), SOUNDS_JSON)?;

    write_panoramas(&pack_dir, panorama)?;

    let music_dir = assets_dir.join("sounds/music");
    std::fs::create_dir_all(&music_dir)?;
    std::fs::write(music_dir.join("menu_theme.ogg"), MENU_THEME_OGG)?;

    Ok(())
}

/// Agrega `file/<pack>` a la lista `resourcePacks` de `options.txt` si no
/// está ya. Preserva el resto del archivo tal cual (u opciones default de
/// Minecraft si todavía no existe, en un primer arranque).
fn enable_pack_in_options(options_path: &Path) -> Result<()> {
    add_resource_pack_entry(options_path, &format!("file/{PACK_NAME}"))
}

/// Agrega cualquier entrada (nombre de pack de carpeta con prefijo `file/`, o
/// nombre de archivo .zip tal cual) a `resourcePacks` en `options.txt`. Lo
/// usa tanto el pack de tema propio como los texture packs que el usuario
/// suma a mano desde "Packs".
pub(crate) fn add_resource_pack_entry(options_path: &Path, entry: &str) -> Result<()> {
    let contents = std::fs::read_to_string(options_path).unwrap_or_default();

    let mut found_line = false;
    let mut lines: Vec<String> = contents
        .lines()
        .map(|line| {
            let Some(raw_list) = line.strip_prefix("resourcePacks:") else {
                return line.to_string();
            };
            found_line = true;
            let mut packs = parse_json_string_array(raw_list);
            if !packs.iter().any(|p| p == entry) {
                packs.push(entry.to_string());
            }
            format!("resourcePacks:{}", to_json_string_array(&packs))
        })
        .collect();

    if !found_line {
        lines.push(format!("resourcePacks:{}", to_json_string_array(&[entry.to_string()])));
    }

    std::fs::write(options_path, lines.join("\n") + "\n")?;
    Ok(())
}

/// Contraparte de `add_resource_pack_entry`: saca una entrada de
/// `resourcePacks` (p. ej. al borrar un texture pack agregado a mano).
pub(crate) fn remove_resource_pack_entry(options_path: &Path, entry: &str) -> Result<()> {
    let Ok(contents) = std::fs::read_to_string(options_path) else {
        return Ok(());
    };

    let lines: Vec<String> = contents
        .lines()
        .map(|line| {
            let Some(raw_list) = line.strip_prefix("resourcePacks:") else {
                return line.to_string();
            };
            let packs: Vec<String> = parse_json_string_array(raw_list)
                .into_iter()
                .filter(|p| p != entry)
                .collect();
            format!("resourcePacks:{}", to_json_string_array(&packs))
        })
        .collect();

    std::fs::write(options_path, lines.join("\n") + "\n")?;
    Ok(())
}

fn parse_json_string_array(raw: &str) -> Vec<String> {
    raw.trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .split(',')
        .map(|s| s.trim().trim_matches('"').to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn to_json_string_array(values: &[String]) -> String {
    // En Windows un nombre de archivo no puede tener comillas ni saltos de
    // línea, pero `core` es multiplataforma y en Linux/macOS sí puede: sin
    // escapar, un nombre así rompería la lista de `options.txt`.
    let quoted: Vec<String> = values
        .iter()
        .map(|v| format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ")))
        .collect();
    format!("[{}]", quoted.join(","))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_entry_to_empty_options() {
        let dir = tempfile::tempdir().unwrap();
        let options_path = dir.path().join("options.txt");
        std::fs::write(&options_path, "version:3839\nfov:0.0\n").unwrap();

        enable_pack_in_options(&options_path).unwrap();

        let contents = std::fs::read_to_string(&options_path).unwrap();
        assert!(contents.contains("resourcePacks:[\"file/segestria-theme\"]"));
        assert!(contents.contains("fov:0.0"));
    }

    #[test]
    fn preserves_existing_packs_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let options_path = dir.path().join("options.txt");
        std::fs::write(&options_path, "resourcePacks:[\"programmer_art.zip\"]\n").unwrap();

        enable_pack_in_options(&options_path).unwrap();
        enable_pack_in_options(&options_path).unwrap();

        let contents = std::fs::read_to_string(&options_path).unwrap();
        let line = contents.lines().find(|l| l.starts_with("resourcePacks:")).unwrap();
        assert_eq!(line, "resourcePacks:[\"programmer_art.zip\",\"file/segestria-theme\"]");
    }

    #[test]
    fn creates_options_file_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let options_path = dir.path().join("options.txt");

        enable_pack_in_options(&options_path).unwrap();

        let contents = std::fs::read_to_string(&options_path).unwrap();
        assert_eq!(contents, "resourcePacks:[\"file/segestria-theme\"]\n");
    }

    fn fake_skin_png(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = PNG_MAGIC.to_vec();
        bytes.extend_from_slice(&[0, 0, 0, 0]); // largo de chunk (no importa para el chequeo)
        bytes.extend_from_slice(b"IHDR");
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    #[test]
    fn validates_skin_dimensions() {
        assert!(validate_skin_png(&fake_skin_png(64, 64)).is_ok());
        assert!(validate_skin_png(&fake_skin_png(64, 32)).is_err());
        assert!(validate_skin_png(&[0u8; 10]).is_err());
    }

    #[test]
    fn writes_and_clears_custom_skin() {
        let dir = tempfile::tempdir().unwrap();
        let png = fake_skin_png(64, 64);

        write_custom_skin(dir.path(), &png).unwrap();
        for rel in SKIN_TEXTURE_PATHS {
            assert_eq!(std::fs::read(dir.path().join(PACK_NAME).join(rel)).unwrap(), png);
        }
        assert_eq!(read_custom_skin_preview(dir.path()).unwrap(), png);

        clear_custom_skin_from(dir.path()).unwrap();
        for rel in SKIN_TEXTURE_PATHS {
            assert!(!dir.path().join(PACK_NAME).join(rel).exists());
        }
        assert!(read_custom_skin_preview(dir.path()).is_none());
    }

    #[test]
    fn writes_full_pack_layout() {
        let dir = tempfile::tempdir().unwrap();

        write_pack_files(dir.path(), Panorama::Dia).unwrap();

        let pack_dir = dir.path().join(PACK_NAME);
        assert!(pack_dir.join("pack.mcmeta").exists());
        assert!(pack_dir.join("assets/minecraft/sounds.json").exists());
        assert!(pack_dir.join("assets/minecraft/sounds/music/menu_theme.ogg").exists());
        for i in 0..6 {
            assert!(pack_dir
                .join(format!("assets/minecraft/textures/gui/title/background/panorama_{i}.png"))
                .exists());
        }
        assert_eq!(current_panorama(dir.path()), Some(Panorama::Dia));
    }

    #[test]
    fn panorama_follows_the_clock_unless_the_user_picked_one() {
        assert_eq!(Panorama::resolve(None, 10), Panorama::Dia);
        assert_eq!(Panorama::resolve(None, 19), Panorama::Atardecer);
        assert_eq!(Panorama::resolve(None, 3), Panorama::Noche);
        assert_eq!(Panorama::resolve(Some("auto"), 3), Panorama::Noche);
        assert_eq!(Panorama::resolve(Some("dia"), 3), Panorama::Dia);
    }

    fn solid_png(r: u8, g: u8, b: u8) -> Vec<u8> {
        let img = image::RgbImage::from_pixel(4, 4, image::Rgb([r, g, b]));
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, image::ImageFormat::Png).unwrap();
        out.into_inner()
    }

    #[test]
    fn blur_softens_hard_edges() {
        let mut img = image::RgbImage::new(56, 56);
        for (x, _y, pixel) in img.enumerate_pixels_mut() {
            *pixel = if x < 28 { image::Rgb([0, 0, 0]) } else { image::Rgb([255, 255, 255]) };
        }

        let blurred = blur(&img);
        let edge = blurred.get_pixel(28, 28).0[0];
        assert!(edge > 10 && edge < 245, "el borde tendría que quedar difuso, y quedó en {edge}");
    }

    #[test]
    fn grading_darkens_at_night_and_warms_at_sunset() {
        let source = solid_png(120, 120, 120);
        let pixel_of = |bytes: &[u8]| image::load_from_memory(bytes).unwrap().to_rgb8().get_pixel(0, 0).0;

        let night = pixel_of(&grade_png(&source, Panorama::Noche).unwrap());
        let sunset = pixel_of(&grade_png(&source, Panorama::Atardecer).unwrap());

        assert!(night[0] < 120 && night[2] > night[0], "la noche va oscura y azulada: {night:?}");
        assert!(sunset[0] > sunset[2], "el atardecer va cálido: {sunset:?}");
    }

    /// Virar y re-comprimir seis PNG grandes cuesta ~1s: si la variante no
    /// cambió, el segundo arranque tiene que salir sin tocar nada.
    #[test]
    fn skips_rewriting_panoramas_when_the_variant_did_not_change() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join(PACK_NAME);
        let first_side = pack_dir.join("assets/minecraft/textures/gui/title/background/panorama_0.png");

        write_pack_files(dir.path(), Panorama::Noche).unwrap();
        let written_at = std::fs::metadata(&first_side).unwrap().modified().unwrap();

        write_pack_files(dir.path(), Panorama::Noche).unwrap();
        assert_eq!(std::fs::metadata(&first_side).unwrap().modified().unwrap(), written_at);

        write_pack_files(dir.path(), Panorama::Dia).unwrap();
        assert_eq!(current_panorama(dir.path()), Some(Panorama::Dia));
    }

    /// Si el marcador solo guardara la variante, a quien ya tiene el pack
    /// escrito no le llegaría un cambio de tratamiento (difuminado, viraje)
    /// hasta que además cambiara la hora del día.
    #[test]
    fn rewrites_panoramas_when_the_style_changed() {
        let dir = tempfile::tempdir().unwrap();
        let pack_dir = dir.path().join(PACK_NAME);
        let first_side = pack_dir.join("assets/minecraft/textures/gui/title/background/panorama_0.png");

        write_pack_files(dir.path(), Panorama::Noche).unwrap();
        let stale = std::fs::read(&first_side).unwrap();

        // Simula un pack escrito por una versión anterior del launcher.
        std::fs::write(pack_dir.join(PANORAMA_MARKER), "noche v1").unwrap();
        std::fs::write(&first_side, b"textura vieja").unwrap();

        write_pack_files(dir.path(), Panorama::Noche).unwrap();

        assert_eq!(std::fs::read(&first_side).unwrap(), stale, "tendría que haberla redibujado");
        assert_eq!(current_panorama(dir.path()), Some(Panorama::Noche));
    }
}
