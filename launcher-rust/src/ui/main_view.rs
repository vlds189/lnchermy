// ui/main_view.rs - Main launcher screen: version list, RAM/username, launch button.
use crate::state::{AppState, Task, APP_VERSION};
use crate::theme::{ACCENT, ERROR, WARN};
use egui::{Align, Color32, Layout, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    // Top bar: title + version + theme toggle + settings gear.
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Minecraft Launcher").color(ACCENT).strong());
            ui.label(
                RichText::new(format!("v{}", APP_VERSION))
                    .small()
                    .color(Color32::GRAY),
            );

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("⚙ Settings").clicked() {
                    state.show_settings = true;
                }
                let (icon, hover) = match state.settings.theme {
                    crate::settings::Theme::Dark => ("☀", "Switch to light theme"),
                    crate::settings::Theme::Light => ("🌙", "Switch to dark theme"),
                };
                if ui.button(icon).on_hover_text(hover).clicked() {
                    state.settings.theme = state.settings.theme.toggle();
                    crate::theme::apply(ui.ctx(), state.settings.theme);
                    let _ = state.save_settings();
                }
            });
        });
        ui.add_space(4.0);
        ui.separator();
    });

    // Status bar at the bottom mirrors the PowerShell "Press Enter" lines.
    egui::Panel::bottom("status_bar").show(ui, |ui| {
        ui.add_space(2.0);
        let snapshot = state.task_snapshot();
        let (msg, color) = match &snapshot {
            Task::Idle => (
                match &state.selected_version {
                    Some(v) => format!("Ready — selected {v}"),
                    None => "No version selected".to_string(),
                },
                Color32::GRAY,
            ),
            Task::Running { title, .. } => (format!("Working: {title}…"), WARN),
            Task::Done(m) => (m.clone(), ACCENT),
            Task::Error(e) => (e.clone(), ERROR),
        };
        ui.label(RichText::new(msg).color(color).small());
        ui.add_space(2.0);
    });

    // Main content.
    egui::CentralPanel::default().show(ui, |ui| {
        version_list_section(ui, state);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        launch_options_section(ui, state);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        install_section(ui, state);
    });

    // Progress bar overlay while a background task runs.
    progress_overlay(ui.ctx(), state);

    // Install-vanilla version picker window.
    if state.show_install_vanilla {
        install_vanilla_window(ui.ctx(), state);
    }

    // Forge / OptiFine / Java / Content windows.
    super::install_view::render_windows(ui.ctx(), state);

    // Delete confirmation dialog.
    if state.pending_delete.is_some() {
        delete_confirm_window(ui.ctx(), state);
    }

    // Close game confirmation dialog.
    if state.pending_close_game {
        close_game_confirm_window(ui.ctx(), state);
    }
}

/// Inline progress bar shown at the bottom of the central panel when a task is
/// running. Mirrors the PowerShell step labels + counts.
fn progress_overlay(ctx: &egui::Context, state: &AppState) {
    let snapshot = state.task_snapshot();
    if let Task::Running { title, progress_current, progress_total, .. } = &snapshot {
        let cur = *progress_current;
        let tot = *progress_total;
        egui::Area::new(egui::Id::new("progress_area"))
            .order(egui::Order::Foreground)
            .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(8.0, -8.0))
            .show(ctx, |ui| {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(title).strong());
                    });
                    if tot > 0 {
                        let frac = cur as f32 / tot as f32;
                        ui.add(egui::ProgressBar::new(frac).show_percentage());
                        ui.label(format!("{cur} / {tot}"));
                    } else {
                        ui.add(egui::ProgressBar::new(0.0).animate(true));
                    }
                });
            });
    }
}

