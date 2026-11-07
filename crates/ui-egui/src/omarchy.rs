//! Read-only Omarchy appearance. Never changes a workbook or executes theme files.

use egui::Color32;

const MAX_PALETTE: usize = 64 * 1024;

/// The small intersection of Omarchy v3 and v4 used by application chrome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    pub dark: bool,
    pub background: Color32,
    pub foreground: Color32,
    pub accent: Color32,
    pub selection: Color32,
    pub muted: Color32,
    pub surface: Color32,
}

impl Palette {
    /// Parse a flat colors.toml. Canonical v4 keys win aliases; v3 selection keys remain valid.
    /// Missing required colours or malformed TOML leaves the current appearance untouched.
    pub fn parse(text: &str, light_marker: bool) -> Option<Self> {
        if text.len() > MAX_PALETTE {
            return None;
        }
        let values: toml::Table = toml::from_str(text).ok()?;
        let color = |keys: &[&str]| keys.iter().find_map(|key| values.get(*key)).and_then(toml::Value::as_str).and_then(parse_color);
        let background = color(&["background", "bg"])?;
        let foreground = color(&["foreground", "fg"])?;
        let dark = match values.get("mode").or_else(|| values.get("theme_type")) {
            Some(toml::Value::String(mode)) if mode == "dark" => true,
            Some(toml::Value::String(mode)) if mode == "light" => false,
            Some(_) => return None,
            None => !light_marker && u16::from(background.r()) + u16::from(background.g()) + u16::from(background.b()) <= 382,
        };
        Some(Self {
            dark,
            background,
            foreground,
            accent: color(&["accent", "blue", "color4"]).unwrap_or(foreground),
            selection: color(&["selection", "selection_background"]).unwrap_or_else(|| blend(background, foreground, 0.18)),
            muted: color(&["muted", "dark_foreground", "dark_fg", "color8"]).unwrap_or_else(|| blend(background, foreground, 0.65)),
            surface: color(&["lighter_background", "lighter_bg"]).unwrap_or_else(|| blend(background, foreground, 0.04)),
        })
    }
}

