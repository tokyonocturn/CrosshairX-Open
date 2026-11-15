//! Settings, preset library and on-disk persistence.
//!
//! Everything lives under `%APPDATA%\CrosshairX-Open`; nothing ever leaves the
//! machine.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::{Crosshair, Point};
use crate::preset;

pub const APP_DIR: &str = "CrosshairX-Open";
pub const SETTINGS_FILE: &str = "settings.json";
pub const PRESETS_FILE: &str = "presets.json";

/// A keyboard shortcut stored as named modifiers plus a key label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkey {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
    /// Key label, e.g. `Z`, `F9`, `Space`.
    pub key: String,
}

impl Default for Hotkey {
    fn default() -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: true,
            win: false,
            key: "Z".to_string(),
        }
    }
}

impl Hotkey {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl".to_string());
        }
        if self.alt {
            parts.push("Alt".to_string());
        }
        if self.shift {
            parts.push("Shift".to_string());
        }
        if self.win {
            parts.push("Win".to_string());
        }
        parts.push(self.key.clone());
        parts.join(" + ")
    }

    /// Whether the chord has at least one modifier (Windows reserves bare keys).
    pub fn has_modifier(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.win
    }
}

/// Modifier set used for the arrow-key position nudges.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MoveShortcut {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub win: bool,
}

impl Default for MoveShortcut {
    fn default() -> Self {
        Self {
            ctrl: false,
            alt: true,
            shift: true,
            win: false,
        }
    }
}

impl MoveShortcut {
    pub fn describe(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("Ctrl");
        }
        if self.alt {
            parts.push("Alt");
        }
        if self.shift {
            parts.push("Shift");
        }
        if self.win {
            parts.push("Win");
        }
        if parts.is_empty() {
            parts.push("(none)");
        }
        format!("{} + Arrow keys", parts.join(" + "))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Global size multiplier applied on top of the preset geometry.
    pub scale: f32,
    /// Offset from the centre of the chosen monitor, in pixels (+x right, +y up).
    pub offset: Point,
    /// Zero-based monitor index used by the overlay.
    pub monitor: usize,
    /// When true the overlay ignores the monitor DPI factor (size is device px).
    pub ignore_dpi: bool,
    /// Global opacity multiplier.
    pub global_opacity: f32,
    /// Master visibility switch (also toggled by the hotkey).
    pub overlay_visible: bool,
    pub toggle_hotkey: Hotkey,
    pub move_hotkey: MoveShortcut,
    pub launch_on_startup: bool,
    /// Name of the preset currently loaded in the editor.
    pub active_preset: String,
    /// Nudge distance for the position hotkeys, in pixels.
    pub move_step: f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: Point::ZERO,
            monitor: 0,
            ignore_dpi: false,
            global_opacity: 1.0,
            overlay_visible: true,
            toggle_hotkey: Hotkey::default(),
            move_hotkey: MoveShortcut::default(),
            launch_on_startup: false,
            active_preset: String::new(),
            move_step: 1.0,
        }
    }
}

impl Settings {
    pub fn sanitize(&mut self) {
        self.scale = crate::model::clamp(self.scale, 0.1, 8.0);
        self.global_opacity = crate::model::clamp(self.global_opacity, 0.05, 1.0);
        self.move_step = crate::model::clamp(self.move_step, 1.0, 50.0);
        self.offset.x = crate::model::clamp(self.offset.x, -8192.0, 8192.0);
        self.offset.y = crate::model::clamp(self.offset.y, -8192.0, 8192.0);
        if self.toggle_hotkey.key.trim().is_empty() {
            self.toggle_hotkey.key = "Z".to_string();
        }
        if !self.toggle_hotkey.has_modifier() {
            self.toggle_hotkey.alt = true;
            self.toggle_hotkey.shift = true;
        }
        if !(self.move_hotkey.ctrl
            || self.move_hotkey.alt
            || self.move_hotkey.shift
            || self.move_hotkey.win)
        {
            self.move_hotkey.alt = true;
            self.move_hotkey.shift = true;
        }
    }
}

/// In-memory application state that is persisted between runs.
#[derive(Clone, Debug)]
pub struct Store {
    pub dir: PathBuf,
    pub settings: Settings,
    pub presets: Vec<Crosshair>,
    /// Non-fatal problem encountered while loading (surfaced in the UI).
    pub warning: Option<String>,
}

impl Store {
    pub fn default_dir() -> PathBuf {
        match std::env::var_os("APPDATA") {
            Some(base) => PathBuf::from(base).join(APP_DIR),
            None => PathBuf::from(".").join(APP_DIR),
        }
    }

    pub fn load() -> Self {
        Self::load_from(Self::default_dir())
    }

