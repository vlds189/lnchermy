// ui/icons.rs - All UI icons as SVG files (assets/icons/*.svg), embedded at
// compile time and rasterized by the egui_extras SvgLoader (installed once
// per Context in main.rs). The assets are monochrome: WHITE on transparent,
// because egui multiplies the texture with the tint color when painting
// (vertex color * texel), and black texels would stay black under any tint.
// White lets dark/light themes and colored buttons recolor the icon freely.
//
// egui's Button content is text-only and egui's default fonts lack several
// glyphs that were used before (⟳, ✕, ...), so icon buttons are painted by
// hand here — the same technique ui/selector.rs uses for its combo button.

use egui::{
    Color32, Image, ImageSource, Pos2, Rect, Response, Sense, StrokeKind, TextWrapMode, TextStyle,
    Ui, Vec2, Widget,
};

pub const GAMEPAD: ImageSource<'static> = egui::include_image!("../../assets/icons/gamepad.svg");
pub const GEAR: ImageSource<'static> = egui::include_image!("../../assets/icons/gear.svg");
pub const SUN: ImageSource<'static> = egui::include_image!("../../assets/icons/sun.svg");
pub const MOON: ImageSource<'static> = egui::include_image!("../../assets/icons/moon.svg");
pub const RELOAD: ImageSource<'static> = egui::include_image!("../../assets/icons/reload.svg");
pub const CLOSE: ImageSource<'static> = egui::include_image!("../../assets/icons/close.svg");
pub const PLAY: ImageSource<'static> = egui::include_image!("../../assets/icons/play.svg");
pub const DOWNLOAD: ImageSource<'static> = egui::include_image!("../../assets/icons/download.svg");
pub const WARNING: ImageSource<'static> = egui::include_image!("../../assets/icons/warning.svg");
pub const TRASH: ImageSource<'static> = egui::include_image!("../../assets/icons/trash.svg");
pub const LOADING: ImageSource<'static> = egui::include_image!("../../assets/icons/loading.svg");

/// Non-interactive `Image` widget: icon at a fixed display size.
pub fn widget(source: ImageSource<'static>, size: f32) -> Image<'static> {
    Image::new(source).fit_to_exact_size(Vec2::splat(size))
}

/// Same as [`widget`], tinted (recolored) with `color`.
pub fn tinted(source: ImageSource<'static>, size: f32, color: Color32) -> Image<'static> {
    widget(source, size).tint(color)
}

/// Texture of an icon, for painter-based drawing (TextInput's clear cross).
/// The SvgLoader caches per (uri, size), so after the first frame this is
/// just a cache lookup; icons are embedded, so the first load is synchronous.
pub fn texture(ctx: &egui::Context, source: ImageSource<'static>, size: f32) -> Option<egui::TextureId> {
    match Image::new(source).load_for_size(ctx, Vec2::splat(size)) {
        Ok(egui::load::TexturePoll::Ready { texture }) => Some(texture.id),
        _ => None,
    }
}

/// Painter-drawn button: an icon with an optional text label, styled exactly
/// like `egui::Button` (same visuals pipeline as ui/selector.rs's combo
/// button). egui's Button content is text-only, so icon buttons cannot reuse
/// it; painting keeps the pre-SVG glyph-button look, including hover and
/// active feedback and the disabled state.
///
/// - `source` / `icon_size`: the SVG icon.
/// - `min_size`: floor for the button rect (`Vec2::ZERO` for content size).
/// - `text`: optional label right of the icon; `None` centers the icon.
/// - `fill`: optional background color (Launch button states use theme colors).
/// - `enabled`: `false` registers the widget as disabled via `Ui::add_enabled`,
///   so `Response::clicked()` stays false and `on_disabled_hover_text`/the
///   gray visuals behave exactly like on a real `egui::Button`.
pub fn icon_button(
    ui: &mut Ui,
    source: ImageSource<'static>,
    icon_size: f32,
    min_size: Vec2,
    text: Option<&str>,
    fill: Option<Color32>,
    enabled: bool,
) -> Response {
    ui.add_enabled(
        enabled,
        IconButton {
            source,
            icon_size,
            min_size,
            text,
            fill,
        },
    )
}

struct IconButton<'a> {
    source: ImageSource<'static>,
    icon_size: f32,
    min_size: Vec2,
    text: Option<&'a str>,
    fill: Option<Color32>,
}

impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let style = ui.style().clone();
        let margin = style.spacing.button_padding;
        let icon_gap = style.spacing.icon_spacing;

        let galley = self.text.map(|t| {
            egui::WidgetText::from(t).into_galley(
                ui,
                Some(TextWrapMode::Extend),
                ui.available_width(),
                TextStyle::Button,
            )
        });
        let (content_w, content_h) = match galley.as_ref() {
            Some(g) => (self.icon_size + icon_gap + g.size().x, g.size().y.max(self.icon_size)),
            None => (self.icon_size, self.icon_size),
        };
        let size = Vec2::new(
            (content_w + 2.0 * margin.x).max(self.min_size.x),
            (content_h + 2.0 * margin.y)
                .max(self.min_size.y)
                .max(style.spacing.interact_size.y),
        );

        let (rect, mut resp) = ui.allocate_exact_size(size, Sense::click());
        // A disabled widget has `hovered() == false`, so `Style::interact`
        // returns the inactive visuals and the hover cursor never fires.
        let visuals = style.interact(&resp);
        resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

        if ui.is_rect_visible(rect) {
            let full_uv = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0));
            let fg = visuals.fg_stroke.color;
            // Same flat button frame as egui's Button (stroke from the state).
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                self.fill.unwrap_or(visuals.bg_fill),
                visuals.bg_stroke,
                StrokeKind::Inside,
            );
            match galley.as_ref() {
                Some(g) => {
                    // [icon][label], both vertically centered, tinted with the
                    // text color (which brightens on hover like a glyph would).
                    let inner = Rect::from_min_size(rect.min + margin, rect.size() - 2.0 * margin);
                    let icon_rect = Rect::from_center_size(
                        Pos2::new(inner.left() + self.icon_size / 2.0, inner.center().y),
                        Vec2::splat(self.icon_size),
                    );
                    if let Some(tex) = texture(ui.ctx(), self.source, self.icon_size) {
                        ui.painter().image(tex, icon_rect, full_uv, fg);
                    }
                    let text_pos = Pos2::new(icon_rect.right() + icon_gap, inner.center().y - g.size().y / 2.0);
                    ui.painter().galley(text_pos, g.clone(), fg);
                }
                None => {
                    // Icon-only buttons center the icon, like the old glyph buttons.
                    let icon_rect = Rect::from_center_size(rect.center(), Vec2::splat(self.icon_size));
                    if let Some(tex) = texture(ui.ctx(), self.source, self.icon_size) {
                        ui.painter().image(tex, icon_rect, full_uv, fg);
                    }
                }
            }
        }

        resp
    }
}