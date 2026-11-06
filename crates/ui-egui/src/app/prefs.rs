use super::{DEFAULT_TAB, MAX_DICTIONARY, UiState, WordApp, desktop_theme, recent_fonts_from, theme};

/// What the operating system says about light and dark. macOS and Windows answer through winit;
/// winit reports nothing on Linux, where the desktop portal does ([`desktop_theme`]). `None` means
/// nobody answered, not a preference.
fn system_theme(ctx: &egui::Context, desktop_dark: Option<bool>) -> Option<egui::Theme> {
    ctx.system_theme().or_else(|| desktop_dark.map(|dark| if dark { egui::Theme::Dark } else { egui::Theme::Light }))
}

impl WordApp {
    /// Preferences to save between runs: the UI state plus the current user name and editing
    /// preferences.
    pub fn prefs(&self) -> UiState {
        let mut ui = self.ui.clone();
        ui.author = Some(self.session.author.clone());
        ui.initials = self.session.initials.clone();
        ui.dictionary = wordcraft_engine::proof::user_dictionary();
        ui.editing = self.session.prefs.clone();
        ui.read_aloud_rate = self.session.read_aloud.rate();
        ui.read_aloud_skip_citations = self.session.read_aloud.skip_citations;
        ui.dark_page = self.session.view.dark_mode;
        ui.nav_pane = self.session.view.nav_pane;
        ui.styles_pane_dock = self.session.view.styles_pane_dock;
        ui.check_spelling = self.session.view.proofing;
        ui.keyboard = self.session.keymap.clone();
        ui.ribbon = self.session.ribbon.clone();
        ui.ink_pens = self.session.view.draw.pens.clone();
        ui
    }

    /// Restore preferences saved by [`WordApp::prefs`].
    pub fn apply_prefs(&mut self, ui: UiState) {
        self.ui = ui;
        self.ui.backstage = false;
        // Word opens on Home every launch, so the tab `ui.json` holds is not restored. Going File
        // and back to the editor within the run still returns to the tab that was showing.
        self.ui.tab = DEFAULT_TAB.into();
        let recent_fonts = recent_fonts_from(self.ui.recent_fonts.iter().map(String::as_str));
        self.ui.recent_fonts = recent_fonts;
        // A `ui.json` from before the theme setting: its dark-mode switch picks Dark or Light.
        if let Some(dark) = self.ui.legacy_dark.take() {
            self.ui.theme = if dark { theme::Appearance::Dark } else { theme::Appearance::Light };
        }
        // A `ui.json` from before #822: its interface size was on top of the display scale, so it
        // becomes absolute on the first frame that knows that scale (`logic`). A chosen `uiScale`
        // (the current key) wins; "auto" can't be told from a missing key, so the old size does.
        self.legacy_interface_size =
            self.ui.legacy_interface_size.take().filter(|_| self.ui.interface_size.is_auto()).and_then(|old| theme::legacy_interface_percent(&old));
        // The session owns the name from here on; `prefs` copies it back when saving.
        // A saved empty name stays empty (#4368); one of only spaces keeps the default.
        if let Some(author) = self.ui.author.take()
            && (author.is_empty() || !author.trim().is_empty())
        {
            self.session.author = author;
        }
        self.session.initials = std::mem::take(&mut self.ui.initials).chars().take(wordcraft_engine::cmd::options::MAX_INITIALS_CHARS).collect();
        for w in std::mem::take(&mut self.ui.dictionary).iter().take(MAX_DICTIONARY) {
            let w = w.trim();
            if !w.is_empty() && w.chars().count() <= wordcraft_engine::cmd::options::MAX_WORD_CHARS {
                wordcraft_engine::proof::add_word(w);
            }
        }
        self.session.prefs = std::mem::take(&mut self.ui.editing);
        // Check spelling as you type follows the saved preference (#1792).
        self.session.view.proofing = self.session.prefs.check_spelling;
        self.session.view.read_page_color = self.session.prefs.read_page_color;
        // The saved dictionaries folder (#1063) is searched from the start.
        if let Some(f) = self.session.prefs.dictionaries_folder.as_deref() {
            wordcraft_engine::cmd::options::use_dictionaries_folder(Some(f));
        }
        self.session.read_aloud.set_rate(self.ui.read_aloud_rate);
        self.session.read_aloud.skip_citations = self.ui.read_aloud_skip_citations;
        self.session.view.dark_mode = self.ui.dark_page;
        self.session.view.nav_pane = self.ui.nav_pane;
        self.session.view.styles_pane_dock = self.ui.styles_pane_dock;
        self.session.view.proofing = self.ui.check_spelling;
        self.session.prefs.check_spelling = self.ui.check_spelling;
        // Keys for commands this version doesn't have are dropped.
        self.session.keymap = std::mem::take(&mut self.ui.keyboard);
        let reg = self.session.registry.clone();
        self.session.keymap.retain_known(&reg);
        // So are ribbon and toolbar commands.
        self.session.ribbon = std::mem::take(&mut self.ui.ribbon);
        self.session.ribbon.retain_known(&reg);
        self.session.view.draw.pens = std::mem::take(&mut self.ui.ink_pens);
    }