/// The version-picker window for installing vanilla Minecraft.
fn install_vanilla_window(ctx: &egui::Context, state: &mut AppState) {
    // Drain the manifest from the shared global if the background fetch finished.
    if let Some(ids) = MANIFEST.lock().unwrap().take() {
        state.remote_versions = ids;
    }

    let mut open = state.show_install_vanilla;
    egui::Window::new("Install Minecraft (vanilla)")
        .open(&mut open)
        .resizable(true)
        .default_width(380.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            if state.remote_versions.is_empty() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching version list from Mojang…");
                });
                ui.ctx().request_repaint();
                return;
            }
            ui.label("Filter:");
            ui.add(
                egui::TextEdit::singleline(&mut state.vanilla_filter)
                    .hint_text("e.g. 1.20.1"),
            );
            ui.add_space(4.0);

            let busy = state.task_snapshot().is_busy();
            let filter = state.vanilla_filter.trim().to_ascii_lowercase();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let mut to_install: Option<String> = None;
                for id in &state.remote_versions {
                    if !filter.is_empty() && !id.to_ascii_lowercase().contains(&filter) {
                        continue;
                    }
                    // Highlight if already installed.
                    let installed = state.installed_versions.iter().any(|v| v == id);
                    let label = if installed {
                        format!("{id}  (installed)")
                    } else {
                        id.clone()
                    };
                    if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                        to_install = Some(id.clone());
                    }
                }
                if let Some(ver) = to_install {
                    state.show_install_vanilla = false;
                    start_vanilla_download(state, ver);
                }
            });
        });
    state.show_install_vanilla = open;
}

/// Start a vanilla download in a background thread, updating the shared task
/// with progress callbacks.
fn start_vanilla_download(state: &mut AppState, version: String) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    state.set_task(Task::Running {
        title: format!("Installing {version}"),
        steps: Vec::new(),
        progress_current: 0,
        progress_total: 0,
    });

    let progress: crate::install::vanilla::Progress =
        std::sync::Arc::new({
            let task = task.clone();
            move |label: &str, current: usize, total: usize| {
                if let Ok(mut t) = task.lock() {
                    *t = Task::Running {
                        title: label.to_string(),
                        steps: Vec::new(),
                        progress_current: current,
                        progress_total: total,
                    };
                }
            }
        });

    std::thread::spawn(move || {
        let result = crate::install::vanilla::download_version(&version, &work_dir, &progress);
        let msg = match result {
            Ok(()) => Task::Done(format!("Installed {version}")),
            Err(e) => Task::Error(format!("Install failed: {e}")),
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
        // Clear the manifest cache so a re-open refreshes.
        *MANIFEST.lock().unwrap() = None;
    });
}

fn version_list_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Installed versions").strong());
    ui.add_space(2.0);

    if state.installed_versions.is_empty() {
        ui.label(
            RichText::new("No versions found. Install one below (Vanilla / Forge).")
                .color(Color32::GRAY)
                .italics(),
        );
        return;
    }

    egui::ScrollArea::vertical()
        .max_height(160.0)
        .show(ui, |ui| {
            for v in state.installed_versions.clone() {
                let tag = AppState::version_tag(&v);
                let selected = state.selected_version.as_deref() == Some(v.as_str());
                let marker = if selected { "▶ " } else { "  " };
                let label_text = if tag.is_empty() {
                    format!("{marker}{v}")
                } else {
                    format!("{marker}{v}  {tag}")
                };
                ui.horizontal(|ui| {
                    if ui.selectable_label(selected, &label_text).clicked() {
                        state.selected_version = Some(v.clone());
                        // Reset launch status when switching versions.
                        *state.launch_status.lock().unwrap() =
                            crate::state::LaunchStatus::Idle;
                    }
                    // Delete button
                    if ui
                        .button("🗑")
                        .on_hover_text(format!("Delete {v}"))
                        .clicked()
                    {
                        state.pending_delete = Some(v.clone());
                    }
                });
            }
        });
}

