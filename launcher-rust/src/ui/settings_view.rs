// ui/settings_view.rs - Settings screen: RAM, username, content index, theme, update check.
use crate::settings::{Settings, Theme};
use crate::state::{AppState, Task, APP_VERSION};
use crate::theme::{ACCENT, ERROR, INFO};
use egui::{Align, Color32, Layout, RichText, Ui};

pub fn render(ui: &mut Ui, state: &mut AppState) {
    // The text fields (username, content URL) edit a persistent buffer in
    // egui's data store, not a per-frame clone. Cloning from settings each
    // frame made egui force the OLD value back while typing — you could
    // never erase below 3 chars (the field bounced) and the URL field was
    // practically uneditable. The buffer is re-seeded on every entry.
    let open_flag_id = egui::Id::new("settings_was_open");
    let was_open = ui
        .ctx()
        .data(|d| d.get_temp::<bool>(open_flag_id))
        .unwrap_or(false);
    if !was_open {
        ui.ctx().data_mut(|d| {
            d.insert_temp(username_edit_id(), state.settings.username.clone());
            d.insert_temp(url_edit_id(), state.settings.content_index_url.clone());
        });
    }
    ui.ctx().data_mut(|d| d.insert_temp(open_flag_id, true));

    // This view is docked into the main window's central area (main_view
    // owns the top bar / side panel / status bar) — no panels of its own.
    // Header row: back button + title.
    ui.horizontal(|ui| {
        if ui.button("‹ Back").clicked() {
            // Leaving settings: forget the text buffers so the next visit
            // re-seeds them from the saved values (see render()).
            ui.ctx().data_mut(|d| d.insert_temp(open_flag_id, false));
            state.show_settings = false;
        }
        ui.heading(RichText::new("Settings").color(ACCENT).strong());
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.label(
                RichText::new(format!("v{}", APP_VERSION))
                    .small()
                    .color(Color32::GRAY),
            );
        });
    });
    ui.add_space(4.0);

    egui::ScrollArea::vertical().show(ui, |ui| {
        memory_section(ui, state);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        username_section(ui, state);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        content_index_section(ui, state);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        theme_section(ui, state);
        ui.add_space(10.0);
        ui.separator();
        ui.add_space(10.0);

        update_section(ui, state);
    });
}

fn memory_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Memory (allocated to Minecraft)").strong());
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        ui.label("Current:");
        ui.label(format!(
            "MIN {} / MAX {}",
            state.settings.ram_min, state.settings.ram_max
        ));
    });
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        ui.label("MIN:");
        // NB: clearing via the ✖ does NOT fire `changed()` (the flag is
        // computed before the clear), so a ✖-emptied RAM value is not
        // persisted until the next keystroke — safer than a manual
        // select-all+delete, which would save "" straight to disk.
        let r = crate::ui::input::TextInput::new(&mut state.settings.ram_min)
            .desired_width(60.0)
            .show(ui);
        if r.changed() {
            let _ = state.save_settings();
        }
        ui.label("  MAX:");
        let r = crate::ui::input::TextInput::new(&mut state.settings.ram_max)
            .desired_width(60.0)
            .show(ui);
        if r.changed() {
            let _ = state.save_settings();
        }
        ui.label(RichText::new("e.g. 4G, 8192M").small().color(Color32::GRAY));
    });
}

const USERNAME_HINT: &str = "3–16 chars, letters / numbers / underscore";

fn username_edit_id() -> egui::Id {
    egui::Id::new("settings_username_edit")
}

fn username_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("In-game username").strong());
    ui.add_space(2.0);
    // Persistent edit buffer (seeded each time settings is opened): the
    // widget edits this string frame-to-frame, so the user's text is never
    // overwritten by the old saved value mid-edit.
    let mut name = ui
        .ctx()
        .data(|d| d.get_temp::<String>(username_edit_id()))
        .unwrap_or_default();
    let resp = crate::ui::input::TextInput::new(&mut name)
        .desired_width(180.0)
        .show(ui);
    ui.ctx().data_mut(|d| d.insert_temp(username_edit_id(), name.clone()));
    ui.label(RichText::new(USERNAME_HINT).small().color(Color32::GRAY));
    // NB: clearing via the field's ✖ (ui/input.rs) blurs the edit for a
    // frame, so this lost_focus handler fires mid-clear. An empty name is
    // therefore left alone: nothing is persisted, the launch keeps using the
    // last saved nick, and re-opening settings re-seeds the buffer from it.
    if resp.lost_focus() {
        let trimmed = name.trim();
        if Settings::is_valid_username(trimmed) {
            if trimmed != state.settings.username {
                state.settings.username = trimmed.to_string();
                let _ = state.save_settings();
            }
        } else if !trimmed.is_empty() {
            // Bad length/characters: keep the field editable and just
            // surface the validation error.
            state.set_task(Task::Error("Invalid username: 3–16 chars, A-Z a-z 0-9 _".into()));
        }
    }
}