    /// Follow the desktop's light/dark choice while the app runs (Linux, where winit reports no
    /// system theme; see [`desktop_theme`]). The desktop app calls this once at start-up; unit
    /// tests and `ui_shot` don't, so what they draw doesn't depend on the host's desktop theme.
    pub fn watch_desktop_theme(&mut self, ctx: &egui::Context) {
        let _ = ctx;
        self.desktop_theme = desktop_theme::DesktopTheme::start();
        self.omarchy = crate::omarchy::Watcher::start();
    }

    /// Cached background state only; an appearance change never changes document fonts/layout.
    pub(super) fn sync_omarchy(&mut self, ctx: &egui::Context) {
        self.omarchy_snapshot = self.omarchy.current(ctx);
        let palette = (self.ui.theme == theme::Appearance::System).then_some(self.omarchy_snapshot.palette).flatten();
        if theme::set_omarchy_palette(ctx, palette) {
            self.applied_theme = None;
        }
    }

    /// Font activation belongs to a real UI pass, never a hidden logic tick carrying stale input.
    /// This pass consumes the one-frame font gate immediately; the next input frame remains live.
    pub(super) fn apply_omarchy_fonts(&mut self, ctx: &egui::Context) {
        let palette = (self.ui.theme == theme::Appearance::System).then_some(self.omarchy_snapshot.palette).flatten();
        let idle = self.fonts_frames >= 2
            && !self.synthetic_pending
            && self.synthetic.is_empty()
            && self.canvas.ime_preedit.is_empty()
            && self.dialog.is_none()
            && !ctx.text_edit_focused()
            && ctx.input(|i| i.events.is_empty() && !i.pointer.any_down());
        if idle && theme::refresh_omarchy_fonts(ctx, palette.and(self.omarchy_snapshot.fonts.clone())) {
            self.fonts_frames = 1;
            ctx.request_repaint();
        }
    }

    /// Whether the interface theme setting currently resolves to dark (`System` asks the OS).
    pub fn ui_is_dark(&self) -> bool {
        if self.ui.theme == theme::Appearance::System
            && let Some(p) = self.omarchy_snapshot.palette
        {
            return p.dark;
        }
        self.ui.theme.is_dark(self.ctx.as_ref().and_then(|ctx| system_theme(ctx, self.desktop_dark)))
    }

    /// Whether animation effects are on: the Animation effects setting, where `System` follows
    /// what the system last reported (on when it reports nothing).
    pub fn animations_on(&self) -> bool {
        self.ui.animations.enabled(self.system_animations)
    }

    /// What the system last said about its animation effects (`None`: it doesn't say).
    pub fn system_animations(&self) -> Option<bool> {
        self.system_animations
    }

    /// Ask the host for the system's animation setting.
    pub(super) fn read_system_animations(&mut self) {
        self.system_animations = self.services.animations_enabled.as_ref().and_then(|ask| ask());
    }

