//! The ribbon: tab strip and every tab's groups. Each control runs a command by id.

use egui::{Align2, Rect, Sense, Stroke, Ui, pos2, vec2};
use serde_json::{Value, json};

use crate::colors::{Remember, Target};
use crate::options::MeasureExt;
use crate::theme::{Tokens, bold, regular};
use crate::widgets::{CONTENT_H, big, big_toggle, group, menu_button, small, split};
use crate::{WordApp, icons};

mod arrange;
mod charts;
mod common;
mod design;
mod draw;
mod home;
mod insert;
mod language;
mod layout;
mod mailings;
mod picture;
mod references;
mod review;
mod shape;
mod table;
mod tabs;
mod underline;
mod view;

#[cfg(test)]
use arrange::align_menu;
pub(crate) use arrange::wrap_text_items;
use charts::{chart_design, smart_art_design};
pub(crate) use common::mi;
#[cfg(test)]
pub(crate) use common::size_menu;
use common::{custom_groups, edge_cue};
use design::design;
#[cfg(test)]
use design::style_set_box;
use draw::draw;
use home::home;
#[cfg(test)]
use home::painter_double_click;
use insert::insert;
#[cfg(test)]
use language::language_checks;
#[cfg(test)]
pub(crate) use language::{PROOFING_LANGUAGES, language_name};
pub(crate) use language::{
    caret_language, caret_language_proofed, caret_missing_dictionary, caret_no_proof, caret_unreadable_dictionary, language_menu,
};
use layout::layout;
#[cfg(test)]
use layout::{columns_items, margins_items, orientation_items};
use mailings::mailings;
use picture::picture_format;
use references::{references, zotero};
use review::{developer, help, review};
#[cfg(test)]
pub(crate) use shape::gradient_menu;
use shape::shape_format;
use table::{table_design, table_layout};
#[cfg(test)]
use tabs::MIN_TAB_W;
use tabs::{MORE_TABS_W, reading_toolbar};
pub use tabs::{
    StripPlan, TABS, TabSpec, contextual_tabs, fit_tabs, has_chart_selected, has_picture_selected, has_shape_selected, has_smart_art_selected,
    in_table_public, plan_strip, resolve_tab,
};
#[cfg(test)]
use underline::{UNDERLINE_STYLES, underline_menu};
use view::{outlining, view};

/// Height of the tab strip row (OmaCraft ui-consistency spec); each tab is 30 pt high in it.
const TAB_ROW_H: f32 = 34.0;

/// Height of the ribbon card under the tab strip (OmaGrid's).
const CARD_H: f32 = 88.0;

/// A tab label's font: UI regular 13.5, bold for the active tab (OmaGrid's tab strip).
fn tab_font(active: bool) -> egui::FontId {
    if active { bold(13.5) } else { regular(13.5) }
}

