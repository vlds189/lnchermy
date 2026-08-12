// main.rs - Entry point. Boots the eframe window and owns AppState.
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

fn main() -> eframe::Result {
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
        }
        self.was_busy = is_busy;

        // Keep repainting while a background task runs.
        if is_busy {
            ctx.request_repaint();
        }

        ui::render(ui, &mut self.state);
    }
}
