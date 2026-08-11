// ui/install_view.rs - Modal windows for Forge / OptiFine / Java / Content installs.
use crate::state::{AppState, Task};
use egui::RichText;

thread_local! {
    static FORGE_DATA: std::cell::RefCell<Option<ForgeData>> = std::cell::RefCell::new(None);
    static OPTIFINE_DATA: std::cell::RefCell<Option<OptiFineData>> = std::cell::RefCell::new(None);
}

/// Whether Forge metadata has been fetched and is ready to display.
pub fn forge_data_cached() -> bool {
    FORGE_DATA.with(|d| d.borrow().is_some())
        || FORGE_PICK.with(|p| p.borrow().is_some())
}

/// Whether OptiFine metadata has been fetched and is ready to display.
pub fn optifine_data_cached() -> bool {
    OPTIFINE_DATA.with(|d| d.borrow().is_some())
        || OPTIFINE_PICK.with(|p| p.borrow().is_some())
}

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

/// Render all open install windows. Called from main_view after the central panel.
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
    // Drain background data if ready.
    FORGE_DATA.with(|d| {
        if let Some(data) = d.borrow_mut().take() {
            FORGE_PICK.with(|p| *p.borrow_mut() = Some(data));
        }
    });

    let mut open = state.show_install_forge;
    egui::Window::new("Install Forge")
        .open(&mut open)
        .default_width(420.0)
        .default_height(480.0)
        .show(ctx, |ui| {
            let data_ready = FORGE_PICK.with(|p| p.borrow().is_some());
            if !data_ready {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching Forge versions…");
                });
                ctx.request_repaint();
                return;
            }
            FORGE_PICK.with(|p| {
                let p = p.borrow();
                let data = p.as_ref().unwrap();
                let busy = state.task_snapshot().is_busy();

                ui.label("Minecraft version (newest first):");
                let show = data.mc_sorted.len().min(20);
                let mut to_install: Option<(String, String)> = None;
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for (i, mc) in data.mc_sorted.iter().take(show).enumerate() {
                        let builds = &data.by_mc[mc];
                        let default_b = crate::install::forge::default_build(
                            mc,
                            builds,
                            &data.promos,
                        );
                        let label = format!("{}. {}  ({})", i + 1, mc, default_b);
                        if ui
                            .add_enabled(!busy, egui::Button::new(label))
                            .clicked()
                        {
                            to_install = Some((mc.clone(), default_b));
                        }
                    }
                });
                ui.add_space(6.0);
                ui.label(RichText::new("Or pick a specific build:").small());
                ui.text_edit_singleline(&mut state.forge_custom);
                if let Some((mc, build)) = to_install {
                    drop(p);
                    state.show_install_forge = false;
                    start_forge_install(state, mc, build);
                }
            });
        });
    state.show_install_forge = open;
}

thread_local! {
    static FORGE_PICK: std::cell::RefCell<Option<ForgeData>> = const { std::cell::RefCell::new(None) };
}

