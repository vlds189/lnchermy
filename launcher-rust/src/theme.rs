// theme.rs - egui visual themes for the launcher.
use crate::settings::Theme;
use egui::{Color32, Context};

/// Minecraft-grass-ish accent green used for primary actions / highlights.
pub const ACCENT: Color32 = Color32::from_rgb(0x5A, 0xA8, 0x4A);
/// A muted warning amber.
pub const WARN: Color32 = Color32::from_rgb(0xD4, 0xA0, 0x17);
/// Error red.
pub const ERROR: Color32 = Color32::from_rgb(0xC0, 0x39, 0x2B);
/// Informational blue (e.g. "update available") — clearly not an error.
pub const INFO: Color32 = Color32::from_rgb(0x4A, 0x9E, 0xFF);

/// Apply the selected theme to the egui context.
///
/// egui 0.36 has its own `egui::Theme` (Dark/Light) accessed via `ctx.set_theme`.
/// We map our settings Theme onto it, then override the selection / hyperlink
/// colors with our Minecraft-green accent for a distinctive look.
pub fn apply(ctx: &Context, theme: Theme) {
    // Switch egui's built-in dark/light base.
    let egui_theme = match theme {
        Theme::Dark => egui::Theme::Dark,
        Theme::Light => egui::Theme::Light,
    };
    ctx.set_theme(egui_theme);

    // Override the visuals for the *active* theme so our accent colors stick.
    // set_visuals_of writes to the theme-specific slot so toggling later still works.
    let mut visuals = ctx.style_of(egui_theme).visuals.clone();
    visuals.selection.stroke.color = ACCENT;
    visuals.hyperlink_color = ACCENT;
    visuals.interact_cursor = Some(egui::CursorIcon::PointingHand);
    ctx.set_visuals_of(egui_theme, visuals);

    // Nudge widget style: slightly rounder corners + looser button padding.
    ctx.style_mut_of(egui_theme, |style| {
        let w = &mut style.visuals.widgets;
        let cr = egui::CornerRadius::same(4);
        w.noninteractive.corner_radius = cr;
        w.inactive.corner_radius = cr;
        w.hovered.corner_radius = cr;
        w.active.corner_radius = cr;
        style.spacing.button_padding = egui::vec2(10.0, 4.0);
    });
}
