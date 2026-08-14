// ui/main_view.rs - Main launcher screen: version list, RAM/username, launch button.
use crate::state::{AppState, Task, APP_VERSION};
use crate::theme::{ACCENT, ERROR, INFO, WARN};
use egui::{Color32, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    // Top bar: title + version.
    egui::Panel::top("top_bar").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.heading(RichText::new("Minecraft Launcher").color(ACCENT).strong());
            ui.label(
                RichText::new(format!("v{}", APP_VERSION))
                    .small()
                    .color(Color32::GRAY),
            );
        });
        ui.add_space(4.0);
    });

    // Side bar: icon-only when not hovered, expands to show labels on hover.
    // Width is animated; the previous frame's rect is remembered via temp data
    // so hover can be tested before the panel is laid out this frame.
    let side_id = egui::Id::new("side_panel_anim");
    let prev_rect = ui
        .ctx()
        .data(|d| d.get_temp::<egui::Rect>(side_id))
        .unwrap_or_else(|| egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(110.0, 200.0)));
    let hovered = ui
        .ctx()
        .pointer_hover_pos()
        .is_some_and(|p| prev_rect.expand(12.0).contains(p));
    // Collapsed width fits the icon button (≈34px) plus an 8px margin on
    // either side, so the buttons don't press against the panel edge.
    let target_w = if hovered { 110.0 } else { 52.0 };
    let side_w = ui.ctx().animate_value_with_time(side_id, target_w, 0.18);
    if (side_w - target_w).abs() > 0.5 {
        ui.ctx().request_repaint();
    }
    // Buttons keep constant size while the panel animates: icon-only until the
    // panel is nearly full, then the label pops in. Same left edge → no shake.
    let show_text = side_w > 108.0;

    let side_inner = egui::Panel::left("side_panel")
        .exact_size(side_w)
        .resizable(false)
        .show(ui, |ui| {
        ui.add_space(6.0);
        let (icon, hover) = match state.settings.theme {
            crate::settings::Theme::Dark => ("☀", "Switch to light theme"),
            crate::settings::Theme::Light => ("🌙", "Switch to dark theme"),
        };
        if show_text {
            if ui
                .add(egui::Button::new(format!("{icon} Theme")).min_size(egui::vec2(94.0, 0.0)))
                .on_hover_text(hover)
                .clicked()
            {
                state.settings.theme = state.settings.theme.toggle();
                crate::theme::apply(ui.ctx(), state.settings.theme);
                let _ = state.save_settings();
            }
            ui.add_space(4.0);
            if ui
                .add(egui::Button::new("⚙ Settings").min_size(egui::vec2(94.0, 0.0)))
                .clicked()
            {
                state.show_settings = true;
            }
        } else {
            if ui.add(egui::Button::new(icon)).on_hover_text(hover).clicked() {
                state.settings.theme = state.settings.theme.toggle();
                crate::theme::apply(ui.ctx(), state.settings.theme);
                let _ = state.save_settings();
            }
            ui.add_space(4.0);
            if ui.add(egui::Button::new("⚙")).clicked() {
                state.show_settings = true;
            }
        }
    });
    ui.ctx().data_mut(|d| d.insert_temp(side_id, side_inner.response.rect));

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
        launch_options_section(ui, state);

        ui.add_space(10.0);
        ui.separator();
        ui.add_space(6.0);

        install_section(ui, state);
    });

    // Progress bar overlay while a background task runs.
    progress_overlay(ui.ctx(), state);

    // Install-vanilla picker is gone: the combined install selector in the
    // main view lists everything, and picking an item starts the install.

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
    let launch_version_id = state.selected_version.clone().unwrap_or_default();

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
                    RichText::new("✖  Close Game").strong(),
                    Some(ERROR),
                    true,
                )
            } else {
                // Always enabled so hover is detected (disabled buttons
                // don't register hover in egui). Click opens a confirm dialog.
                (
                    RichText::new(format!("▶  Running: {ver}")).strong(),
                    Some(ACCENT),
                    true,
                )
            }
        }
        _ if state.pending_install.is_some() => {
            // A version picked in the 🔄 picker that is still missing on
            // disk: the Launch button turns into the installer for it. Takes
            // precedence over any stale launch Error — install is the active
            // intent now.
            let v = state.pending_install.as_deref().unwrap_or_default();
            (
                RichText::new(format!("⬇  Install {v}")).strong(),
                Some(INFO),
                !task_busy,
            )
        }
        crate::state::LaunchStatus::Error(_) if !state.launch_btn_hovered => (
            RichText::new("⚠  Error").strong(),
            Some(ERROR),
            true,
        ),
        _ => (
            RichText::new(format!("▶  LAUNCH {launch_version_id}")).strong(),
            None,
            !no_version && !task_busy,
        ),
    };

    let mut btn = egui::Button::new(btn_text).min_size(egui::vec2(200.0, 34.0));
    if let Some(bg) = btn_bg {
        btn = btn.fill(bg);
    }

    // Reload / picker button next to Launch. Uses the 🔄 emoji glyph (present
    // in egui's bundled NotoEmoji font; verified via cmap). Clicking it
    // toggles a picker popup anchored below the launch row.
    let mut reload_clicked = false;
    let (resp, row_response) = {
        let row = ui.horizontal(|ui| {
            let resp = ui.add_enabled(enabled, btn);
            state.launch_btn_hovered = resp.hovered();

            ui.add_space(6.0);
            let rresp = ui.add(
                egui::Button::new("🔄")
                    .min_size(egui::vec2(34.0, 34.0))
                    .fill(ACCENT.linear_multiply(0.15)),
            );
            reload_clicked = rresp.clicked();
            resp
        });
        (row.inner, row.response)
    };

    if resp.clicked() {
        match &launch_status {
            crate::state::LaunchStatus::Running(_) => {
                state.pending_close_game = true;
            }
            _ if state.pending_install.is_some() => {
                // The launch button is in "install" mode: clicking it starts
                // the download of the version picked in the 🔄 picker.
                if let Some(v) = state.pending_install.clone() {
                    start_vanilla_download(state, v);
                }
            }
            _ if !no_version => {
                if let Some(v) = state.selected_version.clone() {
                    launch_version(state, v);
                }
            }
            _ => {}
        }
    }

    // Version picker popup, toggled by the 🔄 button. Anchored to the whole
    // launch row so it opens right under the launch button. The open state
    // lives in egui's memory (so it survives repaints); a click on the 🔄
    // button toggles it, a click anywhere else closes it.
    let picker_id = egui::Id::new("launch_picker");
    let toggle = reload_clicked.then_some(egui::SetOpenCommand::Toggle);
    egui::Popup::new(picker_id, ui.ctx().clone(), &row_response, ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .gap(0.0)
        .open_memory(toggle)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(320.0)
        .show(|ui| {
            picker_popup_content(ui, state);
        });

    if let Some(v) = &state.pending_install {
        ui.label(
            RichText::new(format!("{v} is not installed yet — clicking above installs it."))
                .small()
                .color(INFO),
        );
    } else if no_version {
        ui.label(
            RichText::new("Pick an installed version with 🔄 to launch.")
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

/// Content of the 🔄 version picker popup: installed versions first, then
/// catalog versions that are not installed yet. Picking an installed version
/// selects it; picking a missing one arms the "Install <version>" mode on
/// the launch button (stored in `state.pending_install`).
fn picker_popup_content(ui: &mut Ui, state: &mut AppState) {
    enum Row {
        Header(&'static str),
        Hint(&'static str),
        Installed(String, String), // label, id
        Remote(String),            // id
    }

    ui.set_min_width(320.0);
    ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);

    // While a game runs, the main version selector is disabled too — picking
    // a row here must not reset launch_status to Idle under a live process.
    let game_running = matches!(
        *state.launch_status.lock().unwrap(),
        crate::state::LaunchStatus::Running(_)
    );

    if state.remote_versions.is_empty() {
        // Catalog not fetched yet: trigger the same background fetch the
        // install section uses and show a spinner while it loads.
        if let Some(Err(e)) = MANIFEST.lock().unwrap().clone() {
            ui.label(
                egui::RichText::new(format!("⚠ Manifest error: {e}"))
                    .small()
                    .color(ERROR),
            );
        } else {
            fetch_manifest_async(state);
        }
        ui.set_min_height(120.0);
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label("Loading versions…");
        });
        ui.ctx().request_repaint();
        return;
    }

    let mut rows: Vec<Row> = Vec::new();
    rows.push(Row::Header("Installed"));
    if state.installed_versions.is_empty() {
        // Empty state: gray hint mirrors the old standalone list message.
        rows.push(Row::Hint("Nothing installed yet — pick a version below"));
    }
    for v in &state.installed_versions {
        let tag = AppState::version_tag(v);
        let label = if tag.is_empty() {
            v.clone()
        } else {
            format!("{v}  {tag}")
        };
        rows.push(Row::Installed(label, v.clone()));
    }
    rows.push(Row::Header("Not installed"));
    for v in &state.remote_versions {
        if !state.installed_versions.iter().any(|x| x == v) {
            rows.push(Row::Remote(v.clone()));
        }
    }

    let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical()
        .id_salt(egui::Id::new("launch_picker_scroll"))
        .max_height(260.0)
        .min_scrolled_height(260.0)
        .auto_shrink(false)
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            for idx in range {
                match &rows[idx] {
                    Row::Header(label) => {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(*label)
                                .strong()
                                .color(egui::Color32::GRAY),
                        );
                        ui.add_space(2.0);
                    }
                    Row::Hint(label) => {
                        ui.add_space(2.0);
                        ui.label(
                            egui::RichText::new(*label)
                                .small()
                                .color(egui::Color32::GRAY)
                                .italics(),
                        );
                        ui.add_space(2.0);
                    }
                    Row::Installed(label, id) => {
                        ui.horizontal(|ui| {
                            let selected =
                                state.selected_version.as_deref() == Some(id.as_str());
                            let resp = if game_running {
                                ui.add_enabled(
                                    false,
                                    egui::Button::selectable(selected, label.clone()),
                                )
                            } else {
                                ui.add(egui::Button::selectable(selected, label.clone()))
                            };
                            if resp.clicked() {
                                state.selected_version = Some(id.clone());
                                *state.launch_status.lock().unwrap() =
                                    crate::state::LaunchStatus::Idle;
                                state.pending_install = None;
                                ui.close();
                            }
                            // Delete button on installed rows only (mirrors the
                            // old top selector's 🗑). Disabled while a game runs.
                            let del = if game_running {
                                ui.add_enabled(false, egui::Button::new("🗑"))
                            } else {
                                ui.add(egui::Button::new("🗑"))
                            };
                            if del.clicked() {
                                state.pending_delete = Some(id.clone());
                                ui.close();
                            }
                        });
                    }
                    Row::Remote(id) => {
                        let selected =
                            state.pending_install.as_deref() == Some(id.as_str());
                        let resp = if game_running {
                            ui.add_enabled(false, egui::Button::selectable(selected, id.clone()))
                        } else {
                            ui.add(egui::Button::selectable(selected, id.clone()))
                        };
                        if resp.clicked() {
                            state.pending_install = Some(id.clone());
                            ui.close();
                        }
                    }
                }
            }
        });
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
    if let Some(inner) = egui::Window::new("Close game")
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
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    if !open {
        state.pending_close_game = false;
    }
}

