//! The overlay window: a layered, click-through, always-on-top window that is
//! exactly as large as the crosshair it draws.
//!
//! The window lives on its own thread with a plain Win32 message loop so global
//! hotkeys (which are thread-affine) work without a polling timer. The UI pushes
//! a "refresh" message after every edit; nothing runs while the overlay is idle.

use std::path::PathBuf;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use crosshair_core::{render, Crosshair, RenderOptions, Settings, Store};

use crate::win32::*;

/// One display as reported by the OS.
#[derive(Clone, Copy, Debug)]
pub struct DisplayInfo {
    pub index: usize,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub primary: bool,
    pub dpi: u32,
}

impl DisplayInfo {
    pub fn label(&self) -> String {
        format!(
            "Display {} - {}x{} @ {} DPI{}",
            self.index + 1,
            self.width,
            self.height,
            self.dpi,
            if self.primary { " (primary)" } else { "" }
        )
    }

    pub fn scale(&self) -> f32 {
        self.dpi as f32 / 96.0
    }
}

/// Enumerate every monitor in a stable order (left to right, then top to
/// bottom).
pub fn enumerate_displays() -> Vec<DisplayInfo> {
    extern "system" fn callback(monitor: Hmonitor, _dc: Hdc, _rect: *mut Rect, data: isize) -> i32 {
        let displays = unsafe { &mut *(data as *mut Vec<DisplayInfo>) };
        let mut info = MonitorInfo {
            cb_size: std::mem::size_of::<MonitorInfo>() as u32,
            ..Default::default()
        };
        let ok = unsafe { GetMonitorInfoW(monitor, &mut info) };
        if ok != 0 {
            displays.push(DisplayInfo {
                index: 0,
                x: info.rc_monitor.left,
                y: info.rc_monitor.top,
                width: info.rc_monitor.right - info.rc_monitor.left,
                height: info.rc_monitor.bottom - info.rc_monitor.top,
                primary: info.flags & MONITORINFOF_PRIMARY != 0,
                dpi: monitor_dpi(monitor),
            });
        }
        1
    }

    let mut displays: Vec<DisplayInfo> = Vec::new();
    unsafe {
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            callback,
            &mut displays as *mut _ as isize,
        );
    }
    if displays.is_empty() {
        // Extremely unlikely; fall back to the virtual screen metrics.
        let width = unsafe { GetSystemMetrics(0) };
        let height = unsafe { GetSystemMetrics(1) };
        displays.push(DisplayInfo {
            index: 0,
            x: 0,
            y: 0,
            width: width.max(1920),
            height: height.max(1080),
            primary: true,
            dpi: 96,
        });
    }
    displays.sort_by_key(|d| (!d.primary, d.x, d.y));
    for (i, display) in displays.iter_mut().enumerate() {
        display.index = i;
    }
    displays
}

/// Everything the overlay and the editor share.
pub struct AppData {
    pub settings: Settings,
    pub presets: Vec<Crosshair>,
    pub current: Crosshair,
    pub displays: Vec<DisplayInfo>,
    pub data_dir: PathBuf,
    pub hotkey_log: Vec<String>,
}

impl AppData {
    pub fn new(store: &Store) -> Self {
        let settings = store.settings.clone();
        let current = store
            .find_preset(&settings.active_preset)
            .cloned()
            .or_else(|| store.presets.first().cloned())
            .unwrap_or_default();
        Self {
            settings,
            presets: store.presets.clone(),
            current,
            displays: enumerate_displays(),
            data_dir: store.dir.clone(),
            hotkey_log: Vec::new(),
        }
    }

    pub fn save_settings(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&self.settings).map_err(|e| e.to_string())?;
        std::fs::write(self.data_dir.join("settings.json"), text).map_err(|e| e.to_string())
    }

    pub fn save_presets(&self) -> Result<(), String> {
        std::fs::create_dir_all(&self.data_dir).map_err(|e| e.to_string())?;
        let text = serde_json::to_string_pretty(&self.presets).map_err(|e| e.to_string())?;
        std::fs::write(self.data_dir.join("presets.json"), text).map_err(|e| e.to_string())
    }
}

struct Surface {
    dc: Hdc,
    bitmap: Hbitmap,
    width: u32,
    height: u32,
    bits: *mut std::ffi::c_void,
}

// The DIB bits are only touched from the overlay thread.
unsafe impl Send for Surface {}

