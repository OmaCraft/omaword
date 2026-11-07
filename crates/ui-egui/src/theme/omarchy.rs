//! Transient desktop chrome; document rendering and document font families stay unchanged.
use super::{Color32, FontDefinitions, FontFamily, Tokens};
use crate::omarchy::{Palette, blend};
use omacraft_ui::{Face, FontOptions, SystemFaces};
fn palette_id() -> egui::Id {
    egui::Id::unique("omaword.appearance.palette")
}
fn fonts_id() -> egui::Id {
    egui::Id::unique("omaword.appearance.fonts")
}
fn base_fonts_id() -> egui::Id {
    egui::Id::unique("omaword.appearance.base-fonts")
}

/// Store a UI-only palette. The caller reapplies its effective appearance when this changes.
pub fn set_omarchy_palette(ctx: &egui::Context, palette: Option<Palette>) -> bool {
    if ctx.data(|d| d.get_temp::<Palette>(palette_id())) == palette {
        return false;
    }
    ctx.data_mut(|d| {
        if let Some(p) = palette {
            d.insert_temp(palette_id(), p);
        } else {
            d.remove::<Palette>(palette_id());
        }
    });
    ctx.request_repaint();
    true
}

pub(super) fn resolved(ctx: &egui::Context, dark: bool) -> Tokens {
    let mut t = if dark { Tokens::dark() } else { Tokens::light() };
    let Some(p) = ctx.data(|d| d.get_temp::<Palette>(palette_id())).filter(|p| p.dark == dark) else {
        return t;
    };
    t.title_bar = p.background;
    t.ribbon = p.surface;
    t.tab_strip = p.background;
    t.canvas = p.background;
    t.panel = p.surface;
    t.menu = p.surface;
    t.input = p.background;
    t.text = readable(p.foreground, p.surface, 4.5);
    t.text_dim = readable(p.muted, p.surface, 4.5);
    t.text_disabled = blend(p.surface, t.text, 0.45);
    t.icon = t.text;
    t.group_label = t.text_dim;
    t.accent = readable(p.accent, p.surface, 3.0);
    t.accent_text = readable(p.accent, p.surface, 4.5);
    t.on_accent = readable(p.background, t.accent, 4.5);
    // Backstage's white-on-blue sidebar remains readable under every selected palette.
    t.nav = if contrast(Color32::WHITE, p.accent) >= 4.5 { p.accent } else { p.background };
    if contrast(Color32::WHITE, t.nav) < 4.5 {
        t.nav = Color32::from_rgb(0x28, 0x28, 0x28);
    }
    t.border = blend(p.surface, t.text, 0.22);
    t.border_strong = blend(p.surface, t.text, 0.4);
    t.input_border = t.border;
    t.hover = blend(p.surface, t.text, 0.08);
    t.pressed = blend(p.surface, t.text, 0.14);
    t.checked = p.selection;
    t.ruler = p.surface;
    t.ruler_margin = p.background;
    t.ruler_tick = t.text;
    t.status_bar = p.background;
    // Paper, selection, find marks, handles, guides, equation/crop ink and author colours
    // retain upstream values; these belong to document drawing/interaction.
    t
}
fn contrast(a: Color32, b: Color32) -> f32 {
    let luminance = |c: Color32| {
        let linear = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    };
    let (a, b) = (luminance(a), luminance(b));
    (a.max(b) + 0.05) / (a.min(b) + 0.05)
}
fn readable(color: Color32, bg: Color32, ratio: f32) -> Color32 {
    if contrast(color, bg) >= ratio {
        return color;
    }
    let target = if contrast(Color32::WHITE, bg) > contrast(Color32::BLACK, bg) { Color32::WHITE } else { Color32::BLACK };
    for i in 1..=20 {
        let c = blend(color, target, i as f32 / 20.0);
        if contrast(c, bg) >= ratio {
            return c;
        }
    }
    target
}
/// What the chrome fonts depend on: the "Use system font" setting and the faces the appearance
/// worker found. Document fonts depend on neither.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct ChromeFonts {
    pub options: FontOptions,
    pub system: SystemFaces,
}

/// The key of upstream WordCraft's own Inter copy; `omacraft-ui`'s identical copy replaces it in
/// the chrome families. The document `medium`/`semibold` families keep using it.
const WORD_INTER: &str = "Inter";

