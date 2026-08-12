// ui/install_view.rs - Modal windows for Forge / OptiFine / Java / Content installs.
//
// Background workers communicate fetched data to the UI thread via global
// LazyLock<Mutex<Option<...>>> slots — NOT thread_local (which is per-thread
// and would never be visible to the UI thread).
use crate::state::{AppState, Task};
use egui::RichText;
use std::sync::{LazyLock, Mutex};

// ------------------------------------------------------------------
// Shared data slots (worker thread writes, UI thread reads)
// ------------------------------------------------------------------

#[derive(Clone)]
struct ForgeData {
    mc_sorted: Vec<String>,
    promos: std::collections::HashMap<String, String>,
    by_mc: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Clone)]
struct OptiFineData {
    mc_sorted: Vec<String>,
    by_mc: std::collections::BTreeMap<String, Vec<String>>,
}

static FORGE_SLOT: LazyLock<Mutex<Option<ForgeData>>> = LazyLock::new(|| Mutex::new(None));
static OPTIFINE_SLOT: LazyLock<Mutex<Option<OptiFineData>>> = LazyLock::new(|| Mutex::new(None));
static CONTENT_SLOT: LazyLock<Mutex<Option<crate::content::ContentIndex>>> =
    LazyLock::new(|| Mutex::new(None));

/// Whether Forge metadata has been fetched and is ready to display.
pub fn forge_data_cached() -> bool {
    FORGE_SLOT.lock().unwrap().is_some()
}

/// Whether OptiFine metadata has been fetched and is ready to display.
pub fn optifine_data_cached() -> bool {
    OPTIFINE_SLOT.lock().unwrap().is_some()
}

// ------------------------------------------------------------------
// Window router
// ------------------------------------------------------------------

pub fn render_windows(ctx: &egui::Context, state: &mut AppState) {
    if state.show_install_forge {
        forge_window(ctx, state);
    }
    if state.show_install_java {
        java_window(ctx, state);
    }
    if state.show_install_optifine {
        optifine_window(ctx, state);
    }
    if state.show_content {
        content_window(ctx, state);
    }
}

// ------------------------------------------------------------------
// Forge
// ------------------------------------------------------------------

fn forge_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_install_forge;
    if let Some(inner) = egui::Window::new("Install Forge")
        .open(&mut open)
        .default_width(420.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            let data = FORGE_SLOT.lock().unwrap().clone();
            let busy = state.task_snapshot().is_busy();
            let data = match data {
                Some(d) => d,
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Fetching Forge versions…");
                    });
                    ctx.request_repaint();
                    return;
                }
            };
            ui.label("Minecraft version (newest first):");
            let show = data.mc_sorted.len().min(20);
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (i, mc) in data.mc_sorted.iter().take(show).enumerate() {
                    let builds = &data.by_mc[mc];
                    let default_b =
                        crate::install::forge::default_build(mc, builds, &data.promos);
                    let label = format!("{}. {}  ({})", i + 1, mc, default_b);
                    if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                        state.show_install_forge = false;
                        start_forge_install(state, mc.clone(), default_b);
                        return;
                    }
                }
            });
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    state.show_install_forge = open;
}

pub fn fetch_forge_async() {
    std::thread::spawn(|| {
        match crate::install::forge::fetch_metadata() {
            Ok(by_mc) => {
                let promos = crate::install::forge::fetch_promos();
                let mc_sorted = crate::install::forge::sorted_mc_versions(&by_mc);
                *FORGE_SLOT.lock().unwrap() = Some(ForgeData {
                    mc_sorted,
                    promos,
                    by_mc,
                });
            }
            Err(e) => eprintln!("forge fetch error: {e}"),
        }
    });
}

fn start_forge_install(state: &mut AppState, mc: String, build: String) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    state.set_task(Task::Running {
        title: format!("Installing Forge {mc}-{build}"),
        steps: Vec::new(),
        progress_current: 0,
        progress_total: 0,
    });
    std::thread::spawn(move || {
        let java = match crate::java::find_java(&work_dir, 17) {
            Some(j) => j,
            None => {
                if let Ok(mut t) = task.lock() {
                    *t = Task::Error("Java 17+ required for Forge installer".into());
                }
                return;
            }
        };
        let result = crate::install::forge::install_forge(&mc, &build, &work_dir, &java);
        let msg = match result {
            Ok(()) => Task::Done(format!("Forge {mc}-{build} installed")),
            Err(e) => Task::Error(format!("Forge install failed: {e}")),
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
        *FORGE_SLOT.lock().unwrap() = None;
    });
}