impl Surface {
    fn new(width: u32, height: u32) -> Option<Self> {
        unsafe {
            let dc = CreateCompatibleDC(0);
            if dc == 0 {
                return None;
            }
            let info = BitmapInfo {
                header: BitmapInfoHeader {
                    bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                    bi_width: width as i32,
                    bi_height: -(height as i32),
                    bi_planes: 1,
                    bi_bit_count: 32,
                    bi_compression: BI_RGB,
                    ..Default::default()
                },
                colors: [0],
            };
            let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
            if bitmap == 0 || bits.is_null() {
                DeleteDC(dc);
                return None;
            }
            SelectObject(dc, bitmap);
            Some(Self {
                dc,
                bitmap,
                width,
                height,
                bits,
            })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}

struct Runtime {
    data: Arc<Mutex<AppData>>,
    wake: Arc<dyn Fn() + Send + Sync>,
    surface: Mutex<Option<Surface>>,
    /// Last rectangle handed to SetWindowPos, so unchanged refreshes skip it.
    placed: Mutex<Option<(i32, i32, i32, i32)>>,
}

static RUNTIME: Mutex<Option<Arc<Runtime>>> = Mutex::new(None);
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
static STATS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Turn on per-refresh timing on the overlay thread (used by `--ui-stats`).
pub fn set_stats(on: bool) {
    STATS.store(on, Ordering::SeqCst);
}

fn stats_on() -> bool {
    STATS.load(Ordering::Relaxed)
}

const WINDOW_CLASS: &str = "CrosshairXOpenOverlay";
const WINDOW_TITLE: &str = "__CROSSHAIRX_OPEN_OVERLAY";

const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_LEFT: i32 = 2;
const HOTKEY_RIGHT: i32 = 3;
const HOTKEY_UP: i32 = 4;
const HOTKEY_DOWN: i32 = 5;

fn runtime() -> Option<Arc<Runtime>> {
    RUNTIME.lock().ok().and_then(|guard| guard.clone())
}

/// Map a key label (`Z`, `F9`, `Space`, ...) to a virtual-key code.
pub fn vk_from_label(label: &str) -> Option<u32> {
    let upper = label.trim().to_ascii_uppercase();
    if upper.len() == 1 {
        let ch = upper.chars().next()?;
        if ch.is_ascii_alphanumeric() {
            return Some(ch as u32);
        }
    }
    if let Some(rest) = upper.strip_prefix('F') {
        if let Ok(n) = rest.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Some(0x70 + n - 1);
            }
        }
    }
    Some(match upper.as_str() {
        "SPACE" => 0x20,
        "TAB" => 0x09,
        "ENTER" | "RETURN" => 0x0D,
        "ESC" | "ESCAPE" => 0x1B,
        "INSERT" => 0x2D,
        "DELETE" | "DEL" => 0x2E,
        "HOME" => 0x24,
        "END" => 0x23,
        "PAGEUP" => 0x21,
        "PAGEDOWN" => 0x22,
        "BACKQUOTE" | "`" => 0xC0,
        "MINUS" | "-" => 0xBD,
        "EQUALS" | "=" => 0xBB,
        "SEMICOLON" | ";" => 0xBA,
        "COMMA" | "," => 0xBC,
        "PERIOD" | "." => 0xBE,
        "SLASH" | "/" => 0xBF,
        "LEFTBRACKET" | "[" => 0xDB,
        "RIGHTBRACKET" | "]" => 0xDD,
        "BACKSLASH" | "\\" => 0xDC,
        "QUOTE" | "'" => 0xDE,
        "NUMPAD0" => 0x60,
        "NUMPAD1" => 0x61,
        "NUMPAD2" => 0x62,
        "NUMPAD3" => 0x63,
        "NUMPAD4" => 0x64,
        "NUMPAD5" => 0x65,
        "NUMPAD6" => 0x66,
        "NUMPAD7" => 0x67,
        "NUMPAD8" => 0x68,
        "NUMPAD9" => 0x69,
        _ => return None,
    })
}

fn hotkey_modifiers(hotkey: &crosshair_core::Hotkey) -> u32 {
    let mut mods = MOD_NOREPEAT;
    if hotkey.ctrl {
        mods |= MOD_CONTROL;
    }
    if hotkey.alt {
        mods |= MOD_ALT;
    }
    if hotkey.shift {
        mods |= MOD_SHIFT;
    }
    if hotkey.win {
        mods |= MOD_WIN;
    }
    mods
}

