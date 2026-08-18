// ui/main_view.rs - Main launcher screen: version list, RAM/username, launch button.
use crate::state::{AppState, PendingInstall, Task, APP_VERSION};
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

    // Side bar: icon-only when collapsed, expands to show labels via the ☰
    // toggle button at its top (no more hover-based expansion — the toggle
    // is explicit and the state is stable while the pointer wanders).
    let side_id = egui::Id::new("side_panel_anim");
    // Collapsed width fits the icon button (≈34px) plus an 8px margin on
    // either side, so the buttons don't press against the panel edge.
    // Expanded width must fit the longest label ("Dashboard" needs ~104px
    // of button) plus the panel frame margins.
    let target_w = if state.sidebar_expanded { 126.0 } else { 52.0 };
    let side_w = ui.ctx().animate_value_with_time(side_id, target_w, 0.18);
    if (side_w - target_w).abs() > 0.5 {
        ui.ctx().request_repaint();
    }
    // Buttons keep constant size while the panel animates: icon-only until the
    // panel is nearly full, then the label pops in. Same left edge → no shake.
    // The threshold (120) equals the expanded width minus frame margins minus
    // the widest label, so the label never overflows the panel mid-animation.
    let show_text = side_w > 120.0;

    egui::Panel::left("side_panel")
        .exact_size(side_w)
        .resizable(false)
        .show(ui, |ui| {
        ui.add_space(6.0);
        // Fixed width ≥ widest label ("Dashboard" ≈104px), so all
        // sidebar buttons render exactly the same size.
        const SIDEBAR_BTN_W: f32 = 104.0;
        // Theme icons/hover depend on the current theme (sun = switch
        // to light, moon = switch to dark).
        let theme_icon = match state.settings.theme {
            crate::settings::Theme::Dark => super::icons::SUN,
            crate::settings::Theme::Light => super::icons::MOON,
        };
        let theme_hover = match state.settings.theme {
            crate::settings::Theme::Dark => "Switch to light theme",
            crate::settings::Theme::Light => "Switch to dark theme",
        };

        // ☰ Sidebar toggle above everything: collapses/expands the panel.
        // Only the ICON is the button (hover pill wraps it alone); the
        // "lnchermy" name in expanded mode is a plain label to its left,
        // on the same row. The row is allocated explicitly (fixed height =
        // one button) — a plain horizontal + with_layout produced a
        // full-height sub-region that stacked the label above the icon.
        let mut toggle_clicked = false;
        let toggle_hover = if state.sidebar_expanded {
            "Collapse sidebar"
        } else {
            "Expand sidebar"
        };
        if show_text {
            ui.allocate_ui_with_layout(
                egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                // right_to_left: first added lands at the right edge — the
                // icon button — then the label flows to the left of it.
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    toggle_clicked = super::icons::nav_button(
                        ui,
                        super::icons::SIDEBAR,
                        18.0,
                        egui::Vec2::ZERO,
                        None,
                        false,
                    )
                    .on_hover_text(toggle_hover)
                    .clicked();
                    // The name is PAINTED (same layout_no_wrap +
                    // TextStyle::Button pipeline the nav buttons use for
                    // their labels — pixel-identical font) instead of being
                    // a Label widget: there is no text widget, so it can
                    // never be selected, and no hover/selection visuals.
                    let galley = ui.painter().layout_no_wrap(
                        "lnchermy".to_owned(),
                        egui::TextStyle::Button.resolve(ui.style()),
                        ui.visuals().text_color(),
                    );
                    let (rect, _) =
                        ui.allocate_exact_size(galley.size(), egui::Sense::hover());
                    let pos = egui::pos2(
                        rect.left(),
                        rect.center().y - galley.size().y / 2.0,
                    );
                    ui.painter().galley(pos, galley, ui.visuals().text_color());
                },
            );
        } else {
            toggle_clicked = super::icons::nav_button(
                ui,
                super::icons::SIDEBAR,
                18.0,
                egui::Vec2::ZERO,
                None,
                false,
            )
            .on_hover_text(toggle_hover)
            .clicked();
        }
        if toggle_clicked {
            state.sidebar_expanded = !state.sidebar_expanded;
        }
        ui.add_space(4.0);

        if show_text {
            // Gamepad = Dashboard: leaves settings (if open) and shows the
            // main screen. The version picker is NOT opened here anymore —
            // it has its own reload button next to Launch.
            if super::icons::nav_button(
                ui,
                super::icons::GAMEPAD,
                18.0,
                egui::vec2(SIDEBAR_BTN_W, 0.0),
                Some("Dashboard"),
                !state.show_settings,
            )
            .on_hover_text("Back to the main screen")
            .clicked()
            {
                state.show_settings = false;
                super::settings_view::mark_leaving(ui.ctx());
            }
            ui.add_space(4.0);
            if super::icons::nav_button(
                ui,
                super::icons::GEAR,
                18.0,
                egui::vec2(SIDEBAR_BTN_W, 0.0),
                Some("Settings"),
                state.show_settings,
            )
            .clicked()
            {
                state.show_settings = true;
            }
        } else {
            if super::icons::nav_button(
                ui,
                super::icons::GAMEPAD,
                18.0,
                egui::Vec2::ZERO,
                None,
                !state.show_settings,
            )
            .on_hover_text("Back to the main screen")
            .clicked()
            {
                state.show_settings = false;
                super::settings_view::mark_leaving(ui.ctx());
            }
            ui.add_space(4.0);
            if super::icons::nav_button(
                ui,
                super::icons::GEAR,
                18.0,
                egui::Vec2::ZERO,
                None,
                state.show_settings,
            )
            .clicked()
            {
                state.show_settings = true;
            }
        }

        // Theme toggle pinned to the very bottom of the sidebar. The icon
        // depends on the current theme (sun = switch to light, moon = switch
        // to dark). bottom_up layout: first added = bottommost, so the 6px
        // space sits below the button, mirroring the top margin.
        ui.with_layout(egui::Layout::bottom_up(egui::Align::Min), |ui| {
            ui.add_space(6.0);
            if show_text {
                if super::icons::nav_button(
                    ui,
                    theme_icon,
                    18.0,
                    egui::vec2(104.0, 0.0),
                    Some("Theme"),
                    false,
                )
                .on_hover_text(theme_hover)
                .clicked()
                {
                    state.settings.theme = state.settings.theme.toggle();
                    crate::theme::apply(ui.ctx(), state.settings.theme);
                    let _ = state.save_settings();
                }
            } else {
                if super::icons::nav_button(ui, theme_icon, 18.0, egui::Vec2::ZERO, None, false)
                    .on_hover_text(theme_hover)
                    .clicked()
                {
                    state.settings.theme = state.settings.theme.toggle();
                    crate::theme::apply(ui.ctx(), state.settings.theme);
                    let _ = state.save_settings();
                }
            }
        });
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

    // Main content. Settings is docked into this same central area (window
    // chrome above/below stays) — it swaps out the launch section.
    egui::CentralPanel::default().show(ui, |ui| {
        if state.show_settings {
            super::settings_view::render(ui, state);
        } else {
            launch_options_section(ui, state);
        }
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
    // Drain the manifest produced by the background fetch into the picker
    // (Ok only; an error stays in the slot so the picker can surface it).
    // Was previously drained by the Install section, which now lives in
    // settings — without this the picker would never see the catalog.
    if let Some(Ok(ids)) = MANIFEST.lock().unwrap().take() {
        state.remote_versions = ids;
    }

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

    let (btn_icon, btn_text, btn_bg, enabled) = match &launch_status {
        crate::state::LaunchStatus::Launching => (
            super::icons::LOADING,
            "Launching…".to_owned(),
            Some(Color32::from_rgb(0xD4, 0xA0, 0x17)),
            false,
        ),
        crate::state::LaunchStatus::Running(ver) => {
            if state.launch_btn_hovered {
                // On hover: offer to close the game.
                (super::icons::CLOSE, "Close Game".to_owned(), Some(ERROR), true)
            } else {
                // Always enabled so hover is detected (disabled buttons
                // don't register hover in egui). Click opens a confirm dialog.
                (super::icons::PLAY, format!("Running: {ver}"), Some(ACCENT), true)
            }
        }
        _ if state.pending_install.is_some() => {
            // A version picked in the reload picker that is still missing on
            // disk: the Launch button turns into the installer for it. Takes
            // precedence over any stale launch Error — install is the active
            // intent now.
            let label = state
                .pending_install
                .as_ref()
                .map(PendingInstall::label)
                .unwrap_or_default();
            (
                super::icons::DOWNLOAD,
                format!("Install {label}"),
                Some(INFO),
                !task_busy,
            )
        }
        crate::state::LaunchStatus::Error(_) if !state.launch_btn_hovered => (
            super::icons::WARNING,
            "Error".to_owned(),
            Some(ERROR),
            true,
        ),
        _ => (
            super::icons::PLAY,
            format!("LAUNCH {launch_version_id}"),
            None,
            !no_version && !task_busy,
        ),
    };

    // Placed before the button below: `launch_btn_hovered` must reflect the
    // PREVIOUS frame's hover so the state match above can offer "Close Game".
    let mut reload_clicked = false;
    let (resp, row_response) = {
        let row = ui.horizontal(|ui| {
            let resp = super::icons::icon_button(
                ui,
                btn_icon,
                18.0,
                egui::vec2(200.0, 34.0),
                Some(&btn_text),
                btn_bg,
                enabled,
            );
            state.launch_btn_hovered = resp.hovered();

            ui.add_space(6.0);
            let rresp = super::icons::icon_button(
                ui,
                super::icons::RELOAD,
                20.0,
                egui::vec2(34.0, 34.0),
                None,
                Some(ACCENT.linear_multiply(0.15)),
                // Always enabled: it is the ONLY way to pick a version when
                // none exists yet (no_version disables the launch button).
                true,
            );
            let rresp = rresp.on_hover_text("Change / Install new version");
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
                // the install of the version picked in the reload picker (the
                // kind decides vanilla vs Forge vs OptiFine install).
                match state.pending_install.clone() {
                    Some(PendingInstall::Vanilla(v)) => start_vanilla_download(state, v),
                    Some(PendingInstall::Forge(mc, build)) => {
                        super::install_view::start_forge_install(state, mc, build);
                    }
                    Some(PendingInstall::OptiFine(_mc, build)) => {
                        super::install_view::start_optifine_install(state, build);
                    }
                    None => {}
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

    // Version picker popup, toggled by the reload button. Anchored to the whole
    // launch row so it opens right under the launch button. The open state
    // lives in egui's memory (so it survives repaints); a click on the reload
    // button toggles it, a click anywhere else closes it.
    let picker_id = egui::Id::new("launch_picker");
    let toggle = reload_clicked.then_some(egui::SetOpenCommand::Toggle);
    // Memory reflects the PREVIOUS frame's state, so `was_open` is false
    // exactly on the opening frame — the search bar uses this to autofocus.
    let was_open = egui::Popup::is_id_open(ui.ctx(), picker_id);
    egui::Popup::new(picker_id, ui.ctx().clone(), &row_response, ui.layer_id())
        .kind(egui::PopupKind::Menu)
        .layout(egui::Layout::top_down_justified(egui::Align::Min))
        .gap(0.0)
        .open_memory(toggle)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .width(320.0)
        .show(|ui| {
            picker_popup_content(ui, state, was_open);
        });

    if let Some(p) = &state.pending_install {
        ui.label(
            RichText::new(format!(
                "{} is not installed yet — clicking above installs it.",
                p.label()
            ))
            .small()
            .color(INFO),
        );
    } else if no_version {
        ui.label(
            RichText::new("Pick an installed version to launch.")
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

/// Content of the version picker popup: installed versions first, then
/// catalog versions that are not installed yet. Picking an installed version
/// selects it; picking a missing one arms the "Install <...>" mode on the
/// launch button (stored in `state.pending_install`, kind-aware: vanilla /
/// Forge / OptiFine — all rows share the same selectable look).
fn picker_popup_content(ui: &mut Ui, state: &mut AppState, was_open: bool) {
    /// A collapsible section of the picker. Collapse state lives in egui's
    /// data store (keyed per section) so it survives repaints.
    #[derive(Clone, Copy)]
    enum Section {
        Installed,
        Remote,
    }
    impl Section {
        fn label(self) -> &'static str {
            match self {
                Self::Installed => "Installed",
                Self::Remote => "Not installed",
            }
        }
        fn collapse_id(self) -> egui::Id {
            egui::Id::new("launch_picker_section").with(match self {
                Self::Installed => "installed",
                Self::Remote => "remote",
            })
        }
        fn is_collapsed(self, ctx: &egui::Context) -> bool {
            ctx.data(|d| d.get_temp::<bool>(self.collapse_id()))
                .unwrap_or(false)
        }
    }

    enum Row {
        Header(Section),
        /// `(text, warn)`: `warn` shows a warning icon next to the hint
        /// (catalog errors); plain loading hints have no icon.
        Hint(String, bool),
        Installed(String, String), // label, id
        Remote(String),            // id
        Forge(String, String),     // mc version, build
        OptiFine(String, String),  // mc version, build
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
            ui.horizontal(|ui| {
                ui.add(super::icons::tinted(super::icons::WARNING, 14.0, ERROR));
                ui.label(
                    egui::RichText::new(format!("Manifest error: {e}"))
                        .small()
                        .color(ERROR),
                );
            });
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

    // Search query persists in egui's data store (same pattern as the
    // selector's search bar): it survives repaints while the popup is open,
    // and even a close/reopen (data lives in Memory, not in the popup ui).
    let query_id = egui::Id::new("launch_picker_search");
    let mut query = ui
        .ctx()
        .data(|d| d.get_temp::<String>(query_id))
        .unwrap_or_default();

    // Search bar pinned above the list; autofocus on the opening frame.
    let search_resp = ui.add(
        crate::ui::input::TextInput::new(&mut query).hint_text("Search versions…"),
    );
    if !was_open {
        search_resp.request_focus();
    }
    ui.add_space(4.0);

    let q = query.trim().to_ascii_lowercase();
    let matches_q = |id: &str| q.is_empty() || id.to_ascii_lowercase().contains(&q);

    let mut rows: Vec<Row> = Vec::new();
    let mut installed_matches = false;
    for v in &state.installed_versions {
        if matches_q(v) {
            installed_matches = true;
            break;
        }
    }
    // Section collapse state is read once per frame; a click on a header
    // toggles it in the data store, so the next frame rebuilds without the
    // section's version rows (headers stay visible to expand back).
    let installed_collapsed = Section::Installed.is_collapsed(ui.ctx());
    let remote_collapsed = Section::Remote.is_collapsed(ui.ctx());

    if installed_matches || state.installed_versions.is_empty() {
        rows.push(Row::Header(Section::Installed));
        if !installed_collapsed {
            if state.installed_versions.is_empty() && q.is_empty() {
                // Empty state: gray hint mirrors the old standalone list message.
                rows.push(Row::Hint(
                    "Nothing installed yet — pick a version below".to_string(),
                    false,
                ));
            }
            for v in &state.installed_versions {
                if matches_q(v) {
                    let tag = AppState::version_tag(v);
                    let label = if tag.is_empty() {
                        v.clone()
                    } else {
                        format!("{v}  {tag}")
                    };
                    rows.push(Row::Installed(label, v.clone()));
                }
            }
        }
    }
    // "Not installed" = every installable version: vanilla versions from the
    // manifest that aren't installed yet, plus the Forge / OptiFine mod-loader
    // catalogs (the same background slots the old "Install type…" selector
    // used). Rows are grouped by MC version — vanilla, its Forge and its
    // OptiFine sit next to each other, so the "1.7.10 family" is visible as
    // one block instead of three scattered lists. A catalog that is still
    // loading contributes a hint row at the end. The header is pushed only
    // when the section actually has content — a fully installed set shows
    // no empty section.
    let mut not_installed: Vec<Row> = Vec::new();
    let forge_data = super::install_view::forge_catalog();
    let optifine_data = super::install_view::optifine_catalog();

    // Membership checks run per frame while the popup is open, and the
    // manifest is ~700 entries — HashSet keeps this O(n) instead of O(n²).
    let remote_set: std::collections::HashSet<&str> =
        state.remote_versions.iter().map(String::as_str).collect();
    let installed_set: std::collections::HashSet<&str> =
        state.installed_versions.iter().map(String::as_str).collect();

    // MC version order: vanilla manifest order first (newest first), then
    // any Forge/OptiFine-only versions (e.g. very old ones not in the
    // manifest). Dedup keeps each version listed once.
    let mut mc_order: Vec<String> = Vec::new();
    for v in &state.remote_versions {
        if !mc_order.contains(v) {
            mc_order.push(v.clone());
        }
    }
    if let Some(fd) = &forge_data {
        for mc in &fd.mc_sorted {
            if !mc_order.contains(mc) {
                mc_order.push(mc.clone());
            }
        }
    }
    if let Some(od) = &optifine_data {
        for mc in &od.mc_sorted {
            if !mc_order.contains(mc) {
                mc_order.push(mc.clone());
            }
        }
    }

    for mc in &mc_order {
        // Vanilla row for this MC (only when it is not installed).
        if remote_set.contains(mc.as_str())
            && !installed_set.contains(mc.as_str())
            && matches_q(mc)
        {
            not_installed.push(Row::Remote(mc.clone()));
        }
        // Forge row right under it — skipped if any Forge for this MC is
        // already installed (installing another would just overwrite it).
        if let Some(fd) = &forge_data {
            if let Some(builds) = fd.by_mc.get(mc) {
                if !state
                    .installed_versions
                    .iter()
                    .any(|id| id.starts_with(&format!("{mc}-forge-")))
                {
                    let build = crate::install::forge::default_build(mc, builds, &fd.promos);
                    let label = format!("Forge {mc} ({build})");
                    if matches_q(&label) {
                        not_installed.push(Row::Forge(mc.clone(), build));
                    }
                }
            }
        }
        // OptiFine row right under Forge.
        if let Some(od) = &optifine_data {
            if let Some(builds) = od.by_mc.get(mc) {
                if !state
                    .installed_versions
                    .iter()
                    .any(|id| id.starts_with(&format!("{mc}-OptiFine_")))
                {
                    let build = builds.last().cloned().unwrap_or_default();
                    let label = format!("OptiFine {mc} ({build})");
                    if matches_q(&label) {
                        not_installed.push(Row::OptiFine(mc.clone(), build));
                    }
                }
            }
        }
    }

    // Loading catalogs: hint rows pinned to the end (their versions are not
    // known yet; once loaded they slot into mc_order above).
    if forge_data.is_none() {
        super::install_view::fetch_forge_async();
        let (msg, warn) = match super::install_view::forge_error() {
            Some(e) => (format!("Forge: {e}"), true),
            None => ("Loading Forge catalog…".to_string(), false),
        };
        not_installed.push(Row::Hint(msg, warn));
    }
    if optifine_data.is_none() {
        super::install_view::fetch_optifine_async();
        let (msg, warn) = match super::install_view::optifine_error() {
            Some(e) => (format!("OptiFine: {e}"), true),
            None => ("Loading OptiFine catalog…".to_string(), false),
        };
        not_installed.push(Row::Hint(msg, warn));
    }

    if !not_installed.is_empty() {
        rows.push(Row::Header(Section::Remote));
        if !remote_collapsed {
            rows.append(&mut not_installed);
        }
    }

    // No matches at all: a bare empty scroll area looks like a bug, so show
    // a hint (mirrors the selector's "Empty" row).
    if rows.is_empty() {
        rows.push(Row::Hint("No versions match the search".to_string(), false));
    }

    // Persist the query for the next frame while the popup is open.
    ui.ctx().data_mut(|d| d.insert_temp(query_id, query));

    let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
    egui::ScrollArea::vertical()
        .id_salt(egui::Id::new("launch_picker_scroll"))
        .max_height(260.0)
        .min_scrolled_height(260.0)
        .auto_shrink(false)
        .show_rows(ui, row_h, rows.len(), |ui, range| {
            for idx in range {
                match &rows[idx] {
                    Row::Header(section) => {
                        // Clickable section title: a small painter-drawn
                        // triangle (▼ expanded, ▸ collapsed — egui's default
                        // fonts lack these glyphs) + the label.
                        let collapsed = section.is_collapsed(ui.ctx());
                        let (rect, resp) = ui.allocate_exact_size(
                            egui::vec2(ui.available_width(), row_h - ui.spacing().item_spacing.y),
                            egui::Sense::click(),
                        );
                        if resp.clicked() {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(section.collapse_id(), !collapsed);
                            });
                        }
                        if resp.hovered() {
                            // Custom widget: real Buttons get a pointing hand
                            // automatically, this painted header doesn't.
                            ui.output_mut(|o| {
                                o.cursor_icon = egui::CursorIcon::PointingHand;
                            });
                        }
                        if ui.is_rect_visible(rect) {
                            let text_color = if resp.hovered() {
                                ui.visuals().text_color()
                            } else {
                                egui::Color32::GRAY
                            };
                            let icon_rect = egui::Rect::from_center_size(
                                egui::pos2(rect.left() + 12.0, rect.center().y),
                                egui::vec2(10.0, 8.0),
                            );
                            let tri = if collapsed {
                                // Right-pointing triangle ▸
                                vec![
                                    icon_rect.left_top(),
                                    icon_rect.left_bottom(),
                                    icon_rect.right_center(),
                                ]
                            } else {
                                // Downward-pointing triangle ▼
                                vec![
                                    icon_rect.left_top(),
                                    icon_rect.right_top(),
                                    icon_rect.center_bottom(),
                                ]
                            };
                            ui.painter().add(egui::Shape::convex_polygon(
                                tri,
                                text_color,
                                egui::Stroke::NONE,
                            ));
                            let galley = ui.painter().layout_no_wrap(
                                section.label().to_string(),
                                egui::FontId::proportional(14.0),
                                text_color,
                            );
                            // Vertically center the galley on the row: galley() anchors top-left.
                            let text_pos = egui::pos2(
                                rect.left() + 24.0,
                                rect.center().y - galley.size().y / 2.0,
                            );
                            ui.painter().galley(text_pos, galley, text_color);
                        }
                    }
                    Row::Hint(label, warn) => {
                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            if *warn {
                                ui.add(super::icons::tinted(
                                    super::icons::WARNING,
                                    14.0,
                                    egui::Color32::GRAY,
                                ));
                            }
                            ui.label(
                                egui::RichText::new(label)
                                    .small()
                                    .color(egui::Color32::GRAY)
                                    .italics(),
                            );
                        });
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
                            // old top selector's trash). Disabled while a game
                            // runs.
                            let del = super::icons::icon_button(
                                ui,
                                super::icons::TRASH,
                                15.0,
                                egui::Vec2::ZERO,
                                None,
                                None,
                                !game_running,
                            );
                            let del = if game_running {
                                del.on_disabled_hover_text("Close the running game first")
                            } else {
                                del
                            };
                            if del.clicked() {
                                state.pending_delete = Some(id.clone());
                                ui.close();
                            }
                        });
                    }
                    Row::Remote(id) => {
                        let selected = matches!(
                            state.pending_install,
                            Some(PendingInstall::Vanilla(ref v)) if v == id
                        );
                        let resp = if game_running {
                            ui.add_enabled(false, egui::Button::selectable(selected, id.clone()))
                        } else {
                            ui.add(egui::Button::selectable(selected, id.clone()))
                        };
                        if resp.clicked() {
                            state.pending_install = Some(PendingInstall::Vanilla(id.clone()));
                            ui.close();
                        }
                    }
                    Row::Forge(mc, build) => {
                        let label = format!("Forge {mc}  ({build})");
                        let selected = matches!(
                            state.pending_install,
                            Some(PendingInstall::Forge(ref m, ref b)) if m == mc && b == build
                        );
                        let resp = if game_running {
                            ui.add_enabled(
                                false,
                                egui::Button::selectable(selected, label.clone()),
                            )
                        } else {
                            ui.add(egui::Button::selectable(selected, label.clone()))
                        };
                        if resp.clicked() {
                            state.pending_install =
                                Some(PendingInstall::Forge(mc.clone(), build.clone()));
                            ui.close();
                        }
                    }
                    Row::OptiFine(mc, build) => {
                        let label = format!("OptiFine {mc}  ({build})");
                        let selected = matches!(
                            state.pending_install,
                            Some(PendingInstall::OptiFine(ref m, ref b)) if m == mc && b == build
                        );
                        let resp = if game_running {
                            ui.add_enabled(
                                false,
                                egui::Button::selectable(selected, label.clone()),
                            )
                        } else {
                            ui.add(egui::Button::selectable(selected, label.clone()))
                        };
                        if resp.clicked() {
                            state.pending_install =
                                Some(PendingInstall::OptiFine(mc.clone(), build.clone()));
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
    // Remember what the user launched (restored as the selection on the next
    // start). Saved on the UI thread BEFORE spawning: background-thread
    // writes could race with concurrent settings edits, and a failed spawn
    // only means we remembered a launch attempt — harmless.
    state.settings.last_version = Some(version_id.clone());
    let _ = state.save_settings();

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
