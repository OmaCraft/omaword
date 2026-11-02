#[cfg(test)]
use super::TALL_GROUPS;
use super::buttons::{BIG_ICON_Y, BIG_LABEL_Y, CONTENT_H, DROPDOWN, bg, label_line};
use super::{Rect, Response, Sense, Stroke, Tokens, Ui, WordApp, icons, pos2, regular, vec2};

/// Room above and below a group's content in the ribbon card's 72 pt band (OmaGrid's band).
const GROUP_PAD_Y: f32 = 3.0;

/// Height of the band a group and its separator take: the content and its padding.
pub const GROUP_BAND_H: f32 = CONTENT_H + 2.0 * GROUP_PAD_Y;

/// A ribbon group: its content, then OmaGrid's separator on the right. There is no caption row
/// (OmaCraft ui-consistency spec, Phase 2): the group's name is its accessible name, and the
/// dialog launcher, with the name in its tooltip, sits at the foot of the separator.
pub fn group(ui: &mut Ui, title: &str, launcher: Option<&str>, app: &mut WordApp, add: impl FnOnce(&mut Ui, &mut WordApp)) {
    // The English title keys the launcher's id; the shown name is translated.
    group_titled(ui, title, tl!(title), launcher, app, add);
}

/// A ribbon group whose name is used as is (a custom group's name, Customize Ribbon).
pub fn group_titled(ui: &mut Ui, id_title: &str, title: &str, launcher: Option<&str>, app: &mut WordApp, add: impl FnOnce(&mut Ui, &mut WordApp)) {
    let start = ui.cursor().min;
    // #662: the window may be too narrow for the tab; the plan says how this group fits.
    let position = crate::ribbon_fit::position(ui.ctx());
    let level = crate::ribbon_fit::level(ui.ctx(), id_title);
    if level == crate::ribbon_fit::COLLAPSED {
        ui.vertical(|ui| {
            ui.add_space(GROUP_PAD_Y);
            collapsed_group(ui, id_title, position, title, app, add);
        });
    } else {
        // Not folded: a drop-down left open while it was folded doesn't come back by itself.
        ui.data_mut(|d| d.remove::<bool>(collapsed_open_id(ui, id_title, position)));
        let inner = ui.vertical(|ui| {
            ui.add_space(GROUP_PAD_Y);
            crate::ribbon_fit::set_compact(ui.ctx(), level == crate::ribbon_fit::COMPACT);
            let content = ui.allocate_ui_with_layout(vec2(ui.available_width(), CONTENT_H), egui::Layout::left_to_right(egui::Align::Min), |ui| {
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                add(ui, app);
            });
            crate::ribbon_fit::end_compact(ui.ctx());
            #[cfg(test)]
            if content.response.rect.height() > CONTENT_H + 0.5 {
                TALL_GROUPS.with(|g| g.borrow_mut().push(format!("{id_title}: {:.1}", content.response.rect.height())));
            }
            #[cfg(not(test))]
            let _ = content;
        });
        // The group's name for assistive technology; hover only, so its buttons keep their clicks.
        let named = ui.interact(inner.response.rect, ui.scope_id().with(("ribbon_group", id_title, position)), Sense::hover());
        named.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Group, true, title));
    }
    separator(ui, id_title, title, launcher, app);
    crate::ribbon_fit::record(ui.ctx(), id_title, level, ui.cursor().min.x - start.x);
}

