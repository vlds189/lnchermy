// ui/selector.rs - Reusable ComboBox-based dropdown selector with an
// optional per-option delete button.

use egui::Ui;

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
/// - `enabled`: `false` grays out the options and the trash icon (no hover),
///   e.g. while the game is running.
/// - `on_delete`: optional callback `(index, id)`. If `None`, no 🗑 button is
///   shown; if `Some`, every option row gets a trash button that fires the
///   callback without changing the selection.
/// - `on_search`: optional filter callback `(query) -> filtered items`. If
///   `Some`, a search bar is rendered on top of the dropdown; as long as the
///   query is empty all `items` are shown (the filter is NOT called). If
///   `None`, no search bar is rendered. Filtered rows are matched back to the
///   original `items` by id so `selected_idx` / delete indices stay stable.
/// - `search_hint`: placeholder text shown in the search bar while it is
///   empty; `None` falls back to "Search…". Ignored without `on_search`.
pub fn selector(
    ui: &mut Ui,
    id: &str,
    items: &[SelectorItem],
    selected_idx: &mut Option<usize>,
    enabled: bool,
    mut on_delete: Option<&mut dyn FnMut(usize, &str)>,
    mut on_search: Option<&mut dyn FnMut(&str) -> Vec<SelectorItem>>,
    search_hint: Option<&str>,
) {
    let selected_text = selected_idx
        .and_then(|idx| items.get(idx))
        .map(|(_, label)| label.clone())
        .unwrap_or_default();

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
    let popup_id = ui.make_persistent_id(id).with("popup");
    let was_open = egui::Popup::is_id_open(ui.ctx(), popup_id);

    // Fixed height for the option list: the search bar stays pinned on top and
    // the list keeps its size when filtering narrows the results (a shrinking
    // + scrolling list looks broken). Outer ComboBox cap is slightly taller
    // than searchbar + list so no foreign scrollbar appears.
    const LIST_H: f32 = 220.0;

    egui::ComboBox::from_id_salt(id)
        .selected_text(selected_text)
        .height(LIST_H + 40.0)
        // Default CloseOnClick closes the popup on ANY click, including the
        // search bar; close only on outside clicks and manually on select.
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show_ui(ui, |ui| {
            if on_search.is_some() {
                let search_resp = ui.add(
                    egui::TextEdit::singleline(&mut query)
                        .hint_text(search_hint.unwrap_or("Search…")),
                );
                if !was_open {
                    search_resp.request_focus();
                }
                ui.add_space(4.0);
            }

            // Rows to render: all items, or the filter callback's result when
            // the query is non-empty. Indices always point into the original
            // `items` (matched by id), so callbacks see stable indices.
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

            egui::ScrollArea::vertical()
                .id_salt(egui::Id::new(id).with("options_scroll"))
                .max_height(LIST_H)
                .min_scrolled_height(LIST_H)
                .show(ui, |ui| {
                    if rows.is_empty() {
                        // Keep the fixed height even with zero matches: an
                        // empty content would otherwise collapse the popup,
                        // and clearing the search would bounce it back.
                        ui.set_min_height(LIST_H);
                        ui.label(
                            egui::RichText::new("Empty")
                                .color(egui::Color32::GRAY)
                                .italics(),
                        );
                        return;
                    }
                    for (idx, (item_id, label)) in rows {
                let selected = *selected_idx == Some(idx);
                ui.horizontal(|ui| {
                    // Grayed out (no hover) while the whole widget is disabled.
                    let label_resp = if enabled {
                        ui.add(egui::Button::selectable(selected, label))
                    } else {
                        ui.add_enabled(false, egui::Button::selectable(selected, label))
                    };
                    if label_resp.clicked() {
                        *selected_idx = Some(idx);
                        ui.close();
                    }
                    if let Some(cb) = on_delete.as_deref_mut() {
                        let del = if enabled {
                            ui.add(egui::Button::new("🗑"))
                        } else {
                            ui.add_enabled(false, egui::Button::new("🗑"))
                        };
                        let clicked = del.clicked();
                        if enabled {
                            del.on_hover_text("Delete");
                        } else {
                            del.on_disabled_hover_text("Close the running game first");
                        }
                        if clicked {
                            cb(idx, &item_id);
                        }
                    }
                });
                    }
                });
        });

    // Persist the query for the next frame while the popup is open.
    ui.ctx().data_mut(|d| d.insert_temp(query_id, query));
}