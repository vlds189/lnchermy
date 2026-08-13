// ui/selector.rs - Reusable dropdown selector with an optional per-option
// delete button and an optional search bar. Built on a hand-drawn
// ComboBox-like button + egui::Popup instead of egui::ComboBox: the ComboBox
// wraps the contents in its own ScrollArea, which produced a second scrollbar
// that scrolled the search bar away. Our popup has exactly one ScrollArea вЂ”
// the option list вЂ” so the search bar stays pinned.

use egui::{Align2, Sense, Shape, Stroke, TextWrapMode, TextStyle, Ui, vec2};

/// Items of the selector: `(id, label)`. `id` is the machine value (used by
/// callbacks), `label` is what the user sees (may include a tag, e.g.
/// "1.20.1  [Forge]").
pub type SelectorItem = (String, String);

/// Render a dropdown selector.
///
/// - `id`: stable per-site id salt for egui state.
/// - `items`: options to offer, in display order.
/// - `selected_idx`: current selection (`None` = nothing chosen), updated on
///   user picks; the caller may then map the index back to its own data.
/// - `enabled`: `false` grays out the button and the options (no hover, no
///   popup), e.g. while the game is running.
/// - `on_delete`: optional callback `(index, id)`. If `None`, no рџ—‘ button is
///   shown; if `Some`, every option row gets a trash button that fires the
///   callback without changing the selection.
/// - `on_search`: optional filter callback `(query) -> filtered items`. If
///   `Some`, a search bar is rendered on top of the dropdown; as long as the
///   query is empty all `items` are shown (the filter is NOT called). If
///   `None`, no search bar is rendered. Filtered rows are matched back to the
///   original `items` by id so `selected_idx` / delete indices stay stable.
/// - `search_hint`: placeholder text shown in the search bar while it is
///   empty; `None` falls back to "SearchвЂ¦". Ignored without `on_search`.
/// - `none_text`: text shown on the closed button when `selected_idx` is
///   `None` (e.g. "Install typeвЂ¦"); `None` shows an empty button.
/// - `loading`: when `true`, a spinner + "LoadingвЂ¦" row replaces the options
///   (data is being fetched); any search bar is hidden too.
/// - `loading_error`: optional red caption shown under the spinner while
///   loading (e.g. a failed fetch that will retry).
pub fn selector(
    ui: &mut Ui,
    id: &str,
    items: &[SelectorItem],
    selected_idx: &mut Option<usize>,
    enabled: bool,
    mut on_delete: Option<&mut dyn FnMut(usize, &str)>,
    mut on_search: Option<&mut dyn FnMut(&str) -> Vec<SelectorItem>>,
    search_hint: Option<&str>,
    none_text: Option<&str>,
    loading: bool,
    loading_error: Option<&str>,
) {
    let selected_text = selected_idx
        .and_then(|idx| items.get(idx))
        .map(|(_, label)| label.clone())
        .unwrap_or_else(|| none_text.unwrap_or_default().to_string());

    // The search query lives in egui's data store (keyed by our id salt) so it
    // survives across frames while the popup is open. It clears on close too,
    // since the popup ui is rebuilt fresh every time it opens.
    let query_id = egui::Id::new(id).with("search_query");
    let mut query = ui
        .ctx()
        .data(|d| d.get_temp::<String>(query_id))
        .unwrap_or_default();

    // Auto-focus the search bar on the frame the popup transitions closed →
    // open: memory only reflects the PREVIOUS frame's state, so `was_open`
    // is false exactly on the opening frame.
    let button_id = ui.make_persistent_id(id);
    let popup_id = button_id.with("popup");
    let was_open = egui::Popup::is_id_open(ui.ctx(), popup_id);
    let is_popup_open = was_open;

    // Fixed height for the option list: the search bar stays pinned on top and
    // the list keeps its size when filtering narrows the results (a shrinking
    // + scrolling list looks broken). The popup sizes to exactly
    // searchbar + LIST_H + frame margins, so no foreign scrollbar can exist.
    const LIST_H: f32 = 220.0;

    // ---- ComboBox-lookalike button (bg + selected text + ▼ icon) ----
    let margin = ui.spacing().button_padding;
    let icon_spacing = ui.spacing().icon_spacing;
    let icon_size = vec2(ui.spacing().icon_width, ui.spacing().icon_width);
    let min_width =
        ui.spacing().combo_width - 2.0 * margin.x;
    let wrap_width = ui.available_width() - icon_spacing - icon_size.x;
    let galley = egui::WidgetText::from(selected_text)
        .into_galley(ui, Some(TextWrapMode::Extend), wrap_width, TextStyle::Button);
    let actual_width = (galley.size().x + icon_spacing + icon_size.x).max(min_width);
    let actual_height = galley.size().y.max(icon_size.y);
    // Allocate the full button rect (content + button padding) so the painted
    // background aligns flush with the column edge, like egui's own buttons;
    // the text then sits inset by `margin` (all widths match ComboBox's).
    // `allocate_space` returns (Id, Rect) in egui 0.36.
    let (_, space_rect) = ui.allocate_space(vec2(
        actual_width + 2.0 * margin.x,
        actual_height + 2.0 * margin.y,
    ));
    let inner_rect = space_rect.shrink2(margin);
    let mut outer_rect = space_rect;
    outer_rect.set_height(outer_rect.height().max(ui.spacing().interact_size.y));

    // Sense::hover (not click) while disabled: the response can never be
    // "clicked", so no popup can be toggled; visuals come from `inactive`.
    let response = ui.interact(
        outer_rect,
        button_id,
        if enabled { Sense::click() } else { Sense::hover() },
    );
    let visuals = if enabled {
        if is_popup_open {
            &ui.visuals().widgets.open
        } else {
            ui.style().interact(&response)
        }
    } else {
        &ui.visuals().widgets.inactive
    };

    if ui.is_rect_visible(inner_rect) {
        // Flat background like ComboBox's button frame; painted before the
        // text so the galley stays on top.
        ui.painter()
            .rect(outer_rect, visuals.corner_radius, visuals.bg_fill, visuals.bg_stroke, egui::StrokeKind::Inside);

        let icon_rect = Align2::RIGHT_CENTER.align_size_within_rect(icon_size, inner_rect);
        let icon_rect = egui::Rect::from_center_size(
            icon_rect.center(),
            vec2(icon_rect.width() * 0.7, icon_rect.height() * 0.45),
        );
        // Downward pointing triangle (default ComboBox icon).
        ui.painter().add(Shape::convex_polygon(
            vec![icon_rect.left_top(), icon_rect.right_top(), icon_rect.center_bottom()],
            visuals.fg_stroke.color,
            Stroke::NONE,
        ));

        let text_pos = Align2::LEFT_CENTER.align_size_within_rect(galley.size(), inner_rect);
        ui.painter()
            .galley(text_pos.min, galley, visuals.text_color());
    }

    // ---- Popup: search bar pinned above ONE ScrollArea with the options ----
    if enabled {
        egui::Popup::menu(&response)
            .id(popup_id)
            .width(outer_rect.width())
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_min_width(ui.available_width());
                ui.style_mut().wrap_mode = Some(TextWrapMode::Extend);

                if loading {
                    // Data is being fetched: spinner replaces the whole content.
                    ui.set_min_height(LIST_H);
                    if let Some(err) = loading_error {
                        ui.label(
                            egui::RichText::new(format!("вљ  {err}"))
                                .small()
                                .color(egui::Color32::from_rgb(0xE0, 0x4A, 0x4A)),
                        );
                        ui.add_space(6.0);
                    }
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("LoadingвЂ¦");
                    });
                    ui.ctx().request_repaint();
                    return;
                }

                if on_search.is_some() {
                    let search_resp = ui.add(
                        egui::TextEdit::singleline(&mut query)
                            .hint_text(search_hint.unwrap_or("SearchвЂ¦")),
                    );
                    if !was_open {
                        search_resp.request_focus();
                    }
                    ui.add_space(4.0);
                }

                // Rows to render: all items, or the filter callback's result
                // when the query is non-empty. Indices always point into the
                // original `items` (matched by id), so callbacks see stable
                // indices.
                let rows: Vec<(usize, SelectorItem)> = if !query.trim().is_empty() {
                    match on_search.as_deref_mut() {
                        Some(filter) => filter(query.trim())
                            .into_iter()
                            .filter_map(|(fid, flabel)| {
                                items
                                    .iter()
                                    .position(|(id, _)| *id == fid)
                                    .map(|idx| (idx, (fid, flabel)))
                            })
                            .collect(),
                        None => items.iter().cloned().enumerate().collect(),
                    }
                } else {
                    items.iter().cloned().enumerate().collect()
                };

                if rows.is_empty() {
                    // Keep the fixed height even with zero matches: an
                    // empty content would otherwise collapse the popup,
                    // and clearing the search would bounce it back.
                    egui::ScrollArea::vertical()
                        .id_salt(egui::Id::new(id).with("options_scroll"))
                        .max_height(LIST_H)
                        .min_scrolled_height(LIST_H)
                        .show(ui, |ui| {
                            ui.set_min_height(LIST_H);
                            ui.label(
                                egui::RichText::new("Empty")
                                    .color(egui::Color32::GRAY)
                                    .italics(),
                            );
                        });
                    return;
                }

                // Virtualized rows: only the visible slice is built into
                // widgets, so catalogs with hundreds of versions stay smooth.
                // auto_shrink off keeps the widget exactly LIST_H tall even
                // with few rows, so the popup layout never jumps.
                let row_h = ui.spacing().interact_size.y + ui.spacing().item_spacing.y;
                egui::ScrollArea::vertical()
                    .id_salt(egui::Id::new(id).with("options_scroll"))
                    .max_height(LIST_H)
                    .min_scrolled_height(LIST_H)
                    .auto_shrink(false)
                    .show_rows(ui, row_h, rows.len(), |ui, range| {
                        for idx in range {
                            let (row_idx, (item_id, label)) = &rows[idx];
                            let selected = *selected_idx == Some(*row_idx);
                            ui.horizontal(|ui| {
                                // Grayed out (no hover) while the whole widget
                                // is disabled.
                                let label_resp = if enabled {
                                    ui.add(egui::Button::selectable(selected, label.clone()))
                                } else {
                                    ui.add_enabled(
                                        false,
                                        egui::Button::selectable(selected, label.clone()),
                                    )
                                };
                                if label_resp.clicked() {
                                    *selected_idx = Some(*row_idx);
                                    ui.close();
                                }
                                if let Some(cb) = on_delete.as_deref_mut() {
                                    let del = if enabled {
                                        ui.add(egui::Button::new("рџ—‘"))
                                    } else {
                                        ui.add_enabled(false, egui::Button::new("рџ—‘"))
                                    };
                                    let clicked = del.clicked();
                                    if enabled {
                                        del.on_hover_text("Delete");
                                    } else {
                                        del.on_disabled_hover_text("Close the running game first");
                                    }
                                    if clicked {
                                        cb(*row_idx, item_id);
                                    }
                                }
                            });
                        }
                    });
            });
    }

    // Persist the query for the next frame while the popup is open.
    ui.ctx().data_mut(|d| d.insert_temp(query_id, query));
}
