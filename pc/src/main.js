const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

const STAGE_LABELS = {
  backing_up_worlds: "Copiando tus mundos",
  checking_java: "Revisando Java",
  downloading_client: "Descargando Minecraft",
  downloading_libraries: "Descargando librerías",
  downloading_assets: "Descargando assets",
  installing_forge: "Instalando Forge",
  syncing_mods: "Sincronizando mods",
  launching: "Iniciando",
};

const SERVER_PING_INTERVAL_MS = 15000;

let usernameEl,
  serverEl,
  noServerToggle,
  playButton,
  progressWrap,
  progressFill,
  statusText,
  errorText,
  extraJvmEl,
  serverStatusDot,
  serverStatusTitle,
  serverStatusText,
  serverIconImg,
  serverIconFallback,
  skinAvatar,
  newsSection,
  newsTrack,
  newsDots,
  modsPanel,
  modsList,
  ramAutoToggle,
  ramSlider,
  ramValueLabel,
  skinPickButton,
  skinClearButton,
  skinViewerCanvas,
  themeSwatches,
  fontSelect,
  wordmarkImg,
  tabButtons,
  tabPanels,
  texturePacksList,
  texturePackAddButton,
  shaderPacksList,
  shaderPackAddButton,
  serverPlayers,
  copyIpButton,
  skinGallery,
  capePickButton,
  capeClearButton,
  panoramaSelect,
  soundToggle,
  backupToggle,
  backupNowButton,
  backupsList,
  statTotal,
  statSessions,
  statLast,
  statLastLabel,
  crashNote,
  viewLogButton,
  logModal,
  logContent,
  logCloseButton,
  logCopyButton,
  appVersionLabel,
  checkUpdateButton,
  updateStatus,
  updateDownloadButton;

let pingTimer = null;
let skinDebounceTimer = null;
let skinFallbackUsed = false;
let ramSuggestedMb = 2048;
let skinViewer = null;
let hasCustomSkin = false;
let activeSkinId = null;
let pendingUpdateUrl = null;
let clickSound = null;

const SKIN_AVATAR_SIZE = 64;
const PLAYER_HEAD_SIZE = 32;

function skinAvatarUrl(name) {
  return `https://mc-heads.net/avatar/${encodeURIComponent(name)}/${SKIN_AVATAR_SIZE}`;
}

function skinTextureUrl(name) {
  return `https://mc-heads.net/skin/${encodeURIComponent(name)}`;
}

// Preview del skin: como las cuentas acá son offline, esto solo muestra algo
// real si el nombre coincide con una cuenta de Mojang existente; si no,
// mc-heads.net devuelve el Steve/Alex de siempre — no es un error, es lo
// esperado (ver README sobre el login offline).
function updateSkinPreview() {
  skinFallbackUsed = false;
  const name = usernameEl.value.trim() || "Steve";
  skinAvatar.src = skinAvatarUrl(name);
  // Si hay una skin propia cargada, esa manda: no la pisamos solo porque
  // cambió el nombre de usuario.
  if (!hasCustomSkin && skinViewer) {
    skinViewer.loadSkin(skinTextureUrl(name));
  }
}

function splitServer(value) {
  const trimmed = value.trim();
  if (!trimmed) return { ip: "", port: 25565 };
  const [ip, port] = trimmed.split(":");
  return { ip, port: port ? parseInt(port, 10) : 25565 };
}

function setBusy(busy) {
  playButton.disabled = busy;
  usernameEl.disabled = busy;
  serverEl.disabled = busy;
  progressWrap.classList.toggle("hidden", !busy);
}

function showError(message) {
  errorText.textContent = message;
  errorText.classList.remove("hidden");
}

function currentSettings() {
  const { ip, port } = noServerToggle.checked ? { ip: "", port: 25565 } : splitServer(serverEl.value);
  return {
    username: usernameEl.value.trim(),
    server_ip: ip,
    server_port: port,
    // null = el core elige la RAM óptima según la PC (ver profile::suggested_ram_mb).
    allocated_ram_mb: ramAutoToggle.checked ? null : Number(ramSlider.value),
    extra_jvm_args: extraJvmEl.value.trim(),
    backup_worlds: backupToggle.checked,
    panorama: panoramaSelect.value,
  };
}

// Las preferencias se guardaban solo al apretar Jugar. Para las que cambian
// algo que no es "esta partida" (copias de mundos, fondo del menú) eso
// significaba perderlas si cerrabas el launcher sin jugar.
async function persistSettings() {
  try {
    await invoke("save_settings", { settings: currentSettings() });
  } catch (err) {
    console.error("no se pudieron guardar los ajustes", err);
  }
}

// Pide los límites de RAM de la PC (total instalada / valor sugerido) para
// calibrar el slider de "Avanzado". Es una llamada local (sysinfo), sin red.
async function loadRamInfo() {
  const info = await invoke("get_ram_info");
  ramSuggestedMb = info.suggested_mb;
  ramSlider.min = "1024";
  ramSlider.max = String(Math.max(2048, Math.min(info.total_mb - 1024, 16384)));
}