// ------------------------------------------------------------------
// OptiFine
// ------------------------------------------------------------------

fn optifine_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_install_optifine;
    if let Some(inner) = egui::Window::new("Install OptiFine")
        .open(&mut open)
        .default_width(420.0)
        .default_height(440.0)
        .show(ctx, |ui| {
            let data = OPTIFINE_SLOT.lock().unwrap().clone();
            let busy = state.task_snapshot().is_busy();
            let data = match data {
                Some(d) => d,
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Fetching OptiFine versions…");
                    });
                    ctx.request_repaint();
                    return;
                }
            };
            ui.label("Minecraft version (newest first):");
            let show = data.mc_sorted.len().min(20);
            egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                for (i, mc) in data.mc_sorted.iter().take(show).enumerate() {
                    let builds = &data.by_mc[mc];
                    let latest = builds.last().cloned().unwrap_or_default();
                    let label = format!("{}. {}  ({})", i + 1, mc, latest);
                    if ui.add_enabled(!busy, egui::Button::new(label)).clicked() {
                        state.show_install_optifine = false;
                        start_optifine_install(state, latest);
                        return;
                    }
                }
            });
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    state.show_install_optifine = open;
}

pub fn fetch_optifine_async() {
    std::thread::spawn(|| {
        match crate::install::optifine::fetch_versions() {
            Ok(by_mc) => {
                let mc_sorted = crate::install::optifine::sorted_mc_versions(&by_mc);
                *OPTIFINE_SLOT.lock().unwrap() = Some(OptiFineData { mc_sorted, by_mc });
            }
            Err(e) => eprintln!("optifine fetch error: {e}"),
        }
    });
}

fn start_optifine_install(state: &mut AppState, build: String) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    state.set_task(Task::Running {
        title: format!("Installing OptiFine {build}"),
        steps: Vec::new(),
        progress_current: 0,
        progress_total: 0,
    });
    std::thread::spawn(move || {
        let java = match crate::java::find_java(&work_dir, 8) {
            Some(j) => j,
            None => {
                if let Ok(mut t) = task.lock() {
                    *t = Task::Error("Java 8+ required for OptiFine installer".into());
                }
                return;
            }
        };
        let result = crate::install::optifine::install_optifine(&build, &work_dir, &java);
        let msg = match result {
            Ok(()) => Task::Done(format!("OptiFine {build} installed")),
            Err(e) => Task::Error(format!("OptiFine install failed: {e}")),
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
        *OPTIFINE_SLOT.lock().unwrap() = None;
    });
}

// ------------------------------------------------------------------
// Java JDK
// ------------------------------------------------------------------

fn java_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_install_java;
    if let Some(inner) = egui::Window::new("Install Java")
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            let busy = state.task_snapshot().is_busy();
            ui.label("Choose a Java version to install (portable):");
            ui.add_space(6.0);
            let mut invalid: Option<String> = None;
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(!busy, egui::Button::new("Java 21\n(MC 1.20.5+)"))
                    .clicked()
                {
                    state.show_install_java = false;
                    start_java_install(state, 21);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Java 17\n(MC 1.17–1.20.4)"))
                    .clicked()
                {
                    state.show_install_java = false;
                    start_java_install(state, 17);
                }
                if ui
                    .add_enabled(!busy, egui::Button::new("Java 8\n(MC 1.16.5 and older)"))
                    .clicked()
                {
                    state.show_install_java = false;
                    start_java_install(state, 8);
                }
            });
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label("Custom:");
                ui.add(
                    egui::TextEdit::singleline(&mut state.java_custom)
                        .desired_width(50.0)
                        .hint_text("e.g. 11"),
                );
                let valid = !state.java_custom.trim().is_empty()
                    && state.java_custom.trim().parse::<u32>().map_or(false, |n| (8..=100).contains(&n));
                let custom_clicked = ui
                    .add_enabled(!busy && valid, egui::Button::new("Install"))
                    .clicked();
                if custom_clicked {
                    if let Ok(n) = state.java_custom.trim().parse::<u32>() {
                        state.show_install_java = false;
                        start_java_install(state, n);
                    }
                }
                if !state.java_custom.trim().is_empty() && !valid {
                    invalid = Some(
                        "Any major version 8–100, e.g. 11, 21, 24".into(),
                    );
                }
            });
            if let Some(msg) = invalid {
                ui.label(RichText::new(msg).color(crate::theme::ERROR).small());
            }
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    state.show_install_java = open;
}