fn url_edit_id() -> egui::Id {
    egui::Id::new("settings_url_edit")
}

fn content_index_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Content index URL (mods / resourcepacks / shaders)").strong());
    ui.add_space(2.0);
    let mut url = ui
        .ctx()
        .data(|d| d.get_temp::<String>(url_edit_id()))
        .unwrap_or_default();
    let resp = crate::ui::input::TextInput::new(&mut url)
        .desired_width(420.0)
        .show(ui);
    ui.ctx().data_mut(|d| d.insert_temp(url_edit_id(), url.clone()));
    ui.label(
        RichText::new("Direct/raw link to your index JSON, e.g. https://raw.githubusercontent.com/.../index.json")
            .small()
            .color(Color32::GRAY),
    );
    if resp.lost_focus() && url.trim() != state.settings.content_index_url {
        state.settings.content_index_url = url.trim().to_string();
        let _ = state.save_settings();
    }
}

fn theme_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Theme").strong());
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        let cur = state.settings.theme;
        if ui
            .radio(cur == Theme::Dark, "🌙 Dark")
            .on_hover_text("Dark theme (default)")
            .clicked()
            && cur != Theme::Dark
        {
            state.settings.theme = Theme::Dark;
            crate::theme::apply(ui.ctx(), Theme::Dark);
            let _ = state.save_settings();
        }
        if ui
            .radio(cur == Theme::Light, "☀ Light")
            .on_hover_text("Light theme")
            .clicked()
            && cur != Theme::Light
        {
            state.settings.theme = Theme::Light;
            crate::theme::apply(ui.ctx(), Theme::Light);
            let _ = state.save_settings();
        }
    });
}

fn update_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Updates").strong());
    ui.add_space(2.0);
    ui.horizontal(|ui| {
        if ui.button("Check for updates").clicked() {
            match crate::update::check_latest() {
                Ok(latest) => {
                    if crate::update::is_newer(&latest, APP_VERSION) {
                        state.update_available = Some(latest.clone());
                        state.update_msg = Some((
                            false,
                            format!("New version available: v{latest}"),
                        ));
                        state.set_task(Task::Done(format!(
                            "Update available: v{} → v{}. Click 'Install update' below.",
                            APP_VERSION, latest
                        )));
                    } else {
                        state.update_available = None;
                        state.update_msg = Some((
                            true,
                            format!("You are on the latest version (v{APP_VERSION})"),
                        ));
                        state.set_task(Task::Done(format!(
                            "You are on the latest version (v{}).",
                            APP_VERSION
                        )));
                    }
                }
                Err(e) => {
                    state.update_msg = Some((false, format!("Update check failed: {e}")));
                    state.set_task(Task::Error(format!("Update check failed: {e}")));
                }
            }
        }
        if let Some((ok, msg)) = &state.update_msg {
            // Update-available is info, not an error: blue while a newer
            // release exists, green when already up to date, red only for
            // real failures (which come without `update_available`).
            let color = if *ok {
                ACCENT
            } else if state.update_available.is_some() {
                INFO
            } else {
                ERROR
            };
            ui.label(RichText::new(msg).color(color).small());
        }
        if let Some(latest) = &state.update_available {
            ui.label(RichText::new(format!("New version: v{latest}")).color(INFO));
            if ui.button("Install update & restart").clicked() {
                // Run update in the background; UI will reflect completion.
                let task = state.task.clone();
                state.set_task(Task::Running {
                    title: "Downloading update…".into(),
                    steps: Vec::new(),
                    progress_current: 0,
                    progress_total: 0,
                });
                std::thread::spawn(move || {
                    let result = crate::update::check_and_update(APP_VERSION);
                    let msg = match result {
                        Ok(crate::update::UpdateOutcome::Updated(path)) => {
                            // Restart is part of the button's promise: spawn
                            // the fresh binary and flag this instance to exit
                            // (checked each frame in main.rs).
                            match crate::update::relaunch_after_update(&path) {
                                Ok(()) => Task::Done(
                                    "Update installed — restarting…".into(),
                                ),
                                Err(e) => Task::Error(format!(
                                    "Update installed, but restart failed: {e}. \
                                     Restart the launcher ({}) manually.",
                                    path.display()
                                )),
                            }
                        }
                        Ok(crate::update::UpdateOutcome::UpToDate) => {
                            Task::Done("Already up to date.".into())
                        }
                        Err(e) => Task::Error(format!("Update failed: {e}")),
                    };
                    if let Ok(mut t) = task.lock() {
                        *t = msg;
                    }
                });
            }
        }
    });
}
