// ui/mod.rs - UI module root. Routes between the main view and settings.
pub mod input;
pub mod install_view;
pub mod main_view;
pub mod selector;
pub mod settings_view;

use crate::state::AppState;
use egui::Ui;

/// Render the window. The window chrome (top bar, side panel, status bar)
/// always belongs to main_view; the settings screen is docked INTO the main
/// view's central area (where launch options / install sit) instead of
/// replacing the whole window layout.
pub fn render(ui: &mut Ui, state: &mut AppState) {
    main_view::render(ui, state);
}

/// Show a hand cursor over a modal window's close button.
///
/// egui paints the window title-bar close button via raw `ui.interact`
/// (containers/window.rs), so `Visuals::interact_cursor` never applies to it.
/// After `Window::show(...)` we know the window rect, so we emulate the
/// close-button zone (top-right corner of the title bar) manually.
pub fn window_close_cursor(ctx: &egui::Context, window_rect: egui::Rect) {
    let Some(pointer) = ctx.pointer_interact_pos() else {
        return;
    };
    let close_zone = egui::Rect::from_min_max(
        egui::pos2(window_rect.right() - 34.0, window_rect.top() - 2.0),
        egui::pos2(window_rect.right() + 2.0, window_rect.top() + 34.0),
    );
    if close_zone.contains(pointer) {
        ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
    }
}
