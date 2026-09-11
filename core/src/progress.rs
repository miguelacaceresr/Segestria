use serde::Serialize;

/// Etapa del pipeline de arranque, para que la UI muestre algo con sentido.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    BackingUpWorlds,
    CheckingJava,
    DownloadingClient,
    DownloadingLibraries,
    DownloadingAssets,
    InstallingForge,
    SyncingMods,
    Launching,
}

#[derive(Debug, Clone, Serialize)]
pub struct Progress {
    pub stage: Stage,
    pub message: String,
    /// 0.0 - 1.0, o None si la etapa no tiene un porcentaje significativo.
    pub fraction: Option<f32>,
}

impl Progress {
    pub fn new(stage: Stage, message: impl Into<String>, fraction: Option<f32>) -> Self {
        Self {
            stage,
            message: message.into(),
            fraction,
        }
    }
}

/// Callback genérico para reportar avance hacia quien llama (comandos de Tauri, tests, etc).
pub type ProgressSink = std::sync::Arc<dyn Fn(Progress) + Send + Sync>;

pub fn noop_sink() -> ProgressSink {
    std::sync::Arc::new(|_| {})
}