/// Kick off a background fetch of Forge metadata.
pub fn fetch_forge_async() {
    std::thread::spawn(|| {
        match crate::install::forge::fetch_metadata() {
            Ok(by_mc) => {
                let promos = crate::install::forge::fetch_promos();
                let mc_sorted = crate::install::forge::sorted_mc_versions(&by_mc);
                FORGE_DATA.with(|d| {
                    *d.borrow_mut() = Some(ForgeData {
                        mc_sorted,
                        promos,
                        by_mc,
                    })
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
        // Find Java 17+ for the installer.
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
        FORGE_DATA.with(|d| *d.borrow_mut() = None);
    });
}

// ------------------------------------------------------------------
// OptiFine
// ------------------------------------------------------------------

fn optifine_window(ctx: &egui::Context, state: &mut AppState) {
    OPTIFINE_DATA.with(|d| {
        if let Some(data) = d.borrow_mut().take() {
            OPTIFINE_PICK.with(|p| *p.borrow_mut() = Some(data));
        }
    });

    let mut open = state.show_install_optifine;
    egui::Window::new("Install OptiFine")
        .open(&mut open)
        .default_width(420.0)
        .default_height(440.0)
        .show(ctx, |ui| {
            let data_ready = OPTIFINE_PICK.with(|p| p.borrow().is_some());
            if !data_ready {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching OptiFine versions…");
                });
                ctx.request_repaint();
                return;
            }
            OPTIFINE_PICK.with(|p| {
                let p = p.borrow();
                let data = p.as_ref().unwrap();
                let busy = state.task_snapshot().is_busy();
                ui.label("Minecraft version (newest first):");
                let show = data.mc_sorted.len().min(20);
                let mut to_install: Option<String> = None;
                egui::ScrollArea::vertical().max_height(200.0).show(ui, |ui| {
                    for (i, mc) in data.mc_sorted.iter().take(show).enumerate() {
                        let builds = &data.by_mc[mc];
                        let latest = builds.last().cloned().unwrap_or_default();
                        let label = format!("{}. {}  ({})", i + 1, mc, latest);
                        if ui
                            .add_enabled(!busy, egui::Button::new(label))
                            .clicked()
                        {
                            to_install = Some(latest);
                        }
                    }
                });
                if let Some(build) = to_install {
                    drop(p);
                    state.show_install_optifine = false;
                    start_optifine_install(state, build);
                }
            });
        });
    state.show_install_optifine = open;
}

thread_local! {
    static OPTIFINE_PICK: std::cell::RefCell<Option<OptiFineData>> = const { std::cell::RefCell::new(None) };
}

pub fn fetch_optifine_async() {
    std::thread::spawn(|| {
        match crate::install::optifine::fetch_versions() {
            Ok(by_mc) => {
                let mc_sorted = crate::install::optifine::sorted_mc_versions(&by_mc);
                OPTIFINE_DATA.with(|d| {
                    *d.borrow_mut() = Some(OptiFineData { mc_sorted, by_mc })
                });
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
        // OptiFine installer runs on Java 8+.
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
        OPTIFINE_DATA.with(|d| *d.borrow_mut() = None);
    });
}

// ------------------------------------------------------------------
// Java JDK
// ------------------------------------------------------------------

fn java_window(ctx: &egui::Context, state: &mut AppState) {
    let mut open = state.show_install_java;
    egui::Window::new("Install Java")
        .open(&mut open)
        .default_width(360.0)
        .show(ctx, |ui| {
            let busy = state.task_snapshot().is_busy();
            ui.label("Choose a Java version to install (portable):");
            ui.add_space(6.0);
            let mut picked: Option<u32> = None;
            ui.horizontal(|ui| {
                if ui.add_enabled(!busy, egui::Button::new("Java 21\n(MC 1.20.5+)"))
                    .clicked()
                {
                    picked = Some(21);
                }
                if ui.add_enabled(!busy, egui::Button::new("Java 17\n(MC 1.17–1.20.4)"))
                    .clicked()
                {
                    picked = Some(17);
                }
                if ui.add_enabled(!busy, egui::Button::new("Java 8\n(MC 1.16.5 and older)"))
                    .clicked()
                {
                    picked = Some(8);
                }
            });
            ui.add_space(6.0);
            if let Some(major) = picked {
                state.show_install_java = false;
                start_java_install(state, major);
            }
        });
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
    egui::Window::new("Download content")
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
            // Fetch index, show versions/categories. For simplicity, download
            // everything for each version (the index is usually small).
            if ui
                .add_enabled(!busy, egui::Button::new("Refresh index"))
                .clicked()
            {
                state.content_index_error.clear();
                CONTENT_DATA.with(|d| *d.borrow_mut() = None);
                fetch_content_async(state.settings.content_index_url.clone());
            }
            // Drain fetched data.
            CONTENT_DATA.with(|d| {
                if let Some(data) = d.borrow_mut().take() {
                    CONTENT_PICK.with(|p| *p.borrow_mut() = Some(data));
                }
            });
            let ready = CONTENT_PICK.with(|p| p.borrow().is_some());
            if !ready {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Fetching content index…");
                });
                ctx.request_repaint();
                return;
            }
            CONTENT_PICK.with(|p| {
                let p = p.borrow();
                let idx = p.as_ref().unwrap();
                let mut versions: Vec<&String> = idx.versions.keys().collect();
                versions.sort();
                let mut trigger: Option<(String, String)> = None;
                for mc in &versions {
                    ui.collapsing(format!("Minecraft {mc}"), |ui| {
                        for cat in ["mods", "resourcepacks", "shaderpacks"] {
                            if let Some(files) = idx.versions.get(*mc).and_then(|m| m.get(cat)) {
                                if files.is_empty() {
                                    continue;
                                }
                                ui.add_space(2.0);
                                ui.label(
                                    RichText::new(format!("{cat} ({})", files.len())).strong(),
                                );
                                for (i, f) in files.iter().enumerate() {
                                    if ui
                                        .add_enabled(!busy, egui::Button::new(&f.name))
                                        .clicked()
                                    {
                                        trigger = Some((mc.to_string(), cat.to_string()));
                                    }
                                    let _ = i;
                                }
                            }
                        }
                    });
                }
                if let Some((mc, cat)) = trigger {
                    drop(p);
                    state.show_content = false;
                    start_content_download(state, mc, cat);
                }
            });
        });
    state.show_content = open;
}

thread_local! {
    static CONTENT_DATA: std::cell::RefCell<Option<crate::content::ContentIndex>> = const { std::cell::RefCell::new(None) };
    static CONTENT_PICK: std::cell::RefCell<Option<crate::content::ContentIndex>> = const { std::cell::RefCell::new(None) };
}

fn fetch_content_async(url: String) {
    std::thread::spawn(move || {
        match crate::content::fetch_index(&url) {
            Ok(idx) => {
                CONTENT_DATA.with(|d| *d.borrow_mut() = Some(idx));
            }
            Err(e) => eprintln!("content fetch error: {e}"),
        }
    });
}

fn start_content_download(state: &mut AppState, mc: String, cat: String) {
    let work_dir = state.work_dir.clone();
    let task = state.task.clone();
    // Snapshot the files for the chosen mc+cat from the picked index.
    let files = CONTENT_PICK
        .with(|p| {
            p.borrow()
                .as_ref()
                .and_then(|idx| idx.versions.get(&mc))
                .and_then(|m| m.get(&cat))
                .cloned()
        })
        .unwrap_or_default();
    let dest_sub = crate::content::category_folder(&cat).unwrap_or("mods");
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
        let dest = work_dir.join(dest_sub);
        let (ok, failed) = crate::content::download_files(&files, &dest, &*progress);
        let msg = if failed == 0 {
            Task::Done(format!("Downloaded {ok} {cat} to {}", dest.display()))
        } else {
            Task::Error(format!(
                "{ok} ok, {failed} failed downloading {cat}"
            ))
        };
        if let Ok(mut t) = task.lock() {
            *t = msg;
        }
    });
}