/// Install the OmaCraft chrome fonts into `fonts` (`omacraft-ui`): Inter, Inter Tabular, Instrument
/// Sans and the system mono for code, with the interface's existing script faces kept, in their
/// order, after Inter and the system UI stack. Also points the three `omaword-*` chrome aliases at
/// the shared families. The document `medium`/`semibold` families are untouched, and so is
/// everything drawn on the page (that never goes through egui's fonts).
pub(crate) fn chrome_fonts(fonts: &mut FontDefinitions, chrome: &ChromeFonts) {
    let fallbacks: Vec<String> = fonts.families.get(&FontFamily::Proportional).into_iter().flatten().filter(|k| *k != WORD_INTER).cloned().collect();
    fonts.families.insert(FontFamily::Proportional, fallbacks.clone());
    let scripts: Vec<&str> = fallbacks.iter().map(String::as_str).collect();
    omacraft_ui::fonts::apply(fonts, chrome.options, &chrome.system, &scripts);
    let family = |fonts: &FontDefinitions, face: Face| fonts.families.get(&face.family()).cloned().unwrap_or_default();
    let ui = family(fonts, Face::Ui);
    let strong = family(fonts, Face::UiStrong);
    // Inter Medium is only in upstream's copy; with the system font on, medium is the system bold.
    let medium = if chrome.options.use_system_font || !fonts.font_data.contains_key("InterMedium") {
        strong.clone()
    } else {
        std::iter::once("InterMedium".to_string()).chain(ui.iter().cloned()).collect()
    };
    for (alias, chain) in [("omaword-ui", ui), ("omaword-medium", medium), ("omaword-semibold", strong)] {
        fonts.families.insert(FontFamily::Name(alias.into()), chain);
    }
}

pub(crate) fn set_chrome_fonts(ctx: &egui::Context, chrome: ChromeFonts) -> bool {
    if ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default() == chrome {
        return false;
    }
    ctx.data_mut(|d| d.insert_temp(fonts_id(), chrome));
    true
}

/// Reuse the exact current document font set; a chrome refresh must never rerun discovery,
/// clear the bound-family registry, or briefly display workbook fallback faces.
pub(crate) fn refresh_chrome_fonts(ctx: &egui::Context, chrome: ChromeFonts) -> bool {
    if ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default() == chrome {
        return false;
    }
    let Some(base) = ctx.data(|d| d.get_temp::<FontDefinitions>(base_fonts_id())) else { return false };
    set_chrome_fonts(ctx, chrome);
    ctx.set_fonts(with_omarchy_fonts(ctx, base));
    true
}

