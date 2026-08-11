// ui/mod.rs - UI module root. Routes between the main view and settings.
pub mod install_view;
pub mod main_view;
pub mod settings_view;

use crate::state::AppState;
use egui::Ui;

/// Render the active screen based on app state.
pub fn render(ui: &mut Ui, state: &mut AppState) {
    if state.show_settings {
        settings_view::render(ui, state);
    } else {
        main_view::render(ui, state);
    }
}
