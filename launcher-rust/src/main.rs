// main.rs - Entry point. Boots the eframe window and owns AppState.
// Graphical app: hide the console window on Windows (the default build
// targets the console subsystem and spawns a black box next to the UI).
// Note: stderr/stdout "disappear" — use log files if needed later.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
// Dead-code is allowed project-wide: many structs/fields/helpers are wired up
// progressively across stages and are intentionally retained for later use.
#![allow(dead_code)]

mod content;
mod http;
mod install;
mod java;
mod launch;
mod maven;
mod natives;
mod rules;
mod settings;
mod state;
mod theme;
mod ui;
mod update;
mod versions;

use eframe::App;

/// Install a panic hook that dumps panic details to `crash.log` next to the
/// executable. Release builds use `panic = "abort"` and hide the console
/// (windows_subsystem), so a panic in any thread silently kills the whole
/// launcher with no trace left behind. The hook still runs just before abort,
/// turning the next crash into a diagnosable file: thread name, message,
/// source location and backtrace.
fn install_panic_hook() {
    std::panic::set_hook(Box::new(|info| {
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>").to_string();
        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            (*s).to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "<non-string panic payload>".to_string()
        };
        let location = info
            .location()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "<unknown>".to_string());
        let text = format!(
            "Panic on thread [{thread_name}] at {location}:\n{payload}\n\nBacktrace:\n{}\n",
            std::backtrace::Backtrace::force_capture()
        );
        // The launcher is self-contained — the log lands next to the exe,
        // i.e. in the user's game folder.
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        {
            if let Err(e) = std::fs::write(exe_dir.join("crash.log"), &text) {
                eprintln!("failed to write crash.log: {e}");
            }
        }
        eprintln!("{text}");
    }));
}

fn main() -> eframe::Result {
    install_panic_hook();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([560.0, 600.0])
            .with_min_inner_size([420.0, 460.0])
            .with_title("Minecraft Launcher"),
        ..Default::default()
    };

    eframe::run_native(
        "Minecraft Launcher",
        options,
        Box::new(|_cc| Ok(Box::new(LauncherApp::new()))),
    )
}

struct LauncherApp {
    state: state::AppState,
    /// Tracks whether a task was busy on the previous frame, so we can detect
    /// when a download/install finishes and rescan the version list.
    was_busy: bool,
}

impl LauncherApp {
    fn new() -> Self {
        let mut state = state::AppState::new();
        state.rescan_versions();
        check_update_async(&state);
        LauncherApp {
            state,
            was_busy: false,
        }
    }
}

/// Silently check for a newer version in the background. On success, sets
/// `state.update_available` so the UI can show a non-intrusive hint.
fn check_update_async(state: &state::AppState) {
    let task = state.task.clone();
    let cur = state::APP_VERSION.to_string();
    std::thread::spawn(move || {
        match update::check_latest() {
            Ok(latest) => {
                if update::is_newer(&latest, &cur) {
                    // Don't override an active error/busy task; just stash the
                    // info so Settings can show it. We use a "Done"-like info
                    // message that the status bar will display in green.
                    if let Ok(mut t) = task.lock() {
                        if !t.is_busy() {
                            *t = state::Task::Done(format!(
                                "Update available: v{cur} → v{latest}. See Settings → Updates."
                            ));
                        }
                    }
                }
            }
            Err(_) => {
                // Network failure on the silent check is intentionally ignored.
            }
        }
    });
}

impl App for LauncherApp {
    /// egui 0.36 uses a `ui` callback (instead of the older `update`).
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Self-update: a fresh binary has been swapped in and spawned; close
        // this instance so the new process takes over.
        if update::RESTART_PENDING.swap(false, std::sync::atomic::Ordering::SeqCst) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            return;
        }

        // Apply the theme once on first frame.
        if !self.state.theme_applied {
            theme::apply(&ctx, self.state.settings.theme);
            self.state.theme_applied = true;
        }

        // Poll the running game process: detect when the game exits.
        {
            let mut guard = self.state.game_child.lock().unwrap();
            if let Some(child) = guard.as_mut() {
                match child.try_wait() {
                    Ok(Some(_exit_status)) => {
                        // Game exited — clear handle and reset launch status.
                        *guard = None;
                        *self.state.launch_status.lock().unwrap() =
                            state::LaunchStatus::Idle;
                    }
                    Ok(None) => {
                        // Still running — request repaint so we keep polling.
                        ctx.request_repaint();
                    }
                    Err(_) => {
                        *guard = None;
                        *self.state.launch_status.lock().unwrap() =
                            state::LaunchStatus::Idle;
                    }
                }
            }
        }

        // Detect when a background task finishes (busy → idle) and rescan the
        // installed versions list so newly installed versions appear.
        let is_busy = self.state.task_snapshot().is_busy();
        if self.was_busy && !is_busy {
            self.state.rescan_versions();
            // A version picked in the 🔄 picker just finished installing:
            // auto-select it and take the launch button out of install mode.
            if let Some(v) = self.state.pending_install.clone() {
                if self.state.installed_versions.iter().any(|x| x == &v) {
                    self.state.selected_version = Some(v);
                    self.state.pending_install = None;
                    *self.state.launch_status.lock().unwrap() =
                        state::LaunchStatus::Idle;
                }
            }
        }
        self.was_busy = is_busy;

        // Keep repainting while a background task runs.
        if is_busy {
            ctx.request_repaint();
        }

        ui::render(ui, &mut self.state);
    }
}
