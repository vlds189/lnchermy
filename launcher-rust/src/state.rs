// state.rs - Central application state shared between UI and background work.
use crate::settings::Settings;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// Launcher version, bumped per release. Matches version.json in the repo.
pub const APP_VERSION: &str = "3.0.5";

/// What the launcher is currently doing. Drives the UI (idle vs progress vs error).
#[derive(Debug, Clone, Default)]
pub enum Task {
    #[default]
    Idle,
    Running {
        title: String,
        /// (label, done, total) — total==0 means indeterminate.
        steps: Vec<(String, bool)>,
        progress_current: usize,
        progress_total: usize,
    },
    Done(String),
    Error(String),
}

/// Status of the Launch button — separate from Task because the game runs
/// asynchronously and we need to track it independently.
#[derive(Debug, Clone)]
pub enum LaunchStatus {
    Idle,
    /// Building command + extracting natives (brief).
    Launching,
    /// Game process is running.
    Running(String),
    /// Launch failed. Persists until the user changes version or retries.
    Error(String),
}

impl Default for LaunchStatus {
    fn default() -> Self {
        LaunchStatus::Idle
    }
}

impl Task {
    pub fn is_busy(&self) -> bool {
        matches!(self, Task::Running { .. })
    }
}

/// Cross-thread handle to the current task, so a background worker thread
/// (download/launch) can update progress while the UI keeps repainting.
pub type SharedTask = Arc<Mutex<Task>>;

pub fn new_shared_task() -> SharedTask {
    Arc::new(Mutex::new(Task::Idle))
}

/// The root game directory. Defaults to the launcher .exe's folder, mirroring
/// the PowerShell `$PSScriptRoot` behavior. Overridable later via settings.
pub fn work_dir() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            return parent.to_path_buf();
        }
    }
    // Fallback: current working directory.
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// Top-level app state owned by the eframe::App.
pub struct AppState {
    pub settings: Settings,
    pub settings_path: PathBuf,
    pub work_dir: PathBuf,
    pub task: SharedTask,
    /// Currently selected installed version (string id, e.g. "1.20.1-forge-47.4.5").
    pub selected_version: Option<String>,
    /// Cached list of installed version ids (rescanned on demand).
    pub installed_versions: Vec<String>,
    /// Last update check result: Some(latest_version_string) if newer than APP_VERSION.
    pub update_available: Option<String>,
    /// Inline result shown next to the "Check for updates" button.
    /// (is_success, message) — green text if successful, red otherwise.
    pub update_msg: Option<(bool, String)>,
    /// Whether the settings screen should be shown.
    pub show_settings: bool,
    /// Internal: whether the theme has been applied to the egui context yet.
    pub theme_applied: bool,
    /// Whether the "install vanilla" version-picker window is open.
    pub show_install_vanilla: bool,
    /// Whether the "install Forge" window is open.
    pub show_install_forge: bool,
    /// Whether the "install Java" window is open.
    pub show_install_java: bool,
    /// Whether the "install OptiFine" window is open.
    pub show_install_optifine: bool,
    /// Whether the "download content" window is open.
    pub show_content: bool,
    /// Cached list of remote vanilla versions (id) from the manifest.
    pub remote_versions: Vec<String>,
    /// Search/filter text for the vanilla version picker.
    pub vanilla_filter: String,
    /// Custom Forge build override text input.
    pub forge_custom: String,
    /// Custom Java major version text input.
    pub java_custom: String,
    /// Last content-index fetch error message (for the content window).
    pub content_index_error: String,
    /// Version id pending deletion confirmation (None = no dialog open).
    pub pending_delete: Option<String>,
    /// Whether the "close game" confirmation dialog is open.
    pub pending_close_game: bool,
    /// Launch button status (separate from Task — tracks the game process).
    pub launch_status: Arc<Mutex<LaunchStatus>>,
    /// Handle to the running game process (None = no game running).
    /// Polled via try_wait() each frame to detect when the game exits.
    pub game_child: Arc<Mutex<Option<std::process::Child>>>,
    /// Tracks whether the launch button was hovered last frame (for showing
    /// "Launch" text on hover when in Error state).
    pub launch_btn_hovered: bool,
    /// Version picked from the 🔄 picker popup that is NOT installed yet.
    /// While set, the Launch button becomes "Install <version>" instead of
    /// "LAUNCH". Cleared automatically once the version installs.
    pub pending_install: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        let work_dir = work_dir();
        let settings_path = work_dir.join("mc_console_settings.json");
        let settings = Settings::load(&settings_path);
        let state = AppState {
            settings,
            settings_path,
            work_dir,
            task: new_shared_task(),
            selected_version: None,
            installed_versions: Vec::new(),
            update_available: None,
            update_msg: None,
            show_settings: false,
            theme_applied: false,
            show_install_vanilla: false,
            show_install_forge: false,
            show_install_java: false,
            show_install_optifine: false,
            show_content: false,
            remote_versions: Vec::new(),
            vanilla_filter: String::new(),
            forge_custom: String::new(),
            java_custom: String::new(),
            content_index_error: String::new(),
            pending_delete: None,
            pending_close_game: false,
            launch_status: Arc::new(Mutex::new(LaunchStatus::Idle)),
            game_child: Arc::new(Mutex::new(None)),
            launch_btn_hovered: false,
            pending_install: None,
        };
        state
    }

    pub fn save_settings(&self) -> Result<(), String> {
        self.settings.save(&self.settings_path)
    }

    /// Scan the versions/ folder for installed version ids. A version is any
    /// folder containing <name>.json OR <name>.jar (jar-only covers some Forge
    /// installs; mirrors the PowerShell detection logic).
    pub fn rescan_versions(&mut self) {
        self.installed_versions.clear();
        let versions_dir = self.work_dir.join("versions");
        let Ok(entries) = std::fs::read_dir(&versions_dir) else {
            return;
        };
        let mut found: Vec<String> = Vec::new();
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let dir = entry.path();
            let has_json = dir.join(format!("{name}.json")).exists();
            let has_jar = dir.join(format!("{name}.jar")).exists();
            if has_json || has_jar {
                found.push(name);
            }
        }
        found.sort();
        self.installed_versions = found;
        // Keep selection valid if possible.
        if let Some(sel) = &self.selected_version {
            if !self.installed_versions.iter().any(|v| v == sel) {
                self.selected_version = None;
            }
        }
        if self.selected_version.is_none() && !self.installed_versions.is_empty() {
            self.selected_version = Some(self.installed_versions[0].clone());
        }
    }

    /// Tag for display: [Forge] / [OptiFine] / none.
    pub fn version_tag(name: &str) -> &'static str {
        let lower = name.to_ascii_lowercase();
        if lower.contains("forge") {
            "[Forge]"
        } else if lower.contains("optifine") {
            "[OptiFine]"
        } else {
            ""
        }
    }

    /// Helper to set a task from the UI thread (no background work).
    pub fn set_task(&self, task: Task) {
        if let Ok(mut t) = self.task.lock() {
            *t = task;
        }
    }

    /// Helper to read a snapshot of the current task for rendering.
    pub fn task_snapshot(&self) -> Task {
        match self.task.lock() {
            Ok(t) => t.clone(),
            Err(_) => Task::Idle,
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