fn launch_options_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Launch options").strong());
    ui.add_space(2.0);

    ui.horizontal(|ui| {
        ui.label("Memory:");
        ui.add_space(4.0);
        ui.label(format!(
            "MIN {} / MAX {}",
            state.settings.ram_min, state.settings.ram_max
        ));
        ui.add_space(8.0);
        ui.label("•");
        ui.add_space(8.0);
        ui.label("Player:");
        ui.add_space(4.0);
        ui.label(RichText::new(&state.settings.username).strong());
        ui.add_space(8.0);
        ui.label("•");
        ui.add_space(8.0);
        ui.label(format!("Theme: {}", state.settings.theme.label()));
    });

    ui.add_space(6.0);

    let launch_status = state.launch_status.lock().unwrap().clone();
    let no_version = state.selected_version.is_none();
    let task_busy = state.task_snapshot().is_busy();

    let (btn_text, btn_bg, enabled) = match &launch_status {
        crate::state::LaunchStatus::Launching => (
            RichText::new("⟳  Launching…").strong(),
            Some(Color32::from_rgb(0xD4, 0xA0, 0x17)),
            false,
        ),
        crate::state::LaunchStatus::Running(ver) => {
            if state.launch_btn_hovered {
                // On hover: offer to close the game.
                (
                    RichText::new("✕  Close Game").strong(),
                    Some(ERROR),
                    true,
                )
            } else {
                (
                    RichText::new(format!("▶  Running: {ver}")).strong(),
                    Some(ACCENT),
                    false,
                )
            }
        }
        crate::state::LaunchStatus::Error(_) if !state.launch_btn_hovered => (
            RichText::new("⚠  Error").strong(),
            Some(ERROR),
            true,
        ),
        _ => (
            RichText::new("▶  LAUNCH").strong(),
            None,
            !no_version && !task_busy,
        ),
    };

    let mut btn = egui::Button::new(btn_text).min_size(egui::vec2(200.0, 34.0));
    if let Some(bg) = btn_bg {
        btn = btn.fill(bg);
    }
    let resp = ui.add_enabled(enabled, btn);
    state.launch_btn_hovered = resp.hovered();

    if resp.clicked() {
        match &launch_status {
            crate::state::LaunchStatus::Running(_) => {
                state.pending_close_game = true;
            }
            _ if !no_version => {
                if let Some(v) = state.selected_version.clone() {
                    launch_version(state, v);
                }
            }
            _ => {}
        }
    }

    if no_version {
        ui.label(
            RichText::new("Select a version above to launch.")
                .small()
                .color(Color32::GRAY),
        );
    } else if let crate::state::LaunchStatus::Error(msg) = &launch_status {
        if !state.launch_btn_hovered {
            ui.label(RichText::new(msg).small().color(ERROR));
        }
    }
}

fn launch_section(_ui: &mut Ui, _state: &mut AppState) {
    // Reserved for future install UI; currently unused.
}

/// Spawn the Minecraft process for the selected version in a background thread.
/// Updates launch_status directly (not Task) so the button reflects Running/Error.
fn launch_version(state: &mut AppState, version_id: String) {
    let settings = state.settings.clone();
    let work_dir = state.work_dir.clone();
    let launch_status = state.launch_status.clone();
    let game_child = state.game_child.clone();

    // Show "Launching…" immediately.
    *launch_status.lock().unwrap() = crate::state::LaunchStatus::Launching;

    std::thread::spawn(move || {
        let result = crate::launch::launch(&version_id, &work_dir, &settings);
        match result {
            crate::launch::LaunchResult::Ok(child) => {
                // Store the child handle so the UI loop can detect when it exits.
                *game_child.lock().unwrap() = Some(child);
                *launch_status.lock().unwrap() =
                    crate::state::LaunchStatus::Running(version_id);
            }
            crate::launch::LaunchResult::Failed(e) => {
                let msg = format!("Launch failed: {e}");
                *launch_status.lock().unwrap() =
                    crate::state::LaunchStatus::Error(msg);
            }
        }
    });
}