/// Confirmation dialog for deleting a version. Removes the entire
/// `versions/<id>/` folder (including natives-extracted, .json, .jar).
fn delete_confirm_window(ctx: &egui::Context, state: &mut AppState) {
    let ver = state.pending_delete.clone().unwrap_or_default();
    let mut open = state.pending_delete.is_some();
    if let Some(inner) = egui::Window::new("Delete version")
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
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    if !open {
        state.pending_delete = None;
    }
}

fn install_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Install").strong());
    ui.add_space(2.0);

    // Drain the manifest produced by the background fetch (Ok only; an error
    // stays in the slot so `manifest_error()` can surface it).
    if let Some(Ok(ids)) = MANIFEST.lock().unwrap().take() {
        state.remote_versions = ids;
    }

    // One combined catalog: everything installable. Sources are fetched on
    // demand in the background; while any is missing we show the loader.
    #[derive(Clone)]
    enum Choice {
        Vanilla(String),
        Forge(String, String),
        OptiFine(String, String),
    }

    let busy = state.task_snapshot().is_busy();
    let mut loading = false;
    let mut error: Option<String> = None;

    if state.remote_versions.is_empty() {
        loading = true;
        if let Some(Err(e)) = MANIFEST.lock().unwrap().clone() {
            error = Some(e);
        }
        fetch_manifest_async(state);
    }

    let forge = super::install_view::forge_catalog();
    if forge.is_none() {
        loading = true;
        if let Some(e) = super::install_view::forge_error() {
            error = Some(e);
        }
        super::install_view::fetch_forge_async();
    }

    let optifine = super::install_view::optifine_catalog();
    if optifine.is_none() {
        loading = true;
        if let Some(e) = super::install_view::optifine_error() {
            error = Some(e);
        }
        super::install_view::fetch_optifine_async();
    }

    // Build the catalog: manifest order (newest first), existing installs
    // marked. No caps — the selector's list is virtualized, so a full
    // manifest with ~1000 versions renders only the visible rows.
    let mut choices: Vec<Choice> = Vec::new();
    let mut items: Vec<super::selector::SelectorItem> = Vec::new();
    for id in state.remote_versions.clone() {
        let installed = state.installed_versions.iter().any(|v| v == &id);
        let label = if installed {
            format!("Vanilla {id}  (installed)")
        } else {
            format!("Vanilla {id}")
        };
        choices.push(Choice::Vanilla(id.clone()));
        items.push((format!("vanilla {id}"), label));
    }
    if let Some(fd) = forge {
        for mc in fd.mc_sorted {
            let build = crate::install::forge::default_build(&mc, &fd.by_mc[&mc], &fd.promos);
            choices.push(Choice::Forge(mc.clone(), build.clone()));
            items.push((
                format!("forge {mc}"),
                format!("Forge {mc}  ({build})"),
            ));
        }
    }
    if let Some(od) = optifine {
        for mc in od.mc_sorted {
            let build = od.by_mc[&mc].last().cloned().unwrap_or_default();
            choices.push(Choice::OptiFine(mc.clone(), build.clone()));
            items.push((
                format!("optifine {mc}"),
                format!("OptiFine {mc}  ({build})"),
            ));
        }
    }

    let mut pick: Option<usize> = None;
    super::selector::selector(
        ui,
        "install_select",
        &items,
        &mut pick,
        !busy,
        None,
        Some(&mut |q: &str| {
            let q = q.to_ascii_lowercase();
            items
                .iter()
                .filter(|(id, label)| {
                    id.to_ascii_lowercase().contains(&q)
                        || label.to_ascii_lowercase().contains(&q)
                })
                .cloned()
                .collect()
        }),
        Some("Type to filter…"),
        Some("Install type…"),
        loading,
        error.as_deref(),
    );

    // The pick is transient (not persisted between frames), so act on it here.
    if let Some(idx) = pick {
        match &choices[idx] {
            Choice::Vanilla(ver) => start_vanilla_download(state, ver.clone()),
            Choice::Forge(mc, build) => {
                super::install_view::start_forge_install(state, mc.clone(), build.clone())
            }
            Choice::OptiFine(_mc, build) => {
                super::install_view::start_optifine_install(state, build.clone())
            }
        }
    }

    ui.horizontal_wrapped(|ui| {
        if ui.add_enabled(!busy, egui::Button::new("Java")).clicked() {
            state.show_install_java = true;
        }
        if ui.add_enabled(!busy, egui::Button::new("Mods / Resourcepacks")).clicked() {
            state.show_content = true;
        }
    });
}