    pub fn load_from(dir: impl Into<PathBuf>) -> Self {
        let dir = dir.into();
        let mut warning = None;
        let mut settings = Settings::default();
        let settings_path = dir.join(SETTINGS_FILE);
        if let Ok(text) = std::fs::read_to_string(&settings_path) {
            match serde_json::from_str::<Settings>(&text) {
                Ok(parsed) => settings = parsed,
                Err(e) => warning = Some(format!("settings.json: {e}")),
            }
        }
        settings.sanitize();

        let mut presets: Vec<Crosshair> = Vec::new();
        let presets_path = dir.join(PRESETS_FILE);
        if let Ok(text) = std::fs::read_to_string(&presets_path) {
            match serde_json::from_str::<Vec<Crosshair>>(&text) {
                Ok(mut parsed) => {
                    for crosshair in &mut parsed {
                        crosshair.sanitize();
                    }
                    presets = parsed;
                }
                Err(e) => warning = Some(format!("presets.json: {e}")),
            }
        }
        if presets.is_empty() {
            presets = builtin_presets();
        }

        Self {
            dir,
            settings,
            presets,
            warning,
        }
    }

    pub fn settings_path(&self) -> PathBuf {
        self.dir.join(SETTINGS_FILE)
    }

    pub fn presets_path(&self) -> PathBuf {
        self.dir.join(PRESETS_FILE)
    }

    pub fn save_settings(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        std::fs::write(self.settings_path(), text).map_err(|e| e.to_string())
    }

    pub fn save_presets(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&self.presets).map_err(|e| e.to_string())?;
        std::fs::write(self.presets_path(), text).map_err(|e| e.to_string())
    }

    /// Insert or replace a preset by name, returning its index.
    pub fn upsert_preset(&mut self, crosshair: Crosshair) -> usize {
        if let Some(index) = self
            .presets
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(&crosshair.name))
        {
            self.presets[index] = crosshair;
            index
        } else {
            self.presets.push(crosshair);
            self.presets.len() - 1
        }
    }

    pub fn delete_preset(&mut self, name: &str) -> bool {
        let before = self.presets.len();
        self.presets.retain(|p| !p.name.eq_ignore_ascii_case(name));
        before != self.presets.len()
    }

    pub fn find_preset(&self, name: &str) -> Option<&Crosshair> {
        self.presets.iter().find(|p| p.name == name)
    }

    /// Export one preset to `path`.
    pub fn export_preset(&self, name: &str, path: &Path) -> Result<(), String> {
        let crosshair = self
            .find_preset(name)
            .ok_or_else(|| format!("no preset named {name}"))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, preset::export_json(crosshair))
            .map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Import every preset in a file, returning how many were added.
    pub fn import_file(&mut self, path: &Path) -> Result<usize, String> {
        let imported = preset::import_file(path)?;
        let count = imported.len();
        for crosshair in imported {
            self.upsert_preset(crosshair);
        }
        Ok(count)
    }
}