function updateRamValueLabel() {
  const mb = ramAutoToggle.checked ? ramSuggestedMb : Number(ramSlider.value);
  const gb = (mb / 1024).toFixed(1);
  ramValueLabel.textContent = ramAutoToggle.checked ? `Automático (${gb} GB)` : `${gb} GB`;
}

function applyRamSettings(allocatedMb) {
  if (allocatedMb) {
    ramAutoToggle.checked = false;
    ramSlider.disabled = false;
    const clamped = Math.min(Math.max(allocatedMb, Number(ramSlider.min)), Number(ramSlider.max));
    ramSlider.value = String(clamped);
  } else {
    ramAutoToggle.checked = true;
    ramSlider.disabled = true;
    ramSlider.value = String(ramSuggestedMb);
  }
  updateRamValueLabel();
}

async function loadSettings() {
  const [settings] = await Promise.all([invoke("get_settings"), loadRamInfo()]);
  usernameEl.value = settings.username ?? "";
  serverEl.value = settings.server_ip ? `${settings.server_ip}:${settings.server_port}` : "";
  extraJvmEl.value = settings.extra_jvm_args ?? "";
  backupToggle.checked = Boolean(settings.backup_worlds);
  panoramaSelect.value = settings.panorama ?? "auto";
  // Un valor viejo o desconocido deja el select en blanco; mejor automático.
  if (!panoramaSelect.value) panoramaSelect.value = "auto";
  setNoServerMode(!settings.server_ip);
  applyRamSettings(settings.allocated_ram_mb);
  updateSkinPreview();
}

function setServerIcon(dataUri) {
  if (dataUri) {
    serverIconImg.src = dataUri;
    serverIconImg.classList.remove("hidden");
    serverIconFallback.classList.add("hidden");
  } else {
    serverIconImg.classList.add("hidden");
    serverIconFallback.classList.remove("hidden");
  }
}

function renderServerStatus(status) {
  serverStatusDot.classList.remove("status-dot--online", "status-dot--offline", "status-dot--unknown");
  setServerIcon(status.favicon);
  if (status.online) {
    serverStatusDot.classList.add("status-dot--online");
    serverStatusTitle.textContent = status.motd || "Servidor";
    serverStatusText.textContent = `Online — ${status.players_online}/${status.players_max} jugadores`;
  } else {
    serverStatusDot.classList.add("status-dot--offline");
    serverStatusTitle.textContent = "Servidor";
    serverStatusText.textContent = "Offline o no responde";
  }
  renderPlayers(status);
}

// El "sample" del protocolo son algunos de los conectados, no todos: vanilla
// manda hasta 12 al azar y varios servers lo apagan. Cuando no viene nada, la
// tira no se muestra en vez de mentir con una lista vacía.
function renderPlayers(status) {
  serverPlayers.textContent = "";
  const players = status.online ? status.players ?? [] : [];
  if (players.length === 0) {
    serverPlayers.classList.add("hidden");
    return;
  }

  for (const player of players) {
    const head = document.createElement("img");
    head.className = "player-head";
    head.src = `https://mc-heads.net/avatar/${encodeURIComponent(player.uuid)}/${PLAYER_HEAD_SIZE}`;
    head.alt = player.name;
    head.title = player.name;
    serverPlayers.appendChild(head);
  }

  const hidden = (status.players_online ?? players.length) - players.length;
  if (hidden > 0) {
    const note = document.createElement("span");
    note.className = "server-players-note";
    note.textContent = `+${hidden}`;
    serverPlayers.appendChild(note);
  }

  serverPlayers.classList.remove("hidden");
}

// navigator.clipboard puede no estar disponible según cómo sirva la ventana
// sus archivos, así que queda el camino viejo del textarea como respaldo.
async function copyToClipboard(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (err) {
    const helper = document.createElement("textarea");
    helper.value = text;
    helper.setAttribute("readonly", "");
    helper.style.position = "fixed";
    helper.style.opacity = "0";
    document.body.appendChild(helper);
    helper.select();
    const copied = document.execCommand("copy");
    helper.remove();
    return copied;
  }
}

async function copyServerIp() {
  const value = serverEl.value.trim();
  if (!value) return;
  if (!(await copyToClipboard(value))) return;

  copyIpButton.classList.add("is-done");
  setTimeout(() => copyIpButton.classList.remove("is-done"), 1200);
}

function setNoServerMode(enabled) {
  noServerToggle.checked = enabled;
  serverEl.disabled = enabled;

  if (!enabled) {
    scheduleServerPing();
    return;
  }

  if (pingTimer) clearInterval(pingTimer);
  serverStatusDot.classList.remove("status-dot--online", "status-dot--offline");
  serverStatusDot.classList.add("status-dot--unknown");
  serverIconFallback.textContent = "S";
  setServerIcon(null);
  serverPlayers.classList.add("hidden");
  serverStatusTitle.textContent = "Sin servidor";
  serverStatusText.textContent = "Vas a jugar sin conectarte a un server";
}

