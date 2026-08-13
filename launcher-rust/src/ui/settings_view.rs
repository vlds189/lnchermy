// ui/settings_view.rs - Settings screen: RAM, username, content index, theme, update check.
use crate::settings::{Settings, Theme};
use crate::state::{AppState, Task, APP_VERSION};
use crate::theme::{ACCENT, ERROR};
use egui::{Align, Color32, Layout, RichText, Ui};

// RAM presets mirror the PowerShell launcher (option 1-5 in Run-Settings).
const RAM_PRESETS: &[(&str, &str, &str)] = &[
    ("2G", "4G", "Default (8GB+ RAM)"),
    ("4G", "6G", ""),
    ("4G", "8G", "Recommended for mods"),
    ("8G", "12G", "Heavy modpacks"),
    ("8G", "16G", ""),
];

pub fn render(ui: &mut Ui, state: &mut AppState) {
    egui::Panel::top("settings_top").show(ui, |ui| {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("‹ Back").clicked() {
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
        ui.separator();
    });

    egui::CentralPanel::default().show(ui, |ui| {
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

    ui.label(RichText::new("Presets").color(Color32::GRAY).small());
    ui.horizontal_wrapped(|ui| {
        for (mn, mx, note) in RAM_PRESETS {
            let label = if note.is_empty() {
                format!("{mn} / {mx}")
            } else {
                format!("{mn} / {mx}  ({note})")
            };
            if ui.button(label).clicked() {
                state.settings.ram_min = mn.to_string();
                state.settings.ram_max = mx.to_string();
                let _ = state.save_settings();
            }
        }
    });

    ui.add_space(6.0);
    ui.label(RichText::new("Custom").color(Color32::GRAY).small());
    ui.horizontal(|ui| {
        ui.label("MIN:");
        let r = egui::TextEdit::singleline(&mut state.settings.ram_min)
            .desired_width(60.0)
            .show(ui)
            .response;
        if r.changed() {
            let _ = state.save_settings();
        }
        ui.label("  MAX:");
        let r = egui::TextEdit::singleline(&mut state.settings.ram_max)
            .desired_width(60.0)
            .show(ui)
            .response;
        if r.changed() {
            let _ = state.save_settings();
        }
        ui.label(RichText::new("e.g. 4G, 8192M").small().color(Color32::GRAY));
    });
}

fn username_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("In-game username").strong());
    ui.add_space(2.0);
    let mut name = state.settings.username.clone();
    let resp = egui::TextEdit::singleline(&mut name)
        .desired_width(180.0)
        .show(ui)
        .response;
    ui.label(
        RichText::new("3–16 chars, letters / numbers / underscore")
            .small()
            .color(Color32::GRAY),
    );
    if resp.lost_focus() {
        let trimmed = name.trim();
        if trimmed != state.settings.username && Settings::is_valid_username(trimmed) {
            state.settings.username = trimmed.to_string();
            let _ = state.save_settings();
        } else if !trimmed.is_empty() && !Settings::is_valid_username(trimmed) {
            state.set_task(Task::Error("Invalid username: 3–16 chars, A-Z a-z 0-9 _".into()));
        }
    } else if resp.changed() {
        // Allow live editing but only persist valid values.
        if Settings::is_valid_username(name.trim()) || name.trim().is_empty() {
            state.settings.username = name.trim().to_string();
        }
    }
}

fn content_index_section(ui: &mut Ui, state: &mut AppState) {
    ui.label(RichText::new("Content index URL (mods / resourcepacks / shaders)").strong());
    ui.add_space(2.0);
    let mut url = state.settings.content_index_url.clone();
    let resp = egui::TextEdit::singleline(&mut url)
        .desired_width(420.0)
        .show(ui)
        .response;
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
                        state.set_task(Task::Error(format!(
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
            let color = if *ok { ACCENT } else { ERROR };
            ui.label(RichText::new(msg).color(color).small());
        }
        if let Some(latest) = &state.update_available {
            ui.label(RichText::new(format!("New version: v{latest}")).color(ERROR));
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