    /// Carry View › Full Screen (`view.fullScreen`, F11), Focus and Full Screen View (#1678) to
    /// the window, and follow the window when the system takes it in or out of full screen (the
    /// macOS title bar button, the window manager): in is Full Screen, out ends all three. Leaving
    /// full screen gives the window back its earlier size and place: the system keeps them, and
    /// [`window_geometry`] never saves a full-screen size (#980).
    pub(crate) fn sync_full_screen(&mut self, ctx: &egui::Context) {
        let v = &self.session.view;
        let want = v.full_screen || v.focus_mode || v.full_screen_view;
        let seen = ctx.input(|i| i.viewport().fullscreen);
        if seen != self.seen_full_screen {
            self.seen_full_screen = seen;
            if let Some(on) = seen
                && on != want
            {
                let v = &mut self.session.view;
                v.full_screen = on;
                if !on {
                    v.focus_mode = false;
                    v.full_screen_view = false;
                }
                self.sent_full_screen = on;
            }
        }
        let v = &self.session.view;
        let want = v.full_screen || v.focus_mode || v.full_screen_view;
        if want != self.sent_full_screen {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(want));
            self.sent_full_screen = want;
        }
    }
}

#[cfg(test)]
mod omarchy_tests {
    use super::*;

    #[test]
    fn manual_modes_ignore_desktop_palette_without_touching_document() {
        let ctx = egui::Context::default();
        let palette = crate::omarchy::Palette::parse("background='#101020'\nforeground='#eeeeff'\nmode='dark'", false).unwrap();
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Preserved text")), Default::default());
        app.omarchy = crate::omarchy::Watcher::fixed(palette);
        let doc = app.session.doc.clone();
        let revision = app.session.rev();
        for appearance in [theme::Appearance::System, theme::Appearance::Light, theme::Appearance::Dark, theme::Appearance::System] {
            app.ui.theme = appearance;
            app.sync_omarchy(&ctx);
            theme::apply(&ctx, if appearance == theme::Appearance::System { theme::Appearance::Dark } else { appearance });
            let tokens = theme::Tokens::get(&ctx);
            if appearance == theme::Appearance::System {
                assert_eq!(tokens.title_bar, palette.background);
            } else {
                assert_eq!(tokens, if appearance == theme::Appearance::Dark { theme::Tokens::dark() } else { theme::Tokens::light() });
            }
            assert_eq!(app.session.doc, doc);
            assert_eq!(app.session.rev(), revision);
            assert!(!app.session.can_undo());
        }
    }

    #[test]
    fn font_refresh_defers_on_real_text_and_never_skips_the_following_input_frame() {
        let ctx = egui::Context::default();
        let mut app = WordApp::new(wordcraft_engine::Session::new(wordcraft_doc::Document::from_text("Seed")), Default::default());
        app.ui.start_screen = false;
        let frame = |app: &mut WordApp, events| {
            let raw = egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))),
                events,
                ..Default::default()
            };
            let mut out = ctx.run_ui(raw, |ui| {
                app.logic(ui.ctx());
                app.ui(ui);
            });
            out.textures_delta.clear();
        };
        for _ in 0..3 {
            frame(&mut app, vec![]);
        }
        assert!(app.fonts_frames >= 2);
        let base = theme::font_definitions(false, false);
        let data = base.font_data.values().next().unwrap();
        let face = crate::omarchy::FontFace { bytes: std::sync::Arc::new(data.bytes().to_vec()), index: data.index };
        assert!(theme::refresh_omarchy_fonts(&ctx, Some((face.clone(), face))));
        // A hidden logic tick must not arm a future blank paint gate with stale input.
        let _ = ctx.run_logic(&egui::RawInput::default(), |ctx| app.logic(ctx));
        assert!(app.fonts_frames >= 2);
        // Removal is pending but the real event owns this frame.
        frame(&mut app, vec![egui::Event::Text("x".into())]);
        assert!(app.fonts_frames >= 2);
        assert!(app.session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains('x'));
        // One idle frame applies the removal and consumes the font gate immediately.
        frame(&mut app, vec![]);
        assert!(app.fonts_frames >= 2);
        frame(&mut app, vec![egui::Event::Text("y".into())]);
        assert!(app.session.doc.plain_text(wordcraft_doc::StoryRef::Body).contains('y'));
    }
}