async function refreshServerStatus() {
  const { ip, port } = splitServer(serverEl.value);
  if (!ip) {
    serverStatusDot.classList.remove("status-dot--online", "status-dot--offline");
    serverStatusDot.classList.add("status-dot--unknown");
    serverIconFallback.textContent = "?";
    setServerIcon(null);
    serverPlayers.classList.add("hidden");
    serverStatusTitle.textContent = "Sin servidor";
    serverStatusText.textContent = "Configurá la IP del server";
    return;
  }
  serverIconFallback.textContent = ip[0].toUpperCase();
  const status = await invoke("ping_server", { ip, port });
  renderServerStatus(status);
}

function scheduleServerPing() {
  if (pingTimer) clearInterval(pingTimer);
  refreshServerStatus();
  pingTimer = setInterval(refreshServerStatus, SERVER_PING_INTERVAL_MS);
}

const NEWS_AUTO_ADVANCE_MS = 6000;

let newsSlideCount = 0;
let newsCurrentIndex = 0;
let newsAutoTimer = null;
let newsScrollDebounce = null;

// Mueve solo la pista horizontal. Con scrollIntoView el navegador además
// arrastraba la página entera hacia abajo en cada auto-avance.
function goToNewsSlide(index) {
  newsTrack.scrollTo({ left: index * newsTrack.clientWidth, behavior: "smooth" });
}

function updateActiveNewsDot(index) {
  Array.from(newsDots.children).forEach((dot, i) => dot.classList.toggle("is-active", i === index));
}

// Carrusel: pasa sola cada 6s, pero cada vez que el usuario scrollea/swipea
// a mano el temporizador se reinicia desde ahí en vez de pelearse con él.
function scheduleNewsAutoAdvance() {
  clearTimeout(newsAutoTimer);
  if (newsSlideCount <= 1) return;
  newsAutoTimer = setTimeout(() => {
    goToNewsSlide((newsCurrentIndex + 1) % newsSlideCount);
  }, NEWS_AUTO_ADVANCE_MS);
}

function renderNewsCarousel(items) {
  newsTrack.innerHTML = "";
  newsDots.innerHTML = "";

  items.forEach((item, index) => {
    const slide = document.createElement("div");
    slide.className = "news-slide";
    // Con textContent en vez de innerHTML: hoy las novedades vienen embebidas
    // en el binario, pero el día que se bajen de la nube esto sería XSS.
    for (const [cls, tag, value] of [
      ["news-item-title", "div", item.title],
      ["news-item-date", "div", item.date],
      ["news-item-body", "p", item.body],
    ]) {
      const el = document.createElement(tag);
      el.className = cls;
      el.textContent = value ?? "";
      slide.appendChild(el);
    }
    newsTrack.appendChild(slide);

    const dot = document.createElement("button");
    dot.type = "button";
    dot.className = "news-dot";
    dot.setAttribute("aria-label", `Novedad ${index + 1}`);
    dot.addEventListener("click", () => goToNewsSlide(index));
    newsDots.appendChild(dot);
  });

  updateActiveNewsDot(0);
}

async function loadNews() {
  try {
    const feed = await invoke("get_news");
    if (!feed.items || feed.items.length === 0) return;

    newsSlideCount = feed.items.length;
    newsCurrentIndex = 0;
    renderNewsCarousel(feed.items);
    newsSection.classList.remove("hidden");
    scheduleNewsAutoAdvance();
  } catch (err) {
    console.error("no se pudieron cargar las novedades", err);
  }
}

async function loadMods() {
  try {
    const mods = await invoke("get_mods");
    if (!mods || mods.length === 0) return;
    modsList.innerHTML = "";
    for (const mod of mods) {
      const li = document.createElement("li");
      li.className = "mods-list-item";
      li.textContent = mod.name;
      modsList.appendChild(li);
    }
    modsPanel.classList.remove("hidden");
  } catch (err) {
    console.error("no se pudo cargar la lista de mods", err);
  }
}

// Visor 3D real (skinview3d, ver vendor/skinview3d.bundle.js) en vez de una
// imagen 2D: hace su propio mapeo UV sobre un modelo, con drag-to-rotate
// gratis. Se crea una sola vez; después solo le cambiamos la textura.
function initSkinViewer() {
  // Dimensiones fijas (no clientWidth/clientHeight): al arrancar, la pestaña
  // "Skin" todavía está con display:none (no es la activa por default), así
  // que el canvas todavía no tiene layout y esas propiedades darían 0.
  skinViewer = new window.skinview3d.SkinViewer({
    canvas: skinViewerCanvas,
    width: 112,
    height: 190,
  });
  skinViewer.autoRotate = true;
  skinViewer.autoRotateSpeed = 0.8;
  skinViewer.zoom = 0.85;
  skinViewer.fov = 55;
  skinViewer.controls.enableZoom = false;
}