fn start_java_install(state: &mut AppState, major: u32) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    state.set_task(Task::Running {
        title: format!("Installing Java {major}"),
        steps: Vec::new(),
        progress_current: 0,
        progress_total: 0,
    });
    std::thread::spawn(move || {
        let result = crate::install::java_jdk::install_jdk(major, &work_dir);
        let msg = match result {
            Ok(p) => {
                let ver = crate::java::get_java_version(&p);
                Task::Done(format!("Java {ver} installed to {}", p.display()))
            }
            Err(e) => Task::Error(format!("Java install failed: {e}")),
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
    });
}

// ------------------------------------------------------------------
// Content (mods/resourcepacks/shaders)
// ------------------------------------------------------------------

fn content_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_content;
    if let Some(inner) = egui::Window::new("Download content")
        .open(&mut open)
        .default_width(420.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            if state.settings.content_index_url.is_empty() {
                ui.label(
                    RichText::new(
                        "No content index URL set. Configure it in Settings → Content index URL.",
                    )
                    .color(egui::Color32::GRAY),
                );
                return;
            }
            let busy = state.task_snapshot().is_busy();
            ui.label(format!("Index: {}", state.settings.content_index_url));
            ui.add_space(6.0);
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh index"))
                .clicked()
            {
                *CONTENT_SLOT.lock().unwrap() = None;
                fetch_content_async(state.settings.content_index_url.clone());
            }
            let data = CONTENT_SLOT.lock().unwrap().clone();
            let idx = match data {
                Some(d) => d,
                None => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Fetching content index…");
                    });
                    ctx.request_repaint();
                    return;
                }
            };
            let mut versions: Vec<&String> = idx.versions.keys().collect();
            versions.sort();
            let mut trigger: Option<(String, String)> = None;
            for mc in &versions {
                ui.collapsing(format!("Minecraft {mc}"), |ui| {
                    for cat in ["mods", "resourcepacks", "shaderpacks"] {
                        if let Some(files) =
                            idx.versions.get(*mc).and_then(|m| m.get(cat))
                        {
                            if files.is_empty() {
                                continue;
                            }
                            ui.add_space(2.0);
                            ui.label(
                                RichText::new(format!("{cat} ({})", files.len())).strong(),
                            );
                            for f in files.iter() {
                                if ui.add_enabled(!busy, egui::Button::new(&f.name)).clicked() {
                                    trigger = Some((mc.to_string(), cat.to_string()));
                                }
                            }
                        }
                    }
                });
            }
            if let Some((mc, cat)) = trigger {
                state.show_content = false;
                start_content_download(state, mc, cat);
            }
        }) {
        super::window_close_cursor(ctx, inner.response.rect);
    }
    state.show_content = open;
}

fn fetch_content_async(url: String) {
    std::thread::spawn(move || match crate::content::fetch_index(&url) {
        Ok(idx) => {
            *CONTENT_SLOT.lock().unwrap() = Some(idx);
        }
        Err(e) => eprintln!("content fetch error: {e}"),
    });
}

fn start_content_download(state: &mut AppState, mc: String, cat: String) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    let files = CONTENT_SLOT
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|idx| idx.versions.get(&mc))
        .and_then(|m| m.get(&cat))
        .cloned()
        .unwrap_or_default();
    let dest_sub = crate::content::category_folder(&cat).unwrap_or("mods").to_string();
    state.set_task(Task::Running {
        title: format!("Downloading {cat} for {mc}"),
        steps: Vec::new(),
        progress_current: 0,
        progress_total: files.len(),
    });
    let task_clone = task.clone();
    std::thread::spawn(move || {
        let progress = std::sync::Arc::new(move |label: &str, cur: usize, tot: usize| {
            if let Ok(mut t) = task_clone.lock() {
                *t = Task::Running {
                    title: label.to_string(),
                    steps: Vec::new(),
                    progress_current: cur,
                    progress_total: tot,
                };
            }
        });
        let dest = work_dir.join(&dest_sub);
        let (ok, failed) = crate::content::download_files(&files, &dest, &*progress);
        let msg = if failed == 0 {
            Task::Done(format!("Downloaded {ok} {cat} to {}", dest.display()))
        } else {
            Task::Error(format!("{ok} ok, {failed} failed downloading {cat}"))
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
    });
}
