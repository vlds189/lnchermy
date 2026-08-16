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
/// there is something to clear. Visibility deliberately does NOT depend on
/// keyboard focus — pressing the cross moves focus to it (egui focuses any
/// clicked widget), so a focus-gated cross would vanish on mouse-DOWN, a
/// frame before the click completes, and the release would hit nothing.
fn show_clear_button(buf: &str) -> bool {
    !buf.is_empty()
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
        if !show_clear_button(buf) {
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
        let hovered = clear_resp.hovered();
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
        // Nothing typed → never shown.
        assert!(!show_clear_button(""));
        // Typed → shown (no focus requirement: a focus-gated cross would
        // vanish mid-press and the click would never complete).
        assert!(show_clear_button("abc"));
        // Whitespace-only still counts as "something entered".
        assert!(show_clear_button("  "));
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

    /// Full pointer-driven reproduction: focus the edit, hover the cross,
    /// press + release the primary button over it. The buffer must be
    /// cleared and focus handed back to the edit. This test exists because
    /// exactly this flow was reported broken in the real app — egui's
    /// hit-testing and focus transfer are exercised for real via synthetic
    /// RawInput events (egui 0.36 API: `run_ui`, tuple `PointerMoved`).
    #[test]
    fn clear_click_clears_buffer_via_pointer() {
        let ctx = egui::Context::default();
        let mut buf = "hello".to_owned();
        let edit_id = egui::Id::new("t");
        let cross_id = edit_id.with("input_clear");

        let render = |ui: &mut egui::Ui, buf: &mut String, focus_edit: bool| {
            egui::CentralPanel::default().show(ui, |ui| {
                let resp = ui.add(TextInput::new(buf).id(edit_id));
                if focus_edit {
                    resp.request_focus();
                }
            });
        };

        // Off-screen tests have no renderer to submit the texture delta to,
        // so egui's FullOutput would panic on drop — drop it explicitly.
        let mut frame = |input: egui::RawInput, buf: &mut String, focus_edit: bool| {
            let mut out = ctx.run_ui(input, |ui| render(ui, buf, focus_edit));
            out.textures_delta.clear();
        };

        // Frame 1: render once, then focus the edit (focus takes effect for
        // the next frame; request it in the same frame as creation).
        frame(egui::RawInput::default(), &mut buf, true);

        // Locate the cross from the edit's rect (same geometry as show()).
        let edit_rect = ctx.read_response(edit_id).expect("edit response").rect;
        let cross_pos = egui::pos2(edit_rect.right() - CLEAR_PAD, edit_rect.center().y);

        let pointer = |pressed: bool| egui::Event::PointerButton {
            pos: cross_pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::default(),
        };
        let input = |events: Vec<egui::Event>| egui::RawInput {
            events,
            ..Default::default()
        };

        // Frame 2: move the pointer over the cross.
        frame(input(vec![egui::Event::PointerMoved(cross_pos)]), &mut buf, false);
        let cross = ctx.read_response(cross_id);
        assert!(cross.is_some(), "cross must be visible while text non-empty");
        assert!(
            cross.as_ref().is_some_and(|c| c.hovered()),
            "cross must be hovered after pointer move"
        );

        // Frame 3: press the primary button on the cross.
        frame(input(vec![pointer(true)]), &mut buf, false);
        let cross = ctx.read_response(cross_id);
        assert!(
            cross.as_ref().is_some_and(|c| c.is_pointer_button_down_on()),
            "cross must catch the press"
        );

        // Frame 4: release → the click must clear the buffer.
        frame(input(vec![pointer(false)]), &mut buf, false);
        assert!(buf.is_empty(), "buffer must be cleared after the click, got {buf:?}");

        // Frame 5: egui's `request_focus` takes effect from the next frame —
        // the edit must be focused again (and stay clear).
        frame(egui::RawInput::default(), &mut buf, false);
        assert!(buf.is_empty(), "buffer must stay cleared, got {buf:?}");
        let edit = ctx.read_response(edit_id).expect("edit response");
        assert!(edit.has_focus(), "focus must return to the edit after clearing");
    }
}