/// Draws the Read toolbar or the full ribbon, switching to and from the contextual Equation and
/// Outlining tabs and showing the available tab strip.
pub fn show(app: &mut WordApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    if app.session.view.read_mode {
        reading_toolbar(app, ui, t);
        return;
    }
    // Editing an equation brings up the Equation tab; leaving it goes back.
    if app.session.math.is_some() {
        if app.equation_prev_tab.is_none() {
            app.equation_prev_tab = Some(app.ui.tab.clone());
            app.ui.tab = "Equation".into();
        }
    } else if let Some(prev) = app.equation_prev_tab.take()
        && app.ui.tab == "Equation"
    {
        app.ui.tab = prev;
    }
    // Outline view brings up the Outlining tab; closing it goes back.
    if app.session.view.mode == wordcraft_layout::ViewMode::Outline {
        if app.outline_prev_tab.is_none() {
            app.outline_prev_tab = Some(app.ui.tab.clone());
            app.ui.tab = "Outlining".into();
        }
    } else if let Some(prev) = app.outline_prev_tab.take()
        && app.ui.tab == "Outlining"
    {
        app.ui.tab = prev;
    }
    // The tabs shown: File, the user's tabs in their order (Customize Ribbon, #379), then the
    // contextual ones (the Outlining tab right after File). A contextual tab (Table, Picture
    // Format) may be stored while the selection moved away, and a tab may have been hidden.
    let user_tabs = app.session.ribbon.tabs().into_owned();
    let mut tabs: Vec<(String, bool)> = vec![("File".to_string(), false)];
    tabs.extend(user_tabs.iter().filter(|e| !e.hidden).map(|e| (e.name.clone(), e.custom)));
    for ct in contextual_tabs(&app.session) {
        if ct == "Outlining" {
            tabs.insert(1.min(tabs.len()), (ct.to_string(), false));
        } else if !tabs.iter().any(|(n, _)| n == ct) {
            tabs.push((ct.to_string(), false));
        }
    }
    {
        let names: Vec<&str> = tabs.iter().map(|(n, _)| n.as_str()).collect();
        let next = resolve_tab(&app.ui.tab, &names);
        let next = if names.contains(&next) { next } else { names.iter().copied().find(|n| *n != "File").unwrap_or(next) };
        if next != app.ui.tab {
            app.ui.tab = next.to_string();
        }
    }
    // Tab strip (OmaCraft ui-consistency spec, OmaGrid's metrics): 30 pt tabs 1 pt below the top
    // of the row, inset 10 + 8 from the window edge. Collapsed it is a 34 pt row; with the ribbon
    // open the card follows 32 pt down, on the same fill and with no line between them.
    let collapsed = app.ui.ribbon_collapsed;
    egui::Panel::top("tabs")
        .exact_size(if collapsed { TAB_ROW_H } else { TAB_ROW_H - 2.0 })
        .show_separator_line(collapsed)
        .frame(egui::Frame::NONE.fill(t.tab_strip).inner_margin(egui::Margin { left: 18, right: 10, top: 1, bottom: if collapsed { 3 } else { 1 } }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing = vec2(2.0, 0.0);
                let labels: Vec<&str> = tabs.iter().map(|(tab, custom)| if *custom { tab.as_str() } else { tl!(tab) }).collect();
                let specs: Vec<TabSpec> = tabs
                    .iter()
                    .zip(&labels)
                    .map(|((tab, custom), shown)| TabSpec {
                        text_w: ui.ctx().fonts_mut(|f| f.layout_no_wrap(shown.to_string(), tab_font(*tab == app.ui.tab), t.text).size().x),
                        file: tab == "File",
                        active: *tab == app.ui.tab,
                        contextual: !custom && wordcraft_engine::ribbon::CONTEXTUAL_TABS.contains(&tab.as_str()),
                    })
                    .collect();
                // Share, Editing and Comments are in the title bar now (ui-consistency spec): the tabs have
                // the whole strip, less its right margin.
                let plan = plan_strip(&specs, [0.0; 3], ui.available_width(), MORE_TABS_W + 2.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        // The tabs that don't fit go into a » menu at the end of the strip (#897, #2026).
                        for (i, ((tab, custom), _)) in tabs.iter().zip(&plan.fits).enumerate().filter(|(_, (_, fits))| **fits) {
                            let (tab, custom) = (tab.as_str(), *custom);
                            let contextual = !custom && wordcraft_engine::ribbon::CONTEXTUAL_TABS.contains(&tab);
                            // A custom tab's name is the user's own text: shown as typed.
                            let shown = if custom { tab } else { tl!(tab) };
                            let w = plan.widths.get(i).copied().unwrap_or(0.0) - 2.0;
                            let cut = specs.get(i).is_some_and(|sp| sp.text_w + plan.pad > w + 0.5);
                            let (r, resp) = ui.allocate_exact_size(vec2(w, 30.0), Sense::click());
                            if app.ui.keytips == crate::keytips::Phase::Tabs {
                                crate::keytips::record_tab(r, tab, &app.ui.keytip_prefix, &mut app.keytip_rects);
                            } else if app.ui.keytips == crate::keytips::Phase::Legacy {
                                // Every tab: they only place the legacy keys' hint (#1697).
                                crate::keytips::record_tab(r, tab, "", &mut app.keytip_rects);
                            }
                            let active = app.ui.tab == tab && !app.ui.backstage;
                            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, active, shown));
                            if resp.hovered() && !active {
                                ui.painter().rect_filled(r.shrink2(vec2(2.0, 4.0)), 5.0, t.hover);
                            }
                            // File and contextual tabs in the accent's text colour, the active tab
                            // bold in `text`, the others regular in `text_dim` (OmaGrid).
                            let color = if tab == "File" || contextual {
                                t.accent_text
                            } else if active {
                                t.text
                            } else {
                                t.text_dim
                            };
                            let font = tab_font(active);
                            if cut {
                                let mut job =
                                    egui::text::LayoutJob::single_section(shown.to_string(), egui::text::TextFormat::simple(font.clone(), color));
                                job.wrap = egui::text::TextWrapping {
                                    max_width: (w - plan.pad).max(1.0),
                                    max_rows: 1,
                                    break_anywhere: true,
                                    overflow_character: Some('…'),
                                };
                                let galley = ui.painter().layout_job(job);
                                ui.painter().galley(r.center() - galley.size() / 2.0, galley, color);
                            } else {
                                ui.painter().text(r.center(), Align2::CENTER_CENTER, shown, font, color);
                            }
                            let resp = if cut { resp.on_hover_text(shown) } else { resp };
                            if active {
                                // OmaGrid's underline: as wide as the label, kept within 20–40 pt.
                                let label_w = specs.get(i).map_or(w - plan.pad, |sp| sp.text_w.min(w - plan.pad));
                                let u = Rect::from_center_size(pos2(r.center().x, r.max.y - 3.0), vec2(label_w.clamp(20.0, 40.0), 3.0));
                                ui.painter().rect_filled(u, 1.5, t.accent);
                            }
                            if resp.clicked() {
                                let _ = app.run("ui.tab", json!({"tab": tab}));
                            }
                            if resp.double_clicked() && tab != "File" {
                                let _ = app.run("ui.collapseRibbon", json!({}));
                            }
                            resp.context_menu(|ui| crate::widgets::customize_menu_items(ui, app));
                        }
                        let left_out: Vec<(&str, &str)> = tabs
                            .iter()
                            .zip(&labels)
                            .zip(&plan.fits)
                            .filter(|(_, fits)| !**fits)
                            .map(|(((tab, _), shown), _)| (tab.as_str(), *shown))
                            .collect();
                        if !left_out.is_empty() {
                            let (r, resp) = ui.allocate_exact_size(vec2(MORE_TABS_W, 30.0), Sense::click());
                            if resp.hovered() {
                                ui.painter().rect_filled(r.shrink2(vec2(2.0, 4.0)), 5.0, t.hover);
                            }
                            // The active tab is in the menu: the button says so.
                            let holds_active = left_out.iter().any(|(tab, _)| *tab == app.ui.tab);
                            ui.painter().text(
                                r.center(),
                                Align2::CENTER_CENTER,
                                "»",
                                tab_font(false),
                                if holds_active { t.accent_text } else { t.text_dim },
                            );
                            // "»" means nothing read aloud: the button says what it holds.
                            let more = tl!("More tabs");
                            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, more));
                            let resp = resp.on_hover_text(more);
                            egui::Popup::menu(&resp).show(|ui| {
                                for (tab, shown) in left_out {
                                    if ui.selectable_label(app.ui.tab == tab, shown).clicked() {
                                        let _ = app.run("ui.tab", json!({"tab": tab}));
                                        ui.close();
                                    }
                                }
                            });
                        }
                    });
                });
            });
        });
    if app.ui.ribbon_collapsed {
        return;
    }
    // OmaGrid's ribbon card (ui-consistency spec, Phase 2): 88 pt high, radius 10, `ribbon` fill
    // and a 1 px border, 10 / 6 inside, on the tab strip's fill with a 10 pt margin each side
    // and 6 below.
    egui::Panel::top("ribbon")
        .exact_size(CARD_H + 6.0)
        .frame(egui::Frame::NONE.fill(t.tab_strip).inner_margin(egui::Margin { left: 10, right: 10, top: 0, bottom: 6 }))
        .show(ui, |ui| {
            let card = Rect::from_min_size(ui.cursor().min, vec2(ui.available_width(), CARD_H));
            ui.painter().rect_filled(card, 10.0, t.ribbon);
            ui.painter().rect_stroke(card, 10.0, Stroke::new(1.0, t.border), egui::StrokeKind::Inside);
            // Inline labels, fields and check boxes in the ribbon body are 12.5 pt (OmaCraft
            // ui-consistency spec), not the 13 pt Body of dialogs and menus.
            for style in [egui::TextStyle::Body, egui::TextStyle::Button] {
                ui.style_mut().text_styles.insert(style, regular(12.5));
            }
            // Right-click on the ribbon (outside a button): Customize Ribbon. Registered first, so
            // the buttons drawn over it keep their own clicks.
            // No document open (#1129): every command is greyed out but Help's.
            if !app.has_document() && app.ui.tab != "Help" {
                ui.disable();
            }
            let bg = ui.interact(card, ui.scope_id().with("ribbon_background"), Sense::click());
            bg.context_menu(|ui| crate::widgets::customize_menu_items(ui, app));
            ui.scope_builder(egui::UiBuilder::new().max_rect(card.shrink2(vec2(10.0, 6.0))), |ui| {
                let entry = user_tabs.iter().find(|e| e.name == app.ui.tab);
                crate::ribbon_fit::begin(ui.ctx(), &app.ui.tab, ui.available_width() - 4.0);
                // 0.37 keeps the scroll offset private: an edge-chevron click leaves a one-frame
                // request here and the next scroll area is built at that offset.
                let pending_id = ui.scope_id().with("ribbon_scroll_pending");
                let pending = ui.data(|d| d.get_temp::<f32>(pending_id));
                let mut area = egui::ScrollArea::horizontal().scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden);
                if let Some(x) = pending {
                    area = area.horizontal_scroll_offset(x);
                }
                let scroll = area.show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                        match entry {
                            // Every tab hidden: nothing to show.
                            Some(e) if e.hidden => return,
                            Some(e) if e.custom => return custom_groups(app, ui, &e.groups),
                            _ => {}
                        }
                        match app.ui.tab.as_str() {
                            "Home" => home(app, ui),
                            "Insert" => insert(app, ui),
                            "Draw" => draw(app, ui),
                            "Design" => design(app, ui),
                            "Layout" => layout(app, ui),
                            "References" => references(app, ui),
                            "Mailings" => mailings(app, ui),
                            "Review" => review(app, ui),
                            "View" => view(app, ui),
                            "Developer" => developer(app, ui),
                            "Zotero" => zotero(app, ui),
                            "Help" => help(app, ui),
                            "Table Design" => table_design(app, ui),
                            "Table Layout" => table_layout(app, ui),
                            "Equation" if app.session.math.is_some() => crate::equation_tab::show(app, ui),
                            "Picture Format" => picture_format(app, ui),
                            "Shape Format" => shape_format(app, ui),
                            "SmartArt Design" => smart_art_design(app, ui),
                            "Outlining" => outlining(app, ui),
                            "Chart Design" => chart_design(app, ui),
                            _ => home(app, ui),
                        }
                        if let Some(e) = entry {
                            custom_groups(app, ui, &e.groups);
                        }
                    });
                });
                if pending.is_some() {
                    let _ = ui.data_mut(|d| d.remove_temp::<f32>(pending_id));
                }
                crate::ribbon_fit::end(ui.ctx());
                // The bar is hidden, so a row wider than the window would be clipped with no sign. Edge chevrons show
                // where more is and scroll it on click. Every command also stays reachable through Search Commands.
                let r = scroll.inner_rect;
                let overflow = scroll.content_size.x - r.width();
                let offset = scroll.state.clamped_offset().x;
                let step = 160.0;
                let mut next = None;
                if offset > 1.0 {
                    let left = Rect::from_min_max(r.left_top(), pos2(r.left() + 18.0, r.bottom()));
                    if edge_cue(ui, left, "‹", tl!("Scroll ribbon left"), &t) {
                        next = Some(offset - step);
                    }
                }
                if overflow - offset > 1.0 {
                    let right = Rect::from_min_max(pos2(r.right() - 18.0, r.top()), r.right_bottom());
                    if edge_cue(ui, right, "›", tl!("Scroll ribbon right"), &t) {
                        next = Some(offset + step);
                    }
                }
                if let Some(x) = next {
                    ui.data_mut(|d| d.insert_temp(pending_id, x.clamp(0.0, overflow.max(0.0))));
                    ui.ctx().request_repaint();
                }
            });
        });
}

#[cfg(test)]
mod tests;