/// Confirmation dialog for closing the running game.
fn close_game_confirm_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = true;
    egui::Window::new("Close game")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label("Вы точно хотите закрыть игру?");
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Да, закрыть").clicked() {
                    let mut guard = state.game_child.lock().unwrap();
                    if let Some(child) = guard.as_mut() {
                        let _ = child.kill();
                        let _ = child.wait(); // reap the process
                    }
                    *guard = None;
                    drop(guard);
                    *state.launch_status.lock().unwrap() =
                        crate::state::LaunchStatus::Idle;
                    state.pending_close_game = false;
                }
                if ui.button("Отмена").clicked() {
                    state.pending_close_game = false;
                }
            });
        });
    if !open {
        state.pending_close_game = false;
    }
}

/// Confirmation dialog for deleting a version. Removes the entire
/// `versions/<id>/` folder (including natives-extracted, .json, .jar).
fn delete_confirm_window(ctx: &egui::Context, state: &mut AppState) {
    let ver = state.pending_delete.clone().unwrap_or_default();
    let mut open = state.pending_delete.is_some();
    egui::Window::new("Delete version")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(format!("Вы точно хотите удалить версию {ver}?"));
            ui.label(
                RichText::new("Папка versions/ver/ будет удалена полностью (.jar, .json, natives).")
                    .small()
                    .color(egui::Color32::GRAY),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button("Да, удалить").clicked() {
                    let dir = state.work_dir.join("versions").join(&ver);
                    if let Err(e) = std::fs::remove_dir_all(&dir) {
                        state.set_task(Task::Error(format!("Failed to delete {ver}: {e}")));
                    } else {
                        state.set_task(Task::Done(format!("Deleted {ver}")));
                    }
                    state.pending_delete = None;
                    state.rescan_versions();
                }
                if ui.button("Отмена").clicked() {
                    state.pending_delete = None;
                }
            });
        });
    if !open {
        state.pending_delete = None;
    }
}

fn install_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Install").strong());
    ui.add_space(2.0);
    let busy = state.task_snapshot().is_busy();
    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!busy, egui::Button::new("Vanilla")).clicked() {
            state.show_install_vanilla = true;
            if state.remote_versions.is_empty() {
                fetch_manifest_async(state);
            }
        }
        if ui.add_enabled(!busy, egui::Button::new("Forge")).clicked() {
            state.show_install_forge = true;
            // Fetch metadata in background if not already cached.
            let need_fetch = !super::install_view::forge_data_cached();
            if need_fetch {
                super::install_view::fetch_forge_async();
            }
        }
        if ui.add_enabled(!busy, egui::Button::new("OptiFine")).clicked() {
            state.show_install_optifine = true;
            let need_fetch = !super::install_view::optifine_data_cached();
            if need_fetch {
                super::install_view::fetch_optifine_async();
            }
        }
        if ui.add_enabled(!busy, egui::Button::new("Java")).clicked() {
            state.show_install_java = true;
        }
        if ui.add_enabled(!busy, egui::Button::new("Mods / Resourcepacks")).clicked() {
            state.show_content = true;
        }
    });
}

/// Fetch the Mojang manifest in a background thread, storing the version list
/// into a shared global that the UI picks up.
fn fetch_manifest_async(state: &AppState) {
    let task = state.task.clone();
    std::thread::spawn(move || {
        match crate::install::vanilla::fetch_manifest() {
            Ok(list) => {
                let ids: Vec<String> = list.into_iter().map(|(id, _)| id).collect();
                *MANIFEST.lock().unwrap() = Some(ids);
                if let Ok(mut t) = task.lock() {
                    if !t.is_busy() {
                        *t = Task::Idle;
                    }
                }
            }
            Err(e) => {
                if let Ok(mut t) = task.lock() {
                    if !t.is_busy() {
                        *t = Task::Error(format!("Manifest fetch failed: {e}"));
                    }
                }
            }
        }
    });
}

/// Shared slot for the manifest fetched by a background thread and consumed by
/// the UI thread. Uses a global Mutex (NOT thread_local, which is per-thread).
static MANIFEST: std::sync::LazyLock<std::sync::Mutex<Option<Vec<String>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));