/// OmaGrid's group separator (13 wide, a 1 px line inset 6 at top and bottom), with the group's
/// dialog launcher, when it has one, at its foot.
fn separator(ui: &mut Ui, id_title: &str, title: &str, launcher: Option<&str>, app: &mut WordApp) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(13.0, GROUP_BAND_H), Sense::hover());
    let x = r.center().x;
    let Some(cmd) = launcher else {
        ui.painter().line_segment([pos2(x, r.top() + 6.0), pos2(x, r.bottom() - 6.0)], Stroke::new(1.0, t.border));
        return;
    };
    let lr = Rect::from_center_size(pos2(x, r.bottom() - 7.0), vec2(11.0, 11.0));
    ui.painter().line_segment([pos2(x, r.top() + 6.0), pos2(x, lr.top() - 4.0)], Stroke::new(1.0, t.border));
    let resp = ui.interact(lr.expand(1.0), ui.scope_id().with(("launch", id_title)), Sense::click());
    if resp.hovered() {
        ui.painter().rect_filled(lr.expand(2.0), 2.0, t.hover);
    }
    icons::paint(ui.painter(), lr, "launcher", t.text_dim, t.accent);
    let tip = crate::i18n::fmt(tl!("{group} settings"), &[("group", title)]);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, ui.is_enabled(), &tip));
    if resp.on_hover_text(&tip).clicked() {
        let _ = app.run(cmd, serde_json::json!({}));
    }
}

/// Where a collapsed group remembers whether its drop-down is open.
fn collapsed_open_id(ui: &Ui, id_title: &str, position: usize) -> egui::Id {
    ui.scope_id().with(("collapsed_group", id_title, position))
}

/// A group folded into one drop-down button (the window is too narrow, #662): the button, as tall
/// as the group, shows the group's name, and its drop-down holds the whole group as designed (its
/// launcher stays beside the button). The drop-down stays open while it is used (#1285): only a
/// click outside it, or on the button, closes it; clicks in the menus and lists its controls open
/// (font and size lists, Change Case, colours) count as inside.
fn collapsed_group(ui: &mut Ui, id_title: &str, position: usize, title: &str, app: &mut WordApp, add: impl FnOnce(&mut Ui, &mut WordApp)) {
    let t = Tokens::get(ui.ctx());
    let text_w = ui.ctx().fonts_mut(|f| f.layout_no_wrap(title.to_string(), regular(11.5), t.text).size().x);
    let (r, resp) = ui.allocate_exact_size(vec2((text_w + DROPDOWN + 14.0).max(48.0), CONTENT_H), Sense::click());
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let open_id = collapsed_open_id(ui, id_title, position);
    let was_open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
    bg(ui, r, &resp, was_open, &t);
    icons::paint(
        ui.painter(),
        Rect::from_center_size(pos2(r.center().x, r.min.y + BIG_ICON_Y), vec2(32.0, 32.0)),
        group_icon(id_title),
        t.icon,
        t.accent,
    );
    label_line(ui, r.center().x, r.min.y + BIG_LABEL_Y, title, regular(11.5), t.text, true);
    let resp = resp.on_hover_text(title);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let mut open = was_open;
    if resp.clicked() {
        open = !open;
    } else if open && ui.input(|i| i.pointer.any_click()) {
        // Popups (the drop-down, its controls' menus and lists) sit above the ribbon; a click on
        // the window below them is outside.
        let pos = ui.input(|i| i.pointer.interact_pos());
        let layer = pos.and_then(|p| ui.ctx().layer_id_at(p));
        if !layer.is_some_and(|l| matches!(l.order, egui::Order::Foreground | egui::Order::Tooltip)) {
            open = false;
        }
    }
    // Esc closes an open list inside first, then the drop-down.
    let child_open = egui::Popup::is_any_open(ui.ctx());
    let mut shown = open;
    if open {
        // This popup isn't counted by `Popup::is_any_open`, so keep the same Esc from also leaving
        // full screen.
        app.escape_free = false;
        egui::Popup::menu(&resp).open_bool(&mut shown).close_behavior(egui::PopupCloseBehavior::IgnoreClicks).show(|ui| {
            ui.horizontal_top(|ui| {
                ui.set_min_height(CONTENT_H);
                ui.spacing_mut().item_spacing = vec2(1.0, 1.0);
                add(ui, app);
            });
        });
        if !shown && child_open && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            shown = true;
        }
    }
    ui.data_mut(|d| d.insert_temp(open_id, shown));
}