/// A small set of starter designs so a fresh install is immediately useful.
pub fn builtin_presets() -> Vec<Crosshair> {
    use crate::model::{DotShape, Layer, Line, LineShape, Outline};

    let classic = Crosshair::single("Classic Cross", Layer::classic());

    let mut dot_layer = Layer::dot_only(4.0, "#FF2D2D");
    dot_layer.outline.thickness = 1.0;
    dot_layer.dot.blur = 0.0;
    let dot = Crosshair::single("Precision Dot", dot_layer);

    let mut gap = Layer::cross_with_gap(10.0, 2.0, 6.0);
    gap.dot.opacity = 1.0;
    gap.dot.diameter = 2.0;
    gap.dot.shape = DotShape::Square;
    gap.line.color = "#00FF88".to_string();
    gap.dot.color = "#00FF88".to_string();
    gap.outline.color = "#000000".to_string();
    gap.outline.thickness = 1.0;
    let gap_cross = Crosshair::single("Wide Gap Green", gap);

    let mut tri = Layer::default();
    tri.line.shape = LineShape::Triangle;
    tri.line.length = 7.0;
    tri.line.thickness = 6.0;
    tri.line.offset = 5.0;
    tri.dot.shape = DotShape::Diamond;
    tri.dot.diameter = 3.0;
    tri.dot.opacity = 1.0;
    tri.dot.color = "#FFFFFF".to_string();
    let triangle = Crosshair::single("Triangle Ticks", tri);

    let mut ring = Layer::default();
    ring.line.visible.clear();
    ring.dot.shape = DotShape::Ring;
    ring.dot.diameter = 14.0;
    ring.dot.thickness = 2.0;
    ring.dot.opacity = 1.0;
    ring.dot.color = "#FFFFFF".to_string();
    ring.outline.thickness = 1.0;
    let ring_dot = Crosshair::single("Hollow Ring", ring);

    let mut tiny = Layer::default();
    tiny.line.length = 4.0;
    tiny.line.thickness = 1.0;
    tiny.line.offset = 3.0;
    tiny.line.color = "#FFFFFF".to_string();
    tiny.dot.opacity = 0.0;
    tiny.outline.thickness = 1.0;
    tiny.outline.color = "#000000".to_string();
    tiny.outline.opacity = 0.8;
    let micro = Crosshair::single("Micro Cross", tiny);

    let mut t_style = Layer::default();
    t_style.line = Line {
        length: 9.0,
        thickness: 2.0,
        offset: 5.0,
        rotation: 0.0,
        color: "#00E5FF".to_string(),
        visible: vec![
            crate::model::Direction::Left,
            crate::model::Direction::Right,
            crate::model::Direction::Top,
        ],
        ..Line::default()
    };
    t_style.dot.opacity = 1.0;
    t_style.dot.shape = DotShape::Circle;
    t_style.dot.diameter = 2.0;
    t_style.dot.color = "#00E5FF".to_string();
    t_style.outline = Outline {
        thickness: 1.0,
        opacity: 1.0,
        color: "#001E24".to_string(),
        blur: 0.0,
    };
    let t_cross = Crosshair::single("Cyan T", t_style);

    vec![classic, dot, gap_cross, triangle, ring_dot, micro, t_cross]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "crosshairx-open-test-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        dir
    }

    #[test]
    fn defaults_are_sane() {
        let s = Settings::default();
        assert_eq!(s.toggle_hotkey.describe(), "Alt + Shift + Z");
        assert_eq!(s.move_hotkey.describe(), "Alt + Shift + Arrow keys");
        assert!(s.overlay_visible);
    }

    #[test]
    fn settings_round_trip_through_disk() {
        let dir = temp_dir("settings");
        let mut store = Store::load_from(&dir);
        store.presets = builtin_presets();
        store.settings.scale = 1.75;
        store.settings.offset = Point::new(12.0, -4.0);
        store.settings.monitor = 1;
        store.settings.active_preset = "Classic Cross".to_string();
        store.save_settings().unwrap();
        store.save_presets().unwrap();

        let reloaded = Store::load_from(&dir);
        assert_eq!(reloaded.settings.scale, 1.75);
        assert_eq!(reloaded.settings.offset, Point::new(12.0, -4.0));
        assert_eq!(reloaded.settings.monitor, 1);
        assert_eq!(reloaded.presets.len(), store.presets.len());
        assert!(reloaded.find_preset("Classic Cross").is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_settings_fall_back_with_a_warning() {
        let dir = temp_dir("corrupt");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(SETTINGS_FILE), "{ not json").unwrap();
        let store = Store::load_from(&dir);
        assert_eq!(store.settings.scale, 1.0);
        assert!(store.warning.is_some());
        assert!(
            !store.presets.is_empty(),
            "builtins fill in for a missing library"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn upsert_replaces_case_insensitively() {
        let dir = temp_dir("upsert");
        let mut store = Store::load_from(&dir);
        store.presets.clear();
        let mut a = Crosshair::default();
        a.name = "Mine".into();
        store.upsert_preset(a.clone());
        let mut b = Crosshair::default();
        b.name = "mine".into();
        b.layers[0].line.offset = 9.0;
        let index = store.upsert_preset(b.clone());
        assert_eq!(store.presets.len(), 1);
        assert_eq!(index, 0);
        assert_eq!(store.presets[0].layers[0].line.offset, 9.0);
        assert!(store.delete_preset("MINE"));
        assert!(store.presets.is_empty());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn export_then_import_round_trip() {
        let dir = temp_dir("export");
        let mut store = Store::load_from(&dir);
        store.presets = builtin_presets();
        let path = dir.join("one.json");
        store.export_preset("Triangle Ticks", &path).unwrap();
        let mut other = Store::load_from(temp_dir("export2"));
        other.presets.clear();
        store.presets.clear();
        let count = other.import_file(&path).unwrap();
        assert_eq!(count, 1);
        assert!(other.find_preset("Triangle Ticks").is_some());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn builtins_are_all_drawable() {
        for preset in builtin_presets() {
            let rendered = crate::render::render(&preset, &crate::render::RenderOptions::default())
                .expect("renders");
            assert!(
                rendered.pixmap.data().iter().any(|b| *b > 0),
                "{} rendered empty",
                preset.name
            );
        }
    }
}
