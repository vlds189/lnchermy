// ui/input.rs - Reusable one-line text input with an inline clear (✖) button.
//
// egui's TextEdit has no "clear" affordance, and every launcher field that
// accepts free text (username, content URL, searches, custom Java version)
// benefits from one. The ✖ is painted INSIDE the field, on the right edge,
// as a raw interact rect — not a sibling Button — so the field width never
// changes when it appears (no layout jump) and no extra row/column is used.

use egui::{Align2, Response, Sense, Ui};

/// Horizontal distance from the field's right edge to the ✖ center.
const CLEAR_PAD: f32 = 10.0;
/// Clickable zone around the ✖ (bigger than the glyph = easier to hit).
const CLEAR_SIZE: f32 = 16.0;

/// Pure visibility rule, split out for tests: the cross shows only when
/// there is something to clear AND the input "family" holds keyboard focus.
///
/// `any_focus` must include the CLEAR button's own focus: pressing the cross
/// transfers focus from the TextEdit to the cross widget (egui focuses any
/// clicked widget). If visibility only tracked the edit, the cross would
/// vanish on mouse-DOWN — before the click completes — and never fire.
fn show_clear_button(buf: &str, any_focus: bool) -> bool {
    !buf.is_empty() && any_focus
}

/// One-line text input with a clear (✖) button at the right edge.
///
/// Mirrors the `egui::TextEdit::singleline` builder API (the subset this
/// project uses). Drop it in anywhere a `TextEdit` went:
///
/// ```ignore
/// let resp = ui.add(TextInput::new(&mut buf).desired_width(180.0));
/// ```
///
/// Returns the inner TextEdit's [`Response`] (NOT the cross's), so callers
/// keep their usual `lost_focus()` / `changed()` handling.
pub struct TextInput<'a> {
    buf: &'a mut String,
    hint: Option<String>,
    desired_width: f32,
    id: Option<egui::Id>,
}

impl<'a> TextInput<'a> {
    pub fn new(buf: &'a mut String) -> Self {
        TextInput {
            buf,
            hint: None,
            desired_width: f32::INFINITY,
            id: None,
        }
    }

    /// Placeholder text while the field is empty (same as TextEdit's).
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Minimum width of the field. Reduced by nothing: the cross is an
    /// overlay, so the width the caller asks for is the width they get.
    pub fn desired_width(mut self, width: f32) -> Self {
        self.desired_width = width;
        self
    }

    /// Pin the widget id (TextEdits auto-id from layout position otherwise).
    /// Useful when the caller juggles focus itself.
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let Self {
            buf,
            hint,
            desired_width,
            id,
        } = self;

        let mut edit = egui::TextEdit::singleline(buf).desired_width(desired_width);
        if let Some(h) = hint {
            edit = edit.hint_text(h);
        }
        if let Some(i) = id {
            edit = edit.id(i);
        }
        // egui 0.36 atomics: `.show()` → TextEditOutput, `.response` is an
        // AtomLayoutResponse (Deref<Response>), whose inner `.response` is
        // the real Response. Existing call sites relied on Deref; we need the
        // concrete Response because this component RETURNS it.
        let edit_resp = edit.show(ui).response.response;

        // The cross id derives from the edit's actual id, so two inputs on
        // the same screen never share cross state.
        let clear_id = edit_resp.id.with("input_clear");
        // Focus check goes through egui memory (not a Response) because the
        // cross widget only EXISTS while visible — see show_clear_button.
        let clear_focused = ui.ctx().memory(|m| m.has_focus(clear_id));
        if !show_clear_button(buf, edit_resp.has_focus() || clear_focused) {
            return edit_resp;
        }

        let rect = egui::Rect::from_center_size(
            egui::pos2(
                edit_resp.rect.right() - CLEAR_PAD,
                edit_resp.rect.center().y,
            ),
            egui::vec2(CLEAR_SIZE, edit_resp.rect.height().min(CLEAR_SIZE)),
        );
        let clear_resp = ui.interact(rect, clear_id, Sense::click());

        // Subtle: paint the hover pill BEFORE the glyph so the ✖ stays on top.
        let hovered = clear_resp.hovered() || clear_focused;
        if hovered {
            ui.painter().rect(
                rect.shrink(1.0),
                egui::CornerRadius::same(4),
                ui.visuals().widgets.hovered.weak_bg_fill,
                egui::Stroke::NONE,
                egui::StrokeKind::Inside,
            );
        }
        let color = if hovered {
            ui.visuals().widgets.hovered.fg_stroke.color
        } else {
            ui.visuals().widgets.inactive.fg_stroke.color
        };
        // ✖ (not ✕): the lighter glyph is missing from egui's default font —
        // same reason the Close Game button uses ✖.
        let galley =
            ui.painter()
                .layout_no_wrap("✖".to_owned(), egui::FontId::proportional(11.0), color);
        let pos = Align2::CENTER_CENTER.align_size_within_rect(galley.size(), rect).min;
        ui.painter().galley(pos, galley, color);

        // on_hover_* consume the Response; chain them and capture the click
        // first.
        let clear_clicked = clear_resp.clicked();
        clear_resp
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .on_hover_text("Clear");

        if clear_clicked {
            buf.clear();
            // Hand focus straight back to the field: the user cleared it to
            // type something else, so keep them in the editing flow.
            edit_resp.request_focus();
        }

        edit_resp
    }
}

impl<'a> egui::Widget for TextInput<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        self.show(ui)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clear_button_visibility_rule() {
        // Nothing typed → never shown, even when focused.
        assert!(!show_clear_button("", true));
        // Typed but not focused → hidden (requirement: field must be focused).
        assert!(!show_clear_button("abc", false));
        // Typed + focused (edit OR the cross itself) → shown.
        assert!(show_clear_button("abc", true));
        // Whitespace-only still counts as "something entered".
        assert!(show_clear_button("  ", true));
    }

    /// Builder must produce the same values a plain TextEdit call would.
    #[test]
    fn builder_defaults() {
        let mut buf = String::new();
        let w = TextInput::new(&mut buf);
        assert_eq!(w.desired_width, f32::INFINITY);
        assert!(w.hint.is_none());
        assert!(w.id.is_none());

        let mut buf = String::new();
        let w = TextInput::new(&mut buf)
            .hint_text("Search…")
            .desired_width(180.0)
            .id(egui::Id::new("x"));
        assert_eq!(w.desired_width, 180.0);
        assert_eq!(w.hint.as_deref(), Some("Search…"));
        assert_eq!(w.id, Some(egui::Id::new("x")));
    }
}