function renderCustomSkin(dataUri) {
  hasCustomSkin = true;
  skinViewer.loadSkin(dataUri);
  skinClearButton.classList.remove("hidden");
}

function resetSkinPreview() {
  hasCustomSkin = false;
  skinViewer.loadSkin(skinTextureUrl(usernameEl.value.trim() || "Steve"));
  skinClearButton.classList.add("hidden");
}

async function loadSkinPreview() {
  try {
    const dataUri = await invoke("get_custom_skin_preview");
    if (!dataUri) {
      resetSkinPreview();
      return;
    }
    renderCustomSkin(dataUri);
    // La skin activa vive en el resource pack, no en la galería; se sabe cuál
    // es comparando el PNG contra los guardados.
    activeSkinId = skinGalleryItems.find((skin) => skin.data_uri === dataUri)?.id ?? null;
    renderSkinGallery(skinGalleryItems);
  } catch (err) {
    console.error("no se pudo cargar la skin personalizada", err);
  }
}

async function loadSkinTab() {
  await loadSkinGallery();
  await loadSkinPreview();
  await loadCape();
}

async function pickPng(dialogTitle) {
  try {
    return await window.__TAURI__.dialog.open({
      multiple: false,
      filters: [{ name: dialogTitle, extensions: ["png"] }],
    });
  } catch (err) {
    console.error("no se pudo abrir el selector de archivos", err);
    return null;
  }
}

async function pickSkin() {
  const path = await pickPng("Skin PNG");
  if (!path) return;

  try {
    // Entra a la galería y queda puesta de una: quien abre el selector lo
    // hace para ponerse esa skin, no para archivarla.
    const saved = await invoke("add_skin", { path });
    activeSkinId = saved.id;
    renderCustomSkin(saved.data_uri);
    await loadSkinGallery();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo aplicar la skin.");
  }
}

async function clearSkin() {
  try {
    await invoke("clear_custom_skin");
    activeSkinId = null;
    resetSkinPreview();
    renderSkinGallery(skinGalleryItems);
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo quitar la skin.");
  }
}

let skinGalleryItems = [];

async function loadSkinGallery() {
  try {
    skinGalleryItems = await invoke("list_skins");
    renderSkinGallery(skinGalleryItems);
  } catch (err) {
    console.error("no se pudo cargar la galería de skins", err);
  }
}

function renderSkinGallery(skins) {
  skinGallery.textContent = "";
  if (skins.length === 0) {
    skinGallery.classList.add("hidden");
    return;
  }

  for (const skin of skins) {
    const chip = document.createElement("div");
    chip.className = "skin-chip";
    chip.classList.toggle("is-active", skin.id === activeSkinId);

    // La cara y el sombrero salen de recortar la misma hoja de textura por
    // CSS (ver .skin-chip-apply), así que no hace falta generar miniaturas.
    const apply = document.createElement("button");
    apply.type = "button";
    apply.className = "skin-chip-apply";
    apply.style.backgroundImage = `url("${skin.data_uri}")`;
    apply.title = "Usar esta skin";
    apply.addEventListener("click", () => applySkinFromGallery(skin.id));

    const hat = document.createElement("span");
    hat.className = "skin-chip-hat";
    hat.style.backgroundImage = `url("${skin.data_uri}")`;

    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "skin-chip-remove";
    remove.textContent = "×";
    remove.title = "Sacarla de la galería";
    remove.addEventListener("click", () => deleteSkinFromGallery(skin.id));

    chip.append(apply, hat, remove);
    skinGallery.appendChild(chip);
  }

  const add = document.createElement("button");
  add.type = "button";
  add.className = "skin-chip-add";
  add.textContent = "+";
  add.title = "Sumar otra skin";
  add.addEventListener("click", pickSkin);
  skinGallery.appendChild(add);

  skinGallery.classList.remove("hidden");
}

async function applySkinFromGallery(id) {
  try {
    const dataUri = await invoke("apply_skin", { id });
    activeSkinId = id;
    renderCustomSkin(dataUri);
    renderSkinGallery(skinGalleryItems);
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo aplicar la skin.");
  }
}

async function deleteSkinFromGallery(id) {
  try {
    await invoke("delete_skin", { id });
    if (id === activeSkinId) activeSkinId = null;
    await loadSkinGallery();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo borrar la skin.");
  }
}

// La capa no se ve adentro del juego (Minecraft las toma de la cuenta de
// Mojang, no de un resource pack): es para el visor de acá.
function renderCape(dataUri) {
  skinViewer.loadCape(dataUri);
  capeClearButton.classList.remove("hidden");
}