fn move_modifiers(shortcut: &crosshair_core::MoveShortcut) -> u32 {
    let mut mods = MOD_NOREPEAT;
    if shortcut.ctrl {
        mods |= MOD_CONTROL;
    }
    if shortcut.alt {
        mods |= MOD_ALT;
    }
    if shortcut.shift {
        mods |= MOD_SHIFT;
    }
    if shortcut.win {
        mods |= MOD_WIN;
    }
    mods
}

/// Register (or re-register) every global hotkey, logging the outcome for the
/// settings page.
fn register_hotkeys(hwnd: Hwnd, rt: &Arc<Runtime>) {
    let (toggle, mov, step) = {
        let data = match rt.data.lock() {
            Ok(data) => data,
            Err(_) => return,
        };
        (
            data.settings.toggle_hotkey.clone(),
            data.settings.move_hotkey.clone(),
            data.settings.move_step,
        )
    };
    let mut log = Vec::new();
    unsafe {
        for id in [
            HOTKEY_TOGGLE,
            HOTKEY_LEFT,
            HOTKEY_RIGHT,
            HOTKEY_UP,
            HOTKEY_DOWN,
        ] {
            UnregisterHotKey(hwnd, id);
        }
        let vk = vk_from_label(&toggle.key);
        match vk {
            Some(vk) => {
                let ok = RegisterHotKey(hwnd, HOTKEY_TOGGLE, hotkey_modifiers(&toggle), vk);
                if ok != 0 {
                    log.push(format!("{} toggles the overlay", toggle.describe()));
                } else {
                    log.push(format!(
                        "{} could not be registered (already in use?)",
                        toggle.describe()
                    ));
                }
            }
            None => log.push(format!("Unknown key \"{}\"", toggle.key)),
        }
        let mods = move_modifiers(&mov);
        let arrows = [
            (HOTKEY_LEFT, 0x25, "left"),
            (HOTKEY_RIGHT, 0x27, "right"),
            (HOTKEY_UP, 0x26, "up"),
            (HOTKEY_DOWN, 0x28, "down"),
        ];
        let mut registered = 0;
        for (id, vk, _name) in arrows {
            if RegisterHotKey(hwnd, id, mods, vk) != 0 {
                registered += 1;
            }
        }
        if registered == 4 {
            log.push(format!(
                "{} moves the crosshair {:.0}px per press",
                mov.describe(),
                step
            ));
        } else {
            log.push(format!(
                "{}: only {registered}/4 arrow keys registered",
                mov.describe()
            ));
        }
    }
    if let Ok(mut data) = rt.data.lock() {
        data.hotkey_log = log;
    }
    (rt.wake)();
}

fn handle_hotkey(id: i32) {
    let Some(rt) = runtime() else { return };
    {
        let Ok(mut data) = rt.data.lock() else { return };
        let step = data.settings.move_step.max(1.0);
        match id {
            HOTKEY_TOGGLE => data.settings.overlay_visible = !data.settings.overlay_visible,
            HOTKEY_LEFT => data.settings.offset.x -= step,
            HOTKEY_RIGHT => data.settings.offset.x += step,
            HOTKEY_UP => data.settings.offset.y += step,
            HOTKEY_DOWN => data.settings.offset.y -= step,
            _ => {}
        }
        data.settings.sanitize();
        let _ = data.save_settings();
    }
    (rt.wake)();
    let hwnd = OVERLAY_HWND.load(Ordering::SeqCst);
    if hwnd != 0 {
        unsafe {
            PostMessageW(hwnd, WM_APP_REFRESH, 0, 0);
        }
    }
}