/// Fetch the Mojang manifest in a background thread, storing the result (or
/// error) into a shared global that the UI picks up.
fn fetch_manifest_async(state: &AppState) {
    if !manifest_fetch_allowed() {
        return;
    }
    let task = state.task.clone();
    std::thread::spawn(move || {
        match crate::install::vanilla::fetch_manifest() {
            Ok(list) => {
                let ids: Vec<String> = list.into_iter().map(|(id, _)| id).collect();
                *MANIFEST.lock().unwrap() = Some(Ok(ids));
                if let Ok(mut t) = task.lock() {
                    if !t.is_busy() {
                        *t = Task::Idle;
                    }
                }
            }
            Err(e) => {
                *MANIFEST.lock().unwrap() = Some(Err(e.clone()));
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
/// `Err` = last fetch failed (retried after the 30 s throttle window).
static MANIFEST: std::sync::LazyLock<std::sync::Mutex<Option<Result<Vec<String>, String>>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

/// When the last manifest fetch attempt started (refetch throttle).
static MANIFEST_LAST_FETCH: std::sync::LazyLock<std::sync::Mutex<Option<std::time::Instant>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

fn manifest_fetch_allowed() -> bool {
    let now = std::time::Instant::now();
    let mut guard = MANIFEST_LAST_FETCH.lock().unwrap();
    if let Some(t) = *guard {
        if now.duration_since(t).as_secs() < 30 {
            return false;
        }
    }
    *guard = Some(now);
    true
}
