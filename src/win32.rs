//! Minimal, hand-written Win32 bindings.
//!
//! The app only needs a dozen calls, so declaring them directly keeps the build
//! free of large binding crates and makes the ABI explicit. 64-bit Windows only
//! (the release workflow builds `x86_64-pc-windows-msvc`).

#![allow(non_camel_case_types, non_snake_case, dead_code)]

use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;

pub type Hwnd = isize;
pub type Hmonitor = isize;
pub type Hdc = isize;
pub type Hbitmap = isize;
pub type Hgdiobj = isize;
pub type Hinstance = isize;
pub type Hkey = isize;
pub type Lresult = isize;

pub type WndProc = extern "system" fn(Hwnd, u32, usize, isize) -> Lresult;
pub type EnumWindowsProc = extern "system" fn(Hwnd, isize) -> i32;
pub type MonitorEnumProc = extern "system" fn(Hmonitor, Hdc, *mut Rect, isize) -> i32;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Size {
    pub cx: i32,
    pub cy: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Msg {
    pub hwnd: Hwnd,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt: Point,
    pub private: u32,
}

#[repr(C)]
pub struct WndClassExW {
    pub cb_size: u32,
    pub style: u32,
    pub wnd_proc: Option<WndProc>,
    pub cls_extra: i32,
    pub wnd_extra: i32,
    pub instance: Hinstance,
    pub icon: isize,
    pub cursor: isize,
    pub background: isize,
    pub menu_name: *const u16,
    pub class_name: *const u16,
    pub icon_small: isize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BlendFunction {
    pub blend_op: u8,
    pub blend_flags: u8,
    pub source_constant_alpha: u8,
    pub alpha_format: u8,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct BitmapInfoHeader {
    pub bi_size: u32,
    pub bi_width: i32,
    pub bi_height: i32,
    pub bi_planes: u16,
    pub bi_bit_count: u16,
    pub bi_compression: u32,
    pub bi_size_image: u32,
    pub bi_x_pels_per_meter: i32,
    pub bi_y_pels_per_meter: i32,
    pub bi_clr_used: u32,
    pub bi_clr_important: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct BitmapInfo {
    pub header: BitmapInfoHeader,
    pub colors: [u32; 1],
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct MonitorInfo {
    pub cb_size: u32,
    pub rc_monitor: Rect,
    pub rc_work: Rect,
    pub flags: u32,
}

pub const WS_POPUP: u32 = 0x8000_0000;
pub const WS_EX_LAYERED: u32 = 0x0008_0000;
pub const WS_EX_TRANSPARENT: u32 = 0x0000_0020;
pub const WS_EX_TOPMOST: u32 = 0x0000_0008;
pub const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
pub const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

pub const SW_HIDE: i32 = 0;
pub const SW_SHOWNOACTIVATE: i32 = 4;

pub const HWND_TOPMOST: Hwnd = -1;
pub const SWP_NOACTIVATE: u32 = 0x0010;
pub const SWP_SHOWWINDOW: u32 = 0x0040;
pub const SWP_NOSIZE: u32 = 0x0001;

pub const ULW_ALPHA: u32 = 0x0000_0002;
pub const AC_SRC_OVER: u8 = 0x00;
pub const AC_SRC_ALPHA: u8 = 0x01;
pub const BI_RGB: u32 = 0;
pub const DIB_RGB_COLORS: u32 = 0;
pub const SRCCOPY: u32 = 0x00CC_0020;

pub const WM_DESTROY: u32 = 0x0002;
pub const WM_CLOSE: u32 = 0x0010;
pub const WM_DISPLAYCHANGE: u32 = 0x007E;
pub const WM_HOTKEY: u32 = 0x0312;
pub const WM_APP: u32 = 0x8000;
pub const WM_APP_REFRESH: u32 = WM_APP + 1;
pub const WM_APP_QUIT: u32 = WM_APP + 2;

pub const MOD_ALT: u32 = 0x0001;
pub const MOD_CONTROL: u32 = 0x0002;
pub const MOD_SHIFT: u32 = 0x0004;
pub const MOD_WIN: u32 = 0x0008;
pub const MOD_NOREPEAT: u32 = 0x4000;

pub const MONITORINFOF_PRIMARY: u32 = 0x0000_0001;

pub const GWL_EXSTYLE: i32 = -20;

pub const VK_SHIFT: u8 = 0x10;
pub const VK_CONTROL: u8 = 0x11;
pub const VK_MENU: u8 = 0x12;
pub const VK_LWIN: u8 = 0x5B;
pub const KEYEVENTF_KEYUP: u32 = 0x0002;
pub const KEYEVENTF_EXTENDEDKEY: u32 = 0x0001;

/// Synthesise a chord press+release (used by the self-test to prove that the
/// registered global hotkey actually fires).
pub fn send_hotkey(vk: u8, ctrl: bool, alt: bool, shift: bool, win: bool) {
    // Real keyboards hold modifiers for tens of milliseconds; injected events with
    // no gap can arrive faster than hotkey tracking registers them.
    let gap = || std::thread::sleep(std::time::Duration::from_millis(15));
    // Arrows, Home/End/PgUp/PgDn, Insert/Delete are extended keys; a synthetic
    // press without the extended flag does not always match a registered hotkey.
    let extended = matches!(vk, 0x21..=0x28 | 0x2D..=0x2E);
    unsafe {
        if ctrl {
            keybd_event(VK_CONTROL, 0, 0, 0);
            gap();
        }
        if alt {
            keybd_event(VK_MENU, 0, 0, 0);
            gap();
        }
        if shift {
            keybd_event(VK_SHIFT, 0, 0, 0);
            gap();
        }
        if win {
            keybd_event(VK_LWIN, 0, 0, 0);
        }
        let flags = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
        gap();
        keybd_event(vk, 0, flags, 0);
        gap();
        keybd_event(vk, 0, flags | KEYEVENTF_KEYUP, 0);
        gap();
        if win {
            keybd_event(VK_LWIN, 0, KEYEVENTF_KEYUP, 0);
        }
        if shift {
            keybd_event(VK_SHIFT, 0, KEYEVENTF_KEYUP, 0);
        }
        if alt {
            keybd_event(VK_MENU, 0, KEYEVENTF_KEYUP, 0);
        }
        if ctrl {
            keybd_event(VK_CONTROL, 0, KEYEVENTF_KEYUP, 0);
        }
    }
}

pub const ERROR_ALREADY_EXISTS: u32 = 183;

pub const HKEY_CURRENT_USER: Hkey = 0x8000_0001u32 as i32 as isize;
pub const KEY_SET_VALUE: u32 = 0x0002;
pub const REG_SZ: u32 = 1;
pub const REG_OPTION_NON_VOLATILE: u32 = 0;
pub const ATTACH_PARENT_PROCESS: u32 = 0xFFFF_FFFF;

/// Per-monitor DPI awareness v2 context handle.
pub const DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2: isize = -4;

#[link(name = "kernel32")]
extern "system" {
    pub fn GetModuleHandleW(name: *const u16) -> Hinstance;
    pub fn LoadLibraryW(name: *const u16) -> Hinstance;
    pub fn GetProcAddress(module: Hinstance, name: *const u8) -> *mut c_void;
    pub fn FreeLibrary(module: Hinstance) -> i32;
    pub fn CreateMutexW(attrs: *const c_void, owner: i32, name: *const u16) -> isize;
    pub fn GetConsoleWindow() -> Hwnd;
}

#[link(name = "user32")]
extern "system" {
    pub fn RegisterClassExW(class: *const WndClassExW) -> u16;
    pub fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: isize,
        instance: Hinstance,
        param: *mut c_void,
    ) -> Hwnd;
    pub fn DefWindowProcW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> Lresult;
    pub fn DestroyWindow(hwnd: Hwnd) -> i32;
    pub fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    pub fn SetWindowPos(
        hwnd: Hwnd,
        insert_after: Hwnd,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        flags: u32,
    ) -> i32;
    pub fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    pub fn TranslateMessage(msg: *const Msg) -> i32;
    pub fn DispatchMessageW(msg: *const Msg) -> Lresult;
    pub fn PostMessageW(hwnd: Hwnd, msg: u32, wparam: usize, lparam: isize) -> i32;
    pub fn PostQuitMessage(code: i32);
    pub fn RegisterHotKey(hwnd: Hwnd, id: i32, modifiers: u32, vk: u32) -> i32;
    pub fn UnregisterHotKey(hwnd: Hwnd, id: i32) -> i32;
    pub fn EnumDisplayMonitors(
        hdc: Hdc,
        clip: *const Rect,
        callback: MonitorEnumProc,
        data: isize,
    ) -> i32;
    pub fn GetMonitorInfoW(monitor: Hmonitor, info: *mut MonitorInfo) -> i32;
    pub fn SetProcessDpiAwarenessContext(context: isize) -> i32;
    pub fn GetDpiForWindow(hwnd: Hwnd) -> u32;
    pub fn GetSystemMetrics(index: i32) -> i32;
    pub fn GetDC(hwnd: Hwnd) -> Hdc;
    pub fn ReleaseDC(hwnd: Hwnd, dc: Hdc) -> i32;
    pub fn UpdateLayeredWindow(
        hwnd: Hwnd,
        dst_dc: Hdc,
        dst_point: *const Point,
        size: *const Size,
        src_dc: Hdc,
        src_point: *const Point,
        color_key: u32,
        blend: *const BlendFunction,
        flags: u32,
    ) -> i32;
    pub fn AttachConsole(process_id: u32) -> i32;
    pub fn MessageBeep(kind: u32) -> i32;
    pub fn keybd_event(vk: u8, scan: u8, flags: u32, extra: usize);
    pub fn WindowFromPoint(point: Point) -> Hwnd;
    pub fn GetForegroundWindow() -> Hwnd;
    pub fn EnumWindows(callback: EnumWindowsProc, data: isize) -> i32;
    pub fn GetWindowTextW(hwnd: Hwnd, buffer: *mut u16, max: i32) -> i32;
    pub fn IsWindowVisible(hwnd: Hwnd) -> i32;
    pub fn SetForegroundWindow(hwnd: Hwnd) -> i32;
    pub fn GetWindowRect(hwnd: Hwnd, rect: *mut Rect) -> i32;
    pub fn GetClassNameW(hwnd: Hwnd, buffer: *mut u16, max: i32) -> i32;
    pub fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
}

#[link(name = "gdi32")]
extern "system" {
    pub fn CreateCompatibleDC(dc: Hdc) -> Hdc;
    pub fn CreateDIBSection(
        dc: Hdc,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: isize,
        offset: u32,
    ) -> Hbitmap;
    pub fn SelectObject(dc: Hdc, object: Hgdiobj) -> Hgdiobj;
    pub fn DeleteObject(object: Hgdiobj) -> i32;
    pub fn DeleteDC(dc: Hdc) -> i32;
    pub fn BitBlt(
        dst: Hdc,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        src: Hdc,
        src_x: i32,
        src_y: i32,
        rop: u32,
    ) -> i32;
}

#[link(name = "advapi32")]
extern "system" {
    pub fn RegCreateKeyExW(
        key: Hkey,
        sub_key: *const u16,
        reserved: u32,
        class: *const u16,
        options: u32,
        sam: u32,
        security: *const c_void,
        result: *mut Hkey,
        disposition: *mut u32,
    ) -> i32;
    pub fn RegSetValueExW(
        key: Hkey,
        value_name: *const u16,
        reserved: u32,
        kind: u32,
        data: *const u8,
        data_len: u32,
    ) -> i32;
    pub fn RegDeleteValueW(key: Hkey, value_name: *const u16) -> i32;
    pub fn RegQueryValueExW(
        key: Hkey,
        value_name: *const u16,
        reserved: *mut u32,
        kind: *mut u32,
        data: *mut u8,
        data_len: *mut u32,
    ) -> i32;
    pub fn RegCloseKey(key: Hkey) -> i32;
}

/// Find a visible top-level window by exact title.
pub fn find_window_by_title(title: &str) -> Hwnd {
    extern "system" fn callback(hwnd: Hwnd, data: isize) -> i32 {
        let wanted = unsafe { &*(data as *const String) };
        if unsafe { IsWindowVisible(hwnd) } == 0 {
            return 1;
        }
        let mut buffer = [0u16; 512];
        let len = unsafe { GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
        if len > 0 && from_wide(&buffer[..len as usize]) == *wanted {
            unsafe {
                *(data as *mut isize) = hwnd;
            }
            return 0;
        }
        1
    }
    let mut state = (title.to_string(), 0isize);
    unsafe {
        EnumWindows(callback, &mut state as *mut _ as isize);
    }
    state.1
}

/// Null-terminated UTF-16 literal.
pub fn wide(text: &str) -> Vec<u16> {
    OsStr::new(text)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Turn a UTF-16 buffer back into a `String`.
pub fn from_wide(buf: &[u16]) -> String {
    let end = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..end])
}

/// Opt the process into per-monitor DPI awareness so the overlay lands on
/// physical pixels on any monitor.
pub fn set_dpi_aware() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Attach to the launching terminal so a console-subsystem release build can
/// still print self-test output.
pub fn attach_console() {
    unsafe {
        if GetConsoleWindow() == 0 {
            AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }
}

/// DPI of a monitor in dots per inch (96 when the query is unavailable).
pub fn monitor_dpi(monitor: Hmonitor) -> u32 {
    type GetDpiForMonitorFn = unsafe extern "system" fn(Hmonitor, i32, *mut u32, *mut u32) -> i32;
    static LOADED: std::sync::OnceLock<Option<GetDpiForMonitorFn>> = std::sync::OnceLock::new();
    let function = LOADED.get_or_init(|| unsafe {
        let module = LoadLibraryW(wide("shcore.dll").as_ptr());
        if module == 0 {
            return None;
        }
        let name = b"GetDpiForMonitor ";
        let address = GetProcAddress(module, name.as_ptr());
        if address.is_null() {
            None
        } else {
            Some(std::mem::transmute::<*mut c_void, GetDpiForMonitorFn>(
                address,
            ))
        }
    });
    if let Some(func) = function {
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        // MDT_EFFECTIVE_DPI = 0
        unsafe {
            if func(monitor, 0, &mut dpi_x, &mut dpi_y) == 0 && dpi_x > 0 {
                return dpi_x;
            }
        }
    }
    96
}

/// Copy a screen region into a BGRA buffer (used by the self-test to prove the
/// overlay is actually composited on screen).
pub fn capture_screen(x: i32, y: i32, w: i32, h: i32) -> Option<Vec<u8>> {
    if w <= 0 || h <= 0 {
        return None;
    }
    unsafe {
        let screen = GetDC(0);
        if screen == 0 {
            return None;
        }
        let memory = CreateCompatibleDC(screen);
        let info = BitmapInfo {
            header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: w,
                bi_height: -h,
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                ..Default::default()
            },
            colors: [0],
        };
        let mut bits: *mut c_void = std::ptr::null_mut();
        let bitmap = CreateDIBSection(memory, &info, DIB_RGB_COLORS, &mut bits, 0, 0);
        if bitmap == 0 || bits.is_null() {
            DeleteDC(memory);
            ReleaseDC(0, screen);
            return None;
        }
        let old = SelectObject(memory, bitmap);
        let ok = BitBlt(memory, 0, 0, w, h, screen, x, y, SRCCOPY);
        let mut out = None;
        if ok != 0 {
            let len = (w as usize) * (h as usize) * 4;
            let slice = std::slice::from_raw_parts(bits as *const u8, len);
            out = Some(slice.to_vec());
        }
        SelectObject(memory, old);
        DeleteObject(bitmap);
        DeleteDC(memory);
        ReleaseDC(0, screen);
        out
    }
}