/// Re-render the crosshair and push it to the layered window.
fn refresh(hwnd: Hwnd) {
    let Some(rt) = runtime() else { return };
    let t_start = std::time::Instant::now();
    let (settings, current, displays) = {
        let Ok(data) = rt.data.lock() else { return };
        (
            data.settings.clone(),
            data.current.clone(),
            data.displays.clone(),
        )
    };

    if !settings.overlay_visible {
        unsafe {
            ShowWindow(hwnd, SW_HIDE);
        }
        // Forget where the window was, so turning the overlay back on re-places it.
        if let Ok(mut slot) = rt.placed.lock() {
            *slot = None;
        }
        return;
    }

    let display = displays
        .get(settings.monitor)
        .or_else(|| displays.first())
        .copied();
    let Some(display) = display else { return };

    let dpi_scale = if settings.ignore_dpi {
        1.0
    } else {
        display.scale()
    };
    let options = RenderOptions {
        scale: settings.scale * dpi_scale,
        opacity: settings.global_opacity,
        padding: 2.0,
    };
    let t_render_start = std::time::Instant::now();
    let Some(rendered) = render(&current, &options) else {
        return;
    };
    let render_us = t_render_start.elapsed().as_micros();

    // Model space has +Y up; the screen has +Y down.
    let target_x = display.x + display.width / 2 + settings.offset.x.round() as i32;
    let target_y = display.y + display.height / 2 - settings.offset.y.round() as i32;
    let left = target_x - rendered.center_x.round() as i32;
    let top = target_y - rendered.center_y.round() as i32;

    let (width, height) = (rendered.width(), rendered.height());
    let mut guard = match rt.surface.lock() {
        Ok(guard) => guard,
        Err(_) => return,
    };
    let needs_new = guard
        .as_ref()
        .map(|s| s.width != width || s.height != height)
        .unwrap_or(true);
    if needs_new {
        *guard = Surface::new(width, height);
    }
    let Some(surface) = guard.as_ref() else {
        return;
    };

    // tiny-skia hands us premultiplied RGBA; layered windows want premultiplied BGRA.
    let source = rendered.pixmap.data();
    unsafe {
        let target =
            std::slice::from_raw_parts_mut(surface.bits as *mut u8, (width * height * 4) as usize);
        for i in (0..source.len()).step_by(4) {
            target[i] = source[i + 2];
            target[i + 1] = source[i + 1];
            target[i + 2] = source[i];
            target[i + 3] = source[i + 3];
        }
        let screen = GetDC(0);
        let size = Size {
            cx: width as i32,
            cy: height as i32,
        };
        let destination = Point { x: left, y: top };
        let origin = Point { x: 0, y: 0 };
        let blend = BlendFunction {
            blend_op: AC_SRC_OVER,
            blend_flags: 0,
            source_constant_alpha: 255,
            alpha_format: AC_SRC_ALPHA,
        };
        UpdateLayeredWindow(
            hwnd,
            screen,
            &destination,
            &size,
            surface.dc,
            &origin,
            0,
            &blend,
            ULW_ALPHA,
        );
        ReleaseDC(0, screen);
        let ulw_us = t_render_start.elapsed().as_micros() - render_us;
        let t_swp = std::time::Instant::now();
        // UpdateLayeredWindow already moved and sized the window, and WS_EX_TOPMOST was
        // set at creation, so SetWindowPos is only needed to show it or to place it when
        // the crosshair's own rectangle changed. Calling it on every refresh costs more
        // than the render and the blit together.
        let geometry = (left, top, width as i32, height as i32);
        let needs_placement = match rt.placed.lock() {
            Ok(mut slot) => {
                let changed = *slot != Some(geometry);
                if changed {
                    *slot = Some(geometry);
                }
                changed
            }
            Err(_) => true,
        };
        if needs_placement {
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                left,
                top,
                width as i32,
                height as i32,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
        }
        if stats_on() {
            println!(
                "overlay-ms render={:.3} blit={:.3} swp={:.3} total={:.3}",
                render_us as f64 / 1000.0,
                ulw_us as f64 / 1000.0,
                t_swp.elapsed().as_micros() as f64 / 1000.0,
                t_start.elapsed().as_micros() as f64 / 1000.0
            );
        }
    }
}