async function loadCape() {
  try {
    const dataUri = await invoke("get_cape");
    if (dataUri) renderCape(dataUri);
  } catch (err) {
    console.error("no se pudo cargar la capa", err);
  }
}

async function pickCape() {
  const path = await pickPng("Capa PNG");
  if (!path) return;

  try {
    renderCape(await invoke("set_cape", { path }));
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo aplicar la capa.");
  }
}

async function clearCape() {
  try {
    await invoke("clear_cape");
    skinViewer.resetCape();
    capeClearButton.classList.add("hidden");
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo quitar la capa.");
  }
}

// Tema y fuente viven en localStorage: son preferencias visuales de esta PC,
// no configuración del juego, así que no pasan por el backend.
const THEME_KEY = "segestria:theme";
const FONT_KEY = "segestria:font";
const DEFAULT_THEME = "veneno";
const DEFAULT_FONT = "manrope";

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  for (const swatch of themeSwatches) {
    swatch.classList.toggle("is-active", swatch.dataset.theme === theme);
  }
  // El wordmark es una imagen con las letras horneadas: en el tema claro hace
  // falta la variante de letras oscuras o no se leería.
  wordmarkImg.src = theme === "seda" ? "assets/segestria-wordmark-dark.png" : "assets/segestria-wordmark.png";
}

function applyFont(font) {
  document.documentElement.dataset.font = font;
  fontSelect.value = font;
}

function setActiveTab(tabId) {
  for (const button of tabButtons) {
    button.classList.toggle("is-active", button.dataset.tab === tabId);
  }
  for (const panel of tabPanels) {
    panel.classList.toggle("is-active", panel.dataset.tabPanel === tabId);
  }
}


function renderPackList(listEl, filenames, onRemove) {
  listEl.innerHTML = "";
  if (filenames.length === 0) {
    const empty = document.createElement("li");
    empty.className = "pack-list-empty";
    empty.textContent = "Todavía no agregaste ninguno.";
    listEl.appendChild(empty);
    return;
  }
  for (const filename of filenames) {
    const li = document.createElement("li");
    li.className = "pack-list-item";

    const name = document.createElement("span");
    name.className = "pack-list-name";
    name.textContent = filename;

    const removeButton = document.createElement("button");
    removeButton.type = "button";
    removeButton.className = "pack-list-remove";
    removeButton.textContent = "Quitar";
    removeButton.addEventListener("click", () => onRemove(filename));

    li.append(name, removeButton);
    listEl.appendChild(li);
  }
}

async function pickZip(dialogTitle) {
  try {
    return await window.__TAURI__.dialog.open({
      multiple: false,
      filters: [{ name: dialogTitle, extensions: ["zip"] }],
    });
  } catch (err) {
    console.error("no se pudo abrir el selector de archivos", err);
    return null;
  }
}

async function loadTexturePacks() {
  try {
    const packs = await invoke("list_texture_packs");
    renderPackList(texturePacksList, packs, removeTexturePack);
  } catch (err) {
    console.error("no se pudieron cargar los texture packs", err);
  }
}

async function addTexturePack() {
  const path = await pickZip("Texture pack");
  if (!path) return;
  try {
    await invoke("add_texture_pack", { path });
    loadTexturePacks();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo agregar el texture pack.");
  }
}

async function removeTexturePack(filename) {
  try {
    await invoke("remove_texture_pack", { filename });
    loadTexturePacks();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo quitar el texture pack.");
  }
}

async function loadShaderPacks() {
  try {
    const packs = await invoke("list_shader_packs");
    renderPackList(shaderPacksList, packs, removeShaderPack);
  } catch (err) {
    console.error("no se pudieron cargar los shaders", err);
  }
}

async function addShaderPack() {
  const path = await pickZip("Shader pack");
  if (!path) return;
  try {
    await invoke("add_shader_pack", { path });
    loadShaderPacks();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo agregar el shader.");
  }
}

async function removeShaderPack(filename) {
  try {
    await invoke("remove_shader_pack", { filename });
    loadShaderPacks();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo quitar el shader.");
  }
}

function formatDuration(seconds) {
  if (!seconds) return "—";
  const hours = Math.floor(seconds / 3600);
  const minutes = Math.round((seconds % 3600) / 60);
  if (hours === 0) return `${Math.max(minutes, 1)} m`;
  if (minutes === 0) return `${hours} h`;
  return `${hours} h ${minutes} m`;
}

function formatDate(epoch) {
  if (!epoch) return "";
  return new Date(epoch * 1000).toLocaleDateString("es", { day: "numeric", month: "short" });
}