fn parse_color(s: &str) -> Option<Color32> {
    let s = s.strip_prefix('#')?;
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = u32::from_str_radix(s, 16).ok()?;
    Some(Color32::from_rgb((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

pub(crate) fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    let mix = |x: u8, y: u8| (f32::from(x) * (1.0 - amount) + f32::from(y) * amount).round() as u8;
    Color32::from_rgb(mix(a.r(), b.r()), mix(a.g(), b.g()), mix(a.b(), b.b()))
}

/// Desktop appearance as the worker last saw it. System fonts are found by `omacraft-ui`'s
/// resolver on the worker, never by an egui frame; `None` until the first detection finishes.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Snapshot {
    pub palette: Option<Palette>,
    pub system_fonts: Option<omacraft_ui::SystemFaces>,
}

/// Owns a bounded latest-value slot, so a sleeping/minimized UI cannot accumulate updates.
#[derive(Default)]
pub(crate) struct Watcher {
    enabled: bool,
    last: Snapshot,
    #[cfg(target_os = "linux")]
    worker: Option<native::Worker>,
    /// macOS and Windows have no theme files to poll: one background detection of the system faces.
    #[cfg(not(any(target_os = "linux", target_arch = "wasm32")))]
    detect: Option<std::sync::mpsc::Receiver<omacraft_ui::SystemFaces>>,
}

impl Watcher {
    #[cfg(test)]
    pub(crate) fn fixed(palette: Palette) -> Self {
        Self { last: Snapshot { palette: Some(palette), system_fonts: None }, ..Self::default() }
    }

    pub fn start() -> Self {
        Self { enabled: true, ..Self::default() }
    }

    pub fn current(&mut self, ctx: &egui::Context) -> Snapshot {
        #[cfg(target_os = "linux")]
        if self.enabled {
            if self.worker.is_none() {
                self.worker = native::Worker::start(ctx.clone());
                // Resource exhaustion must not retry spawning once per rendered frame.
                if self.worker.is_none() {
                    self.enabled = false;
                }
            }
            if let Some(worker) = &self.worker
                && let Some(snapshot) = worker.current()
            {
                self.last = snapshot;
            }
        }
        #[cfg(not(any(target_os = "linux", target_arch = "wasm32")))]
        if self.enabled {
            if self.detect.is_none() {
                let (send, receive) = std::sync::mpsc::sync_channel(1);
                let wake = ctx.clone();
                let spawned = std::thread::Builder::new().name("omaword-system-fonts".into()).spawn(move || {
                    let _ = send.send(validated(omacraft_ui::SystemFaces::detect()));
                    wake.request_repaint();
                });
                if spawned.is_ok() {
                    self.detect = Some(receive);
                } else {
                    self.enabled = false;
                }
            }
            if let Some(faces) = self.detect.as_ref().and_then(|r| r.try_recv().ok()) {
                self.last.system_fonts = Some(faces);
                // One detection per run; the receiver stays so no second thread is spawned.
                self.enabled = false;
            }
        }
        #[cfg(target_arch = "wasm32")]
        let _ = (ctx, self.enabled);
        self.last.clone()
    }
}

/// Keep only faces that parse and have a character map: a damaged system font file must never
/// reach egui, which would fail to load it in the middle of a frame.
#[cfg(any(not(target_arch = "wasm32"), test))]
pub(crate) fn validated(mut faces: omacraft_ui::SystemFaces) -> omacraft_ui::SystemFaces {
    use skrifa::MetadataProvider;
    let ok = |f: &omacraft_ui::fonts::SystemFace| skrifa::FontRef::from_index(&f.data, f.index).is_ok_and(|font| font.charmap().has_map());
    faces.ui.retain(ok);
    faces.ui_bold.retain(ok);
    faces.mono = faces.mono.filter(ok);
    faces
}

#[cfg(any(target_os = "linux", test))]
mod files;
#[cfg(target_os = "linux")]
mod native;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn v3_and_v4_palette_keys_and_mode_precedence() {
        let v3 = Palette::parse("background='#111122'\nforeground='#eeeeff'\naccent='#aabbcc'\nselection_background='#334455'", false).unwrap();
        assert!(v3.dark);
        assert_eq!(v3.selection, Color32::from_rgb(0x33, 0x44, 0x55));
        let v4 = Palette::parse("mode='light'\nbg='#ffffff'\nbackground='#101020'\nfg='#222233'\nselection='#abcDEF'", false).unwrap();
        assert!(!v4.dark);
        assert_eq!(v4.background, Color32::from_rgb(0x10, 0x10, 0x20));
        assert!(!Palette::parse("background='#000000'\nforeground='#ffffff'", true).unwrap().dark);
        assert!(Palette::parse("mode='dark'\nbackground='#ffffff'\nforeground='#000000'", true).unwrap().dark);
    }

    /// A damaged system font file is dropped before it can reach egui.
    #[test]
    fn unreadable_system_faces_are_dropped() {
        let face = |data: Vec<u8>| omacraft_ui::fonts::SystemFace {
            family: "Adwaita Sans".into(),
            path: "/fonts/a.ttf".into(),
            index: 0,
            data: std::sync::Arc::new(data),
        };
        let good = face(omacraft_ui::fonts::INTER_REGULAR.to_vec());
        let faces = validated(omacraft_ui::SystemFaces {
            ui: vec![face(b"not a font".to_vec()), good.clone()],
            ui_bold: vec![face(Vec::new())],
            mono: Some(face(vec![0; 64])),
        });
        assert_eq!(faces.ui, vec![good]);
        assert!(faces.ui_bold.is_empty());
        assert!(faces.mono.is_none());
    }

    #[test]
    fn invalid_partial_or_oversized_palette_is_rejected() {
        for bad in [
            "background='#000000'",
            "background='red'\nforeground='#ffffff'",
            "background='#fff'\nforeground='#ffffff'",
            "mode='unknown'\nbackground='#000000'\nforeground='#ffffff'",
            "mode=1\ntheme_type='light'\nbackground='#000000'\nforeground='#ffffff'",
            "background='x'\nbackground='y'",
        ] {
            assert!(Palette::parse(bad, false).is_none(), "{bad}");
        }
        assert!(Palette::parse(&" ".repeat(MAX_PALETTE + 1), false).is_none());
    }
}