extern "system" fn wndproc(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Lresult {
    match msg {
        WM_HOTKEY => {
            handle_hotkey(wparam as i32);
            0
        }
        WM_APP_REFRESH => {
            refresh(hwnd);
            0
        }
        WM_APP_QUIT => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DISPLAYCHANGE => {
            if let Some(rt) = runtime() {
                let fresh = enumerate_displays();
                if let Ok(mut data) = rt.data.lock() {
                    data.displays = fresh;
                }
            }
            refresh(hwnd);
            0
        }
        WM_CLOSE => {
            unsafe {
                DestroyWindow(hwnd);
            }
            0
        }
        WM_DESTROY => {
            unsafe {
                PostQuitMessage(0);
            }
            0
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn create_window() -> Option<Hwnd> {
    unsafe {
        let instance = GetModuleHandleW(std::ptr::null());
        let class_name = wide(WINDOW_CLASS);
        let class = WndClassExW {
            cb_size: std::mem::size_of::<WndClassExW>() as u32,
            style: 0,
            wnd_proc: Some(wndproc),
            cls_extra: 0,
            wnd_extra: 0,
            instance,
            icon: 0,
            cursor: 0,
            background: 0,
            menu_name: std::ptr::null(),
            class_name: class_name.as_ptr(),
            icon_small: 0,
        };
        if RegisterClassExW(&class) == 0 {
            return None;
        }
        let title = wide(WINDOW_TITLE);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            title.as_ptr(),
            WS_POPUP,
            0,
            0,
            64,
            64,
            0,
            0,
            instance,
            std::ptr::null_mut(),
        );
        if hwnd == 0 {
            None
        } else {
            Some(hwnd)
        }
    }
}

/// Handle to the overlay thread.
pub struct OverlayHandle {
    hwnd: Arc<AtomicIsize>,
    thread: Option<JoinHandle<()>>,
}

impl OverlayHandle {
    pub fn hwnd(&self) -> Hwnd {
        self.hwnd.load(Ordering::SeqCst)
    }

    /// Ask the overlay to re-read shared state and repaint.
    pub fn notify(&self) {
        let hwnd = self.hwnd();
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd, WM_APP_REFRESH, 0, 0);
            }
        }
    }

    /// Re-register hotkeys after the user changes them.
    pub fn reload_hotkeys(&self) {
        let hwnd = self.hwnd();
        if let Some(rt) = runtime() {
            register_hotkeys(hwnd, &rt);
        }
    }

    pub fn stop(&mut self) {
        let hwnd = self.hwnd();
        if hwnd != 0 {
            unsafe {
                PostMessageW(hwnd, WM_APP_QUIT, 0, 0);
            }
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for OverlayHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Start the overlay thread. `wake` is called whenever the overlay changes
/// shared state (a hotkey press) so the editor can repaint.
pub fn spawn(data: Arc<Mutex<AppData>>, wake: Arc<dyn Fn() + Send + Sync>) -> OverlayHandle {
    let hwnd_slot = Arc::new(AtomicIsize::new(0));
    let slot = hwnd_slot.clone();
    let thread = std::thread::Builder::new()
        .name("crosshair-overlay".to_string())
        .spawn(move || {
            let Some(hwnd) = create_window() else {
                return;
            };
            OVERLAY_HWND.store(hwnd, Ordering::SeqCst);
            slot.store(hwnd, Ordering::SeqCst);

            let rt = Arc::new(Runtime {
                data,
                wake,
                surface: Mutex::new(None),
                placed: Mutex::new(None),
            });
            if let Ok(mut global) = RUNTIME.lock() {
                *global = Some(rt.clone());
            }
            register_hotkeys(hwnd, &rt);
            refresh(hwnd);

            let mut msg = Msg::default();
            loop {
                let result = unsafe { GetMessageW(&mut msg, 0, 0, 0) };
                if result <= 0 {
                    break;
                }
                unsafe {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }

            for id in [
                HOTKEY_TOGGLE,
                HOTKEY_LEFT,
                HOTKEY_RIGHT,
                HOTKEY_UP,
                HOTKEY_DOWN,
            ] {
                unsafe {
                    UnregisterHotKey(hwnd, id);
                }
            }
            OVERLAY_HWND.store(0, Ordering::SeqCst);
            slot.store(0, Ordering::SeqCst);
            if let Ok(mut global) = RUNTIME.lock() {
                *global = None;
            }
        })
        .ok();

    OverlayHandle {
        hwnd: hwnd_slot,
        thread,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_letters_digits_and_function_keys() {
        assert_eq!(vk_from_label("Z"), Some(0x5A));
        assert_eq!(vk_from_label("z"), Some(0x5A));
        assert_eq!(vk_from_label("1"), Some(0x31));
        assert_eq!(vk_from_label("F9"), Some(0x78));
        assert_eq!(vk_from_label("F24"), Some(0x87));
        assert_eq!(vk_from_label("Space"), Some(0x20));
        assert_eq!(vk_from_label("nope"), None);
    }
}