/// The chrome fonts with the current setting and system faces. Only the UI families change;
/// the document `medium`/`semibold` chains and workbook-specific chains stay byte-for-byte intact.
pub(crate) fn with_omarchy_fonts(ctx: &egui::Context, mut fonts: FontDefinitions) -> FontDefinitions {
    ctx.data_mut(|d| d.insert_temp(base_fonts_id(), fonts.clone()));
    let chrome = ctx.data(|d| d.get_temp::<ChromeFonts>(fonts_id())).unwrap_or_default();
    chrome_fonts(&mut fonts, &chrome);
    fonts
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn palette_changes_only_chrome_and_restores_exact_original() {
        let ctx = egui::Context::default();
        for dark in [false, true] {
            let base = if dark { Tokens::dark() } else { Tokens::light() };
            let palette = Palette {
                dark,
                background: Color32::from_rgb(22, 25, 30),
                foreground: Color32::WHITE,
                accent: Color32::from_rgb(220, 175, 50),
                selection: Color32::GRAY,
                muted: Color32::GRAY,
                surface: Color32::from_rgb(32, 35, 40),
            };
            assert!(set_omarchy_palette(&ctx, Some(palette)));
            let t = resolved(&ctx, dark);
            assert_eq!(t.ribbon, palette.surface);
            assert_eq!(
                (t.selection, t.find_highlight, t.guide, t.equation_box, t.leader_colors),
                (base.selection, base.find_highlight, base.guide, base.equation_box, base.leader_colors)
            );
            assert!(set_omarchy_palette(&ctx, None));
            assert_eq!(resolved(&ctx, dark), base);
        }
    }
    fn chrome(options: FontOptions, system: SystemFaces) -> ChromeFonts {
        ChromeFonts { options, system }
    }

    /// A system UI face and a mono face, both backed by a real font file.
    fn system_faces(base: &FontDefinitions) -> SystemFaces {
        let f = base.font_data.values().next().unwrap();
        let face = |family: &str, path: &str| omacraft_ui::fonts::SystemFace {
            family: family.into(),
            path: path.into(),
            index: f.index,
            data: std::sync::Arc::new(f.bytes().to_vec()),
        };
        SystemFaces {
            ui: vec![face("Adwaita Sans", "/fonts/AdwaitaSans.ttf")],
            ui_bold: vec![],
            mono: Some(face("JetBrains Mono", "/fonts/JBM.ttf")),
        }
    }

    #[test]
    fn chrome_fonts_preserve_every_original_document_family() {
        let ctx = egui::Context::default();
        let base = super::super::font_definitions(false, false);
        let system = system_faces(&base);
        with_omarchy_fonts(&ctx, base.clone());
        for options in [FontOptions::default(), FontOptions { use_system_font: true }] {
            assert!(refresh_chrome_fonts(&ctx, chrome(options, system.clone())));
            let changed = with_omarchy_fonts(&ctx, base.clone());
            for name in ["medium", "semibold"] {
                let family = FontFamily::Name(name.into());
                assert_eq!(changed.families.get(&family), base.families.get(&family), "{name}");
            }
            let ui = &changed.families[&FontFamily::Name("omaword-ui".into())];
            assert_eq!(ui, &changed.families[&FontFamily::Proportional]);
            // Every original interface face is still in the chain, so no script loses its glyphs.
            for key in &base.families[&FontFamily::Proportional] {
                assert!(ui.contains(key), "{key}");
            }
            let first = &ui[0];
            if options.use_system_font {
                assert!(first.contains("AdwaitaSans"), "{first}");
            } else {
                assert_eq!(first, omacraft_ui::fonts::keys::INTER);
            }
            // The system mono is for code only: never in front of any chrome family.
            for alias in ["omaword-ui", "omaword-medium", "omaword-semibold"] {
                assert!(!changed.families[&FontFamily::Name(alias.into())][0].contains("JBM"), "{alias}");
            }
            assert!(changed.families[&FontFamily::Monospace][0].contains("JBM"));
        }
        assert!(refresh_chrome_fonts(&ctx, ChromeFonts::default()));
        assert_eq!(with_omarchy_fonts(&ctx, base.clone()).families, base.families);
    }

    /// The owner's report: on Omarchy every menu was JetBrains Mono. Every chrome text style
    /// (egui's built-in ones, the shared roles and the `omaword-*` aliases) now starts with the
    /// bundled Inter family, even when the desktop has a monospace font; only `Monospace` and
    /// the mono role use it, and Inter text is proportional.
    #[test]
    fn chrome_text_styles_resolve_to_inter_not_mono() {
        use omacraft_ui::Role;
        use omacraft_ui::fonts::keys;
        let ctx = egui::Context::default();
        let base = super::super::font_definitions(false, false);
        set_chrome_fonts(&ctx, chrome(FontOptions::default(), system_faces(&base)));
        let fonts = with_omarchy_fonts(&ctx, base);
        ctx.set_fonts(fonts.clone());
        super::super::apply(&ctx, super::super::Appearance::Light);
        let inter = [keys::INTER, keys::INTER_STRONG, keys::NUMERIC, keys::DISPLAY, "InterMedium"];
        let style = ctx.global_style();
        for (name, font) in style.text_styles.iter().map(|(s, f)| (s.to_string(), f.clone())).chain(
            [super::super::regular(13.0), super::super::medium(13.0), super::super::semibold(13.0), super::super::numeric(9.5)]
                .into_iter()
                .map(|f| (format!("{:?}", f.family), f)),
        ) {
            let first = &fonts.families[&font.family][0];
            if font.family == FontFamily::Monospace {
                assert!(first.contains("JBM"), "{name}: {first}");
            } else {
                assert!(inter.contains(&first.as_str()), "{name} starts with {first}");
            }
        }
        for role in Role::ALL {
            assert_eq!(style.text_styles.get(&role.text_style()), Some(&role.font()), "{role:?}");
        }
        let mut out = ctx.run_ui(egui::RawInput::default(), |ui| {
            let width = |text: &str, font: egui::FontId| ui.fonts_mut(|f| f.layout_no_wrap(text.into(), font, Color32::WHITE).size().x);
            for role in [Role::Menu, Role::Body, Role::Section, Role::DialogTitle, Role::Caption] {
                assert!(width("iiii", role.font()) < 0.6 * width("MMMM", role.font()), "{role:?} is proportional");
            }
        });
        out.textures_delta.clear();
    }
}