/// The icon a collapsed group's button shows.
fn group_icon(title: &str) -> &'static str {
    match title {
        "Clipboard" => "paste",
        "Font" => "bold",
        "Paragraph" => "alignLeft",
        "Styles" => "styles",
        "Editing" => "find",
        "Voice" => "dictate",
        "Tables" => "table",
        "Links" => "link",
        _ => "more",
    }
}

/// The icon a command's button shows outside its own ribbon group (custom groups, the Quick
/// Access Toolbar): the one its ribbon button uses, which is usually named after the id's last
/// part. Names the icon set doesn't have draw a lettered tile ([`icons::paint`]).
pub fn command_icon(id: &str) -> &str {
    const ICONS: &[(&str, &str)] = &[
        ("arrange.wrap", "wrapText"),
        ("draw.highlighter", "highlight"),
        ("format.border", "charborder"),
        ("format.color", "fontcolor"),
        ("format.growFont", "grow"),
        ("format.shrinkFont", "shrink"),
        ("format.strikethrough", "strike"),
        ("format.highlight", "highlight"),
        ("insert.signatureLine", "signature"),
        ("mailings.envelopes", "envelope"),
        ("mailings.insertField", "mergeField"),
        ("para.ltr", "textLtr"),
        ("para.rtl", "textRtl"),
        ("para.bullets", "bullets"),
        ("para.numbering", "numbering"),
        ("table.insertRowAbove", "insertAbove"),
        ("view.commentsPane", "showComments"),
        ("view.marks", "pilcrow"),
        ("edit.repeat", "redo"),
        ("file.new", "blankPage"),
        ("tools.customizeRibbon", "more"),
    ];
    ICONS.iter().find(|(c, _)| *c == id).map_or_else(|| id.rsplit('.').next().unwrap_or(id), |(_, icon)| *icon)
}

/// Right-click on a command's button: put it on (or take it off) the Quick Access Toolbar, or
/// open Customize Ribbon (#379).
pub fn command_menu(resp: &Response, app: &mut WordApp, id: &str) {
    resp.context_menu(|ui| {
        let known = app.session.registry.get(id).is_some();
        let on_qat = app.session.ribbon.qat().contains(&id);
        if known && !on_qat && ui.button(tl!("Add to Quick Access Toolbar")).clicked() {
            let _ = app.run("tools.customizeRibbon", serde_json::json!({"qatAdd": id}));
            ui.close();
        }
        if on_qat && ui.button(tl!("Remove from Quick Access Toolbar")).clicked() {
            let _ = app.run("tools.customizeRibbon", serde_json::json!({"qatRemove": id}));
            ui.close();
        }
        if known {
            ui.separator();
        }
        customize_menu_items(ui, app);
    });
}

/// "Customize the Ribbon…" and "Customize Quick Access Toolbar…" (context menus), and "Show
/// Quick Access Toolbar" while the toolbar is hidden (#1945).
pub fn customize_menu_items(ui: &mut Ui, app: &mut WordApp) {
    if !app.ui.quick_access_toolbar && ui.button(tl!("Show Quick Access Toolbar")).clicked() {
        let _ = app.run("ui.quickAccessToolbar", serde_json::json!({"value": true}));
        ui.close();
    }
    if ui.button(tl!("Customize the Ribbon…")).clicked() {
        crate::dialogs_ribbon::open(app, crate::dialogs_ribbon::Page::Ribbon);
        ui.close();
    }
    if ui.button(tl!("Customize Quick Access Toolbar…")).clicked() {
        crate::dialogs_ribbon::open(app, crate::dialogs_ribbon::Page::QuickAccess);
        ui.close();
    }
}

/// A row of small buttons (used inside a vertical stack).
pub fn row(ui: &mut Ui, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = vec2(1.0, 0.0);
        add(ui);
    });
}