function formatSize(bytes) {
  const mb = bytes / (1024 * 1024);
  return mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.max(mb, 0.1).toFixed(1)} MB`;
}

// Las estadísticas las escribe el backend cuando el juego se cierra, así que
// se releen al volver a la ventana en vez de quedarse con el número viejo.
async function loadStats() {
  try {
    const stats = await invoke("get_stats");
    statTotal.textContent = formatDuration(stats.total_seconds);
    statSessions.textContent = String(stats.sessions ?? 0);
    statLast.textContent = formatDuration(stats.last_session_seconds);

    const date = formatDate(stats.last_played_epoch);
    statLastLabel.textContent = date ? `Última sesión · ${date}` : "Última sesión";

    const crashed = typeof stats.last_exit_code === "number" && stats.last_exit_code !== 0;
    crashNote.classList.toggle("hidden", !crashed);
  } catch (err) {
    console.error("no se pudieron cargar las estadísticas", err);
  }
}

async function openLogModal() {
  logContent.textContent = "Leyendo...";
  logModal.classList.remove("hidden");
  document.body.classList.add("no-scroll");

  try {
    const log = await invoke("get_latest_log");
    logContent.textContent = log || "Todavía no hay log: entrá a jugar una vez y volvé.";
    logContent.scrollTop = logContent.scrollHeight;
  } catch (err) {
    logContent.textContent = "No se pudo leer el log.";
  }
}

function closeLogModal() {
  logModal.classList.add("hidden");
  document.body.classList.remove("no-scroll");
}

async function loadBackups() {
  try {
    const backups = await invoke("list_backups");
    backupsList.textContent = "";

    if (backups.length === 0) {
      const empty = document.createElement("li");
      empty.className = "pack-list-empty";
      empty.textContent = "Todavía no hay copias.";
      backupsList.appendChild(empty);
      return;
    }

    for (const backup of backups) {
      const li = document.createElement("li");
      li.className = "pack-list-item";

      const name = document.createElement("span");
      name.className = "pack-list-name";
      name.textContent = formatDate(backup.epoch) || backup.filename;
      name.title = backup.filename;

      const size = document.createElement("span");
      size.className = "server-players-note";
      size.textContent = formatSize(backup.size_bytes);

      li.append(name, size);
      backupsList.appendChild(li);
    }
  } catch (err) {
    console.error("no se pudieron cargar las copias", err);
  }
}

async function backupNow() {
  backupNowButton.disabled = true;
  const previousLabel = backupNowButton.textContent;
  backupNowButton.textContent = "Copiando...";

  try {
    const filename = await invoke("backup_now");
    if (!filename) showError("No hay mundos de un jugador para copiar todavía.");
    await loadBackups();
  } catch (err) {
    showError(typeof err === "string" ? err : "No se pudo hacer la copia.");
  } finally {
    backupNowButton.disabled = false;
    backupNowButton.textContent = previousLabel;
  }
}

async function checkUpdate() {
  checkUpdateButton.disabled = true;
  updateStatus.textContent = "Buscando...";
  updateDownloadButton.classList.add("hidden");

  try {
    const update = await invoke("check_update");
    if (!update) {
      updateStatus.textContent = "Estás en la última versión.";
      return;
    }
    pendingUpdateUrl = update.url;
    updateStatus.textContent = update.notes
      ? `Hay una v${update.version}: ${update.notes}`
      : `Hay una v${update.version} lista para bajar.`;
    updateDownloadButton.classList.remove("hidden");
  } catch (err) {
    updateStatus.textContent = "No se pudo consultar (¿sin internet?).";
  } finally {
    checkUpdateButton.disabled = false;
  }
}

async function loadAppVersion() {
  try {
    appVersionLabel.textContent = `Segestria v${await invoke("get_app_version")}`;
  } catch (err) {
    console.error("no se pudo leer la versión", err);
  }
}

function openFolder(which) {
  invoke("open_folder", { which }).catch((err) => {
    showError(typeof err === "string" ? err : "No se pudo abrir la carpeta.");
  });
}

// El sonido del botón Jugar. Se crea una sola vez y se rebobina en cada
// clic: si no, apretar dos veces seguidas no suena la segunda.
const SOUND_KEY = "segestria:sound";

function isSoundEnabled() {
  return localStorage.getItem(SOUND_KEY) !== "off";
}

function playClickSound() {
  if (!isSoundEnabled()) return;
  if (!clickSound) clickSound = new Audio("assets/play.wav");
  clickSound.currentTime = 0;
  // Sin catch, un navegador que bloquee el audio ensucia la consola con una
  // promesa rechazada y no hay nada que hacer al respecto.
  clickSound.play().catch(() => {});
}

const SPLASH_CARD_HOLD_MS = 2800;
const SPLASH_FADE_OUT_MS = 650;

// Cinemática de arranque: estudio -> marca del launcher. Un clic la saltea.
// Puramente decorativa: no bloquea la carga real, que sigue en paralelo.
function runIntroSplash() {
  const splash = document.querySelector("#splash");
  const cards = Array.from(document.querySelectorAll(".splash-card"));
  if (!splash || cards.length === 0) return;

  let index = 0;
  let timer = null;

  document.body.classList.add("no-scroll");

  function finish() {
    if (timer) clearTimeout(timer);
    splash.removeEventListener("click", finish);
    splash.classList.add("is-hidden");
    document.body.classList.remove("no-scroll");
    setTimeout(() => splash.remove(), SPLASH_FADE_OUT_MS);
  }

  function advance() {
    cards[index].classList.remove("is-visible");
    index += 1;
    if (index >= cards.length) {
      finish();
      return;
    }
    cards[index].classList.add("is-visible");
    timer = setTimeout(advance, SPLASH_CARD_HOLD_MS);
  }

  splash.addEventListener("click", finish);
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      cards[0].classList.add("is-visible");
      timer = setTimeout(advance, SPLASH_CARD_HOLD_MS);
    });
  });
}

async function play() {
  errorText.classList.add("hidden");

  if (!usernameEl.value.trim()) {
    showError("Poné un nombre de usuario.");
    return;
  }

  playClickSound();
  const settings = currentSettings();

  setBusy(true);
  statusText.textContent = "Arrancando...";
  progressFill.style.width = "0%";

  try {
    // La hora va desde acá porque el core no conoce la zona horaria del
    // sistema: la usa para elegir el viraje del panorama del menú.
    await invoke("play", { settings, localHour: new Date().getHours() });
    statusText.textContent = "¡Listo! Minecraft debería abrirse solo.";
  } catch (err) {
    showError(typeof err === "string" ? err : "Algo falló al lanzar el juego.");
  } finally {
    setBusy(false);
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  usernameEl = document.querySelector("#username");
  serverEl = document.querySelector("#server");
  noServerToggle = document.querySelector("#no-server-toggle");
  playButton = document.querySelector("#play-button");
  progressWrap = document.querySelector("#progress-wrap");
  progressFill = document.querySelector("#progress-fill");
  statusText = document.querySelector("#status-text");
  errorText = document.querySelector("#error-text");
  extraJvmEl = document.querySelector("#extra-jvm");
  serverStatusDot = document.querySelector("#server-status-dot");
  serverStatusTitle = document.querySelector("#server-status-title");
  serverStatusText = document.querySelector("#server-status-text");
  serverIconImg = document.querySelector("#server-icon");
  serverIconFallback = document.querySelector("#server-icon-fallback");
  skinAvatar = document.querySelector("#skin-avatar");
  newsSection = document.querySelector("#news");
  newsTrack = document.querySelector("#news-track");
  newsDots = document.querySelector("#news-dots");
  modsPanel = document.querySelector("#mods-panel");
  modsList = document.querySelector("#mods-list");
  ramAutoToggle = document.querySelector("#ram-auto-toggle");
  ramSlider = document.querySelector("#ram-slider");
  ramValueLabel = document.querySelector("#ram-value");
  skinPickButton = document.querySelector("#skin-pick-button");
  skinClearButton = document.querySelector("#skin-clear-button");
  skinViewerCanvas = document.querySelector("#skin-viewer-canvas");
  themeSwatches = Array.from(document.querySelectorAll(".theme-swatch"));
  fontSelect = document.querySelector("#font-select");
  wordmarkImg = document.querySelector(".hero-wordmark");
  tabButtons = Array.from(document.querySelectorAll(".tab-button"));
  tabPanels = Array.from(document.querySelectorAll(".tab-panel"));
  texturePacksList = document.querySelector("#texture-packs-list");
  texturePackAddButton = document.querySelector("#texture-pack-add-button");
  shaderPacksList = document.querySelector("#shader-packs-list");
  shaderPackAddButton = document.querySelector("#shader-pack-add-button");
  serverPlayers = document.querySelector("#server-players");
  copyIpButton = document.querySelector("#copy-ip-button");
  skinGallery = document.querySelector("#skin-gallery");
  capePickButton = document.querySelector("#cape-pick-button");
  capeClearButton = document.querySelector("#cape-clear-button");
  panoramaSelect = document.querySelector("#panorama-select");
  soundToggle = document.querySelector("#sound-toggle");
  backupToggle = document.querySelector("#backup-toggle");
  backupNowButton = document.querySelector("#backup-now-button");
  backupsList = document.querySelector("#backups-list");
  statTotal = document.querySelector("#stat-total");
  statSessions = document.querySelector("#stat-sessions");
  statLast = document.querySelector("#stat-last");
  statLastLabel = document.querySelector("#stat-last-label");
  crashNote = document.querySelector("#crash-note");
  viewLogButton = document.querySelector("#view-log-button");
  logModal = document.querySelector("#log-modal");
  logContent = document.querySelector("#log-content");
  logCloseButton = document.querySelector("#log-close-button");
  logCopyButton = document.querySelector("#log-copy-button");
  appVersionLabel = document.querySelector("#app-version");
  checkUpdateButton = document.querySelector("#check-update-button");
  updateStatus = document.querySelector("#update-status");
  updateDownloadButton = document.querySelector("#update-download-button");

  applyTheme(localStorage.getItem(THEME_KEY) ?? DEFAULT_THEME);
  applyFont(localStorage.getItem(FONT_KEY) ?? DEFAULT_FONT);
  initSkinViewer();

  for (const swatch of themeSwatches) {
    swatch.addEventListener("click", () => {
      applyTheme(swatch.dataset.theme);
      localStorage.setItem(THEME_KEY, swatch.dataset.theme);
    });
  }
  fontSelect.addEventListener("change", () => {
    applyFont(fontSelect.value);
    localStorage.setItem(FONT_KEY, fontSelect.value);
  });

  playButton.addEventListener("click", play);
  serverEl.addEventListener("change", scheduleServerPing);
  noServerToggle.addEventListener("change", () => setNoServerMode(noServerToggle.checked));
  usernameEl.addEventListener("input", () => {
    clearTimeout(skinDebounceTimer);
    skinDebounceTimer = setTimeout(updateSkinPreview, 400);
  });
  skinAvatar.addEventListener("error", () => {
    if (skinFallbackUsed) return;
    skinFallbackUsed = true;
    skinAvatar.src = skinAvatarUrl("Steve");
  });
  ramAutoToggle.addEventListener("change", () => {
    ramSlider.disabled = ramAutoToggle.checked;
    if (ramAutoToggle.checked) ramSlider.value = String(ramSuggestedMb);
    updateRamValueLabel();
  });
  ramSlider.addEventListener("input", updateRamValueLabel);
  skinPickButton.addEventListener("click", pickSkin);
  skinClearButton.addEventListener("click", clearSkin);
  capePickButton.addEventListener("click", pickCape);
  capeClearButton.addEventListener("click", clearCape);
  copyIpButton.addEventListener("click", copyServerIp);
  texturePackAddButton.addEventListener("click", addTexturePack);
  shaderPackAddButton.addEventListener("click", addShaderPack);

  soundToggle.checked = isSoundEnabled();
  soundToggle.addEventListener("change", () => {
    localStorage.setItem(SOUND_KEY, soundToggle.checked ? "on" : "off");
    if (soundToggle.checked) playClickSound();
  });
  panoramaSelect.addEventListener("change", persistSettings);
  backupToggle.addEventListener("change", persistSettings);
  backupNowButton.addEventListener("click", backupNow);

  viewLogButton.addEventListener("click", openLogModal);
  logCloseButton.addEventListener("click", closeLogModal);
  logModal.addEventListener("click", (event) => {
    if (event.target === logModal) closeLogModal();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !logModal.classList.contains("hidden")) closeLogModal();
  });
  logCopyButton.addEventListener("click", () => copyToClipboard(logContent.textContent));

  checkUpdateButton.addEventListener("click", checkUpdate);
  updateDownloadButton.addEventListener("click", () => {
    if (pendingUpdateUrl) window.__TAURI__.opener.openUrl(pendingUpdateUrl);
  });

  document.querySelectorAll("[data-open-folder]").forEach((button) => {
    button.addEventListener("click", () => openFolder(button.dataset.openFolder));
  });
  document.querySelectorAll(".support-item").forEach((button) => {
    button.addEventListener("click", () => {
      window.__TAURI__.opener.openUrl(button.dataset.url);
    });
  });
  for (const button of tabButtons) {
    button.addEventListener("click", () => {
      setActiveTab(button.dataset.tab);
      // El tiempo jugado lo escribe el backend al cerrarse el juego: se relee
      // al entrar a Ajustes en vez de quedarse con el número de hace rato.
      if (button.dataset.tab === "settings") loadStats();
    });
  }
  window.addEventListener("focus", loadStats);
  newsTrack.addEventListener("scroll", () => {
    clearTimeout(newsScrollDebounce);
    newsScrollDebounce = setTimeout(() => {
      const index = Math.round(newsTrack.scrollLeft / newsTrack.clientWidth);
      newsCurrentIndex = index;
      updateActiveNewsDot(index);
      scheduleNewsAutoAdvance();
    }, 120);
  });

  runIntroSplash();

  await listen("progress", (event) => {
    const { stage, message, fraction } = event.payload;
    statusText.textContent = message ?? STAGE_LABELS[stage] ?? stage;
    if (typeof fraction === "number") {
      progressFill.style.width = `${Math.round(fraction * 100)}%`;
    }
  });

  // loadSettings ya llama updateSkinPreview con el username real, así que no
  // hace falta pedir un avatar "Steve" de más acá. El resto son independientes
  // entre sí, así que corren todas en paralelo en vez de una atrás de otra.
  const settingsPromise = loadSettings().catch((err) => {
    console.error("no se pudieron cargar los settings guardados", err);
    setNoServerMode(!serverEl.value);
    updateSkinPreview();
  });

  loadNews();
  loadMods();
  loadSkinTab();
  loadTexturePacks();
  loadShaderPacks();
  loadStats();
  loadBackups();
  loadAppVersion();
  await settingsPromise;
});
