//! `--self-test`: checks that do not need a human at the keyboard.
//!
//! It verifies the renderer, the preset round-trip and — most importantly — that
//! the layered window is really composited on screen, is click-through and goes
//! away when the overlay is hidden. Run with `crosshairx-open.exe --self-test`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use crosshair_core::{
    preset, render, store, Crosshair, DotShape, Layer, Point, RenderOptions, Settings, Store,
};

use crate::overlay;
use crate::startup;
use crate::win32::*;

fn count_green(region: (i32, i32, i32, i32)) -> usize {
    let (x, y, w, h) = region;
    match capture_screen(x, y, w, h) {
        Some(bytes) => bytes
            .as_chunks::<4>()
            .0
            .iter()
            // BGRA
            .filter(|p| p[1] > 170 && p[2] < 110 && p[0] < 110)
            .count(),
        None => 0,
    }
}

fn class_name(hwnd: Hwnd) -> String {
    if hwnd == 0 {
        return "<null>".to_string();
    }
    let mut buffer = [0u16; 256];
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    if len <= 0 {
        "<unknown>".to_string()
    } else {
        from_wide(&buffer[..len as usize])
    }
}

pub fn run() -> i32 {
    let mut failures = 0;
    println!(
        "=== CrosshairX-Open {} self-test ===",
        crosshair_core::VERSION
    );
    println!();

    // ---- displays -------------------------------------------------------
    let displays = overlay::enumerate_displays();
    println!("[info] {} display(s) detected", displays.len());
    for display in &displays {
        println!(
            "       {} at ({}, {}) work area {}x{}",
            display.label(),
            display.x,
            display.y,
            display.width,
            display.height
        );
    }
    if displays.is_empty() {
        failures += 1;
        println!("[FAIL] no displays were enumerated");
    }
    println!();

    // ---- renderer -------------------------------------------------------
    for design in store::builtin_presets() {
        match render(&design, &RenderOptions::default()) {
            Some(rendered) => {
                let opaque = rendered
                    .pixmap
                    .data()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .filter(|p| p[3] > 0)
                    .count();
                if opaque == 0 {
                    failures += 1;
                    println!("[FAIL] preset '{}' rendered nothing", design.name);
                } else {
                    println!(
                        "[ ok ] preset {:<20} {}x{} px, {} non-transparent px",
                        design.name,
                        rendered.width(),
                        rendered.height(),
                        opaque
                    );
                }
            }
            None => {
                failures += 1;
                println!(
                    "[FAIL] preset '{}' could not allocate a pixmap",
                    design.name
                );
            }
        }
    }
    println!();

    // ---- preset import / export ----------------------------------------
    let dir = std::env::temp_dir().join("crosshairx-open-selftest");
    let mut store = Store::load_from(&dir);
    store.presets = store::builtin_presets();
    let export_path = dir.join("roundtrip.json");
    match store.export_preset("Triangle Ticks", &export_path) {
        Ok(()) => match preset::import_file(&export_path) {
            Ok(imported) => {
                if imported.len() == 1 && imported[0].name == "Triangle Ticks" {
                    println!(
                        "[ ok ] preset export/import round-trip ({})",
                        imported.len()
                    );
                } else {
                    failures += 1;
                    println!("[FAIL] round-trip returned {} preset(s)", imported.len());
                }
            }
            Err(error) => {
                failures += 1;
                println!("[FAIL] import after export: {error}");
            }
        },
        Err(error) => {
            failures += 1;
            println!("[FAIL] export: {error}");
        }
    }

    // Community-format import (the payload shape other crosshair apps use).
    let community = r##"{"crosshairId":[{"dot":{"diameter":4,"opacity":1,"color":"#FF0000",
        "shape":"ring","thickness":1,"blur":0},"line":{"length":8,"thickness":2,"offset":8,
        "opacity":1,"color":"Red","shape":"rectangle","rotation":0,"blur":0,
        "visible":["left","right","top","bottom"]},"outline":{"thickness":0,"opacity":1,
        "color":"Black","blur":0},"position":{"x":0,"y":0},"layer":{"size":1}}],
        "name":"Community Sample","timestamp":1607035094735}"##;
    match preset::import_str(community) {
        Ok(imported) if imported.len() == 1 => {
            println!(
                "[ ok ] community payload import ('{}', {} layer(s))",
                imported[0].name,
                imported[0].layers.len()
            );
        }
        Ok(other) => {
            failures += 1;
            println!("[FAIL] community import produced {} presets", other.len());
        }
        Err(error) => {
            failures += 1;
            println!("[FAIL] community import: {error}");
        }
    }
    println!();

    // ---- overlay on screen ---------------------------------------------
    let display = match displays.first().copied() {
        Some(display) => display,
        None => return failures + 1,
    };
    let target = (
        display.x + display.width / 2,
        display.y + display.height / 2,
    );
    let region = (target.0 - 60, target.1 - 60, 120, 120);

    let baseline = count_green(region);
    println!("[info] green pixels at screen centre before overlay: {baseline}");

    let mut design = Layer::default();
    design.line.length = 20.0;
    design.line.thickness = 3.0;
    design.line.offset = 6.0;
    design.line.color = "#00FF00".to_string();
    design.dot.shape = DotShape::Circle;
    design.dot.diameter = 6.0;
    design.dot.opacity = 1.0;
    design.dot.color = "#00FF00".to_string();
    design.outline.thickness = 1.0;
    design.outline.opacity = 1.0;
    design.outline.color = "#000000".to_string();

    let mut data = overlay::AppData::new(&Store::load_from(&dir));
    data.settings = Settings {
        overlay_visible: true,
        monitor: 0,
        offset: Point::ZERO,
        scale: 1.0,
        global_opacity: 1.0,
        ignore_dpi: true,
        ..Settings::default()
    };
    data.current = Crosshair::single("self-test", design);
    let shared = Arc::new(Mutex::new(data));

    let overlay = overlay::spawn(shared.clone(), Arc::new(|| {}));
    for _ in 0..80 {
        if overlay.hwnd() != 0 {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
    }
    let hwnd = overlay.hwnd();
    if hwnd == 0 {
        failures += 1;
        println!("[FAIL] the overlay window was not created");
        return failures;
    }
    println!(
        "[ ok ] overlay window created (hwnd {hwnd:#x}, class '{}')",
        class_name(hwnd)
    );

    overlay.notify();
    std::thread::sleep(Duration::from_millis(700));
    let shown = count_green(region);
    println!("[info] green pixels with overlay shown: {shown}");
    if shown > baseline + 20 {
        println!(
            "[ ok ] the crosshair is composited on screen (+{} px)",
            shown - baseline
        );
    } else {
        failures += 1;
        println!("[FAIL] the crosshair does not appear on screen");
    }

    let ex_style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) } as u32;
    for (flag, name) in [
        (WS_EX_LAYERED, "WS_EX_LAYERED"),
        (WS_EX_TRANSPARENT, "WS_EX_TRANSPARENT (click-through)"),
        (WS_EX_TOPMOST, "WS_EX_TOPMOST"),
        (WS_EX_NOACTIVATE, "WS_EX_NOACTIVATE"),
    ] {
        if ex_style & flag != 0 {
            println!("[ ok ] window style has {name}");
        } else {
            failures += 1;
            println!("[FAIL] window style is missing {name}");
        }
    }

    let hit = unsafe {
        WindowFromPoint(crate::win32::Point {
            x: target.0,
            y: target.1,
        })
    };
    if hit == hwnd {
        failures += 1;
        println!("[FAIL] WindowFromPoint returned the overlay, so clicks would not pass through");
    } else {
        println!(
            "[ ok ] clicks pass through (WindowFromPoint -> class '{}')",
            class_name(hit)
        );
    }

    // Hiding through shared state must remove it from the screen.
    if let Ok(mut data) = shared.lock() {
        data.settings.overlay_visible = false;
    }
    overlay.notify();
    std::thread::sleep(Duration::from_millis(500));
    let hidden = count_green(region);
    println!("[info] green pixels after hiding: {hidden}");
    if hidden <= baseline + 20 {
        println!("[ ok ] hiding the overlay removes it from the screen");
    } else {
        failures += 1;
        println!("[FAIL] the overlay is still visible after being hidden");
    }

    // Showing again must bring it back.
    if let Ok(mut data) = shared.lock() {
        data.settings.overlay_visible = true;
        data.settings.offset = Point::new(20.0, 0.0);
    }
    overlay.notify();
    std::thread::sleep(Duration::from_millis(500));
    let offset_region = (target.0 + 12, target.1 - 40, 60, 80);
    let moved = count_green(offset_region);
    if moved > 10 {
        println!("[ ok ] offset moves the crosshair (+{} px at x+20)", moved);
    } else {
        failures += 1;
        println!("[FAIL] the crosshair did not move with the offset");
    }

    // Global hotkey: synthesise Alt+Shift+Z and expect the overlay to toggle.
    let toggle = Settings::default().toggle_hotkey.clone();
    if let Some(vk) = overlay::vk_from_label(&toggle.key) {
        let before = shared
            .lock()
            .map(|d| d.settings.overlay_visible)
            .unwrap_or(true);
        send_hotkey(vk as u8, toggle.ctrl, toggle.alt, toggle.shift, toggle.win);
        std::thread::sleep(Duration::from_millis(400));
        let after = shared
            .lock()
            .map(|d| d.settings.overlay_visible)
            .unwrap_or(true);
        if after != before {
            println!(
                "[ ok ] global hotkey {} toggled the overlay",
                toggle.describe()
            );
        } else {
            failures += 1;
            println!(
                "[FAIL] global hotkey {} did not toggle the overlay",
                toggle.describe()
            );
        }
        let screen_after_toggle = count_green(region);
        if after {
            if screen_after_toggle > baseline + 20 {
                println!("[ ok ] overlay is back on screen after the hotkey");
            } else {
                failures += 1;
                println!("[FAIL] overlay did not return after the hotkey");
            }
        } else if screen_after_toggle <= baseline + 20 {
            println!("[ ok ] hotkey hid the overlay from the screen");
        } else {
            failures += 1;
            println!("[FAIL] hotkey did not hide the overlay");
        }
    }

    if let Ok(data) = shared.lock() {
        for line in &data.hotkey_log {
            println!("[info] hotkey: {line}");
        }
    }

    // Position hotkey: Alt+Shift+Right must move the crosshair by the step.
    let before_offset = shared
        .lock()
        .map(|d| d.settings.offset.x)
        .unwrap_or(f32::NAN);
    let mut after_offset = before_offset;
    for _ in 0..3 {
        send_hotkey(0x27, false, true, true, false);
        std::thread::sleep(Duration::from_millis(300));
        after_offset = shared
            .lock()
            .map(|d| d.settings.offset.x)
            .unwrap_or(f32::NAN);
        if (after_offset - before_offset - 1.0).abs() < 0.01 {
            break;
        }
    }
    if (after_offset - before_offset - 1.0).abs() < 0.01 {
        println!(
            "[ ok ] position hotkey moved the crosshair 1px (x {before_offset} -> {after_offset})"
        );
    } else {
        failures += 1;
        println!(
            "[FAIL] position hotkey did not move the overlay (x {before_offset} -> {after_offset})"
        );
    }

    // Start with Windows: write the Run entry, read it back, restore the state.
    let was_enabled = startup::is_launch_on_startup();
    match startup::set_launch_on_startup(true) {
        Ok(()) if startup::is_launch_on_startup() => {
            println!("[ ok ] autostart entry written to HKCU Run and read back");
        }
        other => {
            failures += 1;
            println!("[FAIL] could not write the autostart entry: {other:?}");
        }
    }
    let restored = startup::set_launch_on_startup(was_enabled);
    if restored.is_ok() && startup::is_launch_on_startup() == was_enabled {
        println!(
            "[ ok ] autostart entry restored to its previous state ({})",
            if was_enabled { "enabled" } else { "disabled" }
        );
    } else {
        failures += 1;
        println!("[FAIL] could not restore the autostart entry: {restored:?}");
    }

    let mut overlay = overlay;
    overlay.stop();
    std::thread::sleep(Duration::from_millis(300));
    let after_stop = count_green(region);
    if after_stop <= baseline + 20 {
        println!("[ ok ] stopping the overlay thread tears the window down");
    } else {
        failures += 1;
        println!("[FAIL] the overlay window survived being stopped");
    }

    println!();
    if failures == 0 {
        println!("=== all checks passed ===");
    } else {
        println!("=== {failures} check(s) failed ===");
    }
    failures
}
